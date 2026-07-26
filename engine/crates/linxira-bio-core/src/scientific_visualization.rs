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

fn write_new_output(path: &Path, bytes: &[u8]) -> Result<(), VisualizationError> {
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
