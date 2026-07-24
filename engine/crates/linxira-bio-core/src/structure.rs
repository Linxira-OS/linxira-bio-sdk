use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Maximum on-disk size accepted for a gzip-compressed PDB source.
pub const MAX_PDB_COMPRESSED_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum on-disk size accepted for an uncompressed PDB source.
pub const MAX_PDB_PLAIN_INPUT_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum bytes emitted by a decoder or read from a caller-provided stream.
pub const MAX_PDB_DECOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum atom records retained in the render-ready result.
pub const MAX_PDB_ATOMS: u64 = 100_000;
/// Maximum combined model, chain, residue, and atom records in the result.
pub const MAX_PDB_RESULT_RECORDS: u64 = 300_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PdbSummaryOptions {
    pub interpret_b_factors_as_plddt: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PdbStructureSummary {
    pub format: &'static str,
    pub coordinate_units: &'static str,
    pub model_count: u64,
    pub chain_count: u64,
    pub residue_count: u64,
    pub atom_count: u64,
    pub polymer_atom_count: u64,
    pub hetero_atom_count: u64,
    pub element_counts: BTreeMap<String, u64>,
    pub bounds: CoordinateBounds,
    pub b_factor_summary: Option<NumericSummary>,
    pub alphafold_confidence: Option<AlphaFoldConfidenceSummary>,
    pub models: Vec<PdbModelSummary>,
    pub residues: Vec<PdbResidue>,
    pub atoms: Vec<PdbAtom>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CoordinateBounds {
    pub min: Point3,
    pub max: Point3,
    pub center: Point3,
    pub span: Point3,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NumericSummary {
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlphaFoldConfidenceSummary {
    pub source: &'static str,
    pub residue_count: u64,
    pub min_plddt: f64,
    pub max_plddt: f64,
    pub mean_plddt: f64,
    pub bands: AlphaFoldConfidenceBands,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AlphaFoldConfidenceBands {
    pub very_high_count: u64,
    pub confident_count: u64,
    pub low_count: u64,
    pub very_low_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PdbModelSummary {
    pub model_id: String,
    pub atom_count: u64,
    pub residue_count: u64,
    pub chains: Vec<PdbChainSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PdbChainSummary {
    pub chain_id: String,
    pub atom_count: u64,
    pub residue_count: u64,
    pub polymer_residue_count: u64,
    pub hetero_residue_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PdbResidue {
    pub index: u64,
    pub model_id: String,
    pub chain_id: String,
    pub sequence_number: String,
    pub insertion_code: Option<String>,
    pub name: String,
    pub is_hetero: bool,
    pub atom_count: u64,
    pub plddt: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdbAtomRecord {
    Atom,
    Hetatm,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PdbAtom {
    pub index: u64,
    pub serial: String,
    pub record: PdbAtomRecord,
    pub model_id: String,
    pub residue_index: u64,
    pub name: String,
    pub alternate_location: Option<String>,
    pub residue_name: String,
    pub chain_id: String,
    pub residue_sequence_number: String,
    pub insertion_code: Option<String>,
    pub position: Point3,
    pub occupancy: Option<f64>,
    pub b_factor: Option<f64>,
    pub element: Option<String>,
    pub formal_charge: Option<String>,
}

#[derive(Debug)]
pub enum PdbError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    LimitExceeded { resource: &'static str, limit: u64 },
    NonFiniteAggregate { quantity: &'static str },
    NoAtoms,
}

impl Display for PdbError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read PDB: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read PDB at line {line}: {source}")
            }
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed PDB record at line {line}: {message}")
            }
            Self::LimitExceeded { resource, limit } => {
                write!(formatter, "PDB {resource} exceeds the limit of {limit}")
            }
            Self::NonFiniteAggregate { quantity } => write!(
                formatter,
                "PDB {quantity} exceeds the supported finite numeric range"
            ),
            Self::NoAtoms => formatter.write_str("PDB contains no ATOM or HETATM records"),
        }
    }
}

impl Error for PdbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            Self::MalformedRecord { .. }
            | Self::LimitExceeded { .. }
            | Self::NonFiniteAggregate { .. }
            | Self::NoAtoms => None,
        }
    }
}

impl From<io::Error> for PdbError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResidueKey {
    model_id: String,
    chain_id: String,
    sequence_number: String,
    insertion_code: Option<String>,
    name: String,
    is_hetero: bool,
}

#[derive(Debug)]
struct ResidueBuilder {
    key: ResidueKey,
    atom_count: u64,
    b_factor_sum: f64,
    b_factor_count: u64,
}

#[derive(Debug, Default)]
struct ChainAggregate {
    atom_count: u64,
    residue_count: u64,
    polymer_residue_count: u64,
    hetero_residue_count: u64,
}

#[derive(Debug, Default)]
struct ModelAggregate {
    atom_count: u64,
    chains: BTreeMap<String, ChainAggregate>,
}

#[derive(Debug, Clone, Copy)]
struct PdbLimits {
    max_decompressed_bytes: u64,
    max_atoms: u64,
    max_result_records: u64,
}

impl PdbLimits {
    fn production() -> Self {
        Self {
            max_decompressed_bytes: MAX_PDB_DECOMPRESSED_BYTES,
            max_atoms: MAX_PDB_ATOMS,
            max_result_records: MAX_PDB_RESULT_RECORDS,
        }
    }
}

pub fn pdb_summary_path(
    path: impl AsRef<Path>,
    options: PdbSummaryOptions,
) -> Result<PdbStructureSummary, PdbError> {
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let source = File::open(path)?;
    let source_bytes = source.metadata()?.len();
    let magic_length = (&source).read(&mut magic)?;
    let compressed = magic_length == magic.len() && magic == [0x1f, 0x8b];
    enforce_source_limit(source_bytes, compressed)?;
    let input: Box<dyn Read> = if compressed {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    pdb_summary(BufReader::new(input), options)
}

pub fn pdb_summary(
    reader: impl BufRead,
    options: PdbSummaryOptions,
) -> Result<PdbStructureSummary, PdbError> {
    pdb_summary_with_limits(reader, options, PdbLimits::production())
}

fn pdb_summary_with_limits(
    reader: impl BufRead,
    options: PdbSummaryOptions,
    limits: PdbLimits,
) -> Result<PdbStructureSummary, PdbError> {
    let read_limit = limits.max_decompressed_bytes.saturating_add(1);
    let mut reader = reader.take(read_limit);
    let mut atoms = Vec::new();
    let mut residues = Vec::<ResidueBuilder>::new();
    let mut residue_indices = BTreeMap::<ResidueKey, usize>::new();
    let mut chain_keys = BTreeSet::<(String, String)>::new();
    let mut model_order = Vec::<String>::new();
    let mut seen_model_ids = BTreeSet::<String>::new();
    let mut current_model = None::<String>;
    let mut explicit_models = false;
    let mut inferred_element_count = 0_u64;
    let mut missing_element_count = 0_u64;
    let mut warnings = Vec::new();
    let mut line_number = 0_usize;
    let mut decompressed_bytes = 0_u64;
    let mut result_record_count = 0_u64;
    let mut line = String::new();

    loop {
        line_number += 1;
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|source| PdbError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }
        decompressed_bytes =
            decompressed_bytes
                .checked_add(bytes_read as u64)
                .ok_or(PdbError::LimitExceeded {
                    resource: "decompressed byte count",
                    limit: limits.max_decompressed_bytes,
                })?;
        if decompressed_bytes > limits.max_decompressed_bytes {
            return Err(PdbError::LimitExceeded {
                resource: "decompressed byte count",
                limit: limits.max_decompressed_bytes,
            });
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if !line.is_ascii() {
            return malformed(line_number, "records must use ASCII fixed-width fields");
        }
        let record = field(line, 0, 6).trim();
        match record {
            "END" => break,
            "MODEL" => {
                if current_model.is_some() {
                    return malformed(line_number, "nested MODEL records are not permitted");
                }
                if !explicit_models && !atoms.is_empty() {
                    return malformed(
                        line_number,
                        "MODEL appears after coordinates that were not enclosed by a model",
                    );
                }
                explicit_models = true;
                let model_id = field(line, 10, 14).trim();
                let model_id = if model_id.is_empty() {
                    (model_order.len() + 1).to_string()
                } else {
                    model_id.to_owned()
                };
                if !seen_model_ids.insert(model_id.clone()) {
                    return malformed(
                        line_number,
                        format!("duplicate MODEL identifier {model_id:?}"),
                    );
                }
                reserve_result_records(&mut result_record_count, 1, limits)?;
                model_order.push(model_id.clone());
                current_model = Some(model_id);
            }
            "ENDMDL" => {
                if !explicit_models || current_model.take().is_none() {
                    return malformed(line_number, "ENDMDL does not close an active MODEL");
                }
            }
            "ATOM" | "HETATM" => {
                let model_id = if explicit_models {
                    current_model
                        .clone()
                        .ok_or_else(|| PdbError::MalformedRecord {
                            line: line_number,
                            message: "coordinate appears outside an explicit MODEL".to_owned(),
                        })?
                } else {
                    if model_order.is_empty() {
                        reserve_result_records(&mut result_record_count, 1, limits)?;
                        model_order.push("1".to_owned());
                        seen_model_ids.insert("1".to_owned());
                    }
                    "1".to_owned()
                };
                if atoms.len() as u64 >= limits.max_atoms {
                    return Err(PdbError::LimitExceeded {
                        resource: "atom count",
                        limit: limits.max_atoms,
                    });
                }
                let parsed = parse_atom(line, line_number, &model_id, record, atoms.len())?;
                let key = ResidueKey {
                    model_id: parsed.model_id.clone(),
                    chain_id: parsed.chain_id.clone(),
                    sequence_number: parsed.residue_sequence_number.clone(),
                    insertion_code: parsed.insertion_code.clone(),
                    name: parsed.residue_name.clone(),
                    is_hetero: parsed.record == PdbAtomRecord::Hetatm,
                };
                let is_new_residue = !residue_indices.contains_key(&key);
                let chain_key = (parsed.model_id.clone(), parsed.chain_id.clone());
                let is_new_chain = !chain_keys.contains(&chain_key);
                reserve_result_records(
                    &mut result_record_count,
                    1 + u64::from(is_new_residue) + u64::from(is_new_chain),
                    limits,
                )?;
                if is_new_chain {
                    chain_keys.insert(chain_key);
                }
                let residue_index = match residue_indices.get(&key).copied() {
                    Some(index) => index,
                    None => {
                        let index = residues.len();
                        residue_indices.insert(key.clone(), index);
                        residues.push(ResidueBuilder {
                            key,
                            atom_count: 0,
                            b_factor_sum: 0.0,
                            b_factor_count: 0,
                        });
                        index
                    }
                };
                let residue = &mut residues[residue_index];
                residue.atom_count += 1;
                if let Some(value) = parsed.b_factor {
                    let sum = residue.b_factor_sum + value;
                    if !sum.is_finite() {
                        return malformed(
                            line_number,
                            "residue B-factor sum exceeds the supported finite numeric range",
                        );
                    }
                    residue.b_factor_sum = sum;
                    residue.b_factor_count += 1;
                }

                let mut parsed = parsed;
                parsed.residue_index = residue_index as u64;
                if parsed.element.is_none() {
                    if let Some(element) = infer_element(field(line, 12, 16)) {
                        parsed.element = Some(element);
                        inferred_element_count += 1;
                    } else {
                        missing_element_count += 1;
                    }
                }
                atoms.push(parsed);
            }
            _ => {}
        }
    }

    if atoms.is_empty() {
        return Err(PdbError::NoAtoms);
    }
    if explicit_models && current_model.is_some() {
        warnings.push("the final MODEL has no ENDMDL record".to_owned());
    }
    if inferred_element_count > 0 {
        warnings.push(format!(
            "inferred elements from atom-name alignment for {inferred_element_count} atoms"
        ));
    }
    if missing_element_count > 0 {
        warnings.push(format!(
            "could not determine an element for {missing_element_count} atoms"
        ));
    }

    let bounds = coordinate_bounds(&atoms)?;
    let b_factor_summary = numeric_summary(
        atoms.iter().filter_map(|atom| atom.b_factor),
        "B-factor summary",
    )?;
    let mut output_residues = residues
        .iter()
        .enumerate()
        .map(|(index, residue)| PdbResidue {
            index: index as u64,
            model_id: residue.key.model_id.clone(),
            chain_id: residue.key.chain_id.clone(),
            sequence_number: residue.key.sequence_number.clone(),
            insertion_code: residue.key.insertion_code.clone(),
            name: residue.key.name.clone(),
            is_hetero: residue.key.is_hetero,
            atom_count: residue.atom_count,
            plddt: None,
        })
        .collect::<Vec<_>>();
    let alphafold_confidence = if options.interpret_b_factors_as_plddt {
        let confidence = alphafold_confidence(&atoms, &residues, &mut output_residues)?;
        warnings.push(
            "B-factor values were interpreted as AlphaFold pLDDT because the caller explicitly requested it; PDB content alone does not establish AlphaFold provenance"
                .to_owned(),
        );
        Some(confidence)
    } else {
        None
    };

    let models = summarize_models(&model_order, &atoms, &output_residues);
    let element_counts = atoms.iter().fold(BTreeMap::new(), |mut counts, atom| {
        let element = atom.element.as_deref().unwrap_or("unknown").to_owned();
        *counts.entry(element).or_insert(0) += 1;
        counts
    });
    let polymer_atom_count = atoms
        .iter()
        .filter(|atom| atom.record == PdbAtomRecord::Atom)
        .count() as u64;
    let chain_count = models.iter().map(|model| model.chains.len() as u64).sum();

    Ok(PdbStructureSummary {
        format: "pdb",
        coordinate_units: "angstrom",
        model_count: models.len() as u64,
        chain_count,
        residue_count: output_residues.len() as u64,
        atom_count: atoms.len() as u64,
        polymer_atom_count,
        hetero_atom_count: atoms.len() as u64 - polymer_atom_count,
        element_counts,
        bounds,
        b_factor_summary,
        alphafold_confidence,
        models,
        residues: output_residues,
        atoms,
        warnings,
    })
}

fn enforce_source_limit(source_bytes: u64, compressed: bool) -> Result<(), PdbError> {
    let (resource, limit) = if compressed {
        (
            "compressed source byte count",
            MAX_PDB_COMPRESSED_INPUT_BYTES,
        )
    } else {
        ("plain-text source byte count", MAX_PDB_PLAIN_INPUT_BYTES)
    };
    if source_bytes > limit {
        Err(PdbError::LimitExceeded { resource, limit })
    } else {
        Ok(())
    }
}

fn reserve_result_records(
    count: &mut u64,
    additional: u64,
    limits: PdbLimits,
) -> Result<(), PdbError> {
    let next = count
        .checked_add(additional)
        .ok_or(PdbError::LimitExceeded {
            resource: "result record count",
            limit: limits.max_result_records,
        })?;
    if next > limits.max_result_records {
        return Err(PdbError::LimitExceeded {
            resource: "result record count",
            limit: limits.max_result_records,
        });
    }
    *count = next;
    Ok(())
}

fn parse_atom(
    line: &str,
    line_number: usize,
    model_id: &str,
    record: &str,
    atom_index: usize,
) -> Result<PdbAtom, PdbError> {
    if line.len() < 54 {
        return malformed(
            line_number,
            format!("{record} record is shorter than the coordinate columns"),
        );
    }
    let serial = required_field(line, 6, 11, "atom serial", line_number)?;
    let name = required_field(line, 12, 16, "atom name", line_number)?;
    let residue_name = required_field(line, 17, 20, "residue name", line_number)?;
    let residue_sequence_number =
        required_field(line, 22, 26, "residue sequence number", line_number)?;
    let position = Point3 {
        x: parse_required_number(line, 30, 38, "x coordinate", line_number)?,
        y: parse_required_number(line, 38, 46, "y coordinate", line_number)?,
        z: parse_required_number(line, 46, 54, "z coordinate", line_number)?,
    };
    let element = optional_field(line, 76, 78)
        .map(normalize_element)
        .transpose()
        .map_err(|message| PdbError::MalformedRecord {
            line: line_number,
            message,
        })?;

    Ok(PdbAtom {
        index: atom_index as u64,
        serial,
        record: if record == "ATOM" {
            PdbAtomRecord::Atom
        } else {
            PdbAtomRecord::Hetatm
        },
        model_id: model_id.to_owned(),
        residue_index: 0,
        name,
        alternate_location: optional_field(line, 16, 17),
        residue_name,
        chain_id: field(line, 21, 22).trim().to_owned(),
        residue_sequence_number,
        insertion_code: optional_field(line, 26, 27),
        position,
        occupancy: parse_optional_number(line, 54, 60, "occupancy", line_number)?,
        b_factor: parse_optional_number(line, 60, 66, "B-factor", line_number)?,
        element,
        formal_charge: optional_field(line, 78, 80),
    })
}

fn summarize_models(
    model_order: &[String],
    atoms: &[PdbAtom],
    residues: &[PdbResidue],
) -> Vec<PdbModelSummary> {
    let mut aggregates = model_order
        .iter()
        .map(|model_id| (model_id.clone(), ModelAggregate::default()))
        .collect::<BTreeMap<_, _>>();
    for atom in atoms {
        let model = aggregates.entry(atom.model_id.clone()).or_default();
        model.atom_count += 1;
        model
            .chains
            .entry(atom.chain_id.clone())
            .or_default()
            .atom_count += 1;
    }
    for residue in residues {
        let chain = aggregates
            .entry(residue.model_id.clone())
            .or_default()
            .chains
            .entry(residue.chain_id.clone())
            .or_default();
        chain.residue_count += 1;
        if residue.is_hetero {
            chain.hetero_residue_count += 1;
        } else {
            chain.polymer_residue_count += 1;
        }
    }

    model_order
        .iter()
        .map(|model_id| {
            let aggregate = aggregates
                .get(model_id)
                .expect("registered model aggregate");
            PdbModelSummary {
                model_id: model_id.clone(),
                atom_count: aggregate.atom_count,
                residue_count: aggregate
                    .chains
                    .values()
                    .map(|chain| chain.residue_count)
                    .sum(),
                chains: aggregate
                    .chains
                    .iter()
                    .map(|(chain_id, chain)| PdbChainSummary {
                        chain_id: chain_id.clone(),
                        atom_count: chain.atom_count,
                        residue_count: chain.residue_count,
                        polymer_residue_count: chain.polymer_residue_count,
                        hetero_residue_count: chain.hetero_residue_count,
                    })
                    .collect(),
            }
        })
        .collect()
}

fn alphafold_confidence(
    atoms: &[PdbAtom],
    residues: &[ResidueBuilder],
    output_residues: &mut [PdbResidue],
) -> Result<AlphaFoldConfidenceSummary, PdbError> {
    for atom in atoms
        .iter()
        .filter(|atom| atom.record == PdbAtomRecord::Atom)
    {
        match atom.b_factor {
            Some(value) if (0.0..=100.0).contains(&value) => {}
            Some(value) => {
                return malformed(
                    0,
                    format!(
                        "polymer atom {} has B-factor {value}, outside the pLDDT range 0..100",
                        atom.serial
                    ),
                );
            }
            None => {
                return malformed(
                    0,
                    format!(
                        "polymer atom {} lacks the B-factor required for pLDDT interpretation",
                        atom.serial
                    ),
                );
            }
        }
    }

    let mut values = Vec::new();
    let mut bands = AlphaFoldConfidenceBands::default();
    for (index, residue) in residues.iter().enumerate() {
        if residue.key.is_hetero {
            continue;
        }
        if residue.b_factor_count != residue.atom_count {
            return malformed(
                0,
                format!(
                    "residue {} {} lacks complete B-factor values",
                    residue.key.name, residue.key.sequence_number
                ),
            );
        }
        let value = residue.b_factor_sum / residue.b_factor_count as f64;
        output_residues[index].plddt = Some(value);
        values.push(value);
        if value >= 90.0 {
            bands.very_high_count += 1;
        } else if value >= 70.0 {
            bands.confident_count += 1;
        } else if value >= 50.0 {
            bands.low_count += 1;
        } else {
            bands.very_low_count += 1;
        }
    }
    let summary =
        numeric_summary(values.into_iter(), "AlphaFold pLDDT summary")?.ok_or_else(|| {
            PdbError::MalformedRecord {
                line: 0,
                message: "AlphaFold pLDDT interpretation requires at least one ATOM residue"
                    .to_owned(),
            }
        })?;
    Ok(AlphaFoldConfidenceSummary {
        source: "pdb-b-factor-explicit",
        residue_count: summary.count,
        min_plddt: summary.min,
        max_plddt: summary.max,
        mean_plddt: summary.mean,
        bands,
    })
}

fn coordinate_bounds(atoms: &[PdbAtom]) -> Result<CoordinateBounds, PdbError> {
    let first = atoms[0].position;
    let (min, max) = atoms
        .iter()
        .skip(1)
        .fold((first, first), |(min, max), atom| {
            let point = atom.position;
            (
                Point3 {
                    x: min.x.min(point.x),
                    y: min.y.min(point.y),
                    z: min.z.min(point.z),
                },
                Point3 {
                    x: max.x.max(point.x),
                    y: max.y.max(point.y),
                    z: max.z.max(point.z),
                },
            )
        });
    let span = Point3 {
        x: finite_difference(max.x, min.x, "x-coordinate span")?,
        y: finite_difference(max.y, min.y, "y-coordinate span")?,
        z: finite_difference(max.z, min.z, "z-coordinate span")?,
    };
    Ok(CoordinateBounds {
        min,
        max,
        center: Point3 {
            x: finite_sum(min.x, span.x / 2.0, "x-coordinate center")?,
            y: finite_sum(min.y, span.y / 2.0, "y-coordinate center")?,
            z: finite_sum(min.z, span.z / 2.0, "z-coordinate center")?,
        },
        span,
    })
}

fn numeric_summary(
    values: impl Iterator<Item = f64>,
    quantity: &'static str,
) -> Result<Option<NumericSummary>, PdbError> {
    let mut count = 0_u64;
    let mut sum = 0.0;
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in values {
        count = count
            .checked_add(1)
            .ok_or(PdbError::NonFiniteAggregate { quantity })?;
        sum = finite_sum(sum, value, quantity)?;
        min = min.min(value);
        max = max.max(value);
    }
    if count == 0 {
        return Ok(None);
    }
    let mean = sum / count as f64;
    if !mean.is_finite() {
        return Err(PdbError::NonFiniteAggregate { quantity });
    }
    Ok(Some(NumericSummary {
        count,
        min,
        max,
        mean,
    }))
}

fn finite_sum(left: f64, right: f64, quantity: &'static str) -> Result<f64, PdbError> {
    let result = left + right;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(PdbError::NonFiniteAggregate { quantity })
    }
}

fn finite_difference(left: f64, right: f64, quantity: &'static str) -> Result<f64, PdbError> {
    let result = left - right;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(PdbError::NonFiniteAggregate { quantity })
    }
}

fn parse_required_number(
    line: &str,
    start: usize,
    end: usize,
    name: &str,
    line_number: usize,
) -> Result<f64, PdbError> {
    let value = required_field(line, start, end, name, line_number)?;
    parse_finite_number(&value).map_err(|message| PdbError::MalformedRecord {
        line: line_number,
        message: format!("invalid {name} {value:?}: {message}"),
    })
}

fn parse_optional_number(
    line: &str,
    start: usize,
    end: usize,
    name: &str,
    line_number: usize,
) -> Result<Option<f64>, PdbError> {
    optional_field(line, start, end)
        .map(|value| {
            parse_finite_number(&value).map_err(|message| PdbError::MalformedRecord {
                line: line_number,
                message: format!("invalid {name} {value:?}: {message}"),
            })
        })
        .transpose()
}

fn parse_finite_number(value: &str) -> Result<f64, &'static str> {
    let parsed = value.parse::<f64>().map_err(|_| "not a number")?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err("must be finite")
    }
}

fn required_field(
    line: &str,
    start: usize,
    end: usize,
    name: &str,
    line_number: usize,
) -> Result<String, PdbError> {
    optional_field(line, start, end).ok_or_else(|| PdbError::MalformedRecord {
        line: line_number,
        message: format!("{name} is empty"),
    })
}

fn optional_field(line: &str, start: usize, end: usize) -> Option<String> {
    let value = field(line, start, end).trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn field(line: &str, start: usize, end: usize) -> &str {
    if start >= line.len() {
        ""
    } else {
        &line[start..end.min(line.len())]
    }
}

fn normalize_element(value: String) -> Result<String, String> {
    if value.len() > 2 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(format!("invalid element field {value:?}"));
    }
    let mut bytes = value.bytes();
    let first = bytes
        .next()
        .expect("non-empty element")
        .to_ascii_uppercase();
    let mut element = String::from(char::from(first));
    if let Some(second) = bytes.next() {
        element.push(char::from(second.to_ascii_lowercase()));
    }
    Ok(element)
}

fn infer_element(raw_atom_name: &str) -> Option<String> {
    let bytes = raw_atom_name.as_bytes();
    let first_index = bytes.iter().position(u8::is_ascii_alphabetic)?;
    let first = bytes[first_index].to_ascii_uppercase();
    if first_index == 0 && bytes.get(1).is_some_and(u8::is_ascii_alphabetic) {
        let candidate = [first, bytes[1].to_ascii_uppercase()];
        const TWO_LETTER_ELEMENTS: [[u8; 2]; 15] = [
            *b"BR", *b"CA", *b"CD", *b"CL", *b"CO", *b"CU", *b"FE", *b"HG", *b"LI", *b"MG", *b"MN",
            *b"NA", *b"NI", *b"PB", *b"ZN",
        ];
        if TWO_LETTER_ELEMENTS.contains(&candidate) {
            return Some(format!(
                "{}{}",
                char::from(candidate[0]),
                char::from(candidate[1].to_ascii_lowercase())
            ));
        }
    }
    Some(String::from(char::from(first)))
}

fn malformed<T>(line: usize, message: impl Into<String>) -> Result<T, PdbError> {
    Err(PdbError::MalformedRecord {
        line,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PDB_COMPRESSED_INPUT_BYTES, MAX_PDB_PLAIN_INPUT_BYTES, PdbError, PdbLimits,
        PdbSummaryOptions, enforce_source_limit, pdb_summary, pdb_summary_with_limits,
    };
    use flate2::Compression;
    use flate2::read::MultiGzDecoder;
    use flate2::write::GzEncoder;
    use std::io::{BufReader, Cursor, Write};

    const ALPHAFOLD_PDB: &str = concat!(
        "HEADER    ALPHAFOLD TEST\n",
        "ATOM      1  N   GLY A   1      11.104  13.207   9.657  1.00 95.00           N  \n",
        "ATOM      2  CA  GLY A   1      12.204  13.707   9.157  1.00 95.00           C  \n",
        "ATOM      3  N   ALA A   2      13.104  14.207   8.657  1.00 45.00           N  \n",
        "HETATM    4  O   HOH B 101      14.104  15.207   7.657  1.00 20.00           O  \n",
        "END\n"
    );

    fn atom_line(serial: &str, residue: &str, x: &str, b_factor: &str) -> String {
        let mut line =
            b"ATOM      1  N   GLY A   1      11.104  13.207   9.657  1.00 95.00           N  \n"
                .to_vec();
        replace_field(&mut line, 6, 11, serial);
        replace_field(&mut line, 22, 26, residue);
        replace_field(&mut line, 30, 38, x);
        replace_field(&mut line, 60, 66, b_factor);
        String::from_utf8(line).expect("ASCII fixture")
    }

    fn replace_field(line: &mut [u8], start: usize, end: usize, value: &str) {
        assert!(value.len() <= end - start);
        line[start..end].fill(b' ');
        line[end - value.len()..end].copy_from_slice(value.as_bytes());
    }

    fn test_limits(max_bytes: u64, max_atoms: u64, max_records: u64) -> PdbLimits {
        PdbLimits {
            max_decompressed_bytes: max_bytes,
            max_atoms,
            max_result_records: max_records,
        }
    }

    #[test]
    fn produces_render_ready_coordinates_and_counts() {
        let summary = pdb_summary(Cursor::new(ALPHAFOLD_PDB), PdbSummaryOptions::default())
            .expect("valid PDB");

        assert_eq!(summary.model_count, 1);
        assert_eq!(summary.chain_count, 2);
        assert_eq!(summary.residue_count, 3);
        assert_eq!(summary.atom_count, 4);
        assert_eq!(summary.polymer_atom_count, 3);
        assert_eq!(summary.hetero_atom_count, 1);
        assert_eq!(summary.element_counts["C"], 1);
        assert_eq!(summary.atoms[1].residue_index, 0);
        assert_eq!(summary.atoms[1].position.x, 12.204);
        assert_eq!(summary.bounds.min.z, 7.657);
        assert_eq!(summary.bounds.max.x, 14.104);
        assert!(summary.alphafold_confidence.is_none());
    }

    #[test]
    fn stops_at_end_record_before_trailing_coordinates() {
        let input = format!(
            "{}END\n{}",
            atom_line("1", "1", "1.0", "10.0"),
            atom_line("2", "2", "2.0", "20.0")
        );
        let summary = pdb_summary(Cursor::new(input), PdbSummaryOptions::default())
            .expect("PDB terminated by END");

        assert_eq!(summary.atom_count, 1);
        assert_eq!(summary.residue_count, 1);
        assert_eq!(summary.atoms[0].serial, "1");
    }

    #[test]
    fn interprets_b_factors_as_plddt_only_when_explicit() {
        let summary = pdb_summary(
            Cursor::new(ALPHAFOLD_PDB),
            PdbSummaryOptions {
                interpret_b_factors_as_plddt: true,
            },
        )
        .expect("valid AlphaFold-style PDB");
        let confidence = summary.alphafold_confidence.expect("confidence summary");

        assert_eq!(confidence.residue_count, 2);
        assert_eq!(confidence.mean_plddt, 70.0);
        assert_eq!(confidence.bands.very_high_count, 1);
        assert_eq!(confidence.bands.very_low_count, 1);
        assert_eq!(summary.residues[0].plddt, Some(95.0));
        assert_eq!(summary.residues[1].plddt, Some(45.0));
        assert_eq!(summary.residues[2].plddt, None);
    }

    #[test]
    fn rejects_non_finite_coordinates() {
        let input =
            "ATOM      1  N   GLY A   1         NaN  13.207   9.657  1.00 95.00           N  \n";
        let error = pdb_summary(Cursor::new(input), PdbSummaryOptions::default())
            .expect_err("NaN must fail");

        assert!(matches!(error, PdbError::MalformedRecord { line: 1, .. }));
    }

    #[test]
    fn rejects_plain_and_compressed_sources_over_their_distinct_limits() {
        assert!(matches!(
            enforce_source_limit(MAX_PDB_PLAIN_INPUT_BYTES + 1, false),
            Err(PdbError::LimitExceeded {
                resource: "plain-text source byte count",
                ..
            })
        ));
        assert!(matches!(
            enforce_source_limit(MAX_PDB_COMPRESSED_INPUT_BYTES + 1, true),
            Err(PdbError::LimitExceeded {
                resource: "compressed source byte count",
                ..
            })
        ));
    }

    #[test]
    fn rejects_gzip_expansion_over_decompressed_limit() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(ALPHAFOLD_PDB.repeat(2).as_bytes())
            .expect("compress fixture");
        let compressed = encoder.finish().expect("finish gzip fixture");
        let decoder = MultiGzDecoder::new(Cursor::new(compressed));

        let error = pdb_summary_with_limits(
            BufReader::new(decoder),
            PdbSummaryOptions::default(),
            test_limits(100, 100, 300),
        )
        .expect_err("decompressed limit");
        assert!(matches!(
            error,
            PdbError::LimitExceeded {
                resource: "decompressed byte count",
                limit: 100
            }
        ));
    }

    #[test]
    fn rejects_atom_and_result_record_limits() {
        let two_atoms = format!(
            "{}{}",
            atom_line("1", "1", "1.0", "10.0"),
            atom_line("2", "2", "2.0", "20.0")
        );
        let atom_error = pdb_summary_with_limits(
            Cursor::new(&two_atoms),
            PdbSummaryOptions::default(),
            test_limits(10_000, 1, 100),
        )
        .expect_err("atom limit");
        assert!(matches!(
            atom_error,
            PdbError::LimitExceeded {
                resource: "atom count",
                limit: 1
            }
        ));

        let result_error = pdb_summary_with_limits(
            Cursor::new(atom_line("1", "1", "1.0", "10.0")),
            PdbSummaryOptions::default(),
            test_limits(10_000, 100, 3),
        )
        .expect_err("result record limit");
        assert!(matches!(
            result_error,
            PdbError::LimitExceeded {
                resource: "result record count",
                limit: 3
            }
        ));
    }

    #[test]
    fn rejects_non_finite_coordinate_aggregates() {
        let input = format!(
            "{}{}",
            atom_line("1", "1", "-9e307", "10.0"),
            atom_line("2", "2", "9e307", "20.0")
        );
        let error = pdb_summary(Cursor::new(input), PdbSummaryOptions::default())
            .expect_err("coordinate span overflow");

        assert!(matches!(
            error,
            PdbError::NonFiniteAggregate {
                quantity: "x-coordinate span"
            }
        ));
    }

    #[test]
    fn computes_large_finite_coordinate_center_without_intermediate_overflow() {
        let summary = pdb_summary(
            Cursor::new(atom_line("1", "1", "9e307", "10.0")),
            PdbSummaryOptions::default(),
        )
        .expect("finite center");

        assert_eq!(summary.bounds.center.x, 9e307);
        assert!(summary.bounds.center.x.is_finite());
    }

    #[test]
    fn rejects_non_finite_residue_and_summary_b_factor_aggregates() {
        let same_residue = format!(
            "{}{}",
            atom_line("1", "1", "1.0", "9e307"),
            atom_line("2", "1", "2.0", "9e307")
        );
        let residue_error = pdb_summary(Cursor::new(same_residue), PdbSummaryOptions::default())
            .expect_err("residue B-factor overflow");
        assert!(matches!(
            residue_error,
            PdbError::MalformedRecord { line: 2, .. }
        ));

        let different_residues = format!(
            "{}{}",
            atom_line("1", "1", "1.0", "9e307"),
            atom_line("2", "2", "2.0", "9e307")
        );
        let summary_error = pdb_summary(
            Cursor::new(different_residues),
            PdbSummaryOptions::default(),
        )
        .expect_err("summary B-factor overflow");
        assert!(matches!(
            summary_error,
            PdbError::NonFiniteAggregate {
                quantity: "B-factor summary"
            }
        ));
    }

    #[test]
    fn rejects_missing_atoms() {
        let error = pdb_summary(
            Cursor::new("HEADER    EMPTY\nEND\n"),
            PdbSummaryOptions::default(),
        )
        .expect_err("empty structure must fail");

        assert!(matches!(error, PdbError::NoAtoms));
    }
}
