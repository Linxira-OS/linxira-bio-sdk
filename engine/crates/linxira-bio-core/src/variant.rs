use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const NO_SAMPLES_WARNING: &str = "VCF header declares no samples; genotype metrics are unavailable";
const NO_RECORDS_WARNING: &str = "VCF contains no variant records";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct VcfStats {
    pub record_count: u64,
    pub sample_count: u64,
    pub pass_record_count: u64,
    pub filtered_record_count: u64,
    /// Number of alternate alleles classified as single-nucleotide variants.
    pub snp_count: u64,
    /// Number of alternate alleles whose sequence length differs from REF.
    pub indel_count: u64,
    /// Number of alternate alleles that replace two or more equally sized bases.
    pub mnv_count: u64,
    /// Number of symbolic, spanning-deletion, or breakend alternate alleles.
    pub symbolic_count: u64,
    pub multiallelic_record_count: u64,
    pub transition_count: u64,
    pub transversion_count: u64,
    pub ti_tv_ratio: Option<f64>,
    pub missing_genotype_count: u64,
    pub called_genotype_count: u64,
    pub missing_genotype_rate: Option<f64>,
    pub contig_counts: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum VcfError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MissingHeader,
    InvalidHeader { line: usize, message: String },
    MalformedRecord { line: usize, message: String },
}

impl Display for VcfError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read VCF: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read VCF at line {line}: {source}")
            }
            Self::MissingHeader => formatter.write_str("VCF column header is missing"),
            Self::InvalidHeader { line, message } => {
                write!(formatter, "invalid VCF header at line {line}: {message}")
            }
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed VCF record at line {line}: {message}")
            }
        }
    }
}

impl Error for VcfError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for VcfError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct VcfHeader {
    column_count: usize,
    sample_count: usize,
}

pub fn vcf_stats_path(path: impl AsRef<Path>) -> Result<VcfStats, VcfError> {
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };

    vcf_stats(BufReader::new(input))
}

fn vcf_stats(mut reader: impl BufRead) -> Result<VcfStats, VcfError> {
    let mut stats = VcfStats::default();
    let mut header = None;
    let mut saw_file_format = false;
    let mut line_number = 0_usize;
    let mut buffer = String::new();

    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| VcfError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }

        let line = buffer.trim_end_matches(['\r', '\n']);
        if header.is_none() {
            if line_number == 1 {
                validate_file_format(line)?;
                saw_file_format = true;
                continue;
            }
            if line.starts_with("##") {
                if line.starts_with("##fileformat=") {
                    return Err(VcfError::InvalidHeader {
                        line: line_number,
                        message:
                            "the fileformat declaration must appear exactly once as the first line"
                                .to_owned(),
                    });
                }
                validate_meta_line(line, line_number)?;
                continue;
            }
            if line.starts_with('#') {
                if !saw_file_format {
                    return Err(VcfError::InvalidHeader {
                        line: line_number,
                        message: "missing fileformat declaration".to_owned(),
                    });
                }
                let parsed_header = parse_column_header(line, line_number)?;
                stats.sample_count = u64::try_from(parsed_header.sample_count)
                    .expect("VCF sample count fits in u64");
                header = Some(parsed_header);
                continue;
            }

            return Err(VcfError::InvalidHeader {
                line: line_number,
                message: "record data appears before the #CHROM column header".to_owned(),
            });
        }

        if line.starts_with('#') || line.is_empty() {
            return Err(VcfError::MalformedRecord {
                line: line_number,
                message: if line.is_empty() {
                    "blank lines are not permitted after the column header".to_owned()
                } else {
                    "header or comment line appears after the #CHROM column header".to_owned()
                },
            });
        }

        parse_record(
            line,
            line_number,
            header.expect("VCF header was parsed"),
            &mut stats,
        )?;
    }

    if header.is_none() {
        return Err(VcfError::MissingHeader);
    }

    stats.ti_tv_ratio = ratio_if_nonzero(stats.transition_count, stats.transversion_count);
    let genotype_count = stats
        .missing_genotype_count
        .saturating_add(stats.called_genotype_count);
    stats.missing_genotype_rate = ratio_if_nonzero(stats.missing_genotype_count, genotype_count);

    if stats.sample_count == 0 {
        stats.warnings.push(NO_SAMPLES_WARNING.to_owned());
    }
    if stats.record_count == 0 {
        stats.warnings.push(NO_RECORDS_WARNING.to_owned());
    }

    Ok(stats)
}

fn validate_file_format(line: &str) -> Result<(), VcfError> {
    let Some(version) = line.strip_prefix("##fileformat=VCFv") else {
        return Err(VcfError::InvalidHeader {
            line: 1,
            message: "the first line must be a ##fileformat=VCFv... declaration".to_owned(),
        });
    };
    let mut components = version.split('.');
    let major = components.next().unwrap_or_default();
    let minor = components.next().unwrap_or_default();
    if major.is_empty()
        || minor.is_empty()
        || components.next().is_some()
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(VcfError::InvalidHeader {
            line: 1,
            message: format!("invalid VCF version {version:?}"),
        });
    }
    Ok(())
}

fn validate_meta_line(line: &str, line_number: usize) -> Result<(), VcfError> {
    let Some((key, _value)) = line[2..].split_once('=') else {
        return Err(VcfError::InvalidHeader {
            line: line_number,
            message: "meta-information lines must use ##key=value syntax".to_owned(),
        });
    };
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(VcfError::InvalidHeader {
            line: line_number,
            message: format!("invalid meta-information key {key:?}"),
        });
    }
    Ok(())
}

fn parse_column_header(line: &str, line_number: usize) -> Result<VcfHeader, VcfError> {
    const REQUIRED_COLUMNS: [&str; 8] = [
        "#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO",
    ];

    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() < REQUIRED_COLUMNS.len() {
        return Err(VcfError::InvalidHeader {
            line: line_number,
            message: format!(
                "expected at least 8 tab-separated columns, found {}",
                columns.len()
            ),
        });
    }
    for (index, expected) in REQUIRED_COLUMNS.iter().enumerate() {
        if columns[index] != *expected {
            return Err(VcfError::InvalidHeader {
                line: line_number,
                message: format!(
                    "column {} must be {expected}, found {:?}",
                    index + 1,
                    columns[index]
                ),
            });
        }
    }
    if columns.len() > REQUIRED_COLUMNS.len() && columns[8] != "FORMAT" {
        return Err(VcfError::InvalidHeader {
            line: line_number,
            message: format!("column 9 must be FORMAT, found {:?}", columns[8]),
        });
    }

    let samples = columns.get(9..).unwrap_or_default();
    let mut unique_samples = BTreeSet::new();
    for sample in samples {
        if sample.is_empty() {
            return Err(VcfError::InvalidHeader {
                line: line_number,
                message: "sample names must not be empty".to_owned(),
            });
        }
        if !unique_samples.insert(*sample) {
            return Err(VcfError::InvalidHeader {
                line: line_number,
                message: format!("duplicate sample name {sample:?}"),
            });
        }
    }

    Ok(VcfHeader {
        column_count: columns.len(),
        sample_count: samples.len(),
    })
}

fn parse_record(
    line: &str,
    line_number: usize,
    header: VcfHeader,
    stats: &mut VcfStats,
) -> Result<(), VcfError> {
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() < 8 {
        return malformed(
            line_number,
            format!(
                "expected at least 8 tab-separated columns, found {}",
                columns.len()
            ),
        );
    }
    if columns.len() != header.column_count {
        return malformed(
            line_number,
            format!(
                "expected {} columns to match the header, found {}",
                header.column_count,
                columns.len()
            ),
        );
    }
    if columns[0].is_empty() || columns[0] == "." {
        return malformed(line_number, "CHROM must identify a contig");
    }
    match columns[1].parse::<u64>() {
        Ok(position) if position > 0 => {}
        _ => return malformed(line_number, format!("invalid POS value {:?}", columns[1])),
    }
    if columns[3].is_empty() || columns[3] == "." {
        return malformed(line_number, "REF must contain an allele");
    }
    if columns[4].is_empty() {
        return malformed(line_number, "ALT must contain an allele or '.'");
    }
    if columns[6].is_empty() {
        return malformed(
            line_number,
            "FILTER must contain PASS, '.', or a filter name",
        );
    }

    let alternate_alleles = parse_alternate_alleles(columns[4], line_number)?;
    let (missing_genotypes, called_genotypes) =
        genotype_counts(&columns, &alternate_alleles, header, line_number)?;

    stats.record_count += 1;
    match columns[6] {
        "PASS" => stats.pass_record_count += 1,
        "." => {}
        _ => stats.filtered_record_count += 1,
    }
    if alternate_alleles.len() > 1 {
        stats.multiallelic_record_count += 1;
    }

    let reference = columns[3];
    for alternate in &alternate_alleles {
        match classify_allele(reference, alternate) {
            AlleleClass::Snp => stats.snp_count += 1,
            AlleleClass::Indel => stats.indel_count += 1,
            AlleleClass::Mnv => stats.mnv_count += 1,
            AlleleClass::Symbolic => stats.symbolic_count += 1,
        }
    }

    if alternate_alleles.len() == 1
        && classify_allele(reference, alternate_alleles[0]) == AlleleClass::Snp
    {
        match substitution_kind(reference, alternate_alleles[0]) {
            Some(SubstitutionKind::Transition) => stats.transition_count += 1,
            Some(SubstitutionKind::Transversion) => stats.transversion_count += 1,
            None => {}
        }
    }

    stats.missing_genotype_count += missing_genotypes;
    stats.called_genotype_count += called_genotypes;
    *stats
        .contig_counts
        .entry(columns[0].to_owned())
        .or_default() += 1;

    Ok(())
}

fn parse_alternate_alleles(field: &str, line_number: usize) -> Result<Vec<&str>, VcfError> {
    if field == "." {
        return Ok(Vec::new());
    }

    let alleles: Vec<&str> = field.split(',').collect();
    if alleles
        .iter()
        .any(|allele| allele.is_empty() || *allele == ".")
    {
        return malformed(
            line_number,
            "ALT contains an empty or missing allele in a non-missing allele list",
        );
    }
    Ok(alleles)
}

fn genotype_counts(
    columns: &[&str],
    alternate_alleles: &[&str],
    header: VcfHeader,
    line_number: usize,
) -> Result<(u64, u64), VcfError> {
    if header.sample_count == 0 {
        return Ok((0, 0));
    }

    let format_fields: Vec<&str> = columns[8].split(':').collect();
    if format_fields.iter().any(|field| field.is_empty()) {
        return malformed(line_number, "FORMAT contains an empty field name");
    }
    let mut unique_fields = BTreeSet::new();
    for field in &format_fields {
        if !unique_fields.insert(*field) {
            return malformed(
                line_number,
                format!("FORMAT contains duplicate field name {field:?}"),
            );
        }
    }

    let Some(gt_index) = format_fields.iter().position(|field| *field == "GT") else {
        return Ok((0, 0));
    };
    if gt_index != 0 {
        return malformed(
            line_number,
            "GT must be the first FORMAT field when present",
        );
    }

    let mut missing = 0_u64;
    let mut called = 0_u64;
    for (sample_index, sample) in columns[9..].iter().enumerate() {
        if sample.is_empty() {
            return malformed(
                line_number,
                format!("sample column {} is empty", sample_index + 1),
            );
        }
        let values: Vec<&str> = sample.split(':').collect();
        if values.len() > format_fields.len() {
            return malformed(
                line_number,
                format!(
                    "sample column {} has more values than FORMAT declares",
                    sample_index + 1
                ),
            );
        }
        let genotype = values.get(gt_index).copied().unwrap_or(".");
        if genotype_is_missing(genotype, alternate_alleles.len(), line_number)? {
            missing += 1;
        } else {
            called += 1;
        }
    }
    Ok((missing, called))
}

fn genotype_is_missing(
    genotype: &str,
    alternate_count: usize,
    line_number: usize,
) -> Result<bool, VcfError> {
    if genotype.is_empty() {
        return malformed(line_number, "GT value is empty");
    }

    let mut missing = false;
    for allele in genotype.split(['/', '|']) {
        if allele.is_empty() {
            return malformed(line_number, format!("invalid GT value {genotype:?}"));
        }
        if allele == "." {
            missing = true;
            continue;
        }
        let index = allele
            .parse::<usize>()
            .map_err(|_| VcfError::MalformedRecord {
                line: line_number,
                message: format!("invalid GT allele {allele:?} in {genotype:?}"),
            })?;
        if index > alternate_count {
            return malformed(
                line_number,
                format!(
                    "GT allele index {index} exceeds the {} alternate alleles",
                    alternate_count
                ),
            );
        }
    }
    Ok(missing)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlleleClass {
    Snp,
    Indel,
    Mnv,
    Symbolic,
}

fn classify_allele(reference: &str, alternate: &str) -> AlleleClass {
    if alternate == "*"
        || (alternate.starts_with('<') && alternate.ends_with('>'))
        || alternate.contains(['[', ']'])
    {
        AlleleClass::Symbolic
    } else if reference.len() != alternate.len() {
        AlleleClass::Indel
    } else if reference.len() == 1 {
        AlleleClass::Snp
    } else {
        AlleleClass::Mnv
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubstitutionKind {
    Transition,
    Transversion,
}

fn substitution_kind(reference: &str, alternate: &str) -> Option<SubstitutionKind> {
    let reference = reference.as_bytes().first()?.to_ascii_uppercase();
    let alternate = alternate.as_bytes().first()?.to_ascii_uppercase();
    if !matches!(reference, b'A' | b'C' | b'G' | b'T')
        || !matches!(alternate, b'A' | b'C' | b'G' | b'T')
        || reference == alternate
    {
        return None;
    }

    if matches!(
        (reference, alternate),
        (b'A', b'G') | (b'G', b'A') | (b'C', b'T') | (b'T', b'C')
    ) {
        Some(SubstitutionKind::Transition)
    } else {
        Some(SubstitutionKind::Transversion)
    }
}

fn ratio_if_nonzero(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator != 0).then_some(numerator as f64 / denominator as f64)
}

fn malformed<T>(line: usize, message: impl Into<String>) -> Result<T, VcfError> {
    Err(VcfError::MalformedRecord {
        line,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{VcfError, vcf_stats, vcf_stats_path};
    use flate2::{Compression, GzBuilder, write::GzEncoder};
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn summarizes_record_and_allele_metrics() {
        let stats = vcf_stats_path(fixture("mixed.vcf")).expect("summarize valid VCF");

        assert_eq!(stats.record_count, 7);
        assert_eq!(stats.sample_count, 2);
        assert_eq!(stats.pass_record_count, 4);
        assert_eq!(stats.filtered_record_count, 2);
        assert_eq!(stats.snp_count, 4);
        assert_eq!(stats.indel_count, 1);
        assert_eq!(stats.mnv_count, 1);
        assert_eq!(stats.symbolic_count, 2);
        assert_eq!(stats.multiallelic_record_count, 1);
        assert_eq!(stats.transition_count, 1);
        assert_eq!(stats.transversion_count, 1);
        assert_eq!(stats.ti_tv_ratio, Some(1.0));
        assert_eq!(stats.missing_genotype_count, 3);
        assert_eq!(stats.called_genotype_count, 11);
        assert_eq!(stats.missing_genotype_rate, Some(3.0 / 14.0));
        assert_eq!(stats.contig_counts["chr1"], 2);
        assert_eq!(stats.contig_counts["chr2"], 2);
        assert_eq!(stats.contig_counts["chrX"], 3);
        assert!(stats.warnings.is_empty());
    }

    #[test]
    fn warns_for_no_samples_and_no_records() {
        let stats = vcf_stats_path(fixture("empty.vcf")).expect("summarize empty VCF");

        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.record_count, 0);
        assert_eq!(stats.missing_genotype_rate, None);
        assert_eq!(stats.warnings.len(), 2);
        assert!(stats.warnings[0].contains("no samples"));
        assert!(stats.warnings[1].contains("no variant records"));
    }

    #[test]
    fn excludes_records_without_gt_from_missing_rate() {
        let input = concat!(
            "##fileformat=VCFv4.3\n",
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample\n",
            "chr1\t1\t.\tA\tG\t.\tPASS\t.\tDP\t8\n",
            "chr1\t2\t.\tC\tT\t.\tPASS\t.\tGT:DP\t.:0\n",
        );

        let stats = vcf_stats(Cursor::new(input)).expect("summarize VCF");

        assert_eq!(stats.missing_genotype_count, 1);
        assert_eq!(stats.called_genotype_count, 0);
        assert_eq!(stats.missing_genotype_rate, Some(1.0));
    }

    #[test]
    fn classifies_each_allele_in_a_mixed_multiallelic_record() {
        let input = concat!(
            "##fileformat=VCFv4.3\n",
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
            "chr1\t1\t.\tA\tG,AT,<DUP>\t.\tPASS\t.\n",
        );

        let stats = vcf_stats(Cursor::new(input)).expect("summarize multiallelic VCF");

        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.multiallelic_record_count, 1);
        assert_eq!(stats.snp_count, 1);
        assert_eq!(stats.indel_count, 1);
        assert_eq!(stats.symbolic_count, 1);
        assert_eq!(stats.transition_count, 0);
        assert_eq!(stats.transversion_count, 0);
    }

    #[test]
    fn rejects_malformed_records_with_line_context() {
        let error = vcf_stats_path(fixture("malformed-columns.vcf"))
            .expect_err("seven-column VCF record must fail");

        assert!(matches!(error, VcfError::MalformedRecord { line: 3, .. }));
        assert!(error.to_string().contains("line 3"));
    }

    #[test]
    fn reads_gzip_by_magic_bytes() {
        let path = temporary_path("compressed.data");
        let mut encoder = GzEncoder::new(
            fs::File::create(&path).expect("create gzip VCF"),
            Compression::default(),
        );
        encoder
            .write_all(minimal_vcf().as_bytes())
            .expect("write gzip VCF");
        encoder.finish().expect("finish gzip VCF");

        let stats = vcf_stats_path(&path).expect("summarize gzip VCF");
        fs::remove_file(path).expect("remove gzip VCF");

        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.snp_count, 1);
    }

    #[test]
    fn reads_bgzf_by_magic_bytes() {
        let path = temporary_path("blocked.data");
        let mut encoder = GzBuilder::new()
            .extra(vec![b'B', b'C', 2, 0, 0, 0])
            .write(Vec::new(), Compression::default());
        encoder
            .write_all(minimal_vcf().as_bytes())
            .expect("write BGZF VCF");
        let mut block = encoder.finish().expect("finish BGZF VCF");
        let block_size = u16::try_from(block.len() - 1).expect("test BGZF block fits in u16");
        block[16..18].copy_from_slice(&block_size.to_le_bytes());
        fs::write(&path, block).expect("create BGZF VCF");

        let stats = vcf_stats_path(&path).expect("summarize BGZF VCF");
        fs::remove_file(path).expect("remove BGZF VCF");

        assert_eq!(stats.record_count, 1);
        assert_eq!(stats.transition_count, 1);
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/variant-stats")
            .join(name)
    }

    fn temporary_path(name: &str) -> PathBuf {
        let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-variant-stats-{}-{counter}-{name}",
            std::process::id()
        ))
    }

    fn minimal_vcf() -> String {
        concat!(
            "##fileformat=VCFv4.3\n",
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
            "chr1\t1\t.\tA\tG\t.\tPASS\t.\n",
        )
        .to_owned()
    }
}
