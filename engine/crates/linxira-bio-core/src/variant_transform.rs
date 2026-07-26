use crate::sequence_transform::{SequenceTransformError, visit_fasta_path};
use crate::variant::{VcfError, vcf_stats_path};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum VariantTransformError {
    Io(io::Error),
    Vcf(VcfError),
    Fasta(SequenceTransformError),
    OutputAlreadyExists(PathBuf),
    InvalidOption(String),
    InvalidRecord { line: usize, message: String },
}

impl Display for VariantTransformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "variant operation failed: {error}"),
            Self::Vcf(error) => Display::fmt(error, formatter),
            Self::Fasta(error) => Display::fmt(error, formatter),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::InvalidRecord { line, message } => {
                write!(formatter, "invalid VCF record at line {line}: {message}")
            }
        }
    }
}

impl Error for VariantTransformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Vcf(error) => Some(error),
            Self::Fasta(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for VariantTransformError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<VcfError> for VariantTransformError {
    fn from(error: VcfError) -> Self {
        Self::Vcf(error)
    }
}

impl From<SequenceTransformError> for VariantTransformError {
    fn from(error: SequenceTransformError) -> Self {
        Self::Fasta(error)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariantFilterOptions {
    pub min_qual: Option<f64>,
    pub require_pass: bool,
    pub contigs: Vec<String>,
    pub min_info_dp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantFilterSummary {
    pub input_records: u64,
    pub output_records: u64,
    pub rejected_by_qual: u64,
    pub rejected_by_filter: u64,
    pub rejected_by_contig: u64,
    pub rejected_by_info_dp: u64,
}

pub fn filter_vcf_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &VariantFilterOptions,
) -> Result<VariantFilterSummary, VariantTransformError> {
    if options.min_qual.is_some_and(|value| !value.is_finite()) {
        return Err(VariantTransformError::InvalidOption(
            "minimum QUAL must be finite".to_owned(),
        ));
    }
    let input = input.as_ref();
    vcf_stats_path(input)?;
    let allowed_contigs = options.contigs.iter().cloned().collect::<BTreeSet<_>>();
    if allowed_contigs.contains("") {
        return Err(VariantTransformError::InvalidOption(
            "contig filters must not be empty".to_owned(),
        ));
    }
    let mut summary = VariantFilterSummary {
        input_records: 0,
        output_records: 0,
        rejected_by_qual: 0,
        rejected_by_filter: 0,
        rejected_by_contig: 0,
        rejected_by_info_dp: 0,
    };

    with_new_vcf_output(output.as_ref(), |writer| {
        let mut reader = open_vcf_reader(input)?;
        let mut buffer = String::new();
        let mut line_number = 0_usize;
        loop {
            buffer.clear();
            if reader.read_line(&mut buffer)? == 0 {
                break;
            }
            line_number += 1;
            if buffer.starts_with('#') {
                writer.write_all(buffer.as_bytes())?;
                continue;
            }
            let line = buffer.trim_end_matches(['\r', '\n']);
            let columns = line.split('\t').collect::<Vec<_>>();
            summary.input_records += 1;

            if let Some(minimum) = options.min_qual {
                let passes = columns[5] != "."
                    && columns[5]
                        .parse::<f64>()
                        .is_ok_and(|quality| quality.is_finite() && quality >= minimum);
                if !passes {
                    summary.rejected_by_qual += 1;
                    continue;
                }
            }
            if options.require_pass && columns[6] != "PASS" {
                summary.rejected_by_filter += 1;
                continue;
            }
            if !allowed_contigs.is_empty() && !allowed_contigs.contains(columns[0]) {
                summary.rejected_by_contig += 1;
                continue;
            }
            if let Some(minimum) = options.min_info_dp {
                let depth = info_integer(columns[7], "DP", line_number)?;
                if depth.is_none_or(|depth| depth < minimum) {
                    summary.rejected_by_info_dp += 1;
                    continue;
                }
            }
            writer.write_all(buffer.as_bytes())?;
            if !buffer.ends_with('\n') {
                writer.write_all(b"\n")?;
            }
            summary.output_records += 1;
        }
        Ok(())
    })?;
    Ok(summary)
}

fn info_integer(info: &str, key: &str, line: usize) -> Result<Option<u64>, VariantTransformError> {
    for field in info.split(';') {
        let Some((field_key, value)) = field.split_once('=') else {
            continue;
        };
        if field_key == key {
            return value.parse::<u64>().map(Some).map_err(|_| {
                VariantTransformError::InvalidRecord {
                    line,
                    message: format!("INFO/{key} must be a non-negative integer"),
                }
            });
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantNormalizeSummary {
    pub input_records: u64,
    pub output_records: u64,
    pub changed_records: u64,
    pub left_aligned_records: u64,
    pub reference_validated_records: u64,
}

pub fn normalize_vcf_path(
    input: impl AsRef<Path>,
    reference: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<VariantNormalizeSummary, VariantTransformError> {
    let input = input.as_ref();
    vcf_stats_path(input)?;
    let reference_sequences = load_reference(reference.as_ref())?;
    let mut summary = VariantNormalizeSummary {
        input_records: 0,
        output_records: 0,
        changed_records: 0,
        left_aligned_records: 0,
        reference_validated_records: 0,
    };

    with_new_vcf_output(output.as_ref(), |writer| {
        let mut reader = open_vcf_reader(input)?;
        let mut buffer = String::new();
        let mut line_number = 0_usize;
        loop {
            buffer.clear();
            if reader.read_line(&mut buffer)? == 0 {
                break;
            }
            line_number += 1;
            if buffer.starts_with('#') {
                writer.write_all(buffer.as_bytes())?;
                continue;
            }
            let line = buffer.trim_end_matches(['\r', '\n']);
            let mut columns = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            summary.input_records += 1;
            let contig = reference_sequences.get(&columns[0]).ok_or_else(|| {
                VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: format!("reference FASTA has no contig {:?}", columns[0]),
                }
            })?;
            if columns[4] == "." || columns[4].contains(',') {
                return Err(VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: "normalization requires exactly one ALT allele".to_owned(),
                });
            }
            if is_symbolic_allele(&columns[3]) || is_symbolic_allele(&columns[4]) {
                return Err(VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: "symbolic, spanning-deletion, and breakend alleles are unsupported"
                        .to_owned(),
                });
            }
            let position =
                columns[1]
                    .parse::<usize>()
                    .map_err(|_| VariantTransformError::InvalidRecord {
                        line: line_number,
                        message: "POS does not fit this platform".to_owned(),
                    })?;
            let reference_allele = normalize_allele(&columns[3], line_number)?;
            let alternate_allele = normalize_allele(&columns[4], line_number)?;
            validate_reference(contig, position, &reference_allele, line_number)?;
            summary.reference_validated_records += 1;
            let normalized = normalize_small_variant(
                contig,
                position,
                reference_allele,
                alternate_allele,
                line_number,
            )?;
            if normalized.position != position
                || normalized.reference != columns[3].as_bytes()
                || normalized.alternate != columns[4].as_bytes()
            {
                summary.changed_records += 1;
            }
            if normalized.position < position {
                summary.left_aligned_records += 1;
            }
            columns[1] = normalized.position.to_string();
            columns[3] = String::from_utf8(normalized.reference)
                .expect("validated alleles contain ASCII nucleotides");
            columns[4] = String::from_utf8(normalized.alternate)
                .expect("validated alleles contain ASCII nucleotides");
            writeln!(writer, "{}", columns.join("\t"))?;
            summary.output_records += 1;
        }
        Ok(())
    })?;
    Ok(summary)
}

struct NormalizedVariant {
    position: usize,
    reference: Vec<u8>,
    alternate: Vec<u8>,
}

fn normalize_small_variant(
    genome: &[u8],
    mut position: usize,
    mut reference: Vec<u8>,
    mut alternate: Vec<u8>,
    line: usize,
) -> Result<NormalizedVariant, VariantTransformError> {
    trim_common(&mut position, &mut reference, &mut alternate);
    if reference == alternate {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "REF and ALT are identical after normalization".to_owned(),
        });
    }
    if reference.len() != alternate.len() {
        loop {
            if position <= 1 {
                break;
            }
            let previous = genome[position - 2];
            let (insertion, payload) =
                if reference.len() == 1 && alternate.first() == reference.first() {
                    (true, &alternate[1..])
                } else if alternate.len() == 1 && reference.first() == alternate.first() {
                    (false, &reference[1..])
                } else {
                    break;
                };
            if payload.last() != Some(&previous) {
                break;
            }
            let mut rotated = Vec::with_capacity(payload.len());
            rotated.push(previous);
            rotated.extend_from_slice(&payload[..payload.len() - 1]);
            position -= 1;
            let anchor = genome[position - 1];
            if insertion {
                reference = vec![anchor];
                alternate = vec![anchor];
                alternate.extend_from_slice(&rotated);
            } else {
                reference = vec![anchor];
                reference.extend_from_slice(&rotated);
                alternate = vec![anchor];
            }
        }
    }
    Ok(NormalizedVariant {
        position,
        reference,
        alternate,
    })
}

fn trim_common(position: &mut usize, reference: &mut Vec<u8>, alternate: &mut Vec<u8>) {
    while reference.len() > 1 && alternate.len() > 1 && reference.last() == alternate.last() {
        reference.pop();
        alternate.pop();
    }
    let mut prefix = 0_usize;
    while reference.len() - prefix > 1
        && alternate.len() - prefix > 1
        && reference[prefix] == alternate[prefix]
    {
        prefix += 1;
    }
    if prefix > 0 {
        reference.drain(..prefix);
        alternate.drain(..prefix);
        *position += prefix;
    }
}

fn normalize_allele(value: &str, line: usize) -> Result<Vec<u8>, VariantTransformError> {
    if value.is_empty() {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "alleles must not be empty".to_owned(),
        });
    }
    value
        .bytes()
        .map(|byte| match byte.to_ascii_uppercase() {
            b'A' | b'C' | b'G' | b'T' | b'N' => Ok(byte.to_ascii_uppercase()),
            symbol => Err(VariantTransformError::InvalidRecord {
                line,
                message: format!("unsupported allele byte 0x{symbol:02x}"),
            }),
        })
        .collect()
}

fn validate_reference(
    genome: &[u8],
    position: usize,
    reference: &[u8],
    line: usize,
) -> Result<(), VariantTransformError> {
    let start = position
        .checked_sub(1)
        .ok_or_else(|| VariantTransformError::InvalidRecord {
            line,
            message: "POS must be positive".to_owned(),
        })?;
    let end =
        start
            .checked_add(reference.len())
            .ok_or_else(|| VariantTransformError::InvalidRecord {
                line,
                message: "REF interval overflows this platform".to_owned(),
            })?;
    if genome.get(start..end) != Some(reference) {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "REF does not match the supplied reference FASTA".to_owned(),
        });
    }
    Ok(())
}

fn is_symbolic_allele(allele: &str) -> bool {
    allele == "*"
        || allele.starts_with('<')
        || allele.ends_with('>')
        || allele.contains('[')
        || allele.contains(']')
}

fn load_reference(path: &Path) -> Result<HashMap<String, Vec<u8>>, VariantTransformError> {
    let mut references = HashMap::new();
    visit_fasta_path(path, |record| {
        let sequence = record
            .sequence
            .iter()
            .copied()
            .enumerate()
            .map(|(index, byte)| match byte.to_ascii_uppercase() {
                b'A' | b'C' | b'G' | b'T' | b'N' => Ok(byte.to_ascii_uppercase()),
                symbol => Err(SequenceTransformError::InvalidNucleotide {
                    identifier: record.identifier.clone(),
                    position: index + 1,
                    symbol,
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if references
            .insert(record.identifier.clone(), sequence)
            .is_some()
        {
            return Err(SequenceTransformError::InvalidOption(format!(
                "reference FASTA contains duplicate identifier {:?}",
                record.identifier
            )));
        }
        Ok(())
    })?;
    Ok(references)
}

fn open_vcf_reader(path: &Path) -> Result<Box<dyn BufRead>, VariantTransformError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    Ok(Box::new(BufReader::new(input)))
}

fn with_new_vcf_output(
    output: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<(), VariantTransformError>,
) -> Result<(), VariantTransformError> {
    if output.exists() {
        return Err(VariantTransformError::OutputAlreadyExists(
            output.to_owned(),
        ));
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = BufWriter::new(file);
    match operation(&mut writer).and_then(|()| {
        writer.flush()?;
        Ok(())
    }) {
        Ok(()) => Ok(()),
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("linxira-{nonce}-{name}"))
    }

    #[test]
    fn filters_vcf_by_quality_pass_contig_and_depth() {
        let input = temp_path("filter.vcf");
        let output = temp_path("filtered.vcf");
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t2\t.\tA\tG\t50\tPASS\tDP=20\nchr2\t2\t.\tA\tT\t10\tq10\tDP=2\n",
        )
        .unwrap();
        let summary = filter_vcf_path(
            &input,
            &output,
            &VariantFilterOptions {
                min_qual: Some(20.0),
                require_pass: true,
                contigs: vec!["chr1".to_owned()],
                min_info_dp: Some(10),
            },
        )
        .unwrap();
        assert_eq!(summary.output_records, 1);
        assert_eq!(summary.rejected_by_qual, 1);
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn validates_reference_and_left_aligns_homopolymer_indel() {
        let fasta = temp_path("reference.fa");
        let input = temp_path("normalize.vcf");
        let output = temp_path("normalized.vcf");
        fs::write(&fasta, ">chr1\nAAAAAC\n").unwrap();
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t4\t.\tA\tAA\t50\tPASS\t.\n",
        )
        .unwrap();
        let summary = normalize_vcf_path(&input, &fasta, &output).unwrap();
        assert_eq!(summary.changed_records, 1);
        assert_eq!(summary.left_aligned_records, 1);
        assert!(
            fs::read_to_string(&output)
                .unwrap()
                .contains("chr1\t1\t.\tA\tAA")
        );
        let _ = fs::remove_file(fasta);
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn rejects_multiallelic_normalization_without_creating_output() {
        let fasta = temp_path("reference-multi.fa");
        let input = temp_path("normalize-multi.vcf");
        let output = temp_path("normalized-multi.vcf");
        fs::write(&fasta, ">chr1\nAAAAAC\n").unwrap();
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t2\t.\tA\tC,G\t50\tPASS\t.\n",
        )
        .unwrap();
        assert!(normalize_vcf_path(&input, &fasta, &output).is_err());
        assert!(!output.exists());
        let _ = fs::remove_file(fasta);
        let _ = fs::remove_file(input);
    }
}
