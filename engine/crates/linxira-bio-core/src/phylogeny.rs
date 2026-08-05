use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const MAX_NEWICK_DECOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_NEWICK_NODES: usize = 1_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeTransformOptions {
    pub reroot_label: Option<String>,
    pub label_map: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreeTransformResult {
    pub leaf_count: u64,
    pub internal_node_count: u64,
    pub max_depth: u64,
    pub total_branch_length: Option<f64>,
    pub rerooted: bool,
    pub relabeled_count: u64,
    pub output: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DistanceMatrixOptions {
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DistanceMatrixResult {
    pub sequence_count: u64,
    pub alignment_length: u64,
    pub compared_position_count: u64,
    pub model: String,
    pub distances: Vec<DistanceMatrixEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DistanceMatrixEntry {
    pub seq_a: String,
    pub seq_b: String,
    pub distance: f64,
}

#[derive(Debug)]
pub enum PhylogenyError {
    Io(io::Error),
    InvalidUtf8,
    MalformedNewick { offset: usize, message: String },
    InvalidOption(String),
    OutputAlreadyExists(PathBuf),
    LimitExceeded { resource: &'static str, limit: u64 },
    AlignmentEmpty,
    AlignmentInconsistent(String),
    InvalidModel(String),
    EmptySequence(String),
}

impl Display for PhylogenyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "phylogeny I/O failed: {error}"),
            Self::InvalidUtf8 => formatter.write_str("Newick input is not valid UTF-8"),
            Self::MalformedNewick { offset, message } => {
                write!(formatter, "malformed Newick near byte {offset}: {message}")
            }
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::OutputAlreadyExists(path) => {
                write!(
                    formatter,
                    "refusing to overwrite existing output: {}",
                    path.display()
                )
            }
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "Newick processing exceeds the deterministic {resource} limit of {limit}"
            ),
            Self::AlignmentEmpty => formatter.write_str("alignment contains no sequences"),
            Self::AlignmentInconsistent(msg) => formatter.write_str(msg),
            Self::InvalidModel(model) => write!(
                formatter,
                "unknown distance model: {model}. Supported: p-distance, jc69, k80"
            ),
            Self::EmptySequence(id) => {
                write!(formatter, "sequence {id} has zero length")
            }
        }
    }
}

impl Error for PhylogenyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidUtf8
            | Self::MalformedNewick { .. }
            | Self::InvalidOption(_)
            | Self::OutputAlreadyExists(_)
            | Self::LimitExceeded { .. }
            | Self::AlignmentEmpty
            | Self::AlignmentInconsistent(_)
            | Self::InvalidModel(_)
            | Self::EmptySequence(_) => None,
        }
    }
}

impl From<io::Error> for PhylogenyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
struct TreeNode {
    label: Option<String>,
    length: Option<f64>,
    children: Vec<TreeNode>,
}

pub fn transform_newick_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: TreeTransformOptions,
) -> Result<TreeTransformResult, PhylogenyError> {
    let text = read_bounded_text(input.as_ref())?;
    let mut parser = NewickParser::new(&text);
    let mut tree = parser.parse()?;
    let rerooted = if let Some(label) = options
        .reroot_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        tree = reroot_tree(&tree, label)?;
        true
    } else {
        false
    };
    let relabeled_count = apply_label_map(&mut tree, &options.label_map)?;
    let summary = summarize_tree(&tree)?;
    let mut normalized = String::new();
    write_node(&tree, &mut normalized);
    normalized.push(';');
    let output = output.as_ref();
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                PhylogenyError::OutputAlreadyExists(output.to_path_buf())
            } else {
                PhylogenyError::Io(error)
            }
        })?;
    writer.write_all(normalized.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    let mut warnings = Vec::new();
    if summary.total_branch_length.is_none() {
        warnings.push("tree contains no branch lengths".to_owned());
    }
    Ok(TreeTransformResult {
        leaf_count: summary.leaf_count,
        internal_node_count: summary.internal_node_count,
        max_depth: summary.max_depth,
        total_branch_length: summary.total_branch_length,
        rerooted,
        relabeled_count,
        output: output.to_string_lossy().into_owned(),
        warnings,
    })
}

pub fn read_tree_label_map_path(
    path: impl AsRef<Path>,
) -> Result<BTreeMap<String, String>, PhylogenyError> {
    let text = read_bounded_text(path.as_ref())?;
    let mut mapping = BTreeMap::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 2 || fields.iter().any(|field| field.trim().is_empty()) {
            return Err(PhylogenyError::InvalidOption(format!(
                "tree label map line {} must contain two non-empty tab-separated fields",
                line_index + 1
            )));
        }
        if mapping
            .insert(fields[0].trim().to_owned(), fields[1].trim().to_owned())
            .is_some()
        {
            return Err(PhylogenyError::InvalidOption(format!(
                "tree label map repeats source label {:?}",
                fields[0].trim()
            )));
        }
    }
    Ok(mapping)
}

struct TreeSummary {
    leaf_count: u64,
    internal_node_count: u64,
    max_depth: u64,
    total_branch_length: Option<f64>,
}

fn summarize_tree(tree: &TreeNode) -> Result<TreeSummary, PhylogenyError> {
    fn visit(
        node: &TreeNode,
        depth: u64,
        summary: &mut TreeSummary,
        saw_length: &mut bool,
    ) -> Result<(), PhylogenyError> {
        summary.max_depth = summary.max_depth.max(depth);
        if node.children.is_empty() {
            summary.leaf_count =
                summary
                    .leaf_count
                    .checked_add(1)
                    .ok_or(PhylogenyError::LimitExceeded {
                        resource: "leaf count",
                        limit: u64::MAX,
                    })?;
        } else {
            summary.internal_node_count = summary.internal_node_count.checked_add(1).ok_or(
                PhylogenyError::LimitExceeded {
                    resource: "internal-node count",
                    limit: u64::MAX,
                },
            )?;
        }
        if let Some(length) = node.length {
            *saw_length = true;
            let total = summary.total_branch_length.unwrap_or(0.0) + length;
            if !total.is_finite() {
                return Err(PhylogenyError::LimitExceeded {
                    resource: "branch-length sum",
                    limit: u64::MAX,
                });
            }
            summary.total_branch_length = Some(total);
        }
        for child in &node.children {
            visit(child, depth + 1, summary, saw_length)?;
        }
        Ok(())
    }
    let mut summary = TreeSummary {
        leaf_count: 0,
        internal_node_count: 0,
        max_depth: 0,
        total_branch_length: None,
    };
    let mut saw_length = false;
    visit(tree, 0, &mut summary, &mut saw_length)?;
    if !saw_length {
        summary.total_branch_length = None;
    }
    Ok(summary)
}

fn apply_label_map(
    tree: &mut TreeNode,
    mapping: &BTreeMap<String, String>,
) -> Result<u64, PhylogenyError> {
    for (source, target) in mapping {
        if source.trim().is_empty() || target.trim().is_empty() {
            return Err(PhylogenyError::InvalidOption(
                "tree label mappings require non-empty source and target labels".to_owned(),
            ));
        }
    }
    fn visit(node: &mut TreeNode, mapping: &BTreeMap<String, String>, count: &mut u64) {
        if let Some(label) = node.label.as_mut()
            && let Some(replacement) = mapping.get(label)
        {
            *label = replacement.clone();
            *count += 1;
        }
        for child in &mut node.children {
            visit(child, mapping, count);
        }
    }
    let mut count = 0_u64;
    visit(tree, mapping, &mut count);
    let mut leaf_labels = BTreeSet::new();
    fn check_unique(node: &TreeNode, labels: &mut BTreeSet<String>) -> Result<(), PhylogenyError> {
        if node.children.is_empty()
            && let Some(label) = &node.label
            && !labels.insert(label.clone())
        {
            return Err(PhylogenyError::InvalidOption(format!(
                "label mapping creates duplicate leaf label {label:?}"
            )));
        }
        for child in &node.children {
            check_unique(child, labels)?;
        }
        Ok(())
    }
    check_unique(tree, &mut leaf_labels)?;
    Ok(count)
}

fn reroot_tree(tree: &TreeNode, label: &str) -> Result<TreeNode, PhylogenyError> {
    #[derive(Clone)]
    struct ArenaNode {
        label: Option<String>,
        edges: Vec<(usize, Option<f64>)>,
    }
    fn flatten(node: &TreeNode, arena: &mut Vec<ArenaNode>) -> usize {
        let index = arena.len();
        arena.push(ArenaNode {
            label: node.label.clone(),
            edges: Vec::new(),
        });
        for child in &node.children {
            let child_index = flatten(child, arena);
            arena[index].edges.push((child_index, child.length));
            arena[child_index].edges.push((index, child.length));
        }
        index
    }
    fn orient(
        index: usize,
        parent: Option<usize>,
        length: Option<f64>,
        arena: &[ArenaNode],
    ) -> TreeNode {
        let children = arena[index]
            .edges
            .iter()
            .filter(|(neighbor, _)| Some(*neighbor) != parent)
            .map(|(neighbor, edge_length)| orient(*neighbor, Some(index), *edge_length, arena))
            .collect();
        TreeNode {
            label: arena[index].label.clone(),
            length,
            children,
        }
    }
    let mut arena = Vec::new();
    let root = flatten(tree, &mut arena);
    let matches = arena
        .iter()
        .enumerate()
        .filter(|(_, node)| node.label.as_deref() == Some(label))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(PhylogenyError::InvalidOption(format!(
            "reroot label {label:?} matched {} nodes; exactly one is required",
            matches.len()
        )));
    }
    let outgroup = matches[0];
    if outgroup == root {
        return Err(PhylogenyError::InvalidOption(
            "reroot label identifies the existing root".to_owned(),
        ));
    }
    let (neighbor, edge_length) =
        arena[outgroup].edges.first().copied().ok_or_else(|| {
            PhylogenyError::InvalidOption("cannot reroot a one-node tree".to_owned())
        })?;
    if arena[outgroup].edges.len() != 1 {
        return Err(PhylogenyError::InvalidOption(
            "reroot label must identify a leaf node".to_owned(),
        ));
    }
    let split = edge_length.map(|value| value / 2.0);
    Ok(TreeNode {
        label: None,
        length: None,
        children: vec![
            orient(outgroup, Some(neighbor), split, &arena),
            orient(neighbor, Some(outgroup), split, &arena),
        ],
    })
}

fn write_node(node: &TreeNode, output: &mut String) {
    if !node.children.is_empty() {
        output.push('(');
        for (index, child) in node.children.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write_node(child, output);
        }
        output.push(')');
    }
    if let Some(label) = &node.label {
        write_label(label, output);
    }
    if let Some(length) = node.length {
        output.push(':');
        output.push_str(
            format!("{length:.12}")
                .trim_end_matches('0')
                .trim_end_matches('.'),
        );
    }
}

fn write_label(label: &str, output: &mut String) {
    if label.chars().all(|character| {
        !character.is_whitespace()
            && !matches!(character, '(' | ')' | ',' | ':' | ';' | '[' | ']' | '\'')
    }) && !label.is_empty()
    {
        output.push_str(label);
    } else {
        output.push('\'');
        output.push_str(&label.replace('\'', "''"));
        output.push('\'');
    }
}

struct NewickParser<'a> {
    input: &'a [u8],
    offset: usize,
    node_count: usize,
}

impl<'a> NewickParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            offset: 0,
            node_count: 0,
        }
    }

    fn parse(&mut self) -> Result<TreeNode, PhylogenyError> {
        self.skip_ignored()?;
        let tree = self.parse_subtree()?;
        self.skip_ignored()?;
        self.expect(b';')?;
        self.skip_ignored()?;
        if self.offset != self.input.len() {
            return self.error("unexpected content after terminating semicolon");
        }
        Ok(tree)
    }

    fn parse_subtree(&mut self) -> Result<TreeNode, PhylogenyError> {
        self.node_count += 1;
        if self.node_count > MAX_NEWICK_NODES {
            return Err(PhylogenyError::LimitExceeded {
                resource: "node count",
                limit: MAX_NEWICK_NODES as u64,
            });
        }
        self.skip_ignored()?;
        let children = if self.peek() == Some(b'(') {
            self.offset += 1;
            let mut children = Vec::new();
            loop {
                children.push(self.parse_subtree()?);
                self.skip_ignored()?;
                match self.peek() {
                    Some(b',') => self.offset += 1,
                    Some(b')') => {
                        self.offset += 1;
                        break;
                    }
                    _ => return self.error("expected ',' or ')' in child list"),
                }
            }
            if children.len() < 2 {
                return self.error("an internal node requires at least two children");
            }
            children
        } else {
            Vec::new()
        };
        self.skip_ignored()?;
        let label = self.parse_optional_label()?;
        if children.is_empty() && label.is_none() {
            return self.error("leaf node is missing a label");
        }
        self.skip_ignored()?;
        let length = if self.peek() == Some(b':') {
            self.offset += 1;
            Some(self.parse_length()?)
        } else {
            None
        };
        Ok(TreeNode {
            label,
            length,
            children,
        })
    }

    fn parse_optional_label(&mut self) -> Result<Option<String>, PhylogenyError> {
        match self.peek() {
            None | Some(b'(' | b')' | b',' | b':' | b';') => Ok(None),
            Some(b'\'') => {
                self.offset += 1;
                let mut value = String::new();
                loop {
                    let Some(byte) = self.peek() else {
                        return self.error("unterminated quoted label");
                    };
                    self.offset += 1;
                    if byte == b'\'' {
                        if self.peek() == Some(b'\'') {
                            self.offset += 1;
                            value.push('\'');
                            continue;
                        }
                        break;
                    }
                    value.push(byte as char);
                }
                Ok(Some(value))
            }
            Some(_) => {
                let start = self.offset;
                while let Some(byte) = self.peek() {
                    if byte.is_ascii_whitespace()
                        || matches!(byte, b'(' | b')' | b',' | b':' | b';' | b'[')
                    {
                        break;
                    }
                    self.offset += 1;
                }
                let value = std::str::from_utf8(&self.input[start..self.offset])
                    .map_err(|_| PhylogenyError::InvalidUtf8)?;
                Ok((!value.is_empty()).then(|| value.to_owned()))
            }
        }
    }

    fn parse_length(&mut self) -> Result<f64, PhylogenyError> {
        self.skip_ignored()?;
        let start = self.offset;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b')' | b';' | b'[') {
                break;
            }
            self.offset += 1;
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| PhylogenyError::InvalidUtf8)?
            .parse::<f64>()
            .map_err(|_| PhylogenyError::MalformedNewick {
                offset: start,
                message: "branch length is not numeric".to_owned(),
            })?;
        if !value.is_finite() || value < 0.0 {
            return self.error("branch length must be finite and non-negative");
        }
        Ok(value)
    }

    fn skip_ignored(&mut self) -> Result<(), PhylogenyError> {
        loop {
            while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
                self.offset += 1;
            }
            if self.peek() != Some(b'[') {
                return Ok(());
            }
            self.offset += 1;
            while self.peek() != Some(b']') {
                if self.peek().is_none() {
                    return self.error("unterminated Newick comment");
                }
                self.offset += 1;
            }
            self.offset += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), PhylogenyError> {
        if self.peek() != Some(expected) {
            return self.error(&format!("expected '{}'", expected as char));
        }
        self.offset += 1;
        Ok(())
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.offset).copied()
    }

    fn error<T>(&self, message: &str) -> Result<T, PhylogenyError> {
        Err(PhylogenyError::MalformedNewick {
            offset: self.offset,
            message: message.to_owned(),
        })
    }
}

fn read_bounded_text(path: &Path) -> Result<String, PhylogenyError> {
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
        .take(MAX_NEWICK_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_NEWICK_DECOMPRESSED_BYTES {
        return Err(PhylogenyError::LimitExceeded {
            resource: "decompressed byte",
            limit: MAX_NEWICK_DECOMPRESSED_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| PhylogenyError::InvalidUtf8)
}

fn with_new_output<T>(
    output: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<T, PhylogenyError>,
) -> Result<T, PhylogenyError> {
    if output.exists() {
        return Err(PhylogenyError::OutputAlreadyExists(output.to_owned()));
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
            let _ = std::fs::remove_file(output);
            Err(error)
        }
    }
}

pub fn distance_matrix_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &DistanceMatrixOptions,
) -> Result<DistanceMatrixResult, PhylogenyError> {
    let model = options.model.trim().to_lowercase();
    if !matches!(model.as_str(), "p-distance" | "jc69" | "k80" | "k2p") {
        return Err(PhylogenyError::InvalidModel(options.model.clone()));
    }
    let model = if model == "k2p" {
        "k80".to_owned()
    } else {
        model
    };

    let mut sequences: Vec<(String, Vec<u8>)> = Vec::new();
    crate::sequence_transform::visit_fasta_path(input.as_ref(), |record| {
        if record.sequence.is_empty() {
            return Err(
                crate::sequence_transform::SequenceTransformError::InvalidOption(format!(
                    "sequence {} has zero length",
                    record.identifier
                )),
            );
        }
        sequences.push((record.identifier, record.sequence));
        Ok(())
    })
    .map_err(|error| PhylogenyError::InvalidOption(error.to_string()))?;

    if sequences.is_empty() {
        return Err(PhylogenyError::AlignmentEmpty);
    }
    if sequences.len() == 1 {
        return Err(PhylogenyError::AlignmentInconsistent(
            "alignment contains only one sequence; at least two are required for a distance matrix"
                .to_owned(),
        ));
    }

    let alignment_length = sequences[0].1.len() as u64;
    if alignment_length == 0 {
        return Err(PhylogenyError::AlignmentInconsistent(
            "alignment sequences have zero length".to_owned(),
        ));
    }

    for (id, seq) in &sequences {
        if seq.len() as u64 != alignment_length {
            return Err(PhylogenyError::AlignmentInconsistent(format!(
                "sequence {id} has length {} but alignment expects {alignment_length}",
                seq.len()
            )));
        }
    }

    let mut warnings = Vec::new();
    let n = sequences.len();
    let mut distances = Vec::with_capacity(n * n);

    let mut compared_position_count = 0_u64;
    for col in 0..alignment_length as usize {
        let mut non_gap_in_column = 0_usize;
        for (_, seq) in &sequences {
            let c = seq[col].to_ascii_uppercase();
            if c != b'-' {
                non_gap_in_column += 1;
            }
        }
        if non_gap_in_column >= 2 {
            compared_position_count += 1;
        }
    }

    for i in 0..n {
        for j in 0..n {
            if i == j {
                distances.push(DistanceMatrixEntry {
                    seq_a: sequences[i].0.clone(),
                    seq_b: sequences[j].0.clone(),
                    distance: 0.0,
                });
                continue;
            }
            let mut differing = 0_u64;
            let mut compared = 0_u64;
            let mut transitions = 0_u64;
            let mut transversions = 0_u64;
            let seq_a = &sequences[i].1;
            let seq_b = &sequences[j].1;
            for col in 0..alignment_length as usize {
                let a = seq_a[col].to_ascii_uppercase();
                let b = seq_b[col].to_ascii_uppercase();
                if a == b'-' && b == b'-' {
                    continue;
                }
                if a == b'-' || b == b'-' {
                    differing += 1;
                    compared += 1;
                    continue;
                }
                compared += 1;
                if a != b {
                    differing += 1;
                    if is_transition(a, b) {
                        transitions += 1;
                    } else {
                        transversions += 1;
                    }
                }
            }
            let p = if compared == 0 {
                0.0
            } else {
                differing as f64 / compared as f64
            };
            let distance = match model.as_str() {
                "jc69" => {
                    if p >= 0.75 {
                        warnings.push(format!(
                            "jc69 correction saturated for {}-{}: p-distance {:.6} >= 0.75",
                            sequences[i].0, sequences[j].0, p
                        ));
                        f64::INFINITY
                    } else {
                        -0.75 * (1.0 - 4.0 / 3.0 * p).ln()
                    }
                }
                "k80" => {
                    let p_trans = if compared == 0 {
                        0.0
                    } else {
                        transitions as f64 / compared as f64
                    };
                    let p_transv = if compared == 0 {
                        0.0
                    } else {
                        transversions as f64 / compared as f64
                    };
                    let term1 = 1.0 - 2.0 * p_trans - p_transv;
                    let term2 = 1.0 - 2.0 * p_transv;
                    if term1 <= 0.0 || term2 <= 0.0 {
                        warnings.push(format!(
                            "k80 correction saturated for {}-{}: P={:.6} Q={:.6}",
                            sequences[i].0, sequences[j].0, p_trans, p_transv
                        ));
                        f64::INFINITY
                    } else {
                        -0.5 * (term1 * term2.sqrt()).ln()
                    }
                }
                _ => p,
            };
            distances.push(DistanceMatrixEntry {
                seq_a: sequences[i].0.clone(),
                seq_b: sequences[j].0.clone(),
                distance,
            });
        }
    }

    let output = output.as_ref();
    with_new_output(output, |writer| {
        writeln!(writer, "seq_a\tseq_b\tdistance")?;
        for entry in &distances {
            writeln!(
                writer,
                "{}\t{}\t{:.10}",
                entry.seq_a, entry.seq_b, entry.distance
            )?;
        }
        Ok(())
    })?;

    Ok(DistanceMatrixResult {
        sequence_count: n as u64,
        alignment_length,
        compared_position_count,
        model,
        distances,
        warnings,
    })
}

fn is_transition(a: u8, b: u8) -> bool {
    matches!(
        (a, b),
        (b'A', b'G') | (b'G', b'A') | (b'C', b'T') | (b'T', b'C')
    )
}

#[cfg(test)]
mod tests {
    use super::{DistanceMatrixOptions, distance_matrix_path};
    use super::{TreeTransformOptions, transform_newick_path};
    use std::collections::BTreeMap;
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
    fn normalizes_maps_and_reroots_newick() {
        let input = temporary("input.nwk", "((A:1,B:1)N:2,C:4)Root;\n");
        let output = input.with_extension("out.nwk");
        let result = transform_newick_path(
            &input,
            &output,
            TreeTransformOptions {
                reroot_label: Some("C".to_owned()),
                label_map: BTreeMap::from([("A".to_owned(), "Alpha sample".to_owned())]),
            },
        )
        .expect("transform tree");
        let normalized = fs::read_to_string(&output).expect("read output");
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
        assert_eq!(result.leaf_count, 3);
        assert_eq!(result.relabeled_count, 1);
        assert!(result.rerooted);
        assert!(normalized.contains("'Alpha sample'"));
        assert!(normalized.ends_with(";\n"));
    }

    #[test]
    fn rejects_duplicate_leaf_labels_after_mapping() {
        let input = temporary("duplicate.nwk", "(A:1,B:1);\n");
        let output = input.with_extension("out.nwk");
        let error = transform_newick_path(
            &input,
            &output,
            TreeTransformOptions {
                reroot_label: None,
                label_map: BTreeMap::from([("A".to_owned(), "B".to_owned())]),
            },
        )
        .expect_err("duplicate labels must fail");
        fs::remove_file(input).expect("remove input");
        assert!(error.to_string().contains("duplicate leaf label"));
        assert!(!output.exists());
    }

    #[test]
    fn computes_pairwise_distance_matrix_from_alignment() {
        let input = temporary("dist.fa", ">seq1\nATCG\n>seq2\nATCA\n>seq3\nAT-G\n");
        let output = input.with_extension("dist.tsv");
        let result = distance_matrix_path(
            &input,
            &output,
            &DistanceMatrixOptions {
                model: "p-distance".to_owned(),
            },
        )
        .expect("compute distance matrix");
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
        assert_eq!(result.sequence_count, 3);
        assert_eq!(result.alignment_length, 4);
        assert_eq!(result.model, "p-distance");
        assert_eq!(result.distances.len(), 9);
        for entry in &result.distances {
            if entry.seq_a == entry.seq_b {
                assert_eq!(entry.distance, 0.0);
            }
        }
        let seq1_seq2 = result
            .distances
            .iter()
            .find(|e| e.seq_a == "seq1" && e.seq_b == "seq2")
            .expect("seq1-seq2 entry");
        assert_eq!(seq1_seq2.distance, 0.25);
        let seq1_seq3 = result
            .distances
            .iter()
            .find(|e| e.seq_a == "seq1" && e.seq_b == "seq3")
            .expect("seq1-seq3 entry");
        assert_eq!(seq1_seq3.distance, 0.25);
        let seq2_seq3 = result
            .distances
            .iter()
            .find(|e| e.seq_a == "seq2" && e.seq_b == "seq3")
            .expect("seq2-seq3 entry");
        assert_eq!(seq2_seq3.distance, 0.5);
    }

    #[test]
    fn computes_jc69_distance_from_alignment() {
        let input = temporary("jc69.fa", ">seq1\nATCG\n>seq2\nATCA\n>seq3\nAT-G\n");
        let output = input.with_extension("jc69.tsv");
        let result = distance_matrix_path(
            &input,
            &output,
            &DistanceMatrixOptions {
                model: "jc69".to_owned(),
            },
        )
        .expect("compute jc69 distance");
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
        let seq1_seq2 = result
            .distances
            .iter()
            .find(|e| e.seq_a == "seq1" && e.seq_b == "seq2")
            .expect("seq1-seq2 entry");
        assert!((seq1_seq2.distance - 0.304098831).abs() < 1e-6);
    }

    #[test]
    fn distance_matrix_rejects_inconsistent_alignment() {
        let input = temporary("bad.fa", ">seq1\nATCG\n>seq2\nATC\n");
        let output = input.with_extension("bad.tsv");
        let error = distance_matrix_path(
            &input,
            &output,
            &DistanceMatrixOptions {
                model: "p-distance".to_owned(),
            },
        )
        .expect_err("inconsistent lengths must fail");
        fs::remove_file(input).expect("remove input");
        assert!(error.to_string().contains("has length"));
    }

    #[test]
    fn distance_matrix_does_not_overwrite_existing_output() {
        let input = temporary("overwrite.fa", ">seq1\nATCG\n>seq2\nATCG\n");
        let output = input.with_extension("out.tsv");
        fs::write(&output, "existing").expect("write existing");
        let error = distance_matrix_path(
            &input,
            &output,
            &DistanceMatrixOptions {
                model: "p-distance".to_owned(),
            },
        )
        .expect_err("overwrite must fail");
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
        assert!(error.to_string().contains("refusing to overwrite"));
    }
}
