#![forbid(unsafe_code)]

use linxira_bio_core::alignment::{SamQcMetrics, sam_qc_path};
use linxira_bio_core::annotation::{
    AnnotationExtractOptions, AnnotationNormalizeOptions, AnnotationStats, GeneDensityOptions,
    GeneDensityResult, GenePositionOptions, annotation_gene_positions_path, annotation_stats_path,
    extract_annotation_sequences_path, gene_density_path, gxf_to_bed_path,
    normalize_annotation_path,
};
use linxira_bio_core::cohort::{CohortTableQc, cohort_table_qc_path};
use linxira_bio_core::coordinate::{
    ContactMapOptions, MmcifStructureSummary, StructureContactMapResult, StructureGeometryResult,
    StructureSequenceResult, StructureSuperpositionResult, SuperpositionOptions,
    extract_structure_sequences_path, measure_structure_geometry_path, mmcif_summary_path,
    parse_atom_selector, structure_contact_map_path, superpose_structures_path,
};
use linxira_bio_core::dataset::{DatasetInspection, DatasetSupport, inspect_dataset};
use linxira_bio_core::domain::{ProteinDomainParseResult, parse_protein_domains_path};
use linxira_bio_core::dotplot::{DotplotOptions, render_dotplot_svg_path};
use linxira_bio_core::environment::{
    ApplyResult, EnvironmentAudit, EnvironmentMode, EnvironmentPlan, EnvironmentPlanOptions,
    PlanActionState, apply_environment, audit_environment, parse_environment_mode,
    plan_environment_with_options,
};
use linxira_bio_core::expression::{
    ExpressionClusterOptions, ExpressionClusterResult, ExpressionHeatmapOptions,
    ExpressionHeatmapResult, ExpressionMatrixQc, ExpressionNormalizeOptions, ExpressionPcaOptions,
    ExpressionPcaResult, expression_cluster_path, expression_heatmap_path,
    expression_matrix_qc_path, expression_pca_path, normalize_expression_matrix_path,
    parse_expression_normalization_method,
};
use linxira_bio_core::fastq::{FastqQcMetrics, FastqQcOptions, QualityEncodingMode, fastq_qc_path};
use linxira_bio_core::fastq_transform::{
    FastqAdapterOptions, FastqDeduplicateKey, FastqDeduplicateOptions, FastqSubsampleOptions,
    FastqTransformQualityEncoding, FastqTrimOptions, fastq_adapter_trim_path,
    fastq_deduplicate_path, fastq_subsample_path, fastq_trim_path,
};
use linxira_bio_core::functional::{
    AnnotationMapResult, EggnogNormalizeResult, EnrichmentKind, EnrichmentOptions,
    EnrichmentResult, GoAnnotationOptions, GseaOptions, GseaResult, gsea_preranked_path,
    normalize_eggnog_path, normalize_go_annotations_path, overrepresentation_path,
};
use linxira_bio_core::interval::{
    IntervalIntersectStats, IntervalMergeOptions, bed_closest_path, bed_intersect_path,
    bed_merge_path, bed_subtract_path,
};
use linxira_bio_core::native_tools::{
    HmmerOptions, IqtreeOptions, MastOptions, MemeOptions, Minimap2LongReadOptions, MuscleOptions,
    NativeToolResult, ShortReadAlignmentOptions, SimilaritySearchOptions, SnpEffOptions,
    WgcnaOptions, parse_blast_program, parse_diamond_mode, parse_hmmer_mode, parse_meme_alphabet,
    parse_minimap2_preset, parse_muscle_mode, parse_trimal_mode, run_bam_to_bigwig_path,
    run_blast_fasta_path, run_diamond_fasta_path, run_dssp_path, run_hmmer_path, run_iqtree_path,
    run_kaks_path, run_mast_path, run_mcscanx_path, run_meme_path, run_minimap2_long_read_path,
    run_muscle_path, run_rnafold_path, run_samtools_report_path, run_short_read_alignment_path,
    run_snpeff_path, run_trimal_path, run_wgcna_path,
};
use linxira_bio_core::phylogeny::{
    DistanceMatrixOptions, DistanceMatrixResult, TreeTransformOptions, TreeTransformResult,
    TreeVisualizationOptions, distance_matrix_path, read_tree_label_map_path, render_tree_svg_path,
    transform_newick_path,
};
use linxira_bio_core::protein::{ProteinPropertiesResult, protein_properties_path};
use linxira_bio_core::runtime::{RuntimeProviderStatus, load_runtime_catalog};
use linxira_bio_core::scientific_visualization::{
    AnnotationStructureOptions, DomainArchitectureOptions, EnrichmentPlotStyle,
    EnrichmentVisualizationOptions, SvgVisualizationResult, SyntenyPlotStyle,
    SyntenyVisualizationOptions, VolcanoPlotOptions, render_annotation_structure_svg_path,
    render_domain_architecture_svg_path, render_enrichment_svg_path, render_motif_logo_svg_path,
    render_synteny_svg_with_options_path, render_volcano_svg_path,
};
use linxira_bio_core::sequence::{SequenceStats, fasta_stats_path};
use linxira_bio_core::sequence_analysis::{
    ConsensusOptions, EpcrOptions, KmerCountOptions, ShuffleOptions, consensus_from_alignment_path,
    count_kmers_path, epcr_path, shuffle_sequences_path,
};
use linxira_bio_core::sequence_transform::{
    SequenceExtractOptions, SequenceFilterOptions, SequenceFromTableOptions,
    SequenceIdNormalizeOptions, SequenceMergeOptions, SequenceOrfOptions, SequenceSplitOptions,
    SequenceTableDelimiter, SequenceToTableOptions, SequenceTranslateOptions, extract_fasta_path,
    fasta_to_table_path, filter_fasta_path, find_orfs_fasta_path, merge_fasta_paths,
    normalize_fasta_ids_path, parse_sequence_region_spec, reverse_complement_fasta_path,
    split_fasta_path, table_to_fasta_path, translate_fasta_path,
};
use linxira_bio_core::set_analysis::{
    SetAnalysisOptions, UpSetAnalysis, VennAnalysis, upset_analysis_path, venn_analysis_path,
};
use linxira_bio_core::similarity::{
    BlastParseResult, ReciprocalBestHitOptions, ReciprocalBestHitResult, parse_blast_path,
    reciprocal_best_hits_path,
};
use linxira_bio_core::structure::{PdbStructureSummary, PdbSummaryOptions, pdb_summary_path};
use linxira_bio_core::table::{
    TableDelimiter, TableFilter, TableManipulateOptions, manipulate_table_path,
};
use linxira_bio_core::variant::{VcfStats, vcf_stats_path};
use linxira_bio_core::variant_transform::{
    VariantComparisonResult, VariantFilterOptions, compare_vcf_paths, filter_vcf_path,
    normalize_vcf_path, vcf_to_table_path,
};
use linxira_bio_export::export_json_file;
use linxira_bio_protocol::{
    AnalysisResult, ExecutionMode, WorkflowPackManifest, WorkflowRuntimeKind,
    semver_range::core_compatibility_matches,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const CAPABILITY_CATALOG: &str = include_str!("../../../../capabilities/catalog.json");
const WORKFLOW_CATALOG: &str = include_str!("../../../../workflows/catalog.json");

/// Resolve `relative` (e.g. `capabilities/catalog.json`) under the runtime
/// catalog root. The runtime root is `LINXIRA_BIO_WORKFLOW_ROOT` when set and
/// non-empty, otherwise `<cwd>/workflows`. Returns `None` when the root or the
/// file does not exist so callers can fall back to the embedded snapshot.
fn runtime_catalog_file(relative: &str) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let root = env::var_os("LINXIRA_BIO_WORKFLOW_ROOT")
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("workflows"));
    let root = match fs::canonicalize(&root) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    let candidate = root.join(relative);
    if candidate.is_file() {
        Ok(Some(candidate))
    } else {
        Ok(None)
    }
}

/// Load the capability catalog: the runtime file when present, else the
/// embedded snapshot. A present-but-invalid runtime catalog is an error.
fn load_capability_catalog() -> Result<serde_json::Value, Box<dyn Error>> {
    match runtime_catalog_file("capabilities/catalog.json")? {
        Some(path) => {
            let text = fs::read_to_string(&path)?;
            let catalog: serde_json::Value = serde_json::from_str(&text)?;
            if catalog
                .get("schema_version")
                .and_then(serde_json::Value::as_str)
                != Some("1")
            {
                return Err(format!(
                    "runtime capability catalog has an unsupported schema: {}",
                    path.display()
                )
                .into());
            }
            Ok(catalog)
        }
        None => Ok(serde_json::from_str(CAPABILITY_CATALOG)?),
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowCatalog {
    schema_version: String,
    packs: Vec<WorkflowCatalogPack>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WorkflowCatalogPack {
    id: String,
    capability: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capability_aliases: Vec<String>,
    status: String,
    trust: String,
    runtime: WorkflowRuntimeKind,
    manifest: String,
}

impl WorkflowCatalogPack {
    fn capabilities(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.capability.as_str())
            .chain(self.capability_aliases.iter().map(String::as_str))
    }

    fn supports_capability(&self, capability: &str) -> bool {
        self.capabilities().any(|candidate| candidate == capability)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ValidatedWorkflowRequest {
    job_id: String,
    capability: String,
}

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Box<dyn Error>> {
    match arguments.as_slice() {
        [help] if matches!(help.as_str(), "-h" | "--help") => {
            println!("{}", usage());
            Ok(())
        }
        [command] if command == "capabilities" => print_capabilities(false),
        [command, json] if command == "capabilities" && json == "--json" => {
            print_capabilities(true)
        }
        [command] if command == "doctor" => print_doctor(false),
        [command, json] if command == "doctor" && json == "--json" => print_doctor(true),
        [environment, audit] if environment == "environment" && audit == "audit" => {
            print_environment_audit(false)
        }
        [environment, audit, json]
            if environment == "environment" && audit == "audit" && json == "--json" =>
        {
            print_environment_audit(true)
        }
        [environment, plan, arguments @ ..] if environment == "environment" && plan == "plan" => {
            print_environment_plan(arguments)
        }
        [environment, apply, arguments @ ..]
            if environment == "environment" && apply == "apply" =>
        {
            print_environment_apply(arguments)
        }
        [fastq, qc, arguments @ ..] if fastq == "fastq" && qc == "qc" => print_fastq_qc(arguments),
        [fastq, trim, arguments @ ..] if fastq == "fastq" && trim == "trim" => {
            print_fastq_trim(arguments)
        }
        [fastq, adapter_trim, arguments @ ..]
            if fastq == "fastq" && adapter_trim == "adapter-trim" =>
        {
            print_fastq_adapter_trim(arguments)
        }
        [fastq, deduplicate, arguments @ ..]
            if fastq == "fastq" && deduplicate == "deduplicate" =>
        {
            print_fastq_deduplicate(arguments)
        }
        [fastq, subsample, arguments @ ..] if fastq == "fastq" && subsample == "subsample" => {
            print_fastq_subsample(arguments)
        }
        [alignment, qc, path] if alignment == "alignment" && qc == "qc" => {
            print_alignment_qc(path, false)
        }
        [alignment, qc, path, json]
            if alignment == "alignment" && qc == "qc" && json == "--json" =>
        {
            print_alignment_qc(path, true)
        }
        [alignment, bam_cram_qc, arguments @ ..]
            if alignment == "alignment" && bam_cram_qc == "bam-cram-qc" =>
        {
            print_samtools_report(arguments, "stats", "alignment.bam-cram.qc.v1")
        }
        [alignment, coverage, arguments @ ..]
            if alignment == "alignment" && coverage == "coverage" =>
        {
            print_samtools_report(arguments, "coverage", "alignment.coverage.v1")
        }
        [alignment, bam_to_bigwig, arguments @ ..]
            if alignment == "alignment" && bam_to_bigwig == "bam-to-bigwig" =>
        {
            print_bam_to_bigwig(arguments)
        }
        [alignment, short_read, arguments @ ..]
            if alignment == "alignment" && short_read == "short-read" =>
        {
            print_short_read_alignment(arguments)
        }
        [alignment, long_read, arguments @ ..]
            if alignment == "alignment" && long_read == "long-read" =>
        {
            print_long_read_alignment(arguments)
        }
        [annotation, stats, arguments @ ..] if annotation == "annotation" && stats == "stats" => {
            print_annotation_stats(arguments)
        }
        [annotation, normalize, arguments @ ..]
            if annotation == "annotation" && normalize == "normalize" =>
        {
            print_annotation_normalize(arguments)
        }
        [annotation, positions, arguments @ ..]
            if annotation == "annotation" && positions == "positions" =>
        {
            print_annotation_positions(arguments)
        }
        [annotation, to_bed, arguments @ ..]
            if annotation == "annotation" && to_bed == "to-bed" =>
        {
            print_annotation_to_bed(arguments)
        }
        [annotation, extract, arguments @ ..]
            if annotation == "annotation" && extract == "extract" =>
        {
            print_annotation_extract(arguments)
        }
        [annotation, gene_density, arguments @ ..]
            if annotation == "annotation" && gene_density == "gene-density" =>
        {
            print_gene_density(arguments)
        }
        [annotation, go, arguments @ ..] if annotation == "annotation" && go == "go" => {
            print_go_annotations(arguments)
        }
        [annotation, eggnog, arguments @ ..]
            if annotation == "annotation" && eggnog == "eggnog" =>
        {
            print_eggnog_annotations(arguments)
        }
        [annotation, plot, arguments @ ..] if annotation == "annotation" && plot == "plot" => {
            print_annotation_structure_plot(arguments)
        }
        [runtime, catalog] if runtime == "runtime" && catalog == "catalog" => {
            print_runtime_catalog(false)
        }
        [runtime, catalog, json]
            if runtime == "runtime" && catalog == "catalog" && json == "--json" =>
        {
            print_runtime_catalog(true)
        }
        [workflow, packs] if workflow == "workflow" && packs == "packs" => {
            print_workflow_packs(false)
        }
        [workflow, packs, json]
            if workflow == "workflow" && packs == "packs" && json == "--json" =>
        {
            print_workflow_packs(true)
        }
        [workflow, run, pack_id, request, result] if workflow == "workflow" && run == "run" => {
            run_workflow_pack(pack_id, Path::new(request), Path::new(result))
        }
        [dataset, inspect, path] if dataset == "dataset" && inspect == "inspect" => {
            print_dataset_inspection(path, false)
        }
        [dataset, inspect, path, json]
            if dataset == "dataset" && inspect == "inspect" && json == "--json" =>
        {
            print_dataset_inspection(path, true)
        }
        [export, table, input, output] if export == "export" && table == "table" => {
            print_table_export(input, output, false)
        }
        [table, manipulate, arguments @ ..] if table == "table" && manipulate == "manipulate" => {
            print_table_manipulate(arguments)
        }
        [export, table, input, output, json]
            if export == "export" && table == "table" && json == "--json" =>
        {
            print_table_export(input, output, true)
        }
        [sequence, stats, path] if sequence == "sequence" && stats == "stats" => {
            print_sequence_stats(path, false)
        }
        [sequence, stats, path, json]
            if sequence == "sequence" && stats == "stats" && json == "--json" =>
        {
            print_sequence_stats(path, true)
        }
        [sequence, extract, arguments @ ..] if sequence == "sequence" && extract == "extract" => {
            print_sequence_extract(arguments)
        }
        [sequence, filter, arguments @ ..] if sequence == "sequence" && filter == "filter" => {
            print_sequence_filter(arguments)
        }
        [sequence, reverse_complement, arguments @ ..]
            if sequence == "sequence" && reverse_complement == "reverse-complement" =>
        {
            print_sequence_reverse_complement(arguments)
        }
        [sequence, translate, arguments @ ..]
            if sequence == "sequence" && translate == "translate" =>
        {
            print_sequence_translate(arguments)
        }
        [sequence, orf, arguments @ ..] if sequence == "sequence" && orf == "orf" => {
            print_sequence_orf(arguments)
        }
        [sequence, normalize_ids, arguments @ ..]
            if sequence == "sequence" && normalize_ids == "normalize-ids" =>
        {
            print_sequence_normalize_ids(arguments)
        }
        [sequence, merge, arguments @ ..] if sequence == "sequence" && merge == "merge" => {
            print_sequence_merge(arguments)
        }
        [sequence, split, arguments @ ..] if sequence == "sequence" && split == "split" => {
            print_sequence_split(arguments)
        }
        [sequence, to_table, arguments @ ..]
            if sequence == "sequence" && to_table == "to-table" =>
        {
            print_sequence_to_table(arguments)
        }
        [sequence, from_table, arguments @ ..]
            if sequence == "sequence" && from_table == "from-table" =>
        {
            print_sequence_from_table(arguments)
        }
        [sequence, kmer_count, arguments @ ..]
            if sequence == "sequence" && kmer_count == "kmer-count" =>
        {
            print_sequence_kmer_count(arguments)
        }
        [sequence, consensus, arguments @ ..]
            if sequence == "sequence" && consensus == "consensus" =>
        {
            print_sequence_consensus(arguments)
        }
        [sequence, shuffle, arguments @ ..] if sequence == "sequence" && shuffle == "shuffle" => {
            print_sequence_shuffle(arguments)
        }
        [sequence, convert, arguments @ ..] if sequence == "sequence" && convert == "convert" => {
            print_sequence_convert(arguments)
        }
        [primer, epcr, arguments @ ..] if primer == "primer" && epcr == "epcr" => {
            print_primer_epcr(arguments)
        }
        [variant, stats, path] if variant == "variant" && stats == "stats" => {
            print_variant_stats(path, false)
        }
        [variant, stats, path, json]
            if variant == "variant" && stats == "stats" && json == "--json" =>
        {
            print_variant_stats(path, true)
        }
        [medical, variant_cohort, path]
            if medical == "medical" && variant_cohort == "variant-cohort" =>
        {
            print_medical_variant_cohort(path, false)
        }
        [medical, variant_cohort, path, json]
            if medical == "medical" && variant_cohort == "variant-cohort" && json == "--json" =>
        {
            print_medical_variant_cohort(path, true)
        }
        [variant, filter, arguments @ ..] if variant == "variant" && filter == "filter" => {
            print_variant_filter(arguments)
        }
        [variant, compare, left, right] if variant == "variant" && compare == "compare" => {
            print_variant_compare(left, right, false)
        }
        [variant, compare, left, right, json]
            if variant == "variant" && compare == "compare" && json == "--json" =>
        {
            print_variant_compare(left, right, true)
        }
        [variant, normalize, arguments @ ..]
            if variant == "variant" && normalize == "normalize" =>
        {
            print_variant_normalize(arguments)
        }
        [variant, to_table, arguments @ ..] if variant == "variant" && to_table == "to-table" => {
            print_variant_to_table(arguments)
        }
        [variant, annotate, arguments @ ..] if variant == "variant" && annotate == "annotate" => {
            print_variant_annotate(arguments)
        }
        [interval, intersect, left, right]
            if interval == "interval" && intersect == "intersect" =>
        {
            print_interval_intersect(left, right, false)
        }
        [interval, intersect, left, right, json]
            if interval == "interval" && intersect == "intersect" && json == "--json" =>
        {
            print_interval_intersect(left, right, true)
        }
        [interval, merge, arguments @ ..] if interval == "interval" && merge == "merge" => {
            print_interval_merge(arguments)
        }
        [interval, subtract, arguments @ ..]
            if interval == "interval" && subtract == "subtract" =>
        {
            print_interval_subtract(arguments)
        }
        [interval, closest, arguments @ ..] if interval == "interval" && closest == "closest" => {
            print_interval_closest(arguments)
        }
        [expression, matrix_qc, path] if expression == "expression" && matrix_qc == "matrix-qc" => {
            print_expression_matrix_qc(path, false)
        }
        [expression, matrix_qc, path, json]
            if expression == "expression" && matrix_qc == "matrix-qc" && json == "--json" =>
        {
            print_expression_matrix_qc(path, true)
        }
        [medical, single_cell_qc, path]
            if medical == "medical" && single_cell_qc == "single-cell-qc" =>
        {
            print_single_cell_qc(path, false)
        }
        [medical, single_cell_qc, path, json]
            if medical == "medical" && single_cell_qc == "single-cell-qc" && json == "--json" =>
        {
            print_single_cell_qc(path, true)
        }
        [medical, cohort_qc, path] if medical == "medical" && cohort_qc == "cohort-qc" => {
            print_cohort_table_qc(path, false)
        }
        [medical, cohort_qc, path, json]
            if medical == "medical" && cohort_qc == "cohort-qc" && json == "--json" =>
        {
            print_cohort_table_qc(path, true)
        }
        [expression, normalize, arguments @ ..]
            if expression == "expression" && normalize == "normalize" =>
        {
            print_expression_normalize(arguments)
        }
        [expression, pca, arguments @ ..] if expression == "expression" && pca == "pca" => {
            print_expression_pca(arguments)
        }
        [expression, cluster, arguments @ ..]
            if expression == "expression" && cluster == "cluster" =>
        {
            print_expression_cluster(arguments)
        }
        [expression, heatmap, arguments @ ..]
            if expression == "expression" && heatmap == "heatmap" =>
        {
            print_expression_heatmap(arguments)
        }
        [expression, volcano, arguments @ ..]
            if expression == "expression" && volcano == "volcano" =>
        {
            print_expression_volcano(arguments)
        }
        [expression, wgcna, arguments @ ..] if expression == "expression" && wgcna == "wgcna" => {
            print_wgcna(arguments)
        }
        [set, venn, arguments @ ..] if set == "set" && venn == "venn" => {
            print_set_analysis(arguments, true)
        }
        [set, upset, arguments @ ..] if set == "set" && upset == "upset" => {
            print_set_analysis(arguments, false)
        }
        [enrichment, custom, arguments @ ..]
            if enrichment == "enrichment" && custom == "custom" =>
        {
            print_enrichment(
                arguments,
                EnrichmentKind::Custom,
                "enrichment.overrepresentation.v1",
            )
        }
        [medical, pathway, arguments @ ..] if medical == "medical" && pathway == "pathway" => {
            print_enrichment(arguments, EnrichmentKind::Custom, "medical.pathway-ruo.v1")
        }
        [enrichment, go, arguments @ ..] if enrichment == "enrichment" && go == "go" => {
            print_enrichment(arguments, EnrichmentKind::Go, "enrichment.go.v1")
        }
        [enrichment, kegg, arguments @ ..] if enrichment == "enrichment" && kegg == "kegg" => {
            print_enrichment(arguments, EnrichmentKind::Kegg, "enrichment.kegg.v1")
        }
        [enrichment, gsea, arguments @ ..] if enrichment == "enrichment" && gsea == "gsea" => {
            print_gsea(arguments)
        }
        [enrichment, visualize, arguments @ ..]
            if enrichment == "enrichment" && visualize == "visualize" =>
        {
            print_enrichment_visualization(arguments)
        }
        [similarity, blast_parse, arguments @ ..]
            if similarity == "similarity" && blast_parse == "blast-parse" =>
        {
            print_blast_parse(arguments)
        }
        [similarity, blast, arguments @ ..] if similarity == "similarity" && blast == "blast" => {
            print_local_blast(arguments)
        }
        [similarity, diamond, arguments @ ..]
            if similarity == "similarity" && diamond == "diamond" =>
        {
            print_local_diamond(arguments)
        }
        [similarity, hmmer, arguments @ ..] if similarity == "similarity" && hmmer == "hmmer" => {
            print_local_hmmer(arguments)
        }
        [motif, meme, arguments @ ..] if motif == "motif" && meme == "meme" => {
            print_meme(arguments)
        }
        [motif, logo, input, output, json]
            if motif == "motif" && logo == "logo" && json == "--json" =>
        {
            print_visualization_result(
                "motif-logo",
                "motif.visualize.v1",
                render_motif_logo_svg_path(input, output)?,
                true,
            )
        }
        [motif, logo, input, output] if motif == "motif" && logo == "logo" => {
            print_visualization_result(
                "motif-logo",
                "motif.visualize.v1",
                render_motif_logo_svg_path(input, output)?,
                false,
            )
        }
        [motif, mast, arguments @ ..] if motif == "motif" && mast == "mast" => {
            print_mast(arguments)
        }
        [comparative, synteny_plot, arguments @ ..]
            if comparative == "comparative" && synteny_plot == "synteny-plot" =>
        {
            print_synteny_plot(arguments)
        }
        [comparative, mcscanx, gene_positions, hits, output, json]
            if comparative == "comparative" && mcscanx == "mcscanx" && json == "--json" =>
        {
            print_native_tool_result(
                "mcscanx",
                "comparative.mcscanx.v1",
                run_mcscanx_path(gene_positions, hits, output)?,
                true,
            )
        }
        [comparative, mcscanx, gene_positions, hits, output]
            if comparative == "comparative" && mcscanx == "mcscanx" =>
        {
            print_native_tool_result(
                "mcscanx",
                "comparative.mcscanx.v1",
                run_mcscanx_path(gene_positions, hits, output)?,
                false,
            )
        }
        [comparative, kaks, alignment, output, method, json]
            if comparative == "comparative" && kaks == "kaks" && json == "--json" =>
        {
            print_native_tool_result(
                "kaks",
                "comparative.kaks.v1",
                run_kaks_path(alignment, output, method)?,
                true,
            )
        }
        [comparative, kaks, alignment, output, method]
            if comparative == "comparative" && kaks == "kaks" =>
        {
            print_native_tool_result(
                "kaks",
                "comparative.kaks.v1",
                run_kaks_path(alignment, output, method)?,
                false,
            )
        }
        [comparative, dotplot, arguments @ ..]
            if comparative == "comparative" && dotplot == "dotplot" =>
        {
            print_dotplot(arguments)
        }
        [rna, secondary_structure, arguments @ ..]
            if rna == "rna" && secondary_structure == "secondary-structure" =>
        {
            print_rnafold(arguments)
        }
        [similarity, rbh, arguments @ ..] if similarity == "similarity" && rbh == "rbh" => {
            print_reciprocal_best_hits(arguments)
        }
        [protein, properties, path] if protein == "protein" && properties == "properties" => {
            print_protein_properties(path, false)
        }
        [protein, properties, path, json]
            if protein == "protein" && properties == "properties" && json == "--json" =>
        {
            print_protein_properties(path, true)
        }
        [protein, domains, arguments @ ..] if protein == "protein" && domains == "domains" => {
            print_protein_domains(arguments)
        }
        [protein, domain_plot, arguments @ ..]
            if protein == "protein" && domain_plot == "domain-plot" =>
        {
            print_domain_architecture_plot(arguments)
        }
        [phylogeny, tree, arguments @ ..] if phylogeny == "phylogeny" && tree == "tree" => {
            print_phylogeny_tree(arguments)
        }
        [phylogeny, tree_plot, arguments @ ..]
            if phylogeny == "phylogeny" && tree_plot == "tree-plot" =>
        {
            print_phylogeny_tree_visualize(arguments)
        }
        [phylogeny, distance, arguments @ ..]
            if phylogeny == "phylogeny" && distance == "distance" =>
        {
            print_phylogeny_distance(arguments)
        }
        [phylogeny, iqtree, arguments @ ..] if phylogeny == "phylogeny" && iqtree == "iqtree" => {
            print_iqtree(arguments)
        }
        [msa, muscle, arguments @ ..] if msa == "msa" && muscle == "muscle" => {
            print_muscle(arguments)
        }
        [msa, trimal, arguments @ ..] if msa == "msa" && trimal == "trimal" => {
            print_trimal(arguments)
        }
        [protein, secondary_structure, arguments @ ..]
            if protein == "protein" && secondary_structure == "secondary-structure" =>
        {
            print_dssp(arguments)
        }
        [structure, pdb, arguments @ ..] if structure == "structure" && pdb == "pdb" => {
            print_pdb_summary(arguments)
        }
        [structure, mmcif_summary, arguments @ ..]
            if structure == "structure" && mmcif_summary == "mmcif-summary" =>
        {
            print_mmcif_summary(arguments)
        }
        [structure, sequence, arguments @ ..]
            if structure == "structure" && sequence == "sequence" =>
        {
            print_structure_sequence(arguments)
        }
        [structure, contact_map, arguments @ ..]
            if structure == "structure" && contact_map == "contact-map" =>
        {
            print_structure_contact_map(arguments)
        }
        [structure, geometry, arguments @ ..]
            if structure == "structure" && geometry == "geometry" =>
        {
            print_structure_geometry(arguments)
        }
        [structure, superpose, arguments @ ..]
            if structure == "structure" && superpose == "superpose" =>
        {
            print_structure_superposition(arguments)
        }
        _ => Err(usage().into()),
    }
}

fn print_capabilities(json: bool) -> Result<(), Box<dyn Error>> {
    let source = runtime_catalog_file("capabilities/catalog.json")?;
    let catalog = load_capability_catalog()?;
    if json {
        println!("{}", serde_json::to_string(&catalog)?);
        return Ok(());
    }
    println!("Available:");
    if let Some(capabilities) = catalog
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
    {
        for capability in capabilities.iter().filter(|capability| {
            capability.get("status").and_then(serde_json::Value::as_str) == Some("available")
        }) {
            if let Some(id) = capability.get("id").and_then(serde_json::Value::as_str) {
                println!("  {id}");
            }
        }
    }
    println!();
    println!("Run with --json for the complete catalog, including planned capabilities.");
    match source {
        Some(path) => println!("Catalog source: {} (runtime)", path.display()),
        None => println!("Catalog source: embedded snapshot"),
    }
    Ok(())
}

fn print_runtime_catalog(json: bool) -> Result<(), Box<dyn Error>> {
    let catalog = load_runtime_catalog()?;
    if json {
        println!("{}", serde_json::to_string(&catalog)?);
        return Ok(());
    }

    println!("Managed runtime providers (read-only catalog):");
    for provider in catalog.providers {
        let state = match provider.status {
            RuntimeProviderStatus::Cataloged => "cataloged",
            RuntimeProviderStatus::Installable => "installable",
            RuntimeProviderStatus::Deprecated => "deprecated",
        };
        let default = if provider.default { " [default]" } else { "" };
        println!(
            "  {}: {} via {} ({state}){default}",
            provider.runtime, provider.display_name, provider.manager
        );
    }
    println!("Installation is not implemented; environment.apply.v1 remains planned.");
    Ok(())
}

fn print_workflow_packs(json: bool) -> Result<(), Box<dyn Error>> {
    let source = runtime_catalog_file("workflows/catalog.json")?;
    let catalog = load_workflow_catalog()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&catalog.packs)?);
        return Ok(());
    }
    println!("Bundled workflow packs:");
    for pack in catalog.packs {
        println!(
            "{}\t{}\t{}\t{:?}\t{}",
            pack.id, pack.capability, pack.status, pack.runtime, pack.trust
        );
    }
    match source {
        Some(path) => println!("Catalog source: {} (runtime)", path.display()),
        None => println!("Catalog source: embedded snapshot"),
    }
    Ok(())
}

fn run_workflow_pack(
    pack_id: &str,
    request_path: &Path,
    result_path: &Path,
) -> Result<(), Box<dyn Error>> {
    let catalog = load_workflow_catalog()?;
    let pack = catalog
        .packs
        .iter()
        .find(|candidate| candidate.id == pack_id)
        .ok_or_else(|| format!("unknown workflow pack: {pack_id}"))?;
    if pack.status == "planned" || pack.status == "deprecated" {
        return Err(format!(
            "workflow pack {} is not runnable ({})",
            pack.id, pack.status
        )
        .into());
    }
    if !request_path.is_file() {
        return Err(format!(
            "workflow request does not exist: {}",
            request_path.display()
        )
        .into());
    }
    if result_path.exists() {
        return Err(format!(
            "refusing to overwrite workflow result: {}",
            result_path.display()
        )
        .into());
    }

    let root = workflow_root()?;
    let relative_manifest = pack
        .manifest
        .strip_prefix("workflows/")
        .ok_or("workflow catalog manifest must be below workflows/")?;
    let manifest_path = safe_pack_path(&root, relative_manifest)?;
    let pack_root = manifest_path
        .parent()
        .ok_or("workflow manifest has no parent")?;
    let manifest: WorkflowPackManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.schema_version != "2"
        || manifest.id != pack.id
        || manifest.runtime.kind != pack.runtime
    {
        return Err(format!(
            "workflow manifest identity mismatch: {}",
            manifest_path.display()
        )
        .into());
    }
    if !core_compatibility_matches(
        &manifest.runtime.core_compatibility,
        env!("CARGO_PKG_VERSION"),
    ) {
        return Err(format!(
            "workflow pack {} requires core {} but this build is {}",
            manifest.id,
            manifest.runtime.core_compatibility,
            env!("CARGO_PKG_VERSION")
        )
        .into());
    }
    verify_workflow_pack_files(pack_root, &manifest)?;
    let request: serde_json::Value = serde_json::from_slice(&fs::read(request_path)?)?;
    let request_identity = validate_workflow_request(&request, pack)?;

    let expected_arguments = ["--request", "{request}", "--result", "{result}"];
    if manifest
        .entrypoint
        .arguments
        .iter()
        .map(String::as_str)
        .ne(expected_arguments)
    {
        return Err(format!(
            "workflow pack {} has unsupported entrypoint arguments",
            pack.id
        )
        .into());
    }
    let entrypoint = safe_pack_path(pack_root, &manifest.entrypoint.path)?;
    let executable = workflow_executable(manifest.runtime.kind)?;
    let status = Command::new(&executable)
        .arg(&entrypoint)
        .arg("--request")
        .arg(request_path)
        .arg("--result")
        .arg(result_path)
        .current_dir(pack_root)
        .env("LINXIRA_BIO_CORE_VERSION", env!("CARGO_PKG_VERSION"))
        .status()?;

    if !result_path.is_file() {
        return Err(format!(
            "workflow pack {} exited with {} without writing a result envelope",
            pack.id, status
        )
        .into());
    }
    let result: serde_json::Value = serde_json::from_slice(&fs::read(result_path)?)?;
    validate_workflow_result(&result, &request_identity)?;
    println!("{}", serde_json::to_string(&result)?);
    if result.get("status").and_then(|value| value.as_str()) != Some("ok") {
        let diagnostic = result
            .get("diagnostics")
            .and_then(|value| value.as_array())
            .and_then(|diagnostics| diagnostics.first())
            .and_then(|first| first.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or("workflow result reported an error");
        return Err(format!("workflow failed: {diagnostic}").into());
    }
    Ok(())
}

fn load_workflow_catalog() -> Result<WorkflowCatalog, Box<dyn Error>> {
    let text = match runtime_catalog_file("workflows/catalog.json")? {
        Some(path) => fs::read_to_string(&path)?,
        None => WORKFLOW_CATALOG.to_owned(),
    };
    let catalog: WorkflowCatalog = serde_json::from_str(&text)?;
    validate_workflow_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_workflow_catalog(catalog: &WorkflowCatalog) -> Result<(), Box<dyn Error>> {
    if catalog.schema_version != "1" || catalog.packs.is_empty() {
        return Err("workflow catalog is invalid".into());
    }
    let mut pack_ids = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    for pack in &catalog.packs {
        if !pack_ids.insert(pack.id.as_str()) {
            return Err(format!("workflow catalog repeats pack id: {}", pack.id).into());
        }
        let mut pack_capabilities = BTreeSet::new();
        for capability in pack.capabilities() {
            if !pack_capabilities.insert(capability) {
                return Err(
                    format!("workflow pack {} repeats capability: {capability}", pack.id).into(),
                );
            }
            if !capabilities.insert(capability) {
                return Err(format!(
                    "workflow catalog assigns capability more than once: {capability}"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn workflow_root() -> Result<PathBuf, Box<dyn Error>> {
    let configured = env::var_os("LINXIRA_BIO_WORKFLOW_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("workflows"));
    let root = fs::canonicalize(&configured)?;
    if !root.is_dir() {
        return Err(format!("workflow root is not a directory: {}", root.display()).into());
    }
    Ok(root)
}

fn safe_pack_path(root: &Path, relative: &str) -> Result<PathBuf, Box<dyn Error>> {
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| component.as_os_str() == "..")
    {
        return Err(format!("workflow path escapes pack root: {relative}").into());
    }
    let resolved = fs::canonicalize(root.join(candidate))?;
    if resolved != root && !resolved.starts_with(root) {
        return Err(format!("workflow path escapes pack root: {relative}").into());
    }
    Ok(resolved)
}

fn verify_workflow_pack_files(
    pack_root: &Path,
    manifest: &WorkflowPackManifest,
) -> Result<(), Box<dyn Error>> {
    let mut declared = BTreeSet::new();
    for file in &manifest.files {
        if !declared.insert(file.path.clone()) {
            return Err(format!("workflow manifest repeats file path: {}", file.path).into());
        }
        let path = safe_pack_path(pack_root, &file.path)?;
        if !path.is_file() || sha256_file(&path)? != file.sha256.to_ascii_lowercase() {
            return Err(format!("workflow file verification failed: {}", file.path).into());
        }
    }
    for required in [
        &manifest.entrypoint.path,
        &manifest.runtime.dependency_lock.path,
    ] {
        if !declared.contains(required) {
            return Err(
                format!("workflow manifest does not declare required file: {required}").into(),
            );
        }
    }
    let lock_path = safe_pack_path(pack_root, &manifest.runtime.dependency_lock.path)?;
    if sha256_file(&lock_path)? != manifest.runtime.dependency_lock.sha256.to_ascii_lowercase() {
        return Err("workflow dependency lock hash does not match manifest".into());
    }
    Ok(())
}

fn validate_workflow_request(
    request: &serde_json::Value,
    pack: &WorkflowCatalogPack,
) -> Result<ValidatedWorkflowRequest, Box<dyn Error>> {
    let job_id = request.get("job_id").and_then(serde_json::Value::as_str);
    let capability = request
        .get("capability")
        .and_then(serde_json::Value::as_str);
    if request
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("2")
        || job_id.is_none_or(str::is_empty)
        || capability.is_none_or(|candidate| !pack.supports_capability(candidate))
    {
        return Err(
            "workflow request must be a schema v2 request for the selected capability".into(),
        );
    }
    Ok(ValidatedWorkflowRequest {
        job_id: job_id.expect("validated job id").to_owned(),
        capability: capability.expect("validated capability").to_owned(),
    })
}

fn validate_workflow_result(
    result: &serde_json::Value,
    request: &ValidatedWorkflowRequest,
) -> Result<(), Box<dyn Error>> {
    if result
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        != Some("2")
        || result.get("capability").and_then(serde_json::Value::as_str)
            != Some(request.capability.as_str())
        || result.get("job_id").and_then(serde_json::Value::as_str) != Some(request.job_id.as_str())
        || !matches!(
            result.get("status").and_then(serde_json::Value::as_str),
            Some("ok" | "error")
        )
    {
        return Err("workflow output is not a valid result envelope for the request".into());
    }
    if result
        .pointer("/provenance/core_version")
        .and_then(serde_json::Value::as_str)
        != Some(env!("CARGO_PKG_VERSION"))
    {
        return Err("workflow output provenance core version does not match this build".into());
    }
    Ok(())
}

fn workflow_executable(kind: WorkflowRuntimeKind) -> Result<String, Box<dyn Error>> {
    let (variable, fallback) = match kind {
        WorkflowRuntimeKind::Python => (
            "LINXIRA_BIO_WORKFLOW_PYTHON",
            if cfg!(windows) { "python" } else { "python3" },
        ),
        WorkflowRuntimeKind::R => ("LINXIRA_BIO_WORKFLOW_R", "Rscript"),
        WorkflowRuntimeKind::Java | WorkflowRuntimeKind::Native => {
            return Err("workflow runtime kind is not implemented by this runner".into());
        }
    };
    Ok(env::var(variable).unwrap_or_else(|_| fallback.to_owned()))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

fn print_doctor(json: bool) -> Result<(), Box<dyn Error>> {
    let audit = audit_environment()?;

    if json {
        let tools = [
            "rust",
            "uv",
            "pixi",
            "conda",
            "miniforge",
            "python",
            "r",
            "java",
            "samtools",
            "bcftools",
            "bedtools",
            "wsl-arch",
            "wsl-debian",
            "docker",
            "podman",
        ]
        .iter()
        .filter_map(|tool_id| audit.tools.iter().find(|tool| tool.id == *tool_id))
        .map(|tool| {
            let name = if tool.id == "rust" { "rustc" } else { &tool.id };
            serde_json::json!({
                "name": name,
                "available": tool.available,
                "version": tool.version,
            })
        })
        .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": "1",
                "product": "linxira-bio-sdk",
                "os": audit.platform.os,
                "arch": audit.platform.arch,
                "tools": tools,
            }))?
        );
    } else {
        print_audit_text("Linxira Bio SDK doctor", &audit);
    }
    Ok(())
}

fn print_environment_audit(json: bool) -> Result<(), Box<dyn Error>> {
    let audit = audit_environment()?;
    if json {
        print_analysis_json("environment-audit", "environment.audit.v1", audit)?;
    } else {
        print_audit_text("Linxira Bio environment audit", &audit);
    }
    Ok(())
}

fn print_environment_plan(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (profile, options, json) = parse_environment_plan_arguments(arguments)?;
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(&profile, &audit, &options)?;
    if json {
        print_analysis_json("environment-plan", "environment.plan.v1", plan)?;
    } else {
        print_plan_text(&plan);
    }
    Ok(())
}

fn print_environment_apply(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (profile, options, json) = parse_environment_plan_arguments(arguments)?;
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(&profile, &audit, &options)?;
    let apply_result = apply_environment(&plan)?;
    if json {
        print_analysis_json("environment-apply", "environment.apply.v1", apply_result)?;
    } else {
        print_apply_text(&apply_result);
    }
    Ok(())
}

fn print_apply_text(result: &ApplyResult) {
    println!("Linxira Bio environment apply");
    println!("  profile : {}", result.profile);
    println!(
        "  platform: {} ({})",
        result.platform.os, result.platform.family
    );
    println!();
    if !result.installed.is_empty() {
        println!("Installed:");
        for tool in &result.installed {
            let version = tool.version.as_deref().unwrap_or("unknown");
            println!(
                "  + {} ({}) -> {}",
                tool.display_name, tool.strategy, version
            );
        }
    }
    if !result.failed.is_empty() {
        println!("Failed:");
        for tool in &result.failed {
            println!(
                "  - {} ({}) -> {}",
                tool.display_name, tool.strategy, tool.reason
            );
        }
    }
    if result.installed.is_empty() && result.failed.is_empty() {
        println!("No tools to install. All required tools are already available.");
    }
    println!();
    println!(
        "Summary: {} installed, {} failed, {} skipped ({} total)",
        result.summary.installed,
        result.summary.failed,
        result.summary.skipped,
        result.summary.total
    );
}

fn parse_environment_plan_arguments(
    arguments: &[String],
) -> Result<(String, EnvironmentPlanOptions, bool), Box<dyn Error>> {
    let mut profile = None;
    let mut mode = EnvironmentMode::ManagedUser;
    let mut project_root = None;
    let mut json = false;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--mode" => {
                index += 1;
                let value = arguments.get(index).ok_or("--mode requires a value")?;
                mode = parse_environment_mode(value)?;
            }
            "--project-root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--project-root requires a path")?;
                project_root = Some(PathBuf::from(value));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown environment plan option: {value}").into());
            }
            value if profile.is_none() => profile = Some(value.to_owned()),
            value => return Err(format!("unexpected environment plan argument: {value}").into()),
        }
        index += 1;
    }

    if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
        return Err("--project-root is only valid with --mode project-isolated".into());
    }

    Ok((
        profile.unwrap_or_else(|| "full-local".to_owned()),
        EnvironmentPlanOptions { mode, project_root },
        json,
    ))
}

fn print_analysis_json<T>(job_id: &str, capability: &str, result: T) -> Result<(), Box<dyn Error>>
where
    T: serde::Serialize,
{
    let result = AnalysisResult::ok(job_id, capability, result, ExecutionMode::LocalCpu);
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn print_analysis_json_with_warnings<T>(
    job_id: &str,
    capability: &str,
    result: T,
    warnings: Vec<String>,
) -> Result<(), Box<dyn Error>>
where
    T: serde::Serialize,
{
    let mut envelope = AnalysisResult::ok(job_id, capability, result, ExecutionMode::LocalCpu);
    envelope.warnings = warnings;
    println!("{}", serde_json::to_string(&envelope)?);
    Ok(())
}

fn print_audit_text(title: &str, audit: &EnvironmentAudit) {
    println!("{title}");
    println!(
        "platform: {} {} ({})",
        audit.platform.family, audit.platform.arch, audit.platform.os
    );
    for tool in &audit.tools {
        let state = if tool.available {
            "available"
        } else {
            "not found"
        };
        let version = tool
            .version
            .as_deref()
            .map(|value| format!(" - {value}"))
            .unwrap_or_default();
        println!("{}: {state}{version}", tool.display_name);
    }
}

fn print_plan_text(plan: &EnvironmentPlan) {
    println!("Environment profile: {}", plan.profile);
    println!("mode: {}", plan.mode.as_str());
    if let Some(target_root) = &plan.target_root {
        println!("target: {target_root}");
    }
    println!("{}", plan.description);
    for action in &plan.actions {
        let state = match action.state {
            PlanActionState::Available => "available",
            PlanActionState::Install => "install",
            PlanActionState::Alternative => "alternative",
            PlanActionState::Missing => "missing",
            PlanActionState::Unsupported => "unsupported",
        };
        let method = action
            .strategy
            .as_deref()
            .map(|strategy| format!(" via {strategy}"))
            .unwrap_or_default();
        println!("{}: {state}{method}", action.display_name);
    }
    for warning in &plan.warnings {
        println!("warning: {warning}");
    }
    for blocker in &plan.transaction.blockers {
        println!("blocked: {blocker}");
    }
    if plan.requires_confirmation {
        println!("No changes were applied. This is a transaction preview only.");
    }
}

fn print_sequence_stats(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = fasta_stats_path(Path::new(path))?;

    if json {
        print_stats_json(&stats)?;
    } else {
        print_stats_text(&stats);
    }
    Ok(())
}

fn print_sequence_extract(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut identifiers = Vec::new();
    let mut regions = Vec::new();
    let mut strict = false;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--id" => {
                index += 1;
                identifiers.push(
                    arguments
                        .get(index)
                        .ok_or("--id requires a FASTA identifier")?
                        .clone(),
                );
            }
            "--region" => {
                index += 1;
                let region = arguments
                    .get(index)
                    .ok_or("--region requires ID:START-END")?;
                regions.push(parse_sequence_region_spec(region)?);
            }
            "--strict" => strict = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence extract option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence extract")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence extract")?;
    let summary = extract_fasta_path(
        input,
        output,
        &SequenceExtractOptions {
            identifiers,
            regions,
            strict,
        },
    )?;
    print_sequence_transform_result(
        "sequence-extract",
        "sequence.extract.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_filter(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = SequenceFilterOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-length" => {
                index += 1;
                options.min_length = parse_sequence_u64(arguments.get(index), "--min-length")?;
            }
            "--max-length" => {
                index += 1;
                options.max_length =
                    Some(parse_sequence_u64(arguments.get(index), "--max-length")?);
            }
            "--min-gc-percent" => {
                index += 1;
                options.min_gc_percent = Some(parse_sequence_percentage(
                    arguments.get(index),
                    "--min-gc-percent",
                )?);
            }
            "--max-gc-percent" => {
                index += 1;
                options.max_gc_percent = Some(parse_sequence_percentage(
                    arguments.get(index),
                    "--max-gc-percent",
                )?);
            }
            "--max-n-percent" => {
                index += 1;
                options.max_n_percent = Some(parse_sequence_percentage(
                    arguments.get(index),
                    "--max-n-percent",
                )?);
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence filter option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence filter")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence filter")?;
    let summary = filter_fasta_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-filter",
        "sequence.filter.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_reverse_complement(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence reverse-complement option: {value}").into());
            }
            value => assign_sequence_path(
                &mut input,
                &mut output,
                value,
                "sequence reverse-complement",
            )?,
        }
    }
    let (input, output) = require_sequence_paths(input, output, "sequence reverse-complement")?;
    let summary = reverse_complement_fasta_path(input, output)?;
    print_sequence_transform_result(
        "sequence-reverse-complement",
        "sequence.reverse-complement.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_translate(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut frames = Vec::new();
    let mut trim_terminal_stop = false;
    let mut stop_at_first = false;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--frame" => {
                index += 1;
                let value = arguments.get(index).ok_or("--frame requires a value")?;
                let frame = value
                    .parse::<i8>()
                    .map_err(|_| format!("--frame requires an integer, got {value:?}"))?;
                if !matches!(frame, -3..=-1 | 1..=3) {
                    return Err(format!(
                        "unsupported translation frame {frame}; expected -3, -2, -1, 1, 2, or 3"
                    )
                    .into());
                }
                frames.push(frame);
            }
            "--trim-terminal-stop" => trim_terminal_stop = true,
            "--stop-at-first" => stop_at_first = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence translate option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence translate")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence translate")?;
    let summary = translate_fasta_path(
        input,
        output,
        &SequenceTranslateOptions {
            frames: if frames.is_empty() { vec![1] } else { frames },
            trim_terminal_stop,
            stop_at_first,
        },
    )?;
    print_sequence_transform_result(
        "sequence-translate",
        "sequence.translate.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_orf(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = SequenceOrfOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-amino-acids" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--min-amino-acids requires a value")?;
                options.min_amino_acids = value.parse::<usize>().map_err(|_| {
                    format!("--min-amino-acids requires a positive integer, got {value:?}")
                })?;
                if options.min_amino_acids == 0 {
                    return Err("--min-amino-acids must be at least 1".into());
                }
            }
            "--forward-only" => options.include_reverse_strand = false,
            "--include-partial-3prime" => options.include_partial_3prime = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence orf option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence orf")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence orf")?;
    let summary = find_orfs_fasta_path(input, output, &options)?;
    print_sequence_transform_result("sequence-orf", "sequence.orf.v1", output, summary, json)
}

fn print_sequence_normalize_ids(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = SequenceIdNormalizeOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--prefix" => {
                index += 1;
                options.prefix = arguments
                    .get(index)
                    .ok_or("--prefix requires a value")?
                    .clone();
            }
            "--start" => {
                index += 1;
                options.start = parse_sequence_u64(arguments.get(index), "--start")?;
                if options.start == 0 {
                    return Err("--start must be at least 1".into());
                }
            }
            "--width" => {
                index += 1;
                let width = parse_sequence_usize(arguments.get(index), "--width")?;
                if width == 0 {
                    return Err("--width must be at least 1".into());
                }
                options.width = Some(width);
            }
            "--no-padding" => options.width = None,
            "--drop-description" => options.keep_description = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence normalize-ids option: {value}").into());
            }
            value => {
                assign_sequence_path(&mut input, &mut output, value, "sequence normalize-ids")?
            }
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence normalize-ids")?;
    let summary = normalize_fasta_ids_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-normalize-ids",
        "sequence.id.normalize.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_merge(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = SequenceMergeOptions::default();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--allow-duplicate-ids" => options.allow_duplicate_ids = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence merge option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() < 2 {
        return Err(
            "sequence merge requires an output FASTA followed by at least one input FASTA".into(),
        );
    }
    let output = paths.remove(0);
    let summary = merge_fasta_paths(&paths, &output, &options)?;
    print_sequence_transform_result(
        "sequence-merge",
        "sequence.merge.v1",
        &output,
        summary,
        json,
    )
}

fn print_sequence_split(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = SequenceSplitOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--records-per-file" => {
                index += 1;
                options.records_per_file =
                    parse_sequence_usize(arguments.get(index), "--records-per-file")?;
                if options.records_per_file == 0 {
                    return Err("--records-per-file must be at least 1".into());
                }
            }
            "--prefix" => {
                index += 1;
                options.prefix = arguments
                    .get(index)
                    .ok_or("--prefix requires a value")?
                    .clone();
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence split option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence split")?,
        }
        index += 1;
    }
    let input = input.ok_or("sequence split requires an input FASTA path")?;
    let output = output.ok_or("sequence split requires an output directory")?;
    let output = Path::new(output);
    let summary = split_fasta_path(Path::new(input), output, &options)?;
    print_sequence_transform_result("sequence-split", "sequence.split.v1", output, summary, json)
}

fn print_sequence_to_table(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut delimiter = None;
    let mut include_header = true;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--delimiter" => {
                index += 1;
                delimiter = Some(parse_sequence_table_delimiter(
                    arguments.get(index),
                    "--delimiter",
                )?);
            }
            "--no-header" => include_header = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence to-table option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence to-table")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence to-table")?;
    let delimiter = delimiter.unwrap_or_else(|| {
        SequenceTableDelimiter::infer_from_path(output).unwrap_or(SequenceTableDelimiter::Csv)
    });
    let summary = fasta_to_table_path(
        input,
        output,
        &SequenceToTableOptions {
            delimiter,
            include_header,
        },
    )?;
    print_sequence_transform_result(
        "sequence-to-table",
        "sequence.to-table.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_from_table(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = SequenceFromTableOptions::default();
    let mut delimiter = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--delimiter" => {
                index += 1;
                delimiter = Some(parse_sequence_table_delimiter(
                    arguments.get(index),
                    "--delimiter",
                )?);
            }
            "--id-column" => {
                index += 1;
                options.id_column = arguments
                    .get(index)
                    .ok_or("--id-column requires a value")?
                    .clone();
            }
            "--sequence-column" => {
                index += 1;
                options.sequence_column = arguments
                    .get(index)
                    .ok_or("--sequence-column requires a value")?
                    .clone();
            }
            "--description-column" => {
                index += 1;
                options.description_column = Some(
                    arguments
                        .get(index)
                        .ok_or("--description-column requires a value")?
                        .clone(),
                );
            }
            "--no-description-column" => options.description_column = None,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence from-table option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence from-table")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence from-table")?;
    options.delimiter = delimiter.unwrap_or_else(|| {
        SequenceTableDelimiter::infer_from_path(input).unwrap_or(SequenceTableDelimiter::Csv)
    });
    let summary = table_to_fasta_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-from-table",
        "sequence.from-table.v1",
        output,
        summary,
        json,
    )
}

fn assign_sequence_path<'a>(
    input: &mut Option<&'a str>,
    output: &mut Option<&'a str>,
    value: &'a str,
    command: &str,
) -> Result<(), Box<dyn Error>> {
    if input.is_none() {
        *input = Some(value);
    } else if output.is_none() {
        *output = Some(value);
    } else {
        return Err(format!("unexpected {command} argument: {value}").into());
    }
    Ok(())
}

fn require_sequence_paths<'a>(
    input: Option<&'a str>,
    output: Option<&'a str>,
    command: &str,
) -> Result<(&'a Path, &'a Path), Box<dyn Error>> {
    let input = input.ok_or_else(|| format!("{command} requires an input FASTA path"))?;
    let output = output.ok_or_else(|| format!("{command} requires an output FASTA path"))?;
    Ok((Path::new(input), Path::new(output)))
}

fn parse_sequence_u64(value: Option<&String>, option: &str) -> Result<u64, Box<dyn Error>> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse::<u64>()
        .map_err(|_| format!("{option} requires a non-negative integer, got {value:?}").into())
}

fn parse_positive_u64(value: Option<&String>, option: &str) -> Result<u64, Box<dyn Error>> {
    let parsed = parse_sequence_u64(value, option)?;
    if parsed == 0 {
        return Err(format!("{option} must be positive").into());
    }
    Ok(parsed)
}

fn parse_sequence_usize(value: Option<&String>, option: &str) -> Result<usize, Box<dyn Error>> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse::<usize>()
        .map_err(|_| format!("{option} requires a non-negative integer, got {value:?}").into())
}

fn parse_sequence_percentage(value: Option<&String>, option: &str) -> Result<f64, Box<dyn Error>> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    let percent = value
        .parse::<f64>()
        .map_err(|_| format!("{option} requires a number, got {value:?}"))?;
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(format!("{option} must be between 0 and 100").into());
    }
    Ok(percent)
}

fn parse_sequence_table_delimiter(
    value: Option<&String>,
    option: &str,
) -> Result<SequenceTableDelimiter, Box<dyn Error>> {
    match value
        .ok_or_else(|| format!("{option} requires a value"))?
        .as_str()
    {
        "csv" => Ok(SequenceTableDelimiter::Csv),
        "tsv" | "tab" => Ok(SequenceTableDelimiter::Tsv),
        value => Err(format!("{option} must be csv or tsv, got {value:?}").into()),
    }
}

fn print_sequence_transform_result<T>(
    job_id: &str,
    capability: &str,
    output: &Path,
    summary: T,
    json: bool,
) -> Result<(), Box<dyn Error>>
where
    T: serde::Serialize,
{
    if json {
        print_analysis_json(job_id, capability, summary)?;
    } else {
        println!("output\t{}", output.display());
        if let serde_json::Value::Object(fields) = serde_json::to_value(summary)? {
            for (name, value) in fields {
                let rendered = match value {
                    serde_json::Value::String(value) => value,
                    serde_json::Value::Number(value) => value.to_string(),
                    serde_json::Value::Bool(value) => value.to_string(),
                    serde_json::Value::Null => "null".to_owned(),
                    value => serde_json::to_string(&value)?,
                };
                println!("{name}\t{rendered}");
            }
        }
    }
    Ok(())
}

fn print_sequence_kmer_count(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = KmerCountOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--k" => {
                index += 1;
                options.k = parse_sequence_usize(arguments.get(index), "--k")?;
            }
            "--top-n" => {
                index += 1;
                options.top_n = parse_sequence_usize(arguments.get(index), "--top-n")?;
            }
            "--canonical" => options.canonical = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence kmer-count option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence kmer-count")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence kmer-count")?;
    let summary = count_kmers_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-kmer-count",
        "sequence.kmer.count.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_consensus(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = ConsensusOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--threshold" => {
                index += 1;
                let value = arguments.get(index).ok_or("--threshold requires a value")?;
                options.threshold = value
                    .parse::<f64>()
                    .map_err(|_| format!("--threshold requires a number, got {value:?}"))?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence consensus option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence consensus")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence consensus")?;
    let summary = consensus_from_alignment_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-consensus",
        "sequence.consensus.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_shuffle(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = ShuffleOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--seed" => {
                index += 1;
                options.seed = arguments
                    .get(index)
                    .ok_or("--seed requires a value")?
                    .parse()?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence shuffle option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "sequence shuffle")?,
        }
        index += 1;
    }
    let (input, output) = require_sequence_paths(input, output, "sequence shuffle")?;
    let summary = shuffle_sequences_path(input, output, &options)?;
    print_sequence_transform_result(
        "sequence-shuffle",
        "sequence.shuffle.v1",
        output,
        summary,
        json,
    )
}

fn print_sequence_convert(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut input_format = None;
    let mut output_format = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--input-format" => {
                index += 1;
                input_format =
                    Some(parse_sequence_format(arguments.get(index).ok_or(
                        "--input-format requires fasta, fastq, genbank, or embl",
                    )?)?);
            }
            "--output-format" => {
                index += 1;
                output_format =
                    Some(parse_sequence_format(arguments.get(index).ok_or(
                        "--output-format requires fasta, fastq, genbank, or embl",
                    )?)?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown sequence convert option: {value}").into());
            }
            value if input.is_none() => input = Some(value.to_owned()),
            value if output.is_none() => output = Some(value.to_owned()),
            value => return Err(format!("unexpected sequence convert argument: {value}").into()),
        }
        index += 1;
    }
    let input_path = Path::new(
        input
            .as_deref()
            .ok_or("sequence convert requires <input> <output>")?,
    );
    let output_path = Path::new(
        output
            .as_deref()
            .ok_or("sequence convert requires <input> <output>")?,
    );
    if !input_path.is_file() {
        return Err(format!(
            "sequence convert input does not exist: {}",
            input_path.display()
        )
        .into());
    }
    if output_path.exists() {
        return Err(format!(
            "refusing to overwrite sequence convert output: {}",
            output_path.display()
        )
        .into());
    }
    // The workflow pack creates output_directory itself and writes
    // output_filename inside it, so the directory must not exist yet while
    // its containing directory must exist. The pack runs with its pack root
    // as the working directory, so every request path must be absolute.
    let input_path = fs::canonicalize(input_path)?;
    let output_directory = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let containing = output_directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(output_directory);
    if !containing.is_dir() {
        return Err(format!(
            "sequence convert output parent directory does not exist: {}",
            containing.display()
        )
        .into());
    }
    if output_directory.exists() {
        return Err(format!(
            "sequence convert output directory must not already exist: {}",
            output_directory.display()
        )
        .into());
    }
    let output_directory = fs::canonicalize(containing)?
        .join(
            output_directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
        .to_string_lossy()
        .into_owned();
    let resolved_input_format = match input_format {
        Some(format) => format,
        None => sequence_format_from_path(&input_path)?,
    };
    let resolved_output_format = match output_format {
        Some(format) => format,
        None => sequence_format_from_path(output_path)?,
    };
    let output_filename = output_path
        .file_name()
        .ok_or("sequence convert output has no file name")?
        .to_string_lossy()
        .into_owned();
    let size_bytes = fs::metadata(&input_path)?.len();
    let request = serde_json::json!({
        "schema_version": "2",
        "job_id": "cli",
        "capability": "sequence.convert.biopython.v1",
        "inputs": [{
            "artifact_id": "sequences",
            "role": "sequences",
            "cardinality": "single",
            "files": [{
                "file_id": "input-sequences-1",
                "path": input_path.to_string_lossy(),
                "format": resolved_input_format,
                "compression": "none",
                "size_bytes": size_bytes,
            }],
        }],
        "execution": {"mode": "local-cpu"},
        "parameters": {
            "output_directory": output_directory,
            "output_filename": output_filename,
            "output_format": resolved_output_format,
        },
    });

    let temporary = tempfile::Builder::new()
        .prefix("linxira-bio-convert-")
        .tempdir()?;
    let request_path = temporary.path().join("request.json");
    // The workflow contract requires the result envelope at
    // <output_directory>/result.json; it remains as output metadata.
    let result_path = Path::new(&output_directory).join("result.json");
    fs::write(&request_path, serde_json::to_vec(&request)?)?;
    run_workflow_pack(
        "org.linxira.sequence-conversion-biopython",
        &request_path,
        &result_path,
    )
}

fn parse_sequence_format(value: &str) -> Result<String, Box<dyn Error>> {
    match value {
        "fasta" | "fastq" | "genbank" | "embl" => Ok(value.to_owned()),
        _ => Err(format!(
            "unsupported sequence format: {value} (expected fasta, fastq, genbank, or embl)"
        )
        .into()),
    }
}

fn sequence_format_from_path(path: &Path) -> Result<String, Box<dyn Error>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "fa" | "fasta" | "fna" => Ok("fasta".to_owned()),
        "fq" | "fastq" => Ok("fastq".to_owned()),
        "gb" | "gbk" | "genbank" => Ok("genbank".to_owned()),
        "embl" => Ok("embl".to_owned()),
        _ => Err(format!(
            "cannot infer a sequence format from extension: .{extension} \
             (use --input-format or --output-format)"
        )
        .into()),
    }
}

fn print_primer_epcr(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = EpcrOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-amplicon" => {
                index += 1;
                options.min_amplicon =
                    parse_sequence_usize(arguments.get(index), "--min-amplicon")?;
            }
            "--max-amplicon" => {
                index += 1;
                options.max_amplicon =
                    parse_sequence_usize(arguments.get(index), "--max-amplicon")?;
            }
            "--max-hits" => {
                index += 1;
                options.max_hits = parse_sequence_usize(arguments.get(index), "--max-hits")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown primer epcr option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err("primer epcr requires <reference.fasta> <primers.tsv> <output.tsv>".into());
    }
    let summary = epcr_path(&paths[0], &paths[1], &paths[2], &options)?;
    print_sequence_transform_result("primer-epcr", "primer.epcr.v1", &paths[2], summary, json)
}

fn print_fastq_qc(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut json = false;
    let mut options = FastqQcOptions::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--max-cycles" => {
                index += 1;
                options.max_cycles = arguments
                    .get(index)
                    .ok_or("--max-cycles requires a value")?
                    .parse()?;
            }
            "--quality-encoding" => {
                index += 1;
                options.quality_encoding = match arguments
                    .get(index)
                    .ok_or("--quality-encoding requires a value")?
                    .as_str()
                {
                    "auto" => QualityEncodingMode::Auto,
                    "phred+33" => QualityEncodingMode::Phred33,
                    "phred+64" => QualityEncodingMode::Phred64,
                    value => return Err(format!("unsupported quality encoding: {value}").into()),
                };
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ QC option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected FASTQ QC argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("fastq qc requires an input path")?;
    let metrics = fastq_qc_path(Path::new(path), options)?;
    if json {
        print_analysis_json("fastq-qc", "fastq.qc.v1", metrics)?;
    } else {
        print_fastq_qc_text(&metrics);
    }
    Ok(())
}

fn print_fastq_qc_text(metrics: &FastqQcMetrics) {
    println!("read_count\t{}", metrics.read_count);
    println!("total_bases\t{}", metrics.total_bases);
    println!("mean_length\t{:.6}", metrics.mean_length);
    println!("gc_percent\t{:.6}", metrics.gc_percent);
    println!("mean_quality\t{:.6}", metrics.mean_quality);
    println!("q20_percent\t{:.6}", metrics.q20_percent);
    println!("q30_percent\t{:.6}", metrics.q30_percent);
    println!("quality_encoding\t{:?}", metrics.quality_encoding);
    for warning in &metrics.warnings {
        println!("warning\t{warning}");
    }
}

fn print_fastq_trim(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = FastqTrimOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-quality" => {
                index += 1;
                options.min_quality = parse_u8(arguments.get(index), "--min-quality")?;
            }
            "--min-length" => {
                index += 1;
                options.min_length = parse_sequence_usize(arguments.get(index), "--min-length")?;
            }
            "--quality-encoding" => {
                index += 1;
                options.quality_encoding = parse_fastq_transform_quality_encoding(
                    arguments.get(index),
                    "--quality-encoding",
                )?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ trim option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "fastq trim")?,
        }
        index += 1;
    }
    let input = input.ok_or("fastq trim requires an input FASTQ path")?;
    let output = output.ok_or("fastq trim requires an output FASTQ path")?;
    let output = Path::new(output);
    let summary = fastq_trim_path(Path::new(input), output, &options)?;
    print_sequence_transform_result("fastq-trim", "fastq.trim.v1", output, summary, json)
}

fn print_fastq_adapter_trim(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = FastqAdapterOptions::default();
    let mut explicit_adapters = Vec::new();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--adapter" => {
                index += 1;
                explicit_adapters.push(
                    arguments
                        .get(index)
                        .ok_or("--adapter requires a sequence")?
                        .clone(),
                );
            }
            "--min-overlap" => {
                index += 1;
                options.min_overlap = parse_sequence_usize(arguments.get(index), "--min-overlap")?;
            }
            "--min-length" => {
                index += 1;
                options.min_length = parse_sequence_usize(arguments.get(index), "--min-length")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ adapter-trim option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "fastq adapter-trim")?,
        }
        index += 1;
    }
    if !explicit_adapters.is_empty() {
        options.adapters = explicit_adapters;
    }
    let input = input.ok_or("fastq adapter-trim requires an input FASTQ path")?;
    let output = output.ok_or("fastq adapter-trim requires an output FASTQ path")?;
    let output = Path::new(output);
    let summary = fastq_adapter_trim_path(Path::new(input), output, &options)?;
    print_sequence_transform_result(
        "fastq-adapter-trim",
        "fastq.adapter.v1",
        output,
        summary,
        json,
    )
}

fn print_fastq_deduplicate(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut key = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--header-umi-delimiter" => {
                index += 1;
                let delimiter = arguments
                    .get(index)
                    .ok_or("--header-umi-delimiter requires a value")?
                    .clone();
                if key.is_some() {
                    return Err("choose only one UMI source".into());
                }
                key = Some(FastqDeduplicateKey::HeaderUmi { delimiter });
            }
            "--sequence-prefix-umi" => {
                index += 1;
                let length = parse_sequence_usize(arguments.get(index), "--sequence-prefix-umi")?;
                if key.is_some() {
                    return Err("choose only one UMI source".into());
                }
                key = Some(FastqDeduplicateKey::SequencePrefixUmi { length });
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ deduplicate option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "fastq deduplicate")?,
        }
        index += 1;
    }
    let input = input.ok_or("fastq deduplicate requires an input FASTQ path")?;
    let output = output.ok_or("fastq deduplicate requires an output FASTQ path")?;
    let output = Path::new(output);
    let summary = fastq_deduplicate_path(
        Path::new(input),
        output,
        &FastqDeduplicateOptions {
            key: key.unwrap_or(FastqDeduplicateKey::Sequence),
        },
    )?;
    print_sequence_transform_result(
        "fastq-deduplicate",
        "fastq.deduplicate.v1",
        output,
        summary,
        json,
    )
}

fn print_fastq_subsample(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = FastqSubsampleOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--target-count" => {
                index += 1;
                options.target_count = Some(
                    arguments
                        .get(index)
                        .ok_or("--target-count requires a value")?
                        .parse()?,
                );
            }
            "--fraction" => {
                index += 1;
                options.fraction = Some(
                    arguments
                        .get(index)
                        .ok_or("--fraction requires a value")?
                        .parse()?,
                );
            }
            "--seed" => {
                index += 1;
                options.seed = arguments
                    .get(index)
                    .ok_or("--seed requires a value")?
                    .parse()?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown FASTQ subsample option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "fastq subsample")?,
        }
        index += 1;
    }
    let input = input.ok_or("fastq subsample requires an input FASTQ path")?;
    let output = output.ok_or("fastq subsample requires an output FASTQ path")?;
    let output = Path::new(output);
    let summary = fastq_subsample_path(Path::new(input), output, &options)?;
    print_sequence_transform_result(
        "fastq-subsample",
        "fastq.subsample.v1",
        output,
        summary,
        json,
    )
}

fn parse_fastq_transform_quality_encoding(
    value: Option<&String>,
    option: &str,
) -> Result<FastqTransformQualityEncoding, Box<dyn Error>> {
    match value
        .ok_or_else(|| format!("{option} requires a value"))?
        .as_str()
    {
        "phred+33" => Ok(FastqTransformQualityEncoding::Phred33),
        "phred+64" => Ok(FastqTransformQualityEncoding::Phred64),
        value => Err(format!("{option} must be phred+33 or phred+64, got {value:?}").into()),
    }
}

fn parse_u8(value: Option<&String>, option: &str) -> Result<u8, Box<dyn Error>> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse::<u8>()
        .map_err(|_| format!("{option} requires an integer from 0 to 255, got {value:?}").into())
}

fn print_variant_stats(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = vcf_stats_path(Path::new(path))?;
    if json {
        print_analysis_json("variant-stats", "variant.stats.v1", stats)?;
    } else {
        print_variant_stats_text(&stats);
    }
    Ok(())
}

fn print_medical_variant_cohort(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = vcf_stats_path(Path::new(path))?;
    if json {
        print_analysis_json("medical-variant-cohort", "medical.variant-cohort.v1", stats)?;
    } else {
        print_variant_stats_text(&stats);
    }
    Ok(())
}

fn print_variant_compare(left: &str, right: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let result = compare_vcf_paths(left, right)?;
    if json {
        print_analysis_json("variant-compare", "variant.compare.v1", result)?;
    } else {
        print_variant_compare_text(&result);
    }
    Ok(())
}

fn print_variant_compare_text(result: &VariantComparisonResult) {
    println!("shared_count\t{}", result.shared_count);
    println!("left_only_count\t{}", result.left_only_count);
    println!("right_only_count\t{}", result.right_only_count);
    println!(
        "sample_genotypes_compared\t{}",
        result.sample_genotypes_compared
    );
    for variant in &result.variants {
        println!(
            "variant\t{}\t{}\t{}\t{}\t{}",
            variant.chrom,
            variant.position,
            variant.reference,
            variant.alternate,
            variant.status.as_str()
        );
    }
}

fn print_variant_filter(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = VariantFilterOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-qual" => {
                index += 1;
                let value = arguments.get(index).ok_or("--min-qual requires a value")?;
                let quality = value
                    .parse::<f64>()
                    .map_err(|_| format!("--min-qual requires a number, got {value:?}"))?;
                if !quality.is_finite() {
                    return Err("--min-qual must be finite".into());
                }
                options.min_qual = Some(quality);
            }
            "--pass-only" => options.require_pass = true,
            "--contig" => {
                index += 1;
                options.contigs.push(
                    arguments
                        .get(index)
                        .ok_or("--contig requires a value")?
                        .clone(),
                );
            }
            "--min-info-dp" => {
                index += 1;
                options.min_info_dp =
                    Some(parse_sequence_u64(arguments.get(index), "--min-info-dp")?);
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown variant filter option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "variant filter")?,
        }
        index += 1;
    }
    let input = input.ok_or("variant filter requires an input VCF path")?;
    let output = output.ok_or("variant filter requires an output VCF path")?;
    let output = Path::new(output);
    let summary = filter_vcf_path(Path::new(input), output, &options)?;
    print_sequence_transform_result("variant-filter", "variant.filter.v1", output, summary, json)
}

fn print_variant_normalize(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown variant normalize option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 3 {
        return Err("variant normalize requires <input.vcf> <reference.fasta> <output.vcf>".into());
    }
    let summary = normalize_vcf_path(&paths[0], &paths[1], &paths[2])?;
    print_sequence_transform_result(
        "variant-normalize",
        "variant.normalize.v1",
        &paths[2],
        summary,
        json,
    )
}

fn print_variant_to_table(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown variant to-table option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "variant to-table")?,
        }
    }
    let input = input.ok_or("variant to-table requires an input VCF path")?;
    let output = output.ok_or("variant to-table requires an output TSV path")?;
    let output = Path::new(output);
    let summary = vcf_to_table_path(Path::new(input), output)?;
    print_sequence_transform_result(
        "variant-to-table",
        "variant.to-table.v1",
        output,
        summary,
        json,
    )
}

fn print_annotation_stats(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation stats option: {value}").into());
            }
            value if input.is_none() => input = Some(value),
            value => return Err(format!("unexpected annotation stats argument: {value}").into()),
        }
    }
    let input = input.ok_or("annotation stats requires an input GFF3 or GTF path")?;
    let stats = annotation_stats_path(input)?;
    if json {
        print_analysis_json("annotation-stats", "annotation.gxf.stats.v1", stats)?;
    } else {
        print_annotation_stats_text(&stats);
    }
    Ok(())
}

fn print_annotation_stats_text(stats: &AnnotationStats) {
    println!("record_count\t{}", stats.record_count);
    println!("directive_count\t{}", stats.directive_count);
    println!("sequence_region_count\t{}", stats.sequence_region_count);
    println!("records_with_id\t{}", stats.records_with_id);
    println!("records_with_parent\t{}", stats.records_with_parent);
    println!(
        "feature_type_counts\t{}",
        serde_json::to_string(&stats.feature_type_counts).unwrap_or_default()
    );
    println!(
        "sequence_counts\t{}",
        serde_json::to_string(&stats.sequence_counts).unwrap_or_default()
    );
    for warning in &stats.warnings {
        println!("warning\t{warning}");
    }
}

fn print_annotation_normalize(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = AnnotationNormalizeOptions::default();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--sort" => options.sort = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation normalize option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "annotation normalize")?,
        }
    }
    let input = input.ok_or("annotation normalize requires an input GFF3 or GTF path")?;
    let output = output.ok_or("annotation normalize requires an output GFF3 path")?;
    let summary = normalize_annotation_path(input, output, options)?;
    print_sequence_transform_result(
        "annotation-normalize",
        "annotation.gxf.normalize.v1",
        Path::new(output),
        summary,
        json,
    )
}

fn print_annotation_positions(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut feature_types = Vec::new();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--feature-type" => {
                index += 1;
                feature_types.push(
                    arguments
                        .get(index)
                        .ok_or("--feature-type requires a value")?
                        .clone(),
                );
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation positions option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "annotation positions")?,
        }
        index += 1;
    }
    let input = input.ok_or("annotation positions requires an input GFF3 or GTF path")?;
    let output = output.ok_or("annotation positions requires an output TSV path")?;
    let options = GenePositionOptions {
        feature_types: if feature_types.is_empty() {
            GenePositionOptions::default().feature_types
        } else {
            feature_types
        },
    };
    let summary = annotation_gene_positions_path(input, output, &options)?;
    print_sequence_transform_result(
        "annotation-positions",
        "annotation.gene-position.v1",
        Path::new(output),
        summary,
        json,
    )
}

fn print_annotation_to_bed(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut feature_types = Vec::new();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--feature-types" => {
                index += 1;
                let raw = arguments
                    .get(index)
                    .ok_or("--feature-types requires a comma-separated list")?;
                feature_types = raw.split(',').map(|s| s.trim().to_owned()).collect();
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation to-bed option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "annotation to-bed")?,
        }
        index += 1;
    }
    let input = input.ok_or("annotation to-bed requires an input GFF3 or GTF path")?;
    let output = output.ok_or("annotation to-bed requires an output BED path")?;
    let feature_types = if feature_types.is_empty() {
        vec!["gene".to_owned()]
    } else {
        feature_types
    };
    let summary = gxf_to_bed_path(input, output, &feature_types)?;
    print_sequence_transform_result(
        "annotation-to-bed",
        "annotation.gxf.to-bed.v1",
        Path::new(output),
        summary,
        json,
    )
}

fn print_annotation_extract(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = AnnotationExtractOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--feature-type" => {
                index += 1;
                options.feature_type = arguments
                    .get(index)
                    .ok_or("--feature-type requires a value")?
                    .clone();
            }
            "--promoter-length" => {
                index += 1;
                options.promoter_length =
                    parse_sequence_u64(arguments.get(index), "--promoter-length")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation extract option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "annotation extract requires <annotation.gff3|gtf> <reference.fasta> <output.fasta>"
                .into(),
        );
    }
    let summary = extract_annotation_sequences_path(&paths[0], &paths[1], &paths[2], &options)?;
    print_sequence_transform_result(
        "annotation-extract",
        "annotation.sequence.extract.v1",
        &paths[2],
        summary,
        json,
    )
}

fn print_gene_density(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut feature_types = Vec::new();
    let mut options = GeneDensityOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--feature-type" => {
                index += 1;
                feature_types.push(
                    arguments
                        .get(index)
                        .ok_or("--feature-type requires a value")?
                        .clone(),
                );
            }
            "--window-size" => {
                index += 1;
                options.window_size = parse_positive_u64(arguments.get(index), "--window-size")?;
            }
            "--step-size" => {
                index += 1;
                options.step_size = parse_positive_u64(arguments.get(index), "--step-size")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation gene-density option: {value}").into());
            }
            value if input.is_none() => input = Some(value),
            value => {
                return Err(format!("unexpected annotation gene-density argument: {value}").into());
            }
        }
        index += 1;
    }
    if !feature_types.is_empty() {
        options.feature_types = feature_types;
    }
    let input = input.ok_or("annotation gene-density requires an input GFF3 or GTF path")?;
    let result = gene_density_path(input, options)?;
    if json {
        print_analysis_json_with_warnings(
            "annotation-gene-density",
            "genome.gene-density.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_gene_density_text(&result);
        Ok(())
    }
}

fn print_gene_density_text(result: &GeneDensityResult) {
    println!("input_record_count\t{}", result.input_record_count);
    println!("selected_feature_count\t{}", result.selected_feature_count);
    println!("sequence_count\t{}", result.sequence_count);
    println!("feature_types\t{}", result.feature_types.join(","));
    println!("window_size\t{}", result.window_size);
    println!("step_size\t{}", result.step_size);
    for bin in &result.bins {
        println!(
            "bin\t{}\t{}\t{}\t{}\t{:.6}",
            bin.seqid, bin.start, bin.end, bin.feature_count, bin.features_per_megabase
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_go_annotations(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = GoAnnotationOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--gene-column" => {
                index += 1;
                options.gene_column = Some(
                    arguments
                        .get(index)
                        .ok_or("--gene-column requires a value")?
                        .clone(),
                );
            }
            "--go-column" => {
                index += 1;
                options.go_column = Some(
                    arguments
                        .get(index)
                        .ok_or("--go-column requires a value")?
                        .clone(),
                );
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation go option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("annotation go requires <input.csv|tsv> <output.tsv>".into());
    }
    let result = normalize_go_annotations_path(&paths[0], &paths[1], &options)?;
    if json {
        print_analysis_json_with_warnings(
            "annotation-go",
            "annotation.go.normalize.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_annotation_map_text(&result);
        Ok(())
    }
}

fn print_annotation_map_text(result: &AnnotationMapResult) {
    println!("input_row_count\t{}", result.input_row_count);
    println!("gene_count\t{}", result.gene_count);
    println!("term_count\t{}", result.term_count);
    println!("association_count\t{}", result.association_count);
    println!("output_path\t{}", result.output_path);
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_eggnog_annotations(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation eggnog option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 2 {
        return Err("annotation eggnog requires <input.tsv> <output.tsv>".into());
    }
    let result = normalize_eggnog_path(&paths[0], &paths[1])?;
    if json {
        print_analysis_json_with_warnings(
            "annotation-eggnog",
            "annotation.eggnog.normalize.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_eggnog_text(&result);
        Ok(())
    }
}

fn print_eggnog_text(result: &EggnogNormalizeResult) {
    println!("input_row_count\t{}", result.input_row_count);
    println!("query_count\t{}", result.query_count);
    println!("go_association_count\t{}", result.go_association_count);
    println!("kegg_association_count\t{}", result.kegg_association_count);
    println!("output_path\t{}", result.output_path);
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_enrichment(
    arguments: &[String],
    kind: EnrichmentKind,
    capability: &str,
) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = EnrichmentOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-overlap" => {
                index += 1;
                options.min_overlap = parse_positive_u64(arguments.get(index), "--min-overlap")?;
            }
            "--max-terms" => {
                index += 1;
                options.max_terms = parse_sequence_usize(arguments.get(index), "--max-terms")?;
            }
            "--include-genes" => options.include_genes = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown enrichment option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("enrichment requires <query-genes.txt|csv|tsv> <associations.csv|tsv>".into());
    }
    let result = overrepresentation_path(&paths[0], &paths[1], kind, options)?;
    if json {
        print_analysis_json_with_warnings(
            &format!("enrichment-{}", kind.as_str()),
            capability,
            result.clone(),
            result.warnings,
        )
    } else {
        print_enrichment_text(&result);
        Ok(())
    }
}

fn print_enrichment_text(result: &EnrichmentResult) {
    println!("analysis_type\t{}", result.analysis_type);
    println!("query_input_count\t{}", result.query_input_count);
    println!("query_mapped_count\t{}", result.query_mapped_count);
    println!("background_gene_count\t{}", result.background_gene_count);
    println!("tested_term_count\t{}", result.tested_term_count);
    for term in &result.terms {
        println!(
            "term\t{}\t{}\t{}\t{:.6e}\t{:.6e}\t{:.6}",
            term.term_id,
            term.term_name.as_deref().unwrap_or_default(),
            term.overlap_count,
            term.p_value,
            term.adjusted_p_value,
            term.fold_enrichment
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_gsea(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = GseaOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--score-exponent" => {
                index += 1;
                options.score_exponent =
                    parse_finite_f64(arguments.get(index), "--score-exponent", Some(0.0))?;
            }
            "--min-set-size" => {
                index += 1;
                options.min_set_size =
                    parse_sequence_usize(arguments.get(index), "--min-set-size")?;
            }
            "--max-set-size" => {
                index += 1;
                options.max_set_size =
                    parse_sequence_usize(arguments.get(index), "--max-set-size")?;
            }
            "--permutations" => {
                index += 1;
                options.permutation_count =
                    parse_sequence_u64(arguments.get(index), "--permutations")?
                        .try_into()
                        .map_err(|_| "--permutations exceeds the supported integer range")?;
            }
            "--seed" => {
                index += 1;
                options.seed = parse_sequence_u64(arguments.get(index), "--seed")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown GSEA option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("GSEA requires <ranked-genes.csv|tsv> <gene-sets.csv|tsv>".into());
    }
    let result = gsea_preranked_path(&paths[0], &paths[1], options)?;
    if json {
        print_analysis_json_with_warnings(
            "enrichment-gsea",
            "enrichment.gsea.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_gsea_text(&result);
        Ok(())
    }
}

fn print_gsea_text(result: &GseaResult) {
    println!("ranked_gene_count\t{}", result.ranked_gene_count);
    println!("input_gene_set_count\t{}", result.input_gene_set_count);
    println!("tested_gene_set_count\t{}", result.tested_gene_set_count);
    for term in &result.terms {
        println!(
            "term\t{}\t{}\t{:.8}\t{}\t{:.8}\t{:.8}",
            term.term_id,
            term.term_name.as_deref().unwrap_or_default(),
            term.enrichment_score,
            term.direction,
            term.nominal_p_value,
            term.fdr_bh
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_annotation_structure_plot(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = AnnotationStructureOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--feature-id" => {
                index += 1;
                options.feature_id = Some(
                    arguments
                        .get(index)
                        .ok_or("--feature-id requires a value")?
                        .clone(),
                );
            }
            "--seqid" => {
                index += 1;
                options.seqid = Some(
                    arguments
                        .get(index)
                        .ok_or("--seqid requires a value")?
                        .clone(),
                );
            }
            "--max-features" => {
                index += 1;
                options.max_features =
                    parse_sequence_usize(arguments.get(index), "--max-features")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown annotation plot option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("annotation plot requires <input.gff3|gtf> <output.svg>".into());
    }
    if options.feature_id.is_some() && options.seqid.is_some() {
        return Err("--feature-id and --seqid are mutually exclusive".into());
    }
    let result = render_annotation_structure_svg_path(&paths[0], &paths[1], &options)?;
    print_visualization_result(
        "annotation-structure-visualization",
        "annotation.structure.visualize.v1",
        result,
        json,
    )
}

fn print_domain_architecture_plot(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = DomainArchitectureOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--sequence-id" => {
                index += 1;
                options.sequence_id = Some(
                    arguments
                        .get(index)
                        .ok_or("--sequence-id requires a value")?
                        .clone(),
                );
            }
            "--max-sequences" => {
                index += 1;
                options.max_sequences =
                    parse_sequence_usize(arguments.get(index), "--max-sequences")?;
            }
            "--max-domains" => {
                index += 1;
                options.max_domains = parse_sequence_usize(arguments.get(index), "--max-domains")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown protein domain-plot option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err(
            "protein domain-plot requires <interproscan.tsv|hmmer.domtblout> <output.svg>".into(),
        );
    }
    let result = render_domain_architecture_svg_path(&paths[0], &paths[1], &options)?;
    print_visualization_result(
        "protein-domain-visualization",
        "protein.domain.visualize.v1",
        result,
        json,
    )
}

fn print_enrichment_visualization(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut kind = None;
    let mut analysis_options = EnrichmentOptions::default();
    let mut visualization_options = EnrichmentVisualizationOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--kind" => {
                index += 1;
                kind = Some(match arguments.get(index).map(String::as_str) {
                    Some("custom") => EnrichmentKind::Custom,
                    Some("go") => EnrichmentKind::Go,
                    Some("kegg") => EnrichmentKind::Kegg,
                    Some(value) => {
                        return Err(format!("unsupported enrichment kind: {value}").into());
                    }
                    None => return Err("--kind requires custom, go, or kegg".into()),
                });
            }
            "--style" => {
                index += 1;
                visualization_options.style = match arguments.get(index).map(String::as_str) {
                    Some("bar") => EnrichmentPlotStyle::Bar,
                    Some("dot") => EnrichmentPlotStyle::Dot,
                    Some("network") => EnrichmentPlotStyle::Network,
                    Some(value) => return Err(format!("unsupported plot style: {value}").into()),
                    None => return Err("--style requires bar, dot, or network".into()),
                };
            }
            "--min-overlap" => {
                index += 1;
                analysis_options.min_overlap =
                    parse_positive_u64(arguments.get(index), "--min-overlap")?;
            }
            "--max-terms" => {
                index += 1;
                visualization_options.max_terms =
                    parse_sequence_usize(arguments.get(index), "--max-terms")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown enrichment visualize option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "enrichment visualize requires <genes.txt> <associations.tsv> <output.svg>".into(),
        );
    }
    let kind = kind.ok_or("enrichment visualize requires --kind custom|go|kegg")?;
    let result = render_enrichment_svg_path(
        &paths[0],
        &paths[1],
        &paths[2],
        kind,
        analysis_options,
        visualization_options,
    )?;
    print_visualization_result(
        "enrichment-visualization",
        "enrichment.visualize.v1",
        result,
        json,
    )
}

fn print_synteny_plot(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut style = SyntenyPlotStyle::Dual;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--style" => {
                index += 1;
                let value = arguments.get(index).ok_or("--style requires a value")?;
                style = SyntenyPlotStyle::parse(value)?;
            }
            "--json" => json = true,
            value if value.starts_with("--") => {
                return Err(format!("unknown synteny option: {value}").into());
            }
            _ => paths.push(arguments[index].clone()),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("comparative synteny-plot requires <anchors.tsv> <output.svg>".into());
    }
    print_visualization_result(
        "synteny-visualization",
        "comparative.synteny.visualize.v1",
        render_synteny_svg_with_options_path(
            &paths[0],
            &paths[1],
            &SyntenyVisualizationOptions { style },
        )?,
        json,
    )
}

fn print_visualization_result(
    job_id: &str,
    capability: &str,
    result: SvgVisualizationResult,
    json: bool,
) -> Result<(), Box<dyn Error>> {
    if json {
        let warnings = result.warnings.clone();
        print_analysis_json_with_warnings(job_id, capability, result, warnings)
    } else {
        println!("visualization_type\t{}", result.visualization_type);
        println!("output_path\t{}", result.output_path);
        println!("width\t{}", result.width);
        println!("height\t{}", result.height);
        println!("track_count\t{}", result.track_count);
        println!("glyph_count\t{}", result.glyph_count);
        for warning in result.warnings {
            println!("warning\t{warning}");
        }
        Ok(())
    }
}

fn print_variant_stats_text(stats: &VcfStats) {
    println!("record_count\t{}", stats.record_count);
    println!("sample_count\t{}", stats.sample_count);
    println!("pass_record_count\t{}", stats.pass_record_count);
    println!("filtered_record_count\t{}", stats.filtered_record_count);
    println!("snp_count\t{}", stats.snp_count);
    println!("indel_count\t{}", stats.indel_count);
    println!(
        "multiallelic_record_count\t{}",
        stats.multiallelic_record_count
    );
    if let Some(ratio) = stats.ti_tv_ratio {
        println!("ti_tv_ratio\t{ratio:.6}");
    }
    for warning in &stats.warnings {
        println!("warning\t{warning}");
    }
}

fn print_samtools_report(
    arguments: &[String],
    mode: &str,
    capability: &str,
) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut reference = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--reference" => {
                index += 1;
                reference = Some(PathBuf::from(
                    arguments.get(index).ok_or("--reference requires a value")?,
                ));
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown samtools {mode} option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err(format!("alignment {} requires <input.bam|cram> <output.tsv>", mode).into());
    }
    let result = run_samtools_report_path(&paths[0], reference.as_deref(), &paths[1], mode)?;
    print_native_tool_result(
        if mode == "stats" {
            "bam-cram-qc"
        } else {
            "alignment-coverage"
        },
        capability,
        result,
        json,
    )
}

fn print_bam_to_bigwig(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut threads = 1;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--threads" => {
                index += 1;
                threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown bam-to-bigwig option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("alignment bam-to-bigwig requires <input.bam|cram> <output.bw>".into());
    }
    print_native_tool_result(
        "bam-to-bigwig",
        "alignment.bam-to-bigwig.v1",
        run_bam_to_bigwig_path(&paths[0], &paths[1], threads)?,
        json,
    )
}

fn print_short_read_alignment(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = ShortReadAlignmentOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown short-read alignment option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "alignment short-read requires <reference.fasta> <reads.fastq> <output.bam>".into(),
        );
    }
    let result = run_short_read_alignment_path(&paths[0], &paths[1], &paths[2], &options)?;
    print_native_tool_result(
        "short-read-alignment",
        "alignment.short-read.v1",
        result,
        json,
    )
}

fn print_long_read_alignment(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = Minimap2LongReadOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--preset" => {
                index += 1;
                options.preset = parse_minimap2_preset(
                    arguments.get(index).ok_or("--preset requires a value")?,
                )?;
            }
            "--secondary" => options.secondary = true,
            "--max-secondary" => {
                index += 1;
                options.max_secondary =
                    parse_sequence_usize(arguments.get(index), "--max-secondary")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown long-read alignment option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "alignment long-read requires <reference.fasta> <reads.fastq> <output.sam>".into(),
        );
    }
    let result = run_minimap2_long_read_path(&paths[0], &paths[1], &paths[2], &options)?;
    print_native_tool_result(
        "long-read-alignment",
        "alignment.long-read.v1",
        result,
        json,
    )
}

fn print_variant_annotate(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = SnpEffOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--database" => {
                index += 1;
                options.database = arguments
                    .get(index)
                    .ok_or("--database requires a value")?
                    .clone();
            }
            "--upstream-downstream" => {
                index += 1;
                options.upstream_downstream = Some(parse_sequence_usize(
                    arguments.get(index),
                    "--upstream-downstream",
                )?);
            }
            "--no-stats" => options.no_stats = true,
            "--no-log" => options.no_log = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown variant annotate option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("variant annotate requires <input.vcf> <output.vcf>".into());
    }
    let result = run_snpeff_path(&paths[0], &paths[1], &options)?;
    print_native_tool_result("variant-annotate", "variant.annotate.v1", result, json)
}

fn print_alignment_qc(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let metrics = sam_qc_path(Path::new(path))?;
    if json {
        print_analysis_json("alignment-qc", "alignment.qc.v1", metrics)?;
    } else {
        print_alignment_qc_text(&metrics);
    }
    Ok(())
}

fn print_alignment_qc_text(metrics: &SamQcMetrics) {
    println!("record_count\t{}", metrics.record_count);
    println!("primary_record_count\t{}", metrics.primary_record_count);
    println!("mapped_record_count\t{}", metrics.mapped_record_count);
    println!("unmapped_record_count\t{}", metrics.unmapped_record_count);
    if let Some(percent) = metrics.mapped_percent {
        println!("mapped_percent\t{percent:.6}");
    }
    println!("duplicate_record_count\t{}", metrics.duplicate_record_count);
    println!("zero_mapq_record_count\t{}", metrics.zero_mapq_record_count);
    if let Some(mean) = metrics.mean_mapq {
        println!("mean_mapq\t{mean:.6}");
    }
    for warning in &metrics.warnings {
        println!("warning\t{warning}");
    }
}

fn print_interval_intersect(left: &str, right: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let stats = bed_intersect_path(Path::new(left), Path::new(right))?;
    if json {
        print_analysis_json("interval-intersect", "interval.intersect.v1", stats)?;
    } else {
        print_interval_intersect_text(&stats);
    }
    Ok(())
}

fn print_interval_intersect_text(stats: &IntervalIntersectStats) {
    println!("left_interval_count\t{}", stats.left_interval_count);
    println!("right_interval_count\t{}", stats.right_interval_count);
    println!("overlap_pair_count\t{}", stats.overlap_pair_count);
    println!("left_overlapped_count\t{}", stats.left_overlapped_count);
    println!("right_overlapped_count\t{}", stats.right_overlapped_count);
    println!("total_overlap_bases\t{}", stats.total_overlap_bases);
    for warning in &stats.warnings {
        println!("warning\t{warning}");
    }
}

fn print_interval_merge(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = IntervalMergeOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--max-gap" => {
                index += 1;
                options.max_gap = parse_sequence_u64(arguments.get(index), "--max-gap")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown interval merge option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "interval merge")?,
        }
        index += 1;
    }
    let input = input.ok_or("interval merge requires an input BED path")?;
    let output = output.ok_or("interval merge requires an output BED path")?;
    let output = Path::new(output);
    let stats = bed_merge_path(Path::new(input), output, options)?;
    print_sequence_transform_result("interval-merge", "interval.merge.v1", output, stats, json)
}

fn print_interval_subtract(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown interval subtract option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 3 {
        return Err("interval subtract requires <left.bed> <right.bed> <output.bed>".into());
    }
    let stats = bed_subtract_path(&paths[0], &paths[1], &paths[2])?;
    print_sequence_transform_result(
        "interval-subtract",
        "interval.subtract.v1",
        &paths[2],
        stats,
        json,
    )
}

fn print_interval_closest(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown interval closest option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 3 {
        return Err("interval closest requires <query.bed> <target.bed> <output.tsv>".into());
    }
    let stats = bed_closest_path(&paths[0], &paths[1], &paths[2])?;
    print_sequence_transform_result(
        "interval-closest",
        "interval.closest.v1",
        &paths[2],
        stats,
        json,
    )
}

fn print_expression_matrix_qc(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let metrics = expression_matrix_qc_path(Path::new(path))?;
    if json {
        print_analysis_json("expression-matrix-qc", "expression.matrix.qc.v1", metrics)?;
    } else {
        print_expression_matrix_qc_text(&metrics);
    }
    Ok(())
}

fn print_expression_matrix_qc_text(metrics: &ExpressionMatrixQc) {
    println!("feature_count\t{}", metrics.feature_count);
    println!("sample_count\t{}", metrics.sample_count);
    println!("numeric_value_count\t{}", metrics.numeric_value_count);
    println!("missing_value_count\t{}", metrics.missing_value_count);
    println!("zero_value_count\t{}", metrics.zero_value_count);
    println!("negative_value_count\t{}", metrics.negative_value_count);
    if let Some(percent) = metrics.zero_percent {
        println!("zero_percent\t{percent:.6}");
    }
    for warning in &metrics.warnings {
        println!("warning\t{warning}");
    }
}

fn print_single_cell_qc(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let metrics = expression_matrix_qc_path(Path::new(path))?;
    if json {
        print_analysis_json("single-cell-qc", "medical.single-cell-qc.v1", metrics)?;
    } else {
        print_expression_matrix_qc_text(&metrics);
    }
    Ok(())
}

fn print_cohort_table_qc(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let metrics = cohort_table_qc_path(Path::new(path))?;
    if json {
        print_analysis_json("cohort-table-qc", "medical.cohort-table.qc.v1", metrics)?;
    } else {
        print_cohort_table_qc_text(&metrics);
    }
    Ok(())
}

fn print_cohort_table_qc_text(metrics: &CohortTableQc) {
    println!("row_count\t{}", metrics.row_count);
    println!("column_count\t{}", metrics.column_count);
    println!("duplicate_row_count\t{}", metrics.duplicate_row_count);
    for column in &metrics.columns {
        println!(
            "column\t{}\tmissing={}\tdistinct={}",
            column.column, column.missing_count, column.distinct_value_count
        );
    }
    for warning in &metrics.warnings {
        println!("warning\t{warning}");
    }
}

fn print_expression_normalize(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = ExpressionNormalizeOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--method" => {
                index += 1;
                options.method = parse_expression_normalization_method(
                    arguments.get(index).ok_or("--method requires a value")?,
                )?;
            }
            "--pseudocount" => {
                index += 1;
                options.pseudocount =
                    parse_finite_f64(arguments.get(index), "--pseudocount", Some(0.0))?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown expression normalize option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "expression normalize")?,
        }
        index += 1;
    }
    let input = input.ok_or("expression normalize requires an input matrix path")?;
    let output = output.ok_or("expression normalize requires an output TSV path")?;
    let output = Path::new(output);
    let summary = normalize_expression_matrix_path(Path::new(input), output, &options)?;
    print_sequence_transform_result(
        "expression-normalize",
        "expression.normalize.v1",
        output,
        summary,
        json,
    )
}

fn print_expression_pca(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut options = ExpressionPcaOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--components" => {
                index += 1;
                options.components = parse_sequence_usize(arguments.get(index), "--components")?;
            }
            "--scale" => options.scale_features = true,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown expression PCA option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected expression PCA argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("expression pca requires an input matrix path")?;
    let result = expression_pca_path(Path::new(path), &options)?;
    if json {
        print_analysis_json("expression-pca", "expression.pca.v1", result)
    } else {
        print_expression_pca_text(&result);
        Ok(())
    }
}

fn print_expression_cluster(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut options = ExpressionClusterOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--sample-clusters" => {
                index += 1;
                options.sample_clusters =
                    parse_sequence_usize(arguments.get(index), "--sample-clusters")?;
            }
            "--feature-clusters" => {
                index += 1;
                options.feature_clusters =
                    parse_sequence_usize(arguments.get(index), "--feature-clusters")?;
            }
            "--max-iterations" => {
                index += 1;
                options.max_iterations =
                    parse_sequence_usize(arguments.get(index), "--max-iterations")?;
            }
            "--no-scale" => options.scale_features = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown expression cluster option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected expression cluster argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("expression cluster requires an input matrix path")?;
    let result = expression_cluster_path(Path::new(path), &options)?;
    if json {
        print_analysis_json("expression-cluster", "expression.cluster.v1", result)
    } else {
        print_expression_cluster_text(&result);
        Ok(())
    }
}

fn print_expression_heatmap(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut options = ExpressionHeatmapOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--top-features" => {
                index += 1;
                options.top_variable_features =
                    parse_sequence_usize(arguments.get(index), "--top-features")?;
            }
            "--no-scale" => options.scale_rows = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown expression heatmap option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected expression heatmap argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("expression heatmap requires an input matrix path")?;
    let result = expression_heatmap_path(Path::new(path), &options)?;
    if json {
        print_analysis_json("expression-heatmap", "expression.heatmap.v1", result)
    } else {
        print_expression_heatmap_text(&result);
        Ok(())
    }
}

fn print_expression_volcano(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut positional = Vec::new();
    let mut options = VolcanoPlotOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--padj" => {
                index += 1;
                options.adjusted_pvalue_threshold =
                    parse_finite_f64(arguments.get(index), "--padj", Some(0.0))?;
            }
            "--log2-fold-change" => {
                index += 1;
                options.absolute_log2_fold_change_threshold =
                    parse_finite_f64(arguments.get(index), "--log2-fold-change", Some(0.0))?;
            }
            "--max-points" => {
                index += 1;
                options.max_points = parse_sequence_usize(arguments.get(index), "--max-points")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown expression volcano option: {value}").into());
            }
            value => positional.push(value),
        }
        index += 1;
    }
    if positional.len() != 2 {
        return Err("expression volcano requires <differential.csv> <output.svg>".into());
    }
    let result = render_volcano_svg_path(positional[0], positional[1], &options)?;
    print_visualization_result("expression-volcano", "expression.volcano.v1", result, json)
}

fn print_wgcna(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = WgcnaOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--min-expression" => {
                index += 1;
                options.min_expression = arguments
                    .get(index)
                    .ok_or("--min-expression requires a value")?
                    .parse::<f64>()
                    .map_err(|_| {
                        format!(
                            "--min-expression requires a number, got {:?}",
                            arguments.get(index)
                        )
                    })?;
            }
            "--min-samples" => {
                index += 1;
                options.min_samples = parse_sequence_usize(arguments.get(index), "--min-samples")?;
            }
            "--min-module-size" => {
                index += 1;
                options.min_module_size =
                    parse_sequence_usize(arguments.get(index), "--min-module-size")?;
            }
            "--merge-cut-height" => {
                index += 1;
                options.merge_cut_height = arguments
                    .get(index)
                    .ok_or("--merge-cut-height requires a value")?
                    .parse::<f64>()
                    .map_err(|_| {
                        format!(
                            "--merge-cut-height requires a number, got {:?}",
                            arguments.get(index)
                        )
                    })?;
            }
            "--network-type" => {
                index += 1;
                options.network_type = arguments
                    .get(index)
                    .ok_or("--network-type requires a value")?
                    .clone();
            }
            "--power" => {
                index += 1;
                options.power = parse_sequence_usize(arguments.get(index), "--power")?;
            }
            "--no-log-transform" => options.log_transform = false,
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown WGCNA option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("expression wgcna requires <expression.csv|tsv> <output.json>".into());
    }
    let result = run_wgcna_path(&paths[0], &paths[1], &options)?;
    print_native_tool_result("wgcna", "expression.wgcna.v1", result, json)
}

fn parse_finite_f64(
    value: Option<&String>,
    option: &str,
    minimum: Option<f64>,
) -> Result<f64, Box<dyn Error>> {
    let value = value.ok_or_else(|| format!("{option} requires a value"))?;
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{option} requires a number, got {value:?}"))?;
    if !parsed.is_finite() {
        return Err(format!("{option} must be finite").into());
    }
    if minimum.is_some_and(|minimum| parsed < minimum) {
        return Err(format!("{option} must be at least {}", minimum.unwrap_or_default()).into());
    }
    Ok(parsed)
}

fn print_expression_pca_text(result: &ExpressionPcaResult) {
    println!("feature_count\t{}", result.feature_count);
    println!("sample_count\t{}", result.sample_count);
    println!("scaled_features\t{}", result.scaled_features);
    println!("total_variance\t{:.6}", result.total_variance);
    for component in &result.components {
        println!(
            "component\tPC{}\t{:.6}\t{:.6}",
            component.component, component.eigenvalue, component.explained_variance_percent
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_expression_cluster_text(result: &ExpressionClusterResult) {
    println!("feature_count\t{}", result.feature_count);
    println!("sample_count\t{}", result.sample_count);
    println!("scaled_features\t{}", result.scaled_features);
    println!(
        "sample_clusters\t{}\t{:.6}",
        result.samples.populated_clusters, result.samples.within_cluster_sum_squares
    );
    println!(
        "feature_clusters\t{}\t{:.6}",
        result.features.populated_clusters, result.features.within_cluster_sum_squares
    );
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_expression_heatmap_text(result: &ExpressionHeatmapResult) {
    println!("input_feature_count\t{}", result.input_feature_count);
    println!("selected_feature_count\t{}", result.selected_feature_count);
    println!("sample_count\t{}", result.sample_count);
    println!("scaled_rows\t{}", result.scaled_rows);
    println!("minimum_value\t{:.6}", result.minimum_value);
    println!("maximum_value\t{:.6}", result.maximum_value);
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_pdb_summary(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut json = false;
    let mut options = PdbSummaryOptions::default();
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            "--alphafold-plddt" => options.interpret_b_factors_as_plddt = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown PDB summary option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected PDB summary argument: {value}").into()),
        }
    }
    let path = path.ok_or("structure pdb requires an input path")?;
    let summary = pdb_summary_path(Path::new(path), options)?;
    if json {
        let mut result = AnalysisResult::ok(
            "structure-pdb-summary",
            "structure.pdb.summary.v1",
            summary.clone(),
            ExecutionMode::LocalCpu,
        );
        result.warnings = summary.warnings;
        println!("{}", serde_json::to_string(&result)?);
    } else {
        print_pdb_summary_text(&summary);
    }
    Ok(())
}

fn print_pdb_summary_text(summary: &PdbStructureSummary) {
    println!("model_count\t{}", summary.model_count);
    println!("chain_count\t{}", summary.chain_count);
    println!("residue_count\t{}", summary.residue_count);
    println!("atom_count\t{}", summary.atom_count);
    println!("polymer_atom_count\t{}", summary.polymer_atom_count);
    println!("hetero_atom_count\t{}", summary.hetero_atom_count);
    if let Some(confidence) = &summary.alphafold_confidence {
        println!("mean_plddt\t{:.6}", confidence.mean_plddt);
    }
    for warning in &summary.warnings {
        println!("warning\t{warning}");
    }
}

fn print_mmcif_summary(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, json) = parse_single_path_json(arguments, "structure mmcif-summary")?;
    let summary = mmcif_summary_path(path)?;
    if json {
        print_analysis_json_with_warnings(
            "structure-mmcif-summary",
            "structure.mmcif.summary.v1",
            summary.clone(),
            summary.warnings,
        )
    } else {
        print_mmcif_summary_text(&summary);
        Ok(())
    }
}

fn print_mmcif_summary_text(summary: &MmcifStructureSummary) {
    println!("model_count\t{}", summary.model_count);
    println!("chain_count\t{}", summary.chain_count);
    println!("residue_count\t{}", summary.residue_count);
    println!("atom_count\t{}", summary.atom_count);
    println!("polymer_atom_count\t{}", summary.polymer_atom_count);
    println!("hetero_atom_count\t{}", summary.hetero_atom_count);
    for warning in &summary.warnings {
        println!("warning\t{warning}");
    }
}

fn print_structure_sequence(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, json) = parse_single_path_json(arguments, "structure sequence")?;
    let result = extract_structure_sequences_path(path)?;
    if json {
        print_analysis_json_with_warnings(
            "structure-sequence",
            "structure.sequence.extract.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_structure_sequence_text(&result);
        Ok(())
    }
}

fn print_structure_sequence_text(result: &StructureSequenceResult) {
    println!("model_id\t{}", result.model_id);
    println!("chain_count\t{}", result.chain_count);
    println!("total_residues\t{}", result.total_residues);
    for chain in &result.chains {
        println!(
            "chain\t{}\t{:?}\t{}\t{}",
            chain.chain_id, chain.polymer_type, chain.residue_count, chain.sequence
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_structure_contact_map(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut options = ContactMapOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--cutoff" => {
                index += 1;
                options.cutoff_angstrom =
                    parse_finite_f64(arguments.get(index), "--cutoff", Some(f64::MIN_POSITIVE))?;
            }
            "--atom" => {
                index += 1;
                options.atom_name = arguments
                    .get(index)
                    .ok_or("--atom requires a value")?
                    .to_owned();
            }
            "--intra-chain-only" => options.include_inter_chain = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown structure contact-map option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => {
                return Err(format!("unexpected structure contact-map argument: {value}").into());
            }
        }
        index += 1;
    }
    let path = path.ok_or("structure contact-map requires an input path")?;
    let result = structure_contact_map_path(path, options)?;
    if json {
        print_analysis_json_with_warnings(
            "structure-contact-map",
            "structure.contact-map.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_structure_contact_map_text(&result);
        Ok(())
    }
}

fn print_structure_contact_map_text(result: &StructureContactMapResult) {
    println!("model_id\t{}", result.model_id);
    println!("atom_name\t{}", result.atom_name);
    println!("cutoff_angstrom\t{:.6}", result.cutoff_angstrom);
    println!(
        "representative_residue_count\t{}",
        result.representative_residue_count
    );
    println!("contact_count\t{}", result.contact_count);
    for contact in &result.contacts {
        println!(
            "contact\t{}:{}\t{}:{}\t{:.6}",
            contact.left.chain_id,
            contact.left.residue_id,
            contact.right.chain_id,
            contact.right.residue_id,
            contact.distance_angstrom
        );
    }
}

fn print_structure_geometry(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut selectors = Vec::new();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--atom" => {
                index += 1;
                selectors.push(parse_atom_selector(
                    arguments.get(index).ok_or("--atom requires a selector")?,
                )?);
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown structure geometry option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected structure geometry argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("structure geometry requires an input path")?;
    let result = measure_structure_geometry_path(path, &selectors)?;
    if json {
        print_analysis_json("structure-geometry", "structure.geometry.v1", result)
    } else {
        print_structure_geometry_text(&result);
        Ok(())
    }
}

fn print_structure_geometry_text(result: &StructureGeometryResult) {
    println!("measurement\t{}", result.measurement);
    println!("value\t{:.6}", result.value);
    println!("units\t{}", result.units);
    for atom in &result.atoms {
        println!(
            "atom\t{}\t{}\t{}\t{}",
            atom.chain_id, atom.residue_id, atom.residue_name, atom.atom_name
        );
    }
}

fn print_structure_superposition(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = SuperpositionOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--atom" => {
                index += 1;
                options.atom_name = arguments
                    .get(index)
                    .ok_or("--atom requires a value")?
                    .to_owned();
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown structure superpose option: {value}").into());
            }
            value if paths.len() < 2 => paths.push(value),
            value => {
                return Err(format!("unexpected structure superpose argument: {value}").into());
            }
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("structure superpose requires reference and mobile input paths".into());
    }
    let result = superpose_structures_path(paths[0], paths[1], options)?;
    if json {
        print_analysis_json_with_warnings(
            "structure-superpose",
            "structure.superpose.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_structure_superposition_text(&result);
        Ok(())
    }
}

fn print_structure_superposition_text(result: &StructureSuperpositionResult) {
    println!("atom_name\t{}", result.atom_name);
    println!("matched_atom_count\t{}", result.matched_atom_count);
    println!("rmsd_before_angstrom\t{:.6}", result.rmsd_before_angstrom);
    println!("rmsd_after_angstrom\t{:.6}", result.rmsd_after_angstrom);
    println!(
        "translation\t{:.6}\t{:.6}\t{:.6}",
        result.translation[0], result.translation[1], result.translation[2]
    );
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn parse_single_path_json<'a>(
    arguments: &'a [String],
    command: &str,
) -> Result<(&'a str, bool), Box<dyn Error>> {
    let mut path = None;
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown {command} option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected {command} argument: {value}").into()),
        }
    }
    Ok((
        path.ok_or_else(|| format!("{command} requires an input path"))?,
        json,
    ))
}

fn print_set_analysis(arguments: &[String], venn: bool) -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut options = SetAnalysisOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--include-items" => options.include_items = true,
            "--max-intersections" => {
                index += 1;
                options.max_intersections =
                    parse_sequence_usize(arguments.get(index), "--max-intersections")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown set analysis option: {value}").into());
            }
            value if path.is_none() => path = Some(value),
            value => return Err(format!("unexpected set analysis argument: {value}").into()),
        }
        index += 1;
    }
    let path = path.ok_or("set analysis requires an input CSV/TSV path")?;
    if venn {
        let result = venn_analysis_path(path, options)?;
        if json {
            print_analysis_json("set-venn", "set.venn.v1", result)?;
        } else {
            print_venn_text(&result);
        }
    } else {
        let result = upset_analysis_path(path, options)?;
        if json {
            print_analysis_json("set-upset", "set.upset.v1", result)?;
        } else {
            print_upset_text(&result);
        }
    }
    Ok(())
}

fn print_venn_text(result: &VennAnalysis) {
    println!("set_count\t{}", result.set_count);
    println!("union_size\t{}", result.union_size);
    for set in &result.set_sizes {
        println!("set\t{}\t{}", set.name, set.count);
    }
    for intersection in &result.intersections {
        println!(
            "intersection\t{}\t{}",
            intersection.sets.join("&"),
            intersection.count
        );
    }
}

fn print_upset_text(result: &UpSetAnalysis) {
    println!("set_count\t{}", result.set_count);
    println!("union_size\t{}", result.union_size);
    println!("intersection_count\t{}", result.intersection_count);
    for intersection in &result.intersections {
        println!(
            "intersection\t{}\t{}",
            intersection.sets.join("&"),
            intersection.count
        );
    }
}

fn print_blast_parse(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, json) = parse_single_path_json(arguments, "similarity blast-parse")?;
    let result = parse_blast_path(path)?;
    if json {
        print_analysis_json_with_warnings(
            "similarity-blast-parse",
            "similarity.blast.parse.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_blast_parse_text(&result);
        Ok(())
    }
}

fn print_blast_parse_text(result: &BlastParseResult) {
    println!("format\t{}", result.format);
    println!("record_count\t{}", result.record_count);
    println!("query_count\t{}", result.query_count);
    println!("subject_count\t{}", result.subject_count);
    for hit in &result.hits {
        println!(
            "hit\t{}\t{}\t{:.6}\t{}\t{}\t{}\t{}\t{}\t{:.6e}\t{:.6}",
            hit.query_id,
            hit.subject_id,
            hit.percent_identity,
            hit.alignment_length,
            hit.query_start,
            hit.query_end,
            hit.subject_start,
            hit.subject_end,
            hit.evalue,
            hit.bit_score
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_reciprocal_best_hits(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = ReciprocalBestHitOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--max-evalue" => {
                index += 1;
                options.max_evalue = Some(parse_finite_f64(
                    arguments.get(index),
                    "--max-evalue",
                    Some(0.0),
                )?);
            }
            "--min-identity" => {
                index += 1;
                options.min_identity_percent = Some(parse_sequence_percentage(
                    arguments.get(index),
                    "--min-identity",
                )?);
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown similarity rbh option: {value}").into());
            }
            value => paths.push(value),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("similarity rbh requires forward and reverse BLAST result paths".into());
    }
    let result = reciprocal_best_hits_path(paths[0], paths[1], options)?;
    if json {
        print_analysis_json_with_warnings(
            "similarity-rbh",
            "similarity.reciprocal.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_reciprocal_best_hits_text(&result);
        Ok(())
    }
}

fn print_reciprocal_best_hits_text(result: &ReciprocalBestHitResult) {
    println!("forward_query_count\t{}", result.forward_query_count);
    println!("reverse_query_count\t{}", result.reverse_query_count);
    println!("reciprocal_pair_count\t{}", result.reciprocal_pair_count);
    for pair in &result.pairs {
        println!(
            "pair\t{}\t{}\t{:.6e}\t{:.6e}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
            pair.left_id,
            pair.right_id,
            pair.forward_evalue,
            pair.reverse_evalue,
            pair.forward_bit_score,
            pair.reverse_bit_score,
            pair.forward_identity_percent,
            pair.reverse_identity_percent
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_protein_domains(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let (path, json) = parse_single_path_json(arguments, "protein domains")?;
    let result = parse_protein_domains_path(path)?;
    if json {
        print_analysis_json_with_warnings(
            "protein-domains",
            "protein.domain.parse.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_protein_domains_text(&result);
        Ok(())
    }
}

fn print_protein_domains_text(result: &ProteinDomainParseResult) {
    println!("format\t{}", result.format);
    println!("sequence_count\t{}", result.sequence_count);
    println!("hit_count\t{}", result.hit_count);
    for hit in &result.hits {
        println!(
            "domain\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            hit.sequence_id,
            hit.source,
            hit.accession,
            hit.start,
            hit.end,
            hit.evalue
                .map(|value| format!("{value:.6e}"))
                .unwrap_or_else(|| "NA".to_owned()),
            hit.score
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "NA".to_owned())
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_phylogeny_tree(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = TreeTransformOptions::default();
    let mut label_map_path = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--reroot" => {
                index += 1;
                options.reroot_label = Some(
                    arguments
                        .get(index)
                        .ok_or("--reroot requires a leaf label")?
                        .clone(),
                );
            }
            "--label-map" => {
                index += 1;
                label_map_path = Some(
                    arguments
                        .get(index)
                        .ok_or("--label-map requires a TSV path")?
                        .clone(),
                );
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown phylogeny tree option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "phylogeny tree")?,
        }
        index += 1;
    }
    let input = input.ok_or("phylogeny tree requires an input Newick path")?;
    let output = output.ok_or("phylogeny tree requires an output Newick path")?;
    if let Some(path) = label_map_path {
        options.label_map = read_tree_label_map_path(path)?;
    }
    let result = transform_newick_path(input, output, options)?;
    if json {
        print_analysis_json_with_warnings(
            "phylogeny-tree",
            "phylogeny.tree.transform.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_phylogeny_tree_text(&result);
        Ok(())
    }
}

fn print_phylogeny_tree_text(result: &TreeTransformResult) {
    println!("leaf_count\t{}", result.leaf_count);
    println!("internal_node_count\t{}", result.internal_node_count);
    println!("max_depth\t{}", result.max_depth);
    if let Some(length) = result.total_branch_length {
        println!("total_branch_length\t{length:.6}");
    }
    println!("rerooted\t{}", result.rerooted);
    println!("relabeled_count\t{}", result.relabeled_count);
    println!("output\t{}", result.output);
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_phylogeny_tree_visualize(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = TreeVisualizationOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--width" => {
                index += 1;
                options.width = arguments
                    .get(index)
                    .ok_or("--width requires a value")?
                    .parse()
                    .map_err(|_| "invalid --width value")?;
            }
            "--height" => {
                index += 1;
                options.height = arguments
                    .get(index)
                    .ok_or("--height requires a value")?
                    .parse()
                    .map_err(|_| "invalid --height value")?;
            }
            "--font-size" => {
                index += 1;
                options.font_size = arguments
                    .get(index)
                    .ok_or("--font-size requires a value")?
                    .parse()
                    .map_err(|_| "invalid --font-size value")?;
            }
            "--no-branch-lengths" => options.show_branch_lengths = false,
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown phylogeny tree-plot option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "phylogeny tree-plot")?,
        }
        index += 1;
    }
    let input = input.ok_or("phylogeny tree-plot requires an input Newick path")?;
    let output = output.ok_or("phylogeny tree-plot requires an output SVG path")?;
    let result = render_tree_svg_path(input, output, &options)
        .map_err(|error| format!("tree visualization failed: {error}"))?;
    if json {
        print_analysis_json_with_warnings(
            "phylogeny-tree-plot",
            "phylogeny.tree.visualize.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        println!("output\t{}", result.output_path);
        println!("width\t{}", result.width);
        println!("height\t{}", result.height);
        println!("leaf_count\t{}", result.glyph_count);
        for warning in &result.warnings {
            println!("warning\t{warning}");
        }
        Ok(())
    }
}

fn print_phylogeny_distance(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = DistanceMatrixOptions {
        model: "p-distance".to_owned(),
    };
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--model" => {
                index += 1;
                options.model = arguments
                    .get(index)
                    .ok_or("--model requires a model name")?
                    .clone();
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown phylogeny distance option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "phylogeny distance")?,
        }
        index += 1;
    }
    let input = input.ok_or("phylogeny distance requires an input alignment FASTA path")?;
    let output = output.ok_or("phylogeny distance requires an output TSV path")?;
    let result = distance_matrix_path(input, output, &options)?;
    if json {
        print_analysis_json_with_warnings(
            "phylogeny-distance",
            "phylogeny.distance.v1",
            result.clone(),
            result.warnings,
        )
    } else {
        print_phylogeny_distance_text(&result);
        Ok(())
    }
}

fn print_phylogeny_distance_text(result: &DistanceMatrixResult) {
    println!("sequence_count\t{}", result.sequence_count);
    println!("alignment_length\t{}", result.alignment_length);
    println!(
        "compared_position_count\t{}",
        result.compared_position_count
    );
    println!("model\t{}", result.model);
    for entry in &result.distances {
        println!("{}\t{}\t{:.10}", entry.seq_a, entry.seq_b, entry.distance);
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_protein_properties(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let result = protein_properties_path(path)?;
    if json {
        let mut envelope = AnalysisResult::ok(
            "protein-properties",
            "protein.properties.v1",
            result.clone(),
            ExecutionMode::LocalCpu,
        );
        envelope.warnings = result.warnings;
        println!("{}", serde_json::to_string(&envelope)?);
    } else {
        print_protein_properties_text(&result);
    }
    Ok(())
}

fn print_protein_properties_text(result: &ProteinPropertiesResult) {
    println!("sequence_count\t{}", result.sequence_count);
    println!("total_residues\t{}", result.total_residues);
    for protein in &result.records {
        println!(
            "protein\t{}\t{}\t{}\t{}\t{}",
            protein.id,
            protein.length,
            protein
                .molecular_weight_da
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "NA".to_owned()),
            protein
                .isoelectric_point
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "NA".to_owned()),
            protein
                .gravy
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "NA".to_owned())
        );
    }
    for warning in &result.warnings {
        println!("warning\t{warning}");
    }
}

fn print_dataset_inspection(path: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let inspection = inspect_dataset(Path::new(path))?;
    if json {
        print_analysis_json("dataset-inspect", "dataset.inspect.v1", inspection)?;
    } else {
        print_inspection_text(&inspection);
    }
    Ok(())
}

fn print_table_export(input: &str, output: &str, json: bool) -> Result<(), Box<dyn Error>> {
    let receipt = export_json_file(Path::new(input), Path::new(output))?;
    if json {
        print_analysis_json("table-export", "table.export.v1", receipt)?;
    } else {
        println!("{}", receipt.output_path);
    }
    Ok(())
}

fn print_table_manipulate(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut options = TableManipulateOptions::default();
    let mut filter_column = None;
    let mut filter_op = None;
    let mut filter_value = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--delimiter" => {
                index += 1;
                options.input_delimiter =
                    Some(parse_table_delimiter(arguments.get(index), "--delimiter")?);
            }
            "--output-delimiter" => {
                index += 1;
                options.output_delimiter = Some(parse_table_delimiter(
                    arguments.get(index),
                    "--output-delimiter",
                )?);
            }
            "--select-column" => {
                index += 1;
                options.select_columns.push(
                    arguments
                        .get(index)
                        .ok_or("--select-column requires a column name")?
                        .clone(),
                );
            }
            "--drop-column" => {
                index += 1;
                options.drop_columns.push(
                    arguments
                        .get(index)
                        .ok_or("--drop-column requires a column name")?
                        .clone(),
                );
            }
            "--filter-column" => {
                index += 1;
                filter_column = Some(
                    arguments
                        .get(index)
                        .ok_or("--filter-column requires a column name")?
                        .clone(),
                );
            }
            "--filter-op" => {
                index += 1;
                filter_op = Some(
                    arguments
                        .get(index)
                        .ok_or("--filter-op requires equals, contains, or non-empty")?
                        .clone(),
                );
            }
            "--filter-value" => {
                index += 1;
                filter_value = Some(
                    arguments
                        .get(index)
                        .ok_or("--filter-value requires a value")?
                        .clone(),
                );
            }
            "--skip-rows" => {
                index += 1;
                options.skip_rows = parse_sequence_usize(arguments.get(index), "--skip-rows")?;
            }
            "--limit" => {
                index += 1;
                options.limit = Some(parse_sequence_usize(arguments.get(index), "--limit")?);
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown table manipulate option: {value}").into());
            }
            value => assign_sequence_path(&mut input, &mut output, value, "table manipulate")?,
        }
        index += 1;
    }
    options.filter = parse_table_filter(filter_column, filter_op, filter_value)?;
    let input = input.ok_or("table manipulate requires an input CSV/TSV path")?;
    let output = output.ok_or("table manipulate requires an output CSV/TSV path")?;
    let output = Path::new(output);
    let summary = manipulate_table_path(Path::new(input), output, &options)?;
    print_sequence_transform_result(
        "table-manipulate",
        "table.manipulate.v1",
        output,
        summary,
        json,
    )
}

fn parse_table_filter(
    column: Option<String>,
    op: Option<String>,
    value: Option<String>,
) -> Result<Option<TableFilter>, Box<dyn Error>> {
    match (column, op, value) {
        (None, None, None) => Ok(None),
        (Some(column), Some(op), value) => match op.as_str() {
            "equals" | "eq" => Ok(Some(TableFilter::Equals {
                column,
                value: value.ok_or("--filter-value is required for equals")?,
            })),
            "contains" => Ok(Some(TableFilter::Contains {
                column,
                value: value.ok_or("--filter-value is required for contains")?,
            })),
            "non-empty" | "nonempty" => {
                if value.is_some() {
                    return Err("--filter-value is not used with non-empty".into());
                }
                Ok(Some(TableFilter::NonEmpty { column }))
            }
            value => Err(format!("unsupported --filter-op: {value}").into()),
        },
        _ => Err("--filter-column and --filter-op must be provided together".into()),
    }
}

fn parse_table_delimiter(
    value: Option<&String>,
    option: &str,
) -> Result<TableDelimiter, Box<dyn Error>> {
    match value
        .ok_or_else(|| format!("{option} requires a value"))?
        .as_str()
    {
        "csv" => Ok(TableDelimiter::Csv),
        "tsv" | "tab" => Ok(TableDelimiter::Tsv),
        value => Err(format!("{option} must be csv or tsv, got {value:?}").into()),
    }
}

fn print_local_blast(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut program = "blastn".to_owned();
    let mut options = SimilaritySearchOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--program" => {
                index += 1;
                program = arguments
                    .get(index)
                    .ok_or("--program requires a value")?
                    .clone();
            }
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--evalue" => {
                index += 1;
                options.evalue = parse_finite_f64(arguments.get(index), "--evalue", Some(0.0))?;
            }
            "--max-targets" => {
                index += 1;
                options.max_target_sequences =
                    parse_sequence_usize(arguments.get(index), "--max-targets")?;
            }
            "--outfmt" => {
                index += 1;
                options.outfmt = parse_sequence_usize(arguments.get(index), "--outfmt")?
                    .try_into()
                    .map_err(|_| "--outfmt is out of range")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown local BLAST option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "similarity blast requires <query.fasta> <reference.fasta> <output.tsv>".into(),
        );
    }
    let program = parse_blast_program(&program)?;
    let result = run_blast_fasta_path(&paths[0], &paths[1], &paths[2], program, &options)?;
    print_native_tool_result("local-blast", "similarity.blast.local.v1", result, json)
}

fn print_local_diamond(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut mode = "blastp".to_owned();
    let mut options = SimilaritySearchOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--mode" => {
                index += 1;
                mode = arguments
                    .get(index)
                    .ok_or("--mode requires a value")?
                    .clone();
            }
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--evalue" => {
                index += 1;
                options.evalue = parse_finite_f64(arguments.get(index), "--evalue", Some(0.0))?;
            }
            "--max-targets" => {
                index += 1;
                options.max_target_sequences =
                    parse_sequence_usize(arguments.get(index), "--max-targets")?;
            }
            "--outfmt" => {
                index += 1;
                options.outfmt = parse_sequence_usize(arguments.get(index), "--outfmt")?
                    .try_into()
                    .map_err(|_| "--outfmt is out of range")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown DIAMOND option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "similarity diamond requires <query.fasta> <reference.fasta> <output.tsv>".into(),
        );
    }
    let mode = parse_diamond_mode(&mode)?;
    let result = run_diamond_fasta_path(&paths[0], &paths[1], &paths[2], mode, &options)?;
    print_native_tool_result("local-diamond", "similarity.diamond.v1", result, json)
}

fn print_local_hmmer(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut mode = "hmmsearch".to_owned();
    let mut options = HmmerOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--mode" => {
                index += 1;
                mode = arguments
                    .get(index)
                    .ok_or("--mode requires a value")?
                    .clone();
            }
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--evalue" => {
                index += 1;
                options.evalue = parse_finite_f64(arguments.get(index), "--evalue", Some(0.0))?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown HMMER option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err(
            "similarity hmmer requires <profile.hmm> <sequences.fasta> <output.domtblout>".into(),
        );
    }
    let mode = parse_hmmer_mode(&mode)?;
    let result = run_hmmer_path(&paths[0], &paths[1], &paths[2], mode, &options)?;
    print_native_tool_result("local-hmmer", "similarity.hmmer.v1", result, json)
}

fn print_muscle(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = MuscleOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--mode" => {
                index += 1;
                options.mode =
                    parse_muscle_mode(arguments.get(index).ok_or("--mode requires a value")?)?;
            }
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown MUSCLE option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("msa muscle requires <input.fasta> <output.fasta>".into());
    }
    let result = run_muscle_path(&paths[0], &paths[1], &options)?;
    print_native_tool_result("muscle-alignment", "msa.muscle.v1", result, json)
}

fn print_trimal(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut mode = "automated1".to_owned();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--mode" => {
                index += 1;
                mode = arguments
                    .get(index)
                    .ok_or("--mode requires a value")?
                    .clone();
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown trimAl option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("msa trimal requires <input.alignment> <output.alignment>".into());
    }
    let result = run_trimal_path(&paths[0], &paths[1], parse_trimal_mode(&mode)?)?;
    print_native_tool_result("trimal-alignment", "msa.trimal.v1", result, json)
}

fn print_iqtree(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = IqtreeOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--model" => {
                index += 1;
                options.model = arguments
                    .get(index)
                    .ok_or("--model requires a value")?
                    .clone();
            }
            "--seed" => {
                index += 1;
                options.seed = arguments
                    .get(index)
                    .ok_or("--seed requires a value")?
                    .parse::<u64>()
                    .map_err(|_| "--seed requires an unsigned integer")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown IQ-TREE option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("phylogeny iqtree requires <alignment> <output.newick>".into());
    }
    let result = run_iqtree_path(&paths[0], &paths[1], &options)?;
    print_native_tool_result("iqtree-phylogeny", "phylogeny.iqtree.v1", result, json)
}

fn print_meme(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = MemeOptions::default();
    let mut alphabet = "dna".to_owned();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--alphabet" => {
                index += 1;
                alphabet = arguments
                    .get(index)
                    .ok_or("--alphabet requires a value")?
                    .clone();
            }
            "--distribution" => {
                index += 1;
                options.distribution = arguments
                    .get(index)
                    .ok_or("--distribution requires a value")?
                    .clone();
            }
            "--motifs" => {
                index += 1;
                options.motif_count = parse_sequence_usize(arguments.get(index), "--motifs")?;
            }
            "--min-width" => {
                index += 1;
                options.minimum_width = parse_sequence_usize(arguments.get(index), "--min-width")?;
            }
            "--max-width" => {
                index += 1;
                options.maximum_width = parse_sequence_usize(arguments.get(index), "--max-width")?;
            }
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown MEME option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 2 {
        return Err("motif meme requires <input.fasta> <output.meme>".into());
    }
    options.alphabet = parse_meme_alphabet(&alphabet)?;
    let result = run_meme_path(&paths[0], &paths[1], &options)?;
    print_native_tool_result("meme-discovery", "motif.meme.v1", result, json)
}

fn print_mast(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = MastOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--evalue" => {
                index += 1;
                options.evalue = arguments
                    .get(index)
                    .ok_or("--evalue requires a value")?
                    .parse::<f64>()
                    .map_err(|_| {
                        format!("--evalue requires a number, got {:?}", arguments.get(index))
                    })?;
            }
            "--hit-list" => options.hit_list = true,
            "--threads" => {
                index += 1;
                options.threads = parse_sequence_usize(arguments.get(index), "--threads")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown MAST option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err("motif mast requires <motif.meme> <sequences.fasta> <output.txt>".into());
    }
    let result = run_mast_path(&paths[0], &paths[1], &paths[2], &options)?;
    print_native_tool_result("mast-scan", "motif.mast.v1", result, json)
}

fn print_dssp(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut json = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown DSSP option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
    }
    if paths.len() != 2 {
        return Err(
            "protein secondary-structure requires <structure.pdb|cif> <output.dssp>".into(),
        );
    }
    let result = run_dssp_path(&paths[0], &paths[1])?;
    print_native_tool_result(
        "dssp-secondary-structure",
        "protein.secondary-structure.v1",
        result,
        json,
    )
}

fn print_dotplot(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut paths = Vec::new();
    let mut options = DotplotOptions::default();
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--width" => {
                index += 1;
                options.width = parse_sequence_usize(arguments.get(index), "width")? as u32;
            }
            "--height" => {
                index += 1;
                options.height = parse_sequence_usize(arguments.get(index), "height")? as u32;
            }
            "--kmer" => {
                index += 1;
                options.kmer_size = parse_sequence_usize(arguments.get(index), "kmer-size")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown dotplot option: {value}").into());
            }
            value => paths.push(PathBuf::from(value)),
        }
        index += 1;
    }
    if paths.len() != 3 {
        return Err("comparative dotplot requires <query.fa> <reference.fa> <output.svg>".into());
    }
    let result = render_dotplot_svg_path(&paths[0], &paths[1], &paths[2], &options)?;
    if json {
        print_analysis_json("dotplot", "comparative.dotplot.v1", result)
    } else {
        println!("query_id\t{}", result.query_id);
        println!("reference_id\t{}", result.reference_id);
        println!("query_length\t{}", result.query_length);
        println!("reference_length\t{}", result.reference_length);
        println!("kmer_size\t{}", result.kmer_size);
        println!("match_count\t{}", result.match_count);
        println!("output_path\t{}", result.output_path);
        Ok(())
    }
}

fn print_rnafold(arguments: &[String]) -> Result<(), Box<dyn Error>> {
    let mut input = None;
    let mut output = None;
    let mut temperature = 37.0_f64;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--temp" => {
                index += 1;
                temperature = arguments
                    .get(index)
                    .ok_or("--temp requires a value")?
                    .parse()
                    .map_err(|_| "temperature must be a number")?;
            }
            "--json" => json = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown RNAfold option: {value}").into());
            }
            value => {
                if input.is_none() {
                    input = Some(PathBuf::from(value));
                } else if output.is_none() {
                    output = Some(PathBuf::from(value));
                } else {
                    return Err("too many positional arguments".into());
                }
            }
        }
        index += 1;
    }
    let input = input.ok_or("rna secondary-structure requires <input.fa>")?;
    let output = output.ok_or("rna secondary-structure requires <output.txt>")?;
    let result = run_rnafold_path(input, output, temperature)?;
    print_native_tool_result(
        "rnafold-secondary-structure",
        "rna.secondary-structure.v1",
        result,
        json,
    )
}

fn print_native_tool_result(
    job_id: &str,
    capability: &str,
    result: NativeToolResult,
    json: bool,
) -> Result<(), Box<dyn Error>> {
    if json {
        let warnings = result.warnings.clone();
        print_analysis_json_with_warnings(job_id, capability, result, warnings)
    } else {
        println!("tool\t{}", result.tool);
        println!("mode\t{}", result.mode);
        println!("output_path\t{}", result.output_path);
        println!("output_bytes\t{}", result.output_bytes);
        println!("thread_count\t{}", result.thread_count);
        println!("command_count\t{}", result.command_count);
        for warning in result.warnings {
            println!("warning\t{warning}");
        }
        Ok(())
    }
}

fn print_inspection_text(inspection: &DatasetInspection) {
    let support = match inspection.support {
        DatasetSupport::Supported => "supported",
        DatasetSupport::RecognizedUnsupported => "recognized, not yet supported",
        DatasetSupport::Unknown => "unknown",
    };
    println!("file\t{}", inspection.path.display());
    println!("format\t{}", inspection.format);
    println!("compression\t{:?}", inspection.compression);
    println!("support\t{support}");
    println!("size_bytes\t{}", inspection.size_bytes);
    if let Some(preview) = &inspection.preview {
        println!("preview_records\t{}", preview.records_shown);
        println!("preview_truncated\t{}", preview.truncated);
    }
    for warning in &inspection.warnings {
        println!("warning\t{}: {}", warning.code, warning.message);
    }
    for error in &inspection.errors {
        println!("error\t{}: {}", error.code, error.message);
    }
}

fn print_stats_text(stats: &SequenceStats) {
    println!("sequence_count\t{}", stats.sequence_count);
    println!("total_bases\t{}", stats.total_bases);
    println!("min_length\t{}", stats.min_length);
    println!("max_length\t{}", stats.max_length);
    println!("mean_length\t{:.6}", stats.mean_length);
    println!("n50\t{}", stats.n50);
    println!("l50\t{}", stats.l50);
    println!("au_n\t{:.6}", stats.au_n);
    println!("gc_percent\t{:.6}", stats.gc_percent);
    println!("n_count\t{}", stats.n_count);
    println!("n_percent\t{:.6}", stats.n_percent);
}

fn print_stats_json(stats: &SequenceStats) -> Result<(), Box<dyn Error>> {
    let result = AnalysisResult::ok("cli", "sequence.stats.v1", stats, ExecutionMode::LocalCpu);
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn usage() -> &'static str {
    concat!(
        "usage:\n",
        "  linxira-bio capabilities [--json]\n",
        "  linxira-bio doctor [--json]\n",
        "  linxira-bio environment audit [--json]\n",
        "  linxira-bio environment plan [PROFILE] [--mode MODE] [--project-root PATH] [--json]\n",
        "  linxira-bio environment apply [PROFILE] [--mode MODE] [--project-root PATH] [--json]\n",
        "  linxira-bio runtime catalog [--json]\n",
        "  linxira-bio workflow packs [--json]\n",
        "  linxira-bio workflow run <pack-id> <request.json> <result.json>\n",
        "  linxira-bio dataset inspect <input> [--json]\n",
        "  linxira-bio sequence stats <input.fasta[.gz]> [--json]\n",
        "  linxira-bio sequence extract <input.fasta[.gz]> <output.fasta> [--id ID ...] [--region ID:START-END[:+|-] ...] [--strict] [--json]\n",
        "  linxira-bio sequence filter <input.fasta[.gz]> <output.fasta> [--min-length N] [--max-length N] [--min-gc-percent P] [--max-gc-percent P] [--max-n-percent P] [--json]\n",
        "  linxira-bio sequence reverse-complement <input.fasta[.gz]> <output.fasta> [--json]\n",
        "  linxira-bio sequence translate <input.fasta[.gz]> <output.fasta> [--frame FRAME ...] [--trim-terminal-stop] [--stop-at-first] [--json]\n",
        "  linxira-bio sequence orf <input.fasta[.gz]> <output.fasta> [--min-amino-acids N] [--forward-only] [--include-partial-3prime] [--json]\n",
        "  linxira-bio sequence normalize-ids <input.fasta[.gz]> <output.fasta> [--prefix PREFIX] [--start N] [--width N|--no-padding] [--drop-description] [--json]\n",
        "  linxira-bio sequence merge <output.fasta> <input.fasta[.gz]>... [--allow-duplicate-ids] [--json]\n",
        "  linxira-bio sequence split <input.fasta[.gz]> <output-dir> [--records-per-file N] [--prefix PREFIX] [--json]\n",
        "  linxira-bio sequence to-table <input.fasta[.gz]> <output.csv|tsv> [--delimiter csv|tsv] [--no-header] [--json]\n",
        "  linxira-bio sequence from-table <input.csv|tsv[.gz]> <output.fasta> [--delimiter csv|tsv] [--id-column NAME] [--sequence-column NAME] [--description-column NAME|--no-description-column] [--json]\n",
        "  linxira-bio sequence kmer-count <input.fasta[.gz]> <output.tsv> [--k N] [--canonical] [--top-n N] [--json]\n",
        "  linxira-bio sequence consensus <input.alignment.fasta> <output.fasta> [--threshold FLOAT] [--json]\n",
        "  linxira-bio sequence shuffle <input.fasta[.gz]> <output.fasta> [--seed N] [--json]\n",
        "  linxira-bio sequence convert <input> <output> [--input-format fasta|fastq|genbank|embl] [--output-format fasta|fastq|genbank|embl]\n",
        "  linxira-bio primer epcr <reference.fasta[.gz]> <primers.tsv> <output.tsv> [--min-amplicon N] [--max-amplicon N] [--max-hits N] [--json]\n",
        "  linxira-bio fastq qc <input.fastq[.gz]> [--quality-encoding MODE] [--max-cycles N] [--json]\n",
        "  linxira-bio fastq trim <input.fastq[.gz]> <output.fastq> [--min-quality N] [--min-length N] [--quality-encoding phred+33|phred+64] [--json]\n",
        "  linxira-bio fastq adapter-trim <input.fastq[.gz]> <output.fastq> [--adapter SEQ ...] [--min-overlap N] [--min-length N] [--json]\n",
        "  linxira-bio fastq deduplicate <input.fastq[.gz]> <output.fastq> [--header-umi-delimiter TEXT | --sequence-prefix-umi N] [--json]\n",
        "  linxira-bio fastq subsample <input.fastq[.gz]> <output.fastq> [--target-count N | --fraction F] [--seed N] [--json]\n",
        "  linxira-bio alignment qc <input.sam[.gz]> [--json]\n",
        "  linxira-bio alignment bam-cram-qc <input.bam|cram> <output.tsv> [--reference reference.fasta] [--json]\n",
        "  linxira-bio alignment coverage <input.bam|cram> <output.tsv> [--reference reference.fasta] [--json]\n",
        "  linxira-bio alignment bam-to-bigwig <input.bam|cram> <output.bw> [--threads N] [--json]\n",
        "  linxira-bio alignment short-read <reference.fasta> <reads.fastq> <output.bam> [--threads N] [--json]\n",
        "  linxira-bio alignment long-read <reference.fasta> <reads.fastq> <output.sam> [--preset map-ont|map-pb|map-hifi|splice] [--threads N] [--secondary] [--json]\n",
        "  linxira-bio variant annotate <input.vcf> <output.vcf> [--database DB] [--upstream-downstream N] [--no-stats] [--json]\n",
        "  linxira-bio annotation stats <input.gff3|gtf[.gz]> [--json]\n",
        "  linxira-bio annotation normalize <input.gff3|gtf[.gz]> <output.gff3> [--sort] [--json]\n",
        "  linxira-bio annotation positions <input.gff3|gtf[.gz]> <output.tsv> [--feature-type TYPE ...] [--json]\n",
        "  linxira-bio annotation extract <input.gff3|gtf[.gz]> <reference.fasta[.gz]> <output.fasta> [--feature-type gene|transcript|cds|exon|utr|five_prime_utr|three_prime_utr|promoter] [--promoter-length N] [--json]\n",
        "  linxira-bio annotation gene-density <input.gff3|gtf[.gz]> [--feature-type TYPE ...] [--window-size N] [--step-size N] [--json]\n",
        "  linxira-bio annotation go <input.csv|tsv[.gz]> <output.tsv> [--gene-column NAME] [--go-column NAME] [--json]\n",
        "  linxira-bio annotation eggnog <input.tsv[.gz]> <output.tsv> [--json]\n",
        "  linxira-bio annotation plot <input.gff3|gtf[.gz]> <output.svg> [--feature-id ID | --seqid NAME] [--max-features N] [--json]\n",
        "  linxira-bio variant stats <input.vcf[.gz]> [--json]\n",
        "  linxira-bio variant compare <left.vcf[.gz]> <right.vcf[.gz]> [--json]\n",
        "  linxira-bio variant filter <input.vcf[.gz]> <output.vcf> [--min-qual Q] [--pass-only] [--contig NAME ...] [--min-info-dp N] [--json]\n",
        "  linxira-bio variant normalize <input.vcf[.gz]> <reference.fasta[.gz]> <output.vcf> [--json]\n",
        "  linxira-bio variant to-table <input.vcf[.gz]> <output.tsv> [--json]\n",
        "  linxira-bio interval intersect <left.bed[.gz]> <right.bed[.gz]> [--json]\n",
        "  linxira-bio interval merge <input.bed[.gz]> <output.bed> [--max-gap N] [--json]\n",
        "  linxira-bio interval subtract <left.bed[.gz]> <right.bed[.gz]> <output.bed> [--json]\n",
        "  linxira-bio interval closest <query.bed[.gz]> <target.bed[.gz]> <output.tsv> [--json]\n",
        "  linxira-bio expression matrix-qc <matrix.csv|tsv[.gz]> [--json]\n",
        "  linxira-bio medical cohort-qc <cohort.csv|tsv[.gz]> [--json]\n",
        "  linxira-bio medical single-cell-qc <counts.csv|tsv[.gz]> [--json]\n",
        "  linxira-bio medical pathway <genes.txt|csv|tsv> <associations.csv|tsv> [--min-overlap N] [--max-terms N] [--include-genes] [--json]\n",
        "  linxira-bio medical variant-cohort <cohort.vcf[.gz]> [--json]\n",
        "  linxira-bio expression normalize <matrix.csv|tsv[.gz]> <output.tsv> [--method cpm|log2-cpm|median-ratio] [--pseudocount X] [--json]\n",
        "  linxira-bio expression pca <matrix.csv|tsv[.gz]> [--components N] [--scale] [--json]\n",
        "  linxira-bio expression cluster <matrix.csv|tsv[.gz]> [--sample-clusters N] [--feature-clusters N] [--max-iterations N] [--no-scale] [--json]\n",
        "  linxira-bio expression heatmap <matrix.csv|tsv[.gz]> [--top-features N] [--no-scale] [--json]\n",
        "  linxira-bio expression volcano <differential.csv> <output.svg> [--padj P] [--log2-fold-change X] [--max-points N] [--json]\n",
        "  linxira-bio expression wgcna <expression.csv|tsv> <output.json> [--min-expression X] [--min-samples N] [--min-module-size N] [--merge-cut-height X] [--network-type signed|unsigned|signed hybrid] [--power N] [--no-log-transform] [--threads N] [--json]\n",
        "  linxira-bio set venn <sets.csv|tsv[.gz]> [--include-items] [--json]\n",
        "  linxira-bio set upset <sets.csv|tsv[.gz]> [--max-intersections N] [--include-items] [--json]\n",
        "  linxira-bio enrichment custom <genes.txt|csv|tsv> <associations.csv|tsv[.gz]> [--min-overlap N] [--max-terms N] [--include-genes] [--json]\n",
        "  linxira-bio enrichment go <genes.txt|csv|tsv> <associations.csv|tsv[.gz]> [--min-overlap N] [--max-terms N] [--include-genes] [--json]\n",
        "  linxira-bio enrichment kegg <genes.txt|csv|tsv> <associations.csv|tsv[.gz]> [--min-overlap N] [--max-terms N] [--include-genes] [--json]\n",
        "  linxira-bio enrichment gsea <ranked-genes.csv|tsv> <gene-sets.csv|tsv> [--score-exponent X] [--min-set-size N] [--max-set-size N] [--permutations N] [--seed N] [--json]\n",
        "  linxira-bio enrichment visualize <genes.txt|csv|tsv> <associations.csv|tsv[.gz]> <output.svg> --kind custom|go|kegg [--style bar|dot|network] [--min-overlap N] [--max-terms N] [--json]\n",
        "  linxira-bio similarity blast-parse <blast.tsv|xml[.gz]> [--json]\n",
        "  linxira-bio similarity blast <query.fasta> <reference.fasta> <output.tsv> [--program blastn|blastp|blastx|tblastn|tblastx] [--threads N] [--evalue X] [--max-targets N] [--outfmt 6|7] [--json]\n",
        "  linxira-bio similarity diamond <query.fasta> <reference.fasta> <output.tsv> [--mode blastp|blastx] [--threads N] [--evalue X] [--max-targets N] [--outfmt 6|7] [--json]\n",
        "  linxira-bio similarity hmmer <profile.hmm> <sequences.fasta> <output.domtblout> [--mode hmmsearch|hmmscan] [--threads N] [--evalue X] [--json]\n",
        "  linxira-bio motif meme <input.fasta> <output.meme> [--alphabet dna|rna|protein] [--distribution oops|zoops|anr] [--motifs N] [--min-width N] [--max-width N] [--threads N] [--json]\n",
        "  linxira-bio motif logo <input.meme> <output.svg> [--json]\n",
        "  linxira-bio motif mast <motif.meme> <sequences.fasta> <output.txt> [--evalue X] [--hit-list] [--threads N] [--json]\n",
        "  linxira-bio comparative synteny-plot <anchors.tsv> <output.svg> [--json]\n",
        "  linxira-bio similarity rbh <forward.tsv|xml[.gz]> <reverse.tsv|xml[.gz]> [--max-evalue X] [--min-identity P] [--json]\n",
        "  linxira-bio protein properties <proteins.fasta[.gz]> [--json]\n",
        "  linxira-bio protein domains <interproscan.tsv|hmmer.domtblout[.gz]> [--json]\n",
        "  linxira-bio protein domain-plot <interproscan.tsv|hmmer.domtblout[.gz]> <output.svg> [--sequence-id ID] [--max-sequences N] [--max-domains N] [--json]\n",
        "  linxira-bio phylogeny tree <input.nwk[.gz]> <output.nwk> [--reroot LEAF] [--label-map labels.tsv] [--json]\n",
        "  linxira-bio phylogeny distance <input.alignment.fasta> <output.tsv> [--model p-distance|jc69|k80] [--json]\n",
        "  linxira-bio msa muscle <input.fasta> <output.fasta> [--mode align|super5] [--threads N] [--json]\n",
        "  linxira-bio msa trimal <input.alignment> <output.alignment> [--mode automated1|gappyout|strict|strictplus|nogaps] [--json]\n",
        "  linxira-bio phylogeny iqtree <alignment> <output.newick> [--threads N] [--model MODEL] [--seed N] [--json]\n",
        "  linxira-bio protein secondary-structure <structure.pdb|cif> <output.dssp> [--json]\n",
        "  linxira-bio table manipulate <input.csv|tsv[.gz]> <output.csv|tsv> [--select-column NAME ...] [--drop-column NAME ...] [--filter-column NAME --filter-op equals|contains|non-empty [--filter-value VALUE]] [--skip-rows N] [--limit N] [--delimiter csv|tsv] [--output-delimiter csv|tsv] [--json]\n",
        "  linxira-bio structure pdb <input.pdb[.gz]> [--alphafold-plddt] [--json]\n",
        "  linxira-bio structure mmcif-summary <input.cif|mmcif[.gz]> [--json]\n",
        "  linxira-bio structure sequence <input.pdb|cif[.gz]> [--json]\n",
        "  linxira-bio structure contact-map <input.pdb|cif[.gz]> [--cutoff ANGSTROM] [--atom NAME] [--intra-chain-only] [--json]\n",
        "  linxira-bio structure geometry <input.pdb|cif[.gz]> --atom CHAIN/RESIDUE/ATOM --atom ... [--json]\n",
        "  linxira-bio structure superpose <reference.pdb|cif[.gz]> <mobile.pdb|cif[.gz]> [--atom NAME] [--json]\n",
        "  linxira-bio export table <input.json> <output.csv|tsv|json|jsonl|xlsx> [--json]"
    )
}

#[cfg(test)]
mod workflow_catalog_tests {
    use super::{
        WorkflowCatalog, WorkflowCatalogPack, WorkflowRuntimeKind, validate_workflow_catalog,
        validate_workflow_request, validate_workflow_result,
    };

    fn pack(id: &str, capability: &str, aliases: &[&str]) -> WorkflowCatalogPack {
        WorkflowCatalogPack {
            id: id.to_owned(),
            capability: capability.to_owned(),
            capability_aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
            status: "cataloged".to_owned(),
            trust: "official".to_owned(),
            runtime: WorkflowRuntimeKind::R,
            manifest: "workflows/example/manifest.json".to_owned(),
        }
    }

    #[test]
    fn accepts_primary_and_alias_capabilities_and_preserves_request_identity() {
        let pack = pack(
            "org.linxira.example",
            "expression.differential.v1",
            &["medical.bulk-rnaseq.v1", "expression.deseq2.v1"],
        );
        for capability in [
            "expression.differential.v1",
            "medical.bulk-rnaseq.v1",
            "expression.deseq2.v1",
        ] {
            let request = serde_json::json!({
                "schema_version": "2",
                "job_id": "alias-test",
                "capability": capability
            });
            let identity = validate_workflow_request(&request, &pack).expect("valid request");
            assert_eq!(identity.capability, capability);
            let result = serde_json::json!({
                "schema_version": "2",
                "job_id": "alias-test",
                "capability": capability,
                "status": "ok",
                "provenance": {"core_version": env!("CARGO_PKG_VERSION")}
            });
            validate_workflow_result(&result, &identity).expect("matching result identity");

            let wrong_result = serde_json::json!({
                "schema_version": "2",
                "job_id": "alias-test",
                "capability": "expression.differential.v1",
                "status": "ok",
                "provenance": {"core_version": env!("CARGO_PKG_VERSION")}
            });
            if capability != "expression.differential.v1" {
                assert!(validate_workflow_result(&wrong_result, &identity).is_err());
            }
        }
    }

    #[test]
    fn rejects_duplicate_and_ambiguous_catalog_capabilities() {
        for aliases in [
            vec!["expression.differential.v1"],
            vec!["medical.bulk-rnaseq.v1", "medical.bulk-rnaseq.v1"],
        ] {
            let catalog = WorkflowCatalog {
                schema_version: "1".to_owned(),
                packs: vec![pack(
                    "org.linxira.example",
                    "expression.differential.v1",
                    &aliases,
                )],
            };
            assert!(validate_workflow_catalog(&catalog).is_err());
        }

        let catalog = WorkflowCatalog {
            schema_version: "1".to_owned(),
            packs: vec![
                pack(
                    "org.linxira.first",
                    "expression.differential.v1",
                    &["medical.bulk-rnaseq.v1"],
                ),
                pack("org.linxira.second", "medical.bulk-rnaseq.v1", &[]),
            ],
        };
        assert!(validate_workflow_catalog(&catalog).is_err());
    }

    #[test]
    fn accepts_a_pack_without_optional_aliases() {
        let catalog = WorkflowCatalog {
            schema_version: "1".to_owned(),
            packs: vec![pack("org.linxira.example", "sequence.convert.v1", &[])],
        };
        validate_workflow_catalog(&catalog).expect("alias-free pack remains valid");
    }
}
