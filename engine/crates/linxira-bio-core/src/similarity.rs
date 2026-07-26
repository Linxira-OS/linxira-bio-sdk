use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_SIMILARITY_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_SIMILARITY_HITS: usize = 2_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlastHit {
    pub query_id: String,
    pub subject_id: String,
    pub percent_identity: f64,
    pub alignment_length: u64,
    pub mismatch_count: Option<u64>,
    pub gap_open_count: Option<u64>,
    pub query_start: u64,
    pub query_end: u64,
    pub subject_start: u64,
    pub subject_end: u64,
    pub evalue: f64,
    pub bit_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlastParseResult {
    pub format: String,
    pub record_count: u64,
    pub query_count: u64,
    pub subject_count: u64,
    pub min_evalue: Option<f64>,
    pub max_bit_score: Option<f64>,
    pub mean_identity_percent: Option<f64>,
    pub hits: Vec<BlastHit>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReciprocalBestHitOptions {
    pub max_evalue: Option<f64>,
    pub min_identity_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReciprocalBestHitPair {
    pub left_id: String,
    pub right_id: String,
    pub forward_evalue: f64,
    pub reverse_evalue: f64,
    pub forward_bit_score: f64,
    pub reverse_bit_score: f64,
    pub forward_identity_percent: f64,
    pub reverse_identity_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReciprocalBestHitResult {
    pub forward_query_count: u64,
    pub reverse_query_count: u64,
    pub reciprocal_pair_count: u64,
    pub forward_unpaired_count: u64,
    pub reverse_unpaired_count: u64,
    pub pairs: Vec<ReciprocalBestHitPair>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum SimilarityError {
    Io(io::Error),
    InvalidUtf8,
    InvalidFormat(String),
    MalformedRecord { record: usize, message: String },
    LimitExceeded { resource: &'static str, limit: u64 },
    InvalidOption(String),
}

impl Display for SimilarityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "similarity input I/O failed: {error}"),
            Self::InvalidUtf8 => formatter.write_str("similarity input is not valid UTF-8 text"),
            Self::InvalidFormat(message) => {
                write!(formatter, "unsupported similarity format: {message}")
            }
            Self::MalformedRecord { record, message } => {
                write!(formatter, "malformed similarity record {record}: {message}")
            }
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "similarity parsing exceeds the deterministic {resource} limit of {limit}"
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
        }
    }
}

impl Error for SimilarityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidUtf8
            | Self::InvalidFormat(_)
            | Self::MalformedRecord { .. }
            | Self::LimitExceeded { .. }
            | Self::InvalidOption(_) => None,
        }
    }
}

impl From<io::Error> for SimilarityError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn parse_blast_path(path: impl AsRef<Path>) -> Result<BlastParseResult, SimilarityError> {
    let text = read_bounded_text(path.as_ref())?;
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let (format, hits, mut warnings) = if trimmed.starts_with('<') {
        (
            "blast-xml1".to_owned(),
            parse_legacy_blast_xml(trimmed)?,
            Vec::new(),
        )
    } else {
        let (hits, outfmt7) = parse_blast_tabular(&text)?;
        (
            if outfmt7 {
                "blast-tabular-outfmt7"
            } else {
                "blast-tabular-outfmt6"
            }
            .to_owned(),
            hits,
            Vec::new(),
        )
    };
    if hits.is_empty() {
        warnings.push("BLAST result contains no hit records".to_owned());
    }
    summarize_hits(format, hits, warnings)
}

pub fn reciprocal_best_hits_path(
    forward: impl AsRef<Path>,
    reverse: impl AsRef<Path>,
    options: ReciprocalBestHitOptions,
) -> Result<ReciprocalBestHitResult, SimilarityError> {
    validate_rbh_options(options)?;
    let forward = parse_blast_path(forward)?;
    let reverse = parse_blast_path(reverse)?;
    let forward_best = select_best_hits(&forward.hits, options);
    let reverse_best = select_best_hits(&reverse.hits, options);
    let mut pairs = Vec::new();
    let mut used_reverse_queries = BTreeSet::new();
    for (left_id, forward_hit) in &forward_best {
        let Some(reverse_hit) = reverse_best.get(&forward_hit.subject_id) else {
            continue;
        };
        if reverse_hit.subject_id != *left_id {
            continue;
        }
        used_reverse_queries.insert(forward_hit.subject_id.clone());
        pairs.push(ReciprocalBestHitPair {
            left_id: left_id.clone(),
            right_id: forward_hit.subject_id.clone(),
            forward_evalue: forward_hit.evalue,
            reverse_evalue: reverse_hit.evalue,
            forward_bit_score: forward_hit.bit_score,
            reverse_bit_score: reverse_hit.bit_score,
            forward_identity_percent: forward_hit.percent_identity,
            reverse_identity_percent: reverse_hit.percent_identity,
        });
    }
    pairs.sort_by(|left, right| {
        left.left_id
            .cmp(&right.left_id)
            .then_with(|| left.right_id.cmp(&right.right_id))
    });
    let pair_count = u64::try_from(pairs.len()).expect("pair count fits in u64");
    let forward_count = u64::try_from(forward_best.len()).expect("query count fits in u64");
    let reverse_count = u64::try_from(reverse_best.len()).expect("query count fits in u64");
    let mut warnings = Vec::new();
    if forward_best.is_empty() || reverse_best.is_empty() {
        warnings.push("one direction contains no hits after filtering".to_owned());
    }
    Ok(ReciprocalBestHitResult {
        forward_query_count: forward_count,
        reverse_query_count: reverse_count,
        reciprocal_pair_count: pair_count,
        forward_unpaired_count: forward_count.saturating_sub(pair_count),
        reverse_unpaired_count: reverse_count
            .saturating_sub(u64::try_from(used_reverse_queries.len()).expect("count fits in u64")),
        pairs,
        warnings,
    })
}

fn validate_rbh_options(options: ReciprocalBestHitOptions) -> Result<(), SimilarityError> {
    if let Some(value) = options.max_evalue
        && (!value.is_finite() || value < 0.0)
    {
        return Err(SimilarityError::InvalidOption(
            "max_evalue must be finite and non-negative".to_owned(),
        ));
    }
    if let Some(value) = options.min_identity_percent
        && (!value.is_finite() || !(0.0..=100.0).contains(&value))
    {
        return Err(SimilarityError::InvalidOption(
            "min_identity_percent must be between 0 and 100".to_owned(),
        ));
    }
    Ok(())
}

fn select_best_hits(
    hits: &[BlastHit],
    options: ReciprocalBestHitOptions,
) -> BTreeMap<String, BlastHit> {
    let mut best = BTreeMap::new();
    for hit in hits {
        if options.max_evalue.is_some_and(|limit| hit.evalue > limit)
            || options
                .min_identity_percent
                .is_some_and(|limit| hit.percent_identity < limit)
        {
            continue;
        }
        best.entry(hit.query_id.clone())
            .and_modify(|current: &mut BlastHit| {
                if compare_hit_rank(hit, current) == Ordering::Less {
                    *current = hit.clone();
                }
            })
            .or_insert_with(|| hit.clone());
    }
    best
}

fn compare_hit_rank(left: &BlastHit, right: &BlastHit) -> Ordering {
    left.evalue
        .total_cmp(&right.evalue)
        .then_with(|| right.bit_score.total_cmp(&left.bit_score))
        .then_with(|| right.percent_identity.total_cmp(&left.percent_identity))
        .then_with(|| right.alignment_length.cmp(&left.alignment_length))
        .then_with(|| left.subject_id.cmp(&right.subject_id))
}

fn summarize_hits(
    format: String,
    hits: Vec<BlastHit>,
    warnings: Vec<String>,
) -> Result<BlastParseResult, SimilarityError> {
    let queries = hits
        .iter()
        .map(|hit| hit.query_id.as_str())
        .collect::<BTreeSet<_>>();
    let subjects = hits
        .iter()
        .map(|hit| hit.subject_id.as_str())
        .collect::<BTreeSet<_>>();
    let min_evalue = hits.iter().map(|hit| hit.evalue).min_by(f64::total_cmp);
    let max_bit_score = hits.iter().map(|hit| hit.bit_score).max_by(f64::total_cmp);
    let mean_identity_percent = if hits.is_empty() {
        None
    } else {
        Some(hits.iter().map(|hit| hit.percent_identity).sum::<f64>() / hits.len() as f64)
    };
    Ok(BlastParseResult {
        format,
        record_count: u64::try_from(hits.len()).expect("record count fits in u64"),
        query_count: u64::try_from(queries.len()).expect("query count fits in u64"),
        subject_count: u64::try_from(subjects.len()).expect("subject count fits in u64"),
        min_evalue,
        max_bit_score,
        mean_identity_percent,
        hits,
        warnings,
    })
}

fn parse_blast_tabular(text: &str) -> Result<(Vec<BlastHit>, bool), SimilarityError> {
    let mut fields = default_tabular_fields();
    let mut hits = Vec::new();
    let mut saw_outfmt7 = false;
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("# Fields:") {
            fields = value.split(',').map(normalize_field_name).collect();
            saw_outfmt7 = true;
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() {
            saw_outfmt7 |= line.starts_with('#');
            continue;
        }
        if hits.len() >= MAX_SIMILARITY_HITS {
            return Err(SimilarityError::LimitExceeded {
                resource: "hit record",
                limit: MAX_SIMILARITY_HITS as u64,
            });
        }
        let values = line.split('\t').collect::<Vec<_>>();
        if values.len() != fields.len() {
            return Err(SimilarityError::MalformedRecord {
                record: line_index + 1,
                message: format!(
                    "expected {} tab-separated fields but found {}",
                    fields.len(),
                    values.len()
                ),
            });
        }
        let row = fields
            .iter()
            .zip(values)
            .map(|(field, value)| (field.as_str(), value))
            .collect::<BTreeMap<_, _>>();
        hits.push(parse_tabular_hit(&row, line_index + 1)?);
    }
    Ok((hits, saw_outfmt7))
}

fn default_tabular_fields() -> Vec<String> {
    [
        "qseqid", "sseqid", "pident", "length", "mismatch", "gapopen", "qstart", "qend", "sstart",
        "send", "evalue", "bitscore",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn normalize_field_name(field: &str) -> String {
    let normalized = field
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '%', '-', ' '], "");
    match normalized.as_str() {
        "queryid" | "queryaccver" | "queryacc" => "qseqid",
        "subjectid" | "subjectaccver" | "subjectacc" => "sseqid",
        "identity" | "identical" => "pident",
        "alignmentlength" | "alignlength" => "length",
        "mismatches" => "mismatch",
        "gapopens" => "gapopen",
        "qstart" => "qstart",
        "qend" => "qend",
        "sstart" => "sstart",
        "send" => "send",
        "bitsscore" | "bitscore" => "bitscore",
        other => other,
    }
    .to_owned()
}

fn parse_tabular_hit(
    row: &BTreeMap<&str, &str>,
    record: usize,
) -> Result<BlastHit, SimilarityError> {
    let required = |key: &str| {
        row.get(key)
            .copied()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| SimilarityError::MalformedRecord {
                record,
                message: format!("required field {key} is missing"),
            })
    };
    let hit = BlastHit {
        query_id: required("qseqid")?.to_owned(),
        subject_id: required("sseqid")?.to_owned(),
        percent_identity: parse_float(required("pident")?, record, "pident")?,
        alignment_length: parse_u64(required("length")?, record, "length")?,
        mismatch_count: row
            .get("mismatch")
            .map(|value| parse_u64(value, record, "mismatch"))
            .transpose()?,
        gap_open_count: row
            .get("gapopen")
            .map(|value| parse_u64(value, record, "gapopen"))
            .transpose()?,
        query_start: parse_u64(required("qstart")?, record, "qstart")?,
        query_end: parse_u64(required("qend")?, record, "qend")?,
        subject_start: parse_u64(required("sstart")?, record, "sstart")?,
        subject_end: parse_u64(required("send")?, record, "send")?,
        evalue: parse_float(required("evalue")?, record, "evalue")?,
        bit_score: parse_float(required("bitscore")?, record, "bitscore")?,
    };
    validate_hit(hit, record)
}

fn parse_legacy_blast_xml(text: &str) -> Result<Vec<BlastHit>, SimilarityError> {
    if !text.contains("<BlastOutput") || !text.contains("<Iteration") {
        return Err(SimilarityError::InvalidFormat(
            "XML input is not legacy NCBI BLAST XML1".to_owned(),
        ));
    }
    let mut hits = Vec::new();
    for (iteration_index, iteration) in xml_blocks(text, "Iteration").enumerate() {
        let query = xml_tag(iteration, "Iteration_query-def")
            .or_else(|| xml_tag(iteration, "Iteration_query-ID"))
            .map(first_identifier)
            .ok_or_else(|| SimilarityError::MalformedRecord {
                record: iteration_index + 1,
                message: "Iteration query identifier is missing".to_owned(),
            })?;
        for hit_block in xml_blocks(iteration, "Hit") {
            let subject = xml_tag(hit_block, "Hit_id")
                .or_else(|| xml_tag(hit_block, "Hit_accession"))
                .map(first_identifier)
                .ok_or_else(|| SimilarityError::MalformedRecord {
                    record: iteration_index + 1,
                    message: "Hit identifier is missing".to_owned(),
                })?;
            for hsp in xml_blocks(hit_block, "Hsp") {
                if hits.len() >= MAX_SIMILARITY_HITS {
                    return Err(SimilarityError::LimitExceeded {
                        resource: "hit record",
                        limit: MAX_SIMILARITY_HITS as u64,
                    });
                }
                let alignment_length = xml_u64(hsp, "Hsp_align-len", iteration_index + 1)?;
                let identity = xml_u64(hsp, "Hsp_identity", iteration_index + 1)?;
                let gaps = xml_tag(hsp, "Hsp_gaps")
                    .map(|value| parse_u64(&value, iteration_index + 1, "Hsp_gaps"))
                    .transpose()?;
                let mismatches = alignment_length
                    .checked_sub(identity)
                    .and_then(|value| value.checked_sub(gaps.unwrap_or(0)));
                let percent_identity = if alignment_length == 0 {
                    0.0
                } else {
                    identity as f64 * 100.0 / alignment_length as f64
                };
                hits.push(validate_hit(
                    BlastHit {
                        query_id: query.clone(),
                        subject_id: subject.clone(),
                        percent_identity,
                        alignment_length,
                        mismatch_count: mismatches,
                        gap_open_count: None,
                        query_start: xml_u64(hsp, "Hsp_query-from", iteration_index + 1)?,
                        query_end: xml_u64(hsp, "Hsp_query-to", iteration_index + 1)?,
                        subject_start: xml_u64(hsp, "Hsp_hit-from", iteration_index + 1)?,
                        subject_end: xml_u64(hsp, "Hsp_hit-to", iteration_index + 1)?,
                        evalue: xml_f64(hsp, "Hsp_evalue", iteration_index + 1)?,
                        bit_score: xml_f64(hsp, "Hsp_bit-score", iteration_index + 1)?,
                    },
                    iteration_index + 1,
                )?);
            }
        }
    }
    Ok(hits)
}

fn xml_blocks<'a>(text: &'a str, tag: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let mut remainder = text;
    std::iter::from_fn(move || {
        let start_index = remainder.find(&start)? + start.len();
        remainder = &remainder[start_index..];
        let end_index = remainder.find(&end)?;
        let block = &remainder[..end_index];
        remainder = &remainder[end_index + end.len()..];
        Some(block)
    })
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let start_index = text.find(&start)? + start.len();
    let tail = &text[start_index..];
    let end_index = tail.find(&end)?;
    Some(xml_unescape(tail[..end_index].trim()))
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn first_identifier(value: String) -> String {
    value
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn xml_u64(text: &str, tag: &str, record: usize) -> Result<u64, SimilarityError> {
    let value = xml_tag(text, tag).ok_or_else(|| SimilarityError::MalformedRecord {
        record,
        message: format!("{tag} is missing"),
    })?;
    parse_u64(&value, record, tag)
}

fn xml_f64(text: &str, tag: &str, record: usize) -> Result<f64, SimilarityError> {
    let value = xml_tag(text, tag).ok_or_else(|| SimilarityError::MalformedRecord {
        record,
        message: format!("{tag} is missing"),
    })?;
    parse_float(&value, record, tag)
}

fn parse_u64(value: &str, record: usize, field: &str) -> Result<u64, SimilarityError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| SimilarityError::MalformedRecord {
            record,
            message: format!("{field} is not a non-negative integer"),
        })
}

fn parse_float(value: &str, record: usize, field: &str) -> Result<f64, SimilarityError> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| SimilarityError::MalformedRecord {
            record,
            message: format!("{field} is not numeric"),
        })?;
    if !parsed.is_finite() {
        return Err(SimilarityError::MalformedRecord {
            record,
            message: format!("{field} must be finite"),
        });
    }
    Ok(parsed)
}

fn validate_hit(hit: BlastHit, record: usize) -> Result<BlastHit, SimilarityError> {
    if hit.query_id.is_empty() || hit.subject_id.is_empty() {
        return Err(SimilarityError::MalformedRecord {
            record,
            message: "query and subject identifiers must be non-empty".to_owned(),
        });
    }
    if !(0.0..=100.0).contains(&hit.percent_identity)
        || hit.alignment_length == 0
        || hit.query_start == 0
        || hit.query_end == 0
        || hit.subject_start == 0
        || hit.subject_end == 0
        || hit.evalue < 0.0
    {
        return Err(SimilarityError::MalformedRecord {
            record,
            message:
                "identity, coordinates, alignment length, or e-value is outside its valid range"
                    .to_owned(),
        });
    }
    Ok(hit)
}

fn read_bounded_text(path: &Path) -> Result<String, SimilarityError> {
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
        .take(MAX_SIMILARITY_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SIMILARITY_DECOMPRESSED_BYTES {
        return Err(SimilarityError::LimitExceeded {
            resource: "decompressed byte",
            limit: MAX_SIMILARITY_DECOMPRESSED_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| SimilarityError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::{ReciprocalBestHitOptions, parse_blast_path, reciprocal_best_hits_path};
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
    fn parses_outfmt6_and_outfmt7_fields() {
        let path = temporary(
            "blast.tsv",
            "# Fields: query id, subject id, % identity, alignment length, mismatches, gap opens, q. start, q. end, s. start, s. end, evalue, bit score\nq1\ts1\t95\t100\t5\t0\t1\t100\t4\t103\t1e-20\t80\n",
        );
        let result = parse_blast_path(&path).expect("parse tabular BLAST");
        fs::remove_file(path).expect("remove fixture");
        assert_eq!(result.format, "blast-tabular-outfmt7");
        assert_eq!(result.record_count, 1);
        assert_eq!(result.hits[0].subject_id, "s1");
        assert_eq!(result.hits[0].bit_score, 80.0);
    }

    #[test]
    fn parses_legacy_xml_hsps() {
        let path = temporary(
            "blast.xml",
            r#"<?xml version="1.0"?>
<BlastOutput><BlastOutput_iterations><Iteration>
<Iteration_query-def>q1 query</Iteration_query-def>
<Iteration_hits><Hit><Hit_id>s1 subject</Hit_id><Hit_hsps><Hsp>
<Hsp_bit-score>50</Hsp_bit-score><Hsp_evalue>1e-10</Hsp_evalue>
<Hsp_query-from>1</Hsp_query-from><Hsp_query-to>20</Hsp_query-to>
<Hsp_hit-from>3</Hsp_hit-from><Hsp_hit-to>22</Hsp_hit-to>
<Hsp_identity>18</Hsp_identity><Hsp_gaps>1</Hsp_gaps><Hsp_align-len>20</Hsp_align-len>
</Hsp></Hit_hsps></Hit></Iteration_hits></Iteration></BlastOutput_iterations></BlastOutput>"#,
        );
        let result = parse_blast_path(&path).expect("parse XML BLAST");
        fs::remove_file(path).expect("remove fixture");
        assert_eq!(result.format, "blast-xml1");
        assert_eq!(result.hits[0].percent_identity, 90.0);
        assert_eq!(result.hits[0].mismatch_count, Some(1));
    }

    #[test]
    fn finds_deterministic_reciprocal_best_hits() {
        let forward = temporary(
            "forward.tsv",
            "a\tx\t90\t10\t1\t0\t1\t10\t1\t10\t1e-20\t50\na\ty\t99\t10\t0\t0\t1\t10\t1\t10\t1e-10\t100\nb\ty\t80\t10\t2\t0\t1\t10\t1\t10\t1e-5\t20\n",
        );
        let reverse = temporary(
            "reverse.tsv",
            "x\ta\t91\t10\t1\t0\t1\t10\t1\t10\t1e-30\t60\ny\tb\t85\t10\t1\t0\t1\t10\t1\t10\t1e-6\t30\n",
        );
        let result =
            reciprocal_best_hits_path(&forward, &reverse, ReciprocalBestHitOptions::default())
                .expect("compute RBH");
        fs::remove_file(forward).expect("remove forward");
        fs::remove_file(reverse).expect("remove reverse");
        assert_eq!(result.reciprocal_pair_count, 2);
        assert_eq!(result.pairs[0].left_id, "a");
        assert_eq!(result.pairs[0].right_id, "x");
    }
}
