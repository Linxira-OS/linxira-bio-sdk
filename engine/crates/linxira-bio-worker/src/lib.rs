#![forbid(unsafe_code)]

//! Execute versioned Linxira Bio jobs through one shared local worker API.

mod workflow;

use linxira_bio_core::alignment::sam_qc_path;
use linxira_bio_core::annotation::{
    AnnotationExtractOptions, AnnotationNormalizeOptions, GeneDensityOptions, GenePositionOptions,
    annotation_gene_positions_path, annotation_stats_path, extract_annotation_sequences_path,
    gene_density_path, gxf_to_bed_path, normalize_annotation_path,
};
use linxira_bio_core::cohort::cohort_table_qc_path;
use linxira_bio_core::coordinate::{
    AtomSelector, ContactMapOptions, SuperpositionOptions, extract_structure_sequences_path,
    measure_structure_geometry_path, mmcif_summary_path, parse_atom_selector,
    structure_contact_map_path, superpose_structures_path,
};
use linxira_bio_core::dataset::{
    DatasetCompression, DatasetFormat, DatasetInspectionOptions, DetectionConfidence,
    inspect_dataset_with_options,
};
use linxira_bio_core::domain::parse_protein_domains_path;
use linxira_bio_core::dotplot::{DotplotOptions, render_dotplot_svg_path};
use linxira_bio_core::environment::{
    EnvironmentMode, EnvironmentPlanOptions, apply_environment, audit_environment,
    parse_environment_mode, plan_environment_with_options,
};
use linxira_bio_core::expression::{
    ExpressionClusterOptions, ExpressionHeatmapOptions, ExpressionNormalizeOptions,
    ExpressionPcaOptions, expression_cluster_path, expression_heatmap_path,
    expression_matrix_qc_path, expression_pca_path, normalize_expression_matrix_path,
    parse_expression_normalization_method,
};
use linxira_bio_core::fastq::{
    DEFAULT_MAX_CYCLES, FastqQcOptions, QualityEncodingMode, fastq_qc_path,
};
use linxira_bio_core::fastq_transform::{
    DEFAULT_ADAPTER_MIN_OVERLAP, DEFAULT_MIN_LENGTH, DEFAULT_TRIM_QUALITY, FastqAdapterOptions,
    FastqDeduplicateKey, FastqDeduplicateOptions, FastqSubsampleOptions, FastqTransformError,
    FastqTransformQualityEncoding, FastqTrimOptions, fastq_adapter_trim_path,
    fastq_deduplicate_path, fastq_subsample_path, fastq_trim_path,
};
use linxira_bio_core::functional::{
    EnrichmentKind, EnrichmentOptions, GoAnnotationOptions, GseaOptions, gsea_preranked_path,
    normalize_eggnog_path, normalize_go_annotations_path, overrepresentation_path,
};
use linxira_bio_core::interval::{
    IntervalMergeOptions, bed_closest_path, bed_intersect_path, bed_merge_path, bed_subtract_path,
};
use linxira_bio_core::native_tools::{
    HmmerOptions, IqtreeOptions, Kraken2Options, MastOptions, MemeOptions, Minimap2LongReadOptions,
    Minimap2Preset, MuscleOptions, ShortReadAlignmentOptions, SimilaritySearchOptions,
    SnpEffOptions, WgcnaOptions, parse_blast_program, parse_diamond_mode, parse_hmmer_mode,
    parse_meme_alphabet, parse_minimap2_preset, parse_muscle_mode, parse_trimal_mode,
    run_bam_to_bigwig_path, run_blast_fasta_path, run_diamond_fasta_path, run_dssp_path,
    run_hmmer_path, run_iqtree_path, run_kaks_path, run_kraken2_path, run_mast_path,
    run_mcscanx_path, run_meme_path, run_minimap2_long_read_path, run_muscle_path,
    run_rnafold_path, run_samtools_report_path, run_short_read_alignment_path, run_snpeff_path,
    run_trimal_path, run_wgcna_path,
};
use linxira_bio_core::phylogeny::{
    DistanceMatrixOptions, TreeTransformOptions, TreeVisualizationOptions, distance_matrix_path,
    render_tree_svg_path, transform_newick_path,
};
use linxira_bio_core::protein::protein_properties_path;
use linxira_bio_core::scientific_visualization::{
    AnnotationStructureOptions, DomainArchitectureOptions, EnrichmentPlotStyle,
    EnrichmentVisualizationOptions, SyntenyPlotStyle, SyntenyVisualizationOptions,
    VolcanoPlotOptions, render_annotation_structure_svg_path, render_domain_architecture_svg_path,
    render_enrichment_svg_path, render_motif_logo_svg_path, render_synteny_svg_with_options_path,
    render_volcano_svg_path,
};
use linxira_bio_core::sequence::fasta_stats_path;
use linxira_bio_core::sequence_analysis::{
    ConsensusOptions, EpcrOptions, KmerCountOptions, ShuffleOptions, consensus_from_alignment_path,
    count_kmers_path, epcr_path, shuffle_sequences_path,
};
use linxira_bio_core::sequence_transform::{
    SequenceExtractOptions, SequenceFilterOptions, SequenceFromTableOptions,
    SequenceIdNormalizeOptions, SequenceMergeOptions, SequenceOrfOptions, SequenceSplitOptions,
    SequenceTableDelimiter, SequenceToTableOptions, SequenceTransformError,
    SequenceTranslateOptions, extract_fasta_path, fasta_to_table_path, filter_fasta_path,
    find_orfs_fasta_path, merge_fasta_paths, normalize_fasta_ids_path, parse_sequence_region_spec,
    reverse_complement_fasta_path, split_fasta_path, table_to_fasta_path, translate_fasta_path,
};
use linxira_bio_core::set_analysis::{SetAnalysisOptions, upset_analysis_path, venn_analysis_path};
use linxira_bio_core::similarity::{
    ReciprocalBestHitOptions, parse_blast_path, reciprocal_best_hits_path,
};
use linxira_bio_core::structure::{PdbSummaryOptions, pdb_summary_path};
use linxira_bio_core::table::{
    TableDelimiter, TableFilter, TableManipulateOptions, manipulate_table_path,
};
use linxira_bio_core::variant::vcf_stats_path;
use linxira_bio_core::variant_transform::{
    VariantFilterOptions, compare_vcf_paths, filter_vcf_path, normalize_vcf_path, vcf_to_table_path,
};
use linxira_bio_export::{ExportFormat, ensure_distinct_input_output, export_json_file};
use linxira_bio_protocol::{
    AnalysisResult, AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, Diagnostic,
    DiagnosticSeverity, ExecutionMode, JobRequest, JobRequestV2, OutputArtifact,
    OutputArtifactKind, SCHEMA_VERSION, SCHEMA_VERSION_V2,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub type WorkerError = Box<dyn Error + Send + Sync>;
pub type WorkerResult<T> = Result<T, WorkerError>;

pub fn execute_path(request_path: &Path) -> WorkerResult<String> {
    // Parsing and typed deserialization happen before a reliable v2 identity exists. Failures at
    // this boundary remain process errors; semantic failures are enveloped by execute_request_v2.
    let request_file = File::open(request_path)?;
    let value: serde_json::Value = serde_json::from_reader(BufReader::new(request_file))?;
    let base_directory = request_path.parent().unwrap_or_else(|| Path::new("."));
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
    {
        Some(SCHEMA_VERSION) => execute_request(serde_json::from_value(value)?, base_directory),
        Some(SCHEMA_VERSION_V2) => {
            execute_request_v2(serde_json::from_value(value)?, base_directory)
        }
        Some(version) => Err(format!("unsupported job schema: {version}").into()),
        None => Err("job request requires schema_version".into()),
    }
}

pub fn execute_request(request: JobRequest, base_directory: &Path) -> WorkerResult<String> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(format!("unsupported job schema: {}", request.schema_version).into());
    }
    if !matches!(
        request.execution.mode,
        ExecutionMode::LocalCpu | ExecutionMode::Container
    ) {
        return Err("the current worker supports local-cpu and container execution only".into());
    }

    match request.capability.as_str() {
        "alignment.qc.v1" => run_alignment_qc(base_directory, request),
        "annotation.gxf.stats.v1" => run_annotation_stats(base_directory, request),
        "annotation.gxf.normalize.v1" => run_annotation_normalize(base_directory, request),
        "annotation.gene-position.v1" => run_annotation_positions(base_directory, request),
        "annotation.gxf.to-bed.v1" => run_gxf_to_bed(base_directory, request),
        "annotation.sequence.extract.v1" => run_annotation_extract(base_directory, request),
        "genome.gene-density.v1" => run_gene_density(base_directory, request),
        "annotation.go.normalize.v1" => run_go_annotations(base_directory, request),
        "annotation.eggnog.normalize.v1" => run_eggnog_annotations(base_directory, request),
        "annotation.structure.visualize.v1" => {
            run_annotation_structure_visualization(base_directory, request)
        }
        "comparative.synteny.visualize.v1" => run_synteny_visualization(base_directory, request),
        "comparative.mcscanx.v1" => run_mcscanx(base_directory, request),
        "comparative.kaks.v1" => run_kaks(base_directory, request),
        "comparative.dotplot.v1" => run_dotplot(base_directory, request),
        "rna.secondary-structure.v1" => run_rnafold(base_directory, request),
        "metagenomics.classify.v1" => run_metagenomics_classify(base_directory, request),
        "enrichment.overrepresentation.v1" => {
            run_enrichment(base_directory, request, EnrichmentKind::Custom)
        }
        "medical.pathway-ruo.v1" => run_enrichment(base_directory, request, EnrichmentKind::Custom),
        "enrichment.go.v1" => run_enrichment(base_directory, request, EnrichmentKind::Go),
        "enrichment.kegg.v1" => run_enrichment(base_directory, request, EnrichmentKind::Kegg),
        "enrichment.gsea.v1" => run_gsea(base_directory, request),
        "enrichment.visualize.v1" => run_enrichment_visualization(base_directory, request),
        "environment.audit.v1" => run_environment_audit(request),
        "environment.plan.v1" => run_environment_plan(base_directory, request),
        "environment.apply.v1" => run_environment_apply(base_directory, request),
        "dataset.inspect.v1" => run_dataset_inspection(base_directory, request),
        "fastq.qc.v1" => run_fastq_qc(base_directory, request),
        "fastq.trim.v1" => run_fastq_trim(base_directory, request),
        "fastq.adapter.v1" => run_fastq_adapter_trim(base_directory, request),
        "fastq.deduplicate.v1" => run_fastq_deduplicate(base_directory, request),
        "fastq.subsample.v1" => run_fastq_subsample(base_directory, request),
        "alignment.bam-cram.qc.v1" => run_bam_cram_report(base_directory, request, "stats"),
        "alignment.coverage.v1" => run_bam_cram_report(base_directory, request, "coverage"),
        "alignment.bam-to-bigwig.v1" => run_bam_to_bigwig(base_directory, request),
        "alignment.short-read.v1" => run_short_read_alignment(base_directory, request),
        "alignment.long-read.v1" => run_long_read_alignment(base_directory, request),
        "expression.matrix.qc.v1" => run_expression_matrix_qc(base_directory, request),
        "medical.cohort-table.qc.v1" => run_cohort_table_qc(base_directory, request),
        "medical.single-cell-qc.v1" => run_single_cell_qc(base_directory, request),
        "expression.normalize.v1" => run_expression_normalize(base_directory, request),
        "expression.pca.v1" => run_expression_pca(base_directory, request),
        "expression.cluster.v1" => run_expression_cluster(base_directory, request),
        "expression.heatmap.v1" => run_expression_heatmap(base_directory, request),
        "expression.differential.v1" | "medical.bulk-rnaseq.v1" => {
            workflow::execute_bulk_expression_v1(base_directory, request)
        }
        "sequence.convert.biopython.v1" => {
            workflow::execute_sequence_convert_v1(base_directory, request)
        }
        "interval.intersect.v1" => run_interval_intersect(base_directory, request),
        "interval.merge.v1" => run_interval_merge(base_directory, request),
        "interval.subtract.v1" => run_interval_subtract(base_directory, request),
        "interval.closest.v1" => run_interval_closest(base_directory, request),
        "table.manipulate.v1" => run_table_manipulate(base_directory, request),
        "table.export.v1" => run_table_export(base_directory, request),
        "sequence.extract.v1" => run_sequence_extract(base_directory, request),
        "sequence.filter.v1" => run_sequence_filter(base_directory, request),
        "sequence.reverse-complement.v1" => {
            run_sequence_reverse_complement(base_directory, request)
        }
        "sequence.stats.v1" => run_sequence_stats(base_directory, request),
        "sequence.translate.v1" => run_sequence_translate(base_directory, request),
        "sequence.orf.v1" => run_sequence_orf(base_directory, request),
        "sequence.id.normalize.v1" => run_sequence_id_normalize(base_directory, request),
        "sequence.merge.v1" => run_sequence_merge(base_directory, request),
        "sequence.split.v1" => run_sequence_split(base_directory, request),
        "sequence.to-table.v1" => run_sequence_to_table(base_directory, request),
        "sequence.from-table.v1" => run_sequence_from_table(base_directory, request),
        "sequence.kmer.count.v1" => run_sequence_kmer_count(base_directory, request),
        "sequence.consensus.v1" => run_sequence_consensus(base_directory, request),
        "sequence.shuffle.v1" => run_sequence_shuffle(base_directory, request),
        "primer.epcr.v1" => run_primer_epcr(base_directory, request),
        "set.venn.v1" => run_set_venn(base_directory, request),
        "set.upset.v1" => run_set_upset(base_directory, request),
        "similarity.blast.parse.v1" => run_blast_parse(base_directory, request),
        "similarity.blast.local.v1" => run_local_blast(base_directory, request),
        "similarity.diamond.v1" => run_local_diamond(base_directory, request),
        "similarity.hmmer.v1" => run_local_hmmer(base_directory, request),
        "motif.meme.v1" => run_meme_discovery(base_directory, request),
        "similarity.reciprocal.v1" => run_reciprocal_best_hits(base_directory, request),
        "protein.properties.v1" => run_protein_properties(base_directory, request),
        "protein.domain.parse.v1" => run_protein_domains(base_directory, request),
        "protein.domain.visualize.v1" => run_protein_domain_visualization(base_directory, request),
        "phylogeny.tree.transform.v1" => run_phylogeny_tree(base_directory, request),
        "phylogeny.tree.visualize.v1" => run_phylogeny_tree_visualize(base_directory, request),
        "phylogeny.distance.v1" => run_phylogeny_distance(base_directory, request),
        "phylogeny.iqtree.v1" => run_iqtree_inference(base_directory, request),
        "msa.muscle.v1" => run_muscle_alignment(base_directory, request),
        "msa.trimal.v1" => run_trimal_alignment(base_directory, request),
        "protein.secondary-structure.v1" => run_dssp_secondary_structure(base_directory, request),
        "structure.pdb.summary.v1" => run_pdb_summary(base_directory, request),
        "structure.mmcif.summary.v1" => run_mmcif_summary(base_directory, request),
        "structure.sequence.extract.v1" => run_structure_sequence(base_directory, request),
        "structure.contact-map.v1" => run_structure_contact_map(base_directory, request),
        "structure.geometry.v1" => run_structure_geometry(base_directory, request),
        "structure.superpose.v1" => run_structure_superposition(base_directory, request),
        "variant.stats.v1" => run_variant_stats(base_directory, request),
        "variant.compare.v1" => run_variant_compare(base_directory, request),
        "medical.variant-cohort.v1" => run_medical_variant_cohort(base_directory, request),
        "variant.filter.v1" => run_variant_filter(base_directory, request),
        "variant.normalize.v1" => run_variant_normalize(base_directory, request),
        "variant.to-table.v1" => run_variant_to_table(base_directory, request),
        "variant.annotate.v1" => run_variant_annotate(base_directory, request),
        "motif.mast.v1" => run_mast(base_directory, request),
        "expression.wgcna.v1" => run_wgcna(base_directory, request),
        capability => Err(format!("unsupported capability: {capability}").into()),
    }
}

pub fn execute_request_v2(request: JobRequestV2, base_directory: &Path) -> WorkerResult<String> {
    if request.schema_version != SCHEMA_VERSION_V2 {
        return Err(format!("unsupported job schema: {}", request.schema_version).into());
    }
    if request.job_id.trim().is_empty() || request.capability.trim().is_empty() {
        return Err("v2 job request requires non-empty job_id and capability".into());
    }

    let job_id = request.job_id.clone();
    let capability = request.capability.clone();
    match execute_request_v2_inner(request, base_directory) {
        Ok(result) => Ok(result),
        Err(error) => Ok(serde_json::to_string(&AnalysisResultV2::error(
            job_id,
            capability,
            "job-failed",
            error.to_string(),
            ExecutionMode::LocalCpu,
        ))?),
    }
}

fn execute_request_v2_inner(request: JobRequestV2, base_directory: &Path) -> WorkerResult<String> {
    if !matches!(
        request.execution.mode,
        ExecutionMode::LocalCpu | ExecutionMode::Container
    ) {
        return Err("the current worker supports local-cpu and container execution only".into());
    }
    if request.execution.mode == ExecutionMode::Container
        && !matches!(
            request.capability.as_str(),
            "expression.differential.v1"
                | "medical.bulk-rnaseq.v1"
                | "expression.deseq2.v1"
                | "sequence.convert.biopython.v1"
        )
    {
        return Err("container execution is only supported for workflow pack capabilities".into());
    }
    validate_v2_contract(&request)?;
    for input in &request.inputs {
        if !input.has_valid_cardinality() {
            return Err(format!(
                "input artifact {} does not match {:?} cardinality",
                input.artifact_id, input.cardinality
            )
            .into());
        }
    }
    let verified_inputs = validate_v2_inputs(&request, base_directory)?;

    match request.capability.as_str() {
        "alignment.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "sam")?;
            let metrics = sam_qc_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "alignment-qc-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "alignment.bam-cram.qc.v1" | "alignment.coverage.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "alignment")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let mode = if request.capability == "alignment.bam-cram.qc.v1" {
                "stats"
            } else {
                "coverage"
            };
            let result = run_samtools_report_path(input, None, &output, mode)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: if mode == "stats" {
                        "alignment-stats"
                    } else {
                        "alignment-coverage"
                    },
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "alignment.bam-to-bigwig.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "alignment")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let threads = optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1);
            let result = run_bam_to_bigwig_path(input, &output, threads)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "alignment-bigwig",
                    role: "track",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bigwig),
                    media_type: Some("application/x-bigwig"),
                },
            )
        }
        "alignment.short-read.v1" => {
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let reads = resolve_v2_single_input(base_directory, &request, "reads")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_short_read_alignment_path(
                reference,
                reads,
                &output,
                &ShortReadAlignmentOptions {
                    threads: optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1),
                },
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "short-read-alignment",
                    role: "alignment",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Unknown),
                    media_type: Some("application/octet-stream"),
                },
            )
        }
        "annotation.gxf.stats.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                annotation_stats_path(input)?,
            )
        }
        "annotation.gxf.normalize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = normalize_annotation_path(
                input,
                &output,
                AnnotationNormalizeOptions {
                    sort: optional_parameter_bool(&request.parameters, "sort")?.unwrap_or(false),
                },
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "normalized-annotation",
                    role: "annotation",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Gff3),
                    media_type: Some("text/x-gff3"),
                },
            )
        }
        "annotation.gene-position.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let feature_types =
                optional_string_array_parameter(&request.parameters, "feature_types")?;
            let options = GenePositionOptions {
                feature_types: if feature_types.is_empty() {
                    GenePositionOptions::default().feature_types
                } else {
                    feature_types
                },
            };
            let summary = annotation_gene_positions_path(input, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "gene-position-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "annotation.gxf.to-bed.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let feature_types =
                optional_string_array_parameter(&request.parameters, "feature_types")?;
            let feature_types = if feature_types.is_empty() {
                vec!["gene".to_owned()]
            } else {
                feature_types
            };
            let summary = gxf_to_bed_path(input, &output, &feature_types)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "bed-output",
                    role: "bed",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bed),
                    media_type: Some("text/x-bed"),
                },
            )
        }
        "annotation.sequence.extract.v1" => {
            let annotation = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let fasta = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = annotation_extract_options(&request.parameters)?;
            let summary = extract_annotation_sequences_path(annotation, fasta, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "annotation-sequences",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "genome.gene-density.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let result = gene_density_path(input, gene_density_options(&request.parameters)?)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "gene-density-warning",
            )
        }
        "annotation.go.normalize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotations")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = normalize_go_annotations_path(
                input,
                &output,
                &go_annotation_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                result,
                FileArtifactSpec {
                    artifact_id: "go-associations",
                    role: "associations",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "annotation.eggnog.normalize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotations")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = normalize_eggnog_path(input, &output)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                result,
                FileArtifactSpec {
                    artifact_id: "eggnog-annotations",
                    role: "annotations",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "annotation.structure.visualize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "annotation")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_annotation_structure_svg_path(
                input,
                &output,
                &annotation_structure_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "annotation-visualization-warning",
                FileArtifactSpec {
                    artifact_id: "annotation-structure-plot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "comparative.synteny.visualize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "anchors")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_synteny_svg_with_options_path(
                input,
                &output,
                &synteny_visualization_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "synteny-warning",
                FileArtifactSpec {
                    artifact_id: "synteny-plot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "comparative.mcscanx.v1" => {
            let gene_positions =
                resolve_v2_single_input(base_directory, &request, "gene-positions")?;
            let similarity_hits =
                resolve_v2_single_input(base_directory, &request, "similarity-hits")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_mcscanx_path(gene_positions, similarity_hits, &output)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "mcscanx-collinearity",
                    role: "collinearity",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::McscanxCollinearity),
                    media_type: Some("text/plain"),
                },
            )
        }
        "comparative.kaks.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "codon-alignment")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let method = optional_parameter_string(&request.parameters, "method")?.unwrap_or("NG");
            let result = run_kaks_path(input, &output, method)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "kaks-estimates",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "comparative.dotplot.v1" => {
            let query = resolve_v2_single_input(base_directory, &request, "query")?;
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_dotplot_svg_path(
                query,
                reference,
                &output,
                &dotplot_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &[],
                "dotplot",
                FileArtifactSpec {
                    artifact_id: "dotplot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "rna.secondary-structure.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "sequence")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let temperature =
                optional_parameter_f64(&request.parameters, "temperature")?.unwrap_or(37.0);
            let result = run_rnafold_path(input, &output, temperature)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "rna-secondary-structure",
                    role: "secondary-structure",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Unknown),
                    media_type: Some("text/plain"),
                },
            )
        }
        "enrichment.overrepresentation.v1"
        | "enrichment.go.v1"
        | "enrichment.kegg.v1"
        | "medical.pathway-ruo.v1" => {
            let genes = resolve_v2_single_input(base_directory, &request, "genes")?;
            let associations = resolve_v2_single_input(base_directory, &request, "associations")?;
            let kind = if request.capability == "medical.pathway-ruo.v1" {
                EnrichmentKind::Custom
            } else {
                enrichment_kind(&request.capability)?
            };
            let result = overrepresentation_path(
                genes,
                associations,
                kind,
                enrichment_options(&request.parameters)?,
            )?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "enrichment-warning",
            )
        }
        "enrichment.gsea.v1" => {
            let ranked = resolve_v2_single_input(base_directory, &request, "ranked")?;
            let gene_sets = resolve_v2_single_input(base_directory, &request, "gene-sets")?;
            let result =
                gsea_preranked_path(ranked, gene_sets, gsea_options(&request.parameters)?)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "gsea-warning",
            )
        }
        "enrichment.visualize.v1" => {
            let genes = resolve_v2_single_input(base_directory, &request, "genes")?;
            let associations = resolve_v2_single_input(base_directory, &request, "associations")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_enrichment_svg_path(
                genes,
                associations,
                &output,
                visualization_enrichment_kind(&request.parameters)?,
                enrichment_options(&request.parameters)?,
                enrichment_visualization_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "enrichment-visualization-warning",
                FileArtifactSpec {
                    artifact_id: "enrichment-plot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "environment.audit.v1" => {
            let audit = audit_environment()?;
            serialize_v2_result(&request, base_directory, &verified_inputs, audit)
        }
        "environment.plan.v1" => {
            let profile = request
                .parameters
                .get("profile")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or("environment plan profile must be a string")
                })
                .transpose()?
                .unwrap_or("full-local");
            let mode = match request.parameters.get("mode") {
                Some(value) => parse_environment_mode(
                    value
                        .as_str()
                        .ok_or("environment plan mode must be a string")?,
                )?,
                None => EnvironmentMode::ManagedUser,
            };
            let project_root = request
                .parameters
                .get("project_root")
                .map(|value| {
                    value
                        .as_str()
                        .map(|path| resolve_input(base_directory, path))
                        .ok_or("environment plan project_root must be a string")
                })
                .transpose()?;
            if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
                return Err("project_root is only valid in project-isolated mode".into());
            }
            let plan = plan_environment_with_options(
                profile,
                &audit_environment()?,
                &EnvironmentPlanOptions { mode, project_root },
            )?;
            serialize_v2_result(&request, base_directory, &verified_inputs, plan)
        }
        "dataset.inspect.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "file")?;
            let max_preview_records = optional_v2_usize_parameter(&request, "max_preview_records")?
                .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_RECORD_LIMIT);
            let max_preview_bytes = optional_v2_u64_parameter(&request, "max_preview_bytes")?
                .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_BYTE_LIMIT);
            let inspection = inspect_dataset_with_options(
                path,
                DatasetInspectionOptions {
                    max_preview_records,
                    max_preview_bytes,
                },
            )?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                inspection.clone(),
                ExecutionMode::LocalCpu,
            );
            result.diagnostics.extend(
                inspection
                    .warnings
                    .iter()
                    .map(|issue| inspection_diagnostic(issue, DiagnosticSeverity::Warning)),
            );
            result.diagnostics.extend(
                inspection
                    .errors
                    .iter()
                    .map(|issue| inspection_diagnostic(issue, DiagnosticSeverity::Error)),
            );
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "fastq.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "fastq")?;
            let metrics = fastq_qc_path(path, fastq_options_v2(&request)?)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "fastq-qc-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "fastq.trim.v1" => {
            let options = fastq_trim_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_trim_path(input, output, &options),
            )
        }
        "fastq.adapter.v1" => {
            let options = fastq_adapter_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_adapter_trim_path(input, output, &options),
            )
        }
        "fastq.deduplicate.v1" => {
            let options = fastq_deduplicate_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_deduplicate_path(input, output, &options),
            )
        }
        "fastq.subsample.v1" => {
            let options = fastq_subsample_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_subsample_path(input, output, &options),
            )
        }
        "expression.matrix.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let metrics = expression_matrix_qc_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "expression-matrix-qc-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "medical.cohort-table.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "cohort")?;
            let metrics = cohort_table_qc_path(path)?;
            serialize_v2_result(&request, base_directory, &verified_inputs, metrics)
        }
        "medical.single-cell-qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let metrics = expression_matrix_qc_path(path)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                metrics.clone(),
                &metrics.warnings,
                "single-cell-qc-warning",
            )
        }
        "expression.normalize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_distinct_input_output(&input, &output)?;
            let options = expression_normalize_options(&request.parameters)?;
            let summary = normalize_expression_matrix_path(&input, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "normalized-expression-matrix",
                    role: "matrix",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "expression.pca.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let result = expression_pca_path(input, &expression_pca_options(&request.parameters)?)?;
            serialize_v2_result(&request, base_directory, &verified_inputs, result)
        }
        "expression.cluster.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let result =
                expression_cluster_path(input, &expression_cluster_options(&request.parameters)?)?;
            serialize_v2_result(&request, base_directory, &verified_inputs, result)
        }
        "expression.heatmap.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let result =
                expression_heatmap_path(input, &expression_heatmap_options(&request.parameters)?)?;
            serialize_v2_result(&request, base_directory, &verified_inputs, result)
        }
        "expression.differential.v1" | "medical.bulk-rnaseq.v1" => {
            workflow::execute_bulk_expression_v2(base_directory, request, &verified_inputs)
        }
        "sequence.convert.biopython.v1" => {
            workflow::execute_sequence_convert_v2(base_directory, request, &verified_inputs)
        }
        "metagenomics.classify.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "reads")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = kraken2_options(&request.parameters)?;
            let result = run_kraken2_path(input, &output, &options)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "abundance-table",
                    role: "abundance",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "expression.volcano.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "differential")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_volcano_svg_path(
                input,
                &output,
                &volcano_plot_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "expression-volcano-warning",
                FileArtifactSpec {
                    artifact_id: "expression-volcano-plot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "motif.visualize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "meme")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_motif_logo_svg_path(input, &output)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "motif-logo-warning",
                FileArtifactSpec {
                    artifact_id: "motif-sequence-logo",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "interval.intersect.v1" => {
            let left = resolve_v2_single_input(base_directory, &request, "left-bed")?;
            let right = resolve_v2_single_input(base_directory, &request, "right-bed")?;
            let stats = bed_intersect_path(left, right)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                stats.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(stats.warnings.iter().map(|message| Diagnostic {
                    code: "interval-intersect-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "interval.merge.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "bed")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let stats = bed_merge_path(
                input,
                &output,
                IntervalMergeOptions {
                    max_gap: optional_parameter_u64(&request.parameters, "max_gap")?.unwrap_or(0),
                },
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                stats,
                FileArtifactSpec {
                    artifact_id: "interval-output",
                    role: "bed",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bed),
                    media_type: Some("text/x-bed"),
                },
            )
        }
        "interval.subtract.v1" => {
            let left = resolve_v2_single_input(base_directory, &request, "left-bed")?;
            let right = resolve_v2_single_input(base_directory, &request, "right-bed")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let stats = bed_subtract_path(left, right, &output)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                stats,
                FileArtifactSpec {
                    artifact_id: "interval-output",
                    role: "bed",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bed),
                    media_type: Some("text/x-bed"),
                },
            )
        }
        "interval.closest.v1" => {
            let query = resolve_v2_single_input(base_directory, &request, "query-bed")?;
            let target = resolve_v2_single_input(base_directory, &request, "target-bed")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = bed_closest_path(query, target, &output)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                summary.clone(),
                &summary.warnings,
                "interval-closest-warning",
                FileArtifactSpec {
                    artifact_id: "closest-intervals",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "table.export.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            let output = required_v2_string_parameter(&request, "output")?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let receipt = export_json_file(&input, &output)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                receipt.clone(),
                ExecutionMode::LocalCpu,
            );
            result.artifacts.push(OutputArtifact {
                artifact_id: "exported-table".to_owned(),
                role: "table".to_owned(),
                kind: OutputArtifactKind::Table,
                path: receipt.output_path,
                format: Some(export_bio_format(receipt.format)),
                media_type: Some(export_media_type(receipt.format).to_owned()),
                size_bytes: Some(receipt.size_bytes),
                sha256: Some(sha256_file(&output)?),
                metadata: Default::default(),
            });
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "table.manipulate.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = table_manipulate_options(&request.parameters)?;
            let summary = manipulate_table_path(&input, &output, &options)?;
            let delimiter = options
                .output_delimiter
                .or_else(|| TableDelimiter::infer_from_path(&output))
                .or(options.input_delimiter)
                .unwrap_or(TableDelimiter::Csv);
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "manipulated-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(table_bio_format(delimiter)),
                    media_type: Some(delimiter.media_type()),
                },
            )
        }
        "sequence.stats.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "fasta")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                fasta_stats_path(path)?,
            )
        }
        "sequence.extract.v1" => {
            let options = sequence_extract_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| extract_fasta_path(input, output, &options),
            )
        }
        "sequence.filter.v1" => {
            let options = sequence_filter_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| filter_fasta_path(input, output, &options),
            )
        }
        "sequence.reverse-complement.v1" => execute_sequence_transform_v2(
            &request,
            base_directory,
            &verified_inputs,
            |input, output| reverse_complement_fasta_path(input, output),
        ),
        "sequence.translate.v1" => {
            let options = sequence_translate_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| translate_fasta_path(input, output, &options),
            )
        }
        "sequence.orf.v1" => {
            let options = sequence_orf_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| find_orfs_fasta_path(input, output, &options),
            )
        }
        "sequence.id.normalize.v1" => {
            let options = sequence_id_normalize_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| normalize_fasta_ids_path(input, output, &options),
            )
        }
        "sequence.merge.v1" => {
            let inputs = resolve_v2_input_files(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = sequence_merge_options(&request.parameters)?;
            let summary = merge_fasta_paths(&inputs, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "sequence.split.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output_directory = request
                .parameters
                .get("output_directory")
                .and_then(serde_json::Value::as_str)
                .ok_or("sequence.split.v1 requires string parameters.output_directory")?;
            let output_directory = resolve_input(base_directory, output_directory);
            let options = sequence_split_options(&request.parameters)?;
            let summary = split_fasta_path(input, &output_directory, &options)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                summary.clone(),
                ExecutionMode::LocalCpu,
            );
            result.artifacts.push(OutputArtifact {
                artifact_id: "sequence-output-directory".to_owned(),
                role: "fasta-directory".to_owned(),
                kind: OutputArtifactKind::Directory,
                path: output_directory.to_string_lossy().into_owned(),
                format: None,
                media_type: None,
                size_bytes: None,
                sha256: None,
                metadata: BTreeMap::from([(
                    "file_count".to_owned(),
                    serde_json::json!(summary.output_files),
                )]),
            });
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "sequence.to-table.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let delimiter =
                sequence_table_delimiter_option(&request.parameters)?.unwrap_or_else(|| {
                    SequenceTableDelimiter::infer_from_path(&output)
                        .unwrap_or(SequenceTableDelimiter::Csv)
                });
            let summary = fasta_to_table_path(
                input,
                &output,
                &SequenceToTableOptions {
                    delimiter,
                    include_header: optional_parameter_bool(&request.parameters, "include_header")?
                        .unwrap_or(true),
                },
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(sequence_table_format(delimiter)),
                    media_type: Some(delimiter.media_type()),
                },
            )
        }
        "sequence.from-table.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let mut options = sequence_from_table_options(&request.parameters)?;
            if sequence_table_delimiter_option(&request.parameters)?.is_none() {
                options.delimiter = SequenceTableDelimiter::infer_from_path(&input)
                    .unwrap_or(SequenceTableDelimiter::Csv);
            }
            let summary = table_to_fasta_path(input, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "sequence.kmer.count.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary =
                count_kmers_path(input, &output, &kmer_count_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "kmer-count-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "sequence.consensus.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = consensus_from_alignment_path(
                input,
                &output,
                &consensus_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "consensus-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "sequence.shuffle.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary =
                shuffle_sequences_path(input, &output, &shuffle_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "shuffle-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "primer.epcr.v1" => {
            let fasta = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let primers = resolve_v2_single_input(base_directory, &request, "primers")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = epcr_path(fasta, primers, &output, &epcr_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "epcr-amplicon-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "set.venn.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                venn_analysis_path(input, set_analysis_options(&request.parameters)?)?,
            )
        }
        "set.upset.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                upset_analysis_path(input, set_analysis_options(&request.parameters)?)?,
            )
        }
        "similarity.blast.local.v1" => {
            let query = resolve_v2_single_input(base_directory, &request, "query")?;
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_blast_fasta_path(
                query,
                reference,
                &output,
                blast_program(&request.parameters)?,
                &similarity_search_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "blast-results",
                    role: "hits",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::BlastTabular),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "similarity.diamond.v1" => {
            let query = resolve_v2_single_input(base_directory, &request, "query")?;
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_diamond_fasta_path(
                query,
                reference,
                &output,
                diamond_mode(&request.parameters)?,
                &similarity_search_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "diamond-results",
                    role: "hits",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::BlastTabular),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "similarity.hmmer.v1" => {
            let profile = resolve_v2_single_input(base_directory, &request, "profile")?;
            let sequences = resolve_v2_single_input(base_directory, &request, "sequences")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_hmmer_path(
                profile,
                sequences,
                &output,
                hmmer_mode(&request.parameters)?,
                &hmmer_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "hmmer-domains",
                    role: "domains",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::ProteinDomains),
                    media_type: Some("text/plain"),
                },
            )
        }
        "similarity.blast.parse.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "blast")?;
            let result = parse_blast_path(input)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "blast-parse-warning",
            )
        }
        "similarity.reciprocal.v1" => {
            let forward = resolve_v2_single_input(base_directory, &request, "forward")?;
            let reverse = resolve_v2_single_input(base_directory, &request, "reverse")?;
            let result = reciprocal_best_hits_path(
                forward,
                reverse,
                reciprocal_best_hit_options(&request.parameters)?,
            )?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "reciprocal-best-hit-warning",
            )
        }
        "protein.properties.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let properties = protein_properties_path(input)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                properties.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(properties.warnings.iter().map(|message| Diagnostic {
                    code: "protein-properties-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "protein.domain.parse.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "domains")?;
            let result = parse_protein_domains_path(input)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "protein-domain-warning",
            )
        }
        "protein.domain.visualize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "domains")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = render_domain_architecture_svg_path(
                input,
                &output,
                &domain_architecture_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "protein-domain-visualization-warning",
                FileArtifactSpec {
                    artifact_id: "protein-domain-plot",
                    role: "plot",
                    kind: OutputArtifactKind::Plot,
                    path: output,
                    format: Some(BioDataFormat::Svg),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "msa.muscle.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_muscle_path(input, &output, &muscle_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "multiple-sequence-alignment",
                    role: "alignment",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "msa.trimal.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "alignment")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let mode = parse_trimal_mode(
                optional_parameter_string(&request.parameters, "mode")?.unwrap_or("automated1"),
            )?;
            let result = run_trimal_path(input, &output, mode)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "trimmed-alignment",
                    role: "alignment",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "phylogeny.iqtree.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "alignment")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_iqtree_path(input, &output, &iqtree_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "maximum-likelihood-tree",
                    role: "tree",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Newick),
                    media_type: Some("text/x-newick"),
                },
            )
        }
        "motif.meme.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_meme_path(input, &output, &meme_options(&request.parameters)?)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "meme-motifs",
                    role: "motifs",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: None,
                    media_type: Some("text/plain"),
                },
            )
        }
        "protein.secondary-structure.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "structure")?;
            let output = resolve_input(
                base_directory,
                required_sequence_output(&request.parameters, &request.capability)?,
            );
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = run_dssp_path(input, &output)?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "native-tool-warning",
                FileArtifactSpec {
                    artifact_id: "secondary-structure",
                    role: "secondary-structure",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: None,
                    media_type: Some("text/plain"),
                },
            )
        }
        "phylogeny.tree.transform.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "tree")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = transform_newick_path(
                input,
                &output,
                tree_transform_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "phylogeny-tree-warning",
                FileArtifactSpec {
                    artifact_id: "transformed-tree",
                    role: "tree",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Unknown),
                    media_type: Some("text/x-newick"),
                },
            )
        }
        "phylogeny.tree.visualize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "tree")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = tree_visualization_options(&request.parameters)?;
            let result = render_tree_svg_path(input, &output, &options)
                .map_err(|error| -> WorkerError { error.to_string().into() })?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "phylogeny-tree-visualize-warning",
                FileArtifactSpec {
                    artifact_id: "tree-visualization",
                    role: "visualization",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Unknown),
                    media_type: Some("image/svg+xml"),
                },
            )
        }
        "phylogeny.distance.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "alignment")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let result = distance_matrix_path(
                input,
                &output,
                &distance_matrix_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "phylogeny-distance-warning",
                FileArtifactSpec {
                    artifact_id: "distance-matrix",
                    role: "distances",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        "structure.pdb.summary.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "pdb")?;
            let summary = pdb_summary_path(path, pdb_options(&request.parameters)?)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                summary.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(summary.warnings.iter().map(|message| Diagnostic {
                    code: "pdb-summary-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "structure.mmcif.summary.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "structure")?;
            let summary = mmcif_summary_path(path)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                summary.clone(),
                &summary.warnings,
                "mmcif-summary-warning",
            )
        }
        "structure.sequence.extract.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "structure")?;
            let result = extract_structure_sequences_path(path)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "structure-sequence-warning",
            )
        }
        "structure.contact-map.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "structure")?;
            let result =
                structure_contact_map_path(path, contact_map_options(&request.parameters)?)?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "structure-contact-warning",
            )
        }
        "structure.geometry.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "structure")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                measure_structure_geometry_path(path, &geometry_selectors(&request.parameters)?)?,
            )
        }
        "structure.superpose.v1" => {
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let mobile = resolve_v2_single_input(base_directory, &request, "mobile")?;
            let result = superpose_structures_path(
                reference,
                mobile,
                superposition_options(&request.parameters)?,
            )?;
            serialize_v2_with_warnings(
                &request,
                base_directory,
                &verified_inputs,
                result.clone(),
                &result.warnings,
                "structure-superposition-warning",
            )
        }
        "variant.stats.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let stats = vcf_stats_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                stats.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(stats.warnings.iter().map(|message| Diagnostic {
                    code: "variant-stats-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "variant.compare.v1" => {
            let left = resolve_v2_single_input(base_directory, &request, "left-vcf")?;
            let right = resolve_v2_single_input(base_directory, &request, "right-vcf")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                compare_vcf_paths(left, right)?,
            )
        }
        "medical.variant-cohort.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let stats = vcf_stats_path(path)?;
            serialize_v2_result(&request, base_directory, &verified_inputs, stats)
        }
        "variant.filter.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = filter_vcf_path(
                input,
                &output,
                &variant_filter_options(&request.parameters)?,
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "filtered-vcf",
                    role: "vcf",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Vcf),
                    media_type: Some("text/x-vcf"),
                },
            )
        }
        "variant.normalize.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let reference = resolve_v2_single_input(base_directory, &request, "reference")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = normalize_vcf_path(input, reference, &output)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "normalized-vcf",
                    role: "vcf",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Vcf),
                    media_type: Some("text/x-vcf"),
                },
            )
        }
        "variant.to-table.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let summary = vcf_to_table_path(input, &output)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "variant-table",
                    role: "tsv",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Tsv),
                    media_type: Some("text/tab-separated-values"),
                },
            )
        }
        capability => Err(format!("unsupported capability: {capability}").into()),
    }
}

fn validate_v2_contract(request: &JobRequestV2) -> WorkerResult<()> {
    let (required_roles, allowed_parameters): (&[&str], &[&str]) = match request.capability.as_str()
    {
        "alignment.qc.v1" => (&["sam"], &[]),
        "alignment.bam-cram.qc.v1" | "alignment.coverage.v1" => (&["alignment"], &["output"]),
        "alignment.bam-to-bigwig.v1" => (&["alignment"], &["output", "threads"]),
        "alignment.short-read.v1" => (&["reference", "reads"], &["output", "threads"]),
        "annotation.gxf.stats.v1" => (&["annotation"], &[]),
        "annotation.gxf.normalize.v1" => (&["annotation"], &["output", "sort"]),
        "annotation.gene-position.v1" => (&["annotation"], &["output", "feature_types"]),
        "annotation.sequence.extract.v1" => (
            &["annotation", "fasta"],
            &["output", "feature_type", "promoter_length"],
        ),
        "genome.gene-density.v1" => (
            &["annotation"],
            &["feature_types", "window_size", "step_size"],
        ),
        "environment.audit.v1" => (&[], &[]),
        "environment.plan.v1" => (&[], &["profile", "mode", "project_root"]),
        "dataset.inspect.v1" => (&["file"], &["max_preview_records", "max_preview_bytes"]),
        "fastq.qc.v1" => (&["fastq"], &["max_cycles", "quality_encoding"]),
        "fastq.trim.v1" => (
            &["fastq"],
            &["output", "min_quality", "min_length", "quality_encoding"],
        ),
        "fastq.adapter.v1" => (
            &["fastq"],
            &["output", "adapter", "adapters", "min_overlap", "min_length"],
        ),
        "fastq.deduplicate.v1" => (
            &["fastq"],
            &["output", "header_umi_delimiter", "sequence_prefix_umi"],
        ),
        "fastq.subsample.v1" => (&["fastq"], &["output", "target_count", "fraction", "seed"]),
        "expression.matrix.qc.v1" => (&["matrix"], &[]),
        "medical.cohort-table.qc.v1" => (&["cohort"], &[]),
        "medical.single-cell-qc.v1" => (&["matrix"], &[]),
        "expression.normalize.v1" => (&["matrix"], &["output", "method", "pseudocount"]),
        "expression.pca.v1" => (&["matrix"], &["components", "scale_features"]),
        "expression.cluster.v1" => (
            &["matrix"],
            &[
                "sample_clusters",
                "feature_clusters",
                "max_iterations",
                "scale_features",
            ],
        ),
        "expression.heatmap.v1" => (&["matrix"], &["top_variable_features", "scale_rows"]),
        "expression.differential.v1" | "medical.bulk-rnaseq.v1" => (
            &["counts", "sample_metadata"],
            &[
                "output_directory",
                "feature_id_column",
                "sample_id_column",
                "condition_column",
                "reference_level",
                "contrast_level",
                "alpha",
                "min_total_count",
            ],
        ),
        "sequence.convert.biopython.v1" => (
            &["sequences"],
            &["output_directory", "output_filename", "output_format"],
        ),
        "metagenomics.classify.v1" => (
            &["reads"],
            &[
                "output",
                "database",
                "confidence",
                "minimum_hit_groups",
                "threads",
            ],
        ),
        "expression.volcano.v1" => (
            &["differential"],
            &["output", "padj", "log2_fold_change", "max_points"],
        ),
        "motif.visualize.v1" => (&["meme"], &["output"]),
        "interval.intersect.v1" => (&["left-bed", "right-bed"], &[]),
        "interval.merge.v1" => (&["bed"], &["output", "max_gap"]),
        "interval.subtract.v1" => (&["left-bed", "right-bed"], &["output"]),
        "interval.closest.v1" => (&["query-bed", "target-bed"], &["output"]),
        "table.export.v1" => (&["table"], &["output"]),
        "table.manipulate.v1" => (
            &["table"],
            &[
                "output",
                "delimiter",
                "output_delimiter",
                "select_columns",
                "drop_columns",
                "filter_column",
                "filter_op",
                "filter_value",
                "skip_rows",
                "limit",
            ],
        ),
        "sequence.stats.v1" => (&["fasta"], &[]),
        "sequence.extract.v1" => (&["fasta"], &["output", "identifiers", "regions", "strict"]),
        "sequence.filter.v1" => (
            &["fasta"],
            &[
                "output",
                "min_length",
                "max_length",
                "min_gc_percent",
                "max_gc_percent",
                "max_n_percent",
            ],
        ),
        "sequence.reverse-complement.v1" => (&["fasta"], &["output"]),
        "sequence.translate.v1" => (
            &["fasta"],
            &["output", "frames", "trim_terminal_stop", "stop_at_first"],
        ),
        "sequence.orf.v1" => (
            &["fasta"],
            &[
                "output",
                "min_amino_acids",
                "include_reverse_strand",
                "include_partial_3prime",
            ],
        ),
        "sequence.id.normalize.v1" => (
            &["fasta"],
            &["output", "prefix", "start", "width", "keep_description"],
        ),
        "sequence.merge.v1" => (&["fasta"], &["output", "allow_duplicate_ids"]),
        "sequence.split.v1" => (
            &["fasta"],
            &["output_directory", "records_per_file", "prefix"],
        ),
        "sequence.to-table.v1" => (&["fasta"], &["output", "delimiter", "include_header"]),
        "sequence.from-table.v1" => (
            &["table"],
            &[
                "output",
                "delimiter",
                "id_column",
                "sequence_column",
                "description_column",
            ],
        ),
        "sequence.kmer.count.v1" => (&["fasta"], &["output", "k", "canonical", "top_n"]),
        "sequence.consensus.v1" => (&["fasta"], &["output", "threshold"]),
        "sequence.shuffle.v1" => (&["fasta"], &["output", "seed"]),
        "primer.epcr.v1" => (
            &["fasta", "primers"],
            &["output", "min_amplicon", "max_amplicon", "max_hits"],
        ),
        "set.venn.v1" | "set.upset.v1" => (&["table"], &["include_items", "max_intersections"]),
        "similarity.blast.parse.v1" => (&["blast"], &[]),
        "similarity.blast.local.v1" => (
            &["query", "reference"],
            &[
                "output",
                "program",
                "threads",
                "evalue",
                "max_target_sequences",
                "outfmt",
            ],
        ),
        "similarity.diamond.v1" => (
            &["query", "reference"],
            &[
                "output",
                "mode",
                "threads",
                "evalue",
                "max_target_sequences",
                "outfmt",
            ],
        ),
        "similarity.hmmer.v1" => (
            &["profile", "sequences"],
            &["output", "mode", "threads", "evalue"],
        ),
        "similarity.reciprocal.v1" => (
            &["forward", "reverse"],
            &["max_evalue", "min_identity_percent"],
        ),
        "protein.properties.v1" => (&["fasta"], &[]),
        "protein.domain.parse.v1" => (&["domains"], &[]),
        "protein.domain.visualize.v1" => (
            &["domains"],
            &["output", "sequence_id", "max_sequences", "max_domains"],
        ),
        "phylogeny.tree.transform.v1" => (&["tree"], &["output", "reroot_label", "label_map"]),
        "phylogeny.tree.visualize.v1" => (
            &["tree"],
            &[
                "output",
                "width",
                "height",
                "font_size",
                "show_branch_lengths",
            ],
        ),
        "phylogeny.distance.v1" => (&["alignment"], &["output", "model"]),
        "msa.muscle.v1" => (&["fasta"], &["output", "mode", "threads"]),
        "msa.trimal.v1" => (&["alignment"], &["output", "mode"]),
        "phylogeny.iqtree.v1" => (&["alignment"], &["output", "threads", "model", "seed"]),
        "motif.meme.v1" => (
            &["fasta"],
            &[
                "output",
                "threads",
                "alphabet",
                "distribution",
                "motif_count",
                "minimum_width",
                "maximum_width",
            ],
        ),
        "protein.secondary-structure.v1" => (&["structure"], &["output"]),
        "annotation.go.normalize.v1" => (&["annotations"], &["output", "gene_column", "go_column"]),
        "annotation.eggnog.normalize.v1" => (&["annotations"], &["output"]),
        "annotation.structure.visualize.v1" => (
            &["annotation"],
            &["output", "feature_id", "seqid", "max_features"],
        ),
        "comparative.synteny.visualize.v1" => (&["anchors"], &["output", "style"]),
        "comparative.mcscanx.v1" => (&["gene-positions", "similarity-hits"], &["output"]),
        "comparative.kaks.v1" => (&["codon-alignment"], &["output", "method"]),
        "comparative.dotplot.v1" => (
            &["query", "reference"],
            &["output", "width", "height", "kmer"],
        ),
        "rna.secondary-structure.v1" => (&["sequence"], &["output", "temperature"]),
        "enrichment.overrepresentation.v1" | "enrichment.go.v1" | "enrichment.kegg.v1" => (
            &["genes", "associations"],
            &["min_overlap", "max_terms", "include_genes"],
        ),
        "enrichment.gsea.v1" => (
            &["ranked", "gene-sets"],
            &[
                "score_exponent",
                "min_set_size",
                "max_set_size",
                "permutations",
                "seed",
            ],
        ),
        "medical.pathway-ruo.v1" => (
            &["genes", "associations"],
            &["min_overlap", "max_terms", "include_genes"],
        ),
        "enrichment.visualize.v1" => (
            &["genes", "associations"],
            &["output", "kind", "style", "min_overlap", "max_terms"],
        ),
        "structure.pdb.summary.v1" => (&["pdb"], &["interpret_b_factors_as_plddt"]),
        "structure.mmcif.summary.v1" | "structure.sequence.extract.v1" => (&["structure"], &[]),
        "structure.contact-map.v1" => (
            &["structure"],
            &["cutoff_angstrom", "atom_name", "include_inter_chain"],
        ),
        "structure.geometry.v1" => (&["structure"], &["atoms"]),
        "structure.superpose.v1" => (&["reference", "mobile"], &["atom_name"]),
        "variant.stats.v1" => (&["vcf"], &[]),
        "variant.compare.v1" => (&["left-vcf", "right-vcf"], &[]),
        "medical.variant-cohort.v1" => (&["vcf"], &[]),
        "variant.filter.v1" => (
            &["vcf"],
            &[
                "output",
                "min_qual",
                "require_pass",
                "contigs",
                "min_info_dp",
            ],
        ),
        "variant.normalize.v1" => (&["vcf", "reference"], &["output"]),
        "variant.to-table.v1" => (&["vcf"], &["output"]),
        capability => return Err(format!("unsupported capability: {capability}").into()),
    };

    let mut artifact_ids = HashSet::new();
    let mut roles = HashSet::new();
    for artifact in &request.inputs {
        if artifact.artifact_id.trim().is_empty() || artifact.role.trim().is_empty() {
            return Err("v2 input artifacts require non-empty artifact_id and role".into());
        }
        if !artifact_ids.insert(artifact.artifact_id.as_str()) {
            return Err(format!("duplicate input artifact_id: {}", artifact.artifact_id).into());
        }
        if !roles.insert(artifact.role.as_str()) {
            return Err(format!("duplicate input role: {}", artifact.role).into());
        }
    }
    for role in required_roles {
        if !roles.contains(role) {
            return Err(format!("{} requires input role {role}", request.capability).into());
        }
    }
    for role in roles {
        if !required_roles.contains(&role) {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }

    let parameters = match &request.parameters {
        serde_json::Value::Null => return Ok(()),
        serde_json::Value::Object(parameters) => parameters,
        _ => return Err("v2 parameters must be an object".into()),
    };
    for parameter in parameters.keys() {
        if !allowed_parameters.contains(&parameter.as_str()) {
            return Err(format!(
                "{} does not accept parameter {parameter}",
                request.capability
            )
            .into());
        }
    }
    Ok(())
}

fn serialize_v2_result<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        value,
        ExecutionMode::LocalCpu,
    );
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn serialize_v2_with_warnings<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
    warnings: &[String],
    diagnostic_code: &str,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        value,
        ExecutionMode::LocalCpu,
    );
    result
        .diagnostics
        .extend(warnings.iter().map(|message| Diagnostic {
            code: diagnostic_code.to_owned(),
            severity: DiagnosticSeverity::Warning,
            message: message.clone(),
            artifact_id: None,
            line: None,
            record: None,
            hint: None,
        }));
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

struct FileArtifactSpec {
    artifact_id: &'static str,
    role: &'static str,
    kind: OutputArtifactKind,
    path: PathBuf,
    format: Option<BioDataFormat>,
    media_type: Option<&'static str>,
}

fn serialize_v2_file_artifact_result<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
    artifact: FileArtifactSpec,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        value,
        ExecutionMode::LocalCpu,
    );
    result.artifacts.push(OutputArtifact {
        artifact_id: artifact.artifact_id.to_owned(),
        role: artifact.role.to_owned(),
        kind: artifact.kind,
        path: artifact.path.to_string_lossy().into_owned(),
        format: artifact.format,
        media_type: artifact.media_type.map(str::to_owned),
        size_bytes: Some(std::fs::metadata(&artifact.path)?.len()),
        sha256: Some(sha256_file(&artifact.path)?),
        metadata: Default::default(),
    });
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn serialize_v2_file_artifact_result_with_warnings<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
    warnings: &[String],
    diagnostic_code: &str,
    artifact: FileArtifactSpec,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        value,
        ExecutionMode::LocalCpu,
    );
    result
        .diagnostics
        .extend(warnings.iter().map(|message| Diagnostic {
            code: diagnostic_code.to_owned(),
            severity: DiagnosticSeverity::Warning,
            message: message.clone(),
            artifact_id: None,
            line: None,
            record: None,
            hint: None,
        }));
    result.artifacts.push(OutputArtifact {
        artifact_id: artifact.artifact_id.to_owned(),
        role: artifact.role.to_owned(),
        kind: artifact.kind,
        path: artifact.path.to_string_lossy().into_owned(),
        format: artifact.format,
        media_type: artifact.media_type.map(str::to_owned),
        size_bytes: Some(std::fs::metadata(&artifact.path)?.len()),
        sha256: Some(sha256_file(&artifact.path)?),
        metadata: Default::default(),
    });
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn validate_v2_inputs(
    request: &JobRequestV2,
    base_directory: &Path,
) -> WorkerResult<BTreeMap<String, String>> {
    let mut file_ids = HashSet::new();
    let mut hashes = BTreeMap::new();
    for artifact in &request.inputs {
        for file in &artifact.files {
            if !file_ids.insert(file.file_id.clone()) {
                return Err(format!("duplicate input file_id: {}", file.file_id).into());
            }
            let path = resolve_input(base_directory, &file.path);
            let actual_size = std::fs::metadata(&path)?.len();
            if actual_size != file.size_bytes {
                return Err(format!(
                    "input {} size mismatch: request declares {} bytes but file has {} bytes",
                    file.file_id, file.size_bytes, actual_size
                )
                .into());
            }
            validate_v2_artifact_declaration(file, &path)?;
            let actual_hash = sha256_file(&path)?;
            if let Some(expected_hash) = &file.sha256
                && !actual_hash.eq_ignore_ascii_case(expected_hash)
            {
                return Err(format!(
                    "input {} SHA-256 mismatch: expected {} but found {}",
                    file.file_id, expected_hash, actual_hash
                )
                .into());
            }
            hashes.insert(file.file_id.clone(), actual_hash);
        }
    }
    Ok(hashes)
}

fn validate_v2_artifact_declaration(file: &ArtifactFile, path: &Path) -> WorkerResult<()> {
    let inspection = inspect_dataset_with_options(
        path,
        DatasetInspectionOptions {
            max_preview_records: 1,
            max_preview_bytes: 64 * 1024,
        },
    )?;

    if format_declaration_conflicts(file.format, inspection.format, inspection.confidence) {
        let declared_format = format!("{:?}", file.format).to_ascii_lowercase();
        return Err(format!(
            "input {} format mismatch: request declares {} but content identifies {}",
            file.file_id, declared_format, inspection.format
        )
        .into());
    }

    if compression_declaration_conflicts(file.compression, inspection.compression) {
        return Err(format!(
            "input {} compression mismatch: request declares {} but signature identifies {}",
            file.file_id,
            compression_format_name(file.compression),
            dataset_compression_name(inspection.compression)
        )
        .into());
    }

    Ok(())
}

fn format_declaration_conflicts(
    declared: BioDataFormat,
    actual: DatasetFormat,
    confidence: DetectionConfidence,
) -> bool {
    if declared == BioDataFormat::Unknown
        || actual == DatasetFormat::Unknown
        || matches!(
            confidence,
            DetectionConfidence::Low | DetectionConfidence::None
        )
    {
        return false;
    }

    match declared_dataset_format(declared) {
        Some(expected) => !dataset_formats_are_compatible(expected, actual),
        None if declared == BioDataFormat::Xlsx && actual == DatasetFormat::Zip => false,
        // Unsupported declarations are contradicted only by a strong, known content signature.
        None => confidence == DetectionConfidence::High,
    }
}

fn declared_dataset_format(format: BioDataFormat) -> Option<DatasetFormat> {
    Some(match format {
        BioDataFormat::Fasta => DatasetFormat::Fasta,
        BioDataFormat::Fastq => DatasetFormat::Fastq,
        BioDataFormat::Csv => DatasetFormat::Csv,
        BioDataFormat::Tsv => DatasetFormat::Tsv,
        BioDataFormat::Bed => DatasetFormat::Bed,
        BioDataFormat::Gff3 => DatasetFormat::Gff3,
        BioDataFormat::Gtf => DatasetFormat::Gtf,
        BioDataFormat::Vcf => DatasetFormat::Vcf,
        BioDataFormat::Sam => DatasetFormat::Sam,
        BioDataFormat::Bam => DatasetFormat::Bam,
        BioDataFormat::Bcf => DatasetFormat::Bcf,
        BioDataFormat::Cram => DatasetFormat::Cram,
        BioDataFormat::Bigwig => DatasetFormat::Bigwig,
        BioDataFormat::H5ad => DatasetFormat::H5ad,
        BioDataFormat::Loom => DatasetFormat::Loom,
        BioDataFormat::Hdf5 => DatasetFormat::Hdf5,
        BioDataFormat::Rds => DatasetFormat::Rds,
        BioDataFormat::Pdb => DatasetFormat::Pdb,
        BioDataFormat::Mmcif => DatasetFormat::Mmcif,
        BioDataFormat::BlastTabular => DatasetFormat::BlastTabular,
        BioDataFormat::BlastXml => DatasetFormat::BlastXml,
        BioDataFormat::ProteinDomains => DatasetFormat::ProteinDomains,
        BioDataFormat::MemeText => DatasetFormat::MemeText,
        BioDataFormat::Axt => DatasetFormat::Axt,
        BioDataFormat::McscanxCollinearity => DatasetFormat::McscanxCollinearity,
        BioDataFormat::Newick => DatasetFormat::Newick,
        BioDataFormat::Zip => DatasetFormat::Zip,
        BioDataFormat::Genbank
        | BioDataFormat::Embl
        | BioDataFormat::Svg
        | BioDataFormat::Xlsx
        | BioDataFormat::Json
        | BioDataFormat::Jsonl
        | BioDataFormat::Parquet
        | BioDataFormat::Unknown => return None,
    })
}

fn dataset_formats_are_compatible(declared: DatasetFormat, actual: DatasetFormat) -> bool {
    declared == actual
        || matches!(
            (declared, actual),
            (
                DatasetFormat::H5ad | DatasetFormat::Loom | DatasetFormat::Hdf5,
                DatasetFormat::H5ad | DatasetFormat::Loom | DatasetFormat::Hdf5
            )
        )
}

fn compression_declaration_conflicts(
    declared: CompressionFormat,
    actual: DatasetCompression,
) -> bool {
    match declared {
        CompressionFormat::Unknown => false,
        CompressionFormat::None => actual != DatasetCompression::None,
        CompressionFormat::Gzip => actual != DatasetCompression::Gzip,
        CompressionFormat::Bgzip => actual != DatasetCompression::Bgzip,
        CompressionFormat::Zip => actual != DatasetCompression::Zip,
        // These formats are valid protocol values, but the current inspector cannot verify them.
        // Reject the declaration instead of recording unverified compression provenance.
        CompressionFormat::Bzip2 | CompressionFormat::Xz | CompressionFormat::Zstd => true,
    }
}

fn compression_format_name(format: CompressionFormat) -> &'static str {
    match format {
        CompressionFormat::None => "none",
        CompressionFormat::Gzip => "gzip",
        CompressionFormat::Bgzip => "bgzip",
        CompressionFormat::Bzip2 => "bzip2",
        CompressionFormat::Xz => "xz",
        CompressionFormat::Zstd => "zstd",
        CompressionFormat::Zip => "zip",
        CompressionFormat::Unknown => "unknown",
    }
}

fn dataset_compression_name(compression: DatasetCompression) -> &'static str {
    match compression {
        DatasetCompression::None => "none",
        DatasetCompression::Gzip => "gzip",
        DatasetCompression::Bgzip => "bgzip",
        DatasetCompression::Zip => "zip",
    }
}

fn finalize_v2_input_hashes<T>(
    result: &mut AnalysisResultV2<T>,
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<()>
where
    T: serde::Serialize,
{
    for artifact in &request.inputs {
        for file in &artifact.files {
            let path = resolve_input(base_directory, &file.path);
            let final_hash = sha256_file(&path)?;
            let initial_hash = verified_inputs
                .get(&file.file_id)
                .ok_or_else(|| format!("input {} was not verified", file.file_id))?;
            if &final_hash != initial_hash {
                return Err(
                    format!("input {} changed while the job was running", file.file_id).into(),
                );
            }
        }
    }
    result.provenance.input_sha256 = verified_inputs.clone();
    Ok(())
}

fn sha256_file(path: &Path) -> WorkerResult<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("write to String");
    }
    Ok(encoded)
}

fn resolve_v2_single_input(
    base_directory: &Path,
    request: &JobRequestV2,
    role: &str,
) -> WorkerResult<PathBuf> {
    let mut matching = request
        .inputs
        .iter()
        .filter(|artifact| artifact.role == role);
    let artifact = matching.next().ok_or_else(|| {
        format!(
            "{} requires an input artifact with role {role}",
            request.capability
        )
    })?;
    if matching.next().is_some() {
        return Err(format!("duplicate input role: {role}").into());
    }
    if artifact.files.len() != 1 {
        return Err(format!("input role {role} requires exactly one file").into());
    }
    Ok(resolve_input(base_directory, &artifact.files[0].path))
}

fn resolve_v2_input_files(
    base_directory: &Path,
    request: &JobRequestV2,
    role: &str,
) -> WorkerResult<Vec<PathBuf>> {
    let mut matching = request
        .inputs
        .iter()
        .filter(|artifact| artifact.role == role);
    let artifact = matching.next().ok_or_else(|| {
        format!(
            "{} requires an input artifact with role {role}",
            request.capability
        )
    })?;
    if matching.next().is_some() {
        return Err(format!("duplicate input role: {role}").into());
    }
    if artifact.files.is_empty() {
        return Err(format!("input role {role} requires at least one file").into());
    }
    Ok(artifact
        .files
        .iter()
        .map(|file| resolve_input(base_directory, &file.path))
        .collect())
}

fn sequence_table_format(delimiter: SequenceTableDelimiter) -> BioDataFormat {
    match delimiter {
        SequenceTableDelimiter::Csv => BioDataFormat::Csv,
        SequenceTableDelimiter::Tsv => BioDataFormat::Tsv,
    }
}

fn ensure_v2_export_output_is_distinct(
    request: &JobRequestV2,
    base_directory: &Path,
    output: &Path,
) -> WorkerResult<()> {
    for artifact in &request.inputs {
        for file in &artifact.files {
            ensure_distinct_input_output(&resolve_input(base_directory, &file.path), output)?;
        }
    }
    Ok(())
}

fn inspection_diagnostic(
    issue: &linxira_bio_core::dataset::InspectionIssue,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    Diagnostic {
        code: issue.code.clone(),
        severity,
        message: issue.message.clone(),
        artifact_id: None,
        line: issue.line,
        record: None,
        hint: None,
    }
}

fn optional_v2_u64_parameter(request: &JobRequestV2, key: &str) -> WorkerResult<Option<u64>> {
    match request.parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_v2_usize_parameter(request: &JobRequestV2, key: &str) -> WorkerResult<Option<usize>> {
    optional_v2_u64_parameter(request, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn run_dataset_inspection(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("file")
        .ok_or("dataset.inspect.v1 requires inputs.file")?;
    let max_preview_records = optional_usize_parameter(&request, "max_preview_records")?
        .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_RECORD_LIMIT);
    let max_preview_bytes = optional_u64_parameter(&request, "max_preview_bytes")?
        .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_BYTE_LIMIT);
    let inspection = inspect_dataset_with_options(
        resolve_input(base_directory, input),
        DatasetInspectionOptions {
            max_preview_records,
            max_preview_bytes,
        },
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        inspection.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = inspection
        .warnings
        .iter()
        .map(|warning| warning.message.clone())
        .collect();
    Ok(serde_json::to_string(&result)?)
}

fn run_table_export(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("json")
        .ok_or("table.export.v1 requires inputs.json")?;
    let output = request
        .parameters
        .get("output")
        .and_then(serde_json::Value::as_str)
        .ok_or("table.export.v1 requires string parameters.output")?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    for declared_input in request.inputs.values() {
        ensure_distinct_input_output(&resolve_input(base_directory, declared_input), &output)?;
    }
    let receipt = export_json_file(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        receipt,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_table_manipulate(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "table",
        &[
            "output",
            "delimiter",
            "output_delimiter",
            "select_columns",
            "drop_columns",
            "filter_column",
            "filter_op",
            "filter_value",
            "skip_rows",
            "limit",
        ],
    )?;
    let input = request
        .inputs
        .get("table")
        .ok_or("table.manipulate.v1 requires inputs.table")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let options = table_manipulate_options(&request.parameters)?;
    let summary = manipulate_table_path(&input, &output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_fastq_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("fastq")
        .ok_or("fastq.qc.v1 requires inputs.fastq")?;
    let metrics = fastq_qc_path(
        resolve_input(base_directory, input),
        fastq_options_v1(&request)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_fastq_trim(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "min_quality", "min_length", "quality_encoding"],
    )?;
    let options = fastq_trim_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_trim_path(input, output, &options)
    })
}

fn run_fastq_adapter_trim(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "adapter", "adapters", "min_overlap", "min_length"],
    )?;
    let options = fastq_adapter_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_adapter_trim_path(input, output, &options)
    })
}

fn run_fastq_deduplicate(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "header_umi_delimiter", "sequence_prefix_umi"],
    )?;
    let options = fastq_deduplicate_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_deduplicate_path(input, output, &options)
    })
}

fn run_fastq_subsample(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "target_count", "fraction", "seed"],
    )?;
    let options = fastq_subsample_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_subsample_path(input, output, &options)
    })
}

fn run_alignment_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("sam")
        .ok_or("alignment.qc.v1 requires inputs.sam")?;
    let metrics = sam_qc_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_annotation_stats(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "annotation", &[])?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("annotation.gxf.stats.v1 requires inputs.annotation")?;
    let stats = annotation_stats_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_annotation_normalize(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "annotation", &["output", "sort"])?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("annotation.gxf.normalize.v1 requires inputs.annotation")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let summary = normalize_annotation_path(
        input,
        output,
        AnnotationNormalizeOptions {
            sort: optional_parameter_bool(&request.parameters, "sort")?.unwrap_or(false),
        },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_annotation_positions(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "annotation", &["output", "feature_types"])?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("annotation.gene-position.v1 requires inputs.annotation")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let feature_types = optional_string_array_parameter(&request.parameters, "feature_types")?;
    let options = GenePositionOptions {
        feature_types: if feature_types.is_empty() {
            GenePositionOptions::default().feature_types
        } else {
            feature_types
        },
    };
    let summary = annotation_gene_positions_path(input, output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_gxf_to_bed(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "annotation", &["output", "feature_types"])?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("annotation.gxf.to-bed.v1 requires inputs.annotation")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let feature_types = optional_string_array_parameter(&request.parameters, "feature_types")?;
    let feature_types = if feature_types.is_empty() {
        vec!["gene".to_owned()]
    } else {
        feature_types
    };
    let summary = gxf_to_bed_path(input, output, &feature_types)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_annotation_extract(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["annotation", "fasta"],
        &["output", "feature_type", "promoter_length"],
    )?;
    let annotation = request
        .inputs
        .get("annotation")
        .ok_or("annotation.sequence.extract.v1 requires inputs.annotation")?;
    let fasta = request
        .inputs
        .get("fasta")
        .ok_or("annotation.sequence.extract.v1 requires inputs.fasta")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let annotation = resolve_input(base_directory, annotation);
    let fasta = resolve_input(base_directory, fasta);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&annotation, &output)?;
    ensure_distinct_input_output(&fasta, &output)?;
    let options = annotation_extract_options(&request.parameters)?;
    let summary = extract_annotation_sequences_path(annotation, fasta, output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_gene_density(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "annotation",
        &["feature_types", "window_size", "step_size"],
    )?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("genome.gene-density.v1 requires inputs.annotation")?;
    let analysis = gene_density_path(
        resolve_input(base_directory, input),
        gene_density_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_go_annotations(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "annotations",
        &["output", "gene_column", "go_column"],
    )?;
    let input = request
        .inputs
        .get("annotations")
        .ok_or("annotation.go.normalize.v1 requires inputs.annotations")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis =
        normalize_go_annotations_path(input, output, &go_annotation_options(&request.parameters)?)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_eggnog_annotations(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "annotations", &["output"])?;
    let input = request
        .inputs
        .get("annotations")
        .ok_or("annotation.eggnog.normalize.v1 requires inputs.annotations")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis = normalize_eggnog_path(input, output)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_enrichment(
    base_directory: &Path,
    request: JobRequest,
    kind: EnrichmentKind,
) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["genes", "associations"],
        &["min_overlap", "max_terms", "include_genes"],
    )?;
    let genes = request
        .inputs
        .get("genes")
        .ok_or("enrichment requires inputs.genes")?;
    let associations = request
        .inputs
        .get("associations")
        .ok_or("enrichment requires inputs.associations")?;
    let analysis = overrepresentation_path(
        resolve_input(base_directory, genes),
        resolve_input(base_directory, associations),
        kind,
        enrichment_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_gsea(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["ranked", "gene-sets"],
        &[
            "score_exponent",
            "min_set_size",
            "max_set_size",
            "permutations",
            "seed",
        ],
    )?;
    let ranked = resolve_required_v1_input(base_directory, &request, "ranked")?;
    let gene_sets = resolve_required_v1_input(base_directory, &request, "gene-sets")?;
    let result = gsea_preranked_path(ranked, gene_sets, gsea_options(&request.parameters)?)?;
    let mut envelope = AnalysisResult::ok(
        request.job_id,
        request.capability,
        result.clone(),
        ExecutionMode::LocalCpu,
    );
    envelope.warnings = result.warnings;
    Ok(serde_json::to_string(&envelope)?)
}

fn run_annotation_structure_visualization(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "annotation",
        &["output", "feature_id", "seqid", "max_features"],
    )?;
    let input = request
        .inputs
        .get("annotation")
        .ok_or("annotation.structure.visualize.v1 requires inputs.annotation")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis = render_annotation_structure_svg_path(
        input,
        output,
        &annotation_structure_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_synteny_visualization(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "anchors", &["output", "style"])?;
    let input = resolve_required_v1_input(base_directory, &request, "anchors")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = render_synteny_svg_with_options_path(
        input,
        output,
        &synteny_visualization_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_mcscanx(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["gene-positions", "similarity-hits"],
        &["output"],
    )?;
    let genes = request
        .inputs
        .get("gene-positions")
        .ok_or("MCScanX requires inputs.gene-positions")?;
    let hits = request
        .inputs
        .get("similarity-hits")
        .ok_or("MCScanX requires inputs.similarity-hits")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let analysis = run_mcscanx_path(
        resolve_input(base_directory, genes),
        resolve_input(base_directory, hits),
        output,
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_kaks(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "codon-alignment", &["output", "method"])?;
    let input = resolve_required_v1_input(base_directory, &request, "codon-alignment")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let method = optional_parameter_string(&request.parameters, "method")?.unwrap_or("NG");
    let analysis = run_kaks_path(input, output, method)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_dotplot(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["query", "reference"],
        &["output", "width", "height", "kmer"],
    )?;
    let query = resolve_required_v1_input(base_directory, &request, "query")?;
    let reference = resolve_required_v1_input(base_directory, &request, "reference")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let result = render_dotplot_svg_path(
        query,
        reference,
        output,
        &dotplot_options(&request.parameters)?,
    )?;
    let mut analysis = AnalysisResult::ok(
        request.job_id,
        request.capability,
        result.clone(),
        ExecutionMode::LocalCpu,
    );
    analysis.warnings = Vec::new();
    Ok(serde_json::to_string(&analysis)?)
}

fn run_rnafold(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "sequence", &["output", "temperature"])?;
    let input = resolve_required_v1_input(base_directory, &request, "sequence")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let temperature = optional_parameter_f64(&request.parameters, "temperature")?.unwrap_or(37.0);
    let analysis = run_rnafold_path(input, output, temperature)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_metagenomics_classify(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "reads",
        &[
            "output",
            "database",
            "confidence",
            "minimum_hit_groups",
            "threads",
        ],
    )?;
    let input = resolve_required_v1_input(base_directory, &request, "reads")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let options = kraken2_options(&request.parameters)?;
    let result = run_kraken2_path(input, output, &options)?;
    let mut analysis = AnalysisResult::ok(
        request.job_id,
        request.capability,
        result.clone(),
        ExecutionMode::LocalCpu,
    );
    analysis.warnings = result.warnings;
    Ok(serde_json::to_string(&analysis)?)
}

fn kraken2_options(parameters: &serde_json::Value) -> WorkerResult<Kraken2Options> {
    let database = optional_parameter_string(parameters, "database")?
        .ok_or("metagenomics.classify.v1 requires string parameters.database")?;
    let confidence = optional_parameter_f64(parameters, "confidence")?.unwrap_or(0.0);
    let minimum_hit_groups =
        optional_parameter_usize(parameters, "minimum_hit_groups")?.unwrap_or(2);
    let threads = optional_parameter_usize(parameters, "threads")?.unwrap_or(1);
    Ok(Kraken2Options {
        database: PathBuf::from(database),
        confidence,
        minimum_hit_groups,
        threads,
    })
}

fn dotplot_options(parameters: &serde_json::Value) -> WorkerResult<DotplotOptions> {
    let mut options = DotplotOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "width")? {
        options.width = u32::try_from(value).map_err(|_| "width exceeds u32 range")?;
    }
    if let Some(value) = optional_parameter_usize(parameters, "height")? {
        options.height = u32::try_from(value).map_err(|_| "height exceeds u32 range")?;
    }
    if let Some(value) = optional_parameter_usize(parameters, "kmer")? {
        options.kmer_size = value;
    }
    Ok(options)
}

fn synteny_visualization_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SyntenyVisualizationOptions> {
    let style = optional_parameter_string(parameters, "style")?
        .map(SyntenyPlotStyle::parse)
        .transpose()?
        .unwrap_or(SyntenyPlotStyle::Dual);
    Ok(SyntenyVisualizationOptions { style })
}

fn run_enrichment_visualization(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["genes", "associations"],
        &["output", "kind", "style", "min_overlap", "max_terms"],
    )?;
    let genes = request
        .inputs
        .get("genes")
        .ok_or("enrichment.visualize.v1 requires inputs.genes")?;
    let associations = request
        .inputs
        .get("associations")
        .ok_or("enrichment.visualize.v1 requires inputs.associations")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let genes = resolve_input(base_directory, genes);
    let associations = resolve_input(base_directory, associations);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&genes, &output)?;
    ensure_distinct_input_output(&associations, &output)?;
    let analysis = render_enrichment_svg_path(
        genes,
        associations,
        output,
        visualization_enrichment_kind(&request.parameters)?,
        enrichment_options(&request.parameters)?,
        enrichment_visualization_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_matrix_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.matrix.qc.v1 requires inputs.matrix")?;
    let metrics = expression_matrix_qc_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_cohort_table_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "cohort", &[])?;
    let input = resolve_required_v1_input(base_directory, &request, "cohort")?;
    let metrics = cohort_table_qc_path(input)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_single_cell_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "matrix", &[])?;
    let input = resolve_required_v1_input(base_directory, &request, "matrix")?;
    let metrics = expression_matrix_qc_path(input)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_normalize(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["matrix"], &["output", "method", "pseudocount"])?;
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.normalize.v1 requires inputs.matrix")?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary = normalize_expression_matrix_path(
        input,
        output,
        &expression_normalize_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = summary.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_pca(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["matrix"], &["components", "scale_features"])?;
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.pca.v1 requires inputs.matrix")?;
    let analysis = expression_pca_path(
        resolve_input(base_directory, input),
        &expression_pca_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_cluster(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["matrix"],
        &[
            "sample_clusters",
            "feature_clusters",
            "max_iterations",
            "scale_features",
        ],
    )?;
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.cluster.v1 requires inputs.matrix")?;
    let analysis = expression_cluster_path(
        resolve_input(base_directory, input),
        &expression_cluster_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_heatmap(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["matrix"],
        &["top_variable_features", "scale_rows"],
    )?;
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.heatmap.v1 requires inputs.matrix")?;
    let analysis = expression_heatmap_path(
        resolve_input(base_directory, input),
        &expression_heatmap_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_intersect(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let left = request
        .inputs
        .get("left-bed")
        .ok_or("interval.intersect.v1 requires inputs.left-bed")?;
    let right = request
        .inputs
        .get("right-bed")
        .ok_or("interval.intersect.v1 requires inputs.right-bed")?;
    let stats = bed_intersect_path(
        resolve_input(base_directory, left),
        resolve_input(base_directory, right),
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_merge(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_interval_merge_contract(&request)?;
    let input = request
        .inputs
        .get("bed")
        .ok_or("interval.merge.v1 requires inputs.bed")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let stats = bed_merge_path(
        &input,
        &output,
        IntervalMergeOptions {
            max_gap: optional_parameter_u64(&request.parameters, "max_gap")?.unwrap_or(0),
        },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_subtract(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_interval_subtract_contract(&request)?;
    let left = request
        .inputs
        .get("left-bed")
        .ok_or("interval.subtract.v1 requires inputs.left-bed")?;
    let right = request
        .inputs
        .get("right-bed")
        .ok_or("interval.subtract.v1 requires inputs.right-bed")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let left = resolve_input(base_directory, left);
    let right = resolve_input(base_directory, right);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&left, &output)?;
    ensure_distinct_input_output(&right, &output)?;
    let stats = bed_subtract_path(&left, &right, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_closest(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["query-bed", "target-bed"], &["output"])?;
    let query = resolve_required_v1_input(base_directory, &request, "query-bed")?;
    let target = resolve_required_v1_input(base_directory, &request, "target-bed")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&query, &output)?;
    ensure_distinct_input_output(&target, &output)?;
    let summary = bed_closest_path(query, target, output)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = summary.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_stats(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("vcf")
        .ok_or("variant.stats.v1 requires inputs.vcf")?;
    let stats = vcf_stats_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_compare(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["left-vcf", "right-vcf"], &[])?;
    let left = resolve_required_v1_input(base_directory, &request, "left-vcf")?;
    let right = resolve_required_v1_input(base_directory, &request, "right-vcf")?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        compare_vcf_paths(left, right)?,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_medical_variant_cohort(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "vcf", &[])?;
    let input = resolve_required_v1_input(base_directory, &request, "vcf")?;
    let stats = vcf_stats_path(input)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_filter(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "vcf",
        &[
            "output",
            "min_qual",
            "require_pass",
            "contigs",
            "min_info_dp",
        ],
    )?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("vcf")
            .ok_or("variant.filter.v1 requires inputs.vcf")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary = filter_vcf_path(
        &input,
        &output,
        &variant_filter_options(&request.parameters)?,
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_normalize(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["vcf", "reference"], &["output"])?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("vcf")
            .ok_or("variant.normalize.v1 requires inputs.vcf")?,
    );
    let reference = resolve_input(
        base_directory,
        request
            .inputs
            .get("reference")
            .ok_or("variant.normalize.v1 requires inputs.reference")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    ensure_distinct_input_output(&reference, &output)?;
    let summary = normalize_vcf_path(&input, &reference, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_to_table(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "vcf", &["output"])?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("vcf")
            .ok_or("variant.to-table.v1 requires inputs.vcf")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary = vcf_to_table_path(&input, &output)?;
    let warnings = summary.warnings.clone();
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    result.warnings = warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_pdb_summary(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("pdb")
        .ok_or("structure.pdb.summary.v1 requires inputs.pdb")?;
    let summary = pdb_summary_path(
        resolve_input(base_directory, input),
        pdb_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = summary.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_mmcif_summary(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "structure", &[])?;
    let input = request
        .inputs
        .get("structure")
        .ok_or("structure.mmcif.summary.v1 requires inputs.structure")?;
    let summary = mmcif_summary_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = summary.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_structure_sequence(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "structure", &[])?;
    let input = request
        .inputs
        .get("structure")
        .ok_or("structure.sequence.extract.v1 requires inputs.structure")?;
    let analysis = extract_structure_sequences_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_structure_contact_map(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "structure",
        &["cutoff_angstrom", "atom_name", "include_inter_chain"],
    )?;
    let input = request
        .inputs
        .get("structure")
        .ok_or("structure.contact-map.v1 requires inputs.structure")?;
    let analysis = structure_contact_map_path(
        resolve_input(base_directory, input),
        contact_map_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_structure_geometry(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "structure", &["atoms"])?;
    let input = request
        .inputs
        .get("structure")
        .ok_or("structure.geometry.v1 requires inputs.structure")?;
    let analysis = measure_structure_geometry_path(
        resolve_input(base_directory, input),
        &geometry_selectors(&request.parameters)?,
    )?;
    Ok(serde_json::to_string(&AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis,
        ExecutionMode::LocalCpu,
    ))?)
}

fn run_structure_superposition(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["reference", "mobile"], &["atom_name"])?;
    let reference = request
        .inputs
        .get("reference")
        .ok_or("structure.superpose.v1 requires inputs.reference")?;
    let mobile = request
        .inputs
        .get("mobile")
        .ok_or("structure.superpose.v1 requires inputs.mobile")?;
    let analysis = superpose_structures_path(
        resolve_input(base_directory, reference),
        resolve_input(base_directory, mobile),
        superposition_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_set_venn(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "table", &["include_items", "max_intersections"])?;
    let input = request
        .inputs
        .get("table")
        .ok_or("set.venn.v1 requires inputs.table")?;
    let result = venn_analysis_path(
        resolve_input(base_directory, input),
        set_analysis_options(&request.parameters)?,
    )?;
    Ok(serde_json::to_string(&AnalysisResult::ok(
        request.job_id,
        request.capability,
        result,
        ExecutionMode::LocalCpu,
    ))?)
}

fn run_set_upset(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "table", &["include_items", "max_intersections"])?;
    let input = request
        .inputs
        .get("table")
        .ok_or("set.upset.v1 requires inputs.table")?;
    let result = upset_analysis_path(
        resolve_input(base_directory, input),
        set_analysis_options(&request.parameters)?,
    )?;
    Ok(serde_json::to_string(&AnalysisResult::ok(
        request.job_id,
        request.capability,
        result,
        ExecutionMode::LocalCpu,
    ))?)
}

fn run_blast_parse(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "blast", &[])?;
    let input = request
        .inputs
        .get("blast")
        .ok_or("similarity.blast.parse.v1 requires inputs.blast")?;
    let analysis = parse_blast_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_local_blast(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["query", "reference"],
        &[
            "output",
            "program",
            "threads",
            "evalue",
            "max_target_sequences",
            "outfmt",
        ],
    )?;
    let query = resolve_required_v1_input(base_directory, &request, "query")?;
    let reference = resolve_required_v1_input(base_directory, &request, "reference")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&query, &output)?;
    ensure_distinct_input_output(&reference, &output)?;
    let analysis = run_blast_fasta_path(
        query,
        reference,
        output,
        blast_program(&request.parameters)?,
        &similarity_search_options(&request.parameters)?,
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_local_diamond(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["query", "reference"],
        &[
            "output",
            "mode",
            "threads",
            "evalue",
            "max_target_sequences",
            "outfmt",
        ],
    )?;
    let query = resolve_required_v1_input(base_directory, &request, "query")?;
    let reference = resolve_required_v1_input(base_directory, &request, "reference")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&query, &output)?;
    ensure_distinct_input_output(&reference, &output)?;
    let analysis = run_diamond_fasta_path(
        query,
        reference,
        output,
        diamond_mode(&request.parameters)?,
        &similarity_search_options(&request.parameters)?,
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_local_hmmer(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["profile", "sequences"],
        &["output", "mode", "threads", "evalue"],
    )?;
    let profile = resolve_required_v1_input(base_directory, &request, "profile")?;
    let sequences = resolve_required_v1_input(base_directory, &request, "sequences")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&profile, &output)?;
    ensure_distinct_input_output(&sequences, &output)?;
    let analysis = run_hmmer_path(
        profile,
        sequences,
        output,
        hmmer_mode(&request.parameters)?,
        &hmmer_options(&request.parameters)?,
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_muscle_alignment(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "fasta", &["output", "mode", "threads"])?;
    let input = resolve_required_v1_input(base_directory, &request, "fasta")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_muscle_path(input, output, &muscle_options(&request.parameters)?)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_trimal_alignment(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "alignment", &["output", "mode"])?;
    let input = resolve_required_v1_input(base_directory, &request, "alignment")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let mode = parse_trimal_mode(
        optional_parameter_string(&request.parameters, "mode")?.unwrap_or("automated1"),
    )?;
    let analysis = run_trimal_path(input, output, mode)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_iqtree_inference(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "alignment",
        &["output", "threads", "model", "seed"],
    )?;
    let input = resolve_required_v1_input(base_directory, &request, "alignment")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_iqtree_path(input, output, &iqtree_options(&request.parameters)?)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_meme_discovery(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fasta",
        &[
            "output",
            "threads",
            "alphabet",
            "distribution",
            "motif_count",
            "minimum_width",
            "maximum_width",
        ],
    )?;
    let input = resolve_required_v1_input(base_directory, &request, "fasta")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_meme_path(input, output, &meme_options(&request.parameters)?)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_mast(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["motif", "sequences"],
        &["output", "evalue", "threads", "hit_list"],
    )?;
    let motif = request
        .inputs
        .get("motif")
        .ok_or("motif.mast.v1 requires inputs.motif")?;
    let sequences = request
        .inputs
        .get("sequences")
        .ok_or("motif.mast.v1 requires inputs.sequences")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let motif = resolve_input(base_directory, motif);
    let sequences = resolve_input(base_directory, sequences);
    ensure_distinct_input_output(&motif, &output)?;
    ensure_distinct_input_output(&sequences, &output)?;
    let evalue = optional_parameter_f64(&request.parameters, "evalue")?.unwrap_or(1e-5);
    let analysis = run_mast_path(
        motif,
        sequences,
        output,
        &MastOptions {
            threads: optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1),
            evalue,
            hit_list: request
                .parameters
                .get("hit_list")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            add_self_compat: request
                .parameters
                .get("add_self_compat")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        },
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_wgcna(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "expression",
        &[
            "output",
            "threads",
            "min_expression",
            "min_samples",
            "min_module_size",
            "merge_cut_height",
            "network_type",
            "power",
            "log_transform",
        ],
    )?;
    let input = resolve_required_v1_input(base_directory, &request, "expression")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_wgcna_path(
        input,
        output,
        &WgcnaOptions {
            threads: optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1),
            min_expression: optional_parameter_f64(&request.parameters, "min_expression")?
                .unwrap_or(1.0),
            min_samples: optional_parameter_usize(&request.parameters, "min_samples")?.unwrap_or(3),
            min_module_size: optional_parameter_usize(&request.parameters, "min_module_size")?
                .unwrap_or(30),
            merge_cut_height: optional_parameter_f64(&request.parameters, "merge_cut_height")?
                .unwrap_or(0.25),
            network_type: optional_parameter_string(&request.parameters, "network_type")?
                .unwrap_or("signed")
                .to_owned(),
            power: optional_parameter_usize(&request.parameters, "power")?.unwrap_or(0),
            log_transform: request
                .parameters
                .get("log_transform")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        },
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_dssp_secondary_structure(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "structure", &["output"])?;
    let input = resolve_required_v1_input(base_directory, &request, "structure")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_dssp_path(input, output)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_bam_cram_report(
    base_directory: &Path,
    request: JobRequest,
    mode: &str,
) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "alignment", &["output"])?;
    let input = resolve_required_v1_input(base_directory, &request, "alignment")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let analysis = run_samtools_report_path(input, None, output, mode)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_bam_to_bigwig(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "alignment", &["output", "threads"])?;
    let input = resolve_required_v1_input(base_directory, &request, "alignment")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let threads = optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1);
    let analysis = run_bam_to_bigwig_path(input, output, threads)?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_short_read_alignment(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(&request, &["reference", "reads"], &["output", "threads"])?;
    let reference = request
        .inputs
        .get("reference")
        .ok_or("alignment.short-read.v1 requires inputs.reference")?;
    let reads = request
        .inputs
        .get("reads")
        .ok_or("alignment.short-read.v1 requires inputs.reads")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let reference = resolve_input(base_directory, reference);
    let reads = resolve_input(base_directory, reads);
    ensure_distinct_input_output(&reference, &output)?;
    ensure_distinct_input_output(&reads, &output)?;
    let analysis = run_short_read_alignment_path(
        reference,
        reads,
        output,
        &ShortReadAlignmentOptions {
            threads: optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1),
        },
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_long_read_alignment(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["reference", "reads"],
        &["output", "threads", "preset", "secondary"],
    )?;
    let reference = request
        .inputs
        .get("reference")
        .ok_or("alignment.long-read.v1 requires inputs.reference")?;
    let reads = request
        .inputs
        .get("reads")
        .ok_or("alignment.long-read.v1 requires inputs.reads")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let reference = resolve_input(base_directory, reference);
    let reads = resolve_input(base_directory, reads);
    ensure_distinct_input_output(&reference, &output)?;
    ensure_distinct_input_output(&reads, &output)?;
    let preset = match request.parameters.get("preset") {
        Some(value) => parse_minimap2_preset(
            value
                .as_str()
                .ok_or("alignment.long-read.v1 preset must be a string")?,
        )?,
        None => Minimap2Preset::MapOnt,
    };
    let analysis = run_minimap2_long_read_path(
        reference,
        reads,
        output,
        &Minimap2LongReadOptions {
            preset,
            threads: optional_parameter_usize(&request.parameters, "threads")?.unwrap_or(1),
            secondary: request
                .parameters
                .get("secondary")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            max_secondary: optional_parameter_usize(&request.parameters, "max_secondary")?
                .unwrap_or(0),
        },
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn run_variant_annotate(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["vcf"],
        &["output", "database", "upstream_downstream", "no_stats"],
    )?;
    let vcf = request
        .inputs
        .get("vcf")
        .ok_or("variant.annotate.v1 requires inputs.vcf")?;
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    let vcf = resolve_input(base_directory, vcf);
    ensure_distinct_input_output(&vcf, &output)?;
    let database = match request.parameters.get("database") {
        Some(value) => value
            .as_str()
            .ok_or("variant.annotate.v1 database must be a string")?
            .to_owned(),
        None => "GRCh38.99".to_owned(),
    };
    let analysis = run_snpeff_path(
        vcf,
        output,
        &SnpEffOptions {
            database,
            upstream_downstream: optional_parameter_usize(
                &request.parameters,
                "upstream_downstream",
            )?,
            no_stats: request
                .parameters
                .get("no_stats")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            no_log: request
                .parameters
                .get("no_log")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        },
    )?;
    serialize_v1_native_tool_result(request, analysis)
}

fn resolve_required_v1_input(
    base_directory: &Path,
    request: &JobRequest,
    role: &str,
) -> WorkerResult<PathBuf> {
    request
        .inputs
        .get(role)
        .map(|path| resolve_input(base_directory, path))
        .ok_or_else(|| format!("{} requires inputs.{role}", request.capability).into())
}

fn serialize_v1_native_tool_result(
    request: JobRequest,
    analysis: linxira_bio_core::native_tools::NativeToolResult,
) -> WorkerResult<String> {
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_reciprocal_best_hits(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["forward", "reverse"],
        &["max_evalue", "min_identity_percent"],
    )?;
    let forward = request
        .inputs
        .get("forward")
        .ok_or("similarity.reciprocal.v1 requires inputs.forward")?;
    let reverse = request
        .inputs
        .get("reverse")
        .ok_or("similarity.reciprocal.v1 requires inputs.reverse")?;
    let analysis = reciprocal_best_hits_path(
        resolve_input(base_directory, forward),
        resolve_input(base_directory, reverse),
        reciprocal_best_hit_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_protein_domains(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "domains", &[])?;
    let input = request
        .inputs
        .get("domains")
        .ok_or("protein.domain.parse.v1 requires inputs.domains")?;
    let analysis = parse_protein_domains_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_protein_domain_visualization(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "domains",
        &["output", "sequence_id", "max_sequences", "max_domains"],
    )?;
    let input = request
        .inputs
        .get("domains")
        .ok_or("protein.domain.visualize.v1 requires inputs.domains")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis = render_domain_architecture_svg_path(
        input,
        output,
        &domain_architecture_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_phylogeny_tree(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "tree", &["output", "reroot_label", "label_map"])?;
    let input = request
        .inputs
        .get("tree")
        .ok_or("phylogeny.tree.transform.v1 requires inputs.tree")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis =
        transform_newick_path(input, output, tree_transform_options(&request.parameters)?)?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_phylogeny_tree_visualize(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "tree",
        &[
            "output",
            "width",
            "height",
            "font_size",
            "show_branch_lengths",
        ],
    )?;
    let input = request
        .inputs
        .get("tree")
        .ok_or("phylogeny.tree.visualize.v1 requires inputs.tree")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let options = tree_visualization_options(&request.parameters)?;
    let analysis = render_tree_svg_path(input, output, &options)
        .map_err(|error| -> WorkerError { error.to_string().into() })?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_phylogeny_distance(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "alignment", &["output", "model"])?;
    let input = request
        .inputs
        .get("alignment")
        .ok_or("phylogeny.distance.v1 requires inputs.alignment")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let analysis = distance_matrix_path(
        input,
        output,
        &distance_matrix_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        analysis.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = analysis.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn distance_matrix_options(parameters: &serde_json::Value) -> WorkerResult<DistanceMatrixOptions> {
    let model = parameters
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("p-distance")
        .to_owned();
    Ok(DistanceMatrixOptions { model })
}

fn run_protein_properties(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(&request, "fasta", &[])?;
    let input = request
        .inputs
        .get("fasta")
        .ok_or("protein.properties.v1 requires inputs.fasta")?;
    let properties = protein_properties_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        properties.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = properties.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn pdb_options(parameters: &serde_json::Value) -> WorkerResult<PdbSummaryOptions> {
    let interpret_b_factors_as_plddt = match parameters.get("interpret_b_factors_as_plddt") {
        Some(value) => value
            .as_bool()
            .ok_or("interpret_b_factors_as_plddt must be a boolean")?,
        None => false,
    };
    Ok(PdbSummaryOptions {
        interpret_b_factors_as_plddt,
    })
}

fn contact_map_options(parameters: &serde_json::Value) -> WorkerResult<ContactMapOptions> {
    let mut options = ContactMapOptions::default();
    if let Some(value) = optional_parameter_f64(parameters, "cutoff_angstrom")? {
        options.cutoff_angstrom = value;
    }
    if let Some(value) = optional_parameter_string(parameters, "atom_name")? {
        options.atom_name = value.to_owned();
    }
    if let Some(value) = optional_parameter_bool(parameters, "include_inter_chain")? {
        options.include_inter_chain = value;
    }
    Ok(options)
}

fn geometry_selectors(parameters: &serde_json::Value) -> WorkerResult<Vec<AtomSelector>> {
    let values = parameters
        .get("atoms")
        .and_then(serde_json::Value::as_array)
        .ok_or("atoms must be an array of two, three, or four selectors")?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let selector = value
                .as_str()
                .ok_or_else(|| format!("atoms[{index}] must be a string"))?;
            parse_atom_selector(selector).map_err(Into::into)
        })
        .collect()
}

fn superposition_options(parameters: &serde_json::Value) -> WorkerResult<SuperpositionOptions> {
    Ok(SuperpositionOptions {
        atom_name: optional_parameter_string(parameters, "atom_name")?
            .unwrap_or("CA")
            .to_owned(),
    })
}

fn set_analysis_options(parameters: &serde_json::Value) -> WorkerResult<SetAnalysisOptions> {
    let mut options = SetAnalysisOptions::default();
    if let Some(include_items) = optional_parameter_bool(parameters, "include_items")? {
        options.include_items = include_items;
    }
    if let Some(max_intersections) = optional_parameter_usize(parameters, "max_intersections")? {
        options.max_intersections = max_intersections;
    }
    Ok(options)
}

fn gene_density_options(parameters: &serde_json::Value) -> WorkerResult<GeneDensityOptions> {
    let mut options = GeneDensityOptions::default();
    let feature_types = optional_string_array_parameter(parameters, "feature_types")?;
    if !feature_types.is_empty() {
        options.feature_types = feature_types;
    }
    if let Some(window_size) = optional_parameter_u64(parameters, "window_size")? {
        if window_size == 0 {
            return Err("window_size must be positive".into());
        }
        options.window_size = window_size;
    }
    if let Some(step_size) = optional_parameter_u64(parameters, "step_size")? {
        if step_size == 0 {
            return Err("step_size must be positive".into());
        }
        options.step_size = step_size;
    }
    Ok(options)
}

fn go_annotation_options(parameters: &serde_json::Value) -> WorkerResult<GoAnnotationOptions> {
    Ok(GoAnnotationOptions {
        gene_column: optional_parameter_string(parameters, "gene_column")?.map(str::to_owned),
        go_column: optional_parameter_string(parameters, "go_column")?.map(str::to_owned),
    })
}

fn enrichment_options(parameters: &serde_json::Value) -> WorkerResult<EnrichmentOptions> {
    let mut options = EnrichmentOptions::default();
    if let Some(value) = optional_parameter_u64(parameters, "min_overlap")? {
        if value == 0 {
            return Err("min_overlap must be positive".into());
        }
        options.min_overlap = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_terms")? {
        if value == 0 {
            return Err("max_terms must be positive".into());
        }
        options.max_terms = value;
    }
    if let Some(value) = optional_parameter_bool(parameters, "include_genes")? {
        options.include_genes = value;
    }
    Ok(options)
}

fn gsea_options(parameters: &serde_json::Value) -> WorkerResult<GseaOptions> {
    let mut options = GseaOptions::default();
    if let Some(value) = optional_parameter_f64(parameters, "score_exponent")? {
        options.score_exponent = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "min_set_size")? {
        options.min_set_size = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_set_size")? {
        options.max_set_size = value;
    }
    if let Some(value) = optional_parameter_u64(parameters, "permutations")? {
        options.permutation_count = value
            .try_into()
            .map_err(|_| "permutations must fit in a 32-bit integer")?;
    }
    if let Some(value) = optional_parameter_u64(parameters, "seed")? {
        options.seed = value;
    }
    Ok(options)
}

fn annotation_structure_options(
    parameters: &serde_json::Value,
) -> WorkerResult<AnnotationStructureOptions> {
    let feature_id = optional_parameter_string(parameters, "feature_id")?.map(str::to_owned);
    let seqid = optional_parameter_string(parameters, "seqid")?.map(str::to_owned);
    if feature_id.is_some() && seqid.is_some() {
        return Err("feature_id and seqid are mutually exclusive".into());
    }
    let mut options = AnnotationStructureOptions {
        feature_id,
        seqid,
        ..Default::default()
    };
    if let Some(value) = optional_parameter_usize(parameters, "max_features")? {
        options.max_features = value;
    }
    Ok(options)
}

fn domain_architecture_options(
    parameters: &serde_json::Value,
) -> WorkerResult<DomainArchitectureOptions> {
    let mut options = DomainArchitectureOptions {
        sequence_id: optional_parameter_string(parameters, "sequence_id")?.map(str::to_owned),
        ..Default::default()
    };
    if let Some(value) = optional_parameter_usize(parameters, "max_sequences")? {
        options.max_sequences = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_domains")? {
        options.max_domains = value;
    }
    Ok(options)
}

fn visualization_enrichment_kind(parameters: &serde_json::Value) -> WorkerResult<EnrichmentKind> {
    match optional_parameter_string(parameters, "kind")? {
        Some("custom") => Ok(EnrichmentKind::Custom),
        Some("go") => Ok(EnrichmentKind::Go),
        Some("kegg") => Ok(EnrichmentKind::Kegg),
        Some(value) => Err(format!("kind must be custom, go, or kegg, got {value:?}").into()),
        None => Err("enrichment visualization requires string parameter kind".into()),
    }
}

fn enrichment_visualization_options(
    parameters: &serde_json::Value,
) -> WorkerResult<EnrichmentVisualizationOptions> {
    let style = match optional_parameter_string(parameters, "style")? {
        Some("bar") | None => EnrichmentPlotStyle::Bar,
        Some("dot") => EnrichmentPlotStyle::Dot,
        Some("network") => EnrichmentPlotStyle::Network,
        Some(value) => {
            return Err(format!("style must be bar, dot, or network, got {value:?}").into());
        }
    };
    Ok(EnrichmentVisualizationOptions {
        style,
        max_terms: optional_parameter_usize(parameters, "max_terms")?.unwrap_or(30),
    })
}

fn enrichment_kind(capability: &str) -> WorkerResult<EnrichmentKind> {
    match capability {
        "enrichment.overrepresentation.v1" => Ok(EnrichmentKind::Custom),
        "enrichment.go.v1" => Ok(EnrichmentKind::Go),
        "enrichment.kegg.v1" => Ok(EnrichmentKind::Kegg),
        _ => Err(format!("unsupported enrichment capability: {capability}").into()),
    }
}

fn reciprocal_best_hit_options(
    parameters: &serde_json::Value,
) -> WorkerResult<ReciprocalBestHitOptions> {
    let max_evalue = optional_parameter_f64(parameters, "max_evalue")?;
    if max_evalue.is_some_and(|value| value < 0.0) {
        return Err("max_evalue must be non-negative".into());
    }
    Ok(ReciprocalBestHitOptions {
        max_evalue,
        min_identity_percent: optional_parameter_percentage(parameters, "min_identity_percent")?,
    })
}

fn similarity_search_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SimilaritySearchOptions> {
    let mut options = SimilaritySearchOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "threads")? {
        options.threads = value;
    }
    if let Some(value) = optional_parameter_f64(parameters, "evalue")? {
        options.evalue = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_target_sequences")? {
        options.max_target_sequences = value;
    }
    if let Some(value) = optional_parameter_u8(parameters, "outfmt")? {
        options.outfmt = value;
    }
    Ok(options)
}

fn blast_program(
    parameters: &serde_json::Value,
) -> WorkerResult<linxira_bio_core::native_tools::BlastProgram> {
    Ok(parse_blast_program(
        optional_parameter_string(parameters, "program")?.unwrap_or("blastn"),
    )?)
}

fn diamond_mode(
    parameters: &serde_json::Value,
) -> WorkerResult<linxira_bio_core::native_tools::DiamondMode> {
    Ok(parse_diamond_mode(
        optional_parameter_string(parameters, "mode")?.unwrap_or("blastp"),
    )?)
}

fn hmmer_mode(
    parameters: &serde_json::Value,
) -> WorkerResult<linxira_bio_core::native_tools::HmmerMode> {
    Ok(parse_hmmer_mode(
        optional_parameter_string(parameters, "mode")?.unwrap_or("hmmsearch"),
    )?)
}

fn hmmer_options(parameters: &serde_json::Value) -> WorkerResult<HmmerOptions> {
    let mut options = HmmerOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "threads")? {
        options.threads = value;
    }
    if let Some(value) = optional_parameter_f64(parameters, "evalue")? {
        options.evalue = value;
    }
    Ok(options)
}

fn muscle_options(parameters: &serde_json::Value) -> WorkerResult<MuscleOptions> {
    let mut options = MuscleOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "threads")? {
        options.threads = value;
    }
    if let Some(value) = optional_parameter_string(parameters, "mode")? {
        options.mode = parse_muscle_mode(value)?;
    }
    Ok(options)
}

fn iqtree_options(parameters: &serde_json::Value) -> WorkerResult<IqtreeOptions> {
    let mut options = IqtreeOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "threads")? {
        options.threads = value;
    }
    if let Some(value) = optional_parameter_string(parameters, "model")? {
        options.model = value.to_owned();
    }
    if let Some(value) = optional_parameter_u64(parameters, "seed")? {
        options.seed = value;
    }
    Ok(options)
}

fn meme_options(parameters: &serde_json::Value) -> WorkerResult<MemeOptions> {
    let mut options = MemeOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "threads")? {
        options.threads = value;
    }
    if let Some(value) = optional_parameter_string(parameters, "alphabet")? {
        options.alphabet = parse_meme_alphabet(value)?;
    }
    if let Some(value) = optional_parameter_string(parameters, "distribution")? {
        options.distribution = value.to_owned();
    }
    if let Some(value) = optional_parameter_usize(parameters, "motif_count")? {
        options.motif_count = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "minimum_width")? {
        options.minimum_width = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "maximum_width")? {
        options.maximum_width = value;
    }
    Ok(options)
}

fn tree_transform_options(parameters: &serde_json::Value) -> WorkerResult<TreeTransformOptions> {
    let reroot_label = optional_parameter_string(parameters, "reroot_label")?
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut label_map = BTreeMap::new();
    if let Some(value) = parameters.get("label_map") {
        let mapping = value.as_object().ok_or("label_map must be an object")?;
        for (source, target) in mapping {
            let source = source.trim();
            let target = target
                .as_str()
                .ok_or("label_map values must be strings")?
                .trim();
            if source.is_empty() || target.is_empty() {
                return Err("label_map keys and values must be non-empty".into());
            }
            label_map.insert(source.to_owned(), target.to_owned());
        }
    }
    Ok(TreeTransformOptions {
        reroot_label,
        label_map,
    })
}

fn tree_visualization_options(
    parameters: &serde_json::Value,
) -> WorkerResult<TreeVisualizationOptions> {
    let mut options = TreeVisualizationOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "width")? {
        options.width = u32::try_from(value).map_err(|_| "width exceeds u32 range")?;
    }
    if let Some(value) = optional_parameter_usize(parameters, "height")? {
        options.height = u32::try_from(value).map_err(|_| "height exceeds u32 range")?;
    }
    if let Some(value) = optional_parameter_usize(parameters, "font_size")? {
        options.font_size = u32::try_from(value).map_err(|_| "font_size exceeds u32 range")?;
    }
    if let Some(value) = parameters
        .get("show_branch_lengths")
        .and_then(|v| v.as_bool())
    {
        options.show_branch_lengths = value;
    }
    Ok(options)
}

fn expression_normalize_options(
    parameters: &serde_json::Value,
) -> WorkerResult<ExpressionNormalizeOptions> {
    let mut options = ExpressionNormalizeOptions::default();
    if let Some(method) = optional_parameter_string(parameters, "method")? {
        options.method = parse_expression_normalization_method(method)?;
    }
    if let Some(pseudocount) = optional_parameter_f64(parameters, "pseudocount")? {
        if pseudocount < 0.0 {
            return Err("pseudocount must be non-negative".into());
        }
        options.pseudocount = pseudocount;
    }
    Ok(options)
}

fn expression_pca_options(parameters: &serde_json::Value) -> WorkerResult<ExpressionPcaOptions> {
    let mut options = ExpressionPcaOptions::default();
    if let Some(components) = optional_parameter_usize(parameters, "components")? {
        if components == 0 {
            return Err("components must be at least 1".into());
        }
        options.components = components;
    }
    if let Some(scale) = optional_parameter_bool(parameters, "scale_features")? {
        options.scale_features = scale;
    }
    Ok(options)
}

fn expression_cluster_options(
    parameters: &serde_json::Value,
) -> WorkerResult<ExpressionClusterOptions> {
    let mut options = ExpressionClusterOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "sample_clusters")? {
        if value == 0 {
            return Err("sample_clusters must be at least 1".into());
        }
        options.sample_clusters = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "feature_clusters")? {
        if value == 0 {
            return Err("feature_clusters must be at least 1".into());
        }
        options.feature_clusters = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_iterations")? {
        if value == 0 || value > 10_000 {
            return Err("max_iterations must be between 1 and 10000".into());
        }
        options.max_iterations = value;
    }
    if let Some(scale) = optional_parameter_bool(parameters, "scale_features")? {
        options.scale_features = scale;
    }
    Ok(options)
}

fn expression_heatmap_options(
    parameters: &serde_json::Value,
) -> WorkerResult<ExpressionHeatmapOptions> {
    let mut options = ExpressionHeatmapOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "top_variable_features")? {
        if value == 0 || value > 200 {
            return Err("top_variable_features must be between 1 and 200".into());
        }
        options.top_variable_features = value;
    }
    if let Some(scale) = optional_parameter_bool(parameters, "scale_rows")? {
        options.scale_rows = scale;
    }
    Ok(options)
}

fn volcano_plot_options(parameters: &serde_json::Value) -> WorkerResult<VolcanoPlotOptions> {
    let mut options = VolcanoPlotOptions::default();
    if let Some(value) = optional_parameter_f64(parameters, "padj")? {
        if !(0.0..=1.0).contains(&value) {
            return Err("padj must be between 0 and 1".into());
        }
        options.adjusted_pvalue_threshold = value;
    }
    if let Some(value) = optional_parameter_f64(parameters, "log2_fold_change")? {
        if value < 0.0 {
            return Err("log2_fold_change must be non-negative".into());
        }
        options.absolute_log2_fold_change_threshold = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_points")? {
        options.max_points = value;
    }
    Ok(options)
}

fn fastq_options_v1(request: &JobRequest) -> WorkerResult<FastqQcOptions> {
    Ok(FastqQcOptions {
        max_cycles: optional_usize_parameter(request, "max_cycles")?.unwrap_or(DEFAULT_MAX_CYCLES),
        quality_encoding: parse_quality_encoding(request.parameters.get("quality_encoding"))?,
    })
}

fn fastq_options_v2(request: &JobRequestV2) -> WorkerResult<FastqQcOptions> {
    Ok(FastqQcOptions {
        max_cycles: optional_v2_usize_parameter(request, "max_cycles")?
            .unwrap_or(DEFAULT_MAX_CYCLES),
        quality_encoding: parse_quality_encoding(request.parameters.get("quality_encoding"))?,
    })
}

fn parse_quality_encoding(value: Option<&serde_json::Value>) -> WorkerResult<QualityEncodingMode> {
    match value.and_then(serde_json::Value::as_str).unwrap_or("auto") {
        "auto" => Ok(QualityEncodingMode::Auto),
        "phred+33" => Ok(QualityEncodingMode::Phred33),
        "phred+64" => Ok(QualityEncodingMode::Phred64),
        value => Err(format!(
            "unsupported quality_encoding {value:?}; expected auto, phred+33, or phred+64"
        )
        .into()),
    }
}

fn fastq_trim_options(parameters: &serde_json::Value) -> WorkerResult<FastqTrimOptions> {
    Ok(FastqTrimOptions {
        min_quality: optional_parameter_u8(parameters, "min_quality")?
            .unwrap_or(DEFAULT_TRIM_QUALITY),
        min_length: optional_parameter_usize(parameters, "min_length")?
            .unwrap_or(DEFAULT_MIN_LENGTH),
        quality_encoding: parse_fastq_transform_quality_encoding(
            parameters.get("quality_encoding"),
        )?,
    })
}

fn fastq_adapter_options(parameters: &serde_json::Value) -> WorkerResult<FastqAdapterOptions> {
    let adapters = match (
        optional_parameter_string(parameters, "adapter")?,
        parameters.get("adapters"),
    ) {
        (Some(adapter), None) => vec![adapter.to_owned()],
        (None, Some(value)) => value
            .as_array()
            .ok_or("adapters must be an array of strings")?
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("adapters[{index}] must be a string").into())
            })
            .collect::<WorkerResult<Vec<_>>>()?,
        (Some(_), Some(_)) => {
            return Err("use either adapter or adapters, not both".into());
        }
        (None, None) => FastqAdapterOptions::default().adapters,
    };
    Ok(FastqAdapterOptions {
        adapters,
        min_overlap: optional_parameter_usize(parameters, "min_overlap")?
            .unwrap_or(DEFAULT_ADAPTER_MIN_OVERLAP),
        min_length: optional_parameter_usize(parameters, "min_length")?
            .unwrap_or(DEFAULT_MIN_LENGTH),
    })
}

fn fastq_deduplicate_options(
    parameters: &serde_json::Value,
) -> WorkerResult<FastqDeduplicateOptions> {
    let header = optional_parameter_string(parameters, "header_umi_delimiter")?;
    let sequence_prefix = optional_parameter_usize(parameters, "sequence_prefix_umi")?;
    let key = match (header, sequence_prefix) {
        (Some(delimiter), None) => FastqDeduplicateKey::HeaderUmi {
            delimiter: delimiter.to_owned(),
        },
        (None, Some(length)) => FastqDeduplicateKey::SequencePrefixUmi { length },
        (Some(_), Some(_)) => return Err("choose only one UMI source".into()),
        (None, None) => FastqDeduplicateKey::Sequence,
    };
    Ok(FastqDeduplicateOptions { key })
}

fn parse_fastq_transform_quality_encoding(
    value: Option<&serde_json::Value>,
) -> WorkerResult<FastqTransformQualityEncoding> {
    match value
        .and_then(serde_json::Value::as_str)
        .unwrap_or("phred+33")
    {
        "phred+33" => Ok(FastqTransformQualityEncoding::Phred33),
        "phred+64" => Ok(FastqTransformQualityEncoding::Phred64),
        value => Err(format!(
            "unsupported quality_encoding {value:?}; expected phred+33 or phred+64"
        )
        .into()),
    }
}

fn table_manipulate_options(
    parameters: &serde_json::Value,
) -> WorkerResult<TableManipulateOptions> {
    Ok(TableManipulateOptions {
        input_delimiter: table_delimiter_parameter(parameters, "delimiter")?,
        output_delimiter: table_delimiter_parameter(parameters, "output_delimiter")?,
        select_columns: optional_string_array_parameter(parameters, "select_columns")?,
        drop_columns: optional_string_array_parameter(parameters, "drop_columns")?,
        filter: table_filter_parameter(parameters)?,
        skip_rows: optional_parameter_usize(parameters, "skip_rows")?.unwrap_or(0),
        limit: optional_parameter_usize(parameters, "limit")?,
    })
}

fn annotation_extract_options(
    parameters: &serde_json::Value,
) -> WorkerResult<AnnotationExtractOptions> {
    Ok(AnnotationExtractOptions {
        feature_type: optional_parameter_string(parameters, "feature_type")?
            .unwrap_or("gene")
            .to_owned(),
        promoter_length: optional_parameter_u64(parameters, "promoter_length")?
            .unwrap_or(linxira_bio_core::annotation::DEFAULT_PROMOTER_LENGTH),
    })
}

fn table_delimiter_parameter(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<TableDelimiter>> {
    match optional_parameter_string(parameters, key)? {
        Some("csv") => Ok(Some(TableDelimiter::Csv)),
        Some("tsv" | "tab") => Ok(Some(TableDelimiter::Tsv)),
        Some(value) => Err(format!("{key} must be csv or tsv, got {value:?}").into()),
        None => Ok(None),
    }
}

fn optional_string_array_parameter(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Vec<String>> {
    optional_parameter_array(parameters, key)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{key}[{index}] must be a string").into())
        })
        .collect()
}

fn table_filter_parameter(parameters: &serde_json::Value) -> WorkerResult<Option<TableFilter>> {
    let column = optional_parameter_string(parameters, "filter_column")?;
    let op = optional_parameter_string(parameters, "filter_op")?;
    let value = optional_parameter_string(parameters, "filter_value")?;
    match (column, op, value) {
        (None, None, None) => Ok(None),
        (Some(column), Some("equals" | "eq"), Some(value)) => Ok(Some(TableFilter::Equals {
            column: column.to_owned(),
            value: value.to_owned(),
        })),
        (Some(column), Some("contains"), Some(value)) => Ok(Some(TableFilter::Contains {
            column: column.to_owned(),
            value: value.to_owned(),
        })),
        (Some(column), Some("non-empty" | "nonempty"), None) => Ok(Some(TableFilter::NonEmpty {
            column: column.to_owned(),
        })),
        (Some(_), Some("equals" | "eq" | "contains"), None) => {
            Err("filter_value is required for equals and contains filters".into())
        }
        (Some(_), Some("non-empty" | "nonempty"), Some(_)) => {
            Err("filter_value is not used with non-empty filters".into())
        }
        (Some(_), Some(op), _) => Err(format!("unsupported filter_op: {op}").into()),
        _ => Err("filter_column and filter_op must be provided together".into()),
    }
}

fn required_v2_string_parameter<'a>(request: &'a JobRequestV2, key: &str) -> WorkerResult<&'a str> {
    request
        .parameters
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{} requires string parameters.{key}", request.capability).into())
}

fn export_bio_format(format: ExportFormat) -> BioDataFormat {
    match format {
        ExportFormat::Csv => BioDataFormat::Csv,
        ExportFormat::Tsv => BioDataFormat::Tsv,
        ExportFormat::Json => BioDataFormat::Json,
        ExportFormat::Jsonl => BioDataFormat::Jsonl,
        ExportFormat::Xlsx => BioDataFormat::Xlsx,
    }
}

fn export_media_type(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => "text/csv",
        ExportFormat::Tsv => "text/tab-separated-values",
        ExportFormat::Json => "application/json",
        ExportFormat::Jsonl => "application/x-ndjson",
        ExportFormat::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    }
}

fn table_bio_format(delimiter: TableDelimiter) -> BioDataFormat {
    match delimiter {
        TableDelimiter::Csv => BioDataFormat::Csv,
        TableDelimiter::Tsv => BioDataFormat::Tsv,
    }
}

fn run_environment_audit(request: JobRequest) -> WorkerResult<String> {
    let audit = audit_environment()?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        audit,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_environment_plan(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let profile = match request.parameters.get("profile") {
        Some(value) => value
            .as_str()
            .ok_or("environment plan profile must be a string")?,
        None => "full-local",
    };
    let mode = match request.parameters.get("mode") {
        Some(value) => parse_environment_mode(
            value
                .as_str()
                .ok_or("environment plan mode must be a string")?,
        )?,
        None => EnvironmentMode::ManagedUser,
    };
    let project_root = match request.parameters.get("project_root") {
        Some(value) => Some(resolve_input(
            base_directory,
            value
                .as_str()
                .ok_or("environment plan project_root must be a string")?,
        )),
        None => None,
    };
    if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
        return Err("project_root is only valid in project-isolated mode".into());
    }
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(
        profile,
        &audit,
        &EnvironmentPlanOptions { mode, project_root },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        plan,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_environment_apply(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let profile = match request.parameters.get("profile") {
        Some(value) => value
            .as_str()
            .ok_or("environment apply profile must be a string")?,
        None => "full-local",
    };
    let mode = match request.parameters.get("mode") {
        Some(value) => parse_environment_mode(
            value
                .as_str()
                .ok_or("environment apply mode must be a string")?,
        )?,
        None => EnvironmentMode::ManagedUser,
    };
    let project_root = match request.parameters.get("project_root") {
        Some(value) => Some(resolve_input(
            base_directory,
            value
                .as_str()
                .ok_or("environment apply project_root must be a string")?,
        )),
        None => None,
    };
    if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
        return Err("project_root is only valid in project-isolated mode".into());
    }
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(
        profile,
        &audit,
        &EnvironmentPlanOptions { mode, project_root },
    )?;
    let apply_result = apply_environment(&plan)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        apply_result,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_extract(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "identifiers", "regions", "strict"])?;
    let options = sequence_extract_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        extract_fasta_path(input, output, &options)
    })
}

fn run_sequence_filter(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &[
            "output",
            "min_length",
            "max_length",
            "min_gc_percent",
            "max_gc_percent",
            "max_n_percent",
        ],
    )?;
    let options = sequence_filter_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        filter_fasta_path(input, output, &options)
    })
}

fn run_sequence_reverse_complement(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output"])?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        reverse_complement_fasta_path(input, output)
    })
}

fn run_sequence_translate(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output", "frames", "trim_terminal_stop", "stop_at_first"],
    )?;
    let options = sequence_translate_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        translate_fasta_path(input, output, &options)
    })
}

fn run_sequence_orf(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &[
            "output",
            "min_amino_acids",
            "include_reverse_strand",
            "include_partial_3prime",
        ],
    )?;
    let options = sequence_orf_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        find_orfs_fasta_path(input, output, &options)
    })
}

fn run_sequence_id_normalize(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output", "prefix", "start", "width", "keep_description"],
    )?;
    let options = sequence_id_normalize_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        normalize_fasta_ids_path(input, output, &options)
    })
}

fn run_sequence_merge(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !["output", "allow_duplicate_ids"].contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    let mut inputs = Vec::new();
    for (role, path) in &request.inputs {
        if role != "fasta" && !role.starts_with("fasta-") {
            return Err(format!("sequence.merge.v1 does not accept input role {role}").into());
        }
        let input = resolve_input(base_directory, path);
        ensure_distinct_input_output(&input, &output)?;
        inputs.push(input);
    }
    if inputs.is_empty() {
        return Err("sequence.merge.v1 requires at least one FASTA input".into());
    }
    let options = sequence_merge_options(&request.parameters)?;
    let summary = merge_fasta_paths(&inputs, &output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_split(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output_directory", "records_per_file", "prefix"],
    )?;
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.split.v1 requires inputs.fasta")?;
    let output_directory = request
        .parameters
        .get("output_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or("sequence.split.v1 requires string parameters.output_directory")?;
    let options = sequence_split_options(&request.parameters)?;
    let summary = split_fasta_path(
        resolve_input(base_directory, input),
        resolve_input(base_directory, output_directory),
        &options,
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_to_table(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "delimiter", "include_header"])?;
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.to-table.v1 requires inputs.fasta")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let delimiter = sequence_table_delimiter_option(&request.parameters)?.unwrap_or_else(|| {
        SequenceTableDelimiter::infer_from_path(&output).unwrap_or(SequenceTableDelimiter::Csv)
    });
    let summary = fasta_to_table_path(
        &input,
        &output,
        &SequenceToTableOptions {
            delimiter,
            include_header: optional_parameter_bool(&request.parameters, "include_header")?
                .unwrap_or(true),
        },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_from_table(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let allowed = [
        "output",
        "delimiter",
        "id_column",
        "sequence_column",
        "description_column",
    ];
    validate_v1_named_input_contract(&request, "table", &allowed)?;
    let input = request
        .inputs
        .get("table")
        .ok_or("sequence.from-table.v1 requires inputs.table")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let mut options = sequence_from_table_options(&request.parameters)?;
    if sequence_table_delimiter_option(&request.parameters)?.is_none() {
        options.delimiter =
            SequenceTableDelimiter::infer_from_path(&input).unwrap_or(SequenceTableDelimiter::Csv);
    }
    let summary = table_to_fasta_path(&input, &output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_kmer_count(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "k", "canonical", "top_n"])?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("fasta")
            .ok_or("sequence.kmer.count.v1 requires inputs.fasta")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary = count_kmers_path(&input, &output, &kmer_count_options(&request.parameters)?)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_consensus(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "threshold"])?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("fasta")
            .ok_or("sequence.consensus.v1 requires inputs.fasta")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary =
        consensus_from_alignment_path(&input, &output, &consensus_options(&request.parameters)?)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_shuffle(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "seed"])?;
    let input = resolve_input(
        base_directory,
        request
            .inputs
            .get("fasta")
            .ok_or("sequence.shuffle.v1 requires inputs.fasta")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&input, &output)?;
    let summary = shuffle_sequences_path(&input, &output, &shuffle_options(&request.parameters)?)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_primer_epcr(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_multi_input_contract(
        &request,
        &["fasta", "primers"],
        &["output", "min_amplicon", "max_amplicon", "max_hits"],
    )?;
    let fasta = resolve_input(
        base_directory,
        request
            .inputs
            .get("fasta")
            .ok_or("primer.epcr.v1 requires inputs.fasta")?,
    );
    let primers = resolve_input(
        base_directory,
        request
            .inputs
            .get("primers")
            .ok_or("primer.epcr.v1 requires inputs.primers")?,
    );
    let output = resolve_input(
        base_directory,
        required_sequence_output(&request.parameters, &request.capability)?,
    );
    ensure_distinct_input_output(&fasta, &output)?;
    ensure_distinct_input_output(&primers, &output)?;
    let summary = epcr_path(
        &fasta,
        &primers,
        &output,
        &epcr_options(&request.parameters)?,
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_sequence_transform_v1<T>(
    base_directory: &Path,
    request: JobRequest,
    operation: impl FnOnce(&Path, &Path) -> Result<T, SequenceTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = request
        .inputs
        .get("fasta")
        .ok_or_else(|| format!("{} requires inputs.fasta", request.capability))?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let summary = operation(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_sequence_transform_v2<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    operation: impl FnOnce(&Path, &Path) -> Result<T, SequenceTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = resolve_v2_single_input(base_directory, request, "fasta")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    ensure_v2_export_output_is_distinct(request, base_directory, &output)?;
    let summary = operation(&input, &output)?;
    let size_bytes = std::fs::metadata(&output)?.len();
    let sha256 = sha256_file(&output)?;
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        summary,
        ExecutionMode::LocalCpu,
    );
    result.artifacts.push(OutputArtifact {
        artifact_id: "sequence-output".to_owned(),
        role: "fasta".to_owned(),
        kind: OutputArtifactKind::DomainFile,
        path: output.to_string_lossy().into_owned(),
        format: Some(BioDataFormat::Fasta),
        media_type: Some("text/x-fasta".to_owned()),
        size_bytes: Some(size_bytes),
        sha256: Some(sha256),
        metadata: Default::default(),
    });
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn execute_fastq_transform_v1<T>(
    base_directory: &Path,
    request: JobRequest,
    operation: impl FnOnce(&Path, &Path) -> Result<T, FastqTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = request
        .inputs
        .get("fastq")
        .ok_or_else(|| format!("{} requires inputs.fastq", request.capability))?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let summary = operation(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_fastq_transform_v2<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    operation: impl FnOnce(&Path, &Path) -> Result<T, FastqTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = resolve_v2_single_input(base_directory, request, "fastq")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    ensure_v2_export_output_is_distinct(request, base_directory, &output)?;
    let summary = operation(&input, &output)?;
    serialize_v2_file_artifact_result(
        request,
        base_directory,
        verified_inputs,
        summary,
        FileArtifactSpec {
            artifact_id: "fastq-output",
            role: "fastq",
            kind: OutputArtifactKind::DomainFile,
            path: output,
            format: Some(BioDataFormat::Fastq),
            media_type: Some("text/x-fastq"),
        },
    )
}

fn validate_v1_sequence_contract(request: &JobRequest, allowed: &[&str]) -> WorkerResult<()> {
    if !request.inputs.contains_key("fasta") {
        return Err(format!("{} requires inputs.fasta", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != "fasta" {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_named_input_contract(
    request: &JobRequest,
    expected_role: &str,
    allowed: &[&str],
) -> WorkerResult<()> {
    if !request.inputs.contains_key(expected_role) {
        return Err(format!("{} requires inputs.{expected_role}", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != expected_role {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_multi_input_contract(
    request: &JobRequest,
    expected_roles: &[&str],
    allowed_parameters: &[&str],
) -> WorkerResult<()> {
    for role in expected_roles {
        if !request.inputs.contains_key(*role) {
            return Err(format!("{} requires inputs.{role}", request.capability).into());
        }
    }
    for role in request.inputs.keys() {
        if !expected_roles.contains(&role.as_str()) {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !allowed_parameters.contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_interval_merge_contract(request: &JobRequest) -> WorkerResult<()> {
    if !request.inputs.contains_key("bed") {
        return Err(format!("{} requires inputs.bed", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != "bed" {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !matches!(parameter.as_str(), "output" | "max_gap") {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_interval_subtract_contract(request: &JobRequest) -> WorkerResult<()> {
    for required in ["left-bed", "right-bed"] {
        if !request.inputs.contains_key(required) {
            return Err(format!("{} requires inputs.{required}", request.capability).into());
        }
    }
    for role in request.inputs.keys() {
        if !matches!(role.as_str(), "left-bed" | "right-bed") {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if parameter != "output" {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn parameter_object(
    parameters: &serde_json::Value,
) -> WorkerResult<Option<&serde_json::Map<String, serde_json::Value>>> {
    match parameters {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Object(parameters) => Ok(Some(parameters)),
        _ => Err("sequence transform parameters must be an object".into()),
    }
}

fn required_sequence_output<'a>(
    parameters: &'a serde_json::Value,
    capability: &str,
) -> WorkerResult<&'a str> {
    let output = parameters
        .get("output")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{capability} requires string parameters.output"))?;
    if output.trim().is_empty() {
        return Err(format!("{capability} requires a non-empty parameters.output").into());
    }
    Ok(output)
}

fn sequence_extract_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceExtractOptions> {
    let identifiers = optional_parameter_array(parameters, "identifiers")?;
    let identifiers = identifiers
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("identifiers[{index}] must be a string").into())
        })
        .collect::<WorkerResult<Vec<_>>>()?;
    let regions = optional_parameter_array(parameters, "regions")?;
    let regions = regions
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let specification = value.as_str().ok_or_else(|| -> WorkerError {
                format!("regions[{index}] must be a string").into()
            })?;
            parse_sequence_region_spec(specification).map_err(Into::into)
        })
        .collect::<WorkerResult<Vec<_>>>()?;
    Ok(SequenceExtractOptions {
        identifiers,
        regions,
        strict: optional_parameter_bool(parameters, "strict")?.unwrap_or(false),
    })
}

fn optional_parameter_array(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Vec<serde_json::Value>> {
    match parameters.get(key) {
        Some(value) => value
            .as_array()
            .cloned()
            .ok_or_else(|| format!("{key} must be an array").into()),
        None => Ok(Vec::new()),
    }
}

fn sequence_filter_options(parameters: &serde_json::Value) -> WorkerResult<SequenceFilterOptions> {
    let options = SequenceFilterOptions {
        min_length: optional_parameter_u64(parameters, "min_length")?.unwrap_or(0),
        max_length: optional_parameter_u64(parameters, "max_length")?,
        min_gc_percent: optional_parameter_percentage(parameters, "min_gc_percent")?,
        max_gc_percent: optional_parameter_percentage(parameters, "max_gc_percent")?,
        max_n_percent: optional_parameter_percentage(parameters, "max_n_percent")?,
    };
    if options
        .max_length
        .is_some_and(|maximum| maximum < options.min_length)
    {
        return Err("max_length must be at least min_length".into());
    }
    if matches!(
        (options.min_gc_percent, options.max_gc_percent),
        (Some(minimum), Some(maximum)) if maximum < minimum
    ) {
        return Err("max_gc_percent must be at least min_gc_percent".into());
    }
    Ok(options)
}

fn sequence_translate_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceTranslateOptions> {
    let frames = match parameters.get("frames") {
        None => vec![1],
        Some(value) => {
            let values = value
                .as_array()
                .ok_or("frames must be an array of integers")?;
            if values.is_empty() {
                return Err("frames must contain at least one translation frame".into());
            }
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let frame = value
                        .as_i64()
                        .ok_or_else(|| format!("frames[{index}] must be an integer"))?;
                    let frame = i8::try_from(frame)
                        .map_err(|_| format!("frames[{index}] is outside the supported range"))?;
                    if !matches!(frame, -3..=-1 | 1..=3) {
                        return Err(format!(
                            "unsupported translation frame {frame}; expected -3, -2, -1, 1, 2, or 3"
                        )
                        .into());
                    }
                    Ok(frame)
                })
                .collect::<WorkerResult<Vec<_>>>()?
        }
    };
    Ok(SequenceTranslateOptions {
        frames,
        trim_terminal_stop: optional_parameter_bool(parameters, "trim_terminal_stop")?
            .unwrap_or(false),
        stop_at_first: optional_parameter_bool(parameters, "stop_at_first")?.unwrap_or(false),
    })
}

fn sequence_orf_options(parameters: &serde_json::Value) -> WorkerResult<SequenceOrfOptions> {
    let mut options = SequenceOrfOptions::default();
    if let Some(minimum) = optional_parameter_usize(parameters, "min_amino_acids")? {
        if minimum == 0 {
            return Err("min_amino_acids must be at least 1".into());
        }
        options.min_amino_acids = minimum;
    }
    if let Some(include) = optional_parameter_bool(parameters, "include_reverse_strand")? {
        options.include_reverse_strand = include;
    }
    if let Some(include) = optional_parameter_bool(parameters, "include_partial_3prime")? {
        options.include_partial_3prime = include;
    }
    Ok(options)
}

fn sequence_id_normalize_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceIdNormalizeOptions> {
    let mut options = SequenceIdNormalizeOptions::default();
    if let Some(prefix) = optional_parameter_string(parameters, "prefix")? {
        options.prefix = prefix.to_owned();
    }
    if let Some(start) = optional_parameter_u64(parameters, "start")? {
        if start == 0 {
            return Err("start must be at least 1".into());
        }
        options.start = start;
    }
    if let Some(width) = optional_parameter_usize(parameters, "width")? {
        if width == 0 {
            return Err("width must be at least 1".into());
        }
        options.width = Some(width);
    }
    if let Some(keep) = optional_parameter_bool(parameters, "keep_description")? {
        options.keep_description = keep;
    }
    Ok(options)
}

fn sequence_merge_options(parameters: &serde_json::Value) -> WorkerResult<SequenceMergeOptions> {
    Ok(SequenceMergeOptions {
        allow_duplicate_ids: optional_parameter_bool(parameters, "allow_duplicate_ids")?
            .unwrap_or(false),
    })
}

fn sequence_split_options(parameters: &serde_json::Value) -> WorkerResult<SequenceSplitOptions> {
    let mut options = SequenceSplitOptions::default();
    if let Some(records_per_file) = optional_parameter_usize(parameters, "records_per_file")? {
        if records_per_file == 0 {
            return Err("records_per_file must be at least 1".into());
        }
        options.records_per_file = records_per_file;
    }
    if let Some(prefix) = optional_parameter_string(parameters, "prefix")? {
        options.prefix = prefix.to_owned();
    }
    Ok(options)
}

fn sequence_from_table_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceFromTableOptions> {
    let mut options = SequenceFromTableOptions::default();
    if let Some(delimiter) = sequence_table_delimiter_option(parameters)? {
        options.delimiter = delimiter;
    }
    if let Some(column) = optional_parameter_string(parameters, "id_column")? {
        options.id_column = column.to_owned();
    }
    if let Some(column) = optional_parameter_string(parameters, "sequence_column")? {
        options.sequence_column = column.to_owned();
    }
    if let Some(value) = parameters.get("description_column") {
        options.description_column = if value.is_null() {
            None
        } else {
            Some(
                value
                    .as_str()
                    .ok_or("description_column must be a string or null")?
                    .to_owned(),
            )
        };
    }
    Ok(options)
}

fn kmer_count_options(parameters: &serde_json::Value) -> WorkerResult<KmerCountOptions> {
    let mut options = KmerCountOptions::default();
    if let Some(k) = optional_parameter_usize(parameters, "k")? {
        options.k = k;
    }
    if let Some(canonical) = optional_parameter_bool(parameters, "canonical")? {
        options.canonical = canonical;
    }
    if let Some(top_n) = optional_parameter_usize(parameters, "top_n")? {
        options.top_n = top_n;
    }
    Ok(options)
}

fn consensus_options(parameters: &serde_json::Value) -> WorkerResult<ConsensusOptions> {
    let mut options = ConsensusOptions::default();
    if let Some(threshold) = optional_parameter_f64(parameters, "threshold")? {
        options.threshold = threshold;
    }
    Ok(options)
}

fn fastq_subsample_options(parameters: &serde_json::Value) -> WorkerResult<FastqSubsampleOptions> {
    let mut options = FastqSubsampleOptions::default();
    if let Some(count) = optional_parameter_u64(parameters, "target_count")? {
        options.target_count = Some(count);
    }
    if let Some(fraction) = optional_parameter_f64(parameters, "fraction")? {
        options.fraction = Some(fraction);
    }
    if let Some(seed) = optional_parameter_u64(parameters, "seed")? {
        options.seed = seed;
    }
    Ok(options)
}

fn shuffle_options(parameters: &serde_json::Value) -> WorkerResult<ShuffleOptions> {
    let mut options = ShuffleOptions::default();
    if let Some(seed) = optional_parameter_u64(parameters, "seed")? {
        options.seed = seed;
    }
    Ok(options)
}

fn epcr_options(parameters: &serde_json::Value) -> WorkerResult<EpcrOptions> {
    let mut options = EpcrOptions::default();
    if let Some(value) = optional_parameter_usize(parameters, "min_amplicon")? {
        options.min_amplicon = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_amplicon")? {
        options.max_amplicon = value;
    }
    if let Some(value) = optional_parameter_usize(parameters, "max_hits")? {
        options.max_hits = value;
    }
    Ok(options)
}

fn variant_filter_options(parameters: &serde_json::Value) -> WorkerResult<VariantFilterOptions> {
    Ok(VariantFilterOptions {
        min_qual: optional_parameter_f64(parameters, "min_qual")?,
        require_pass: optional_parameter_bool(parameters, "require_pass")?.unwrap_or(false),
        contigs: optional_string_array_parameter(parameters, "contigs")?,
        min_info_dp: optional_parameter_u64(parameters, "min_info_dp")?,
    })
}

fn sequence_table_delimiter_option(
    parameters: &serde_json::Value,
) -> WorkerResult<Option<SequenceTableDelimiter>> {
    match optional_parameter_string(parameters, "delimiter")? {
        Some("csv") => Ok(Some(SequenceTableDelimiter::Csv)),
        Some("tsv" | "tab") => Ok(Some(SequenceTableDelimiter::Tsv)),
        Some(value) => Err(format!("delimiter must be csv or tsv, got {value:?}").into()),
        None => Ok(None),
    }
}

fn optional_parameter_u64(parameters: &serde_json::Value, key: &str) -> WorkerResult<Option<u64>> {
    match parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_parameter_usize(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<usize>> {
    optional_parameter_u64(parameters, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn optional_parameter_u8(parameters: &serde_json::Value, key: &str) -> WorkerResult<Option<u8>> {
    optional_parameter_u64(parameters, key)?
        .map(|value| u8::try_from(value).map_err(|_| format!("{key} must be 0..255").into()))
        .transpose()
}

fn optional_parameter_f64(parameters: &serde_json::Value, key: &str) -> WorkerResult<Option<f64>> {
    match parameters.get(key) {
        Some(value) => {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("{key} must be a number"))?;
            if !number.is_finite() {
                return Err(format!("{key} must be finite").into());
            }
            Ok(Some(number))
        }
        None => Ok(None),
    }
}

fn optional_parameter_bool(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<bool>> {
    match parameters.get(key) {
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a boolean").into()),
        None => Ok(None),
    }
}

fn optional_parameter_string<'a>(
    parameters: &'a serde_json::Value,
    key: &str,
) -> WorkerResult<Option<&'a str>> {
    match parameters.get(key) {
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string").into()),
        None => Ok(None),
    }
}

fn optional_parameter_percentage(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<f64>> {
    match parameters.get(key) {
        Some(value) => {
            let percent = value
                .as_f64()
                .ok_or_else(|| format!("{key} must be a number"))?;
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                return Err(format!("{key} must be between 0 and 100").into());
            }
            Ok(Some(percent))
        }
        None => Ok(None),
    }
}

fn run_sequence_stats(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.stats.v1 requires inputs.fasta")?;
    let input_path = resolve_input(base_directory, input);
    let stats = fasta_stats_path(input_path)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn optional_u64_parameter(request: &JobRequest, key: &str) -> WorkerResult<Option<u64>> {
    match request.parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_usize_parameter(request: &JobRequest, key: &str) -> WorkerResult<Option<usize>> {
    optional_u64_parameter(request, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn resolve_input(base_directory: &Path, input: &str) -> PathBuf {
    let input_path = PathBuf::from(input);
    if input_path.is_absolute() {
        input_path
    } else {
        base_directory.join(input_path)
    }
}

#[cfg(test)]
mod tests {
    use super::{execute_request, execute_request_v2, validate_v2_inputs};
    use linxira_bio_protocol::{
        AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, DiagnosticSeverity,
        ExecutionMode, ExecutionRequest, InputArtifact, InputCardinality, JobRequest, JobRequestV2,
        JobStatus, SCHEMA_VERSION, SCHEMA_VERSION_V2,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn rejects_non_string_environment_mode() {
        let error = execute_request(
            environment_plan_request(serde_json::json!({"mode": 42})),
            Path::new("."),
        )
        .expect_err("invalid mode must fail");

        assert!(error.to_string().contains("mode must be a string"));
    }

    #[test]
    fn rejects_project_root_outside_project_mode() {
        let error = execute_request(
            environment_plan_request(serde_json::json!({
                "mode": "managed-user",
                "project_root": "."
            })),
            Path::new("."),
        )
        .expect_err("unexpected project root must fail");

        assert!(error.to_string().contains("only valid in project-isolated"));
    }

    #[test]
    fn v2_execution_failure_returns_an_error_envelope() {
        let request: JobRequestV2 = serde_json::from_value(serde_json::json!({
            "schema_version": "2",
            "job_id": "unsupported-capability-test",
            "capability": "unknown.operation.v1",
            "inputs": [],
            "execution": {"mode": "local-cpu"},
            "parameters": {}
        }))
        .expect("typed v2 request");

        let json = execute_request_v2(request, Path::new("."))
            .expect("failure must use the v2 result transport");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&json).expect("v2 error result");

        assert_eq!(result.job_id, "unsupported-capability-test");
        assert_eq!(result.capability, "unknown.operation.v1");
        assert_eq!(result.status, JobStatus::Error);
        assert_eq!(result.result, serde_json::json!({}));
        assert!(result.artifacts.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "job-failed");
        assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("unsupported capability")
        );
    }

    #[test]
    fn rejects_v2_fasta_and_vcf_format_mismatches() {
        let cases: [(&str, &[u8], BioDataFormat, &str); 3] = [
            (
                "actual-fasta.fa",
                b">sequence\nACGT\n",
                BioDataFormat::Vcf,
                "content identifies fasta",
            ),
            (
                "actual-variants.vcf",
                b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
                BioDataFormat::Fasta,
                "content identifies vcf",
            ),
            (
                "fasta-declared-json.fa",
                b">sequence\nACGT\n",
                BioDataFormat::Json,
                "content identifies fasta",
            ),
        ];

        for (name, contents, declared_format, expected_message) in cases {
            let path = write_temporary(name, contents);
            let request = artifact_request(
                &path,
                declared_format,
                CompressionFormat::None,
                "dataset.inspect.v1",
                "file",
            );
            let error = validate_v2_inputs(&request, Path::new("."))
                .expect_err("format mismatch must fail validation");
            fs::remove_file(&path).expect("remove format fixture");

            assert!(error.to_string().contains("format mismatch"), "{name}");
            assert!(error.to_string().contains(expected_message), "{name}");
        }
    }

    #[test]
    fn rejects_v2_unverifiable_and_detected_compression_mismatches() {
        let gzip_signature = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0];
        let cases: [(&str, &[u8], CompressionFormat, &str); 5] = [
            (
                "plain.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Gzip,
                "signature identifies none",
            ),
            (
                "compressed.data",
                &gzip_signature,
                CompressionFormat::None,
                "signature identifies gzip",
            ),
            (
                "claimed-bzip2.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Bzip2,
                "signature identifies none",
            ),
            (
                "claimed-xz.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Xz,
                "signature identifies none",
            ),
            (
                "claimed-zstd.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Zstd,
                "signature identifies none",
            ),
        ];

        for (name, contents, declared_compression, expected_message) in cases {
            let path = write_temporary(name, contents);
            let request = artifact_request(
                &path,
                BioDataFormat::Unknown,
                declared_compression,
                "dataset.inspect.v1",
                "file",
            );
            let error = validate_v2_inputs(&request, Path::new("."))
                .expect_err("compression mismatch must fail validation");
            fs::remove_file(&path).expect("remove compression fixture");

            assert!(error.to_string().contains("compression mismatch"), "{name}");
            assert!(error.to_string().contains(expected_message), "{name}");
        }
    }

    #[test]
    fn v2_unknown_declarations_and_unknown_detection_are_non_blocking() {
        let fasta = write_temporary("known.fa", b">sequence\nACGT\n");
        let unknown_declaration = artifact_request(
            &fasta,
            BioDataFormat::Unknown,
            CompressionFormat::Unknown,
            "dataset.inspect.v1",
            "file",
        );
        validate_v2_inputs(&unknown_declaration, Path::new("."))
            .expect("unknown declarations are wildcards");
        fs::remove_file(&fasta).expect("remove known fixture");

        let opaque = write_temporary("opaque.fa", b"one opaque line\n");
        let unknown_detection = artifact_request(
            &opaque,
            BioDataFormat::Vcf,
            CompressionFormat::None,
            "dataset.inspect.v1",
            "file",
        );
        validate_v2_inputs(&unknown_detection, Path::new("."))
            .expect("extension-only detection does not contradict a declaration");
        fs::remove_file(&opaque).expect("remove opaque fixture");
    }

    #[test]
    fn v2_json_table_export_is_not_blocked_by_unknown_format_detection() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/results/metrics.json")
            .canonicalize()
            .expect("metrics fixture");
        let output = temporary_path("export.csv");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Json,
            CompressionFormat::None,
            "table.export.v1",
            "table",
        );
        request.parameters = serde_json::json!({"output": output});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("JSON table export remains executable");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid table export result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "table.export.v1");
        assert!(fs::metadata(&output).expect("exported table").len() > 0);
        fs::remove_file(output).expect("remove exported table");
    }

    #[test]
    fn v2_table_export_rejects_an_undeclared_extra_input_role() {
        let table = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/results/metrics.json")
            .canonicalize()
            .expect("metrics fixture");
        let protected = write_temporary("protected.json", br#"{"protected":true}"#);
        let mut request = artifact_request(
            &table,
            BioDataFormat::Json,
            CompressionFormat::None,
            "table.export.v1",
            "table",
        );
        request.inputs.push(InputArtifact {
            artifact_id: "protected-artifact".to_owned(),
            role: "metadata".to_owned(),
            cardinality: InputCardinality::Single,
            files: vec![ArtifactFile {
                file_id: "protected-file".to_owned(),
                path: protected.to_string_lossy().into_owned(),
                role: None,
                format: BioDataFormat::Json,
                compression: CompressionFormat::None,
                size_bytes: fs::metadata(&protected).expect("protected metadata").len(),
                modified_at: None,
                sha256: None,
            }],
            dataset_id: None,
        });
        request.parameters = serde_json::json!({"output": protected});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("does not accept input role metadata")
        );
        assert_eq!(
            fs::read_to_string(&protected).expect("declared input remains readable"),
            r#"{"protected":true}"#
        );
        fs::remove_file(protected).expect("remove protected input");
    }

    #[test]
    fn v2_rejects_duplicate_roles_instead_of_selecting_the_first() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/sequences/tiny.fa")
            .canonicalize()
            .expect("FASTA fixture");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.stats.v1",
            "fasta",
        );
        let mut duplicate = request.inputs[0].clone();
        duplicate.artifact_id = "duplicate-artifact".to_owned();
        duplicate.files[0].file_id = "duplicate-file".to_owned();
        request.inputs.push(duplicate);

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("duplicate input role: fasta")
        );
    }

    #[test]
    fn v2_rejects_unknown_parameters() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/sequences/tiny.fa")
            .canonicalize()
            .expect("FASTA fixture");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.stats.v1",
            "fasta",
        );
        request.parameters = serde_json::json!({"max_cylces": 100});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("does not accept parameter max_cylces")
        );
    }

    #[test]
    fn v1_executes_all_sequence_transform_capabilities() {
        let input = write_temporary(
            "sequence-transform-v1.fa",
            b">gene description\nATGAAATAA\n>gc\nGCGCGC\n>short\nNN\n",
        );
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": ["gene"], "regions": ["gene:1-3"], "strict": true}),
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"min_length": 6, "min_gc_percent": 50}),
            ),
            ("sequence.reverse-complement.v1", serde_json::json!({})),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [1, -1], "trim_terminal_stop": true}),
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({
                    "min_amino_acids": 2,
                    "include_reverse_strand": false
                }),
            ),
            (
                "sequence.id.normalize.v1",
                serde_json::json!({
                    "prefix": "seq",
                    "start": 1,
                    "width": 3,
                    "keep_description": true
                }),
            ),
        ];

        for (capability, mut parameters) in cases {
            let output = temporary_path(&format!("{capability}.fa"));
            parameters["output"] = serde_json::json!(output);
            let request = sequence_v1_request(&input, capability, parameters);
            let serialized = execute_request(request, Path::new("."))
                .unwrap_or_else(|error| panic!("{capability} failed: {error}"));
            let result: serde_json::Value =
                serde_json::from_str(&serialized).expect("valid v1 sequence result");

            assert_eq!(result["status"], "ok", "{capability}");
            assert_eq!(result["capability"], capability, "{capability}");
            assert!(fs::metadata(&output).expect("sequence output").len() > 0);
            fs::remove_file(output).expect("remove sequence output");
        }
        fs::remove_file(input).expect("remove v1 sequence input");
    }

    #[test]
    fn v2_executes_all_sequence_transforms_with_hashed_fasta_artifacts() {
        let input = write_temporary(
            "sequence-transform-v2.fa",
            b">gene description\nATGAAATAA\n>gc\nGCGCGC\n>short\nNN\n",
        );
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": ["gene"], "regions": ["gene:1-3"], "strict": true}),
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"max_n_percent": 0}),
            ),
            ("sequence.reverse-complement.v1", serde_json::json!({})),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [1, -1], "stop_at_first": false}),
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({
                    "min_amino_acids": 2,
                    "include_reverse_strand": true,
                    "include_partial_3prime": true
                }),
            ),
            (
                "sequence.id.normalize.v1",
                serde_json::json!({
                    "prefix": "seq",
                    "start": 1,
                    "width": 3,
                    "keep_description": true
                }),
            ),
        ];

        for (capability, mut parameters) in cases {
            let output = temporary_path(&format!("{capability}-v2.fa"));
            parameters["output"] = serde_json::json!(output);
            let mut request = artifact_request(
                &input,
                BioDataFormat::Fasta,
                CompressionFormat::None,
                capability,
                "fasta",
            );
            request.parameters = parameters;
            let serialized = execute_request_v2(request, Path::new("."))
                .unwrap_or_else(|error| panic!("{capability} failed: {error}"));
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 sequence result");

            assert_eq!(result.status, JobStatus::Ok, "{capability}");
            assert_eq!(result.capability, capability, "{capability}");
            assert_eq!(result.provenance.input_sha256.len(), 1, "{capability}");
            let expected_input_hash = super::sha256_file(&input).expect("hash sequence input");
            assert_eq!(
                result.provenance.input_sha256.get("input-file"),
                Some(&expected_input_hash),
                "{capability}"
            );
            assert_eq!(result.artifacts.len(), 1, "{capability}");
            let artifact = &result.artifacts[0];
            assert_eq!(artifact.role, "fasta", "{capability}");
            assert_eq!(
                artifact.kind,
                linxira_bio_protocol::OutputArtifactKind::DomainFile
            );
            assert_eq!(artifact.format, Some(BioDataFormat::Fasta));
            assert_eq!(artifact.media_type.as_deref(), Some("text/x-fasta"));
            assert_eq!(PathBuf::from(&artifact.path), output, "{capability}");
            assert_eq!(
                artifact.size_bytes,
                Some(fs::metadata(&output).expect("sequence output").len()),
                "{capability}"
            );
            let expected_output_hash = super::sha256_file(&output).expect("hash sequence output");
            assert_eq!(
                artifact.sha256.as_deref(),
                Some(expected_output_hash.as_str()),
                "{capability}"
            );
            fs::remove_file(output).expect("remove v2 sequence output");
        }
        fs::remove_file(input).expect("remove v2 sequence input");
    }

    #[test]
    fn v2_executes_sequence_merge_split_and_table_conversion() {
        let first = write_temporary("sequence-merge-first.fa", b">one\nACGT\n>two\nNN\n");
        let second = write_temporary("sequence-merge-second.fa", b">three\nGG\n");

        let merged_output = temporary_path("sequence-merged.fa");
        let mut merge_request = artifact_request(
            &first,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.merge.v1",
            "fasta",
        );
        merge_request.inputs[0].cardinality = InputCardinality::Batch;
        merge_request.inputs[0].files.push(ArtifactFile {
            file_id: "input-file-2".to_owned(),
            path: second.to_string_lossy().into_owned(),
            role: None,
            format: BioDataFormat::Fasta,
            compression: CompressionFormat::None,
            size_bytes: fs::metadata(&second).expect("second input metadata").len(),
            modified_at: None,
            sha256: None,
        });
        merge_request.parameters = serde_json::json!({"output": merged_output});
        let serialized =
            execute_request_v2(merge_request, Path::new(".")).expect("merge executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid merge result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.merge.v1");
        assert_eq!(result.result["input_files"], 2);
        assert_eq!(result.result["output_records"], 3);
        assert_eq!(result.provenance.input_sha256.len(), 2);
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Fasta));
        assert!(fs::metadata(&merged_output).expect("merged output").len() > 0);

        let split_directory = temporary_path("sequence-split-output");
        let mut split_request = artifact_request(
            &merged_output,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.split.v1",
            "fasta",
        );
        split_request.parameters = serde_json::json!({
            "output_directory": split_directory,
            "records_per_file": 2,
            "prefix": "chunk"
        });
        let serialized =
            execute_request_v2(split_request, Path::new(".")).expect("split executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid split result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.split.v1");
        assert_eq!(result.result["output_files"], 2);
        assert_eq!(
            result.artifacts[0].kind,
            linxira_bio_protocol::OutputArtifactKind::Directory
        );
        assert!(split_directory.join("chunk_001.fa").is_file());

        let table_output = temporary_path("sequence-table.tsv");
        let mut table_request = artifact_request(
            &merged_output,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.to-table.v1",
            "fasta",
        );
        table_request.parameters = serde_json::json!({"output": table_output, "delimiter": "tsv"});
        let serialized = execute_request_v2(table_request, Path::new("."))
            .expect("to-table executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid to-table result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.to-table.v1");
        assert_eq!(result.result["output_rows"], 3);
        assert_eq!(
            result.artifacts[0].kind,
            linxira_bio_protocol::OutputArtifactKind::Table
        );
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Tsv));

        let roundtrip_output = temporary_path("sequence-table-roundtrip.fa");
        let mut roundtrip_request = artifact_request(
            &table_output,
            BioDataFormat::Tsv,
            CompressionFormat::None,
            "sequence.from-table.v1",
            "table",
        );
        roundtrip_request.parameters =
            serde_json::json!({"output": roundtrip_output, "delimiter": "tsv"});
        let serialized = execute_request_v2(roundtrip_request, Path::new("."))
            .expect("from-table executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid from-table result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.from-table.v1");
        assert_eq!(result.result["output_records"], 3);
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Fasta));
        assert!(
            fs::read_to_string(&roundtrip_output)
                .expect("roundtrip FASTA")
                .contains(">three\nGG\n")
        );

        fs::remove_file(first).expect("remove first merge input");
        fs::remove_file(second).expect("remove second merge input");
        fs::remove_file(merged_output).expect("remove merged output");
        fs::remove_dir_all(split_directory).expect("remove split output");
        fs::remove_file(table_output).expect("remove table output");
        fs::remove_file(roundtrip_output).expect("remove roundtrip output");
    }

    #[test]
    fn sequence_transform_requests_reject_invalid_contracts_and_values() {
        let input = write_temporary("sequence-transform-invalid.fa", b">gene\nATGAAATAA\n");
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": "gene"}),
                "identifiers must be an array",
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"min_gc_percent": 101}),
                "must be between 0 and 100",
            ),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [0]}),
                "unsupported translation frame 0",
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({"min_amino_acids": 0}),
                "must be at least 1",
            ),
            (
                "sequence.reverse-complement.v1",
                serde_json::json!({"unexpected": true}),
                "does not accept parameter unexpected",
            ),
        ];

        for (capability, mut parameters, expected_message) in cases {
            let output = temporary_path(&format!("invalid-{capability}.fa"));
            parameters["output"] = serde_json::json!(output);
            let mut request = artifact_request(
                &input,
                BioDataFormat::Fasta,
                CompressionFormat::None,
                capability,
                "fasta",
            );
            request.parameters = parameters;
            let serialized = execute_request_v2(request, Path::new("."))
                .expect("invalid v2 request uses an error envelope");
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 error result");

            assert_eq!(result.status, JobStatus::Error, "{capability}");
            assert!(
                result.diagnostics[0].message.contains(expected_message),
                "{capability}: {}",
                result.diagnostics[0].message
            );
            assert!(result.artifacts.is_empty(), "{capability}");
            assert!(!output.exists(), "{capability}");
        }

        let output = temporary_path("wrong-role.fa");
        let mut wrong_role = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.filter.v1",
            "file",
        );
        wrong_role.parameters = serde_json::json!({"output": output});
        let serialized = execute_request_v2(wrong_role, Path::new("."))
            .expect("wrong role uses an error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid wrong-role result");
        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("requires input role fasta")
        );

        let mut legacy = sequence_v1_request(
            &input,
            "sequence.reverse-complement.v1",
            serde_json::json!({"output": output, "unexpected": true}),
        );
        legacy
            .inputs
            .insert("extra".to_owned(), input.to_string_lossy().into_owned());
        let error = execute_request(legacy, Path::new("."))
            .expect_err("legacy request rejects extra input roles");
        assert!(
            error
                .to_string()
                .contains("does not accept input role extra")
        );

        fs::remove_file(input).expect("remove invalid sequence input");
    }

    #[test]
    fn v2_sequence_transform_preserves_an_existing_output() {
        let input = write_temporary("sequence-transform-input.fa", b">sequence\nACGT\n");
        let output = write_temporary("sequence-transform-protected.fa", b"protected\n");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.reverse-complement.v1",
            "fasta",
        );
        request.parameters = serde_json::json!({"output": output});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("existing output uses an error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid overwrite error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(result.artifacts.is_empty());
        assert!(
            result.diagnostics[0]
                .message
                .contains("refusing to overwrite")
        );
        assert_eq!(
            fs::read_to_string(&output).expect("protected output remains"),
            "protected\n"
        );
        fs::remove_file(input).expect("remove overwrite input");
        fs::remove_file(output).expect("remove protected output");
    }

    #[test]
    fn v2_executes_new_single_input_qc_capabilities() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cases = [
            (
                root.join("tests/fixtures/alignment-qc/valid.sam"),
                BioDataFormat::Sam,
                "alignment.qc.v1",
                "sam",
                "record_count",
                5,
            ),
            (
                root.join("tests/fixtures/expression-matrix/counts.tsv"),
                BioDataFormat::Tsv,
                "expression.matrix.qc.v1",
                "matrix",
                "feature_count",
                4,
            ),
            (
                root.join("tests/fixtures/cohort/participants.tsv"),
                BioDataFormat::Tsv,
                "medical.cohort-table.qc.v1",
                "cohort",
                "row_count",
                4,
            ),
        ];

        for (path, format, capability, role, field, expected) in cases {
            let request = artifact_request(
                &path.canonicalize().expect("fixture path"),
                format,
                CompressionFormat::None,
                capability,
                role,
            );
            let serialized =
                execute_request_v2(request, Path::new(".")).expect("execute v2 local capability");
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 result");
            assert_eq!(result.status, JobStatus::Ok, "{capability}");
            assert_eq!(result.result[field], expected, "{capability}");
            assert_eq!(result.provenance.input_sha256.len(), 1, "{capability}");
        }
    }

    #[test]
    fn v2_executes_interval_intersection_with_two_hashed_inputs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let left = root
            .join("tests/fixtures/interval-intersect/left.bed")
            .canonicalize()
            .expect("left fixture");
        let right = root
            .join("tests/fixtures/interval-intersect/right.bed")
            .canonicalize()
            .expect("right fixture");
        let mut request = artifact_request(
            &left,
            BioDataFormat::Bed,
            CompressionFormat::None,
            "interval.intersect.v1",
            "left-bed",
        );
        request.inputs.push(InputArtifact {
            artifact_id: "right-artifact".to_owned(),
            role: "right-bed".to_owned(),
            cardinality: InputCardinality::Single,
            files: vec![ArtifactFile {
                file_id: "right-file".to_owned(),
                path: right.to_string_lossy().into_owned(),
                role: None,
                format: BioDataFormat::Bed,
                compression: CompressionFormat::None,
                size_bytes: fs::metadata(&right).expect("right metadata").len(),
                modified_at: None,
                sha256: None,
            }],
            dataset_id: None,
        });

        let serialized =
            execute_request_v2(request, Path::new(".")).expect("execute v2 intersection");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.result["overlap_pair_count"], 3);
        assert_eq!(result.provenance.input_sha256.len(), 2);
    }

    fn environment_plan_request(parameters: serde_json::Value) -> JobRequest {
        JobRequest {
            schema_version: SCHEMA_VERSION.to_owned(),
            job_id: "environment-plan-test".to_owned(),
            capability: "environment.plan.v1".to_owned(),
            inputs: BTreeMap::new(),
            execution: ExecutionRequest {
                mode: ExecutionMode::LocalCpu,
            },
            parameters,
        }
    }

    fn sequence_v1_request(
        path: &Path,
        capability: &str,
        parameters: serde_json::Value,
    ) -> JobRequest {
        JobRequest {
            schema_version: SCHEMA_VERSION.to_owned(),
            job_id: "sequence-transform-v1-test".to_owned(),
            capability: capability.to_owned(),
            inputs: BTreeMap::from([("fasta".to_owned(), path.to_string_lossy().into_owned())]),
            execution: ExecutionRequest {
                mode: ExecutionMode::LocalCpu,
            },
            parameters,
        }
    }

    fn artifact_request(
        path: &Path,
        format: BioDataFormat,
        compression: CompressionFormat,
        capability: &str,
        role: &str,
    ) -> JobRequestV2 {
        JobRequestV2 {
            schema_version: SCHEMA_VERSION_V2.to_owned(),
            job_id: "artifact-validation-test".to_owned(),
            capability: capability.to_owned(),
            inputs: vec![InputArtifact {
                artifact_id: "input-artifact".to_owned(),
                role: role.to_owned(),
                cardinality: InputCardinality::Single,
                files: vec![ArtifactFile {
                    file_id: "input-file".to_owned(),
                    path: path.to_string_lossy().into_owned(),
                    role: None,
                    format,
                    compression,
                    size_bytes: fs::metadata(path).expect("input metadata").len(),
                    modified_at: None,
                    sha256: None,
                }],
                dataset_id: None,
            }],
            execution: ExecutionRequest {
                mode: ExecutionMode::LocalCpu,
            },
            parameters: serde_json::json!({}),
        }
    }

    fn write_temporary(name: &str, contents: &[u8]) -> PathBuf {
        let path = temporary_path(name);
        fs::write(&path, contents).expect("write artifact fixture");
        path
    }

    fn temporary_path(name: &str) -> PathBuf {
        let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-bio-worker-artifact-{}-{counter}-{name}",
            std::process::id()
        ))
    }
}
