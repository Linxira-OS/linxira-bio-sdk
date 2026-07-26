use csv::ReaderBuilder;
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_SET_COLUMNS: usize = 64;
pub const MAX_VENN_COLUMNS: usize = 6;
pub const MAX_SET_ROWS: u64 = 1_000_000;
pub const MAX_UNIQUE_SET_ITEMS: usize = 1_000_000;
pub const MAX_REPORTED_INTERSECTIONS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAnalysisOptions {
    pub include_items: bool,
    pub max_intersections: usize,
}

impl Default for SetAnalysisOptions {
    fn default() -> Self {
        Self {
            include_items: false,
            max_intersections: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetSize {
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetIntersection {
    pub sets: Vec<String>,
    pub degree: u64,
    pub count: u64,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VennAnalysis {
    pub set_count: u64,
    pub union_size: u64,
    pub set_sizes: Vec<SetSize>,
    pub intersections: Vec<SetIntersection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpSetAnalysis {
    pub set_count: u64,
    pub union_size: u64,
    pub set_sizes: Vec<SetSize>,
    pub intersection_count: u64,
    pub reported_intersection_count: u64,
    pub omitted_intersection_count: u64,
    pub intersections: Vec<SetIntersection>,
}

#[derive(Debug)]
pub enum SetAnalysisError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidTable(String),
    LimitExceeded { resource: &'static str, limit: u64 },
}

impl Display for SetAnalysisError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read set table: {error}"),
            Self::Csv(error) => write!(formatter, "failed to parse set table: {error}"),
            Self::InvalidTable(message) => write!(formatter, "invalid set table: {message}"),
            Self::LimitExceeded { resource, limit } => {
                write!(
                    formatter,
                    "set table {resource} exceeds the limit of {limit}"
                )
            }
        }
    }
}

impl Error for SetAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            Self::InvalidTable(_) | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<io::Error> for SetAnalysisError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for SetAnalysisError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub fn venn_analysis_path(
    path: impl AsRef<Path>,
    options: SetAnalysisOptions,
) -> Result<VennAnalysis, SetAnalysisError> {
    validate_options(options)?;
    let table = read_membership_table(path.as_ref())?;
    if table.names.len() > MAX_VENN_COLUMNS {
        return Err(SetAnalysisError::InvalidTable(format!(
            "Venn analysis accepts 2 to {MAX_VENN_COLUMNS} set columns; found {}",
            table.names.len()
        )));
    }
    let (set_sizes, intersections) = summarize_memberships(&table, options.include_items);
    Ok(VennAnalysis {
        set_count: table.names.len() as u64,
        union_size: table.memberships.len() as u64,
        set_sizes,
        intersections,
    })
}

pub fn upset_analysis_path(
    path: impl AsRef<Path>,
    options: SetAnalysisOptions,
) -> Result<UpSetAnalysis, SetAnalysisError> {
    validate_options(options)?;
    let table = read_membership_table(path.as_ref())?;
    let (set_sizes, mut intersections) = summarize_memberships(&table, options.include_items);
    intersections.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.degree.cmp(&left.degree))
            .then_with(|| left.sets.cmp(&right.sets))
    });
    let intersection_count = intersections.len();
    intersections.truncate(options.max_intersections);
    Ok(UpSetAnalysis {
        set_count: table.names.len() as u64,
        union_size: table.memberships.len() as u64,
        set_sizes,
        intersection_count: intersection_count as u64,
        reported_intersection_count: intersections.len() as u64,
        omitted_intersection_count: intersection_count.saturating_sub(intersections.len()) as u64,
        intersections,
    })
}

#[derive(Debug)]
struct MembershipTable {
    names: Vec<String>,
    memberships: BTreeMap<String, u64>,
}

fn validate_options(options: SetAnalysisOptions) -> Result<(), SetAnalysisError> {
    if !(1..=MAX_REPORTED_INTERSECTIONS).contains(&options.max_intersections) {
        return Err(SetAnalysisError::InvalidTable(format!(
            "max_intersections must be between 1 and {MAX_REPORTED_INTERSECTIONS}"
        )));
    }
    Ok(())
}

fn read_membership_table(path: &Path) -> Result<MembershipTable, SetAnalysisError> {
    let delimiter = infer_delimiter(path);
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let reader: Box<dyn Read> = if prefix_length == prefix.len() && prefix == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    let mut csv = ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(reader);
    let headers = csv.headers()?.clone();
    if !(2..=MAX_SET_COLUMNS).contains(&headers.len()) {
        return Err(SetAnalysisError::InvalidTable(format!(
            "expected 2 to {MAX_SET_COLUMNS} named columns; found {}",
            headers.len()
        )));
    }
    let mut names = Vec::with_capacity(headers.len());
    for header in &headers {
        let name = header.trim();
        if name.is_empty() {
            return Err(SetAnalysisError::InvalidTable(
                "set names must not be empty".to_owned(),
            ));
        }
        if names.iter().any(|existing| existing == name) {
            return Err(SetAnalysisError::InvalidTable(format!(
                "duplicate set name {name:?}"
            )));
        }
        names.push(name.to_owned());
    }

    let mut memberships = BTreeMap::<String, u64>::new();
    for (row_index, record) in csv.records().enumerate() {
        let row_number = row_index as u64 + 1;
        if row_number > MAX_SET_ROWS {
            return Err(SetAnalysisError::LimitExceeded {
                resource: "row count",
                limit: MAX_SET_ROWS,
            });
        }
        let record = record?;
        if record.len() > names.len() {
            return Err(SetAnalysisError::InvalidTable(format!(
                "data row {} has {} fields but the header has {}",
                row_number + 1,
                record.len(),
                names.len()
            )));
        }
        for (column, value) in record.iter().enumerate() {
            let item = value.trim();
            if item.is_empty() {
                continue;
            }
            if !memberships.contains_key(item) && memberships.len() >= MAX_UNIQUE_SET_ITEMS {
                return Err(SetAnalysisError::LimitExceeded {
                    resource: "unique item count",
                    limit: MAX_UNIQUE_SET_ITEMS as u64,
                });
            }
            *memberships.entry(item.to_owned()).or_insert(0) |= 1_u64 << column;
        }
    }
    if memberships.is_empty() {
        return Err(SetAnalysisError::InvalidTable(
            "no non-empty set items were found".to_owned(),
        ));
    }
    Ok(MembershipTable { names, memberships })
}

fn summarize_memberships(
    table: &MembershipTable,
    include_items: bool,
) -> (Vec<SetSize>, Vec<SetIntersection>) {
    let mut sizes = vec![0_u64; table.names.len()];
    let mut groups = BTreeMap::<u64, (u64, Vec<String>)>::new();
    for (item, mask) in &table.memberships {
        for (column, size) in sizes.iter_mut().enumerate() {
            if mask & (1_u64 << column) != 0 {
                *size += 1;
            }
        }
        let group = groups.entry(*mask).or_default();
        group.0 += 1;
        if include_items {
            group.1.push(item.clone());
        }
    }
    let set_sizes = table
        .names
        .iter()
        .cloned()
        .zip(sizes)
        .map(|(name, count)| SetSize { name, count })
        .collect();
    let intersections = groups
        .into_iter()
        .map(|(mask, (count, items))| SetIntersection {
            sets: table
                .names
                .iter()
                .enumerate()
                .filter(|(column, _)| mask & (1_u64 << column) != 0)
                .map(|(_, name)| name.clone())
                .collect(),
            degree: mask.count_ones() as u64,
            count,
            items,
        })
        .collect();
    (set_sizes, intersections)
}

fn infer_delimiter(path: &Path) -> u8 {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = name
        .strip_suffix(".gz")
        .or_else(|| name.strip_suffix(".bgz"))
        .unwrap_or(&name);
    if name.ends_with(".tsv") || name.ends_with(".tab") {
        b'\t'
    } else {
        b','
    }
}

#[cfg(test)]
mod tests {
    use super::{SetAnalysisOptions, upset_analysis_path, venn_analysis_path};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

    fn fixture(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "linxira-set-{name}-{}-{}.csv",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, content).expect("write set fixture");
        path
    }

    #[test]
    fn computes_exact_venn_regions_and_deduplicates_items() {
        let path = fixture("venn", "A,B,C\na,b,c\nb,b,d\ne,,a\n");
        let result = venn_analysis_path(
            &path,
            SetAnalysisOptions {
                include_items: true,
                ..SetAnalysisOptions::default()
            },
        )
        .expect("valid Venn table");
        fs::remove_file(path).expect("remove fixture");

        assert_eq!(result.set_count, 3);
        assert_eq!(result.union_size, 5);
        assert_eq!(result.set_sizes[0].count, 3);
        let shared_ab = result
            .intersections
            .iter()
            .find(|region| region.sets == ["A", "B"])
            .expect("A/B region");
        assert_eq!(shared_ab.count, 1);
        assert_eq!(shared_ab.items, ["b"]);
    }

    #[test]
    fn sorts_upset_intersections_by_size_and_truncates() {
        let path = fixture("upset", "A,B,C\na,a,a\nb,b,\nc,,\n");
        let result = upset_analysis_path(
            &path,
            SetAnalysisOptions {
                max_intersections: 1,
                ..SetAnalysisOptions::default()
            },
        )
        .expect("valid UpSet table");
        fs::remove_file(path).expect("remove fixture");

        assert_eq!(result.intersection_count, 3);
        assert_eq!(result.reported_intersection_count, 1);
        assert_eq!(result.omitted_intersection_count, 2);
        assert_eq!(result.intersections[0].sets, ["A", "B", "C"]);
    }

    #[test]
    fn rejects_venn_tables_with_more_than_six_sets() {
        let path = fixture("too-many", "A,B,C,D,E,F,G\na,b,c,d,e,f,g\n");
        let error = venn_analysis_path(&path, SetAnalysisOptions::default())
            .expect_err("seven-set Venn must fail");
        fs::remove_file(path).expect("remove fixture");

        assert!(error.to_string().contains("2 to 6"));
    }
}
