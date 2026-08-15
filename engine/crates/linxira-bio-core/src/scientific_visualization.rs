use crate::annotation::{
    AnnotationError, AnnotationVisualFeature, annotation_visual_features_path,
};
use crate::domain::{DomainError, ProteinDomainHit, parse_protein_domains_path};
use crate::functional::{
    EnrichmentKind, EnrichmentOptions, EnrichmentResult, FunctionalError, overrepresentation_path,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_VISUALIZATION_WIDTH: u32 = 1_200;
pub const DEFAULT_MAX_VISUAL_ITEMS: usize = 100;
pub const MAX_VISUAL_ITEMS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentPlotStyle {
    Bar,
    Dot,
    Network,
}

impl EnrichmentPlotStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Dot => "dot",
            Self::Network => "network",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationStructureOptions {
    pub feature_id: Option<String>,
    pub seqid: Option<String>,
    pub max_features: usize,
}

impl Default for AnnotationStructureOptions {
    fn default() -> Self {
        Self {
            feature_id: None,
            seqid: None,
            max_features: DEFAULT_MAX_VISUAL_ITEMS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainArchitectureOptions {
    pub sequence_id: Option<String>,
    pub max_sequences: usize,
    pub max_domains: usize,
}

impl Default for DomainArchitectureOptions {
    fn default() -> Self {
        Self {
            sequence_id: None,
            max_sequences: 30,
            max_domains: 500,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrichmentVisualizationOptions {
    pub style: EnrichmentPlotStyle,
    pub max_terms: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntenyPlotStyle {
    Dual,
    Multiple,
    Micro,
    Circular,
}

impl SyntenyPlotStyle {
    pub fn parse(value: &str) -> Result<Self, VisualizationError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dual" => Ok(Self::Dual),
            "multiple" => Ok(Self::Multiple),
            "micro" => Ok(Self::Micro),
            "circular" => Ok(Self::Circular),
            _ => Err(VisualizationError::InvalidOption(format!(
                "unsupported synteny plot style: {value}"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dual => "dual",
            Self::Multiple => "multiple",
            Self::Micro => "micro",
            Self::Circular => "circular",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntenyVisualizationOptions {
    pub style: SyntenyPlotStyle,
}

impl Default for SyntenyVisualizationOptions {
    fn default() -> Self {
        Self {
            style: SyntenyPlotStyle::Dual,
        }
    }
}

impl Default for EnrichmentVisualizationOptions {
    fn default() -> Self {
        Self {
            style: EnrichmentPlotStyle::Bar,
            max_terms: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgVisualizationResult {
    pub visualization_type: String,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub track_count: u64,
    pub glyph_count: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolcanoPlotOptions {
    pub adjusted_pvalue_threshold: f64,
    pub absolute_log2_fold_change_threshold: f64,
    pub max_points: usize,
}

impl Default for VolcanoPlotOptions {
    fn default() -> Self {
        Self {
            adjusted_pvalue_threshold: 0.05,
            absolute_log2_fold_change_threshold: 1.0,
            max_points: DEFAULT_MAX_VISUAL_ITEMS,
        }
    }
}

#[derive(Debug)]
pub enum VisualizationError {
    Annotation(AnnotationError),
    Domain(DomainError),
    Functional(FunctionalError),
    Io(io::Error),
    OutputAlreadyExists(PathBuf),
    InvalidOption(String),
    EmptyInput(String),
}

impl Display for VisualizationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Annotation(error) => {
                write!(formatter, "annotation visualization failed: {error}")
            }
            Self::Domain(error) => write!(formatter, "domain visualization failed: {error}"),
            Self::Functional(error) => {
                write!(formatter, "enrichment visualization failed: {error}")
            }
            Self::Io(error) => write!(formatter, "visualization I/O failed: {error}"),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::InvalidOption(message) | Self::EmptyInput(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl Error for VisualizationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Annotation(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Functional(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::OutputAlreadyExists(_) | Self::InvalidOption(_) | Self::EmptyInput(_) => None,
        }
    }
}

impl From<AnnotationError> for VisualizationError {
    fn from(error: AnnotationError) -> Self {
        Self::Annotation(error)
    }
}

impl From<DomainError> for VisualizationError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<FunctionalError> for VisualizationError {
    fn from(error: FunctionalError) -> Self {
        Self::Functional(error)
    }
}

impl From<io::Error> for VisualizationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn render_volcano_svg_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &VolcanoPlotOptions,
) -> Result<SvgVisualizationResult, VisualizationError> {
    validate_item_limit(options.max_points, "max_points")?;
    if !options.adjusted_pvalue_threshold.is_finite()
        || !(0.0..=1.0).contains(&options.adjusted_pvalue_threshold)
        || !options.absolute_log2_fold_change_threshold.is_finite()
        || options.absolute_log2_fold_change_threshold < 0.0
    {
        return Err(VisualizationError::InvalidOption(
            "volcano thresholds must be finite, with adjusted p value in [0, 1]".to_owned(),
        ));
    }
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(input.as_ref())
        .map_err(|error| VisualizationError::InvalidOption(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| VisualizationError::InvalidOption(error.to_string()))?
        .clone();
    let fold = headers
        .iter()
        .position(|value| value == "log2FoldChange")
        .ok_or_else(|| {
            VisualizationError::InvalidOption(
                "volcano input requires a log2FoldChange column".to_owned(),
            )
        })?;
    let padj = headers
        .iter()
        .position(|value| value == "padj")
        .ok_or_else(|| {
            VisualizationError::InvalidOption("volcano input requires a padj column".to_owned())
        })?;
    let mut points = Vec::new();
    for record in reader.records().take(options.max_points) {
        let record =
            record.map_err(|error| VisualizationError::InvalidOption(error.to_string()))?;
        let x = record.get(fold).unwrap_or_default().parse::<f64>().ok();
        let p = record.get(padj).unwrap_or_default().parse::<f64>().ok();
        if let (Some(x), Some(p)) = (x, p)
            && x.is_finite()
            && p.is_finite()
            && (0.0..=1.0).contains(&p)
        {
            points.push((x, negative_log10(p)));
        }
    }
    if points.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "volcano input has no finite log2FoldChange/padj rows".to_owned(),
        ));
    }
    let max_x = points.iter().map(|(x, _)| x.abs()).fold(1.0_f64, f64::max);
    let max_y = points.iter().map(|(_, y)| *y).fold(1.0_f64, f64::max);
    let (width, height) = (DEFAULT_VISUALIZATION_WIDTH, 760_u32);
    let mut svg = svg_header(width, height, "Differential expression volcano plot");
    svg.push_str("<line x1=\"100\" y1=\"680\" x2=\"1140\" y2=\"680\" stroke=\"#334\"/><line x1=\"620\" y1=\"60\" x2=\"620\" y2=\"680\" stroke=\"#ccd\"/>");
    for (x, y) in &points {
        let px = 620.0 + x / max_x * 500.0;
        let py = 680.0 - y / max_y * 590.0;
        let significant = *y >= negative_log10(options.adjusted_pvalue_threshold)
            && x.abs() >= options.absolute_log2_fold_change_threshold;
        let color = if significant {
            if *x > 0.0 { "#b1433f" } else { "#2f68a5" }
        } else {
            "#89939d"
        };
        svg.push_str(&format!(
            "<circle cx=\"{px:.2}\" cy=\"{py:.2}\" r=\"3\" fill=\"{color}\" fill-opacity=\"0.75\"/>"
        ));
    }
    push_text(&mut svg, 100.0, 725.0, 18, "#223", "log2 fold change");
    push_text(&mut svg, 20.0, 50.0, 18, "#223", "-log10 adjusted p value");
    svg.push_str("</svg>");
    write_new_output(output.as_ref(), svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "expression-volcano".to_owned(),
        output_path: output.as_ref().to_string_lossy().into_owned(),
        width,
        height,
        track_count: 1,
        glyph_count: points.len() as u64,
        warnings: Vec::new(),
    })
}

pub fn render_motif_logo_svg_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<SvgVisualizationResult, VisualizationError> {
    let text = fs::read_to_string(input).map_err(VisualizationError::Io)?;
    let alphabet = text
        .lines()
        .find_map(|line| line.strip_prefix("ALPHABET=").map(str::trim))
        .ok_or_else(|| VisualizationError::InvalidOption("MEME input lacks ALPHABET".to_owned()))?;
    let symbols = alphabet
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect::<Vec<_>>();
    if symbols.len() < 2 {
        return Err(VisualizationError::InvalidOption(
            "MEME alphabet must contain at least two symbols".to_owned(),
        ));
    }
    let matrix = text
        .lines()
        .skip_while(|line| !line.starts_with("letter-probability matrix:"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let values = line
                .split_whitespace()
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            (values.len() == symbols.len()
                && values
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0))
            .then_some(values)
        })
        .collect::<Vec<_>>();
    if matrix.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "MEME input has no valid probability matrix".to_owned(),
        ));
    }
    let width = 100_u32.saturating_add((matrix.len() as u32).saturating_mul(90));
    let height = 420_u32;
    let mut svg = svg_header(width, height, "Motif sequence logo");
    for (position, values) in matrix.iter().enumerate() {
        for (index, value) in values.iter().enumerate() {
            let h = value * 280.0;
            let x = 55.0 + position as f64 * 90.0;
            let y = 350.0 - index as f64 * 70.0;
            svg.push_str(&format!("<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"sans-serif\" font-size=\"{h:.1}\" fill=\"{}\">{}</text>", color_for(&symbols[index].to_string()), symbols[index]));
        }
    }
    svg.push_str("</svg>");
    write_new_output(output.as_ref(), svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "motif-logo".to_owned(),
        output_path: output.as_ref().to_string_lossy().into_owned(),
        width,
        height,
        track_count: 1,
        glyph_count: matrix.len() as u64 * symbols.len() as u64,
        warnings: Vec::new(),
    })
}

/// Render a two-track synteny plot from a tab-separated anchor table.
/// The table must have `source_id`, `source_position`, `target_id`, and
/// `target_position` headers; positions are normalized within each track.
pub fn render_synteny_svg_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<SvgVisualizationResult, VisualizationError> {
    render_synteny_svg_with_options_path(input, output, &SyntenyVisualizationOptions::default())
}

/// Render a local synteny SVG from a tab-separated anchor table.
/// The style changes only layout; it never infers anchors or collinearity.
pub fn render_synteny_svg_with_options_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SyntenyVisualizationOptions,
) -> Result<SvgVisualizationResult, VisualizationError> {
    let input = input.as_ref();
    let output = output.as_ref();
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .trim(csv::Trim::All)
        .from_path(input)
        .map_err(|error| VisualizationError::InvalidOption(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| VisualizationError::InvalidOption(error.to_string()))?
        .clone();
    let required = [
        "source_id",
        "source_position",
        "target_id",
        "target_position",
    ];
    let indices = required
        .iter()
        .map(|name| {
            headers
                .iter()
                .position(|header| header == *name)
                .ok_or_else(|| {
                    VisualizationError::InvalidOption(format!("synteny table is missing {name}"))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut anchors = Vec::new();
    for record in reader.records().take(MAX_VISUAL_ITEMS) {
        let record =
            record.map_err(|error| VisualizationError::InvalidOption(error.to_string()))?;
        let source = record[indices[1]].parse::<f64>().map_err(|_| {
            VisualizationError::InvalidOption("source_position must be numeric".to_owned())
        })?;
        let target = record[indices[3]].parse::<f64>().map_err(|_| {
            VisualizationError::InvalidOption("target_position must be numeric".to_owned())
        })?;
        if !source.is_finite() || !target.is_finite() {
            return Err(VisualizationError::InvalidOption(
                "synteny positions must be finite".to_owned(),
            ));
        }
        anchors.push((
            record[indices[0]].to_owned(),
            source,
            record[indices[2]].to_owned(),
            target,
        ));
    }
    if anchors.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "synteny table contains no anchors".to_owned(),
        ));
    }
    if options.style == SyntenyPlotStyle::Multiple {
        return render_multiple_synteny_svg(output, &anchors);
    }
    if options.style == SyntenyPlotStyle::Circular {
        return render_circular_synteny_svg(output, &anchors);
    }
    let source_min = anchors
        .iter()
        .map(|anchor| anchor.1)
        .fold(f64::INFINITY, f64::min);
    let source_max = anchors
        .iter()
        .map(|anchor| anchor.1)
        .fold(f64::NEG_INFINITY, f64::max);
    let target_min = anchors
        .iter()
        .map(|anchor| anchor.3)
        .fold(f64::INFINITY, f64::min);
    let target_max = anchors
        .iter()
        .map(|anchor| anchor.3)
        .fold(f64::NEG_INFINITY, f64::max);
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let height = 440;
    let left = 90.0;
    let right = f64::from(width) - 90.0;
    let scale = |value: f64, minimum: f64, maximum: f64| {
        if maximum == minimum {
            (left + right) / 2.0
        } else {
            left + (value - minimum) / (maximum - minimum) * (right - left)
        }
    };
    let title = if options.style == SyntenyPlotStyle::Micro {
        "Micro-synteny anchors"
    } else {
        "Synteny anchors"
    };
    let mut svg = svg_header(width, height, title);
    push_text(&mut svg, 24.0, 34.0, 20, "#18332b", title);
    svg.push_str(&format!("<line x1=\"{left}\" y1=\"110\" x2=\"{right}\" y2=\"110\" stroke=\"#294c62\" stroke-width=\"10\"/><line x1=\"{left}\" y1=\"330\" x2=\"{right}\" y2=\"330\" stroke=\"#6c5634\" stroke-width=\"10\"/>"));
    push_text(&mut svg, left, 88.0, 13, "#263d36", &anchors[0].0);
    push_text(&mut svg, left, 370.0, 13, "#263d36", &anchors[0].2);
    for (index, anchor) in anchors.iter().enumerate() {
        let source = scale(anchor.1, source_min, source_max);
        let target = scale(anchor.3, target_min, target_max);
        svg.push_str(&format!("<path d=\"M {source:.1} 116 C {source:.1} 190, {target:.1} 250, {target:.1} 324\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"0.55\" stroke-width=\"2\"><title>{} -> {}</title></path>", color_for(&format!("{}:{}", anchor.0, anchor.2)), xml_escape(&anchor.0), xml_escape(&anchor.2)));
        if index >= MAX_VISUAL_ITEMS {
            break;
        }
    }
    svg.push_str("</svg>");
    write_new_output(output, svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: format!("synteny-{}", options.style.as_str()),
        output_path: output.display().to_string(),
        width,
        height,
        track_count: 2,
        glyph_count: anchors.len() as u64,
        warnings: Vec::new(),
    })
}

fn render_multiple_synteny_svg(
    output: &Path,
    anchors: &[(String, f64, String, f64)],
) -> Result<SvgVisualizationResult, VisualizationError> {
    let mut labels = BTreeSet::new();
    for (source_id, _, target_id, _) in anchors {
        labels.insert(source_id.clone());
        labels.insert(target_id.clone());
    }
    let labels = labels.into_iter().take(12).collect::<Vec<_>>();
    let track_count = labels.len();
    if track_count < 2 {
        return Err(VisualizationError::InvalidOption(
            "multiple synteny requires at least two track identifiers".to_owned(),
        ));
    }
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let height = 130_u32.saturating_add((track_count as u32).saturating_mul(86));
    let left = 150.0;
    let right = f64::from(width) - 80.0;
    let min = anchors
        .iter()
        .map(|anchor| anchor.1.min(anchor.3))
        .fold(f64::INFINITY, f64::min);
    let max = anchors
        .iter()
        .map(|anchor| anchor.1.max(anchor.3))
        .fold(f64::NEG_INFINITY, f64::max);
    let scale = |value: f64| {
        if max == min {
            (left + right) / 2.0
        } else {
            left + (value - min) / (max - min) * (right - left)
        }
    };
    let y_for = |identifier: &str| {
        95.0 + labels
            .iter()
            .position(|label| label == identifier)
            .unwrap_or(0) as f64
            * 86.0
    };
    let mut svg = svg_header(width, height, "Multiple synteny anchors");
    push_text(
        &mut svg,
        24.0,
        34.0,
        20,
        "#18332b",
        "Multiple synteny anchors",
    );
    for label in &labels {
        let y = y_for(label);
        svg.push_str(&format!("<line x1=\"{left}\" y1=\"{y:.1}\" x2=\"{right}\" y2=\"{y:.1}\" stroke=\"#536b78\" stroke-width=\"8\"/>"));
        push_text(&mut svg, 20.0, y + 5.0, 13, "#263d36", label);
    }
    for (source_id, source_position, target_id, target_position) in anchors {
        if !labels.contains(source_id) || !labels.contains(target_id) {
            continue;
        }
        let sx = scale(*source_position);
        let tx = scale(*target_position);
        let sy = y_for(source_id);
        let ty = y_for(target_id);
        let control_y = (sy + ty) / 2.0;
        svg.push_str(&format!("<path d=\"M {sx:.1} {sy:.1} C {sx:.1} {control_y:.1}, {tx:.1} {control_y:.1}, {tx:.1} {ty:.1}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"0.52\" stroke-width=\"2\"><title>{} -&gt; {}</title></path>", color_for(&format!("{source_id}:{target_id}")), xml_escape(source_id), xml_escape(target_id)));
    }
    svg.push_str("</svg>");
    write_new_output(output, svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "synteny-multiple".to_owned(),
        output_path: output.display().to_string(),
        width,
        height,
        track_count: track_count as u64,
        glyph_count: anchors.len() as u64,
        warnings: Vec::new(),
    })
}

fn render_circular_synteny_svg(
    output: &Path,
    anchors: &[(String, f64, String, f64)],
) -> Result<SvgVisualizationResult, VisualizationError> {
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let height = 760;
    let cx = f64::from(width) / 2.0;
    let cy = f64::from(height) / 2.0 + 20.0;
    let radius = 255.0;
    let min = anchors
        .iter()
        .map(|anchor| anchor.1.min(anchor.3))
        .fold(f64::INFINITY, f64::min);
    let max = anchors
        .iter()
        .map(|anchor| anchor.1.max(anchor.3))
        .fold(f64::NEG_INFINITY, f64::max);
    let angle = |value: f64, offset: f64| {
        if max == min {
            offset
        } else {
            offset + (value - min) / (max - min) * std::f64::consts::PI
        }
    };
    let point = |value: f64, offset: f64| {
        let theta = angle(value, offset);
        (cx + radius * theta.cos(), cy + radius * theta.sin())
    };
    let mut svg = svg_header(width, height, "Circular synteny anchors");
    push_text(
        &mut svg,
        24.0,
        34.0,
        20,
        "#18332b",
        "Circular synteny anchors",
    );
    svg.push_str(&format!("<path d=\"M {:.1} {:.1} A {radius} {radius} 0 0 1 {:.1} {:.1}\" fill=\"none\" stroke=\"#294c62\" stroke-width=\"12\"/><path d=\"M {:.1} {:.1} A {radius} {radius} 0 0 1 {:.1} {:.1}\" fill=\"none\" stroke=\"#6c5634\" stroke-width=\"12\"/>", cx - radius, cy, cx + radius, cy, cx + radius, cy, cx - radius, cy));
    for (source_id, source_position, target_id, target_position) in anchors {
        let (sx, sy) = point(*source_position, std::f64::consts::PI);
        let (tx, ty) = point(*target_position, 0.0);
        svg.push_str(&format!("<path d=\"M {sx:.1} {sy:.1} Q {cx:.1} {cy:.1} {tx:.1} {ty:.1}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"0.5\" stroke-width=\"2\"><title>{} -&gt; {}</title></path>", color_for(&format!("{source_id}:{target_id}")), xml_escape(source_id), xml_escape(target_id)));
    }
    svg.push_str("</svg>");
    write_new_output(output, svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "synteny-circular".to_owned(),
        output_path: output.display().to_string(),
        width,
        height,
        track_count: 2,
        glyph_count: anchors.len() as u64,
        warnings: Vec::new(),
    })
}

pub fn render_annotation_structure_svg_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &AnnotationStructureOptions,
) -> Result<SvgVisualizationResult, VisualizationError> {
    validate_item_limit(options.max_features, "max_features")?;
    let features = annotation_visual_features_path(input)?;
    let (mut selected, locus_start, locus_end, seqid) =
        select_annotation_features(features, options)?;
    let mut warnings = Vec::new();
    if selected.len() > options.max_features {
        selected.truncate(options.max_features);
        warnings.push(format!(
            "annotation visualization was limited to {} features",
            options.max_features
        ));
    }
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let height = 120_u32.saturating_add((selected.len() as u32).saturating_mul(34));
    let svg = annotation_svg(&selected, &seqid, locus_start, locus_end, width, height);
    write_new_output(output.as_ref(), svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "annotation-structure".to_owned(),
        output_path: output.as_ref().to_string_lossy().into_owned(),
        width,
        height,
        track_count: selected.len() as u64,
        glyph_count: selected.len() as u64,
        warnings,
    })
}

pub fn render_domain_architecture_svg_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &DomainArchitectureOptions,
) -> Result<SvgVisualizationResult, VisualizationError> {
    validate_item_limit(options.max_sequences, "max_sequences")?;
    validate_item_limit(options.max_domains, "max_domains")?;
    let parsed = parse_protein_domains_path(input)?;
    let mut warnings = parsed.warnings;
    let mut grouped = BTreeMap::<String, Vec<ProteinDomainHit>>::new();
    for hit in parsed.hits {
        if options
            .sequence_id
            .as_deref()
            .is_some_and(|wanted| wanted != hit.sequence_id)
        {
            continue;
        }
        grouped
            .entry(hit.sequence_id.clone())
            .or_default()
            .push(hit);
    }
    if grouped.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "no protein-domain hits matched the requested sequence".to_owned(),
        ));
    }
    let mut groups = grouped.into_iter().collect::<Vec<_>>();
    if groups.len() > options.max_sequences {
        groups.truncate(options.max_sequences);
        warnings.push(format!(
            "domain visualization was limited to {} sequences",
            options.max_sequences
        ));
    }
    let mut retained = 0_usize;
    for (_, hits) in &mut groups {
        hits.sort_by(|left, right| {
            left.start
                .cmp(&right.start)
                .then_with(|| left.end.cmp(&right.end))
                .then_with(|| left.accession.cmp(&right.accession))
        });
        if retained >= options.max_domains {
            hits.clear();
        } else if retained.saturating_add(hits.len()) > options.max_domains {
            hits.truncate(options.max_domains - retained);
        }
        retained = retained.saturating_add(hits.len());
    }
    groups.retain(|(_, hits)| !hits.is_empty());
    if retained == options.max_domains {
        warnings.push(format!(
            "domain visualization was limited to {} domain glyphs",
            options.max_domains
        ));
    }
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let height = 100_u32.saturating_add((groups.len() as u32).saturating_mul(74));
    let svg = domain_svg(&groups, width, height);
    write_new_output(output.as_ref(), svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: "protein-domain-architecture".to_owned(),
        output_path: output.as_ref().to_string_lossy().into_owned(),
        width,
        height,
        track_count: groups.len() as u64,
        glyph_count: retained as u64,
        warnings,
    })
}

pub fn render_enrichment_svg_path(
    genes: impl AsRef<Path>,
    associations: impl AsRef<Path>,
    output: impl AsRef<Path>,
    kind: EnrichmentKind,
    mut analysis_options: EnrichmentOptions,
    visualization_options: EnrichmentVisualizationOptions,
) -> Result<SvgVisualizationResult, VisualizationError> {
    validate_item_limit(visualization_options.max_terms, "max_terms")?;
    analysis_options.max_terms = visualization_options.max_terms;
    if visualization_options.style == EnrichmentPlotStyle::Network {
        analysis_options.include_genes = true;
    }
    let result = overrepresentation_path(genes, associations, kind, analysis_options)?;
    if result.terms.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "enrichment result contains no reportable terms".to_owned(),
        ));
    }
    let width = DEFAULT_VISUALIZATION_WIDTH;
    let (svg, height, glyph_count) = match visualization_options.style {
        EnrichmentPlotStyle::Bar => enrichment_bar_svg(&result, width),
        EnrichmentPlotStyle::Dot => enrichment_dot_svg(&result, width),
        EnrichmentPlotStyle::Network => enrichment_network_svg(&result, width),
    };
    write_new_output(output.as_ref(), svg.as_bytes())?;
    Ok(SvgVisualizationResult {
        visualization_type: format!("enrichment-{}", visualization_options.style.as_str()),
        output_path: output.as_ref().to_string_lossy().into_owned(),
        width,
        height,
        track_count: result.terms.len() as u64,
        glyph_count,
        warnings: result.warnings,
    })
}

fn select_annotation_features(
    mut features: Vec<AnnotationVisualFeature>,
    options: &AnnotationStructureOptions,
) -> Result<(Vec<AnnotationVisualFeature>, u64, u64, String), VisualizationError> {
    if features.is_empty() {
        return Err(VisualizationError::EmptyInput(
            "annotation contains no feature records".to_owned(),
        ));
    }
    features.sort_by(|left, right| {
        left.seqid
            .cmp(&right.seqid)
            .then_with(|| left.start.cmp(&right.start))
            .then_with(|| left.end.cmp(&right.end))
            .then_with(|| left.feature_type.cmp(&right.feature_type))
    });
    let anchor = if let Some(feature_id) = options.feature_id.as_deref() {
        features
            .iter()
            .find(|feature| {
                feature.id.as_deref() == Some(feature_id)
                    || feature.label.as_deref() == Some(feature_id)
            })
            .ok_or_else(|| {
                VisualizationError::InvalidOption(format!(
                    "annotation feature ID {feature_id:?} was not found"
                ))
            })?
    } else if let Some(seqid) = options.seqid.as_deref() {
        features
            .iter()
            .find(|feature| feature.seqid == seqid)
            .ok_or_else(|| {
                VisualizationError::InvalidOption(format!(
                    "annotation sequence {seqid:?} was not found"
                ))
            })?
    } else {
        features
            .iter()
            .find(|feature| {
                matches!(
                    feature.feature_type.to_ascii_lowercase().as_str(),
                    "gene" | "mrna" | "transcript"
                )
            })
            .unwrap_or(&features[0])
    };
    let seqid = anchor.seqid.clone();
    let (locus_start, locus_end) = if options.feature_id.is_some() {
        (anchor.start, anchor.end)
    } else {
        let mut start = u64::MAX;
        let mut end = 0_u64;
        for feature in features.iter().filter(|feature| feature.seqid == seqid) {
            start = start.min(feature.start);
            end = end.max(feature.end);
        }
        (start, end)
    };
    let selected = features
        .into_iter()
        .filter(|feature| {
            feature.seqid == seqid && feature.start <= locus_end && feature.end >= locus_start
        })
        .collect::<Vec<_>>();
    Ok((selected, locus_start, locus_end, seqid))
}

fn annotation_svg(
    features: &[AnnotationVisualFeature],
    seqid: &str,
    locus_start: u64,
    locus_end: u64,
    width: u32,
    height: u32,
) -> String {
    let left = 270.0_f64;
    let right = f64::from(width) - 40.0;
    let span = locus_end
        .saturating_sub(locus_start)
        .saturating_add(1)
        .max(1) as f64;
    let mut svg = svg_header(width, height, "Annotation structure");
    push_text(
        &mut svg,
        24.0,
        32.0,
        20,
        "#18332b",
        &format!("{}:{}-{}", seqid, locus_start, locus_end),
    );
    svg.push_str(&format!(
        "<line x1=\"{left:.1}\" y1=\"58\" x2=\"{right:.1}\" y2=\"58\" stroke=\"#78928a\" stroke-width=\"1\"/>"
    ));
    for (index, feature) in features.iter().enumerate() {
        let y = 86.0 + index as f64 * 34.0;
        let x = left + feature.start.saturating_sub(locus_start) as f64 / span * (right - left);
        let end_x = left
            + feature.end.saturating_sub(locus_start).saturating_add(1) as f64 / span
                * (right - left);
        let feature_width = (end_x - x).max(2.0);
        let label = feature
            .label
            .as_deref()
            .or(feature.id.as_deref())
            .unwrap_or(&feature.feature_type);
        push_text(
            &mut svg,
            24.0,
            y + 5.0,
            13,
            "#263d36",
            &format!("{}  {}", feature.feature_type, truncate_label(label, 26)),
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.1}\" y1=\"{y:.1}\" x2=\"{right:.1}\" y2=\"{y:.1}\" stroke=\"#d7e1dd\" stroke-width=\"1\"/>"
        ));
        svg.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{feature_width:.1}\" height=\"16\" rx=\"3\" fill=\"{}\"><title>{}</title></rect>",
            y - 8.0,
            color_for(&feature.feature_type),
            xml_escape(&format!(
                "{} {}:{}-{} ({})",
                feature.feature_type, feature.seqid, feature.start, feature.end, feature.strand
            ))
        ));
    }
    svg.push_str("</svg>");
    svg
}

fn domain_svg(groups: &[(String, Vec<ProteinDomainHit>)], width: u32, height: u32) -> String {
    let left = 250.0_f64;
    let right = f64::from(width) - 40.0;
    let mut svg = svg_header(width, height, "Protein domain architecture");
    push_text(
        &mut svg,
        24.0,
        32.0,
        20,
        "#18332b",
        "Protein domain architecture",
    );
    for (index, (sequence_id, hits)) in groups.iter().enumerate() {
        let y = 82.0 + index as f64 * 74.0;
        let sequence_length = hits
            .iter()
            .filter_map(|hit| hit.sequence_length)
            .chain(hits.iter().map(|hit| hit.end))
            .max()
            .unwrap_or(1)
            .max(1);
        push_text(
            &mut svg,
            24.0,
            y + 5.0,
            14,
            "#263d36",
            &truncate_label(sequence_id, 30),
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.1}\" y1=\"{y:.1}\" x2=\"{right:.1}\" y2=\"{y:.1}\" stroke=\"#56746a\" stroke-width=\"3\"/>"
        ));
        for hit in hits {
            let x =
                left + hit.start.saturating_sub(1) as f64 / sequence_length as f64 * (right - left);
            let end_x = left + hit.end as f64 / sequence_length as f64 * (right - left);
            let domain_width = (end_x - x).max(3.0);
            svg.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{domain_width:.1}\" height=\"24\" rx=\"5\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"1\"><title>{}</title></rect>",
                y - 12.0,
                color_for(&hit.source),
                xml_escape(&format!(
                    "{} {} {}-{}",
                    hit.source,
                    hit.name.as_deref().unwrap_or(&hit.accession),
                    hit.start,
                    hit.end
                ))
            ));
        }
        push_text(
            &mut svg,
            right - 70.0,
            y + 28.0,
            11,
            "#56746a",
            &format!("{sequence_length} aa"),
        );
    }
    svg.push_str("</svg>");
    svg
}

fn enrichment_bar_svg(result: &EnrichmentResult, width: u32) -> (String, u32, u64) {
    let height = 110_u32.saturating_add((result.terms.len() as u32).saturating_mul(38));
    let left = 390.0_f64;
    let right = f64::from(width) - 60.0;
    let maximum = result
        .terms
        .iter()
        .map(|term| negative_log10(term.adjusted_p_value))
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut svg = svg_header(width, height, "Enrichment bar plot");
    push_text(
        &mut svg,
        24.0,
        32.0,
        20,
        "#18332b",
        "Enrichment significance",
    );
    push_text(
        &mut svg,
        left,
        58.0,
        12,
        "#56746a",
        "-log10 adjusted p-value",
    );
    for (index, term) in result.terms.iter().enumerate() {
        let y = 84.0 + index as f64 * 38.0;
        let value = negative_log10(term.adjusted_p_value);
        let bar_width = value / maximum * (right - left);
        push_text(
            &mut svg,
            24.0,
            y + 5.0,
            13,
            "#263d36",
            &truncate_label(&term_label(term), 48),
        );
        svg.push_str(&format!(
            "<rect x=\"{left:.1}\" y=\"{:.1}\" width=\"{bar_width:.1}\" height=\"20\" rx=\"4\" fill=\"{}\"><title>{:.4}</title></rect>",
            y - 10.0,
            color_for(term.namespace.as_deref().unwrap_or("enrichment")),
            value
        ));
    }
    svg.push_str("</svg>");
    (svg, height, result.terms.len() as u64)
}

fn enrichment_dot_svg(result: &EnrichmentResult, width: u32) -> (String, u32, u64) {
    let height = 110_u32.saturating_add((result.terms.len() as u32).saturating_mul(42));
    let left = 390.0_f64;
    let right = f64::from(width) - 60.0;
    let maximum = result
        .terms
        .iter()
        .map(|term| term.fold_enrichment)
        .filter(|value| value.is_finite())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut svg = svg_header(width, height, "Enrichment dot plot");
    push_text(&mut svg, 24.0, 32.0, 20, "#18332b", "Enrichment dot plot");
    push_text(&mut svg, left, 58.0, 12, "#56746a", "Fold enrichment");
    for (index, term) in result.terms.iter().enumerate() {
        let y = 86.0 + index as f64 * 42.0;
        let x = left + term.fold_enrichment.max(0.0) / maximum * (right - left);
        let radius = 5.0 + (term.overlap_count as f64).sqrt().min(10.0);
        let opacity = (0.35 + negative_log10(term.adjusted_p_value) / 20.0).clamp(0.35, 1.0);
        push_text(
            &mut svg,
            24.0,
            y + 5.0,
            13,
            "#263d36",
            &truncate_label(&term_label(term), 48),
        );
        svg.push_str(&format!(
            "<line x1=\"{left:.1}\" y1=\"{y:.1}\" x2=\"{right:.1}\" y2=\"{y:.1}\" stroke=\"#edf2f0\"/>"
        ));
        svg.push_str(&format!(
            "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{radius:.1}\" fill=\"{}\" fill-opacity=\"{opacity:.3}\"><title>fold {:.4}; overlap {}; adjusted p {:.4e}</title></circle>",
            color_for(term.namespace.as_deref().unwrap_or("enrichment")),
            term.fold_enrichment,
            term.overlap_count,
            term.adjusted_p_value
        ));
    }
    svg.push_str("</svg>");
    (svg, height, result.terms.len() as u64)
}

fn enrichment_network_svg(result: &EnrichmentResult, width: u32) -> (String, u32, u64) {
    let terms = result.terms.iter().take(12).collect::<Vec<_>>();
    let genes = terms
        .iter()
        .flat_map(|term| term.overlap_genes.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(40)
        .collect::<Vec<_>>();
    let rows = terms.len().max(genes.len()).max(1);
    let height = 120_u32.saturating_add((rows as u32).saturating_mul(34));
    let term_x = 290.0_f64;
    let gene_x = f64::from(width) - 250.0;
    let mut svg = svg_header(width, height, "Enrichment term-gene network");
    push_text(
        &mut svg,
        24.0,
        32.0,
        20,
        "#18332b",
        "Enrichment term-gene network",
    );
    let term_positions = terms
        .iter()
        .enumerate()
        .map(|(index, term)| (term.term_id.as_str(), 82.0 + index as f64 * 34.0))
        .collect::<BTreeMap<_, _>>();
    let gene_positions = genes
        .iter()
        .enumerate()
        .map(|(index, gene)| (gene.as_str(), 82.0 + index as f64 * 34.0))
        .collect::<BTreeMap<_, _>>();
    let mut edge_count = 0_u64;
    for term in &terms {
        let Some(term_y) = term_positions.get(term.term_id.as_str()) else {
            continue;
        };
        for gene in &term.overlap_genes {
            if let Some(gene_y) = gene_positions.get(gene.as_str()) {
                edge_count = edge_count.saturating_add(1);
                svg.push_str(&format!(
                    "<line x1=\"{term_x:.1}\" y1=\"{term_y:.1}\" x2=\"{gene_x:.1}\" y2=\"{gene_y:.1}\" stroke=\"#b9cbc5\" stroke-width=\"1\"/>"
                ));
            }
        }
    }
    for term in &terms {
        let y = term_positions[term.term_id.as_str()];
        svg.push_str(&format!(
            "<circle cx=\"{term_x:.1}\" cy=\"{y:.1}\" r=\"8\" fill=\"{}\"/>",
            color_for(term.namespace.as_deref().unwrap_or("term"))
        ));
        push_text(
            &mut svg,
            24.0,
            y + 5.0,
            12,
            "#263d36",
            &truncate_label(&term_label(term), 38),
        );
    }
    for gene in &genes {
        let y = gene_positions[gene.as_str()];
        svg.push_str(&format!(
            "<circle cx=\"{gene_x:.1}\" cy=\"{y:.1}\" r=\"5\" fill=\"#d99233\"/>"
        ));
        push_text(
            &mut svg,
            gene_x + 14.0,
            y + 5.0,
            12,
            "#263d36",
            &truncate_label(gene, 28),
        );
    }
    svg.push_str("</svg>");
    (
        svg,
        height,
        edge_count
            .saturating_add(terms.len() as u64)
            .saturating_add(genes.len() as u64),
    )
}

fn term_label(term: &crate::functional::EnrichmentTerm) -> String {
    term.term_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!("{name} ({})", term.term_id))
        .unwrap_or_else(|| term.term_id.clone())
}

fn negative_log10(value: f64) -> f64 {
    if !value.is_finite() || value < 0.0 {
        return 0.0;
    }
    -value.clamp(1e-300, 1.0).log10()
}

fn validate_item_limit(value: usize, name: &str) -> Result<(), VisualizationError> {
    if value == 0 || value > MAX_VISUAL_ITEMS {
        return Err(VisualizationError::InvalidOption(format!(
            "{name} must be between 1 and {MAX_VISUAL_ITEMS}"
        )));
    }
    Ok(())
}

pub(crate) fn write_new_output(path: &Path, bytes: &[u8]) -> Result<(), VisualizationError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                VisualizationError::OutputAlreadyExists(path.to_path_buf())
            } else {
                VisualizationError::Io(error)
            }
        })?;
    let mut writer = BufWriter::new(file);
    if let Err(error) = writer.write_all(bytes).and_then(|()| writer.flush()) {
        drop(writer);
        let _ = fs::remove_file(path);
        return Err(VisualizationError::Io(error));
    }
    Ok(())
}

fn svg_header(width: u32, height: u32, title: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><title>{}</title><rect width=\"100%\" height=\"100%\" fill=\"#fbfdfc\"/>",
        xml_escape(title)
    )
}

fn push_text(svg: &mut String, x: f64, y: f64, size: u32, color: &str, value: &str) {
    svg.push_str(&format!(
        "<text x=\"{x:.1}\" y=\"{y:.1}\" font-family=\"sans-serif\" font-size=\"{size}\" fill=\"{color}\">{}</text>",
        xml_escape(value)
    ));
}

fn truncate_label(value: &str, maximum_chars: usize) -> String {
    let mut chars = value.chars();
    let mut truncated = chars.by_ref().take(maximum_chars).collect::<String>();
    if chars.next().is_some() {
        truncated.push('…');
    }
    truncated
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            character if character.is_control() && !matches!(character, '\t' | '\n' | '\r') => {}
            character => escaped.push(character),
        }
    }
    escaped
}

fn color_for(value: &str) -> &'static str {
    const COLORS: [&str; 10] = [
        "#2f68a5", "#277c63", "#c27e22", "#b1433f", "#2a8f9d", "#7955a3", "#b35d8a", "#6d8034",
        "#87633b", "#4f6f9d",
    ];
    let hash = value.bytes().fold(2_166_136_261_u32, |hash, byte| {
        hash.wrapping_mul(16_777_619) ^ u32::from(byte)
    });
    COLORS[hash as usize % COLORS.len()]
}

#[cfg(test)]
mod tests {
    use super::{
        AnnotationStructureOptions, DomainArchitectureOptions, EnrichmentPlotStyle,
        EnrichmentVisualizationOptions, render_annotation_structure_svg_path,
        render_domain_architecture_svg_path, render_enrichment_svg_path,
        render_motif_logo_svg_path, render_volcano_svg_path,
    };
    use crate::functional::{EnrichmentKind, EnrichmentOptions};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn renders_annotation_domain_and_all_enrichment_styles() {
        let root = fixture_root();
        let temporary = temporary_directory();
        let annotation_output = temporary.join("annotation.svg");
        let annotation = render_annotation_structure_svg_path(
            root.join("tests/fixtures/annotation/genes.gff3"),
            &annotation_output,
            &AnnotationStructureOptions::default(),
        )
        .expect("render annotation structure");
        assert!(annotation.glyph_count > 0);
        assert!(
            fs::read_to_string(&annotation_output)
                .expect("annotation SVG")
                .contains("<svg")
        );

        let domain_output = temporary.join("domains.svg");
        let domains = render_domain_architecture_svg_path(
            root.join("tests/fixtures/protein-domains/interproscan.tsv"),
            &domain_output,
            &DomainArchitectureOptions::default(),
        )
        .expect("render protein domains");
        assert!(domains.glyph_count > 0);

        for style in [
            EnrichmentPlotStyle::Bar,
            EnrichmentPlotStyle::Dot,
            EnrichmentPlotStyle::Network,
        ] {
            let output = temporary.join(format!("enrichment-{}.svg", style.as_str()));
            let result = render_enrichment_svg_path(
                root.join("tests/fixtures/functional/genes.txt"),
                root.join("tests/fixtures/functional/associations.tsv"),
                &output,
                EnrichmentKind::Go,
                EnrichmentOptions::default(),
                EnrichmentVisualizationOptions {
                    style,
                    max_terms: 10,
                },
            )
            .expect("render enrichment plot");
            assert!(result.glyph_count > 0);
            assert!(
                fs::read_to_string(output)
                    .expect("enrichment SVG")
                    .contains("</svg>")
            );
        }

        fs::remove_dir_all(temporary).expect("remove visualization directory");
    }

    #[test]
    fn renders_volcano_plot_from_differential_expression_table() {
        let temporary = temporary_directory();
        let input = temporary.join("differential.csv");
        let output = temporary.join("volcano.svg");
        fs::write(
            &input,
            "gene,log2FoldChange,padj\nup,2.0,0.001\ndown,-1.5,0.01\nneutral,0.1,0.8\n",
        )
        .expect("write differential table");
        let result = render_volcano_svg_path(&input, &output, &Default::default())
            .expect("render volcano plot");
        assert_eq!(result.glyph_count, 3);
        let svg = fs::read_to_string(&output).expect("read volcano SVG");
        assert!(svg.contains("#b1433f"));
        assert!(svg.contains("#2f68a5"));
        fs::remove_dir_all(temporary).expect("remove visualization directory");
    }

    #[test]
    fn renders_sequence_logo_from_meme_matrix() {
        let temporary = temporary_directory();
        let output = temporary.join("motif.svg");
        let result = render_motif_logo_svg_path(
            fixture_root().join("tests/fixtures/motifs/tiny.meme"),
            &output,
        )
        .expect("render motif logo");
        assert_eq!(result.glyph_count, 8);
        assert!(
            fs::read_to_string(&output)
                .expect("read SVG")
                .contains(">A</text>")
        );
        fs::remove_dir_all(temporary).expect("remove visualization directory");
    }

    #[test]
    fn renders_synteny_anchor_table_as_svg() {
        let temporary = temporary_directory();
        let input = fixture_root().join("tests/fixtures/comparative/synteny-anchors.tsv");
        for (style, expected_title) in [
            (super::SyntenyPlotStyle::Dual, "Synteny anchors"),
            (super::SyntenyPlotStyle::Micro, "Micro-synteny anchors"),
            (
                super::SyntenyPlotStyle::Multiple,
                "Multiple synteny anchors",
            ),
            (
                super::SyntenyPlotStyle::Circular,
                "Circular synteny anchors",
            ),
        ] {
            let output = temporary.join(format!("synteny-{}.svg", style.as_str()));
            let result = super::render_synteny_svg_with_options_path(
                &input,
                &output,
                &super::SyntenyVisualizationOptions { style },
            )
            .expect("render synteny");
            assert_eq!(result.glyph_count, 3);
            let svg = fs::read_to_string(&output).expect("synteny SVG");
            assert!(svg.contains(expected_title));
            assert!(svg.contains("<path"));
        }
        fs::remove_dir_all(temporary).expect("remove visualization directory");
    }

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn temporary_directory() -> PathBuf {
        let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "linxira-scientific-visualization-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create visualization directory");
        path
    }
}
