use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_DOMAIN_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DOMAIN_HITS: usize = 2_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProteinDomainHit {
    pub sequence_id: String,
    pub sequence_length: Option<u64>,
    pub source: String,
    pub accession: String,
    pub name: Option<String>,
    pub start: u64,
    pub end: u64,
    pub evalue: Option<f64>,
    pub score: Option<f64>,
    pub interpro_accession: Option<String>,
    pub interpro_description: Option<String>,
    pub go_terms: Vec<String>,
    pub pathways: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProteinDomainParseResult {
    pub format: String,
    pub sequence_count: u64,
    pub hit_count: u64,
    pub source_counts: BTreeMap<String, u64>,
    pub accession_counts: BTreeMap<String, u64>,
    pub hits: Vec<ProteinDomainHit>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum DomainError {
    Io(io::Error),
    InvalidUtf8,
    InvalidFormat(String),
    MalformedRecord { line: usize, message: String },
    LimitExceeded { resource: &'static str, limit: u64 },
}

impl Display for DomainError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "protein-domain input I/O failed: {error}"),
            Self::InvalidUtf8 => formatter.write_str("protein-domain input is not valid UTF-8"),
            Self::InvalidFormat(message) => {
                write!(formatter, "unsupported domain format: {message}")
            }
            Self::MalformedRecord { line, message } => {
                write!(
                    formatter,
                    "malformed protein-domain record at line {line}: {message}"
                )
            }
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "protein-domain parsing exceeds the deterministic {resource} limit of {limit}"
            ),
        }
    }
}

impl Error for DomainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidUtf8
            | Self::InvalidFormat(_)
            | Self::MalformedRecord { .. }
            | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<io::Error> for DomainError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn parse_protein_domains_path(
    path: impl AsRef<Path>,
) -> Result<ProteinDomainParseResult, DomainError> {
    let text = read_bounded_text(path.as_ref())?;
    let first_data = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .ok_or_else(|| DomainError::InvalidFormat("input contains no domain records".to_owned()))?;
    let (format, hits) = if first_data.split('\t').count() >= 11 {
        ("interproscan-tsv", parse_interproscan_tsv(&text)?)
    } else if first_data.split_whitespace().count() >= 22 {
        ("hmmer-domtblout", parse_hmmer_domtblout(&text)?)
    } else {
        return Err(DomainError::InvalidFormat(
            "expected InterProScan TSV or HMMER domtblout".to_owned(),
        ));
    };
    summarize(format, hits)
}

fn parse_interproscan_tsv(text: &str) -> Result<Vec<ProteinDomainHit>, DomainError> {
    let mut hits = Vec::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        enforce_hit_limit(hits.len())?;
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 11 {
            return Err(DomainError::MalformedRecord {
                line: line_index + 1,
                message: format!(
                    "InterProScan TSV requires at least 11 columns but found {}",
                    fields.len()
                ),
            });
        }
        let sequence_id = nonempty(fields[0], line_index + 1, "protein accession")?;
        let sequence_length = parse_u64(fields[2], line_index + 1, "sequence length")?;
        let start = parse_u64(fields[6], line_index + 1, "start")?;
        let end = parse_u64(fields[7], line_index + 1, "end")?;
        validate_coordinates(start, end, Some(sequence_length), line_index + 1)?;
        hits.push(ProteinDomainHit {
            sequence_id,
            sequence_length: Some(sequence_length),
            source: nonempty(fields[3], line_index + 1, "analysis")?,
            accession: nonempty(fields[4], line_index + 1, "signature accession")?,
            name: optional_text(fields.get(5).copied()),
            start,
            end,
            evalue: None,
            score: optional_float(fields.get(8).copied(), line_index + 1, "score")?,
            interpro_accession: optional_text(fields.get(11).copied()),
            interpro_description: optional_text(fields.get(12).copied()),
            go_terms: split_annotations(fields.get(13).copied()),
            pathways: split_annotations(fields.get(14).copied()),
        });
    }
    Ok(hits)
}

fn parse_hmmer_domtblout(text: &str) -> Result<Vec<ProteinDomainHit>, DomainError> {
    let mut hits = Vec::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        enforce_hit_limit(hits.len())?;
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 22 {
            return Err(DomainError::MalformedRecord {
                line: line_index + 1,
                message: format!(
                    "HMMER domtblout requires at least 22 fields but found {}",
                    fields.len()
                ),
            });
        }
        let sequence_length = parse_u64(fields[2], line_index + 1, "target length")?;
        let start = parse_u64(fields[17], line_index + 1, "alignment start")?;
        let end = parse_u64(fields[18], line_index + 1, "alignment end")?;
        validate_coordinates(start, end, Some(sequence_length), line_index + 1)?;
        let accession = if fields[4] == "-" {
            fields[3]
        } else {
            fields[4]
        };
        hits.push(ProteinDomainHit {
            sequence_id: nonempty(fields[0], line_index + 1, "target name")?,
            sequence_length: Some(sequence_length),
            source: "HMMER".to_owned(),
            accession: nonempty(accession, line_index + 1, "query accession")?,
            name: Some(fields[3].to_owned()),
            start,
            end,
            evalue: Some(parse_float(
                fields[12],
                line_index + 1,
                "independent e-value",
            )?),
            score: Some(parse_float(fields[13], line_index + 1, "domain score")?),
            interpro_accession: None,
            interpro_description: None,
            go_terms: Vec::new(),
            pathways: Vec::new(),
        });
    }
    Ok(hits)
}

fn summarize(
    format: &str,
    hits: Vec<ProteinDomainHit>,
) -> Result<ProteinDomainParseResult, DomainError> {
    let sequence_count = hits
        .iter()
        .map(|hit| hit.sequence_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let mut source_counts = BTreeMap::new();
    let mut accession_counts = BTreeMap::new();
    for hit in &hits {
        increment(&mut source_counts, &hit.source)?;
        increment(&mut accession_counts, &hit.accession)?;
    }
    let mut warnings = Vec::new();
    if hits.is_empty() {
        warnings.push("domain input contains no hit records".to_owned());
    }
    Ok(ProteinDomainParseResult {
        format: format.to_owned(),
        sequence_count: sequence_count as u64,
        hit_count: hits.len() as u64,
        source_counts,
        accession_counts,
        hits,
        warnings,
    })
}

fn increment(map: &mut BTreeMap<String, u64>, key: &str) -> Result<(), DomainError> {
    let entry = map.entry(key.to_owned()).or_default();
    *entry = entry.checked_add(1).ok_or(DomainError::LimitExceeded {
        resource: "counter",
        limit: u64::MAX,
    })?;
    Ok(())
}

fn enforce_hit_limit(count: usize) -> Result<(), DomainError> {
    if count >= MAX_DOMAIN_HITS {
        return Err(DomainError::LimitExceeded {
            resource: "hit record",
            limit: MAX_DOMAIN_HITS as u64,
        });
    }
    Ok(())
}

fn nonempty(value: &str, line: usize, field: &str) -> Result<String, DomainError> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        return Err(DomainError::MalformedRecord {
            line,
            message: format!("{field} is missing"),
        });
    }
    Ok(value.to_owned())
}

fn optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "-")
        .map(str::to_owned)
}

fn split_annotations(value: Option<&str>) -> Vec<String> {
    value
        .map(|value| {
            value
                .split('|')
                .map(str::trim)
                .filter(|item| !item.is_empty() && *item != "-")
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_float(
    value: Option<&str>,
    line: usize,
    field: &str,
) -> Result<Option<f64>, DomainError> {
    match value.map(str::trim) {
        None | Some("") | Some("-") => Ok(None),
        Some(value) => parse_float(value, line, field).map(Some),
    }
}

fn parse_float(value: &str, line: usize, field: &str) -> Result<f64, DomainError> {
    let value = value
        .parse::<f64>()
        .map_err(|_| DomainError::MalformedRecord {
            line,
            message: format!("{field} is not numeric"),
        })?;
    if !value.is_finite() {
        return Err(DomainError::MalformedRecord {
            line,
            message: format!("{field} must be finite"),
        });
    }
    Ok(value)
}

fn parse_u64(value: &str, line: usize, field: &str) -> Result<u64, DomainError> {
    value
        .parse::<u64>()
        .map_err(|_| DomainError::MalformedRecord {
            line,
            message: format!("{field} is not a non-negative integer"),
        })
}

fn validate_coordinates(
    start: u64,
    end: u64,
    sequence_length: Option<u64>,
    line: usize,
) -> Result<(), DomainError> {
    if start == 0 || end < start || sequence_length.is_some_and(|length| end > length) {
        return Err(DomainError::MalformedRecord {
            line,
            message: "domain coordinates are outside the sequence bounds".to_owned(),
        });
    }
    Ok(())
}

fn read_bounded_text(path: &Path) -> Result<String, DomainError> {
    let mut probe = File::open(path)?;
    let mut magic = [0_u8; 2];
    let read = probe.read(&mut magic)?;
    let file = File::open(path)?;
    let mut reader: Box<dyn Read> = if read == 2 && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_DOMAIN_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOMAIN_DECOMPRESSED_BYTES {
        return Err(DomainError::LimitExceeded {
            resource: "decompressed byte",
            limit: MAX_DOMAIN_DECOMPRESSED_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| DomainError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::parse_protein_domains_path;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary(name: &str, content: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("linxira-{stamp}-{name}"));
        fs::write(&path, content).expect("write fixture");
        path
    }

    #[test]
    fn parses_interproscan_sources_and_annotations() {
        let path = temporary(
            "domains.tsv",
            "P1\tmd5\t100\tPfam\tPF00001\tKinase\t5\t45\t1e-20\tT\t2026-01-01\tIPR000001\tProtein kinase\tGO:0004672|GO:0005524\tReactome:R-HSA-1\nP1\tmd5\t100\tSMART\tSM00001\tDomain\t60\t90\t42\tT\t2026-01-01\t-\t-\t-\t-\n",
        );
        let result = parse_protein_domains_path(&path).expect("parse InterProScan");
        fs::remove_file(path).expect("remove fixture");
        assert_eq!(result.format, "interproscan-tsv");
        assert_eq!(result.sequence_count, 1);
        assert_eq!(result.source_counts["Pfam"], 1);
        assert_eq!(result.hits[0].go_terms.len(), 2);
    }

    #[test]
    fn parses_hmmer_domtblout_coordinates() {
        let path = temporary(
            "domains.domtblout",
            "# hmmer output\nprotein1 - 120 PF00001 PF00001.1 80 1e-30 100 0 1 1 1e-20 1e-20 90 0 1 70 10 75 8 77 0.98 Protein kinase\n",
        );
        let result = parse_protein_domains_path(&path).expect("parse domtblout");
        fs::remove_file(path).expect("remove fixture");
        assert_eq!(result.format, "hmmer-domtblout");
        assert_eq!(result.hits[0].sequence_id, "protein1");
        assert_eq!(result.hits[0].start, 10);
        assert_eq!(result.hits[0].end, 75);
    }
}
