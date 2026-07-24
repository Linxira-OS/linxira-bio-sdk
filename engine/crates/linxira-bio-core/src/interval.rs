use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Maximum deterministic work units for one intersection call. One unit is
/// charged per sweep event and active-pair comparison; sorting is budgeted as
/// `event_count * ceil(log2(event_count))` additional units. The call fails
/// without returning partial statistics when the budget would be exceeded.
pub const MAX_BED_INTERSECTION_WORK_UNITS: u64 = 50_000_000;

/// Maximum number of overlap pairs represented by one intersection result.
/// Inputs producing more pairs must be partitioned by contig or genomic region.
pub const MAX_BED_OVERLAP_PAIRS: u64 = 5_000_000;

/// Maximum bytes emitted by a decoder for either BED input. Applying this to
/// the decoded stream bounds both plain-text inputs and gzip expansion.
pub const MAX_BED_DECOMPRESSED_BYTES_PER_INPUT: u64 = 128 * 1024 * 1024;

/// Maximum number of intervals retained across both BED inputs.
pub const MAX_BED_INTERVAL_RECORDS: u64 = 2_000_000;

/// Maximum deterministic estimate of memory retained across both BED inputs.
/// The estimate charges every interval plus container overhead and each unique
/// chromosome key. The record limit remains an independent hard bound.
pub const MAX_BED_RETAINED_INPUT_BYTES: u64 = 256 * 1024 * 1024;

const RETAINED_BYTES_PER_INTERVAL: u64 =
    (std::mem::size_of::<Interval>() + std::mem::size_of::<usize>() * 2) as u64;
const RETAINED_BYTES_PER_CONTIG: u64 =
    (std::mem::size_of::<String>() + std::mem::size_of::<Vec<Interval>>() + 64) as u64;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalContigStats {
    pub left_interval_count: u64,
    pub right_interval_count: u64,
    pub overlap_pair_count: u64,
    pub left_overlapped_count: u64,
    pub right_overlapped_count: u64,
    pub total_overlap_bases: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalIntersectStats {
    pub left_interval_count: u64,
    pub right_interval_count: u64,
    pub overlap_pair_count: u64,
    pub left_overlapped_count: u64,
    pub right_overlapped_count: u64,
    pub total_overlap_bases: u64,
    pub contigs: BTreeMap<String, IntervalContigStats>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum BedError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    LimitExceeded { resource: &'static str, limit: u64 },
    Overflow,
}

impl Display for BedError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read BED: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read BED at line {line}: {source}")
            }
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed BED record at line {line}: {message}")
            }
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "BED processing exceeds the deterministic {resource} limit of {limit}"
            ),
            Self::Overflow => formatter.write_str("interval intersection exceeds supported range"),
        }
    }
}

impl Error for BedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            Self::MalformedRecord { .. } | Self::LimitExceeded { .. } | Self::Overflow => None,
        }
    }
}

impl From<io::Error> for BedError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct Interval {
    start: u64,
    end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EventKind {
    LeftEnd,
    RightEnd,
    LeftStart,
    RightStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Event {
    position: u64,
    kind: EventKind,
    index: usize,
}

#[derive(Debug)]
struct IntersectionBudget {
    work_units: u64,
    overlap_pairs: u64,
    max_work_units: u64,
    max_overlap_pairs: u64,
}

#[derive(Debug)]
struct BedReadBudget {
    interval_records: u64,
    retained_input_bytes: u64,
    max_interval_records: u64,
    max_retained_input_bytes: u64,
}

impl BedReadBudget {
    fn production() -> Self {
        Self::new(MAX_BED_INTERVAL_RECORDS, MAX_BED_RETAINED_INPUT_BYTES)
    }

    fn new(max_interval_records: u64, max_retained_input_bytes: u64) -> Self {
        Self {
            interval_records: 0,
            retained_input_bytes: 0,
            max_interval_records,
            max_retained_input_bytes,
        }
    }

    fn reserve_interval(
        &mut self,
        contig_length: usize,
        is_new_contig: bool,
        work_budget: &mut IntersectionBudget,
    ) -> Result<(), BedError> {
        let interval_records =
            self.interval_records
                .checked_add(1)
                .ok_or(BedError::LimitExceeded {
                    resource: "input interval record count",
                    limit: self.max_interval_records,
                })?;
        if interval_records > self.max_interval_records {
            return Err(BedError::LimitExceeded {
                resource: "input interval record count",
                limit: self.max_interval_records,
            });
        }

        let contig_bytes = if is_new_contig {
            u64::try_from(contig_length)
                .ok()
                .and_then(|length| length.checked_add(RETAINED_BYTES_PER_CONTIG))
                .ok_or(BedError::LimitExceeded {
                    resource: "retained input byte estimate",
                    limit: self.max_retained_input_bytes,
                })?
        } else {
            0
        };
        let retained_input_bytes = self
            .retained_input_bytes
            .checked_add(RETAINED_BYTES_PER_INTERVAL)
            .and_then(|bytes| bytes.checked_add(contig_bytes))
            .ok_or(BedError::LimitExceeded {
                resource: "retained input byte estimate",
                limit: self.max_retained_input_bytes,
            })?;
        if retained_input_bytes > self.max_retained_input_bytes {
            return Err(BedError::LimitExceeded {
                resource: "retained input byte estimate",
                limit: self.max_retained_input_bytes,
            });
        }

        // Charge parsing before allocating the owned chromosome key or
        // extending a retained interval vector.
        work_budget.reserve_work(1)?;
        self.interval_records = interval_records;
        self.retained_input_bytes = retained_input_bytes;
        Ok(())
    }
}

impl IntersectionBudget {
    fn production() -> Self {
        Self::new(MAX_BED_INTERSECTION_WORK_UNITS, MAX_BED_OVERLAP_PAIRS)
    }

    fn new(max_work_units: u64, max_overlap_pairs: u64) -> Self {
        Self {
            work_units: 0,
            overlap_pairs: 0,
            max_work_units,
            max_overlap_pairs,
        }
    }

    fn reserve_work(&mut self, count: u64) -> Result<(), BedError> {
        let work_units = self
            .work_units
            .checked_add(count)
            .ok_or(BedError::LimitExceeded {
                resource: "work-unit",
                limit: self.max_work_units,
            })?;
        if work_units > self.max_work_units {
            return Err(BedError::LimitExceeded {
                resource: "work-unit",
                limit: self.max_work_units,
            });
        }
        self.work_units = work_units;
        Ok(())
    }

    fn reserve_overlapping_pairs(&mut self, count: usize) -> Result<(), BedError> {
        let count = u64::try_from(count).map_err(|_| BedError::LimitExceeded {
            resource: "work-unit",
            limit: self.max_work_units,
        })?;
        let work_units = self
            .work_units
            .checked_add(count)
            .ok_or(BedError::LimitExceeded {
                resource: "work-unit",
                limit: self.max_work_units,
            })?;
        if work_units > self.max_work_units {
            return Err(BedError::LimitExceeded {
                resource: "work-unit",
                limit: self.max_work_units,
            });
        }
        let overlap_pairs =
            self.overlap_pairs
                .checked_add(count)
                .ok_or(BedError::LimitExceeded {
                    resource: "overlap-pair",
                    limit: self.max_overlap_pairs,
                })?;
        if overlap_pairs > self.max_overlap_pairs {
            return Err(BedError::LimitExceeded {
                resource: "overlap-pair",
                limit: self.max_overlap_pairs,
            });
        }
        self.work_units = work_units;
        self.overlap_pairs = overlap_pairs;
        Ok(())
    }
}

pub fn bed_intersect_path(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
) -> Result<IntervalIntersectStats, BedError> {
    let mut work_budget = IntersectionBudget::production();
    let mut read_budget = BedReadBudget::production();
    let left = read_bed_path(left.as_ref(), &mut read_budget, &mut work_budget)?;
    let right = read_bed_path(right.as_ref(), &mut read_budget, &mut work_budget)?;
    intersect_interval_sets_with_budget(left, right, &mut work_budget)
}

fn read_bed_path(
    path: &Path,
    read_budget: &mut BedReadBudget,
    work_budget: &mut IntersectionBudget,
) -> Result<BTreeMap<String, Vec<Interval>>, BedError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    read_bed_with_limits(
        BufReader::new(input),
        MAX_BED_DECOMPRESSED_BYTES_PER_INPUT,
        read_budget,
        work_budget,
    )
}

#[cfg(test)]
fn read_bed(reader: impl BufRead) -> Result<BTreeMap<String, Vec<Interval>>, BedError> {
    let mut read_budget = BedReadBudget::production();
    let mut work_budget = IntersectionBudget::production();
    read_bed_with_limits(
        reader,
        MAX_BED_DECOMPRESSED_BYTES_PER_INPUT,
        &mut read_budget,
        &mut work_budget,
    )
}

fn read_bed_with_limits(
    reader: impl BufRead,
    max_decompressed_bytes: u64,
    read_budget: &mut BedReadBudget,
    work_budget: &mut IntersectionBudget,
) -> Result<BTreeMap<String, Vec<Interval>>, BedError> {
    let mut reader = reader.take(max_decompressed_bytes.saturating_add(1));
    let mut intervals: BTreeMap<String, Vec<Interval>> = BTreeMap::new();
    let mut line_number = 0_usize;
    let mut decompressed_bytes = 0_u64;
    let mut buffer = String::new();
    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| BedError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }
        decompressed_bytes =
            decompressed_bytes
                .checked_add(bytes_read as u64)
                .ok_or(BedError::LimitExceeded {
                    resource: "decompressed byte count per input",
                    limit: max_decompressed_bytes,
                })?;
        if decompressed_bytes > max_decompressed_bytes {
            return Err(BedError::LimitExceeded {
                resource: "decompressed byte count per input",
                limit: max_decompressed_bytes,
            });
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("track ")
            || line.starts_with("browser ")
        {
            continue;
        }
        // Only the first three BED columns are needed. Avoid collecting every
        // optional field so a delimiter-heavy line cannot amplify temporary
        // parser memory beyond the decoded-byte bound.
        let mut fields = line.split('\t');
        let contig = fields.next().unwrap_or_default();
        let Some(start_field) = fields.next() else {
            return malformed(
                line_number,
                "expected at least 3 tab-separated fields, found 1",
            );
        };
        let Some(end_field) = fields.next() else {
            return malformed(
                line_number,
                "expected at least 3 tab-separated fields, found 2",
            );
        };
        if contig.is_empty() {
            return malformed(line_number, "chromosome name is empty");
        }
        let start = parse_coordinate(start_field, line_number, "start")?;
        let end = parse_coordinate(end_field, line_number, "end")?;
        if end <= start {
            return malformed(
                line_number,
                format!("end coordinate {end} must be greater than start {start}"),
            );
        }
        let is_new_contig = !intervals.contains_key(contig);
        read_budget.reserve_interval(contig.len(), is_new_contig, work_budget)?;
        intervals
            .entry(contig.to_owned())
            .or_default()
            .push(Interval { start, end });
    }
    Ok(intervals)
}

fn parse_coordinate(value: &str, line: usize, name: &str) -> Result<u64, BedError> {
    value.parse::<u64>().map_err(|_| BedError::MalformedRecord {
        line,
        message: format!("invalid {name} coordinate {value:?}"),
    })
}

#[cfg(test)]
fn intersect_interval_sets(
    left: BTreeMap<String, Vec<Interval>>,
    right: BTreeMap<String, Vec<Interval>>,
) -> Result<IntervalIntersectStats, BedError> {
    let mut budget = IntersectionBudget::production();
    intersect_interval_sets_with_budget(left, right, &mut budget)
}

fn intersect_interval_sets_with_budget(
    left: BTreeMap<String, Vec<Interval>>,
    right: BTreeMap<String, Vec<Interval>>,
    budget: &mut IntersectionBudget,
) -> Result<IntervalIntersectStats, BedError> {
    let mut result = IntervalIntersectStats::default();
    let contigs = left
        .keys()
        .chain(right.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for contig in contigs {
        let left_intervals = left.get(&contig).map(Vec::as_slice).unwrap_or_default();
        let right_intervals = right.get(&contig).map(Vec::as_slice).unwrap_or_default();
        let stats = intersect_contig(left_intervals, right_intervals, budget)?;
        add_stats(&mut result, &stats)?;
        result.contigs.insert(contig, stats);
    }
    if result.left_interval_count == 0 || result.right_interval_count == 0 {
        result
            .warnings
            .push("one or both BED inputs contain no intervals".to_owned());
    } else if result.overlap_pair_count == 0 {
        result
            .warnings
            .push("the BED inputs have no overlapping half-open intervals".to_owned());
    }
    Ok(result)
}

fn intersect_contig(
    left: &[Interval],
    right: &[Interval],
    budget: &mut IntersectionBudget,
) -> Result<IntervalContigStats, BedError> {
    let interval_count = left
        .len()
        .checked_add(right.len())
        .ok_or(BedError::LimitExceeded {
            resource: "work-unit",
            limit: budget.max_work_units,
        })?;
    let event_count = interval_count
        .checked_mul(2)
        .ok_or(BedError::LimitExceeded {
            resource: "work-unit",
            limit: budget.max_work_units,
        })?;
    budget.reserve_work(sweep_event_work(event_count, budget.max_work_units)?)?;
    let mut events = Vec::with_capacity(event_count);
    for (index, interval) in left.iter().enumerate() {
        events.push(Event {
            position: interval.start,
            kind: EventKind::LeftStart,
            index,
        });
        events.push(Event {
            position: interval.end,
            kind: EventKind::LeftEnd,
            index,
        });
    }
    for (index, interval) in right.iter().enumerate() {
        events.push(Event {
            position: interval.start,
            kind: EventKind::RightStart,
            index,
        });
        events.push(Event {
            position: interval.end,
            kind: EventKind::RightEnd,
            index,
        });
    }
    events.sort_unstable();

    let mut active_left = BTreeSet::new();
    let mut active_right = BTreeSet::new();
    let mut left_overlapped = vec![false; left.len()];
    let mut right_overlapped = vec![false; right.len()];
    let mut stats = IntervalContigStats {
        left_interval_count: u64::try_from(left.len()).expect("interval count fits in u64"),
        right_interval_count: u64::try_from(right.len()).expect("interval count fits in u64"),
        ..Default::default()
    };

    for event in events {
        match event.kind {
            EventKind::LeftEnd => {
                active_left.remove(&event.index);
            }
            EventKind::RightEnd => {
                active_right.remove(&event.index);
            }
            EventKind::LeftStart => {
                // Every opposite interval still active at this start position
                // has a positive half-open overlap. Reserve the entire loop in
                // O(1) so dense inputs fail before expensive pair enumeration.
                budget.reserve_overlapping_pairs(active_right.len())?;
                for right_index in &active_right {
                    record_overlap(
                        left[event.index],
                        right[*right_index],
                        event.index,
                        *right_index,
                        &mut left_overlapped,
                        &mut right_overlapped,
                        &mut stats,
                    )?;
                }
                active_left.insert(event.index);
            }
            EventKind::RightStart => {
                budget.reserve_overlapping_pairs(active_left.len())?;
                for left_index in &active_left {
                    record_overlap(
                        left[*left_index],
                        right[event.index],
                        *left_index,
                        event.index,
                        &mut left_overlapped,
                        &mut right_overlapped,
                        &mut stats,
                    )?;
                }
                active_right.insert(event.index);
            }
        }
    }
    stats.left_overlapped_count = left_overlapped.iter().filter(|value| **value).count() as u64;
    stats.right_overlapped_count = right_overlapped.iter().filter(|value| **value).count() as u64;
    Ok(stats)
}

fn sweep_event_work(event_count: usize, limit: u64) -> Result<u64, BedError> {
    let event_count = u64::try_from(event_count).map_err(|_| BedError::LimitExceeded {
        resource: "work-unit",
        limit,
    })?;
    let sort_levels = if event_count <= 1 {
        0
    } else {
        u64::from(u64::BITS - (event_count - 1).leading_zeros())
    };
    event_count
        .checked_mul(sort_levels + 1)
        .ok_or(BedError::LimitExceeded {
            resource: "work-unit",
            limit,
        })
}

fn record_overlap(
    left: Interval,
    right: Interval,
    left_index: usize,
    right_index: usize,
    left_overlapped: &mut [bool],
    right_overlapped: &mut [bool],
    stats: &mut IntervalContigStats,
) -> Result<(), BedError> {
    let overlap = left
        .end
        .min(right.end)
        .saturating_sub(left.start.max(right.start));
    if overlap == 0 {
        return Ok(());
    }
    stats.overlap_pair_count = stats
        .overlap_pair_count
        .checked_add(1)
        .ok_or(BedError::Overflow)?;
    stats.total_overlap_bases = stats
        .total_overlap_bases
        .checked_add(overlap)
        .ok_or(BedError::Overflow)?;
    left_overlapped[left_index] = true;
    right_overlapped[right_index] = true;
    Ok(())
}

fn add_stats(
    total: &mut IntervalIntersectStats,
    contig: &IntervalContigStats,
) -> Result<(), BedError> {
    total.left_interval_count = total
        .left_interval_count
        .checked_add(contig.left_interval_count)
        .ok_or(BedError::Overflow)?;
    total.right_interval_count = total
        .right_interval_count
        .checked_add(contig.right_interval_count)
        .ok_or(BedError::Overflow)?;
    total.overlap_pair_count = total
        .overlap_pair_count
        .checked_add(contig.overlap_pair_count)
        .ok_or(BedError::Overflow)?;
    total.left_overlapped_count = total
        .left_overlapped_count
        .checked_add(contig.left_overlapped_count)
        .ok_or(BedError::Overflow)?;
    total.right_overlapped_count = total
        .right_overlapped_count
        .checked_add(contig.right_overlapped_count)
        .ok_or(BedError::Overflow)?;
    total.total_overlap_bases = total
        .total_overlap_bases
        .checked_add(contig.total_overlap_bases)
        .ok_or(BedError::Overflow)?;
    Ok(())
}

fn malformed<T>(line: usize, message: impl Into<String>) -> Result<T, BedError> {
    Err(BedError::MalformedRecord {
        line,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BedError, BedReadBudget, IntersectionBudget, Interval, intersect_contig,
        intersect_interval_sets, read_bed, read_bed_with_limits,
    };
    use flate2::Compression;
    use flate2::read::MultiGzDecoder;
    use flate2::write::GzEncoder;
    use std::io::{BufReader, Cursor, Write};

    #[test]
    fn intersects_half_open_bed_intervals() {
        let left =
            read_bed(Cursor::new(b"chr1\t0\t10\nchr1\t10\t20\nchr2\t5\t12\n")).expect("left BED");
        let right =
            read_bed(Cursor::new(b"chr1\t5\t15\nchr1\t20\t25\nchr2\t0\t7\n")).expect("right BED");
        let stats = intersect_interval_sets(left, right).expect("intersections");

        assert_eq!(stats.left_interval_count, 3);
        assert_eq!(stats.right_interval_count, 3);
        assert_eq!(stats.overlap_pair_count, 3);
        assert_eq!(stats.left_overlapped_count, 3);
        assert_eq!(stats.right_overlapped_count, 2);
        assert_eq!(stats.total_overlap_bases, 12);
        assert_eq!(stats.contigs["chr1"].overlap_pair_count, 2);
        assert!(stats.warnings.is_empty());
    }

    #[test]
    fn treats_touching_boundaries_as_non_overlapping() {
        let left = read_bed(Cursor::new(b"chr1\t0\t10\n")).expect("left BED");
        let right = read_bed(Cursor::new(b"chr1\t10\t20\n")).expect("right BED");
        let stats = intersect_interval_sets(left, right).expect("intersections");
        assert_eq!(stats.overlap_pair_count, 0);
        assert_eq!(stats.warnings.len(), 1);
    }

    #[test]
    fn rejects_reversed_or_empty_intervals() {
        let error = read_bed(Cursor::new(b"chr1\t10\t10\n")).expect_err("empty interval");
        assert!(matches!(error, BedError::MalformedRecord { line: 1, .. }));
    }

    #[test]
    fn rejects_gzip_expansion_during_bed_reading() {
        let input = b"chr1\t0\t10\nchr1\t10\t20\n";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(input).expect("compress BED fixture");
        let compressed = encoder.finish().expect("finish BED fixture");
        let decoder = MultiGzDecoder::new(Cursor::new(compressed));
        let mut read_budget = BedReadBudget::new(10, 10_000);
        let mut work_budget = IntersectionBudget::new(10, 10);

        let error = read_bed_with_limits(
            BufReader::new(decoder),
            15,
            &mut read_budget,
            &mut work_budget,
        )
        .expect_err("decompressed byte limit");

        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "decompressed byte count per input",
                limit: 15
            }
        ));
        assert_eq!(read_budget.interval_records, 1);
    }

    #[test]
    fn rejects_record_and_retained_memory_limits_before_storing_interval() {
        let mut record_budget = BedReadBudget::new(1, 10_000);
        let mut work_budget = IntersectionBudget::new(10, 10);
        let error = read_bed_with_limits(
            Cursor::new(b"chr1\t0\t10\nchr1\t10\t20\n"),
            1_000,
            &mut record_budget,
            &mut work_budget,
        )
        .expect_err("record limit");
        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "input interval record count",
                limit: 1
            }
        ));
        assert_eq!(record_budget.interval_records, 1);
        assert_eq!(work_budget.work_units, 1);

        let mut retained_budget = BedReadBudget::new(10, 1);
        let mut work_budget = IntersectionBudget::new(10, 10);
        let error = read_bed_with_limits(
            Cursor::new(b"chr1\t0\t10\n"),
            1_000,
            &mut retained_budget,
            &mut work_budget,
        )
        .expect_err("retained memory limit");
        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "retained input byte estimate",
                limit: 1
            }
        ));
        assert_eq!(retained_budget.interval_records, 0);
        assert_eq!(work_budget.work_units, 0);
    }

    #[test]
    fn charges_input_parsing_against_the_work_budget() {
        let mut read_budget = BedReadBudget::new(10, 10_000);
        let mut work_budget = IntersectionBudget::new(1, 10);
        let error = read_bed_with_limits(
            Cursor::new(b"chr1\t0\t10\nchr1\t10\t20\n"),
            1_000,
            &mut read_budget,
            &mut work_budget,
        )
        .expect_err("work limit");
        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "work-unit",
                limit: 1
            }
        ));
        assert_eq!(read_budget.interval_records, 1);
        assert_eq!(work_budget.work_units, 1);
    }

    #[test]
    fn rejects_inputs_before_exceeding_work_budget() {
        let left = vec![Interval { start: 0, end: 10 }; 3];
        let right = Vec::new();
        let mut budget = IntersectionBudget::new(23, 10);

        let error = intersect_contig(&left, &right, &mut budget).expect_err("work limit");
        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "work-unit",
                limit: 23
            }
        ));
        assert_eq!(budget.work_units, 0);
    }

    #[test]
    fn rejects_dense_inputs_before_exceeding_overlap_result_budget() {
        let left = vec![Interval { start: 0, end: 10 }; 3];
        let right = vec![Interval { start: 0, end: 10 }; 2];
        let mut budget = IntersectionBudget::new(100, 4);

        let error = intersect_contig(&left, &right, &mut budget).expect_err("result limit");
        assert!(matches!(
            error,
            BedError::LimitExceeded {
                resource: "overlap-pair",
                limit: 4
            }
        ));
        assert_eq!(budget.overlap_pairs, 3);
    }
}
