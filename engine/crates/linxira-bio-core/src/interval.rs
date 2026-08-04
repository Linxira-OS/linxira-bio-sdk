use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IntervalMergeOptions {
    pub max_gap: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalMergeStats {
    pub input_interval_count: u64,
    pub output_interval_count: u64,
    pub merged_interval_count: u64,
    pub input_bases: u64,
    pub output_bases: u64,
    pub max_gap: u64,
    pub contigs: BTreeMap<String, IntervalMergeContigStats>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalMergeContigStats {
    pub input_interval_count: u64,
    pub output_interval_count: u64,
    pub merged_interval_count: u64,
    pub input_bases: u64,
    pub output_bases: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalSubtractStats {
    pub left_interval_count: u64,
    pub right_interval_count: u64,
    pub output_interval_count: u64,
    pub affected_left_interval_count: u64,
    pub removed_bases: u64,
    pub output_bases: u64,
    pub contigs: BTreeMap<String, IntervalSubtractContigStats>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalSubtractContigStats {
    pub left_interval_count: u64,
    pub right_interval_count: u64,
    pub output_interval_count: u64,
    pub affected_left_interval_count: u64,
    pub removed_bases: u64,
    pub output_bases: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IntervalClosestDirection {
    Upstream,
    Downstream,
    Overlap,
}

impl IntervalClosestDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::Downstream => "downstream",
            Self::Overlap => "overlap",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalClosestStats {
    pub query_interval_count: u64,
    pub target_interval_count: u64,
    pub matched_query_count: u64,
    pub unmatched_query_count: u64,
    pub contigs: BTreeMap<String, IntervalClosestContigStats>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IntervalClosestContigStats {
    pub query_interval_count: u64,
    pub target_interval_count: u64,
    pub matched_query_count: u64,
    pub unmatched_query_count: u64,
}

#[derive(Debug)]
pub enum BedError {
    Io(io::Error),
    OutputAlreadyExists(PathBuf),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    LimitExceeded { resource: &'static str, limit: u64 },
    Overflow,
}

impl Display for BedError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read BED: {error}"),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
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
            Self::Overflow => formatter.write_str("interval processing exceeds supported range"),
        }
    }
}

impl Error for BedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            Self::OutputAlreadyExists(_)
            | Self::MalformedRecord { .. }
            | Self::LimitExceeded { .. }
            | Self::Overflow => None,
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

impl Interval {
    fn length(self) -> u64 {
        self.end - self.start
    }
}

#[derive(Debug)]
struct ClosestTargetIndex {
    by_start: Vec<Interval>,
    prefix_max_end: Vec<u64>,
    by_end: Vec<Interval>,
}

#[derive(Debug, Clone, Copy)]
struct ClosestTargetMatch {
    target: Interval,
    distance: u64,
    direction: IntervalClosestDirection,
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

pub fn bed_merge_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: IntervalMergeOptions,
) -> Result<IntervalMergeStats, BedError> {
    let mut work_budget = IntersectionBudget::production();
    let mut read_budget = BedReadBudget::production();
    let intervals = read_bed_path(input.as_ref(), &mut read_budget, &mut work_budget)?;
    let mut stats = IntervalMergeStats {
        max_gap: options.max_gap,
        ..Default::default()
    };

    with_new_output(output.as_ref(), |writer| {
        for (contig, mut contig_intervals) in intervals {
            contig_intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
            let contig_stats = merge_contig(writer, &contig, &contig_intervals, options.max_gap)?;
            add_merge_stats(&mut stats, &contig_stats)?;
            stats.contigs.insert(contig, contig_stats);
        }
        if stats.input_interval_count == 0 {
            stats
                .warnings
                .push("BED input contains no intervals".to_owned());
        }
        Ok(stats.clone())
    })
}

pub fn bed_subtract_path(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<IntervalSubtractStats, BedError> {
    let mut work_budget = IntersectionBudget::production();
    let mut read_budget = BedReadBudget::production();
    let mut left = read_bed_path(left.as_ref(), &mut read_budget, &mut work_budget)?;
    let mut right = read_bed_path(right.as_ref(), &mut read_budget, &mut work_budget)?;
    let contigs = left
        .keys()
        .chain(right.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut stats = IntervalSubtractStats::default();

    with_new_output(output.as_ref(), |writer| {
        for contig in contigs {
            let left_intervals = left.entry(contig.clone()).or_default();
            let right_intervals = right.entry(contig.clone()).or_default();
            left_intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
            right_intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
            let contig_stats = subtract_contig(writer, &contig, left_intervals, right_intervals)?;
            add_subtract_stats(&mut stats, &contig_stats)?;
            stats.contigs.insert(contig, contig_stats);
        }
        if stats.left_interval_count == 0 {
            stats
                .warnings
                .push("left BED input contains no intervals".to_owned());
        } else if stats.right_interval_count == 0 {
            stats.warnings.push(
                "right BED input contains no intervals; left intervals are unchanged".to_owned(),
            );
        }
        Ok(stats.clone())
    })
}

/// Writes one deterministic nearest-target row for each query interval that
/// has at least one target on the same chromosome.
///
/// Inputs accept BED3 or wider tab-separated records; optional BED fields are
/// ignored. Coordinates use zero-based, half-open semantics. The output is a
/// headered TSV containing query BED3, target BED3, the non-negative interval
/// gap, and `upstream`, `downstream`, or `overlap`. Bookended intervals have a
/// zero gap but remain directional because they do not overlap. If targets tie
/// on distance, the lexicographically smallest `(start, end)` target is used.
pub fn bed_closest_path(
    query: impl AsRef<Path>,
    target: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<IntervalClosestStats, BedError> {
    let mut work_budget = IntersectionBudget::production();
    let mut read_budget = BedReadBudget::production();
    let mut queries = read_bed_path(query.as_ref(), &mut read_budget, &mut work_budget)?;
    let mut targets = read_bed_path(target.as_ref(), &mut read_budget, &mut work_budget)?;
    let contigs = queries
        .keys()
        .chain(targets.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut stats = IntervalClosestStats::default();

    with_new_output(output.as_ref(), |writer| {
        writeln!(
            writer,
            "query_contig\tquery_start\tquery_end\ttarget_contig\ttarget_start\ttarget_end\tdistance\tdirection"
        )?;
        for contig in contigs {
            let mut contig_queries = queries.remove(&contig).unwrap_or_default();
            let contig_targets = targets.remove(&contig).unwrap_or_default();
            contig_queries.sort_unstable_by_key(|interval| (interval.start, interval.end));
            let index = ClosestTargetIndex::new(contig_targets);
            let mut contig_stats = IntervalClosestContigStats {
                query_interval_count: u64::try_from(contig_queries.len())
                    .expect("interval count fits in u64"),
                target_interval_count: u64::try_from(index.len())
                    .expect("interval count fits in u64"),
                ..Default::default()
            };

            for query_interval in contig_queries {
                let Some(nearest) = index.closest(query_interval) else {
                    contig_stats.unmatched_query_count = contig_stats
                        .unmatched_query_count
                        .checked_add(1)
                        .ok_or(BedError::Overflow)?;
                    continue;
                };
                write_closest_match(writer, &contig, query_interval, nearest)?;
                contig_stats.matched_query_count = contig_stats
                    .matched_query_count
                    .checked_add(1)
                    .ok_or(BedError::Overflow)?;
            }

            add_closest_stats(&mut stats, &contig_stats)?;
            stats.contigs.insert(contig, contig_stats);
        }
        if stats.query_interval_count == 0 {
            stats
                .warnings
                .push("query BED input contains no intervals".to_owned());
        }
        if stats.target_interval_count == 0 {
            stats.warnings.push(
                "target BED input contains no intervals; no closest intervals were reported"
                    .to_owned(),
            );
        } else if stats.unmatched_query_count > 0 {
            stats.warnings.push(format!(
                "{} query interval(s) have no target on the same chromosome",
                stats.unmatched_query_count
            ));
        }
        Ok(stats.clone())
    })
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

impl ClosestTargetIndex {
    fn new(mut targets: Vec<Interval>) -> Self {
        targets.sort_unstable_by_key(|interval| (interval.start, interval.end));
        let mut prefix_max_end = Vec::with_capacity(targets.len());
        let mut maximum_end = 0_u64;
        for target in &targets {
            maximum_end = maximum_end.max(target.end);
            prefix_max_end.push(maximum_end);
        }
        let mut by_end = targets.clone();
        by_end.sort_unstable_by_key(|interval| (interval.end, interval.start));
        Self {
            by_start: targets,
            prefix_max_end,
            by_end,
        }
    }

    fn len(&self) -> usize {
        self.by_start.len()
    }

    fn closest(&self, query: Interval) -> Option<ClosestTargetMatch> {
        let downstream_index = self
            .by_start
            .partition_point(|target| target.start < query.end);
        let first_possible_overlap = self.prefix_max_end[..downstream_index]
            .partition_point(|maximum_end| *maximum_end <= query.start);
        let overlap = (first_possible_overlap < downstream_index).then(|| ClosestTargetMatch {
            target: self.by_start[first_possible_overlap],
            distance: 0,
            direction: IntervalClosestDirection::Overlap,
        });

        let upstream_count = self
            .by_end
            .partition_point(|target| target.end <= query.start);
        let upstream = (upstream_count > 0).then(|| {
            let maximum_end = self.by_end[upstream_count - 1].end;
            let first_with_maximum_end =
                self.by_end[..upstream_count].partition_point(|target| target.end < maximum_end);
            let target = self.by_end[first_with_maximum_end];
            ClosestTargetMatch {
                target,
                distance: query.start - target.end,
                direction: IntervalClosestDirection::Upstream,
            }
        });
        let downstream =
            self.by_start
                .get(downstream_index)
                .copied()
                .map(|target| ClosestTargetMatch {
                    target,
                    distance: target.start - query.end,
                    direction: IntervalClosestDirection::Downstream,
                });

        [overlap, upstream, downstream]
            .into_iter()
            .flatten()
            .min_by_key(|candidate| {
                (
                    candidate.distance,
                    candidate.target.start,
                    candidate.target.end,
                )
            })
    }
}

fn write_closest_match(
    writer: &mut impl Write,
    contig: &str,
    query: Interval,
    nearest: ClosestTargetMatch,
) -> Result<(), BedError> {
    writeln!(
        writer,
        "{contig}\t{}\t{}\t{contig}\t{}\t{}\t{}\t{}",
        query.start,
        query.end,
        nearest.target.start,
        nearest.target.end,
        nearest.distance,
        nearest.direction.as_str()
    )?;
    Ok(())
}

fn merge_contig(
    writer: &mut impl Write,
    contig: &str,
    intervals: &[Interval],
    max_gap: u64,
) -> Result<IntervalMergeContigStats, BedError> {
    let mut stats = IntervalMergeContigStats {
        input_interval_count: u64::try_from(intervals.len()).expect("interval count fits in u64"),
        ..Default::default()
    };
    let mut active: Option<Interval> = None;

    for interval in intervals {
        stats.input_bases = stats
            .input_bases
            .checked_add(interval.length())
            .ok_or(BedError::Overflow)?;
        match active {
            None => active = Some(*interval),
            Some(current) if intervals_can_merge(current, *interval, max_gap) => {
                active = Some(Interval {
                    start: current.start,
                    end: current.end.max(interval.end),
                });
            }
            Some(current) => {
                write_interval(writer, contig, current)?;
                stats.output_interval_count += 1;
                stats.output_bases = stats
                    .output_bases
                    .checked_add(current.length())
                    .ok_or(BedError::Overflow)?;
                active = Some(*interval);
            }
        }
    }

    if let Some(current) = active {
        write_interval(writer, contig, current)?;
        stats.output_interval_count += 1;
        stats.output_bases = stats
            .output_bases
            .checked_add(current.length())
            .ok_or(BedError::Overflow)?;
    }
    stats.merged_interval_count = stats
        .input_interval_count
        .saturating_sub(stats.output_interval_count);
    Ok(stats)
}

fn intervals_can_merge(current: Interval, next: Interval, max_gap: u64) -> bool {
    if next.start <= current.end {
        true
    } else {
        next.start - current.end <= max_gap
    }
}

fn subtract_contig(
    writer: &mut impl Write,
    contig: &str,
    left: &[Interval],
    right: &[Interval],
) -> Result<IntervalSubtractContigStats, BedError> {
    let mut stats = IntervalSubtractContigStats {
        left_interval_count: u64::try_from(left.len()).expect("interval count fits in u64"),
        right_interval_count: u64::try_from(right.len()).expect("interval count fits in u64"),
        ..Default::default()
    };
    let mut first_relevant_right = 0_usize;

    for left_interval in left {
        while first_relevant_right < right.len()
            && right[first_relevant_right].end <= left_interval.start
        {
            first_relevant_right += 1;
        }

        let mut cursor = left_interval.start;
        let mut removed_for_left = 0_u64;
        let mut right_index = first_relevant_right;
        while right_index < right.len() && right[right_index].start < left_interval.end {
            let right_interval = right[right_index];
            if right_interval.start > cursor {
                let fragment = Interval {
                    start: cursor,
                    end: right_interval.start.min(left_interval.end),
                };
                write_interval(writer, contig, fragment)?;
                stats.output_interval_count += 1;
                stats.output_bases = stats
                    .output_bases
                    .checked_add(fragment.length())
                    .ok_or(BedError::Overflow)?;
            }

            let removal_start = cursor.max(right_interval.start);
            let removal_end = left_interval.end.min(right_interval.end);
            if removal_end > removal_start {
                removed_for_left = removed_for_left
                    .checked_add(removal_end - removal_start)
                    .ok_or(BedError::Overflow)?;
            }
            cursor = cursor.max(right_interval.end);
            if cursor >= left_interval.end {
                break;
            }
            right_index += 1;
        }

        if cursor < left_interval.end {
            let fragment = Interval {
                start: cursor,
                end: left_interval.end,
            };
            write_interval(writer, contig, fragment)?;
            stats.output_interval_count += 1;
            stats.output_bases = stats
                .output_bases
                .checked_add(fragment.length())
                .ok_or(BedError::Overflow)?;
        }
        if removed_for_left > 0 {
            stats.affected_left_interval_count += 1;
            stats.removed_bases = stats
                .removed_bases
                .checked_add(removed_for_left)
                .ok_or(BedError::Overflow)?;
        }
    }

    Ok(stats)
}

fn write_interval(
    writer: &mut impl Write,
    contig: &str,
    interval: Interval,
) -> Result<(), BedError> {
    writeln!(writer, "{contig}	{}	{}", interval.start, interval.end)?;
    Ok(())
}

fn with_new_output<T>(
    output: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<T, BedError>,
) -> Result<T, BedError> {
    if output.exists() {
        return Err(BedError::OutputAlreadyExists(output.to_owned()));
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = BufWriter::new(file);
    match operation(&mut writer).and_then(|value| {
        writer.flush()?;
        Ok(value)
    }) {
        Ok(value) => Ok(value),
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
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

fn add_merge_stats(
    total: &mut IntervalMergeStats,
    contig: &IntervalMergeContigStats,
) -> Result<(), BedError> {
    total.input_interval_count = total
        .input_interval_count
        .checked_add(contig.input_interval_count)
        .ok_or(BedError::Overflow)?;
    total.output_interval_count = total
        .output_interval_count
        .checked_add(contig.output_interval_count)
        .ok_or(BedError::Overflow)?;
    total.merged_interval_count = total
        .merged_interval_count
        .checked_add(contig.merged_interval_count)
        .ok_or(BedError::Overflow)?;
    total.input_bases = total
        .input_bases
        .checked_add(contig.input_bases)
        .ok_or(BedError::Overflow)?;
    total.output_bases = total
        .output_bases
        .checked_add(contig.output_bases)
        .ok_or(BedError::Overflow)?;
    Ok(())
}

fn add_subtract_stats(
    total: &mut IntervalSubtractStats,
    contig: &IntervalSubtractContigStats,
) -> Result<(), BedError> {
    total.left_interval_count = total
        .left_interval_count
        .checked_add(contig.left_interval_count)
        .ok_or(BedError::Overflow)?;
    total.right_interval_count = total
        .right_interval_count
        .checked_add(contig.right_interval_count)
        .ok_or(BedError::Overflow)?;
    total.output_interval_count = total
        .output_interval_count
        .checked_add(contig.output_interval_count)
        .ok_or(BedError::Overflow)?;
    total.affected_left_interval_count = total
        .affected_left_interval_count
        .checked_add(contig.affected_left_interval_count)
        .ok_or(BedError::Overflow)?;
    total.removed_bases = total
        .removed_bases
        .checked_add(contig.removed_bases)
        .ok_or(BedError::Overflow)?;
    total.output_bases = total
        .output_bases
        .checked_add(contig.output_bases)
        .ok_or(BedError::Overflow)?;
    Ok(())
}

fn add_closest_stats(
    total: &mut IntervalClosestStats,
    contig: &IntervalClosestContigStats,
) -> Result<(), BedError> {
    total.query_interval_count = total
        .query_interval_count
        .checked_add(contig.query_interval_count)
        .ok_or(BedError::Overflow)?;
    total.target_interval_count = total
        .target_interval_count
        .checked_add(contig.target_interval_count)
        .ok_or(BedError::Overflow)?;
    total.matched_query_count = total
        .matched_query_count
        .checked_add(contig.matched_query_count)
        .ok_or(BedError::Overflow)?;
    total.unmatched_query_count = total
        .unmatched_query_count
        .checked_add(contig.unmatched_query_count)
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
        BedError, BedReadBudget, ClosestTargetIndex, ClosestTargetMatch, IntersectionBudget,
        Interval, IntervalClosestDirection, IntervalMergeOptions, bed_closest_path, bed_merge_path,
        bed_subtract_path, intersect_contig, intersect_interval_sets, read_bed,
        read_bed_with_limits,
    };
    use flate2::Compression;
    use flate2::read::MultiGzDecoder;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::{BufReader, Cursor, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn fixture_path(suffix: &str) -> PathBuf {
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-interval-{}-{ordinal}.{suffix}",
            std::process::id()
        ))
    }

    fn brute_force_closest(targets: &[Interval], query: Interval) -> Option<ClosestTargetMatch> {
        targets
            .iter()
            .copied()
            .map(|target| {
                if target.end <= query.start {
                    ClosestTargetMatch {
                        target,
                        distance: query.start - target.end,
                        direction: IntervalClosestDirection::Upstream,
                    }
                } else if target.start >= query.end {
                    ClosestTargetMatch {
                        target,
                        distance: target.start - query.end,
                        direction: IntervalClosestDirection::Downstream,
                    }
                } else {
                    ClosestTargetMatch {
                        target,
                        distance: 0,
                        direction: IntervalClosestDirection::Overlap,
                    }
                }
            })
            .min_by_key(|candidate| {
                (
                    candidate.distance,
                    candidate.target.start,
                    candidate.target.end,
                )
            })
    }

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

    #[test]
    fn merges_overlapping_and_bookended_bed_intervals() {
        let input = fixture_path("bed");
        let output = fixture_path("merged.bed");
        fs::write(
            &input,
            b"chr1\t0\t5\nchr1\t5\t10\nchr1\t12\t14\nchr2\t1\t3\n",
        )
        .unwrap();

        let stats = bed_merge_path(&input, &output, IntervalMergeOptions { max_gap: 0 }).unwrap();

        assert_eq!(stats.input_interval_count, 4);
        assert_eq!(stats.output_interval_count, 3);
        assert_eq!(stats.merged_interval_count, 1);
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            "chr1\t0\t10\nchr1\t12\t14\nchr2\t1\t3\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn subtracts_bed_intervals_into_remaining_fragments() {
        let left = fixture_path("left.bed");
        let right = fixture_path("right.bed");
        let output = fixture_path("subtracted.bed");
        fs::write(&left, b"chr1\t0\t10\nchr1\t20\t30\nchr2\t5\t9\n").unwrap();
        fs::write(&right, b"chr1\t3\t6\nchr1\t8\t25\nchr2\t0\t6\n").unwrap();

        let stats = bed_subtract_path(&left, &right, &output).unwrap();

        assert_eq!(stats.left_interval_count, 3);
        assert_eq!(stats.right_interval_count, 3);
        assert_eq!(stats.affected_left_interval_count, 3);
        assert_eq!(stats.output_interval_count, 4);
        assert_eq!(stats.removed_bases, 11);
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            "chr1\t0\t3\nchr1\t6\t8\nchr1\t25\t30\nchr2\t6\t9\n"
        );
        fs::remove_file(left).unwrap();
        fs::remove_file(right).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn finds_deterministic_closest_targets_for_unsorted_wide_bed() {
        let query = fixture_path("query.bed");
        let target = fixture_path("target.bed");
        let first_output = fixture_path("closest.tsv");
        let second_output = fixture_path("closest-reordered.tsv");
        let query_rows = [
            "chr2\t1\t2\tmissing",
            "chr1\t30\t35\tequidistant",
            "chr4\t10\t15\tdownstream",
            "chr1\t10\t20\toverlap\t9",
            "chr1\t5\t10\tbookended",
        ];
        let target_rows = [
            "chr1\t40\t45\tdownstream-tie",
            "chr3\t1\t2\ttarget-only",
            "chr1\t20\t25\tupstream-tie",
            "chr4\t20\t25\tdownstream",
            "chr1\t15\t18\toverlap-later",
            "chr1\t10\t12\toverlap-first",
            "chr1\t0\t5\tupstream-bookended",
        ];
        fs::write(&query, format!("{}\n", query_rows.join("\n"))).unwrap();
        fs::write(&target, format!("{}\n", target_rows.join("\n"))).unwrap();

        let first_stats = bed_closest_path(&query, &target, &first_output).unwrap();

        fs::write(
            &query,
            format!(
                "{}\n",
                query_rows
                    .iter()
                    .rev()
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )
        .unwrap();
        fs::write(
            &target,
            format!(
                "{}\n",
                target_rows
                    .iter()
                    .rev()
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )
        .unwrap();
        let second_stats = bed_closest_path(&query, &target, &second_output).unwrap();

        let expected = concat!(
            "query_contig\tquery_start\tquery_end\ttarget_contig\ttarget_start\ttarget_end\tdistance\tdirection\n",
            "chr1\t5\t10\tchr1\t0\t5\t0\tupstream\n",
            "chr1\t10\t20\tchr1\t10\t12\t0\toverlap\n",
            "chr1\t30\t35\tchr1\t20\t25\t5\tupstream\n",
            "chr4\t10\t15\tchr4\t20\t25\t5\tdownstream\n",
        );
        assert_eq!(fs::read_to_string(&first_output).unwrap(), expected);
        assert_eq!(fs::read_to_string(&second_output).unwrap(), expected);
        assert_eq!(first_stats, second_stats);
        assert_eq!(first_stats.query_interval_count, 5);
        assert_eq!(first_stats.target_interval_count, 7);
        assert_eq!(first_stats.matched_query_count, 4);
        assert_eq!(first_stats.unmatched_query_count, 1);
        assert_eq!(first_stats.contigs["chr3"].target_interval_count, 1);
        assert_eq!(first_stats.contigs["chr2"].unmatched_query_count, 1);
        assert_eq!(first_stats.warnings.len(), 1);

        for path in [query, target, first_output, second_output] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn nested_target_is_found_as_an_overlap() {
        let index = ClosestTargetIndex::new(vec![
            Interval { start: 10, end: 20 },
            Interval { start: 0, end: 100 },
            Interval { start: 40, end: 45 },
        ]);
        let nearest = index
            .closest(Interval { start: 50, end: 60 })
            .expect("overlapping target");

        assert_eq!(nearest.target.start, 0);
        assert_eq!(nearest.target.end, 100);
        assert_eq!(nearest.distance, 0);
        assert_eq!(nearest.direction, IntervalClosestDirection::Overlap);
    }

    #[test]
    fn closest_index_matches_brute_force_across_small_coordinates() {
        let universe = (0..8)
            .flat_map(|start| ((start + 1)..=8).map(move |end| Interval { start, end }))
            .collect::<Vec<_>>();

        for offset in 0..7 {
            let targets = universe
                .iter()
                .enumerate()
                .filter(|(index, _)| (index + offset) % 4 != 0)
                .map(|(_, interval)| *interval)
                .collect::<Vec<_>>();
            let index = ClosestTargetIndex::new(targets.clone());
            for query in &universe {
                let actual = index.closest(*query).map(|nearest| {
                    (
                        nearest.target.start,
                        nearest.target.end,
                        nearest.distance,
                        nearest.direction,
                    )
                });
                let expected = brute_force_closest(&targets, *query).map(|nearest| {
                    (
                        nearest.target.start,
                        nearest.target.end,
                        nearest.distance,
                        nearest.direction,
                    )
                });
                assert_eq!(actual, expected, "query [{}, {})", query.start, query.end);
            }
        }
        assert!(
            ClosestTargetIndex::new(Vec::new())
                .closest(Interval { start: 0, end: 1 })
                .is_none()
        );
    }

    #[test]
    fn empty_target_reports_every_query_as_unmatched() {
        let query = fixture_path("query.bed");
        let target = fixture_path("empty-target.bed");
        let output = fixture_path("closest.tsv");
        fs::write(&query, b"chr1\t0\t10\nchr2\t5\t8\n").unwrap();
        fs::write(&target, b"# no target records\n").unwrap();

        let stats = bed_closest_path(&query, &target, &output).unwrap();

        assert_eq!(stats.query_interval_count, 2);
        assert_eq!(stats.target_interval_count, 0);
        assert_eq!(stats.matched_query_count, 0);
        assert_eq!(stats.unmatched_query_count, 2);
        assert_eq!(stats.warnings.len(), 1);
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            "query_contig\tquery_start\tquery_end\ttarget_contig\ttarget_start\ttarget_end\tdistance\tdirection\n"
        );

        for path in [query, target, output] {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn malformed_closest_input_does_not_create_output() {
        let query = fixture_path("query.bed");
        let target = fixture_path("malformed-target.bed");
        let output = fixture_path("closest.tsv");
        fs::write(&query, b"chr1\t0\t10\n").unwrap();
        fs::write(&target, b"chr1\tnot-a-coordinate\t20\n").unwrap();

        let error = bed_closest_path(&query, &target, &output).expect_err("malformed target");

        assert!(matches!(error, BedError::MalformedRecord { line: 1, .. }));
        assert!(!output.exists());
        fs::remove_file(query).unwrap();
        fs::remove_file(target).unwrap();
    }
}
