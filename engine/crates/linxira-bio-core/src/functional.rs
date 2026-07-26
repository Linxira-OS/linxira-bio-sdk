use csv::{ReaderBuilder, StringRecord};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

pub const MAX_FUNCTIONAL_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_FUNCTIONAL_ROWS: u64 = 2_000_000;
pub const MAX_FUNCTIONAL_IDENTIFIERS: usize = 2_000_000;
pub const MAX_REPORTED_ENRICHMENT_TERMS: usize = 10_000;
const MAX_TERMS_PER_CELL: usize = 10_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoAnnotationOptions {
    pub gene_column: Option<String>,
    pub go_column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionalAssociation {
    pub gene_id: String,
    pub term_id: String,
    pub term_name: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnnotationMapResult {
    pub input_row_count: u64,
    pub gene_count: u64,
    pub term_count: u64,
    pub association_count: u64,
    pub output_path: String,
    pub associations: Vec<FunctionalAssociation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EggnogAnnotationRecord {
    pub query_id: String,
    pub seed_ortholog: Option<String>,
    pub evalue: Option<f64>,
    pub score: Option<f64>,
    pub orthologous_groups: Vec<String>,
    pub annotation_level: Option<String>,
    pub cog_categories: Vec<String>,
    pub description: Option<String>,
    pub preferred_name: Option<String>,
    pub go_terms: Vec<String>,
    pub ec_numbers: Vec<String>,
    pub kegg_orthologs: Vec<String>,
    pub kegg_pathways: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EggnogNormalizeResult {
    pub input_row_count: u64,
    pub query_count: u64,
    pub go_association_count: u64,
    pub kegg_association_count: u64,
    pub output_path: String,
    pub records: Vec<EggnogAnnotationRecord>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrichmentKind {
    Custom,
    Go,
    Kegg,
}

impl EnrichmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Custom => "custom",
            Self::Go => "go",
            Self::Kegg => "kegg",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrichmentOptions {
    pub min_overlap: u64,
    pub max_terms: usize,
    pub include_genes: bool,
}

impl Default for EnrichmentOptions {
    fn default() -> Self {
        Self {
            min_overlap: 1,
            max_terms: 100,
            include_genes: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnrichmentTerm {
    pub term_id: String,
    pub term_name: Option<String>,
    pub namespace: Option<String>,
    pub overlap_count: u64,
    pub query_gene_count: u64,
    pub background_term_count: u64,
    pub background_gene_count: u64,
    pub fold_enrichment: f64,
    pub p_value: f64,
    pub adjusted_p_value: f64,
    pub overlap_genes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnrichmentResult {
    pub analysis_type: String,
    pub query_input_count: u64,
    pub query_mapped_count: u64,
    pub query_unmapped_count: u64,
    pub background_gene_count: u64,
    pub tested_term_count: u64,
    pub reported_term_count: u64,
    pub omitted_term_count: u64,
    pub terms: Vec<EnrichmentTerm>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum FunctionalError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidUtf8,
    InvalidTable(String),
    InvalidOption(String),
    LimitExceeded { resource: &'static str, limit: u64 },
}

impl Display for FunctionalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "functional-analysis I/O failed: {error}"),
            Self::Csv(error) => write!(formatter, "functional table parsing failed: {error}"),
            Self::InvalidUtf8 => formatter.write_str("functional input is not valid UTF-8 text"),
            Self::InvalidTable(message) => write!(formatter, "invalid functional table: {message}"),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "functional analysis exceeds the deterministic {resource} limit of {limit}"
            ),
        }
    }
}

impl Error for FunctionalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            Self::InvalidUtf8
            | Self::InvalidTable(_)
            | Self::InvalidOption(_)
            | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<io::Error> for FunctionalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for FunctionalError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub fn normalize_go_annotations_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &GoAnnotationOptions,
) -> Result<AnnotationMapResult, FunctionalError> {
    let input = input.as_ref();
    let output = output.as_ref();
    let text = read_bounded_text(input)?;
    let delimiter = infer_delimiter(input, &text);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(false)
        .from_reader(text.as_bytes());
    let headers = reader.headers()?.clone();
    let gene_column = resolve_column(
        &headers,
        options.gene_column.as_deref(),
        &["gene_id", "gene", "query", "query_id", "protein_id", "id"],
        "gene",
    )?;
    let go_column = resolve_column(
        &headers,
        options.go_column.as_deref(),
        &["go", "gos", "go_terms", "go_term", "gene_ontology"],
        "GO",
    )?;
    if gene_column == go_column {
        return Err(FunctionalError::InvalidTable(
            "gene and GO columns must be different".to_owned(),
        ));
    }

    let mut input_row_count = 0_u64;
    let mut associations = BTreeMap::<(String, String), FunctionalAssociation>::new();
    let mut rows_without_go = 0_u64;
    for record in reader.records() {
        input_row_count += 1;
        if input_row_count > MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let record = record?;
        let gene_id = record.get(gene_column).unwrap_or_default().trim();
        if gene_id.is_empty() || is_missing_value(gene_id) {
            continue;
        }
        let terms = split_multi_value(record.get(go_column).unwrap_or_default());
        if terms.is_empty() {
            rows_without_go += 1;
            continue;
        }
        for term in terms {
            if !is_go_id(&term) {
                continue;
            }
            if associations.len() >= MAX_FUNCTIONAL_IDENTIFIERS {
                return Err(FunctionalError::LimitExceeded {
                    resource: "GO association count",
                    limit: MAX_FUNCTIONAL_IDENTIFIERS as u64,
                });
            }
            associations
                .entry((gene_id.to_owned(), term.clone()))
                .or_insert(FunctionalAssociation {
                    gene_id: gene_id.to_owned(),
                    term_id: term,
                    term_name: None,
                    namespace: Some("go".to_owned()),
                });
        }
    }
    if associations.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "no valid GO identifiers were found".to_owned(),
        ));
    }
    let associations = associations.into_values().collect::<Vec<_>>();
    let genes = associations
        .iter()
        .map(|association| association.gene_id.as_str())
        .collect::<BTreeSet<_>>();
    let terms = associations
        .iter()
        .map(|association| association.term_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut warnings = Vec::new();
    if rows_without_go > 0 {
        warnings.push(format!(
            "{rows_without_go} input rows contained no GO identifiers"
        ));
    }
    write_associations(output, &associations)?;
    Ok(AnnotationMapResult {
        input_row_count,
        gene_count: genes.len() as u64,
        term_count: terms.len() as u64,
        association_count: associations.len() as u64,
        output_path: output.display().to_string(),
        associations,
        warnings,
    })
}

pub fn normalize_eggnog_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<EggnogNormalizeResult, FunctionalError> {
    let input = input.as_ref();
    let output = output.as_ref();
    let text = read_bounded_text(input)?;
    let lines = text.lines().collect::<Vec<_>>();
    let header_index = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with("#query\t"))
        .or_else(|| {
            lines.iter().position(|line| {
                line.split('\t')
                    .next()
                    .is_some_and(|field| field.trim().eq_ignore_ascii_case("query"))
            })
        })
        .ok_or_else(|| {
            FunctionalError::InvalidTable(
                "eggNOG-mapper input requires a #query tabular header".to_owned(),
            )
        })?;
    let header_line = lines[header_index].trim_start_matches('#');
    let headers = StringRecord::from(header_line.split('\t').collect::<Vec<_>>());
    let query_column = resolve_column(&headers, None, &["query", "query_name"], "query")?;
    let columns = EggnogColumns {
        query: query_column,
        seed_ortholog: optional_column(&headers, &["seed_ortholog"]),
        evalue: optional_column(&headers, &["evalue"]),
        score: optional_column(&headers, &["score"]),
        orthologous_groups: optional_column(&headers, &["eggnog_ogs", "eggnog_ogs"]),
        annotation_level: optional_column(&headers, &["max_annot_lvl", "max_annot_level"]),
        cog_categories: optional_column(&headers, &["cog_category"]),
        description: optional_column(&headers, &["description"]),
        preferred_name: optional_column(&headers, &["preferred_name"]),
        go_terms: optional_column(&headers, &["gos", "go_terms"]),
        ec_numbers: optional_column(&headers, &["ec"]),
        kegg_orthologs: optional_column(&headers, &["kegg_ko"]),
        kegg_pathways: optional_column(&headers, &["kegg_pathway"]),
    };
    let data = lines
        .iter()
        .skip(header_index + 1)
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(false)
        .from_reader(data.as_bytes());
    let mut records = Vec::new();
    for record in reader.records() {
        if records.len() as u64 >= MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let record = record?;
        if record.len() != headers.len() {
            return Err(FunctionalError::InvalidTable(format!(
                "eggNOG row {} has {} columns but the header has {}",
                records.len() + 1,
                record.len(),
                headers.len()
            )));
        }
        let query_id = record.get(columns.query).unwrap_or_default().trim();
        if query_id.is_empty() || is_missing_value(query_id) {
            continue;
        }
        records.push(EggnogAnnotationRecord {
            query_id: query_id.to_owned(),
            seed_ortholog: optional_value(&record, columns.seed_ortholog),
            evalue: optional_numeric(&record, columns.evalue, "evalue")?,
            score: optional_numeric(&record, columns.score, "score")?,
            orthologous_groups: multi_value_at(&record, columns.orthologous_groups),
            annotation_level: optional_value(&record, columns.annotation_level),
            cog_categories: character_categories_at(&record, columns.cog_categories),
            description: optional_value(&record, columns.description),
            preferred_name: optional_value(&record, columns.preferred_name),
            go_terms: multi_value_at(&record, columns.go_terms)
                .into_iter()
                .filter(|term| is_go_id(term))
                .collect(),
            ec_numbers: multi_value_at(&record, columns.ec_numbers),
            kegg_orthologs: multi_value_at(&record, columns.kegg_orthologs),
            kegg_pathways: multi_value_at(&record, columns.kegg_pathways),
        });
    }
    if records.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "eggNOG-mapper table contains no annotation rows".to_owned(),
        ));
    }
    records.sort_by(|left, right| left.query_id.cmp(&right.query_id));
    let query_count = records
        .iter()
        .map(|record| record.query_id.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let go_association_count = records
        .iter()
        .map(|record| record.go_terms.len() as u64)
        .sum();
    let kegg_association_count = records
        .iter()
        .map(|record| (record.kegg_orthologs.len() + record.kegg_pathways.len()) as u64)
        .sum();
    let warnings = if records.iter().all(|record| {
        record.go_terms.is_empty()
            && record.kegg_orthologs.is_empty()
            && record.kegg_pathways.is_empty()
    }) {
        vec!["no GO or KEGG assignments were present in the normalized rows".to_owned()]
    } else {
        Vec::new()
    };
    write_eggnog(output, &records)?;
    Ok(EggnogNormalizeResult {
        input_row_count: records.len() as u64,
        query_count,
        go_association_count,
        kegg_association_count,
        output_path: output.display().to_string(),
        records,
        warnings,
    })
}

pub fn overrepresentation_path(
    genes: impl AsRef<Path>,
    associations: impl AsRef<Path>,
    kind: EnrichmentKind,
    options: EnrichmentOptions,
) -> Result<EnrichmentResult, FunctionalError> {
    validate_enrichment_options(options)?;
    let query = read_gene_set(genes.as_ref())?;
    let terms = read_association_table(associations.as_ref(), kind)?;
    let background = terms
        .values()
        .flat_map(|term| term.genes.iter().cloned())
        .collect::<BTreeSet<_>>();
    if background.is_empty() {
        return Err(FunctionalError::InvalidTable(format!(
            "no {} associations were found",
            kind.as_str()
        )));
    }
    let mapped_query = query
        .intersection(&background)
        .cloned()
        .collect::<BTreeSet<_>>();
    if mapped_query.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "none of the query identifiers occur in the association universe".to_owned(),
        ));
    }

    let population = background.len() as u64;
    let sample = mapped_query.len() as u64;
    let log_factorials = log_factorials(population as usize);
    let mut tested = Vec::new();
    for (term_id, term) in terms {
        let overlap = term
            .genes
            .intersection(&mapped_query)
            .cloned()
            .collect::<Vec<_>>();
        if (overlap.len() as u64) < options.min_overlap {
            continue;
        }
        let term_count = term.genes.len() as u64;
        let overlap_count = overlap.len() as u64;
        let p_value = hypergeometric_upper_tail(
            population,
            term_count,
            sample,
            overlap_count,
            &log_factorials,
        );
        let fold_enrichment =
            (overlap_count as f64 / sample as f64) / (term_count as f64 / population as f64);
        tested.push(EnrichmentTerm {
            term_id,
            term_name: term.name,
            namespace: term.namespace,
            overlap_count,
            query_gene_count: sample,
            background_term_count: term_count,
            background_gene_count: population,
            fold_enrichment,
            p_value,
            adjusted_p_value: 1.0,
            overlap_genes: if options.include_genes {
                overlap
            } else {
                Vec::new()
            },
        });
    }
    if tested.is_empty() {
        return Err(FunctionalError::InvalidTable(format!(
            "no terms meet min_overlap {}",
            options.min_overlap
        )));
    }
    adjust_benjamini_hochberg(&mut tested);
    tested.sort_by(|left, right| {
        left.adjusted_p_value
            .total_cmp(&right.adjusted_p_value)
            .then_with(|| left.p_value.total_cmp(&right.p_value))
            .then_with(|| right.overlap_count.cmp(&left.overlap_count))
            .then_with(|| left.term_id.cmp(&right.term_id))
    });
    let tested_term_count = tested.len();
    tested.truncate(options.max_terms);
    let mut warnings = Vec::new();
    let unmapped = query.len().saturating_sub(mapped_query.len());
    if unmapped > 0 {
        warnings.push(format!(
            "{unmapped} query identifiers were absent from the association universe"
        ));
    }
    Ok(EnrichmentResult {
        analysis_type: kind.as_str().to_owned(),
        query_input_count: query.len() as u64,
        query_mapped_count: mapped_query.len() as u64,
        query_unmapped_count: unmapped as u64,
        background_gene_count: population,
        tested_term_count: tested_term_count as u64,
        reported_term_count: tested.len() as u64,
        omitted_term_count: tested_term_count.saturating_sub(tested.len()) as u64,
        terms: tested,
        warnings,
    })
}

#[derive(Debug)]
struct EggnogColumns {
    query: usize,
    seed_ortholog: Option<usize>,
    evalue: Option<usize>,
    score: Option<usize>,
    orthologous_groups: Option<usize>,
    annotation_level: Option<usize>,
    cog_categories: Option<usize>,
    description: Option<usize>,
    preferred_name: Option<usize>,
    go_terms: Option<usize>,
    ec_numbers: Option<usize>,
    kegg_orthologs: Option<usize>,
    kegg_pathways: Option<usize>,
}

#[derive(Debug)]
struct TermGenes {
    name: Option<String>,
    namespace: Option<String>,
    genes: BTreeSet<String>,
}

fn validate_enrichment_options(options: EnrichmentOptions) -> Result<(), FunctionalError> {
    if options.min_overlap == 0 {
        return Err(FunctionalError::InvalidOption(
            "min_overlap must be greater than zero".to_owned(),
        ));
    }
    if !(1..=MAX_REPORTED_ENRICHMENT_TERMS).contains(&options.max_terms) {
        return Err(FunctionalError::InvalidOption(format!(
            "max_terms must be between 1 and {MAX_REPORTED_ENRICHMENT_TERMS}"
        )));
    }
    Ok(())
}

fn read_association_table(
    path: &Path,
    kind: EnrichmentKind,
) -> Result<BTreeMap<String, TermGenes>, FunctionalError> {
    let text = read_bounded_text(path)?;
    let delimiter = infer_delimiter(path, &text);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(false)
        .from_reader(text.as_bytes());
    let headers = reader.headers()?.clone();
    let gene_column = resolve_column(
        &headers,
        None,
        &["gene_id", "gene", "query", "query_id", "protein_id", "id"],
        "gene",
    )?;
    let term_column = resolve_column(
        &headers,
        None,
        &["term_id", "term", "go_id", "pathway_id", "category_id"],
        "term",
    )?;
    let name_column = optional_column(&headers, &["term_name", "name", "description"]);
    let namespace_column = optional_column(&headers, &["namespace", "category", "source"]);
    let mut terms = BTreeMap::<String, TermGenes>::new();
    let mut row_count = 0_u64;
    for record in reader.records() {
        row_count += 1;
        if row_count > MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "association row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let record = record?;
        let gene = record.get(gene_column).unwrap_or_default().trim();
        let term_id = record.get(term_column).unwrap_or_default().trim();
        if gene.is_empty()
            || term_id.is_empty()
            || is_missing_value(gene)
            || is_missing_value(term_id)
        {
            continue;
        }
        let namespace = optional_value(&record, namespace_column);
        if !term_matches_kind(term_id, namespace.as_deref(), kind) {
            continue;
        }
        let term = terms
            .entry(term_id.to_owned())
            .or_insert_with(|| TermGenes {
                name: optional_value(&record, name_column),
                namespace: namespace.clone(),
                genes: BTreeSet::new(),
            });
        if term.name.is_none() {
            term.name = optional_value(&record, name_column);
        }
        if term.namespace.is_none() {
            term.namespace = namespace;
        }
        term.genes.insert(gene.to_owned());
    }
    Ok(terms)
}

fn term_matches_kind(term_id: &str, namespace: Option<&str>, kind: EnrichmentKind) -> bool {
    match kind {
        EnrichmentKind::Custom => true,
        EnrichmentKind::Go => is_go_id(term_id),
        EnrichmentKind::Kegg => {
            namespace.is_some_and(|value| value.eq_ignore_ascii_case("kegg"))
                || term_id.to_ascii_lowercase().starts_with("path:")
                || term_id.to_ascii_lowercase().starts_with("map")
                || term_id.to_ascii_lowercase().starts_with("ko")
        }
    }
}

fn read_gene_set(path: &Path) -> Result<BTreeSet<String>, FunctionalError> {
    let text = read_bounded_text(path)?;
    let nonempty = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>();
    if nonempty.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "query gene list is empty".to_owned(),
        ));
    }
    let delimiter = if nonempty[0].contains('\t') {
        Some('\t')
    } else if nonempty[0].contains(',') {
        Some(',')
    } else {
        None
    };
    let mut genes = BTreeSet::new();
    for (index, line) in nonempty.into_iter().enumerate() {
        if index as u64 >= MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "query row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let value = delimiter
            .and_then(|delimiter| line.split(delimiter).next())
            .unwrap_or(line)
            .trim();
        if index == 0 && is_gene_header(value) {
            continue;
        }
        if value.is_empty() || is_missing_value(value) {
            continue;
        }
        if genes.len() >= MAX_FUNCTIONAL_IDENTIFIERS {
            return Err(FunctionalError::LimitExceeded {
                resource: "query identifier count",
                limit: MAX_FUNCTIONAL_IDENTIFIERS as u64,
            });
        }
        genes.insert(value.to_owned());
    }
    if genes.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "query gene list contains no identifiers".to_owned(),
        ));
    }
    Ok(genes)
}

fn adjust_benjamini_hochberg(terms: &mut [EnrichmentTerm]) {
    let mut order = (0..terms.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        terms[*left]
            .p_value
            .total_cmp(&terms[*right].p_value)
            .then_with(|| terms[*left].term_id.cmp(&terms[*right].term_id))
    });
    let count = order.len() as f64;
    let mut running = 1.0_f64;
    for (reverse_index, term_index) in order.into_iter().enumerate().rev() {
        let rank = reverse_index as f64 + 1.0;
        let adjusted = (terms[term_index].p_value * count / rank).min(1.0);
        running = running.min(adjusted);
        terms[term_index].adjusted_p_value = running;
    }
}

fn hypergeometric_upper_tail(
    population: u64,
    successes: u64,
    draws: u64,
    observed: u64,
    log_factorials: &[f64],
) -> f64 {
    let upper = successes.min(draws);
    if observed > upper {
        return 0.0;
    }
    let log_probability = ln_choose(successes, observed, log_factorials)
        + ln_choose(population - successes, draws - observed, log_factorials)
        - ln_choose(population, draws, log_factorials);
    let mut probability = log_probability.exp();
    let mut sum = probability;
    let mut current = observed;
    while current < upper {
        let numerator = (successes - current) as f64 * (draws - current) as f64;
        let remaining_failures = population - successes;
        let sampled_failures = draws - current;
        let denominator = (current + 1) as f64 * (remaining_failures - sampled_failures + 1) as f64;
        if denominator <= 0.0 {
            break;
        }
        probability *= numerator / denominator;
        sum += probability;
        current += 1;
    }
    sum.clamp(0.0, 1.0)
}

fn log_factorials(limit: usize) -> Vec<f64> {
    let mut values = Vec::with_capacity(limit + 1);
    values.push(0.0);
    for value in 1..=limit {
        values.push(values[value - 1] + (value as f64).ln());
    }
    values
}

fn ln_choose(total: u64, selected: u64, log_factorials: &[f64]) -> f64 {
    if selected > total {
        return f64::NEG_INFINITY;
    }
    log_factorials[total as usize]
        - log_factorials[selected as usize]
        - log_factorials[(total - selected) as usize]
}

fn read_bounded_text(path: &Path) -> Result<String, FunctionalError> {
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let mut reader: Box<dyn Read> = if prefix_length == prefix.len() && prefix == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_FUNCTIONAL_DECOMPRESSED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FUNCTIONAL_DECOMPRESSED_BYTES {
        return Err(FunctionalError::LimitExceeded {
            resource: "decompressed byte count",
            limit: MAX_FUNCTIONAL_DECOMPRESSED_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| FunctionalError::InvalidUtf8)
}

fn infer_delimiter(path: &Path, text: &str) -> u8 {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.ends_with(".csv") || name.ends_with(".csv.gz") {
        return b',';
    }
    if name.ends_with(".tsv") || name.ends_with(".tab") || name.ends_with(".tsv.gz") {
        return b'\t';
    }
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .unwrap_or_default();
    if first.matches('\t').count() >= first.matches(',').count() {
        b'\t'
    } else {
        b','
    }
}

fn resolve_column(
    headers: &StringRecord,
    requested: Option<&str>,
    aliases: &[&str],
    label: &str,
) -> Result<usize, FunctionalError> {
    if let Some(requested) = requested {
        return headers
            .iter()
            .position(|header| header.trim().eq_ignore_ascii_case(requested.trim()))
            .ok_or_else(|| {
                FunctionalError::InvalidTable(format!(
                    "requested {label} column {requested:?} was not found"
                ))
            });
    }
    optional_column(headers, aliases).ok_or_else(|| {
        FunctionalError::InvalidTable(format!(
            "could not infer the {label} column; expected one of {}",
            aliases.join(", ")
        ))
    })
}

fn optional_column(headers: &StringRecord, aliases: &[&str]) -> Option<usize> {
    headers.iter().position(|header| {
        let normalized = normalize_header(header);
        aliases
            .iter()
            .any(|alias| normalized == normalize_header(alias))
    })
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .to_ascii_lowercase()
        .replace([' ', '-', '.'], "_")
}

fn split_multi_value(value: &str) -> Vec<String> {
    let mut values = value
        .split([',', ';', '|'])
        .map(str::trim)
        .filter(|value| !value.is_empty() && !is_missing_value(value))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values.truncate(MAX_TERMS_PER_CELL);
    values
}

fn character_categories_at(record: &StringRecord, column: Option<usize>) -> Vec<String> {
    let Some(value) = column.and_then(|column| record.get(column)) else {
        return Vec::new();
    };
    if is_missing_value(value.trim()) {
        return Vec::new();
    }
    let mut categories = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_string())
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();
    categories
}

fn multi_value_at(record: &StringRecord, column: Option<usize>) -> Vec<String> {
    column
        .and_then(|column| record.get(column))
        .map(split_multi_value)
        .unwrap_or_default()
}

fn optional_value(record: &StringRecord, column: Option<usize>) -> Option<String> {
    let value = record.get(column?)?.trim();
    (!value.is_empty() && !is_missing_value(value)).then(|| value.to_owned())
}

fn optional_numeric(
    record: &StringRecord,
    column: Option<usize>,
    label: &str,
) -> Result<Option<f64>, FunctionalError> {
    let Some(value) = optional_value(record, column) else {
        return Ok(None);
    };
    let parsed = value.parse::<f64>().map_err(|_| {
        FunctionalError::InvalidTable(format!("eggNOG {label} value {value:?} is not numeric"))
    })?;
    if !parsed.is_finite() {
        return Err(FunctionalError::InvalidTable(format!(
            "eggNOG {label} must be finite"
        )));
    }
    Ok(Some(parsed))
}

fn is_missing_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "-" | "." | "na" | "n/a" | "none" | "null"
    )
}

fn is_go_id(value: &str) -> bool {
    value
        .strip_prefix("GO:")
        .is_some_and(|digits| digits.len() == 7 && digits.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_gene_header(value: &str) -> bool {
    matches!(
        normalize_header(value).as_str(),
        "gene" | "gene_id" | "query" | "query_id" | "protein_id" | "id"
    )
}

fn write_associations(
    output: &Path,
    associations: &[FunctionalAssociation],
) -> Result<(), FunctionalError> {
    let mut text = String::from("gene_id\tterm_id\tterm_name\tnamespace\n");
    for association in associations {
        text.push_str(&tsv_cell(&association.gene_id));
        text.push('\t');
        text.push_str(&tsv_cell(&association.term_id));
        text.push('\t');
        text.push_str(&tsv_cell(
            association.term_name.as_deref().unwrap_or_default(),
        ));
        text.push('\t');
        text.push_str(&tsv_cell(
            association.namespace.as_deref().unwrap_or_default(),
        ));
        text.push('\n');
    }
    write_new_file(output, text.as_bytes())
}

fn write_eggnog(output: &Path, records: &[EggnogAnnotationRecord]) -> Result<(), FunctionalError> {
    let mut text = String::from(
        "query_id\tseed_ortholog\tevalue\tscore\torthologous_groups\tannotation_level\tcog_categories\tdescription\tpreferred_name\tgo_terms\tec_numbers\tkegg_orthologs\tkegg_pathways\n",
    );
    for record in records {
        let values = [
            record.query_id.clone(),
            record.seed_ortholog.clone().unwrap_or_default(),
            record
                .evalue
                .map(|value| value.to_string())
                .unwrap_or_default(),
            record
                .score
                .map(|value| value.to_string())
                .unwrap_or_default(),
            record.orthologous_groups.join(","),
            record.annotation_level.clone().unwrap_or_default(),
            record.cog_categories.join(","),
            record.description.clone().unwrap_or_default(),
            record.preferred_name.clone().unwrap_or_default(),
            record.go_terms.join(","),
            record.ec_numbers.join(","),
            record.kegg_orthologs.join(","),
            record.kegg_pathways.join(","),
        ];
        text.push_str(
            &values
                .iter()
                .map(|value| tsv_cell(value))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        text.push('\n');
    }
    write_new_file(output, text.as_bytes())
}

fn tsv_cell(value: &str) -> String {
    value
        .replace('\t', " ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_owned()
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), FunctionalError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(FunctionalError::Io(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        EnrichmentKind, EnrichmentOptions, GoAnnotationOptions, normalize_eggnog_path,
        normalize_go_annotations_path, overrepresentation_path,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn normalizes_and_deduplicates_go_annotations() {
        let input = temporary("go-input.tsv");
        let output = temporary("go-output.tsv");
        fs::write(
            &input,
            "query\tGOs\nprotein1\tGO:0008150,GO:0003674\nprotein1\tGO:0008150\nprotein2\t-\n",
        )
        .expect("write GO input");
        let result =
            normalize_go_annotations_path(&input, &output, &GoAnnotationOptions::default())
                .expect("normalize GO annotations");
        assert_eq!(result.gene_count, 1);
        assert_eq!(result.term_count, 2);
        assert_eq!(result.association_count, 2);
        assert!(
            fs::read_to_string(&output)
                .expect("read output")
                .contains("GO:0008150")
        );
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
    }

    #[test]
    fn normalizes_standard_eggnog_mapper_columns() {
        let input = temporary("eggnog.emapper.annotations");
        let output = temporary("eggnog-output.tsv");
        fs::write(
            &input,
            "#query\tseed_ortholog\tevalue\tscore\teggNOG_OGs\tmax_annot_lvl\tCOG_category\tDescription\tPreferred_name\tGOs\tEC\tKEGG_ko\tKEGG_Pathway\nprotein1\tseed1\t1e-20\t80\tOG1@1\tBacteria\tCE\tExample enzyme\texa\tGO:0008150,GO:0003674\t1.1.1.1\tko:K00001\tmap00010\n",
        )
        .expect("write eggNOG input");
        let result = normalize_eggnog_path(&input, &output).expect("normalize eggNOG");
        assert_eq!(result.query_count, 1);
        assert_eq!(result.go_association_count, 2);
        assert_eq!(result.kegg_association_count, 2);
        assert_eq!(result.records[0].cog_categories, ["C", "E"]);
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
    }

    #[test]
    fn computes_hypergeometric_enrichment_and_filters_namespaces() {
        let genes = temporary("genes.txt");
        let associations = temporary("associations.tsv");
        fs::write(&genes, "gene_id\ng1\ng2\ng3\nmissing\n").expect("write genes");
        fs::write(
            &associations,
            "gene_id\tterm_id\tterm_name\tnamespace\n\
g1\tGO:0000001\tProcess A\tgo\n\
g2\tGO:0000001\tProcess A\tgo\n\
g4\tGO:0000001\tProcess A\tgo\n\
g3\tGO:0000002\tProcess B\tgo\n\
g5\tGO:0000002\tProcess B\tgo\n\
g1\tmap00010\tPathway A\tkegg\n\
g2\tmap00010\tPathway A\tkegg\n\
g6\tmap00010\tPathway A\tkegg\n\
g7\tcustom:1\tCustom A\tcustom\n\
g8\tcustom:1\tCustom A\tcustom\n",
        )
        .expect("write associations");
        let result = overrepresentation_path(
            &genes,
            &associations,
            EnrichmentKind::Go,
            EnrichmentOptions {
                include_genes: true,
                ..EnrichmentOptions::default()
            },
        )
        .expect("GO enrichment");
        assert_eq!(result.analysis_type, "go");
        assert_eq!(result.query_input_count, 4);
        assert_eq!(result.query_mapped_count, 3);
        assert_eq!(result.query_unmapped_count, 1);
        assert_eq!(result.terms.len(), 2);
        let process_a = result
            .terms
            .iter()
            .find(|term| term.term_id == "GO:0000001")
            .expect("Process A result");
        assert_eq!(process_a.overlap_count, 2);
        assert!(process_a.p_value > 0.0 && process_a.p_value <= 1.0);
        assert_eq!(process_a.overlap_genes, ["g1", "g2"]);

        let kegg = overrepresentation_path(
            &genes,
            &associations,
            EnrichmentKind::Kegg,
            EnrichmentOptions::default(),
        )
        .expect("KEGG enrichment");
        assert_eq!(kegg.terms.len(), 1);
        assert_eq!(kegg.terms[0].term_id, "map00010");
        fs::remove_file(genes).expect("remove genes");
        fs::remove_file(associations).expect("remove associations");
    }

    fn temporary(name: &str) -> PathBuf {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-functional-{}-{counter}-{name}",
            std::process::id()
        ))
    }
}
