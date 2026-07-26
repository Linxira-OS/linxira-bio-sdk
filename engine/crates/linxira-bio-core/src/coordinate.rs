use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub const MAX_COORDINATE_COMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_COORDINATE_TEXT_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_COORDINATE_ATOMS: usize = 100_000;
pub const MAX_CONTACTS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordinateFormat {
    Pdb,
    Mmcif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordinateRecord {
    Atom,
    Hetatm,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoordinateAtom {
    pub index: u64,
    pub serial: Option<String>,
    pub record: CoordinateRecord,
    pub model_id: String,
    pub name: String,
    pub residue_name: String,
    pub chain_id: String,
    pub residue_sequence: String,
    pub insertion_code: Option<String>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub occupancy: Option<f64>,
    pub b_factor: Option<f64>,
    pub element: Option<String>,
}

impl CoordinateAtom {
    pub fn residue_id(&self) -> String {
        match &self.insertion_code {
            Some(insertion) => format!("{}{insertion}", self.residue_sequence),
            None => self.residue_sequence.clone(),
        }
    }

    fn point(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoordinateStructure {
    pub format: CoordinateFormat,
    pub atoms: Vec<CoordinateAtom>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoordinatePoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoordinateBounds {
    pub minimum: CoordinatePoint,
    pub maximum: CoordinatePoint,
    pub span: CoordinatePoint,
    pub center: CoordinatePoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoordinateChainSummary {
    pub chain_id: String,
    pub atom_count: u64,
    pub residue_count: u64,
    pub polymer_residue_count: u64,
    pub hetero_residue_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoordinateModelSummary {
    pub model_id: String,
    pub atom_count: u64,
    pub residue_count: u64,
    pub chains: Vec<CoordinateChainSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MmcifStructureSummary {
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
    pub models: Vec<CoordinateModelSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolymerType {
    Protein,
    NucleicAcid,
    MixedOrUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtractedChainSequence {
    pub chain_id: String,
    pub polymer_type: PolymerType,
    pub sequence: String,
    pub residue_count: u64,
    pub unknown_residue_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructureSequenceResult {
    pub format: CoordinateFormat,
    pub model_id: String,
    pub chain_count: u64,
    pub total_residues: u64,
    pub chains: Vec<ExtractedChainSequence>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactMapOptions {
    pub cutoff_angstrom: f64,
    pub atom_name: String,
    pub include_inter_chain: bool,
}

impl Default for ContactMapOptions {
    fn default() -> Self {
        Self {
            cutoff_angstrom: 8.0,
            atom_name: "CA".to_owned(),
            include_inter_chain: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ResidueReference {
    pub chain_id: String,
    pub residue_id: String,
    pub residue_name: String,
    pub ordinal: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResidueContact {
    pub left: ResidueReference,
    pub right: ResidueReference,
    pub distance_angstrom: f64,
    pub inter_chain: bool,
    pub sequence_separation: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureContactMapResult {
    pub format: CoordinateFormat,
    pub model_id: String,
    pub atom_name: String,
    pub cutoff_angstrom: f64,
    pub representative_residue_count: u64,
    pub contact_count: u64,
    pub contacts: Vec<ResidueContact>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomSelector {
    pub model_id: Option<String>,
    pub chain_id: String,
    pub residue_id: String,
    pub atom_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SelectedAtom {
    pub model_id: String,
    pub chain_id: String,
    pub residue_id: String,
    pub residue_name: String,
    pub atom_name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureGeometryResult {
    pub format: CoordinateFormat,
    pub measurement: &'static str,
    pub units: &'static str,
    pub value: f64,
    pub atoms: Vec<SelectedAtom>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuperpositionOptions {
    pub atom_name: String,
}

impl Default for SuperpositionOptions {
    fn default() -> Self {
        Self {
            atom_name: "CA".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureSuperpositionResult {
    pub reference_format: CoordinateFormat,
    pub mobile_format: CoordinateFormat,
    pub reference_model_id: String,
    pub mobile_model_id: String,
    pub atom_name: String,
    pub matched_atom_count: u64,
    pub rmsd_before_angstrom: f64,
    pub rmsd_after_angstrom: f64,
    pub rotation: [[f64; 3]; 3],
    pub translation: [f64; 3],
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum CoordinateError {
    Io(io::Error),
    Invalid(String),
    LimitExceeded { resource: &'static str, limit: u64 },
}

impl Display for CoordinateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read coordinate structure: {error}"),
            Self::Invalid(message) => write!(formatter, "invalid coordinate structure: {message}"),
            Self::LimitExceeded { resource, limit } => {
                write!(
                    formatter,
                    "coordinate {resource} exceeds the limit of {limit}"
                )
            }
        }
    }
}

impl Error for CoordinateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Invalid(_) | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<io::Error> for CoordinateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn load_coordinate_structure_path(
    path: impl AsRef<Path>,
) -> Result<CoordinateStructure, CoordinateError> {
    let path = path.as_ref();
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(CoordinateError::Invalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let compressed = magic_length == magic.len() && magic == [0x1f, 0x8b];
    let source_limit = if compressed {
        MAX_COORDINATE_COMPRESSED_BYTES
    } else {
        MAX_COORDINATE_TEXT_BYTES
    };
    if metadata.len() > source_limit {
        return Err(CoordinateError::LimitExceeded {
            resource: if compressed {
                "compressed source byte count"
            } else {
                "plain-text source byte count"
            },
            limit: source_limit,
        });
    }
    let file = File::open(path)?;
    let input: Box<dyn Read> = if compressed {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    input
        .take(MAX_COORDINATE_TEXT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_COORDINATE_TEXT_BYTES {
        return Err(CoordinateError::LimitExceeded {
            resource: "decompressed byte count",
            limit: MAX_COORDINATE_TEXT_BYTES,
        });
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| CoordinateError::Invalid("coordinate text is not UTF-8".to_owned()))?;
    let lower_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let uncompressed_name = lower_name
        .strip_suffix(".gz")
        .or_else(|| lower_name.strip_suffix(".bgz"))
        .unwrap_or(&lower_name);
    if uncompressed_name.ends_with(".cif")
        || uncompressed_name.ends_with(".mmcif")
        || text.trim_start().starts_with("data_")
    {
        parse_mmcif(&text)
    } else {
        parse_pdb(&text)
    }
}

pub fn mmcif_summary_path(
    path: impl AsRef<Path>,
) -> Result<MmcifStructureSummary, CoordinateError> {
    let structure = load_coordinate_structure_path(path)?;
    if structure.format != CoordinateFormat::Mmcif {
        return Err(CoordinateError::Invalid(
            "mmCIF summary requires mmCIF coordinate content".to_owned(),
        ));
    }
    summarize_mmcif(structure)
}

pub fn extract_structure_sequences_path(
    path: impl AsRef<Path>,
) -> Result<StructureSequenceResult, CoordinateError> {
    let structure = load_coordinate_structure_path(path)?;
    extract_structure_sequences(&structure)
}

pub fn structure_contact_map_path(
    path: impl AsRef<Path>,
    options: ContactMapOptions,
) -> Result<StructureContactMapResult, CoordinateError> {
    let structure = load_coordinate_structure_path(path)?;
    structure_contact_map(&structure, options)
}

pub fn parse_atom_selector(value: &str) -> Result<AtomSelector, CoordinateError> {
    let fields = value.split('/').collect::<Vec<_>>();
    let (model_id, chain_id, residue_id, atom_name) = match fields.as_slice() {
        [chain, residue, atom] => (None, *chain, *residue, *atom),
        [model, chain, residue, atom] => (Some((*model).to_owned()), *chain, *residue, *atom),
        _ => {
            return Err(CoordinateError::Invalid(format!(
                "atom selector {value:?} must be CHAIN/RESIDUE/ATOM or MODEL/CHAIN/RESIDUE/ATOM"
            )));
        }
    };
    if residue_id.is_empty() || atom_name.is_empty() {
        return Err(CoordinateError::Invalid(format!(
            "atom selector {value:?} has an empty residue or atom name"
        )));
    }
    Ok(AtomSelector {
        model_id,
        chain_id: chain_id.to_owned(),
        residue_id: residue_id.to_owned(),
        atom_name: atom_name.to_ascii_uppercase(),
    })
}

pub fn measure_structure_geometry_path(
    path: impl AsRef<Path>,
    selectors: &[AtomSelector],
) -> Result<StructureGeometryResult, CoordinateError> {
    let structure = load_coordinate_structure_path(path)?;
    measure_structure_geometry(&structure, selectors)
}

pub fn superpose_structures_path(
    reference: impl AsRef<Path>,
    mobile: impl AsRef<Path>,
    options: SuperpositionOptions,
) -> Result<StructureSuperpositionResult, CoordinateError> {
    let reference = load_coordinate_structure_path(reference)?;
    let mobile = load_coordinate_structure_path(mobile)?;
    superpose_structures(&reference, &mobile, options)
}

fn parse_pdb(text: &str) -> Result<CoordinateStructure, CoordinateError> {
    let mut atoms = Vec::new();
    let mut model_id = "1".to_owned();
    let mut explicit_models = false;
    let mut model_open = false;
    let mut skipped_alternates = 0_u64;
    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        if !line.is_ascii() {
            return invalid(format!(
                "PDB line {line_number} is not ASCII fixed-width text"
            ));
        }
        let record = fixed_field(line, 0, 6);
        match record {
            "MODEL" => {
                if model_open {
                    return invalid(format!("nested MODEL record at PDB line {line_number}"));
                }
                explicit_models = true;
                model_open = true;
                let value = fixed_field(line, 10, 14);
                model_id = if value.is_empty() {
                    (atoms
                        .iter()
                        .map(|atom: &CoordinateAtom| atom.model_id.as_str())
                        .collect::<BTreeSet<_>>()
                        .len()
                        + 1)
                    .to_string()
                } else {
                    value.to_owned()
                };
                continue;
            }
            "ENDMDL" => {
                if explicit_models && !model_open {
                    return invalid(format!("unmatched ENDMDL at PDB line {line_number}"));
                }
                model_open = false;
                continue;
            }
            "END" => break,
            _ => {}
        }
        let atom_record = match record {
            "ATOM" => CoordinateRecord::Atom,
            "HETATM" => CoordinateRecord::Hetatm,
            _ => continue,
        };
        if explicit_models && !model_open {
            return invalid(format!(
                "coordinate outside MODEL block at PDB line {line_number}"
            ));
        }
        let alternate = fixed_field(line, 16, 17);
        if !matches!(alternate, "" | "A" | "1" | "." | "?") {
            skipped_alternates += 1;
            continue;
        }
        if line.len() < 54 {
            return invalid(format!(
                "PDB coordinate at line {line_number} is shorter than column 54"
            ));
        }
        let name = required_pdb_field(line, 12, 16, "atom name", line_number)?;
        let element_text = fixed_field(line, 76, 78);
        atoms.push(CoordinateAtom {
            index: atoms.len() as u64,
            serial: optional_string(fixed_field(line, 6, 11)),
            record: atom_record,
            model_id: model_id.clone(),
            name: name.clone(),
            residue_name: required_pdb_field(line, 17, 20, "residue name", line_number)?,
            chain_id: fixed_field(line, 21, 22).to_owned(),
            residue_sequence: required_pdb_field(line, 22, 26, "residue sequence", line_number)?,
            insertion_code: optional_string(fixed_field(line, 26, 27)),
            x: parse_finite(fixed_field(line, 30, 38), "PDB x coordinate")?,
            y: parse_finite(fixed_field(line, 38, 46), "PDB y coordinate")?,
            z: parse_finite(fixed_field(line, 46, 54), "PDB z coordinate")?,
            occupancy: parse_optional_finite(fixed_field(line, 54, 60), "PDB occupancy")?,
            b_factor: parse_optional_finite(fixed_field(line, 60, 66), "PDB B-factor")?,
            element: if element_text.is_empty() {
                infer_element(&name)
            } else {
                Some(normalize_element(element_text)?)
            },
        });
        enforce_atom_limit(atoms.len())?;
    }
    finish_structure(CoordinateFormat::Pdb, atoms, skipped_alternates)
}

fn parse_mmcif(text: &str) -> Result<CoordinateStructure, CoordinateError> {
    let mut tokens = CifTokenCursor::new(text);
    while let Some(token) = tokens.next_token()? {
        if token != "loop_" {
            continue;
        }
        let mut headers = Vec::new();
        let first_value = loop {
            match tokens.next_token()? {
                Some(header) if header.starts_with('_') => headers.push(header),
                value => break value,
            }
        };
        if headers
            .iter()
            .any(|header| header.starts_with("_atom_site."))
        {
            return parse_atom_site_loop(&mut tokens, &headers, first_value);
        }
        skip_loop_values(&mut tokens, first_value)?;
    }
    invalid("mmCIF does not contain an _atom_site coordinate loop")
}

fn parse_atom_site_loop<'a>(
    tokens: &mut CifTokenCursor<'a>,
    headers: &[&str],
    mut first_value: Option<&'a str>,
) -> Result<CoordinateStructure, CoordinateError> {
    let x = required_column(headers, "_atom_site.Cartn_x")?;
    let y = required_column(headers, "_atom_site.Cartn_y")?;
    let z = required_column(headers, "_atom_site.Cartn_z")?;
    let group = column(headers, &["_atom_site.group_PDB"]);
    let serial = column(headers, &["_atom_site.id"]);
    let name = column(
        headers,
        &["_atom_site.auth_atom_id", "_atom_site.label_atom_id"],
    );
    let residue = column(
        headers,
        &["_atom_site.auth_comp_id", "_atom_site.label_comp_id"],
    );
    let chain = column(
        headers,
        &["_atom_site.auth_asym_id", "_atom_site.label_asym_id"],
    );
    let residue_sequence = column(
        headers,
        &["_atom_site.auth_seq_id", "_atom_site.label_seq_id"],
    );
    let insertion = column(headers, &["_atom_site.pdbx_PDB_ins_code"]);
    let element = column(headers, &["_atom_site.type_symbol"]);
    let occupancy = column(headers, &["_atom_site.occupancy"]);
    let b_factor = column(headers, &["_atom_site.B_iso_or_equiv"]);
    let alternate = column(
        headers,
        &["_atom_site.label_alt_id", "_atom_site.auth_alt_id"],
    );
    let model = column(headers, &["_atom_site.pdbx_PDB_model_num"]);
    let width = headers.len();
    let mut atoms = Vec::new();
    let mut skipped_alternates = 0_u64;
    let mut row_number = 0_u64;
    loop {
        let next = match first_value.take() {
            Some(value) => Some(value),
            None => tokens.next_token()?,
        };
        let Some(first) = next else {
            break;
        };
        if is_loop_boundary(first) {
            tokens.put_back(first)?;
            break;
        }
        row_number += 1;
        let mut row = Vec::with_capacity(width);
        row.push(first);
        for _ in 1..width {
            let Some(value) = tokens.next_token()? else {
                return invalid(format!(
                    "incomplete mmCIF _atom_site row {row_number}: found {} of {width} values",
                    row.len()
                ));
            };
            row.push(value);
        }
        if value_at(&row, alternate)
            .is_some_and(|value| !is_missing(value) && !matches!(value, "A" | "1"))
        {
            skipped_alternates += 1;
            continue;
        }
        let atom_name = value_at(&row, name)
            .filter(|value| !is_missing(value))
            .unwrap_or("?")
            .to_owned();
        let serial_value = value_at(&row, serial).filter(|value| !is_missing(value));
        let residue_sequence_value = value_at(&row, residue_sequence)
            .filter(|value| !is_missing(value))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                serial_value
                    .map(|value| format!("atom-{value}"))
                    .unwrap_or_else(|| format!("row-{row_number}"))
            });
        let element_value = value_at(&row, element).filter(|value| !is_missing(value));
        atoms.push(CoordinateAtom {
            index: atoms.len() as u64,
            serial: serial_value.map(str::to_owned),
            record: if value_at(&row, group).is_some_and(|value| value == "HETATM") {
                CoordinateRecord::Hetatm
            } else {
                CoordinateRecord::Atom
            },
            model_id: value_at(&row, model)
                .filter(|value| !is_missing(value))
                .unwrap_or("1")
                .to_owned(),
            name: atom_name.clone(),
            residue_name: value_at(&row, residue)
                .filter(|value| !is_missing(value))
                .unwrap_or("UNK")
                .to_owned(),
            chain_id: value_at(&row, chain)
                .filter(|value| !is_missing(value))
                .unwrap_or("")
                .to_owned(),
            residue_sequence: residue_sequence_value,
            insertion_code: value_at(&row, insertion)
                .filter(|value| !is_missing(value))
                .map(str::to_owned),
            x: parse_finite(row[x], "mmCIF x coordinate")?,
            y: parse_finite(row[y], "mmCIF y coordinate")?,
            z: parse_finite(row[z], "mmCIF z coordinate")?,
            occupancy: value_at(&row, occupancy)
                .filter(|value| !is_missing(value))
                .map(|value| parse_finite(value, "mmCIF occupancy"))
                .transpose()?,
            b_factor: value_at(&row, b_factor)
                .filter(|value| !is_missing(value))
                .map(|value| parse_finite(value, "mmCIF B-factor"))
                .transpose()?,
            element: match element_value {
                Some(value) => Some(normalize_element(value)?),
                None => infer_element(&atom_name),
            },
        });
        enforce_atom_limit(atoms.len())?;
    }
    finish_structure(CoordinateFormat::Mmcif, atoms, skipped_alternates)
}

fn finish_structure(
    format: CoordinateFormat,
    atoms: Vec<CoordinateAtom>,
    skipped_alternates: u64,
) -> Result<CoordinateStructure, CoordinateError> {
    if atoms.is_empty() {
        return invalid("no ATOM or HETATM coordinates were found");
    }
    let mut warnings = Vec::new();
    if skipped_alternates > 0 {
        warnings.push(format!(
            "ignored {skipped_alternates} alternate-location atoms other than blank, A, or 1"
        ));
    }
    Ok(CoordinateStructure {
        format,
        atoms,
        warnings,
    })
}

fn summarize_mmcif(
    structure: CoordinateStructure,
) -> Result<MmcifStructureSummary, CoordinateError> {
    let bounds = coordinate_bounds(&structure.atoms)?;
    let models = summarize_models(&structure.atoms);
    let residue_count = models.iter().map(|model| model.residue_count).sum();
    let chain_count = models.iter().map(|model| model.chains.len() as u64).sum();
    let polymer_atom_count = structure
        .atoms
        .iter()
        .filter(|atom| atom.record == CoordinateRecord::Atom)
        .count() as u64;
    let element_counts = structure
        .atoms
        .iter()
        .fold(BTreeMap::new(), |mut counts, atom| {
            *counts
                .entry(atom.element.as_deref().unwrap_or("unknown").to_owned())
                .or_insert(0) += 1;
            counts
        });
    Ok(MmcifStructureSummary {
        format: "mmcif",
        coordinate_units: "angstrom",
        model_count: models.len() as u64,
        chain_count,
        residue_count,
        atom_count: structure.atoms.len() as u64,
        polymer_atom_count,
        hetero_atom_count: structure.atoms.len() as u64 - polymer_atom_count,
        element_counts,
        bounds,
        models,
        warnings: structure.warnings,
    })
}

fn summarize_models(atoms: &[CoordinateAtom]) -> Vec<CoordinateModelSummary> {
    let mut model_atoms = BTreeMap::<String, Vec<&CoordinateAtom>>::new();
    for atom in atoms {
        model_atoms
            .entry(atom.model_id.clone())
            .or_default()
            .push(atom);
    }
    model_atoms
        .into_iter()
        .map(|(model_id, atoms)| {
            let mut chain_atoms = BTreeMap::<String, Vec<&CoordinateAtom>>::new();
            for atom in &atoms {
                chain_atoms
                    .entry(atom.chain_id.clone())
                    .or_default()
                    .push(*atom);
            }
            let chains = chain_atoms
                .into_iter()
                .map(|(chain_id, atoms)| {
                    let residues = atoms
                        .iter()
                        .map(|atom| {
                            (
                                atom.residue_sequence.as_str(),
                                atom.insertion_code.as_deref(),
                                atom.residue_name.as_str(),
                                atom.record,
                            )
                        })
                        .collect::<BTreeSet<_>>();
                    let polymer_residue_count = residues
                        .iter()
                        .filter(|residue| residue.3 == CoordinateRecord::Atom)
                        .count() as u64;
                    CoordinateChainSummary {
                        chain_id,
                        atom_count: atoms.len() as u64,
                        residue_count: residues.len() as u64,
                        polymer_residue_count,
                        hetero_residue_count: residues.len() as u64 - polymer_residue_count,
                    }
                })
                .collect::<Vec<_>>();
            CoordinateModelSummary {
                model_id,
                atom_count: atoms.len() as u64,
                residue_count: chains.iter().map(|chain| chain.residue_count).sum(),
                chains,
            }
        })
        .collect()
}

fn extract_structure_sequences(
    structure: &CoordinateStructure,
) -> Result<StructureSequenceResult, CoordinateError> {
    let model_id = first_model_id(structure)?;
    let mut chains = BTreeMap::<String, Vec<&CoordinateAtom>>::new();
    for atom in structure
        .atoms
        .iter()
        .filter(|atom| atom.model_id == model_id && atom.record == CoordinateRecord::Atom)
    {
        chains.entry(atom.chain_id.clone()).or_default().push(atom);
    }
    let mut output = Vec::new();
    let mut warnings = structure.warnings.clone();
    for (chain_id, atoms) in chains {
        let mut seen = BTreeSet::<(String, Option<String>, String)>::new();
        let mut sequence = String::new();
        let mut amino_count = 0_u64;
        let mut nucleotide_count = 0_u64;
        let mut unknown_count = 0_u64;
        for atom in atoms {
            let key = (
                atom.residue_sequence.clone(),
                atom.insertion_code.clone(),
                atom.residue_name.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            let (symbol, kind) = residue_symbol(&atom.residue_name);
            sequence.push(symbol);
            match kind {
                ResidueKind::Amino => amino_count += 1,
                ResidueKind::Nucleotide => nucleotide_count += 1,
                ResidueKind::Unknown => unknown_count += 1,
            }
        }
        if sequence.is_empty() {
            continue;
        }
        if unknown_count > 0 {
            warnings.push(format!(
                "chain {chain_id:?} contains {unknown_count} residues without a standard one-letter mapping"
            ));
        }
        let polymer_type = if amino_count > 0 && nucleotide_count == 0 {
            PolymerType::Protein
        } else if nucleotide_count > 0 && amino_count == 0 {
            PolymerType::NucleicAcid
        } else {
            PolymerType::MixedOrUnknown
        };
        output.push(ExtractedChainSequence {
            chain_id,
            polymer_type,
            residue_count: sequence.len() as u64,
            sequence,
            unknown_residue_count: unknown_count,
        });
    }
    if output.is_empty() {
        return invalid("the first model has no polymer ATOM residues");
    }
    Ok(StructureSequenceResult {
        format: structure.format,
        model_id,
        chain_count: output.len() as u64,
        total_residues: output.iter().map(|chain| chain.residue_count).sum(),
        chains: output,
        warnings,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResidueKind {
    Amino,
    Nucleotide,
    Unknown,
}

fn residue_symbol(name: &str) -> (char, ResidueKind) {
    match name.trim().to_ascii_uppercase().as_str() {
        "ALA" => ('A', ResidueKind::Amino),
        "ARG" => ('R', ResidueKind::Amino),
        "ASN" => ('N', ResidueKind::Amino),
        "ASP" => ('D', ResidueKind::Amino),
        "CYS" => ('C', ResidueKind::Amino),
        "GLN" => ('Q', ResidueKind::Amino),
        "GLU" => ('E', ResidueKind::Amino),
        "GLY" => ('G', ResidueKind::Amino),
        "HIS" => ('H', ResidueKind::Amino),
        "ILE" => ('I', ResidueKind::Amino),
        "LEU" => ('L', ResidueKind::Amino),
        "LYS" => ('K', ResidueKind::Amino),
        "MET" => ('M', ResidueKind::Amino),
        "PHE" => ('F', ResidueKind::Amino),
        "PRO" => ('P', ResidueKind::Amino),
        "SER" => ('S', ResidueKind::Amino),
        "THR" => ('T', ResidueKind::Amino),
        "TRP" => ('W', ResidueKind::Amino),
        "TYR" => ('Y', ResidueKind::Amino),
        "VAL" => ('V', ResidueKind::Amino),
        "SEC" => ('U', ResidueKind::Amino),
        "PYL" => ('O', ResidueKind::Amino),
        "ASX" => ('B', ResidueKind::Amino),
        "GLX" => ('Z', ResidueKind::Amino),
        "A" | "DA" | "ADE" => ('A', ResidueKind::Nucleotide),
        "C" | "DC" | "CYT" => ('C', ResidueKind::Nucleotide),
        "G" | "DG" | "GUA" => ('G', ResidueKind::Nucleotide),
        "U" | "DU" | "URI" => ('U', ResidueKind::Nucleotide),
        "T" | "DT" | "THY" => ('T', ResidueKind::Nucleotide),
        _ => ('X', ResidueKind::Unknown),
    }
}

fn structure_contact_map(
    structure: &CoordinateStructure,
    options: ContactMapOptions,
) -> Result<StructureContactMapResult, CoordinateError> {
    if !options.cutoff_angstrom.is_finite() || options.cutoff_angstrom <= 0.0 {
        return invalid("contact cutoff must be a positive finite number");
    }
    if options.cutoff_angstrom > 100.0 {
        return invalid("contact cutoff must not exceed 100 angstrom");
    }
    let atom_name = options.atom_name.trim().to_ascii_uppercase();
    if atom_name.is_empty() {
        return invalid("contact representative atom name must not be empty");
    }
    let model_id = first_model_id(structure)?;
    let mut chain_ordinals = BTreeMap::<String, u64>::new();
    let mut representatives = Vec::<(&CoordinateAtom, ResidueReference)>::new();
    let mut seen = BTreeSet::<(String, String)>::new();
    for atom in structure.atoms.iter().filter(|atom| {
        atom.model_id == model_id
            && atom.record == CoordinateRecord::Atom
            && atom.name.eq_ignore_ascii_case(&atom_name)
    }) {
        let residue_id = atom.residue_id();
        if !seen.insert((atom.chain_id.clone(), residue_id.clone())) {
            continue;
        }
        let ordinal = chain_ordinals.entry(atom.chain_id.clone()).or_insert(0);
        let reference = ResidueReference {
            chain_id: atom.chain_id.clone(),
            residue_id,
            residue_name: atom.residue_name.clone(),
            ordinal: *ordinal,
        };
        *ordinal += 1;
        representatives.push((atom, reference));
    }
    if representatives.len() < 2 {
        return invalid(format!(
            "first model contains fewer than two polymer {atom_name} atoms"
        ));
    }
    let cutoff_squared = options.cutoff_angstrom * options.cutoff_angstrom;
    let cell_size = options.cutoff_angstrom;
    let mut cells = HashMap::<(i64, i64, i64), Vec<usize>>::new();
    let mut contacts = Vec::new();
    for (index, (atom, reference)) in representatives.iter().enumerate() {
        let cell = spatial_cell(atom.point(), cell_size);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(candidates) = cells.get(&(cell.0 + dx, cell.1 + dy, cell.2 + dz)) {
                        for &other_index in candidates {
                            let (other_atom, other_reference) = &representatives[other_index];
                            let inter_chain = reference.chain_id != other_reference.chain_id;
                            if inter_chain && !options.include_inter_chain {
                                continue;
                            }
                            let distance_squared =
                                squared_distance(atom.point(), other_atom.point());
                            if distance_squared > cutoff_squared {
                                continue;
                            }
                            if contacts.len() >= MAX_CONTACTS {
                                return Err(CoordinateError::LimitExceeded {
                                    resource: "contact result count",
                                    limit: MAX_CONTACTS as u64,
                                });
                            }
                            contacts.push(ResidueContact {
                                left: other_reference.clone(),
                                right: reference.clone(),
                                distance_angstrom: distance_squared.sqrt(),
                                inter_chain,
                                sequence_separation: (!inter_chain)
                                    .then(|| reference.ordinal.abs_diff(other_reference.ordinal)),
                            });
                        }
                    }
                }
            }
        }
        cells.entry(cell).or_default().push(index);
    }
    contacts.sort_by(|left, right| {
        left.left
            .cmp(&right.left)
            .then_with(|| left.right.cmp(&right.right))
    });
    Ok(StructureContactMapResult {
        format: structure.format,
        model_id,
        atom_name,
        cutoff_angstrom: options.cutoff_angstrom,
        representative_residue_count: representatives.len() as u64,
        contact_count: contacts.len() as u64,
        contacts,
        warnings: structure.warnings.clone(),
    })
}

fn measure_structure_geometry(
    structure: &CoordinateStructure,
    selectors: &[AtomSelector],
) -> Result<StructureGeometryResult, CoordinateError> {
    if !(2..=4).contains(&selectors.len()) {
        return invalid("geometry measurement requires two, three, or four atom selectors");
    }
    let default_model = first_model_id(structure)?;
    let mut selected = Vec::new();
    let mut points = Vec::new();
    for selector in selectors {
        let model = selector.model_id.as_deref().unwrap_or(&default_model);
        let matches = structure
            .atoms
            .iter()
            .filter(|atom| {
                atom.model_id == model
                    && atom.chain_id == selector.chain_id
                    && atom.residue_id() == selector.residue_id
                    && atom.name.eq_ignore_ascii_case(&selector.atom_name)
            })
            .collect::<Vec<_>>();
        let atom = match matches.as_slice() {
            [atom] => *atom,
            [] => {
                return invalid(format!(
                    "atom selector {}/{}/{}/{} did not match",
                    model, selector.chain_id, selector.residue_id, selector.atom_name
                ));
            }
            _ => {
                return invalid(format!(
                    "atom selector {}/{}/{}/{} matched more than one atom",
                    model, selector.chain_id, selector.residue_id, selector.atom_name
                ));
            }
        };
        points.push(atom.point());
        selected.push(SelectedAtom {
            model_id: atom.model_id.clone(),
            chain_id: atom.chain_id.clone(),
            residue_id: atom.residue_id(),
            residue_name: atom.residue_name.clone(),
            atom_name: atom.name.clone(),
            x: atom.x,
            y: atom.y,
            z: atom.z,
        });
    }
    let (measurement, units, value) = match points.as_slice() {
        [left, right] => (
            "distance",
            "angstrom",
            squared_distance(*left, *right).sqrt(),
        ),
        [left, center, right] => ("angle", "degree", angle_degrees(*left, *center, *right)?),
        [first, second, third, fourth] => (
            "torsion",
            "degree",
            torsion_degrees(*first, *second, *third, *fourth)?,
        ),
        _ => unreachable!("selector count validated"),
    };
    Ok(StructureGeometryResult {
        format: structure.format,
        measurement,
        units,
        value,
        atoms: selected,
    })
}

fn superpose_structures(
    reference: &CoordinateStructure,
    mobile: &CoordinateStructure,
    options: SuperpositionOptions,
) -> Result<StructureSuperpositionResult, CoordinateError> {
    let atom_name = options.atom_name.trim().to_ascii_uppercase();
    if atom_name.is_empty() {
        return invalid("superposition atom name must not be empty");
    }
    let reference_model_id = first_model_id(reference)?;
    let mobile_model_id = first_model_id(mobile)?;
    let reference_atoms = matched_atom_map(reference, &reference_model_id, &atom_name);
    let mobile_atoms = matched_atom_map(mobile, &mobile_model_id, &atom_name);
    let keys = reference_atoms
        .keys()
        .filter(|key| mobile_atoms.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    if keys.len() < 3 {
        return invalid(format!(
            "superposition requires at least three matched {atom_name} atoms; found {}",
            keys.len()
        ));
    }
    let reference_points = keys
        .iter()
        .map(|key| reference_atoms[key].point())
        .collect::<Vec<_>>();
    let mobile_points = keys
        .iter()
        .map(|key| mobile_atoms[key].point())
        .collect::<Vec<_>>();
    let reference_center = centroid(&reference_points);
    let mobile_center = centroid(&mobile_points);
    let rmsd_before = rmsd(&reference_points, &mobile_points);
    let rotation = optimal_rotation(
        &reference_points,
        &mobile_points,
        reference_center,
        mobile_center,
    )?;
    let rotated_mobile_center = matrix_vector(rotation, mobile_center);
    let translation = [
        reference_center[0] - rotated_mobile_center[0],
        reference_center[1] - rotated_mobile_center[1],
        reference_center[2] - rotated_mobile_center[2],
    ];
    let transformed = mobile_points
        .iter()
        .map(|point| add(matrix_vector(rotation, *point), translation))
        .collect::<Vec<_>>();
    let rmsd_after = rmsd(&reference_points, &transformed);
    if !rmsd_after.is_finite() {
        return invalid("superposition produced a non-finite RMSD");
    }
    let mut warnings = reference.warnings.clone();
    warnings.extend(mobile.warnings.clone());
    warnings.push(
        "atoms were matched by chain identifier, residue identifier, and atom name; no sequence alignment was performed"
            .to_owned(),
    );
    Ok(StructureSuperpositionResult {
        reference_format: reference.format,
        mobile_format: mobile.format,
        reference_model_id,
        mobile_model_id,
        atom_name,
        matched_atom_count: keys.len() as u64,
        rmsd_before_angstrom: rmsd_before,
        rmsd_after_angstrom: rmsd_after,
        rotation,
        translation,
        warnings,
    })
}

type AtomMatchKey = (String, String);

fn matched_atom_map<'a>(
    structure: &'a CoordinateStructure,
    model_id: &str,
    atom_name: &str,
) -> BTreeMap<AtomMatchKey, &'a CoordinateAtom> {
    let mut atoms = BTreeMap::new();
    for atom in structure.atoms.iter().filter(|atom| {
        atom.model_id == model_id
            && atom.record == CoordinateRecord::Atom
            && atom.name.eq_ignore_ascii_case(atom_name)
    }) {
        atoms
            .entry((atom.chain_id.clone(), atom.residue_id()))
            .or_insert(atom);
    }
    atoms
}

fn optimal_rotation(
    reference: &[[f64; 3]],
    mobile: &[[f64; 3]],
    reference_center: [f64; 3],
    mobile_center: [f64; 3],
) -> Result<[[f64; 3]; 3], CoordinateError> {
    let mut covariance = [[0.0_f64; 3]; 3];
    for (reference, mobile) in reference.iter().zip(mobile) {
        let left = subtract(*mobile, mobile_center);
        let right = subtract(*reference, reference_center);
        for row in 0..3 {
            for column in 0..3 {
                covariance[row][column] += left[row] * right[column];
            }
        }
    }
    let s = covariance;
    let trace = s[0][0] + s[1][1] + s[2][2];
    let mut horn = [
        [
            trace,
            s[1][2] - s[2][1],
            s[2][0] - s[0][2],
            s[0][1] - s[1][0],
        ],
        [
            s[1][2] - s[2][1],
            s[0][0] - s[1][1] - s[2][2],
            s[0][1] + s[1][0],
            s[0][2] + s[2][0],
        ],
        [
            s[2][0] - s[0][2],
            s[0][1] + s[1][0],
            -s[0][0] + s[1][1] - s[2][2],
            s[1][2] + s[2][1],
        ],
        [
            s[0][1] - s[1][0],
            s[0][2] + s[2][0],
            s[1][2] + s[2][1],
            -s[0][0] - s[1][1] + s[2][2],
        ],
    ];
    let quaternion = largest_symmetric_eigenvector(&mut horn)?;
    Ok(quaternion_rotation(quaternion))
}

fn largest_symmetric_eigenvector(matrix: &mut [[f64; 4]; 4]) -> Result<[f64; 4], CoordinateError> {
    let mut vectors = [[0.0_f64; 4]; 4];
    for (index, row) in vectors.iter_mut().enumerate() {
        row[index] = 1.0;
    }
    for _ in 0..64 {
        let mut pivot = (0_usize, 1_usize);
        let mut magnitude = 0.0_f64;
        for (row, values) in matrix.iter().enumerate() {
            for (column, value) in values.iter().enumerate().skip(row + 1) {
                if value.abs() > magnitude {
                    magnitude = value.abs();
                    pivot = (row, column);
                }
            }
        }
        if magnitude < 1e-14 {
            break;
        }
        let (p, q) = pivot;
        let angle = 0.5 * (2.0 * matrix[p][q]).atan2(matrix[q][q] - matrix[p][p]);
        let cosine = angle.cos();
        let sine = angle.sin();
        for index in [0_usize, 1, 2, 3] {
            if index != p && index != q {
                let left = matrix[index][p];
                let right = matrix[index][q];
                matrix[index][p] = cosine * left - sine * right;
                matrix[p][index] = matrix[index][p];
                matrix[index][q] = sine * left + cosine * right;
                matrix[q][index] = matrix[index][q];
            }
        }
        let pp = matrix[p][p];
        let qq = matrix[q][q];
        let pq = matrix[p][q];
        matrix[p][p] = cosine * cosine * pp - 2.0 * sine * cosine * pq + sine * sine * qq;
        matrix[q][q] = sine * sine * pp + 2.0 * sine * cosine * pq + cosine * cosine * qq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;
        for row in &mut vectors {
            let left = row[p];
            let right = row[q];
            row[p] = cosine * left - sine * right;
            row[q] = sine * left + cosine * right;
        }
    }
    let largest = (0..4)
        .max_by(|left, right| matrix[*left][*left].total_cmp(&matrix[*right][*right]))
        .expect("four eigenvalues");
    let mut quaternion = [
        vectors[0][largest],
        vectors[1][largest],
        vectors[2][largest],
        vectors[3][largest],
    ];
    let norm = quaternion
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= 1e-12 {
        return invalid("matched coordinates do not define a stable rotation");
    }
    for value in &mut quaternion {
        *value /= norm;
    }
    Ok(quaternion)
}

fn quaternion_rotation(quaternion: [f64; 4]) -> [[f64; 3]; 3] {
    let [w, x, y, z] = quaternion;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

fn first_model_id(structure: &CoordinateStructure) -> Result<String, CoordinateError> {
    structure
        .atoms
        .first()
        .map(|atom| atom.model_id.clone())
        .ok_or_else(|| CoordinateError::Invalid("coordinate structure has no atoms".to_owned()))
}

fn coordinate_bounds(atoms: &[CoordinateAtom]) -> Result<CoordinateBounds, CoordinateError> {
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for atom in atoms {
        let point = atom.point();
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    let span = subtract(maximum, minimum);
    let center = [
        minimum[0] + span[0] * 0.5,
        minimum[1] + span[1] * 0.5,
        minimum[2] + span[2] * 0.5,
    ];
    if minimum
        .iter()
        .chain(maximum.iter())
        .chain(span.iter())
        .chain(center.iter())
        .any(|value| !value.is_finite())
    {
        return invalid("coordinate bounds exceed the finite numeric range");
    }
    Ok(CoordinateBounds {
        minimum: point_struct(minimum),
        maximum: point_struct(maximum),
        span: point_struct(span),
        center: point_struct(center),
    })
}

fn point_struct(point: [f64; 3]) -> CoordinatePoint {
    CoordinatePoint {
        x: point[0],
        y: point[1],
        z: point[2],
    }
}

fn angle_degrees(
    left: [f64; 3],
    center: [f64; 3],
    right: [f64; 3],
) -> Result<f64, CoordinateError> {
    let first = subtract(left, center);
    let second = subtract(right, center);
    let denominator = norm(first) * norm(second);
    if denominator <= 1e-12 {
        return invalid("angle contains coincident atoms");
    }
    Ok((dot(first, second) / denominator)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees())
}

fn torsion_degrees(
    first: [f64; 3],
    second: [f64; 3],
    third: [f64; 3],
    fourth: [f64; 3],
) -> Result<f64, CoordinateError> {
    let b0 = subtract(first, second);
    let b1 = subtract(third, second);
    let b2 = subtract(fourth, third);
    let b1_norm = norm(b1);
    if b1_norm <= 1e-12 {
        return invalid("torsion contains coincident central atoms");
    }
    let axis = scale(b1, 1.0 / b1_norm);
    let v = subtract(b0, scale(axis, dot(b0, axis)));
    let w = subtract(b2, scale(axis, dot(b2, axis)));
    if norm(v) <= 1e-12 || norm(w) <= 1e-12 {
        return invalid("torsion contains collinear atoms");
    }
    Ok(dot(cross(axis, v), w).atan2(dot(v, w)).to_degrees())
}

fn centroid(points: &[[f64; 3]]) -> [f64; 3] {
    let sum = points.iter().copied().fold([0.0; 3], add);
    scale(sum, 1.0 / points.len() as f64)
}

fn rmsd(reference: &[[f64; 3]], mobile: &[[f64; 3]]) -> f64 {
    (reference
        .iter()
        .zip(mobile)
        .map(|(left, right)| squared_distance(*left, *right))
        .sum::<f64>()
        / reference.len() as f64)
        .sqrt()
}

fn matrix_vector(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    [
        dot(matrix[0], vector),
        dot(matrix[1], vector),
        dot(matrix[2], vector),
    ]
}

fn spatial_cell(point: [f64; 3], size: f64) -> (i64, i64, i64) {
    (
        (point[0] / size).floor() as i64,
        (point[1] / size).floor() as i64,
        (point[2] / size).floor() as i64,
    )
}

fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = subtract(left, right);
    dot(difference, difference)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(vector: [f64; 3], factor: f64) -> [f64; 3] {
    [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

fn fixed_field(line: &str, start: usize, end: usize) -> &str {
    line.as_bytes()
        .get(start..end.min(line.len()))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or("")
        .trim()
}

fn required_pdb_field(
    line: &str,
    start: usize,
    end: usize,
    name: &str,
    line_number: usize,
) -> Result<String, CoordinateError> {
    let value = fixed_field(line, start, end);
    if value.is_empty() {
        invalid(format!("PDB {name} is empty at line {line_number}"))
    } else {
        Ok(value.to_owned())
    }
}

fn optional_string(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_finite(value: &str, context: &str) -> Result<f64, CoordinateError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| CoordinateError::Invalid(format!("invalid {context}: {value:?}")))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        invalid(format!("non-finite {context}: {value:?}"))
    }
}

fn parse_optional_finite(value: &str, context: &str) -> Result<Option<f64>, CoordinateError> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_finite(value, context).map(Some)
    }
}

fn normalize_element(value: &str) -> Result<String, CoordinateError> {
    let element = value
        .trim()
        .chars()
        .take(2)
        .flat_map(char::to_uppercase)
        .collect::<String>();
    if element.is_empty()
        || !element
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        invalid(format!("invalid element symbol {value:?}"))
    } else {
        Ok(element)
    }
}

fn infer_element(atom_name: &str) -> Option<String> {
    let name = atom_name.trim_start_matches(|character: char| character.is_ascii_digit());
    let first = name.chars().next()?.to_ascii_uppercase();
    first.is_ascii_alphabetic().then(|| first.to_string())
}

fn enforce_atom_limit(count: usize) -> Result<(), CoordinateError> {
    if count > MAX_COORDINATE_ATOMS {
        Err(CoordinateError::LimitExceeded {
            resource: "atom count",
            limit: MAX_COORDINATE_ATOMS as u64,
        })
    } else {
        Ok(())
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, CoordinateError> {
    Err(CoordinateError::Invalid(message.into()))
}

fn is_missing(value: &str) -> bool {
    matches!(value, "." | "?")
}

fn required_column(headers: &[&str], name: &str) -> Result<usize, CoordinateError> {
    headers
        .iter()
        .position(|header| *header == name)
        .ok_or_else(|| CoordinateError::Invalid(format!("mmCIF atom loop is missing {name}")))
}

fn column(headers: &[&str], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|name| headers.iter().position(|header| *header == *name))
}

fn value_at<'a>(row: &[&'a str], index: Option<usize>) -> Option<&'a str> {
    index.and_then(|index| row.get(index)).copied()
}

struct CifTokenCursor<'a> {
    text: &'a str,
    position: usize,
    pending: Option<&'a str>,
}

impl<'a> CifTokenCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            position: 0,
            pending: None,
        }
    }

    fn next_token(&mut self) -> Result<Option<&'a str>, CoordinateError> {
        if let Some(token) = self.pending.take() {
            return Ok(Some(token));
        }
        let bytes = self.text.as_bytes();
        loop {
            while self.position < bytes.len() && bytes[self.position].is_ascii_whitespace() {
                self.position += 1;
            }
            if self.position >= bytes.len() {
                return Ok(None);
            }
            if bytes[self.position] != b'#' {
                break;
            }
            while self.position < bytes.len() && bytes[self.position] != b'\n' {
                self.position += 1;
            }
        }
        if bytes[self.position] == b';' && (self.position == 0 || bytes[self.position - 1] == b'\n')
        {
            let start = self.position + 1;
            let mut delimiter = start;
            while delimiter < bytes.len()
                && !(bytes[delimiter] == b';' && bytes[delimiter - 1] == b'\n')
            {
                delimiter += 1;
            }
            if delimiter >= bytes.len() {
                return invalid("unterminated semicolon-delimited mmCIF value");
            }
            self.position = delimiter + 1;
            while self.position < bytes.len() && bytes[self.position] != b'\n' {
                self.position += 1;
            }
            return Ok(Some(
                self.text[start..delimiter].trim_end_matches(['\r', '\n']),
            ));
        }
        if matches!(bytes[self.position], b'\'' | b'"') {
            let quote = bytes[self.position];
            self.position += 1;
            let start = self.position;
            while self.position < bytes.len() && bytes[self.position] != quote {
                self.position += 1;
            }
            if self.position >= bytes.len() {
                return invalid("unterminated quoted mmCIF value");
            }
            let token = &self.text[start..self.position];
            self.position += 1;
            return Ok(Some(token));
        }
        let start = self.position;
        while self.position < bytes.len()
            && !bytes[self.position].is_ascii_whitespace()
            && bytes[self.position] != b'#'
        {
            self.position += 1;
        }
        Ok(Some(&self.text[start..self.position]))
    }

    fn put_back(&mut self, token: &'a str) -> Result<(), CoordinateError> {
        if self.pending.replace(token).is_some() {
            invalid("internal mmCIF tokenizer pushback overflow")
        } else {
            Ok(())
        }
    }
}

fn skip_loop_values<'a>(
    tokens: &mut CifTokenCursor<'a>,
    mut value: Option<&'a str>,
) -> Result<(), CoordinateError> {
    loop {
        let next = match value.take() {
            Some(value) => Some(value),
            None => tokens.next_token()?,
        };
        let Some(token) = next else {
            return Ok(());
        };
        if is_loop_boundary(token) {
            tokens.put_back(token)?;
            return Ok(());
        }
    }
}

fn is_loop_boundary(token: &str) -> bool {
    token == "loop_"
        || token == "stop_"
        || token.starts_with('_')
        || token.starts_with("data_")
        || token.starts_with("save_")
}

#[cfg(test)]
mod tests {
    use super::{
        AtomSelector, ContactMapOptions, SuperpositionOptions, extract_structure_sequences,
        measure_structure_geometry, parse_mmcif, parse_pdb, structure_contact_map,
        superpose_structures,
    };

    fn pdb(points: &[[f64; 3]]) -> String {
        points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                format!(
                    "ATOM  {:>5}  CA  ALA A{:>4}    {:>8.3}{:>8.3}{:>8.3}  1.00 20.00           C\n",
                    index + 1,
                    index + 1,
                    point[0],
                    point[1],
                    point[2]
                )
            })
            .collect()
    }

    #[test]
    fn parses_mmcif_and_extracts_protein_sequence() {
        let cif = r#"data_demo
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 1 C CA ALA A 1 0 0 0
ATOM 2 C CA GLY A 2 4 0 0
#
"#;
        let structure = parse_mmcif(cif).expect("valid mmCIF");
        let result = extract_structure_sequences(&structure).expect("sequence extraction");
        assert_eq!(result.chains[0].sequence, "AG");
    }

    #[test]
    fn contact_map_uses_exact_distance_cutoff() {
        let structure = parse_pdb(&pdb(&[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [20.0, 0.0, 0.0]]))
            .expect("valid PDB");
        let result = structure_contact_map(
            &structure,
            ContactMapOptions {
                cutoff_angstrom: 5.0,
                ..ContactMapOptions::default()
            },
        )
        .expect("contact map");
        assert_eq!(result.contact_count, 1);
        assert!((result.contacts[0].distance_angstrom - 4.0).abs() < 1e-9);
    }

    #[test]
    fn measures_distance_angle_and_torsion() {
        let structure = parse_pdb(&pdb(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ]))
        .expect("valid PDB");
        let selector = |residue: &str| AtomSelector {
            model_id: None,
            chain_id: "A".to_owned(),
            residue_id: residue.to_owned(),
            atom_name: "CA".to_owned(),
        };
        let angle =
            measure_structure_geometry(&structure, &[selector("1"), selector("2"), selector("3")])
                .expect("angle");
        assert!((angle.value - 90.0).abs() < 1e-9);
        let torsion = measure_structure_geometry(
            &structure,
            &[selector("1"), selector("2"), selector("3"), selector("4")],
        )
        .expect("torsion");
        assert!((torsion.value.abs() - 90.0).abs() < 1e-9);
    }

    #[test]
    fn superposes_identifier_matched_atoms() {
        let reference = parse_pdb(&pdb(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ]))
        .expect("reference");
        let mobile = parse_pdb(&pdb(&[
            [5.0, -2.0, 3.0],
            [5.0, -1.0, 3.0],
            [4.0, -2.0, 3.0],
            [5.0, -2.0, 4.0],
        ]))
        .expect("mobile");
        let result = superpose_structures(&reference, &mobile, SuperpositionOptions::default())
            .expect("superposition");
        assert!(result.rmsd_before_angstrom > 1.0);
        assert!(result.rmsd_after_angstrom < 1e-9);
    }
}
