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
pub const MAX_GSEA_PERMUTATIONS: u32 = 100_000;
pub const MAX_GSEA_PERMUTATION_DRAWS: u64 = 250_000_000;
const MAX_TERMS_PER_CELL: usize = 10_000;
pub const GSEA_CAPABILITY_ID: &str = "enrichment.gsea.v1";

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct GseaOptions {
    pub score_exponent: f64,
    pub min_set_size: usize,
    pub max_set_size: usize,
    pub permutation_count: u32,
    pub seed: u64,
}

impl Default for GseaOptions {
    fn default() -> Self {
        Self {
            score_exponent: 1.0,
            min_set_size: 15,
            max_set_size: 500,
            permutation_count: 1_000,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GseaTermResult {
    pub term_id: String,
    pub term_name: Option<String>,
    pub namespace: Option<String>,
    pub input_gene_count: u64,
    pub mapped_gene_count: u64,
    pub enrichment_score: f64,
    pub direction: String,
    pub peak_rank: u64,
    pub leading_edge_gene_count: u64,
    pub leading_edge_genes: Vec<String>,
    pub nominal_p_value: f64,
    pub fdr_bh: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GseaResult {
    pub capability_id: String,
    pub schema_version: String,
    pub analysis_type: String,
    pub score_exponent: f64,
    pub permutation_count: u32,
    pub seed: u64,
    pub permutation_method: String,
    pub multiple_testing_method: String,
    pub ranked_gene_count: u64,
    pub input_gene_set_count: u64,
    pub tested_gene_set_count: u64,
    pub input_membership_count: u64,
    pub mapped_membership_count: u64,
    pub skipped_no_overlap_count: u64,
    pub skipped_below_min_size_count: u64,
    pub skipped_above_max_size_count: u64,
    pub skipped_full_universe_count: u64,
    pub terms: Vec<GseaTermResult>,
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

pub fn gsea_preranked_path(
    ranked_genes: impl AsRef<Path>,
    gene_sets: impl AsRef<Path>,
    options: GseaOptions,
) -> Result<GseaResult, FunctionalError> {
    validate_gsea_options(options)?;
    let ranked = read_ranked_genes(ranked_genes.as_ref(), options.score_exponent)?;
    let gene_sets = read_gsea_gene_sets(gene_sets.as_ref())?;
    let GseaGeneSetTable {
        terms: gene_set_terms,
        input_gene_set_count,
        input_membership_count,
        duplicate_membership_count,
    } = gene_sets;
    let ranked_index = ranked
        .genes
        .iter()
        .enumerate()
        .map(|(index, gene)| (gene.gene_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let ranked_weights = ranked
        .genes
        .iter()
        .map(|gene| gene.weight)
        .collect::<Vec<_>>();
    let weight_prefix = prefix_sums(&ranked_weights);

    let mut skipped_no_overlap_count = 0_u64;
    let mut skipped_below_min_size_count = 0_u64;
    let mut skipped_above_max_size_count = 0_u64;
    let mut skipped_full_universe_count = 0_u64;
    let mut mapped_membership_count = 0_u64;
    let mut zero_weight_set_count = 0_u64;
    let mut tested = Vec::new();
    let mut permutation_draws = 0_u64;

    for (term_id, term) in gene_set_terms {
        let mut positions = term
            .genes
            .iter()
            .filter_map(|gene| ranked_index.get(gene.as_str()).copied())
            .collect::<Vec<_>>();
        positions.sort_unstable();
        let mapped_size = positions.len();
        mapped_membership_count = mapped_membership_count
            .checked_add(mapped_size as u64)
            .ok_or(FunctionalError::LimitExceeded {
                resource: "mapped GSEA membership count",
                limit: u64::MAX,
            })?;
        if mapped_size == 0 {
            skipped_no_overlap_count += 1;
            continue;
        }
        if mapped_size < options.min_set_size {
            skipped_below_min_size_count += 1;
            continue;
        }
        if mapped_size > options.max_set_size {
            skipped_above_max_size_count += 1;
            continue;
        }
        if mapped_size == ranked.genes.len() {
            skipped_full_universe_count += 1;
            continue;
        }
        if tested.len() >= MAX_REPORTED_ENRICHMENT_TERMS {
            return Err(FunctionalError::LimitExceeded {
                resource: "tested GSEA gene-set count",
                limit: MAX_REPORTED_ENRICHMENT_TERMS as u64,
            });
        }
        let sampled_permutation_indices = mapped_size.min(ranked.genes.len() - mapped_size);
        let term_draws = (sampled_permutation_indices as u64)
            .checked_mul(options.permutation_count as u64)
            .ok_or(FunctionalError::LimitExceeded {
                resource: "GSEA permutation sampled-index count",
                limit: MAX_GSEA_PERMUTATION_DRAWS,
            })?;
        permutation_draws =
            permutation_draws
                .checked_add(term_draws)
                .ok_or(FunctionalError::LimitExceeded {
                    resource: "GSEA permutation sampled-index count",
                    limit: MAX_GSEA_PERMUTATION_DRAWS,
                })?;
        if permutation_draws > MAX_GSEA_PERMUTATION_DRAWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "GSEA permutation sampled-index count",
                limit: MAX_GSEA_PERMUTATION_DRAWS,
            });
        }

        let trace = enrichment_score_from_hits(&ranked_weights, &positions);
        if trace.used_unweighted_fallback {
            zero_weight_set_count += 1;
        }
        let nominal_p_value = gsea_permutation_p_value(
            &ranked_weights,
            &weight_prefix,
            mapped_size,
            trace.score,
            options.permutation_count,
            derive_gsea_seed(options.seed, &term_id),
        );
        let leading_edge_genes = if trace.score >= 0.0 {
            positions
                .iter()
                .copied()
                .take_while(|position| *position <= trace.peak_index)
                .map(|position| ranked.genes[position].gene_id.clone())
                .collect::<Vec<_>>()
        } else {
            positions
                .iter()
                .copied()
                .filter(|position| *position > trace.peak_index)
                .map(|position| ranked.genes[position].gene_id.clone())
                .collect::<Vec<_>>()
        };
        tested.push(GseaTermResult {
            term_id,
            term_name: term.name,
            namespace: term.namespace,
            input_gene_count: term.genes.len() as u64,
            mapped_gene_count: mapped_size as u64,
            enrichment_score: trace.score,
            direction: if trace.score >= 0.0 {
                "positive".to_owned()
            } else {
                "negative".to_owned()
            },
            peak_rank: trace.peak_index as u64 + 1,
            leading_edge_gene_count: leading_edge_genes.len() as u64,
            leading_edge_genes,
            nominal_p_value,
            fdr_bh: 1.0,
        });
    }

    if tested.is_empty() {
        return Err(FunctionalError::InvalidTable(format!(
            "no gene sets remain after mapping and size filtering (min_set_size={}, max_set_size={})",
            options.min_set_size, options.max_set_size
        )));
    }
    adjust_gsea_benjamini_hochberg(&mut tested);
    tested.sort_by(|left, right| {
        left.fdr_bh
            .total_cmp(&right.fdr_bh)
            .then_with(|| left.nominal_p_value.total_cmp(&right.nominal_p_value))
            .then_with(|| {
                right
                    .enrichment_score
                    .abs()
                    .total_cmp(&left.enrichment_score.abs())
            })
            .then_with(|| left.term_id.cmp(&right.term_id))
    });

    let mut warnings = Vec::new();
    if ranked.tie_group_count > 0 {
        warnings.push(format!(
            "{} ranked genes occur in {} tied-score groups; ties were ordered by gene identifier",
            ranked.tied_gene_count, ranked.tie_group_count
        ));
    }
    if duplicate_membership_count > 0 {
        warnings.push(format!(
            "{} duplicate gene-set membership rows were deduplicated",
            duplicate_membership_count
        ));
    }
    let unmapped_memberships = input_membership_count.saturating_sub(mapped_membership_count);
    if unmapped_memberships > 0 {
        warnings.push(format!(
            "{unmapped_memberships} unique gene-set memberships were absent from the ranked gene table"
        ));
    }
    if skipped_no_overlap_count > 0 {
        warnings.push(format!(
            "{skipped_no_overlap_count} gene sets had no ranked members and were not tested"
        ));
    }
    if skipped_below_min_size_count > 0 || skipped_above_max_size_count > 0 {
        warnings.push(format!(
            "{skipped_below_min_size_count} gene sets were below min_set_size and {skipped_above_max_size_count} were above max_set_size after mapping"
        ));
    }
    if skipped_full_universe_count > 0 {
        warnings.push(format!(
            "{skipped_full_universe_count} gene sets contained the complete ranked universe and had no valid miss denominator"
        ));
    }
    if zero_weight_set_count > 0 {
        warnings.push(format!(
            "{zero_weight_set_count} tested gene sets had zero total weighted hit score and used the documented unweighted hit fallback"
        ));
    }

    Ok(GseaResult {
        capability_id: GSEA_CAPABILITY_ID.to_owned(),
        schema_version: "1".to_owned(),
        analysis_type: "preranked-gsea".to_owned(),
        score_exponent: options.score_exponent,
        permutation_count: options.permutation_count,
        seed: options.seed,
        permutation_method: "deterministic gene-label permutation with per-set SplitMix64 streams and add-one correction".to_owned(),
        multiple_testing_method: "Benjamini-Hochberg across all tested gene sets".to_owned(),
        ranked_gene_count: ranked.genes.len() as u64,
        input_gene_set_count,
        tested_gene_set_count: tested.len() as u64,
        input_membership_count,
        mapped_membership_count,
        skipped_no_overlap_count,
        skipped_below_min_size_count,
        skipped_above_max_size_count,
        skipped_full_universe_count,
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

#[derive(Debug)]
struct RankedGene {
    gene_id: String,
    score: f64,
    weight: f64,
}

#[derive(Debug)]
struct RankedGeneTable {
    genes: Vec<RankedGene>,
    tie_group_count: u64,
    tied_gene_count: u64,
}

#[derive(Debug)]
struct GseaGeneSetTable {
    terms: BTreeMap<String, TermGenes>,
    input_gene_set_count: u64,
    input_membership_count: u64,
    duplicate_membership_count: u64,
}

#[derive(Debug, Clone, Copy)]
struct GseaTrace {
    score: f64,
    peak_index: usize,
    used_unweighted_fallback: bool,
}

fn validate_gsea_options(options: GseaOptions) -> Result<(), FunctionalError> {
    if !options.score_exponent.is_finite() || options.score_exponent < 0.0 {
        return Err(FunctionalError::InvalidOption(
            "score_exponent must be finite and greater than or equal to zero".to_owned(),
        ));
    }
    if options.min_set_size == 0 {
        return Err(FunctionalError::InvalidOption(
            "min_set_size must be greater than zero".to_owned(),
        ));
    }
    if options.max_set_size < options.min_set_size {
        return Err(FunctionalError::InvalidOption(
            "max_set_size must be greater than or equal to min_set_size".to_owned(),
        ));
    }
    if options.max_set_size > MAX_FUNCTIONAL_IDENTIFIERS {
        return Err(FunctionalError::InvalidOption(format!(
            "max_set_size must not exceed {MAX_FUNCTIONAL_IDENTIFIERS}"
        )));
    }
    if !(1..=MAX_GSEA_PERMUTATIONS).contains(&options.permutation_count) {
        return Err(FunctionalError::InvalidOption(format!(
            "permutation_count must be between 1 and {MAX_GSEA_PERMUTATIONS}"
        )));
    }
    Ok(())
}

fn read_ranked_genes(path: &Path, score_exponent: f64) -> Result<RankedGeneTable, FunctionalError> {
    let text = read_bounded_text(path)?;
    let delimiter = infer_delimiter(path, &text);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .flexible(false)
        .from_reader(text.as_bytes());
    let headers = reader.headers()?.clone();
    let gene_column = resolve_column(
        &headers,
        None,
        &["gene_id", "gene", "query", "query_id", "protein_id", "id"],
        "gene",
    )?;
    let score_column = resolve_column(
        &headers,
        None,
        &[
            "score",
            "rank_score",
            "ranking_metric",
            "metric",
            "stat",
            "statistic",
            "wald_stat",
            "log2_fold_change",
            "log2fc",
        ],
        "ranking score",
    )?;
    if gene_column == score_column {
        return Err(FunctionalError::InvalidTable(
            "gene and ranking score columns must be different".to_owned(),
        ));
    }

    let mut genes = Vec::new();
    let mut seen = BTreeMap::<String, usize>::new();
    for (row_index, record) in reader.records().enumerate() {
        if row_index as u64 >= MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "ranked gene row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let record = record?;
        let gene_id = record.get(gene_column).unwrap_or_default().trim();
        let raw_score = record.get(score_column).unwrap_or_default().trim();
        let display_row = row_index + 2;
        if gene_id.is_empty() || is_missing_value(gene_id) {
            return Err(FunctionalError::InvalidTable(format!(
                "ranked gene row {display_row} has no gene identifier"
            )));
        }
        if raw_score.is_empty() || is_missing_value(raw_score) {
            return Err(FunctionalError::InvalidTable(format!(
                "ranked gene row {display_row} has no ranking score"
            )));
        }
        let mut score = raw_score.parse::<f64>().map_err(|_| {
            FunctionalError::InvalidTable(format!(
                "ranked gene row {display_row} has non-numeric score {raw_score:?}"
            ))
        })?;
        if !score.is_finite() {
            return Err(FunctionalError::InvalidTable(format!(
                "ranked gene row {display_row} must have a finite score"
            )));
        }
        if score == 0.0 {
            score = 0.0;
        }
        if let Some(previous_row) = seen.insert(gene_id.to_owned(), display_row) {
            return Err(FunctionalError::InvalidTable(format!(
                "ranked gene identifier {gene_id:?} is duplicated at rows {previous_row} and {display_row}"
            )));
        }
        if genes.len() >= MAX_FUNCTIONAL_IDENTIFIERS {
            return Err(FunctionalError::LimitExceeded {
                resource: "ranked gene identifier count",
                limit: MAX_FUNCTIONAL_IDENTIFIERS as u64,
            });
        }
        genes.push(RankedGene {
            gene_id: gene_id.to_owned(),
            score,
            weight: 0.0,
        });
    }
    if genes.len() < 2 {
        return Err(FunctionalError::InvalidTable(
            "preranked GSEA requires at least two uniquely ranked genes".to_owned(),
        ));
    }
    genes.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.gene_id.cmp(&right.gene_id))
    });
    if genes
        .iter()
        .skip(1)
        .all(|gene| gene.score == genes[0].score)
    {
        return Err(FunctionalError::InvalidTable(
            "all ranked genes have the same score; GSEA requires an informative ordering"
                .to_owned(),
        ));
    }

    let maximum_absolute_score = genes
        .iter()
        .map(|gene| gene.score.abs())
        .fold(0.0_f64, f64::max);
    for gene in &mut genes {
        gene.weight = if score_exponent == 0.0 {
            1.0
        } else {
            (gene.score.abs() / maximum_absolute_score).powf(score_exponent)
        };
    }
    let mut tie_group_count = 0_u64;
    let mut tied_gene_count = 0_u64;
    let mut run_start = 0_usize;
    while run_start < genes.len() {
        let mut run_end = run_start + 1;
        while run_end < genes.len() && genes[run_end].score == genes[run_start].score {
            run_end += 1;
        }
        if run_end - run_start > 1 {
            tie_group_count += 1;
            tied_gene_count += (run_end - run_start) as u64;
        }
        run_start = run_end;
    }
    Ok(RankedGeneTable {
        genes,
        tie_group_count,
        tied_gene_count,
    })
}

fn read_gsea_gene_sets(path: &Path) -> Result<GseaGeneSetTable, FunctionalError> {
    let text = read_bounded_text(path)?;
    let delimiter = infer_delimiter(path, &text);
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
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
        &[
            "term_id",
            "term",
            "gene_set",
            "set_id",
            "pathway_id",
            "category_id",
        ],
        "gene set",
    )?;
    if gene_column == term_column {
        return Err(FunctionalError::InvalidTable(
            "gene and gene-set columns must be different".to_owned(),
        ));
    }
    let name_column = optional_column(
        &headers,
        &[
            "term_name",
            "gene_set_name",
            "set_name",
            "name",
            "description",
        ],
    );
    let namespace_column = optional_column(&headers, &["namespace", "category", "source"]);
    let mut terms = BTreeMap::<String, TermGenes>::new();
    let mut input_membership_count = 0_u64;
    let mut duplicate_membership_count = 0_u64;
    for (row_index, record) in reader.records().enumerate() {
        if row_index as u64 >= MAX_FUNCTIONAL_ROWS {
            return Err(FunctionalError::LimitExceeded {
                resource: "GSEA membership row count",
                limit: MAX_FUNCTIONAL_ROWS,
            });
        }
        let record = record?;
        let gene_id = record.get(gene_column).unwrap_or_default().trim();
        let term_id = record.get(term_column).unwrap_or_default().trim();
        let display_row = row_index + 2;
        if gene_id.is_empty() || is_missing_value(gene_id) {
            return Err(FunctionalError::InvalidTable(format!(
                "gene-set membership row {display_row} has no gene identifier"
            )));
        }
        if term_id.is_empty() || is_missing_value(term_id) {
            return Err(FunctionalError::InvalidTable(format!(
                "gene-set membership row {display_row} has no gene-set identifier"
            )));
        }
        let name = optional_value(&record, name_column);
        let namespace = optional_value(&record, namespace_column);
        let term = terms
            .entry(term_id.to_owned())
            .or_insert_with(|| TermGenes {
                name: name.clone(),
                namespace: namespace.clone(),
                genes: BTreeSet::new(),
            });
        merge_gsea_metadata(&mut term.name, name, term_id, "name")?;
        merge_gsea_metadata(&mut term.namespace, namespace, term_id, "namespace")?;
        if term.genes.insert(gene_id.to_owned()) {
            input_membership_count += 1;
            if input_membership_count as usize > MAX_FUNCTIONAL_IDENTIFIERS {
                return Err(FunctionalError::LimitExceeded {
                    resource: "unique GSEA membership count",
                    limit: MAX_FUNCTIONAL_IDENTIFIERS as u64,
                });
            }
        } else {
            duplicate_membership_count += 1;
        }
    }
    if terms.is_empty() {
        return Err(FunctionalError::InvalidTable(
            "gene-set membership table contains no memberships".to_owned(),
        ));
    }
    Ok(GseaGeneSetTable {
        input_gene_set_count: terms.len() as u64,
        terms,
        input_membership_count,
        duplicate_membership_count,
    })
}

fn merge_gsea_metadata(
    stored: &mut Option<String>,
    candidate: Option<String>,
    term_id: &str,
    label: &str,
) -> Result<(), FunctionalError> {
    match (stored.as_ref(), candidate) {
        (Some(existing), Some(candidate)) if existing != &candidate => {
            Err(FunctionalError::InvalidTable(format!(
                "gene set {term_id:?} has conflicting {label} values {existing:?} and {candidate:?}"
            )))
        }
        (None, Some(candidate)) => {
            *stored = Some(candidate);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn prefix_sums(values: &[f64]) -> Vec<f64> {
    let mut prefix = Vec::with_capacity(values.len() + 1);
    prefix.push(0.0);
    for value in values {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + value);
    }
    prefix
}

fn enrichment_score_from_hits(weights: &[f64], hit_positions: &[usize]) -> GseaTrace {
    debug_assert!(!hit_positions.is_empty() && hit_positions.len() < weights.len());
    let hit_weight_total = hit_positions
        .iter()
        .map(|position| weights[*position])
        .sum::<f64>();
    let used_unweighted_fallback = hit_weight_total == 0.0;
    let hit_count = hit_positions.len();
    let miss_decrement = 1.0 / (weights.len() - hit_count) as f64;
    let mut running = 0.0_f64;
    let mut maximum = 0.0_f64;
    let mut minimum = 0.0_f64;
    let mut maximum_index = 0_usize;
    let mut minimum_index = 0_usize;
    let mut previous_hit = None;
    // Between hits the running sum only decreases, so sparse boundary evaluation is exact.
    for position in hit_positions.iter().copied() {
        let misses = position - previous_hit.map_or(0, |previous| previous + 1);
        if misses > 0 {
            running -= misses as f64 * miss_decrement;
            if running < minimum {
                minimum = running;
                minimum_index = position - 1;
            }
        }
        running += if used_unweighted_fallback {
            1.0 / hit_count as f64
        } else {
            weights[position] / hit_weight_total
        };
        if running > maximum {
            maximum = running;
            maximum_index = position;
        }
        previous_hit = Some(position);
    }
    let trailing_misses = weights.len() - previous_hit.unwrap_or(0) - 1;
    if trailing_misses > 0 {
        running -= trailing_misses as f64 * miss_decrement;
        if running < minimum {
            minimum = running;
            minimum_index = weights.len() - 1;
        }
    }
    if maximum.abs() + 64.0 * f64::EPSILON >= minimum.abs() {
        GseaTrace {
            score: maximum.clamp(-1.0, 1.0),
            peak_index: maximum_index,
            used_unweighted_fallback,
        }
    } else {
        GseaTrace {
            score: minimum.clamp(-1.0, 1.0),
            peak_index: minimum_index,
            used_unweighted_fallback,
        }
    }
}

fn enrichment_score_from_misses(
    weights: &[f64],
    weight_prefix: &[f64],
    miss_positions: &[usize],
) -> GseaTrace {
    debug_assert!(!miss_positions.is_empty() && miss_positions.len() < weights.len());
    let hit_count = weights.len() - miss_positions.len();
    let missed_weight = miss_positions
        .iter()
        .map(|position| weights[*position])
        .sum::<f64>();
    let hit_weight_total = (weight_prefix[weights.len()] - missed_weight).max(0.0);
    let used_unweighted_fallback = hit_weight_total == 0.0;
    let miss_decrement = 1.0 / miss_positions.len() as f64;
    let mut running = 0.0_f64;
    let mut maximum = 0.0_f64;
    let mut minimum = 0.0_f64;
    let mut maximum_index = 0_usize;
    let mut minimum_index = 0_usize;
    let mut start = 0_usize;
    // This complementary path keeps permutations of large sets proportional to miss count.
    for position in miss_positions.iter().copied() {
        let hits = position - start;
        if hits > 0 {
            running += if used_unweighted_fallback {
                hits as f64 / hit_count as f64
            } else {
                (weight_prefix[position] - weight_prefix[start]) / hit_weight_total
            };
            if running > maximum {
                maximum = running;
                maximum_index = position - 1;
            }
        }
        running -= miss_decrement;
        if running < minimum {
            minimum = running;
            minimum_index = position;
        }
        start = position + 1;
    }
    if start < weights.len() {
        let hits = weights.len() - start;
        running += if used_unweighted_fallback {
            hits as f64 / hit_count as f64
        } else {
            (weight_prefix[weights.len()] - weight_prefix[start]) / hit_weight_total
        };
        if running > maximum {
            maximum = running;
            maximum_index = weights.len() - 1;
        }
    }
    if maximum.abs() + 64.0 * f64::EPSILON >= minimum.abs() {
        GseaTrace {
            score: maximum.clamp(-1.0, 1.0),
            peak_index: maximum_index,
            used_unweighted_fallback,
        }
    } else {
        GseaTrace {
            score: minimum.clamp(-1.0, 1.0),
            peak_index: minimum_index,
            used_unweighted_fallback,
        }
    }
}

fn gsea_permutation_p_value(
    weights: &[f64],
    weight_prefix: &[f64],
    set_size: usize,
    observed_score: f64,
    permutation_count: u32,
    seed: u64,
) -> f64 {
    if observed_score == 0.0 {
        return 1.0;
    }
    let mut random = SplitMix64::new(seed);
    let sample_hits = set_size <= weights.len() - set_size;
    let sample_size = if sample_hits {
        set_size
    } else {
        weights.len() - set_size
    };
    let mut same_direction = 0_u64;
    let mut at_least_as_extreme = 0_u64;
    for _ in 0..permutation_count {
        let positions = sample_sorted_indices(weights.len(), sample_size, &mut random);
        let permuted_score = if sample_hits {
            enrichment_score_from_hits(weights, &positions).score
        } else {
            enrichment_score_from_misses(weights, weight_prefix, &positions).score
        };
        if observed_score > 0.0 && permuted_score > 0.0 {
            same_direction += 1;
            if permuted_score >= observed_score {
                at_least_as_extreme += 1;
            }
        } else if observed_score < 0.0 && permuted_score < 0.0 {
            same_direction += 1;
            if permuted_score <= observed_score {
                at_least_as_extreme += 1;
            }
        }
    }
    // Broad-style preranked nominal probabilities condition on null scores in the observed direction.
    (at_least_as_extreme + 1) as f64 / (same_direction + 1) as f64
}

fn sample_sorted_indices(
    population_size: usize,
    sample_size: usize,
    random: &mut SplitMix64,
) -> Vec<usize> {
    let mut selected = BTreeSet::new();
    for candidate in (population_size - sample_size)..population_size {
        let draw = random.index(candidate + 1);
        if !selected.insert(draw) {
            selected.insert(candidate);
        }
    }
    selected.into_iter().collect()
}

fn derive_gsea_seed(seed: u64, term_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in term_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    seed ^ hash
}

#[derive(Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        let upper = upper as u64;
        let rejection_threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next();
            let product = u128::from(value) * u128::from(upper);
            let low = product as u64;
            if low >= rejection_threshold {
                return (product >> 64) as usize;
            }
        }
    }
}

fn adjust_gsea_benjamini_hochberg(terms: &mut [GseaTermResult]) {
    let mut order = (0..terms.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        terms[*left]
            .nominal_p_value
            .total_cmp(&terms[*right].nominal_p_value)
            .then_with(|| terms[*left].term_id.cmp(&terms[*right].term_id))
    });
    let count = order.len() as f64;
    let mut running = 1.0_f64;
    for (reverse_index, term_index) in order.into_iter().enumerate().rev() {
        let rank = reverse_index as f64 + 1.0;
        let adjusted = (terms[term_index].nominal_p_value * count / rank).min(1.0);
        running = running.min(adjusted);
        terms[term_index].fdr_bh = running;
    }
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
        EnrichmentKind, EnrichmentOptions, GoAnnotationOptions, GseaOptions,
        enrichment_score_from_hits, enrichment_score_from_misses, gsea_preranked_path,
        normalize_eggnog_path, normalize_go_annotations_path, overrepresentation_path, prefix_sums,
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

    #[test]
    fn computes_deterministic_preranked_gsea_with_leading_edges_and_bh() {
        let ranks = temporary("gsea-ranks.csv");
        let memberships = temporary("gsea-memberships.tsv");
        fs::write(
            &ranks,
            "gene_id,score\ng1,5\ng2,4\ng3,3\ng4,-1\ng5,-2\ng6,-4\n",
        )
        .expect("write GSEA ranks");
        fs::write(
            &memberships,
            "gene_id\tterm_id\tterm_name\tnamespace\n\
g1\ttop\tTop genes\tcustom\n\
g2\ttop\tTop genes\tcustom\n\
g1\ttop\tTop genes\tcustom\n\
g5\tbottom\tBottom genes\tcustom\n\
g6\tbottom\tBottom genes\tcustom\n\
g1\tmixed\tMixed genes\tcustom\n\
g6\tmixed\tMixed genes\tcustom\n\
z1\tmissing\tMissing genes\tcustom\n\
g3\tsmall\tSmall set\tcustom\n\
g1\tall\tAll genes\tcustom\n\
g2\tall\tAll genes\tcustom\n\
g3\tall\tAll genes\tcustom\n\
g4\tall\tAll genes\tcustom\n\
g5\tall\tAll genes\tcustom\n\
g6\tall\tAll genes\tcustom\n",
        )
        .expect("write GSEA memberships");
        let options = GseaOptions {
            score_exponent: 1.0,
            min_set_size: 2,
            max_set_size: 6,
            permutation_count: 127,
            seed: 42,
        };
        let result = gsea_preranked_path(&ranks, &memberships, options).expect("run GSEA");
        let repeated = gsea_preranked_path(&ranks, &memberships, options).expect("repeat GSEA");
        assert_eq!(result, repeated);
        assert_eq!(result.capability_id, "enrichment.gsea.v1");
        assert_eq!(result.schema_version, "1");
        assert_eq!(result.ranked_gene_count, 6);
        assert_eq!(result.input_gene_set_count, 6);
        assert_eq!(result.tested_gene_set_count, 3);
        assert_eq!(result.skipped_no_overlap_count, 1);
        assert_eq!(result.skipped_below_min_size_count, 1);
        assert_eq!(result.skipped_full_universe_count, 1);
        assert_eq!(result.input_membership_count, 14);
        assert_eq!(result.mapped_membership_count, 13);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("duplicate gene-set membership"))
        );

        let top = result
            .terms
            .iter()
            .find(|term| term.term_id == "top")
            .expect("top term");
        assert!((top.enrichment_score - 1.0).abs() < 1e-12);
        assert_eq!(top.direction, "positive");
        assert_eq!(top.peak_rank, 2);
        assert_eq!(top.leading_edge_genes, ["g1", "g2"]);
        let bottom = result
            .terms
            .iter()
            .find(|term| term.term_id == "bottom")
            .expect("bottom term");
        assert!((bottom.enrichment_score + 1.0).abs() < 1e-12);
        assert_eq!(bottom.direction, "negative");
        assert_eq!(bottom.leading_edge_genes, ["g5", "g6"]);
        assert!(result.terms.iter().all(|term| {
            (0.0..=1.0).contains(&term.nominal_p_value) && (0.0..=1.0).contains(&term.fdr_bh)
        }));
        let json = serde_json::to_value(&result).expect("serialize GSEA result");
        assert_eq!(json["permutation_count"], 127);
        assert_eq!(json["seed"], 42);
        fs::remove_file(ranks).expect("remove ranks");
        fs::remove_file(memberships).expect("remove memberships");
    }

    #[test]
    fn stabilizes_rank_ties_and_documents_zero_weight_fallback() {
        let ranks = temporary("gsea-tied-ranks.tsv");
        let memberships = temporary("gsea-zero-memberships.tsv");
        fs::write(
            &ranks,
            "gene\tranking_metric\ng2\t2\ng1\t2\ng3\t0\ng4\t0\ng5\t-1\n",
        )
        .expect("write tied ranks");
        fs::write(
            &memberships,
            "gene\tgene_set\ng1\thigh\ng2\thigh\ng3\tzero\ng4\tzero\n",
        )
        .expect("write zero-weight set");
        let result = gsea_preranked_path(
            &ranks,
            &memberships,
            GseaOptions {
                min_set_size: 2,
                max_set_size: 3,
                permutation_count: 31,
                seed: 7,
                ..GseaOptions::default()
            },
        )
        .expect("run tied GSEA");
        let high = result
            .terms
            .iter()
            .find(|term| term.term_id == "high")
            .expect("high set");
        assert_eq!(high.leading_edge_genes, ["g1", "g2"]);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("tied-score groups"))
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("unweighted hit fallback"))
        );
        fs::remove_file(ranks).expect("remove ranks");
        fs::remove_file(memberships).expect("remove memberships");
    }

    #[test]
    fn rejects_duplicate_ranked_identifiers_and_invalid_options() {
        let ranks = temporary("gsea-duplicate-ranks.tsv");
        let memberships = temporary("gsea-valid-memberships.tsv");
        fs::write(&ranks, "gene_id\tscore\ng1\t2\ng1\t1\ng2\t0\n").expect("write duplicate ranks");
        fs::write(&memberships, "gene_id\tterm_id\ng1\tset1\n").expect("write memberships");
        let duplicate_error = gsea_preranked_path(
            &ranks,
            &memberships,
            GseaOptions {
                min_set_size: 1,
                max_set_size: 2,
                permutation_count: 10,
                seed: 0,
                score_exponent: 1.0,
            },
        )
        .expect_err("duplicate ranks must fail");
        assert!(duplicate_error.to_string().contains("duplicated"));
        let option_error = gsea_preranked_path(
            &ranks,
            &memberships,
            GseaOptions {
                permutation_count: 0,
                ..GseaOptions::default()
            },
        )
        .expect_err("zero permutations must fail");
        assert!(option_error.to_string().contains("permutation_count"));
        fs::remove_file(ranks).expect("remove ranks");
        fs::remove_file(memberships).expect("remove memberships");
    }

    #[test]
    fn sparse_hit_and_sparse_miss_score_algorithms_agree() {
        let weights = [1.0, 0.8, 0.6, 0.2, 0.4, 0.8];
        let prefix = prefix_sums(&weights);
        for mask in 1_u32..((1_u32 << weights.len()) - 1) {
            let hits = (0..weights.len())
                .filter(|index| mask & (1 << index) != 0)
                .collect::<Vec<_>>();
            let misses = (0..weights.len())
                .filter(|index| mask & (1 << index) == 0)
                .collect::<Vec<_>>();
            let from_hits = enrichment_score_from_hits(&weights, &hits);
            let from_misses = enrichment_score_from_misses(&weights, &prefix, &misses);
            assert!(
                (from_hits.score - from_misses.score).abs() < 1e-12,
                "sparse paths differ for membership mask {mask:#08b}"
            );
        }
    }

    fn temporary(name: &str) -> PathBuf {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-functional-{}-{counter}-{name}",
            std::process::id()
        ))
    }
}
