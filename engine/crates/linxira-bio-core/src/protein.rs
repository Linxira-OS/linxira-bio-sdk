use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

pub const MAX_PROTEIN_SEQUENCES: u64 = 100_000;
pub const MAX_PROTEIN_RESIDUES: u64 = 100_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProteinPropertiesResult {
    pub sequence_count: u64,
    pub total_residues: u64,
    pub records: Vec<ProteinProperties>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProteinProperties {
    pub id: String,
    pub description: Option<String>,
    pub length: u64,
    pub standard_residue_count: u64,
    pub ambiguous_residue_count: u64,
    pub composition: BTreeMap<String, u64>,
    pub molecular_weight_da: Option<f64>,
    pub isoelectric_point: Option<f64>,
    pub charge_at_ph7: Option<f64>,
    pub aromaticity_percent: Option<f64>,
    pub gravy: Option<f64>,
    pub extinction_coefficient_reduced: Option<u64>,
    pub extinction_coefficient_oxidized: Option<u64>,
}

#[derive(Debug)]
pub enum ProteinError {
    Io(io::Error),
    EmptyIdentifier { line: usize },
    SequenceBeforeHeader { line: usize },
    InvalidResidue { line: usize, residue: char },
    EmptySequence { id: String },
    NoRecords,
    LimitExceeded { resource: &'static str, limit: u64 },
}

impl Display for ProteinError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read protein FASTA: {error}"),
            Self::EmptyIdentifier { line } => {
                write!(
                    formatter,
                    "protein FASTA header at line {line} has no identifier"
                )
            }
            Self::SequenceBeforeHeader { line } => write!(
                formatter,
                "protein sequence appears before a FASTA header at line {line}"
            ),
            Self::InvalidResidue { line, residue } => write!(
                formatter,
                "unsupported protein residue {residue:?} at line {line}"
            ),
            Self::EmptySequence { id } => write!(formatter, "protein {id:?} has no residues"),
            Self::NoRecords => formatter.write_str("protein FASTA contains no records"),
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "protein FASTA {resource} exceeds the limit of {limit}"
            ),
        }
    }
}

impl Error for ProteinError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ProteinError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn protein_properties_path(
    path: impl AsRef<Path>,
) -> Result<ProteinPropertiesResult, ProteinError> {
    let path = path.as_ref();
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let input: Box<dyn Read> = if prefix_length == prefix.len() && prefix == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    protein_properties(BufReader::new(input))
}

pub fn protein_properties(reader: impl BufRead) -> Result<ProteinPropertiesResult, ProteinError> {
    let mut records = Vec::new();
    let mut current = None::<ProteinBuilder>;
    let mut total_residues = 0_u64;
    let mut warnings = Vec::new();

    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('>') {
            if let Some(builder) = current.take() {
                push_record(builder, &mut records, &mut warnings)?;
            }
            if records.len() as u64 >= MAX_PROTEIN_SEQUENCES {
                return Err(ProteinError::LimitExceeded {
                    resource: "sequence count",
                    limit: MAX_PROTEIN_SEQUENCES,
                });
            }
            let mut fields = header.splitn(2, char::is_whitespace);
            let id = fields.next().unwrap_or_default().trim();
            if id.is_empty() {
                return Err(ProteinError::EmptyIdentifier { line: line_number });
            }
            current = Some(ProteinBuilder {
                id: id.to_owned(),
                description: fields
                    .next()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
                sequence: Vec::new(),
            });
            continue;
        }
        let builder = current
            .as_mut()
            .ok_or(ProteinError::SequenceBeforeHeader { line: line_number })?;
        for byte in trimmed.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            let residue = byte.to_ascii_uppercase();
            if !residue.is_ascii_alphabetic() || !is_supported_residue(residue) {
                return Err(ProteinError::InvalidResidue {
                    line: line_number,
                    residue: char::from(byte),
                });
            }
            total_residues = total_residues
                .checked_add(1)
                .ok_or(ProteinError::LimitExceeded {
                    resource: "residue count",
                    limit: MAX_PROTEIN_RESIDUES,
                })?;
            if total_residues > MAX_PROTEIN_RESIDUES {
                return Err(ProteinError::LimitExceeded {
                    resource: "residue count",
                    limit: MAX_PROTEIN_RESIDUES,
                });
            }
            builder.sequence.push(residue);
        }
    }
    if let Some(builder) = current {
        push_record(builder, &mut records, &mut warnings)?;
    }
    if records.is_empty() {
        return Err(ProteinError::NoRecords);
    }
    Ok(ProteinPropertiesResult {
        sequence_count: records.len() as u64,
        total_residues,
        records,
        warnings,
    })
}

#[derive(Debug)]
struct ProteinBuilder {
    id: String,
    description: Option<String>,
    sequence: Vec<u8>,
}

fn push_record(
    builder: ProteinBuilder,
    records: &mut Vec<ProteinProperties>,
    warnings: &mut Vec<String>,
) -> Result<(), ProteinError> {
    if builder.sequence.is_empty() {
        return Err(ProteinError::EmptySequence { id: builder.id });
    }
    let properties = calculate_properties(builder);
    if properties.ambiguous_residue_count > 0 {
        warnings.push(format!(
            "protein {:?} contains {} ambiguous or non-standard residues; derived physicochemical values are null",
            properties.id, properties.ambiguous_residue_count
        ));
    }
    records.push(properties);
    Ok(())
}

fn calculate_properties(builder: ProteinBuilder) -> ProteinProperties {
    let mut counts = [0_u64; 26];
    for residue in &builder.sequence {
        counts[(residue - b'A') as usize] += 1;
    }
    let ambiguous_residue_count = AMBIGUOUS
        .iter()
        .map(|residue| counts[(residue - b'A') as usize])
        .sum::<u64>();
    let standard_residue_count = builder.sequence.len() as u64 - ambiguous_residue_count;
    let composition = counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(index, count)| (((b'A' + index as u8) as char).to_string(), *count))
        .collect::<BTreeMap<_, _>>();
    let complete = ambiguous_residue_count == 0;
    let molecular_weight_da = complete.then(|| molecular_weight(&builder.sequence));
    let isoelectric_point = complete.then(|| isoelectric_point(&counts));
    let charge_at_ph7 = complete.then(|| net_charge(&counts, 7.0));
    let aromaticity_percent = complete.then(|| {
        100.0 * (count(&counts, b'F') + count(&counts, b'W') + count(&counts, b'Y')) as f64
            / builder.sequence.len() as f64
    });
    let gravy = complete.then(|| {
        builder
            .sequence
            .iter()
            .map(|residue| hydropathy(*residue))
            .sum::<f64>()
            / builder.sequence.len() as f64
    });
    let reduced = complete.then(|| count(&counts, b'W') * 5_500 + count(&counts, b'Y') * 1_490);
    let oxidized = reduced.map(|value| value + (count(&counts, b'C') / 2) * 125);
    ProteinProperties {
        id: builder.id,
        description: builder.description,
        length: builder.sequence.len() as u64,
        standard_residue_count,
        ambiguous_residue_count,
        composition,
        molecular_weight_da,
        isoelectric_point,
        charge_at_ph7,
        aromaticity_percent,
        gravy,
        extinction_coefficient_reduced: reduced,
        extinction_coefficient_oxidized: oxidized,
    }
}

const AMBIGUOUS: &[u8] = b"BJOUXZ";

fn is_supported_residue(residue: u8) -> bool {
    b"ACDEFGHIKLMNPQRSTVWYBJOUXZ".contains(&residue)
}

fn count(counts: &[u64; 26], residue: u8) -> u64 {
    counts[(residue - b'A') as usize]
}

fn molecular_weight(sequence: &[u8]) -> f64 {
    18.015_28
        + sequence
            .iter()
            .map(|residue| residue_mass(*residue))
            .sum::<f64>()
}

fn residue_mass(residue: u8) -> f64 {
    match residue {
        b'A' => 71.0788,
        b'C' => 103.1388,
        b'D' => 115.0886,
        b'E' => 129.1155,
        b'F' => 147.1766,
        b'G' => 57.0519,
        b'H' => 137.1411,
        b'I' | b'L' => 113.1594,
        b'K' => 128.1741,
        b'M' => 131.1926,
        b'N' => 114.1038,
        b'P' => 97.1167,
        b'Q' => 128.1307,
        b'R' => 156.1875,
        b'S' => 87.0782,
        b'T' => 101.1051,
        b'V' => 99.1326,
        b'W' => 186.2132,
        b'Y' => 163.1760,
        _ => unreachable!("molecular weight only receives standard residues"),
    }
}

fn hydropathy(residue: u8) -> f64 {
    match residue {
        b'I' => 4.5,
        b'V' => 4.2,
        b'L' => 3.8,
        b'F' => 2.8,
        b'C' => 2.5,
        b'M' => 1.9,
        b'A' => 1.8,
        b'G' => -0.4,
        b'T' => -0.7,
        b'S' => -0.8,
        b'W' => -0.9,
        b'Y' => -1.3,
        b'P' => -1.6,
        b'H' => -3.2,
        b'E' | b'Q' | b'D' | b'N' => -3.5,
        b'K' => -3.9,
        b'R' => -4.5,
        _ => unreachable!("GRAVY only receives standard residues"),
    }
}

fn isoelectric_point(counts: &[u64; 26]) -> f64 {
    let mut low = 0.0;
    let mut high = 14.0;
    for _ in 0..80 {
        let middle = (low + high) * 0.5;
        if net_charge(counts, middle) > 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) * 0.5
}

fn net_charge(counts: &[u64; 26], ph: f64) -> f64 {
    let positive = positive_fraction(ph, 9.69)
        + count(counts, b'K') as f64 * positive_fraction(ph, 10.5)
        + count(counts, b'R') as f64 * positive_fraction(ph, 12.4)
        + count(counts, b'H') as f64 * positive_fraction(ph, 6.0);
    let negative = negative_fraction(ph, 2.34)
        + count(counts, b'D') as f64 * negative_fraction(ph, 3.86)
        + count(counts, b'E') as f64 * negative_fraction(ph, 4.25)
        + count(counts, b'C') as f64 * negative_fraction(ph, 8.33)
        + count(counts, b'Y') as f64 * negative_fraction(ph, 10.07);
    positive - negative
}

fn positive_fraction(ph: f64, pka: f64) -> f64 {
    1.0 / (1.0 + 10_f64.powf(ph - pka))
}

fn negative_fraction(ph: f64, pka: f64) -> f64 {
    1.0 / (1.0 + 10_f64.powf(pka - ph))
}

#[cfg(test)]
mod tests {
    use super::protein_properties;
    use std::io::Cursor;

    #[test]
    fn calculates_standard_protein_properties() {
        let result = protein_properties(Cursor::new(b">all standard\nACDEFGHIKLMNPQRSTVWY\n"))
            .expect("valid protein FASTA");
        let protein = &result.records[0];

        assert_eq!(protein.length, 20);
        assert_eq!(protein.standard_residue_count, 20);
        assert_eq!(protein.ambiguous_residue_count, 0);
        assert!(
            protein
                .molecular_weight_da
                .is_some_and(|value| value > 2_000.0)
        );
        assert!(
            protein
                .isoelectric_point
                .is_some_and(|value| (0.0..=14.0).contains(&value))
        );
        assert!(protein.gravy.is_some_and(f64::is_finite));
        assert_eq!(protein.extinction_coefficient_reduced, Some(6_990));
    }

    #[test]
    fn preserves_ambiguous_sequences_without_inventing_properties() {
        let result = protein_properties(Cursor::new(b">ambiguous\nACDX\n"))
            .expect("ambiguous protein is accepted");
        let protein = &result.records[0];

        assert_eq!(protein.ambiguous_residue_count, 1);
        assert_eq!(protein.molecular_weight_da, None);
        assert_eq!(protein.isoelectric_point, None);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn rejects_non_amino_acid_symbols_with_line_context() {
        let error = protein_properties(Cursor::new(b">bad\nACD*\n"))
            .expect_err("stop symbol is not accepted as a residue");
        assert!(error.to_string().contains("line 2"));
    }
}
