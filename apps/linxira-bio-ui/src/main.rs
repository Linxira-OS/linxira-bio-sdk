#![forbid(unsafe_code)]

mod structure_viewer;
mod visualization;

use eframe::egui;
use linxira_bio_export::export_value;
use linxira_bio_protocol::{ExecutionMode, ExecutionRequest, JobRequest, SCHEMA_VERSION};
use linxira_bio_worker::execute_request;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender, TryRecvError},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use structure_viewer::StructureViewer;

/// Central visual theme for the desktop UI. Every color, spacing, radius, and
/// type size used in this file is drawn from these tokens so the interface
/// stays coherent as it grows. Keep values here instead of scattering them
/// across the code.
mod theme {
    use eframe::egui;

    // --- Accent palette (light theme) ---
    /// Brand green: sidebar masthead and primary emphasis.
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(23, 81, 72);
    /// Success/primary green: ready states, section rules, drop-zone hover.
    pub const ACCENT_STRONG: egui::Color32 = egui::Color32::from_rgb(32, 116, 86);
    /// Interactive accent: selections and hovered controls.
    pub const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(42, 123, 105);
    /// Pale green tint for hovered/dragged surfaces (drop zone).
    pub const ACCENT_TINT: egui::Color32 = egui::Color32::from_rgb(226, 241, 237);
    /// Neutral muted text: captions, secondary labels, empty states.
    pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(73, 88, 83);
    /// Informational blue: inspecting/running states.
    pub const INFO: egui::Color32 = egui::Color32::from_rgb(49, 103, 158);
    /// Warning amber: pending/warning states.
    pub const WARNING: egui::Color32 = egui::Color32::from_rgb(176, 104, 24);
    /// Strong amber for environment warnings.
    pub const WARNING_AMBER: egui::Color32 = egui::Color32::from_rgb(160, 90, 0);
    /// Error red: invalid/failed states.
    pub const DANGER: egui::Color32 = egui::Color32::from_rgb(174, 57, 57);
    /// Deep red for blockers and missing documents.
    pub const DANGER_DEEP: egui::Color32 = egui::Color32::from_rgb(160, 70, 40);
    /// Hyperlink blue.
    pub const LINK: egui::Color32 = egui::Color32::from_rgb(32, 101, 145);

    // --- Surfaces ---
    /// Main window background.
    pub const WINDOW_BG: egui::Color32 = egui::Color32::from_rgb(252, 253, 252);
    /// Panel/sidebar background.
    pub const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(249, 250, 249);
    /// Faint background: stripes, table tints, subtle fills.
    pub const FAINT_BG: egui::Color32 = egui::Color32::from_rgb(238, 242, 240);
    /// Elevated surfaces: cards and result panels.
    pub const ELEVATED_BG: egui::Color32 = egui::Color32::WHITE;
    /// Code and monospace backgrounds.
    pub const CODE_BG: egui::Color32 = egui::Color32::from_rgb(235, 239, 237);
    /// Widget resting background.
    pub const WIDGET_BG: egui::Color32 = egui::Color32::from_rgb(239, 243, 241);
    /// Widget hovered background.
    pub const WIDGET_BG_HOVER: egui::Color32 = egui::Color32::from_rgb(224, 236, 232);
    /// Drop-zone resting fill.
    pub const DROP_BG: egui::Color32 = egui::Color32::from_rgb(246, 248, 247);
    /// Hairline border color.
    pub const BORDER: egui::Color32 = egui::Color32::from_rgb(190, 199, 196);

    // --- Spacing and layout ---
    /// Fixed sidebar width (navigation + dataset list).
    pub const SIDEBAR_WIDTH: f32 = 200.0;
    /// Right-hand context panel width in the workspace.
    pub const CONTEXT_WIDTH: f32 = 280.0;
    /// Gap between workspace content and the context panel.
    pub const CONTEXT_GAP: f32 = 18.0;
    /// Height reserved for the bottom status bar.
    pub const STATUS_BAR_HEIGHT: f32 = 28.0;
    /// Height of a sidebar navigation button.
    pub const NAV_BUTTON_HEIGHT: f32 = 36.0;
    /// Height of a workspace tab button.
    pub const TAB_BUTTON_HEIGHT: f32 = 34.0;
    /// Uniform inner padding for panels and cards.
    pub const PANEL_MARGIN: egui::Margin = egui::Margin::symmetric(14, 12);
    /// Uniform corner rounding for frames and widgets.
    pub const CORNER_RADIUS: egui::CornerRadius = egui::CornerRadius::same(6);

    // --- Typography ---
    /// Page/section titles.
    pub const TITLE_SIZE: f32 = 18.0;
    /// Panel and card headers.
    pub const SUBTITLE_SIZE: f32 = 14.0;
    /// Body emphasis (dataset list labels).
    pub const BODY_SIZE: f32 = 13.0;
    /// Small labels and captions.
    pub const SMALL_SIZE: f32 = 12.0;
}

fn main() -> eframe::Result {
    let startup_paths = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let options = eframe::NativeOptions {
        renderer: preferred_renderer(),
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_100.0, 700.0])
            .with_min_inner_size([760.0, 500.0])
            .with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "Linxira Bio SDK",
        options,
        Box::new(move |context| Ok(Box::new(BioApp::new(context, startup_paths)))),
    )
}

fn preferred_renderer() -> eframe::Renderer {
    match std::env::var("LINXIRA_BIO_RENDERER")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "glow" => eframe::Renderer::Glow,
        "wgpu" => eframe::Renderer::Wgpu,
        _ if cfg!(target_os = "windows") => eframe::Renderer::Glow,
        _ => eframe::Renderer::Wgpu,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Workspace,
    Structure,
    Environment,
    Documentation,
    Licenses,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspaceTab {
    Import,
    Dataset,
    Analysis,
    Results,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UserMode {
    Guided,
    Expert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatasetState {
    Inspecting,
    Ready,
    Warning,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPathIssue {
    DriveRelative,
    Directory,
    NonUtf8,
    Unreadable,
}

impl DatasetState {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Inspecting => language.text("检查中", "Inspecting"),
            Self::Ready => language.text("可用", "Ready"),
            Self::Warning => language.text("有警告", "Warning"),
            Self::Invalid => language.text("不可用", "Invalid"),
        }
    }

    fn color(self) -> egui::Color32 {
        match self {
            Self::Inspecting => theme::INFO,
            Self::Ready => theme::ACCENT_STRONG,
            Self::Warning => theme::WARNING,
            Self::Invalid => theme::DANGER,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JobState {
    Running,
    Completed,
    Failed,
}

impl JobState {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Running => language.text("运行中", "Running"),
            Self::Completed => language.text("已完成", "Completed"),
            Self::Failed => language.text("失败", "Failed"),
        }
    }
}

struct DatasetEntry {
    id: String,
    name: String,
    path: String,
    format_hint: String,
    state: DatasetState,
    inspection: Option<Value>,
    message: String,
}

struct InspectionMessage {
    generation: u64,
    dataset_id: String,
    result: UiJobResult,
}

#[derive(Clone)]
struct InspectionTask {
    generation: u64,
    dataset_id: String,
    path: String,
}

struct AnalysisMessage {
    generation: u64,
    job_id: String,
    dataset_id: String,
    capability: String,
    result: UiJobResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnalysisRoute {
    capability: &'static str,
    input_role: &'static str,
}

struct JobRecord {
    id: String,
    capability: String,
    dataset_name: String,
    state: JobState,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectFile {
    schema_version: String,
    name: String,
    files: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    ZhCn,
    EnUs,
}

impl Language {
    fn locale(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LegalDocument {
    ProjectLicense,
    ThirdPartyPolicy,
    RustDependencies,
    FontLicense,
}

impl LegalDocument {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::ProjectLicense => language.text("项目 AGPL", "Project AGPL"),
            Self::ThirdPartyPolicy => language.text("第三方组件", "Third-party components"),
            Self::RustDependencies => language.text("Rust 依赖", "Rust dependencies"),
            Self::FontLicense => language.text("字体协议", "Font license"),
        }
    }
}

#[derive(Debug)]
struct DependencyNotices {
    lines: Vec<String>,
    directory: PathBuf,
    package_count: usize,
}

impl Language {
    fn text(self, zh_cn: &'static str, en_us: &'static str) -> &'static str {
        match self {
            Self::ZhCn => zh_cn,
            Self::EnUs => en_us,
        }
    }
}

#[derive(Clone, Copy)]
enum EnvironmentJob {
    Audit,
    Plan,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvironmentPlanMode {
    UseExisting,
    ManagedUser,
    ProjectIsolated,
    SystemMissingOnly,
}

impl EnvironmentPlanMode {
    fn id(self) -> &'static str {
        match self {
            Self::UseExisting => "use-existing",
            Self::ManagedUser => "managed-user",
            Self::ProjectIsolated => "project-isolated",
            Self::SystemMissingOnly => "system-missing-only",
        }
    }

    fn label(self, language: Language) -> &'static str {
        match self {
            Self::UseExisting => language.text("仅使用现有", "Use existing"),
            Self::ManagedUser => language.text("用户隔离", "Managed user"),
            Self::ProjectIsolated => language.text("项目隔离", "Project isolated"),
            Self::SystemMissingOnly => language.text("系统缺失项", "System missing only"),
        }
    }
}

type UiJobResult = Result<String, String>;

const MAX_CONCURRENT_INSPECTIONS: usize = 2;

const DOCUMENTED_CAPABILITIES: &[&str] = &[
    "dataset.inspect.v1",
    "table.export.v1",
    "table.manipulate.v1",
    "sequence.stats.v1",
    "sequence.kmer.count.v1",
    "primer.epcr.v1",
    "fastq.qc.v1",
    "fastq.trim.v1",
    "fastq.adapter.v1",
    "fastq.deduplicate.v1",
    "alignment.qc.v1",
    "alignment.bam-to-bigwig.v1",
    "annotation.gxf.stats.v1",
    "annotation.gxf.normalize.v1",
    "annotation.gene-position.v1",
    "annotation.sequence.extract.v1",
    "annotation.structure.visualize.v1",
    "comparative.synteny.visualize.v1",
    "comparative.mcscanx.v1",
    "comparative.kaks.v1",
    "annotation.go.normalize.v1",
    "annotation.eggnog.normalize.v1",
    "enrichment.overrepresentation.v1",
    "enrichment.go.v1",
    "enrichment.kegg.v1",
    "enrichment.gsea.v1",
    "enrichment.visualize.v1",
    "genome.gene-density.v1",
    "interval.intersect.v1",
    "interval.merge.v1",
    "interval.subtract.v1",
    "interval.closest.v1",
    "expression.matrix.qc.v1",
    "expression.differential.v1",
    "medical.bulk-rnaseq.v1",
    "medical.cohort-table.qc.v1",
    "medical.pathway-ruo.v1",
    "medical.variant-cohort.v1",
    "medical.single-cell-qc.v1",
    "expression.normalize.v1",
    "expression.pca.v1",
    "expression.cluster.v1",
    "expression.heatmap.v1",
    "expression.volcano.v1",
    "motif.visualize.v1",
    "set.venn.v1",
    "set.upset.v1",
    "protein.properties.v1",
    "similarity.blast.local.v1",
    "similarity.diamond.v1",
    "similarity.hmmer.v1",
    "similarity.blast.parse.v1",
    "similarity.reciprocal.v1",
    "protein.domain.parse.v1",
    "protein.domain.visualize.v1",
    "phylogeny.tree.transform.v1",
    "phylogeny.iqtree.v1",
    "msa.muscle.v1",
    "msa.trimal.v1",
    "motif.meme.v1",
    "variant.stats.v1",
    "variant.filter.v1",
    "variant.normalize.v1",
    "structure.pdb.summary.v1",
    "structure.viewer.v1",
    "structure.mmcif.summary.v1",
    "structure.sequence.extract.v1",
    "structure.contact-map.v1",
    "structure.geometry.v1",
    "structure.superpose.v1",
    "protein.secondary-structure.v1",
    "environment.audit.v1",
    "environment.plan.v1",
    "runtime.catalog.v1",
    "system.doctor.v1",
    "system.worker.v1",
];

struct BioApp {
    language: Language,
    page: Page,
    workspace_tab: WorkspaceTab,
    user_mode: UserMode,
    project_name: String,
    project_status: String,
    import_path: String,
    import_status: String,
    datasets: Vec<DatasetEntry>,
    selected_dataset: Option<usize>,
    secondary_dataset: Option<usize>,
    tertiary_dataset: Option<usize>,
    survival_time_column: String,
    survival_event_column: String,
    survival_group_column: String,
    survival_reference_level: String,
    microbiome_database: String,
    microbiome_confidence: f64,
    project_generation: u64,
    inspection_sender: Sender<InspectionMessage>,
    inspection_receiver: Receiver<InspectionMessage>,
    inspection_queue: VecDeque<InspectionTask>,
    active_inspections: usize,
    selected_capability: String,
    annotation_feature_type: String,
    annotation_sort: bool,
    annotation_visual_feature_id: String,
    annotation_visual_seqid: String,
    annotation_visual_max_features: usize,
    go_gene_column: String,
    go_term_column: String,
    enrichment_min_overlap: u64,
    enrichment_max_terms: usize,
    enrichment_include_genes: bool,
    enrichment_visual_kind: String,
    enrichment_visual_style: String,
    gsea_score_exponent: f64,
    gsea_min_set_size: usize,
    gsea_max_set_size: usize,
    gsea_permutations: u32,
    gsea_seed: u64,
    gene_density_feature_type: String,
    gene_density_window_size: u64,
    gene_density_step_size: u64,
    kmer_size: usize,
    kmer_canonical: bool,
    epcr_max_amplicon: usize,
    fastq_deduplicate_mode: String,
    fastq_umi_delimiter: String,
    fastq_umi_length: usize,
    variant_filter_min_qual: f64,
    variant_filter_pass_only: bool,
    variant_filter_min_dp: u64,
    expression_normalization_method: String,
    expression_pseudocount: f64,
    expression_pca_components: usize,
    expression_pca_scale: bool,
    expression_sample_clusters: usize,
    expression_feature_clusters: usize,
    expression_cluster_scale: bool,
    expression_heatmap_top_features: usize,
    expression_heatmap_scale: bool,
    differential_feature_id_column: String,
    differential_sample_id_column: String,
    differential_condition_column: String,
    differential_reference_level: String,
    differential_contrast_level: String,
    differential_alpha: f64,
    differential_min_total_count: u64,
    interpret_pdb_b_factors_as_plddt: bool,
    structure_contact_cutoff: f64,
    structure_contact_atom: String,
    structure_contact_include_inter_chain: bool,
    structure_geometry_atom_count: usize,
    structure_geometry_atoms: [String; 4],
    structure_superpose_atom: String,
    protein_domain_visual_sequence_id: String,
    protein_domain_visual_max_sequences: usize,
    protein_domain_visual_max_domains: usize,
    similarity_max_evalue: f64,
    similarity_min_identity_percent: f64,
    blast_program: String,
    diamond_mode: String,
    hmmer_mode: String,
    muscle_mode: String,
    trimal_mode: String,
    iqtree_model: String,
    iqtree_seed: u64,
    meme_alphabet: String,
    meme_distribution: String,
    meme_motif_count: usize,
    meme_minimum_width: usize,
    meme_maximum_width: usize,
    native_threads: usize,
    native_evalue: f64,
    native_max_targets: usize,
    native_outfmt: u8,
    kaks_method: String,
    phylogeny_reroot_label: String,
    job_history: Vec<JobRecord>,
    analysis_job_id: Option<String>,
    export_status: String,
    analysis_status: String,
    analysis_result: Option<Value>,
    analysis_receiver: Option<Receiver<AnalysisMessage>>,
    analysis_running: bool,
    environment_status: String,
    environment_result: Option<Value>,
    environment_receiver: Option<Receiver<(EnvironmentJob, UiJobResult)>>,
    environment_running: bool,
    environment_profile: String,
    environment_mode: EnvironmentPlanMode,
    environment_project_root: String,
    document_capability: String,
    structure_viewer: StructureViewer,
    legal_document: LegalDocument,
    dependency_notices: Result<DependencyNotices, String>,
}

impl BioApp {
    fn new(context: &eframe::CreationContext<'_>, startup_paths: Vec<PathBuf>) -> Self {
        configure_style(&context.egui_ctx);
        install_cjk_font(&context.egui_ctx);
        let (inspection_sender, inspection_receiver) = mpsc::channel();
        let mut app = Self {
            language: Language::ZhCn,
            page: Page::Workspace,
            workspace_tab: WorkspaceTab::Import,
            user_mode: UserMode::Guided,
            project_name: "未命名本地项目".to_owned(),
            project_status: String::new(),
            import_path: String::new(),
            import_status: "等待导入本地数据。".to_owned(),
            datasets: Vec::new(),
            selected_dataset: None,
            secondary_dataset: None,
            tertiary_dataset: None,
            survival_time_column: "time".to_owned(),
            survival_event_column: "event".to_owned(),
            survival_group_column: "group".to_owned(),
            survival_reference_level: "control".to_owned(),
            microbiome_database: String::new(),
            microbiome_confidence: 0.5,
            project_generation: 0,
            inspection_sender,
            inspection_receiver,
            inspection_queue: VecDeque::new(),
            active_inspections: 0,
            selected_capability: "sequence.stats.v1".to_owned(),
            annotation_feature_type: "gene".to_owned(),
            annotation_sort: false,
            annotation_visual_feature_id: String::new(),
            annotation_visual_seqid: String::new(),
            annotation_visual_max_features: 100,
            go_gene_column: String::new(),
            go_term_column: String::new(),
            enrichment_min_overlap: 1,
            enrichment_max_terms: 100,
            enrichment_include_genes: false,
            enrichment_visual_kind: "go".to_owned(),
            enrichment_visual_style: "bar".to_owned(),
            gsea_score_exponent: 1.0,
            gsea_min_set_size: 15,
            gsea_max_set_size: 500,
            gsea_permutations: 1_000,
            gsea_seed: 0,
            gene_density_feature_type: "gene".to_owned(),
            gene_density_window_size: 1_000_000,
            gene_density_step_size: 1_000_000,
            kmer_size: 21,
            kmer_canonical: true,
            epcr_max_amplicon: 5_000,
            fastq_deduplicate_mode: "sequence".to_owned(),
            fastq_umi_delimiter: ":".to_owned(),
            fastq_umi_length: 8,
            variant_filter_min_qual: 20.0,
            variant_filter_pass_only: true,
            variant_filter_min_dp: 0,
            expression_normalization_method: "cpm".to_owned(),
            expression_pseudocount: 1.0,
            expression_pca_components: 2,
            expression_pca_scale: false,
            expression_sample_clusters: 2,
            expression_feature_clusters: 4,
            expression_cluster_scale: true,
            expression_heatmap_top_features: 50,
            expression_heatmap_scale: true,
            differential_feature_id_column: "feature_id".to_owned(),
            differential_sample_id_column: "sample_id".to_owned(),
            differential_condition_column: "condition".to_owned(),
            differential_reference_level: "control".to_owned(),
            differential_contrast_level: "treatment".to_owned(),
            differential_alpha: 0.05,
            differential_min_total_count: 10,
            interpret_pdb_b_factors_as_plddt: false,
            structure_contact_cutoff: 8.0,
            structure_contact_atom: "CA".to_owned(),
            structure_contact_include_inter_chain: true,
            structure_geometry_atom_count: 3,
            structure_geometry_atoms: [
                "A/1/N".to_owned(),
                "A/1/CA".to_owned(),
                "A/1/C".to_owned(),
                "A/2/N".to_owned(),
            ],
            structure_superpose_atom: "CA".to_owned(),
            protein_domain_visual_sequence_id: String::new(),
            protein_domain_visual_max_sequences: 30,
            protein_domain_visual_max_domains: 500,
            similarity_max_evalue: 1e-5,
            similarity_min_identity_percent: 30.0,
            blast_program: "blastn".to_owned(),
            diamond_mode: "blastp".to_owned(),
            hmmer_mode: "hmmsearch".to_owned(),
            muscle_mode: "align".to_owned(),
            trimal_mode: "automated1".to_owned(),
            iqtree_model: "MFP".to_owned(),
            iqtree_seed: 1,
            meme_alphabet: "dna".to_owned(),
            meme_distribution: "zoops".to_owned(),
            meme_motif_count: 3,
            meme_minimum_width: 6,
            meme_maximum_width: 15,
            native_threads: 4,
            native_evalue: 1e-3,
            native_max_targets: 50,
            native_outfmt: 6,
            kaks_method: "NG".to_owned(),
            phylogeny_reroot_label: String::new(),
            job_history: Vec::new(),
            analysis_job_id: None,
            export_status: String::new(),
            analysis_status: "已准备好进行本地分析。".to_owned(),
            analysis_result: None,
            analysis_receiver: None,
            analysis_running: false,
            environment_status: "正在审计本地环境...".to_owned(),
            environment_result: None,
            environment_receiver: None,
            environment_running: false,
            environment_profile: "full-local".to_owned(),
            environment_mode: EnvironmentPlanMode::ManagedUser,
            environment_project_root: String::new(),
            document_capability: "sequence.stats.v1".to_owned(),
            structure_viewer: StructureViewer::default(),
            legal_document: LegalDocument::ProjectLicense,
            dependency_notices: load_packaged_dependency_notices(),
        };
        app.queue_paths(startup_paths);
        app.start_environment_job(EnvironmentJob::Audit);
        app
    }

    fn text(&self, zh_cn: &'static str, en_us: &'static str) -> &'static str {
        self.language.text(zh_cn, en_us)
    }

    fn selected_dataset(&self) -> Option<&DatasetEntry> {
        self.selected_dataset
            .and_then(|index| self.datasets.get(index))
    }

    fn project_work_active(&self) -> bool {
        self.analysis_running || self.active_inspections > 0 || !self.inspection_queue.is_empty()
    }

    fn queue_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for path in paths {
            self.queue_path(path);
        }
    }

    fn queue_path(&mut self, path: PathBuf) -> bool {
        let path = match importable_file_path(path) {
            Ok(path) => path,
            Err((path, issue)) => {
                self.import_status = import_path_error(&path, issue, self.language);
                return false;
            }
        };

        let normalized = path
            .to_str()
            .expect("importable_file_path rejects non-UTF-8 paths")
            .to_owned();
        if let Some(index) = self
            .datasets
            .iter()
            .position(|dataset| dataset.path == normalized)
        {
            self.selected_dataset = Some(index);
            self.page = Page::Workspace;
            self.workspace_tab = WorkspaceTab::Dataset;
            self.import_status = self
                .text(
                    "该文件已在当前项目中。",
                    "The file is already in this project.",
                )
                .to_owned();
            return true;
        }

        let dataset_id = new_dataset_id(self.datasets.len());
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dataset")
            .to_owned();
        let format_hint = format_hint(&path).to_owned();
        let index = self.datasets.len();
        self.datasets.push(DatasetEntry {
            id: dataset_id.clone(),
            name,
            path: normalized.clone(),
            format_hint,
            state: DatasetState::Inspecting,
            inspection: None,
            message: self
                .text(
                    "正在识别格式并快速校验...",
                    "Detecting format and validating...",
                )
                .to_owned(),
        });
        self.selected_dataset = Some(index);
        self.page = Page::Workspace;
        self.workspace_tab = WorkspaceTab::Dataset;
        self.import_status = self
            .text("文件已加入导入队列。", "File added to the import queue.")
            .to_owned();

        self.inspection_queue.push_back(InspectionTask {
            generation: self.project_generation,
            dataset_id,
            path: normalized,
        });
        self.pump_inspection_queue();
        true
    }

    fn pump_inspection_queue(&mut self) {
        while self.active_inspections < MAX_CONCURRENT_INSPECTIONS {
            let Some(task) = self.inspection_queue.pop_front() else {
                break;
            };
            let fallback_task = task.clone();
            let sender = self.inspection_sender.clone();
            let thread_sender = sender.clone();
            self.active_inspections += 1;
            let spawn_result = thread::Builder::new()
                .name("linxira-dataset-inspection".to_owned())
                .spawn(move || {
                    let _ = thread_sender.send(run_inspection_task(task));
                });
            if let Err(error) = spawn_result {
                let _ = sender.send(InspectionMessage {
                    generation: fallback_task.generation,
                    dataset_id: fallback_task.dataset_id,
                    result: Err(format!("failed to start inspection worker: {error}")),
                });
            }
        }
    }

    fn poll_inspection_jobs(&mut self) {
        loop {
            let message = match self.inspection_receiver.try_recv() {
                Ok(message) => message,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            };
            self.active_inspections = self.active_inspections.saturating_sub(1);
            if !generation_matches(message.generation, self.project_generation) {
                continue;
            }
            let Some(dataset) = self
                .datasets
                .iter_mut()
                .find(|dataset| dataset.id == message.dataset_id)
            else {
                continue;
            };
            match message.result {
                Ok(json) => match serde_json::from_str::<Value>(&json) {
                    Ok(result) => {
                        dataset.state = inspection_state(&result);
                        dataset.message = first_diagnostic_message(&result).unwrap_or_else(|| {
                            match dataset.state {
                                DatasetState::Ready => "Inspection completed".to_owned(),
                                DatasetState::Warning => {
                                    "Inspection completed with warnings".to_owned()
                                }
                                DatasetState::Invalid => "Validation failed".to_owned(),
                                DatasetState::Inspecting => String::new(),
                            }
                        });
                        dataset.inspection = Some(result);
                    }
                    Err(error) => {
                        dataset.state = DatasetState::Invalid;
                        dataset.message = format!("Invalid worker JSON: {error}");
                    }
                },
                Err(error) => {
                    dataset.state = DatasetState::Invalid;
                    dataset.message = error;
                }
            }
        }
        self.pump_inspection_queue();
    }

    fn remove_selected_dataset(&mut self) {
        if self.project_work_active() {
            return;
        }
        let Some(index) = self.selected_dataset else {
            return;
        };
        if index < self.datasets.len() {
            self.datasets.remove(index);
        }
        self.selected_dataset = if self.datasets.is_empty() {
            None
        } else {
            Some(index.min(self.datasets.len() - 1))
        };
        self.workspace_tab = if self.datasets.is_empty() {
            WorkspaceTab::Import
        } else {
            WorkspaceTab::Dataset
        };
    }

    fn save_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.text("保存项目", "Save project"))
            .set_file_name("linxira-bio-project.json")
            .add_filter("Linxira Bio project", &["json"])
            .save_file()
        else {
            return;
        };
        let project = ProjectFile {
            schema_version: "1".to_owned(),
            name: self.project_name.clone(),
            files: self
                .datasets
                .iter()
                .map(|dataset| dataset.path.clone())
                .collect(),
        };
        self.project_status = match serde_json::to_vec_pretty(&project)
            .map_err(|error| error.to_string())
            .and_then(|content| fs::write(&path, content).map_err(|error| error.to_string()))
        {
            Ok(()) => match self.language {
                Language::ZhCn => format!("项目已保存到 {}", path.display()),
                Language::EnUs => format!("Project saved to {}", path.display()),
            },
            Err(error) => match self.language {
                Language::ZhCn => format!("保存项目失败：{error}"),
                Language::EnUs => format!("Failed to save project: {error}"),
            },
        };
    }

    fn open_project(&mut self) {
        if self.project_work_active() {
            self.project_status = self
                .text(
                    "请等待当前导入或分析任务完成后再打开项目。",
                    "Wait for the current import or analysis job before opening a project.",
                )
                .to_owned();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.text("打开项目", "Open project"))
            .add_filter("Linxira Bio project", &["json"])
            .pick_file()
        else {
            return;
        };
        let project = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|content| {
                serde_json::from_slice::<ProjectFile>(&content).map_err(|error| error.to_string())
            });
        match project {
            Ok(project) if project.schema_version == "1" => {
                self.project_generation = self.project_generation.wrapping_add(1);
                self.inspection_queue.clear();
                self.project_name = project.name;
                self.datasets.clear();
                self.selected_dataset = None;
                self.secondary_dataset = None;
                self.tertiary_dataset = None;
                self.analysis_job_id = None;
                self.analysis_receiver = None;
                self.analysis_running = false;
                self.analysis_result = None;
                self.analysis_status = self
                    .text("已准备好进行本地分析。", "Ready for local analysis.")
                    .to_owned();
                self.job_history.clear();
                self.queue_paths(project.files.into_iter().map(PathBuf::from));
                self.project_status = match self.language {
                    Language::ZhCn => format!("已打开项目 {}", path.display()),
                    Language::EnUs => format!("Opened project {}", path.display()),
                };
                self.page = Page::Workspace;
            }
            Ok(project) => {
                self.project_status = match self.language {
                    Language::ZhCn => {
                        format!("不支持的项目版本：{}", project.schema_version)
                    }
                    Language::EnUs => {
                        format!("Unsupported project version: {}", project.schema_version)
                    }
                };
            }
            Err(error) => {
                self.project_status = match self.language {
                    Language::ZhCn => format!("打开项目失败：{error}"),
                    Language::EnUs => format!("Failed to open project: {error}"),
                };
            }
        }
    }

    fn start_selected_analysis(&mut self) {
        let Some((dataset_id, dataset_path, dataset_name, format, runnable)) =
            self.selected_dataset().map(|dataset| {
                (
                    dataset.id.clone(),
                    dataset.path.clone(),
                    dataset.name.clone(),
                    dataset_detected_format(dataset).to_owned(),
                    dataset
                        .inspection
                        .as_ref()
                        .is_some_and(inspection_is_runnable)
                        && !matches!(
                            dataset.state,
                            DatasetState::Inspecting | DatasetState::Invalid
                        ),
                )
            })
        else {
            return;
        };
        let Some(route) = analysis_route_for_capability(&self.selected_capability, &format) else {
            return;
        };
        if self.analysis_running || !runnable {
            return;
        }

        let job_id = new_job_id();
        let mut request = build_analysis_request(&job_id, route, &dataset_path);
        let mut dataset_name = dataset_name;
        if capability_requires_secondary(route.capability) {
            let Some(secondary_index) = self.secondary_dataset else {
                return;
            };
            let Some(secondary) = self.datasets.get(secondary_index) else {
                return;
            };
            let secondary_runnable =
                secondary_input_matches(route.capability, dataset_detected_format(secondary))
                    && secondary
                        .inspection
                        .as_ref()
                        .is_some_and(inspection_is_runnable)
                    && !matches!(
                        secondary.state,
                        DatasetState::Inspecting | DatasetState::Invalid
                    )
                    && secondary.path != dataset_path;
            if !secondary_runnable {
                return;
            }
            let Some(role) = secondary_input_role(route.capability) else {
                return;
            };
            request
                .inputs
                .insert(role.to_owned(), secondary.path.clone());
            dataset_name = format!("{dataset_name} + {}", secondary.name);
        }
        if let Some(tertiary_role) = tertiary_input_role(route.capability) {
            let Some(tertiary_index) = self.tertiary_dataset else {
                return;
            };
            let Some(tertiary) = self.datasets.get(tertiary_index) else {
                return;
            };
            let tertiary_runnable =
                tertiary_input_format(route.capability).is_some_and(|required| {
                    dataset_detected_format(tertiary).eq_ignore_ascii_case(required)
                });
            if !tertiary_runnable || tertiary.path == dataset_path {
                return;
            }
            request
                .inputs
                .insert(tertiary_role.to_owned(), tertiary.path.clone());
            dataset_name = format!("{dataset_name} + {}", tertiary.name);
        }
        if let Some(extension) = capability_output_extension(route.capability) {
            let output = derived_analysis_output_path(&dataset_path, route.capability, extension);
            let mut parameters = serde_json::Map::new();
            parameters.insert(
                "output".to_owned(),
                Value::String(output.to_string_lossy().into_owned()),
            );
            if route.capability == "annotation.gxf.normalize.v1" {
                parameters.insert("sort".to_owned(), Value::Bool(self.annotation_sort));
            }
            if route.capability == "annotation.sequence.extract.v1" {
                parameters.insert(
                    "feature_type".to_owned(),
                    Value::String(self.annotation_feature_type.clone()),
                );
            }
            if route.capability == "annotation.structure.visualize.v1" {
                if !self.annotation_visual_feature_id.trim().is_empty() {
                    parameters.insert(
                        "feature_id".to_owned(),
                        Value::String(self.annotation_visual_feature_id.trim().to_owned()),
                    );
                } else if !self.annotation_visual_seqid.trim().is_empty() {
                    parameters.insert(
                        "seqid".to_owned(),
                        Value::String(self.annotation_visual_seqid.trim().to_owned()),
                    );
                }
                parameters.insert(
                    "max_features".to_owned(),
                    serde_json::json!(self.annotation_visual_max_features),
                );
            }
            if route.capability == "annotation.go.normalize.v1" {
                if !self.go_gene_column.trim().is_empty() {
                    parameters.insert(
                        "gene_column".to_owned(),
                        Value::String(self.go_gene_column.trim().to_owned()),
                    );
                }
                if !self.go_term_column.trim().is_empty() {
                    parameters.insert(
                        "go_column".to_owned(),
                        Value::String(self.go_term_column.trim().to_owned()),
                    );
                }
            }
            if route.capability == "phylogeny.tree.transform.v1"
                && !self.phylogeny_reroot_label.trim().is_empty()
            {
                parameters.insert(
                    "reroot_label".to_owned(),
                    Value::String(self.phylogeny_reroot_label.trim().to_owned()),
                );
            }
            if route.capability == "sequence.kmer.count.v1" {
                parameters.insert("k".to_owned(), serde_json::json!(self.kmer_size));
                parameters.insert("canonical".to_owned(), Value::Bool(self.kmer_canonical));
                parameters.insert("top_n".to_owned(), serde_json::json!(50));
            }
            if route.capability == "primer.epcr.v1" {
                parameters.insert(
                    "max_amplicon".to_owned(),
                    serde_json::json!(self.epcr_max_amplicon),
                );
            }
            if route.capability == "variant.filter.v1" {
                parameters.insert(
                    "min_qual".to_owned(),
                    serde_json::json!(self.variant_filter_min_qual),
                );
                parameters.insert(
                    "require_pass".to_owned(),
                    Value::Bool(self.variant_filter_pass_only),
                );
                if self.variant_filter_min_dp > 0 {
                    parameters.insert(
                        "min_info_dp".to_owned(),
                        serde_json::json!(self.variant_filter_min_dp),
                    );
                }
            }
            if route.capability == "fastq.deduplicate.v1" {
                match self.fastq_deduplicate_mode.as_str() {
                    "header-umi" => {
                        parameters.insert(
                            "header_umi_delimiter".to_owned(),
                            Value::String(self.fastq_umi_delimiter.clone()),
                        );
                    }
                    "sequence-prefix-umi" => {
                        parameters.insert(
                            "sequence_prefix_umi".to_owned(),
                            serde_json::json!(self.fastq_umi_length),
                        );
                    }
                    _ => {}
                }
            }
            if route.capability == "expression.normalize.v1" {
                parameters.insert(
                    "method".to_owned(),
                    Value::String(self.expression_normalization_method.clone()),
                );
                parameters.insert(
                    "pseudocount".to_owned(),
                    serde_json::json!(self.expression_pseudocount),
                );
            }
            if route.capability == "comparative.kaks.v1" {
                parameters.insert("method".to_owned(), Value::String(self.kaks_method.clone()));
            }
            if route.capability == "protein.domain.visualize.v1" {
                if !self.protein_domain_visual_sequence_id.trim().is_empty() {
                    parameters.insert(
                        "sequence_id".to_owned(),
                        Value::String(self.protein_domain_visual_sequence_id.trim().to_owned()),
                    );
                }
                parameters.insert(
                    "max_sequences".to_owned(),
                    serde_json::json!(self.protein_domain_visual_max_sequences),
                );
                parameters.insert(
                    "max_domains".to_owned(),
                    serde_json::json!(self.protein_domain_visual_max_domains),
                );
            }
            if route.capability == "enrichment.visualize.v1" {
                parameters.insert(
                    "kind".to_owned(),
                    Value::String(self.enrichment_visual_kind.clone()),
                );
                parameters.insert(
                    "style".to_owned(),
                    Value::String(self.enrichment_visual_style.clone()),
                );
                parameters.insert(
                    "min_overlap".to_owned(),
                    serde_json::json!(self.enrichment_min_overlap),
                );
                parameters.insert(
                    "max_terms".to_owned(),
                    serde_json::json!(self.enrichment_max_terms),
                );
            }
            if route.capability == "similarity.blast.local.v1" {
                parameters.insert(
                    "program".to_owned(),
                    Value::String(self.blast_program.clone()),
                );
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
                parameters.insert("evalue".to_owned(), serde_json::json!(self.native_evalue));
                parameters.insert(
                    "max_target_sequences".to_owned(),
                    serde_json::json!(self.native_max_targets),
                );
                parameters.insert("outfmt".to_owned(), serde_json::json!(self.native_outfmt));
            }
            if route.capability == "similarity.diamond.v1" {
                parameters.insert("mode".to_owned(), Value::String(self.diamond_mode.clone()));
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
                parameters.insert("evalue".to_owned(), serde_json::json!(self.native_evalue));
                parameters.insert(
                    "max_target_sequences".to_owned(),
                    serde_json::json!(self.native_max_targets),
                );
                parameters.insert("outfmt".to_owned(), serde_json::json!(self.native_outfmt));
            }
            if route.capability == "similarity.hmmer.v1" {
                parameters.insert("mode".to_owned(), Value::String(self.hmmer_mode.clone()));
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
                parameters.insert("evalue".to_owned(), serde_json::json!(self.native_evalue));
            }
            if route.capability == "msa.muscle.v1" {
                parameters.insert("mode".to_owned(), Value::String(self.muscle_mode.clone()));
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
            }
            if route.capability == "msa.trimal.v1" {
                parameters.insert("mode".to_owned(), Value::String(self.trimal_mode.clone()));
            }
            if route.capability == "phylogeny.iqtree.v1" {
                parameters.insert("model".to_owned(), Value::String(self.iqtree_model.clone()));
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
                parameters.insert("seed".to_owned(), serde_json::json!(self.iqtree_seed));
            }
            if route.capability == "motif.meme.v1" {
                parameters.insert(
                    "alphabet".to_owned(),
                    Value::String(self.meme_alphabet.clone()),
                );
                parameters.insert(
                    "distribution".to_owned(),
                    Value::String(self.meme_distribution.clone()),
                );
                parameters.insert(
                    "motif_count".to_owned(),
                    serde_json::json!(self.meme_motif_count),
                );
                parameters.insert(
                    "minimum_width".to_owned(),
                    serde_json::json!(self.meme_minimum_width),
                );
                parameters.insert(
                    "maximum_width".to_owned(),
                    serde_json::json!(self.meme_maximum_width),
                );
                parameters.insert("threads".to_owned(), serde_json::json!(self.native_threads));
            }
            request.parameters = Value::Object(parameters);
        }
        if route.capability == "expression.pca.v1" {
            request.parameters = serde_json::json!({
                "components": self.expression_pca_components,
                "scale_features": self.expression_pca_scale,
            });
        }
        if route.capability == "expression.cluster.v1" {
            request.parameters = serde_json::json!({
                "sample_clusters": self.expression_sample_clusters,
                "feature_clusters": self.expression_feature_clusters,
                "max_iterations": 100,
                "scale_features": self.expression_cluster_scale,
            });
        }
        if route.capability == "expression.heatmap.v1" {
            request.parameters = serde_json::json!({
                "top_variable_features": self.expression_heatmap_top_features,
                "scale_rows": self.expression_heatmap_scale,
            });
        }
        if matches!(
            route.capability,
            "expression.differential.v1" | "medical.bulk-rnaseq.v1"
        ) {
            let output_directory =
                derived_analysis_output_path(&dataset_path, route.capability, "results");
            request.parameters = serde_json::json!({
                "output_directory": output_directory.to_string_lossy(),
                "feature_id_column": self.differential_feature_id_column.trim(),
                "sample_id_column": self.differential_sample_id_column.trim(),
                "condition_column": self.differential_condition_column.trim(),
                "reference_level": self.differential_reference_level.trim(),
                "contrast_level": self.differential_contrast_level.trim(),
                "alpha": self.differential_alpha,
                "min_total_count": self.differential_min_total_count,
            });
        }
        if route.capability == "medical.survival.v1" {
            let output_directory =
                derived_analysis_output_path(&dataset_path, route.capability, "results");
            request.parameters = serde_json::json!({
                "output_directory": output_directory.to_string_lossy(),
                "time_column": self.survival_time_column.trim(),
                "event_column": self.survival_event_column.trim(),
                "group_column": self.survival_group_column.trim(),
                "reference_level": self.survival_reference_level.trim(),
            });
        }
        if route.capability == "chemistry.descriptors.v1" {
            let output_directory =
                derived_analysis_output_path(&dataset_path, route.capability, "results");
            request.parameters = serde_json::json!({
                "output_directory": output_directory.to_string_lossy(),
                "output_filename": "descriptors.tsv",
            });
        }
        if route.capability == "medical.microbiome.v1"
            || route.capability == "metagenomics.classify.v1"
        {
            let output = derived_analysis_output_path(&dataset_path, route.capability, "tsv");
            request.parameters = serde_json::json!({
                "output": output.to_string_lossy(),
                "database": self.microbiome_database.trim(),
                "confidence": self.microbiome_confidence,
                "threads": self.native_threads,
            });
        }
        if route.capability == "enrichment.gsea.v1" {
            request.parameters = serde_json::json!({
                "score_exponent": self.gsea_score_exponent,
                "min_set_size": self.gsea_min_set_size,
                "max_set_size": self.gsea_max_set_size,
                "permutations": self.gsea_permutations,
                "seed": self.gsea_seed,
            });
        }
        if matches!(route.capability, "set.venn.v1" | "set.upset.v1") {
            request.parameters = serde_json::json!({
                "include_items": false,
                "max_intersections": 50,
            });
        }
        if matches!(
            route.capability,
            "enrichment.overrepresentation.v1"
                | "enrichment.go.v1"
                | "enrichment.kegg.v1"
                | "medical.pathway-ruo.v1"
        ) {
            request.parameters = serde_json::json!({
                "min_overlap": self.enrichment_min_overlap,
                "max_terms": self.enrichment_max_terms,
                "include_genes": self.enrichment_include_genes,
            });
        }
        if route.capability == "genome.gene-density.v1" {
            request.parameters = serde_json::json!({
                "feature_types": [self.gene_density_feature_type.trim()],
                "window_size": self.gene_density_window_size,
                "step_size": self.gene_density_step_size,
            });
        }
        if route.capability == "similarity.reciprocal.v1" {
            request.parameters = serde_json::json!({
                "max_evalue": self.similarity_max_evalue,
                "min_identity_percent": self.similarity_min_identity_percent,
            });
        }
        if route.capability == "structure.pdb.summary.v1" {
            request.parameters = serde_json::json!({
                "interpret_b_factors_as_plddt": self.interpret_pdb_b_factors_as_plddt,
            });
        }
        if route.capability == "structure.contact-map.v1" {
            request.parameters = serde_json::json!({
                "cutoff_angstrom": self.structure_contact_cutoff,
                "atom_name": self.structure_contact_atom.trim(),
                "include_inter_chain": self.structure_contact_include_inter_chain,
            });
        }
        if route.capability == "structure.geometry.v1" {
            request.parameters = serde_json::json!({
                "atoms": self.structure_geometry_atoms
                    .iter()
                    .take(self.structure_geometry_atom_count)
                    .map(|selector| selector.trim())
                    .collect::<Vec<_>>(),
            });
        }
        if route.capability == "structure.superpose.v1" {
            request.parameters = serde_json::json!({
                "atom_name": self.structure_superpose_atom.trim(),
            });
        }
        let capability = route.capability.to_owned();
        let generation = self.project_generation;
        let (sender, receiver) = mpsc::channel();
        self.analysis_job_id = Some(job_id.clone());
        self.analysis_receiver = Some(receiver);
        self.analysis_running = true;
        self.analysis_result = None;
        self.analysis_status = match self.language {
            Language::ZhCn => format!("正在本地运行 {}...", route.capability),
            Language::EnUs => format!("Running {} locally...", route.capability),
        };
        self.job_history.push(JobRecord {
            id: job_id.clone(),
            capability: capability.clone(),
            dataset_name,
            state: JobState::Running,
            message: self.text("本地 CPU", "Local CPU").to_owned(),
        });
        self.workspace_tab = WorkspaceTab::Results;

        let fallback_sender = sender.clone();
        let fallback_job_id = job_id.clone();
        let fallback_dataset_id = dataset_id.clone();
        let fallback_capability = capability.clone();
        let spawn_result = thread::Builder::new()
            .name("linxira-analysis".to_owned())
            .spawn(move || {
                let result = run_worker_request(request);
                let _ = sender.send(AnalysisMessage {
                    generation,
                    job_id,
                    dataset_id,
                    capability,
                    result,
                });
            });
        if let Err(error) = spawn_result {
            let _ = fallback_sender.send(AnalysisMessage {
                generation,
                job_id: fallback_job_id,
                dataset_id: fallback_dataset_id,
                capability: fallback_capability,
                result: Err(format!("failed to start analysis worker: {error}")),
            });
        }
    }

    fn start_environment_job(&mut self, kind: EnvironmentJob) {
        if self.environment_running {
            return;
        }

        let profile = self.environment_profile.clone();
        let mode = self.environment_mode;
        let project_root = self.environment_project_root.trim().to_owned();
        let (capability, parameters) = match kind {
            EnvironmentJob::Audit => ("environment.audit.v1", serde_json::json!({})),
            EnvironmentJob::Plan => {
                let mut parameters = serde_json::json!({
                    "profile": profile,
                    "mode": mode.id(),
                });
                if mode == EnvironmentPlanMode::ProjectIsolated {
                    parameters["project_root"] = Value::String(project_root);
                }
                ("environment.plan.v1", parameters)
            }
        };
        let (sender, receiver) = mpsc::channel();
        self.environment_receiver = Some(receiver);
        self.environment_running = true;
        self.environment_result = None;
        self.environment_status = match kind {
            EnvironmentJob::Audit => {
                self.text("正在审计本地环境...", "Auditing the local environment...")
            }
            EnvironmentJob::Plan => {
                self.text("正在生成事务预览...", "Building a transaction preview...")
            }
        }
        .to_owned();

        thread::spawn(move || {
            let request = JobRequest {
                schema_version: SCHEMA_VERSION.to_owned(),
                job_id: new_job_id(),
                capability: capability.to_owned(),
                inputs: BTreeMap::new(),
                execution: ExecutionRequest {
                    mode: ExecutionMode::LocalCpu,
                },
                parameters,
            };
            let result =
                execute_request(request, Path::new(".")).map_err(|error| error.to_string());
            let _ = sender.send((kind, result));
        });
    }

    fn poll_analysis_job(&mut self) {
        let message = match self.analysis_receiver.as_ref().map(Receiver::try_recv) {
            Some(Ok(message)) => message,
            Some(Err(TryRecvError::Empty)) | None => return,
            Some(Err(TryRecvError::Disconnected)) => {
                self.analysis_receiver = None;
                self.analysis_running = false;
                self.analysis_result = None;
                self.analysis_status = self
                    .text(
                        "分析后台通道意外关闭。",
                        "The analysis background channel closed unexpectedly.",
                    )
                    .to_owned();
                self.finish_analysis_record(JobState::Failed);
                return;
            }
        };

        self.analysis_receiver = None;
        self.analysis_running = false;
        if !generation_matches(message.generation, self.project_generation) {
            return;
        }
        let context_matches = self.analysis_job_id.as_deref() == Some(message.job_id.as_str())
            && self
                .datasets
                .iter()
                .any(|dataset| dataset.id == message.dataset_id);
        if !context_matches {
            self.analysis_result = None;
            self.analysis_status = self
                .text(
                    "分析结果与当前项目不匹配，已丢弃。",
                    "The analysis result did not match the current project and was discarded.",
                )
                .to_owned();
            self.finish_analysis_record(JobState::Failed);
            return;
        }

        match message.result {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(result)
                    if analysis_result_matches(&result, &message.job_id, &message.capability) =>
                {
                    self.analysis_result = Some(result);
                    self.analysis_status =
                        self.text("分析已完成。", "Analysis completed.").to_owned();
                }
                Ok(_) => {
                    self.analysis_result = None;
                    self.analysis_status = self
                        .text(
                            "Worker 结果的任务标识不匹配。",
                            "The worker result identifiers did not match the request.",
                        )
                        .to_owned();
                }
                Err(error) => {
                    self.analysis_status = match self.language {
                        Language::ZhCn => format!("Worker 返回了无效 JSON：{error}"),
                        Language::EnUs => format!("Worker returned invalid JSON: {error}"),
                    };
                }
            },
            Err(error) => {
                self.analysis_status = match self.language {
                    Language::ZhCn => format!("分析失败：{error}"),
                    Language::EnUs => format!("Analysis failed: {error}"),
                };
            }
        }
        let state = if self.analysis_result.is_some() {
            JobState::Completed
        } else {
            JobState::Failed
        };
        self.finish_analysis_record(state);
    }

    fn finish_analysis_record(&mut self, state: JobState) {
        if let Some(job_id) = self.analysis_job_id.take()
            && let Some(job) = self.job_history.iter_mut().find(|job| job.id == job_id)
        {
            job.state = state;
            job.message = self.analysis_status.clone();
        }
    }

    fn poll_environment_job(&mut self) {
        let message = match self.environment_receiver.as_ref().map(Receiver::try_recv) {
            Some(Ok(message)) => message,
            Some(Err(TryRecvError::Empty)) | None => return,
            Some(Err(TryRecvError::Disconnected)) => {
                self.environment_receiver = None;
                self.environment_running = false;
                self.environment_result = None;
                self.environment_status = self
                    .text(
                        "环境后台通道意外关闭。",
                        "The environment background channel closed unexpectedly.",
                    )
                    .to_owned();
                return;
            }
        };
        let (kind, message) = message;

        self.environment_receiver = None;
        self.environment_running = false;
        match message {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(result) => {
                    self.environment_result = Some(result);
                    self.environment_status = match kind {
                        EnvironmentJob::Audit => {
                            self.text("环境审计已完成。", "Environment audit completed.")
                        }
                        EnvironmentJob::Plan => self.text(
                            "事务预览已生成，未对系统进行任何更改。",
                            "Transaction preview completed. No changes applied.",
                        ),
                    }
                    .to_owned();
                }
                Err(error) => {
                    self.environment_status = match self.language {
                        Language::ZhCn => format!("Worker 返回了无效 JSON：{error}"),
                        Language::EnUs => format!("Worker returned invalid JSON: {error}"),
                    };
                }
            },
            Err(error) => {
                self.environment_status = match self.language {
                    Language::ZhCn => format!("环境操作失败：{error}"),
                    Language::EnUs => format!("Environment operation failed: {error}"),
                };
            }
        }
    }

    fn show_navigation(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        ui.label(
            egui::RichText::new("LINXIRA BIO")
                .strong()
                .size(18.0)
                .color(theme::ACCENT),
        );
        ui.label(
            egui::RichText::new(self.text("本地分析工作台", "Local analysis workbench"))
                .size(theme::SMALL_SIZE)
                .color(theme::TEXT_MUTED),
        );
        ui.add_space(14.0);

        nav_button(
            ui,
            &mut self.page,
            Page::Workspace,
            language.text("数据工作台", "Data workbench"),
        );
        nav_button(
            ui,
            &mut self.page,
            Page::Structure,
            language.text("结构查看器", "Structure viewer"),
        );
        nav_button(
            ui,
            &mut self.page,
            Page::Environment,
            language.text("运行环境", "Environment"),
        );
        nav_button(
            ui,
            &mut self.page,
            Page::Documentation,
            language.text("离线文档", "Offline docs"),
        );
        nav_button(
            ui,
            &mut self.page,
            Page::Licenses,
            language.text("许可证", "Licenses"),
        );

        ui.add_space(18.0);
        ui.weak(self.text("项目数据", "PROJECT DATA"));
        ui.add_space(4.0);
        if self.datasets.is_empty() {
            egui::Frame::NONE
                .fill(theme::FAINT_BG)
                .corner_radius(theme::CORNER_RADIUS)
                .inner_margin(egui::Margin::symmetric(8, 10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(self.text("尚未导入数据", "No datasets imported"))
                            .size(theme::SMALL_SIZE)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(self.text(
                            "拖放文件到主区域，或点击下方导入。",
                            "Drop files in the main area, or use Import below.",
                        ))
                        .size(theme::SMALL_SIZE)
                        .color(theme::TEXT_MUTED),
                    );
                });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("navigation-datasets")
                .max_height(280.0)
                .show(ui, |ui| {
                    for index in 0..self.datasets.len() {
                        let dataset = &self.datasets[index];
                        let selected = self.selected_dataset == Some(index);
                        let label = egui::RichText::new(&dataset.name).size(theme::BODY_SIZE);
                        let response = ui.add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::new(label).selected(selected),
                        );
                        if response.clicked() {
                            self.selected_dataset = Some(index);
                            self.page = Page::Workspace;
                            self.workspace_tab = WorkspaceTab::Dataset;
                        }
                        response.on_hover_text(&dataset.path);
                    }
                });
        }
        if ui
            .add_sized(
                [ui.available_width(), 32.0],
                egui::Button::new(self.text("＋ 导入数据", "+ Import data")),
            )
            .clicked()
        {
            self.page = Page::Workspace;
            self.workspace_tab = WorkspaceTab::Import;
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("AGPL-3.0-or-later");
            ui.small(self.language.text(
                "Windows 优先 | Debian | Arch",
                "Windows first | Debian | Arch",
            ));
        });
    }

    fn show_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(&self.project_name).strong().size(16.0));
            ui.weak(self.text("本地项目", "Local project"));
            if ui
                .add_enabled(
                    !self.project_work_active(),
                    egui::Button::new(self.text("打开", "Open")),
                )
                .on_hover_text(self.text(
                    "导入和分析任务结束后可打开其他项目。",
                    "Open another project after import and analysis jobs finish.",
                ))
                .clicked()
            {
                self.open_project();
            }
            if ui.button(self.text("保存", "Save")).clicked() {
                self.save_project();
            }
            ui.separator();
            let environment_color = if self.environment_running {
                theme::INFO
            } else if self.environment_result.is_some() {
                theme::ACCENT_STRONG
            } else {
                theme::WARNING
            };
            ui.colored_label(
                environment_color,
                self.text("本地环境", "Local environment"),
            );
            ui.separator();
            ui.selectable_value(
                &mut self.user_mode,
                UserMode::Guided,
                self.language.text("引导", "Guided"),
            );
            ui.selectable_value(
                &mut self.user_mode,
                UserMode::Expert,
                self.language.text("专家", "Expert"),
            );
            ui.separator();
            egui::ComboBox::from_id_salt("interface-language")
                .selected_text(match self.language {
                    Language::ZhCn => "简体中文",
                    Language::EnUs => "English",
                })
                .width(92.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.language, Language::ZhCn, "简体中文");
                    ui.selectable_value(&mut self.language, Language::EnUs, "English");
                });
        });
        if !self.project_status.is_empty() {
            ui.small(&self.project_status);
        }
    }

    /// Bottom status bar: dataset/job counts on the left, environment state
    /// and build version on the right. One coherent strip below every page.
    fn show_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(self.text("状态", "Status")).size(theme::SMALL_SIZE));
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    self.text("数据集", "Datasets"),
                    self.datasets.len()
                ))
                .size(theme::SMALL_SIZE),
            );
            let running = self
                .job_history
                .iter()
                .filter(|job| job.state == JobState::Running)
                .count();
            let completed = self
                .job_history
                .iter()
                .filter(|job| job.state == JobState::Completed)
                .count();
            let failed = self
                .job_history
                .iter()
                .filter(|job| job.state == JobState::Failed)
                .count();
            if running > 0 {
                ui.separator();
                ui.colored_label(
                    theme::INFO,
                    egui::RichText::new(format!("{} {}", running, self.text("运行中", "running")))
                        .size(theme::SMALL_SIZE),
                );
            }
            if completed > 0 {
                ui.separator();
                ui.colored_label(
                    theme::ACCENT_STRONG,
                    egui::RichText::new(format!("{} {}", completed, self.text("已完成", "done")))
                        .size(theme::SMALL_SIZE),
                );
            }
            if failed > 0 {
                ui.separator();
                ui.colored_label(
                    theme::DANGER,
                    egui::RichText::new(format!("{} {}", failed, self.text("失败", "failed")))
                        .size(theme::SMALL_SIZE),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .weak()
                        .size(theme::SMALL_SIZE),
                );
                ui.separator();
                let (environment_color, environment_text) = if self.environment_running {
                    (
                        theme::INFO,
                        self.text("环境检查中…", "Checking environment…"),
                    )
                } else if self.environment_result.is_some() {
                    (
                        theme::ACCENT_STRONG,
                        self.text("环境已检查", "Environment checked"),
                    )
                } else {
                    (
                        theme::WARNING,
                        self.text("环境待检查", "Environment pending"),
                    )
                };
                ui.colored_label(
                    environment_color,
                    egui::RichText::new(environment_text).size(theme::SMALL_SIZE),
                );
            });
        });
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        ui.columns(4, |columns| {
            workspace_tab_button(
                &mut columns[0],
                &mut self.workspace_tab,
                WorkspaceTab::Import,
                language.text("1  导入", "1  Import"),
            );
            workspace_tab_button(
                &mut columns[1],
                &mut self.workspace_tab,
                WorkspaceTab::Dataset,
                language.text("2  数据检查", "2  Inspect"),
            );
            workspace_tab_button(
                &mut columns[2],
                &mut self.workspace_tab,
                WorkspaceTab::Analysis,
                language.text("3  分析", "3  Analyze"),
            );
            workspace_tab_button(
                &mut columns[3],
                &mut self.workspace_tab,
                WorkspaceTab::Results,
                language.text("4  结果", "4  Results"),
            );
        });
        ui.separator();

        let show_context = ui.available_width() >= 900.0;
        if show_context {
            ui.horizontal_top(|ui| {
                let context_width = theme::CONTEXT_WIDTH;
                let content_width =
                    (ui.available_width() - context_width - theme::CONTEXT_GAP).max(420.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| self.show_workspace_tab(ui),
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(context_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let rect = ui.max_rect();
                        let painter = ui.painter();
                        painter.rect_filled(rect, theme::CORNER_RADIUS, theme::PANEL_BG);
                        painter.rect_stroke(
                            rect,
                            theme::CORNER_RADIUS,
                            egui::Stroke::new(1.0, theme::BORDER),
                            egui::StrokeKind::Inside,
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.vertical(|ui| self.show_workspace_context(ui));
                        });
                    },
                );
            });
        } else {
            self.show_workspace_tab(ui);
        }
    }

    fn show_workspace_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("workspace-content")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.workspace_tab {
                WorkspaceTab::Import => self.show_import_workspace(ui),
                WorkspaceTab::Dataset => self.show_dataset_workspace(ui),
                WorkspaceTab::Analysis => self.show_analysis_workspace(ui),
                WorkspaceTab::Results => self.show_results_workspace(ui),
            });
    }

    fn show_import_workspace(&mut self, ui: &mut egui::Ui) {
        section_title(ui, self.text("导入本地数据", "Import local data"));
        ui.add_space(10.0);

        let hovered = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
        let drop_fill = if hovered {
            theme::ACCENT_TINT
        } else {
            theme::DROP_BG
        };
        egui::Frame::NONE
            .fill(drop_fill)
            .stroke(egui::Stroke::new(
                1.0,
                if hovered {
                    theme::ACCENT_STRONG
                } else {
                    theme::BORDER
                },
            ))
            .corner_radius(theme::CORNER_RADIUS)
            .inner_margin(egui::Margin::symmetric(16, 18))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(self.text(
                            "拖放 FASTA、FASTQ、表格或基因组文件",
                            "Drop FASTA, FASTQ, tables, or genomic files",
                        ))
                        .strong(),
                    );
                    ui.small(self.text(
                        "文件保持在原位置；仅保存本地路径和检查结果",
                        "Files stay in place; only paths and inspection results are kept",
                    ));
                    ui.add_space(8.0);
                    if ui.button(self.text("选择文件…", "Choose files…")).clicked()
                        && let Some(paths) = rfd::FileDialog::new()
                            .set_title(
                                self.text("导入生物信息学数据", "Import bioinformatics data"),
                            )
                            .add_filter(
                                "Bioinformatics",
                                &[
                                    "fa",
                                    "fasta",
                                    "fna",
                                    "faa",
                                    "fq",
                                    "fastq",
                                    "csv",
                                    "tsv",
                                    "bed",
                                    "gff",
                                    "gff3",
                                    "gtf",
                                    "vcf",
                                    "sam",
                                    "bam",
                                    "pdb",
                                    "cif",
                                    "mmcif",
                                    "blast",
                                    "m8",
                                    "domtblout",
                                    "hmm",
                                    "axt",
                                    "collinearity",
                                    "nwk",
                                    "newick",
                                    "tree",
                                    "tre",
                                    "mtx",
                                    "mzml",
                                    "sdf",
                                    "gz",
                                ],
                            )
                            .pick_files()
                    {
                        self.queue_paths(paths);
                    }
                });
            });

        ui.add_space(12.0);
        ui.label(self.text("文件路径", "File path"));
        let mut add_path = false;
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.import_path)
                    .desired_width(f32::INFINITY)
                    .hint_text("C:\\data\\sample.fastq.gz or /data/sample.fastq.gz"),
            );
            add_path = ui
                .add_enabled(
                    !self.import_path.trim().is_empty(),
                    egui::Button::new(self.text("添加", "Add")),
                )
                .clicked()
                || (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        });
        if add_path {
            let path = PathBuf::from(self.import_path.trim());
            if self.queue_path(path) {
                self.import_path.clear();
            }
        }
        ui.colored_label(theme::TEXT_MUTED, &self.import_status);

        ui.add_space(18.0);
        section_title(ui, self.text("格式边界", "Format boundaries"));
        egui::Grid::new("format-support-matrix")
            .striped(true)
            .min_col_width(150.0)
            .show(ui, |ui| {
                ui.strong(self.text("状态", "Status"));
                ui.strong(self.text("格式", "Formats"));
                ui.strong(self.text("压缩", "Compression"));
                ui.end_row();
                ui.colored_label(
                    DatasetState::Ready.color(),
                    self.text("可检查", "Inspect now"),
                );
                ui.label("FASTA, FASTQ, CSV/TSV, BED, GFF3/GTF, VCF, SAM, PDB, mmCIF");
                ui.label(".gz / BGZF");
                ui.end_row();
                ui.colored_label(
                    DatasetState::Warning.color(),
                    self.text("识别但暂不分析", "Recognize only"),
                );
                ui.label("BAM, BCF, CRAM, HDF5/H5AD, LOOM, RDS");
                ui.label(self.text("保持原文件", "Preserved"));
                ui.end_row();
                ui.colored_label(
                    DatasetState::Invalid.color(),
                    self.text("拒绝归档", "Archive rejected"),
                );
                ui.label("ZIP");
                ui.label("-");
                ui.end_row();
            });

        if !self.datasets.is_empty() {
            ui.add_space(18.0);
            self.show_import_queue(ui);
        }
    }

    fn show_import_queue(&mut self, ui: &mut egui::Ui) {
        section_title(ui, self.text("导入队列", "Import queue"));
        egui::Grid::new("import-queue")
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                ui.strong(self.text("文件", "File"));
                ui.strong(self.text("格式", "Format"));
                ui.strong(self.text("状态", "Status"));
                ui.end_row();
                for index in 0..self.datasets.len() {
                    let dataset = &self.datasets[index];
                    if ui.link(&dataset.name).clicked() {
                        self.selected_dataset = Some(index);
                        self.workspace_tab = WorkspaceTab::Dataset;
                    }
                    ui.monospace(&dataset.format_hint);
                    ui.colored_label(dataset.state.color(), dataset.state.label(self.language));
                    ui.end_row();
                }
            });
    }

    fn show_dataset_workspace(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected_dataset else {
            empty_state(
                ui,
                self.text("没有已选数据", "No dataset selected"),
                self.text("导入数据", "Import data"),
                &mut self.workspace_tab,
                WorkspaceTab::Import,
            );
            return;
        };
        let Some(dataset) = self.datasets.get(index) else {
            return;
        };
        let name = dataset.name.clone();
        let path = dataset.path.clone();
        let state = dataset.state;
        let message = dataset.message.clone();
        let hint = dataset.format_hint.clone();
        let inspection = dataset.inspection.clone();

        ui.horizontal_wrapped(|ui| {
            section_title(ui, &name);
            ui.colored_label(state.color(), state.label(self.language));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.project_work_active(),
                        egui::Button::new(self.text("移除", "Remove")),
                    )
                    .on_hover_text(self.text(
                        "导入和分析任务结束后可移除数据。",
                        "Remove data after import and analysis jobs finish.",
                    ))
                    .clicked()
                {
                    self.remove_selected_dataset();
                }
            });
        });
        ui.monospace(&path);
        ui.add_space(8.0);
        if state == DatasetState::Inspecting {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&message);
            });
            return;
        }
        if !message.is_empty()
            && (state == DatasetState::Invalid || message != "Inspection completed")
        {
            ui.colored_label(state.color(), &message);
        }

        let payload = inspection
            .as_ref()
            .map(inspection_payload)
            .unwrap_or(&Value::Null);
        let format = detected_format(payload).unwrap_or(&hint);
        let compression = lookup_string(payload, &["compression"])
            .or_else(|| first_file_field(payload, "compression"))
            .unwrap_or("none");
        let size =
            lookup_u64(payload, &["size_bytes"]).or_else(|| first_file_u64(payload, "size_bytes"));

        ui.add_space(12.0);
        egui::Grid::new("dataset-summary")
            .striped(true)
            .min_col_width(150.0)
            .show(ui, |ui| {
                ui.label(self.text("检测格式", "Detected format"));
                ui.monospace(format);
                ui.end_row();
                ui.label(self.text("压缩", "Compression"));
                ui.monospace(compression);
                ui.end_row();
                ui.label(self.text("文件大小", "File size"));
                ui.label(size.map(format_bytes).unwrap_or_else(|| "-".to_owned()));
                ui.end_row();
                ui.label(self.text("校验状态", "Validation"));
                ui.colored_label(state.color(), state.label(self.language));
                ui.end_row();
            });

        show_diagnostics(ui, payload, self.language);
        ui.add_space(16.0);
        section_title(ui, self.text("数据预览", "Data preview"));
        let preview = find_preview(payload).unwrap_or(payload);
        render_value_preview(ui, preview, self.language);

        ui.add_space(12.0);
        if inspection.as_ref().is_some_and(inspection_is_runnable)
            && ui
                .button(self.text("选择分析能力  →", "Choose analysis  →"))
                .clicked()
        {
            let detected = inspection
                .as_ref()
                .and_then(|value| detected_format(inspection_payload(value)))
                .unwrap_or(&hint);
            if let Some(route) = analysis_route_for_format(detected) {
                self.selected_capability = route.capability.to_owned();
            }
            self.workspace_tab = WorkspaceTab::Analysis;
        }

        if self.user_mode == UserMode::Expert
            && let Some(inspection) = inspection.as_ref()
        {
            ui.add_space(12.0);
            egui::CollapsingHeader::new(self.text("原始检查 JSON", "Raw inspection JSON")).show(
                ui,
                |ui| {
                    ui.monospace(pretty_json(inspection));
                },
            );
        }
    }

    fn show_analysis_workspace(&mut self, ui: &mut egui::Ui) {
        section_title(ui, self.text("选择分析能力", "Choose analysis capability"));
        let Some(dataset) = self.selected_dataset() else {
            empty_state(
                ui,
                self.text("先选择一个数据集", "Select a dataset first"),
                self.text("查看导入", "Open imports"),
                &mut self.workspace_tab,
                WorkspaceTab::Import,
            );
            return;
        };
        let dataset_name = dataset.name.clone();
        let state = dataset.state;
        let format = dataset_detected_format(dataset).to_owned();
        let supported = dataset
            .inspection
            .as_ref()
            .is_some_and(inspection_is_runnable);
        let route = analysis_route_for_format(&format);
        let dataset_ready =
            supported && state != DatasetState::Inspecting && state != DatasetState::Invalid;
        let capability_matches =
            analysis_route_for_capability(&self.selected_capability, &format).is_some();

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(self.text("输入", "Input"));
            ui.strong(dataset_name);
            ui.monospace(format.to_uppercase());
        });
        ui.add_space(14.0);

        egui::ComboBox::from_id_salt("analysis-capability")
            .selected_text(capability_title(&self.selected_capability, self.language))
            .width(360.0)
            .show_ui(ui, |ui| {
                for capability in [
                    "sequence.stats.v1",
                    "sequence.kmer.count.v1",
                    "primer.epcr.v1",
                    "fastq.qc.v1",
                    "fastq.trim.v1",
                    "fastq.adapter.v1",
                    "fastq.deduplicate.v1",
                    "alignment.qc.v1",
                    "alignment.bam-to-bigwig.v1",
                    "annotation.gxf.stats.v1",
                    "annotation.gxf.normalize.v1",
                    "annotation.gene-position.v1",
                    "annotation.sequence.extract.v1",
                    "annotation.structure.visualize.v1",
                    "comparative.synteny.visualize.v1",
                    "comparative.mcscanx.v1",
                    "comparative.kaks.v1",
                    "annotation.go.normalize.v1",
                    "annotation.eggnog.normalize.v1",
                    "enrichment.overrepresentation.v1",
                    "enrichment.go.v1",
                    "enrichment.kegg.v1",
                    "enrichment.gsea.v1",
                    "enrichment.visualize.v1",
                    "genome.gene-density.v1",
                    "variant.stats.v1",
                    "variant.filter.v1",
                    "variant.normalize.v1",
                    "interval.intersect.v1",
                    "interval.merge.v1",
                    "interval.subtract.v1",
                    "interval.closest.v1",
                    "expression.matrix.qc.v1",
                    "expression.differential.v1",
                    "medical.bulk-rnaseq.v1",
                    "medical.cohort-table.qc.v1",
                    "medical.pathway-ruo.v1",
                    "medical.variant-cohort.v1",
                    "medical.single-cell-qc.v1",
                    "expression.normalize.v1",
                    "expression.pca.v1",
                    "expression.cluster.v1",
                    "expression.heatmap.v1",
                    "set.venn.v1",
                    "set.upset.v1",
                    "protein.properties.v1",
                    "similarity.blast.local.v1",
                    "similarity.diamond.v1",
                    "similarity.hmmer.v1",
                    "similarity.blast.parse.v1",
                    "similarity.reciprocal.v1",
                    "protein.domain.parse.v1",
                    "protein.domain.visualize.v1",
                    "phylogeny.tree.transform.v1",
                    "phylogeny.iqtree.v1",
                    "msa.muscle.v1",
                    "msa.trimal.v1",
                    "motif.meme.v1",
                    "table.manipulate.v1",
                    "structure.pdb.summary.v1",
                    "structure.mmcif.summary.v1",
                    "structure.sequence.extract.v1",
                    "structure.contact-map.v1",
                    "structure.geometry.v1",
                    "structure.superpose.v1",
                    "protein.secondary-structure.v1",
                    "medical.pharmacogenomics.v1",
                    "medical.spatial-transcriptomics.v1",
                    "medical.metabolomics.v1",
                    "medical.microbiome.v1",
                    "medical.survival.v1",
                    "chemistry.descriptors.v1",
                ] {
                    ui.selectable_value(
                        &mut self.selected_capability,
                        capability.to_owned(),
                        capability_title(capability, self.language),
                    );
                }
            });

        let requires_secondary = capability_requires_secondary(&self.selected_capability);
        let primary_index = self.selected_dataset;
        let secondary_format = secondary_input_format(&self.selected_capability);
        let secondary_candidates = self
            .datasets
            .iter()
            .enumerate()
            .filter(|(index, dataset)| {
                Some(*index) != primary_index
                    && secondary_input_matches(
                        &self.selected_capability,
                        dataset_detected_format(dataset),
                    )
                    && dataset
                        .inspection
                        .as_ref()
                        .is_some_and(inspection_is_runnable)
                    && !matches!(
                        dataset.state,
                        DatasetState::Inspecting | DatasetState::Invalid
                    )
            })
            .map(|(index, dataset)| (index, dataset.name.clone()))
            .collect::<Vec<_>>();
        if requires_secondary
            && !self.secondary_dataset.is_some_and(|selected| {
                secondary_candidates
                    .iter()
                    .any(|(index, _)| *index == selected)
            })
        {
            self.secondary_dataset = secondary_candidates.first().map(|(index, _)| *index);
        }
        if requires_secondary {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let (secondary_label, missing_label) = match self.selected_capability.as_str() {
                    "primer.epcr.v1" => (
                        self.text("引物 TSV", "Primer TSV"),
                        self.text("没有可用引物 TSV", "No primer TSV available"),
                    ),
                    "variant.normalize.v1" | "annotation.sequence.extract.v1" => (
                        self.text("参考 FASTA", "Reference FASTA"),
                        self.text("没有可用 FASTA", "No FASTA available"),
                    ),
                    "structure.superpose.v1" => (
                        self.text("移动结构", "Mobile structure"),
                        self.text(
                            "没有其他可用 PDB/mmCIF",
                            "No other PDB/mmCIF structure available",
                        ),
                    ),
                    "similarity.reciprocal.v1" => (
                        self.text("反向相似性结果", "Reverse similarity result"),
                        self.text("没有其他可用 BLAST 结果", "No other BLAST result available"),
                    ),
                    "similarity.blast.local.v1" | "similarity.diamond.v1" => (
                        self.text("参考 FASTA", "Reference FASTA"),
                        self.text("没有其他可用 FASTA", "No other FASTA available"),
                    ),
                    "similarity.hmmer.v1" => (
                        self.text("目标序列 FASTA", "Target sequence FASTA"),
                        self.text("没有可用 FASTA", "No FASTA available"),
                    ),
                    "comparative.mcscanx.v1" => (
                        self.text("相似性命中表", "Similarity hits table"),
                        self.text(
                            "没有可用 BLAST 表格结果",
                            "No BLAST tabular result available",
                        ),
                    ),
                    "expression.differential.v1" | "medical.bulk-rnaseq.v1" => (
                        self.text("样本元数据", "Sample metadata"),
                        self.text(
                            "没有可用 CSV/TSV 样本元数据",
                            "No CSV/TSV sample metadata available",
                        ),
                    ),
                    "enrichment.gsea.v1" => (
                        self.text("基因集成员表", "Gene-set membership table"),
                        self.text(
                            "没有可用 CSV/TSV 基因集表",
                            "No CSV/TSV gene-set table available",
                        ),
                    ),
                    "enrichment.overrepresentation.v1"
                    | "enrichment.go.v1"
                    | "enrichment.kegg.v1"
                    | "medical.pathway-ruo.v1"
                    | "enrichment.visualize.v1" => (
                        self.text("功能关联表", "Association table"),
                        self.text(
                            "没有可用 CSV/TSV 关联表",
                            "No CSV/TSV association table available",
                        ),
                    ),
                    "interval.closest.v1" => (
                        self.text("目标 BED", "Target BED"),
                        self.text("没有其他 BED", "No other BED"),
                    ),
                    _ => (
                        self.text("右侧 BED", "Right BED"),
                        self.text("没有其他 BED", "No other BED"),
                    ),
                };
                ui.label(secondary_label);
                egui::ComboBox::from_id_salt("secondary-analysis-dataset")
                    .selected_text(
                        self.secondary_dataset
                            .and_then(|selected| {
                                secondary_candidates
                                    .iter()
                                    .find(|(index, _)| *index == selected)
                                    .map(|(_, name)| name.as_str())
                            })
                            .unwrap_or(missing_label),
                    )
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (index, name) in &secondary_candidates {
                            ui.selectable_value(&mut self.secondary_dataset, Some(*index), name);
                        }
                    });
            });
        }
        let requires_tertiary = tertiary_input_format(&self.selected_capability).is_some();
        if requires_tertiary {
            let tertiary_format = tertiary_input_format(&self.selected_capability);
            let tertiary_candidates = self
                .datasets
                .iter()
                .enumerate()
                .filter(|(index, dataset)| {
                    Some(*index) != primary_index
                        && Some(*index) != self.secondary_dataset
                        && tertiary_format.is_some_and(|required| {
                            dataset_detected_format(dataset).eq_ignore_ascii_case(required)
                        })
                        && dataset
                            .inspection
                            .as_ref()
                            .is_some_and(inspection_is_runnable)
                        && !matches!(
                            dataset.state,
                            DatasetState::Inspecting | DatasetState::Invalid
                        )
                })
                .map(|(index, dataset)| (index, dataset.name.clone()))
                .collect::<Vec<_>>();
            if !self.tertiary_dataset.is_some_and(|selected| {
                tertiary_candidates
                    .iter()
                    .any(|(index, _)| *index == selected)
            }) {
                self.tertiary_dataset = tertiary_candidates.first().map(|(index, _)| *index);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let (label, missing) = match self.selected_capability.as_str() {
                    "medical.spatial-transcriptomics.v1" => (
                        self.text("条形码 TSV", "Barcode TSV"),
                        self.text("没有可用条形码 TSV", "No barcode TSV available"),
                    ),
                    _ => (
                        self.text("第三输入", "Third input"),
                        self.text("没有可用输入", "No input available"),
                    ),
                };
                ui.label(label);
                egui::ComboBox::from_id_salt("tertiary-analysis-dataset")
                    .selected_text(
                        self.tertiary_dataset
                            .and_then(|selected| {
                                tertiary_candidates
                                    .iter()
                                    .find(|(index, _)| *index == selected)
                                    .map(|(_, name)| name.as_str())
                            })
                            .unwrap_or(missing),
                    )
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (index, name) in &tertiary_candidates {
                            ui.selectable_value(&mut self.tertiary_dataset, Some(*index), name);
                        }
                    });
            });
        }
        if self.selected_capability == "medical.survival.v1" {
            ui.add_space(8.0);
            ui.label(
                self.language
                    .text("生存分析列", "Survival analysis columns"),
            );
            egui::Grid::new("survival-columns")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(self.language.text("时间列", "Time column"));
                    ui.text_edit_singleline(&mut self.survival_time_column);
                    ui.end_row();
                    ui.label(self.language.text("事件列 (0/1)", "Event column (0/1)"));
                    ui.text_edit_singleline(&mut self.survival_event_column);
                    ui.end_row();
                    ui.label(self.language.text("分组列", "Group column"));
                    ui.text_edit_singleline(&mut self.survival_group_column);
                    ui.end_row();
                    ui.label(self.language.text("参考水平", "Reference level"));
                    ui.text_edit_singleline(&mut self.survival_reference_level);
                    ui.end_row();
                });
        }
        if self.selected_capability == "medical.microbiome.v1"
            || self.selected_capability == "metagenomics.classify.v1"
        {
            ui.add_space(8.0);
            ui.label(
                self.language
                    .text("Kraken2 数据库目录", "Kraken2 database directory"),
            );
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.microbiome_database);
                if ui.button(self.language.text("浏览…", "Browse…")).clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_title(
                            self.language
                                .text("选择 Kraken2 数据库", "Select Kraken2 database"),
                        )
                        .pick_folder()
                {
                    self.microbiome_database = path.to_string_lossy().into_owned();
                }
            });
            ui.horizontal(|ui| {
                ui.label(self.language.text("置信度", "Confidence"));
                ui.add(egui::Slider::new(
                    &mut self.microbiome_confidence,
                    0.0..=1.0,
                ));
            });
        }
        if self.selected_capability == "annotation.gxf.normalize.v1" {
            ui.add_space(8.0);
            ui.checkbox(
                &mut self.annotation_sort,
                self.language
                    .text("按序列和坐标排序", "Sort by sequence and coordinate"),
            );
        }
        if self.selected_capability == "annotation.sequence.extract.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("提取类型", "Feature type"));
                egui::ComboBox::from_id_salt("annotation-feature-type")
                    .selected_text(&self.annotation_feature_type)
                    .show_ui(ui, |ui| {
                        for feature_type in [
                            "gene",
                            "transcript",
                            "cds",
                            "exon",
                            "utr",
                            "five_prime_utr",
                            "three_prime_utr",
                            "promoter",
                        ] {
                            ui.selectable_value(
                                &mut self.annotation_feature_type,
                                feature_type.to_owned(),
                                feature_type,
                            );
                        }
                    });
            });
        }
        if self.selected_capability == "annotation.structure.visualize.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("特征 ID（优先）", "Feature ID (preferred)"));
                ui.text_edit_singleline(&mut self.annotation_visual_feature_id);
                ui.label(self.text("序列 ID", "Sequence ID"));
                ui.text_edit_singleline(&mut self.annotation_visual_seqid);
                ui.label(self.text("最多特征", "Maximum features"));
                ui.add(
                    egui::DragValue::new(&mut self.annotation_visual_max_features).range(1..=2_000),
                );
            });
            ui.small(self.text(
                "填写特征 ID 时忽略序列 ID；留空则自动选择首个基因或转录本。",
                "Feature ID takes precedence over sequence ID; leave both blank for automatic locus selection.",
            ));
        }
        if self.selected_capability == "annotation.go.normalize.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text(
                    "基因列（留空自动识别）",
                    "Gene column (blank for auto-detect)",
                ));
                ui.text_edit_singleline(&mut self.go_gene_column);
                ui.label(self.text("GO 列（留空自动识别）", "GO column (blank for auto-detect)"));
                ui.text_edit_singleline(&mut self.go_term_column);
            });
        }
        if self.selected_capability == "fastq.deduplicate.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("去重键", "Deduplication key"));
                egui::ComboBox::from_id_salt("fastq-deduplicate-mode")
                    .selected_text(match self.fastq_deduplicate_mode.as_str() {
                        "header-umi" => self.text("序列 + 读名 UMI", "Sequence + header UMI"),
                        "sequence-prefix-umi" => {
                            self.text("插入序列 + 前缀 UMI", "Insert + prefix UMI")
                        }
                        _ => self.text("仅序列", "Sequence only"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.fastq_deduplicate_mode,
                            "sequence".to_owned(),
                            self.language.text("仅序列", "Sequence only"),
                        );
                        ui.selectable_value(
                            &mut self.fastq_deduplicate_mode,
                            "header-umi".to_owned(),
                            self.language
                                .text("序列 + 读名 UMI", "Sequence + header UMI"),
                        );
                        ui.selectable_value(
                            &mut self.fastq_deduplicate_mode,
                            "sequence-prefix-umi".to_owned(),
                            self.language
                                .text("插入序列 + 前缀 UMI", "Insert + prefix UMI"),
                        );
                    });
                if self.fastq_deduplicate_mode == "header-umi" {
                    ui.label(self.text("UMI 分隔符", "UMI delimiter"));
                    ui.text_edit_singleline(&mut self.fastq_umi_delimiter);
                } else if self.fastq_deduplicate_mode == "sequence-prefix-umi" {
                    ui.label(self.text("UMI 长度", "UMI length"));
                    ui.add(egui::DragValue::new(&mut self.fastq_umi_length).range(1..=256));
                }
            });
        }
        if matches!(
            self.selected_capability.as_str(),
            "enrichment.overrepresentation.v1"
                | "enrichment.go.v1"
                | "enrichment.kegg.v1"
                | "medical.pathway-ruo.v1"
        ) {
            ui.add_space(8.0);
            let include_genes_label = self.text("结果包含命中基因", "Include overlapping genes");
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("最小重叠数", "Minimum overlap"));
                ui.add(egui::DragValue::new(&mut self.enrichment_min_overlap).range(1..=u64::MAX));
                ui.label(self.text("最多报告条目", "Maximum reported terms"));
                ui.add(egui::DragValue::new(&mut self.enrichment_max_terms).range(1..=100_000));
                ui.checkbox(&mut self.enrichment_include_genes, include_genes_label);
            });
        }
        if self.selected_capability == "enrichment.gsea.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("评分指数", "Score exponent"));
                ui.add(egui::DragValue::new(&mut self.gsea_score_exponent).range(0.0..=10.0));
                ui.label(self.text("最小集合", "Minimum set size"));
                ui.add(egui::DragValue::new(&mut self.gsea_min_set_size).range(1..=2_000_000));
                ui.label(self.text("最大集合", "Maximum set size"));
                ui.add(egui::DragValue::new(&mut self.gsea_max_set_size).range(1..=2_000_000));
                ui.label(self.text("置换次数", "Permutations"));
                ui.add(egui::DragValue::new(&mut self.gsea_permutations).range(1..=100_000));
                ui.label(self.text("种子", "Seed"));
                ui.add(egui::DragValue::new(&mut self.gsea_seed));
            });
        }
        if self.selected_capability == "enrichment.visualize.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("富集类型", "Enrichment kind"));
                egui::ComboBox::from_id_salt("enrichment-visual-kind")
                    .selected_text(&self.enrichment_visual_kind)
                    .show_ui(ui, |ui| {
                        for kind in ["custom", "go", "kegg"] {
                            ui.selectable_value(
                                &mut self.enrichment_visual_kind,
                                kind.to_owned(),
                                kind,
                            );
                        }
                    });
                ui.label(self.text("图形", "Plot"));
                egui::ComboBox::from_id_salt("enrichment-visual-style")
                    .selected_text(&self.enrichment_visual_style)
                    .show_ui(ui, |ui| {
                        for style in ["bar", "dot", "network"] {
                            ui.selectable_value(
                                &mut self.enrichment_visual_style,
                                style.to_owned(),
                                style,
                            );
                        }
                    });
                ui.label(self.text("最小重叠数", "Minimum overlap"));
                ui.add(egui::DragValue::new(&mut self.enrichment_min_overlap).range(1..=u64::MAX));
                ui.label(self.text("最多条目", "Maximum terms"));
                ui.add(egui::DragValue::new(&mut self.enrichment_max_terms).range(1..=2_000));
            });
        }
        if self.selected_capability == "protein.domain.visualize.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("序列 ID（可选）", "Sequence ID (optional)"));
                ui.text_edit_singleline(&mut self.protein_domain_visual_sequence_id);
                ui.label(self.text("最多序列", "Maximum sequences"));
                ui.add(
                    egui::DragValue::new(&mut self.protein_domain_visual_max_sequences)
                        .range(1..=2_000),
                );
                ui.label(self.text("最多结构域", "Maximum domains"));
                ui.add(
                    egui::DragValue::new(&mut self.protein_domain_visual_max_domains)
                        .range(1..=2_000),
                );
            });
        }
        if self.selected_capability == "genome.gene-density.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("特征类型", "Feature type"));
                ui.text_edit_singleline(&mut self.gene_density_feature_type);
                ui.label(self.text("窗口大小", "Window size"));
                ui.add(
                    egui::DragValue::new(&mut self.gene_density_window_size).range(1..=u64::MAX),
                );
                ui.label(self.text("步长", "Step size"));
                ui.add(egui::DragValue::new(&mut self.gene_density_step_size).range(1..=u64::MAX));
            });
        }
        if self.selected_capability == "comparative.kaks.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("计算方法", "Method"));
                egui::ComboBox::from_id_salt("comparative-kaks-method")
                    .selected_text(&self.kaks_method)
                    .show_ui(ui, |ui| {
                        for method in ["NG", "LWL", "LPB", "YN"] {
                            ui.selectable_value(&mut self.kaks_method, method.to_owned(), method);
                        }
                    });
            });
        }
        if self.selected_capability == "similarity.reciprocal.v1" {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("最大 e-value", "Maximum e-value"));
                ui.add(
                    egui::DragValue::new(&mut self.similarity_max_evalue)
                        .range(0.0..=1.0)
                        .speed(0.000001),
                );
                ui.label(self.text("最低相似度（%）", "Minimum identity (%)"));
                ui.add(
                    egui::DragValue::new(&mut self.similarity_min_identity_percent)
                        .range(0.0..=100.0)
                        .speed(0.1),
                );
            });
        }
        if matches!(
            self.selected_capability.as_str(),
            "similarity.blast.local.v1"
                | "similarity.diamond.v1"
                | "similarity.hmmer.v1"
                | "msa.muscle.v1"
                | "phylogeny.iqtree.v1"
                | "motif.meme.v1"
        ) {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("线程", "Threads"));
                ui.add(egui::DragValue::new(&mut self.native_threads).range(1..=1024));
                match self.selected_capability.as_str() {
                    "similarity.blast.local.v1" => {
                        ui.label(self.text("程序", "Program"));
                        egui::ComboBox::from_id_salt("native-blast-program")
                            .selected_text(&self.blast_program)
                            .show_ui(ui, |ui| {
                                for program in ["blastn", "blastp", "blastx", "tblastn", "tblastx"]
                                {
                                    ui.selectable_value(
                                        &mut self.blast_program,
                                        program.to_owned(),
                                        program,
                                    );
                                }
                            });
                    }
                    "similarity.diamond.v1" => {
                        ui.label(self.text("模式", "Mode"));
                        egui::ComboBox::from_id_salt("native-diamond-mode")
                            .selected_text(&self.diamond_mode)
                            .show_ui(ui, |ui| {
                                for mode in ["blastp", "blastx"] {
                                    ui.selectable_value(
                                        &mut self.diamond_mode,
                                        mode.to_owned(),
                                        mode,
                                    );
                                }
                            });
                    }
                    "similarity.hmmer.v1" => {
                        ui.label(self.text("模式", "Mode"));
                        egui::ComboBox::from_id_salt("native-hmmer-mode")
                            .selected_text(&self.hmmer_mode)
                            .show_ui(ui, |ui| {
                                for mode in ["hmmsearch", "hmmscan"] {
                                    ui.selectable_value(
                                        &mut self.hmmer_mode,
                                        mode.to_owned(),
                                        mode,
                                    );
                                }
                            });
                    }
                    "msa.muscle.v1" => {
                        ui.label(self.text("模式", "Mode"));
                        egui::ComboBox::from_id_salt("native-muscle-mode")
                            .selected_text(&self.muscle_mode)
                            .show_ui(ui, |ui| {
                                for mode in ["align", "super5"] {
                                    ui.selectable_value(
                                        &mut self.muscle_mode,
                                        mode.to_owned(),
                                        mode,
                                    );
                                }
                            });
                    }
                    "phylogeny.iqtree.v1" => {
                        ui.label(self.text("替换模型", "Substitution model"));
                        ui.text_edit_singleline(&mut self.iqtree_model);
                        ui.label(self.text("随机种子", "Random seed"));
                        ui.add(egui::DragValue::new(&mut self.iqtree_seed).range(1..=u64::MAX));
                    }
                    "motif.meme.v1" => {
                        ui.label(self.text("字母表", "Alphabet"));
                        egui::ComboBox::from_id_salt("native-meme-alphabet")
                            .selected_text(&self.meme_alphabet)
                            .show_ui(ui, |ui| {
                                for alphabet in ["dna", "rna", "protein"] {
                                    ui.selectable_value(
                                        &mut self.meme_alphabet,
                                        alphabet.to_owned(),
                                        alphabet,
                                    );
                                }
                            });
                        ui.label(self.text("出现模型", "Occurrence model"));
                        egui::ComboBox::from_id_salt("native-meme-distribution")
                            .selected_text(&self.meme_distribution)
                            .show_ui(ui, |ui| {
                                for distribution in ["oops", "zoops", "anr"] {
                                    ui.selectable_value(
                                        &mut self.meme_distribution,
                                        distribution.to_owned(),
                                        distribution,
                                    );
                                }
                            });
                    }
                    _ => {}
                }
            });
            if matches!(
                self.selected_capability.as_str(),
                "similarity.blast.local.v1" | "similarity.diamond.v1" | "similarity.hmmer.v1"
            ) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(self.text("e-value 阈值", "E-value threshold"));
                    ui.add(
                        egui::DragValue::new(&mut self.native_evalue)
                            .range(1e-300..=1e300)
                            .speed(0.0001),
                    );
                    if matches!(
                        self.selected_capability.as_str(),
                        "similarity.blast.local.v1" | "similarity.diamond.v1"
                    ) {
                        ui.label(self.text("最大目标数", "Maximum targets"));
                        ui.add(
                            egui::DragValue::new(&mut self.native_max_targets).range(1..=1_000_000),
                        );
                        ui.label("outfmt");
                        egui::ComboBox::from_id_salt("native-search-outfmt")
                            .selected_text(self.native_outfmt.to_string())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.native_outfmt, 6, "6");
                                ui.selectable_value(&mut self.native_outfmt, 7, "7");
                            });
                    }
                });
            }
        }
        if self.selected_capability == "msa.trimal.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("裁剪模式", "Trimming mode"));
                egui::ComboBox::from_id_salt("native-trimal-mode")
                    .selected_text(&self.trimal_mode)
                    .show_ui(ui, |ui| {
                        for mode in ["automated1", "gappyout", "strict", "strictplus"] {
                            ui.selectable_value(&mut self.trimal_mode, mode.to_owned(), mode);
                        }
                    });
            });
        }
        if self.selected_capability == "motif.meme.v1" {
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("基序数", "Motif count"));
                ui.add(egui::DragValue::new(&mut self.meme_motif_count).range(1..=100));
                ui.label(self.text("最小宽度", "Minimum width"));
                ui.add(egui::DragValue::new(&mut self.meme_minimum_width).range(2..=1_000));
                ui.label(self.text("最大宽度", "Maximum width"));
                ui.add(egui::DragValue::new(&mut self.meme_maximum_width).range(2..=1_000));
            });
        }
        if self.selected_capability == "phylogeny.tree.transform.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("外群叶标签（可选）", "Outgroup leaf label (optional)"));
                ui.text_edit_singleline(&mut self.phylogeny_reroot_label);
            });
        }
        if self.selected_capability == "sequence.kmer.count.v1" {
            ui.add_space(8.0);
            let canonical_label = self.text("合并反向互补", "Canonical reverse complement");
            ui.horizontal(|ui| {
                ui.label("k");
                ui.add(egui::DragValue::new(&mut self.kmer_size).range(1..=31));
                ui.checkbox(&mut self.kmer_canonical, canonical_label);
            });
        }
        if self.selected_capability == "primer.epcr.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("最大扩增子长度", "Maximum amplicon length"));
                ui.add(egui::DragValue::new(&mut self.epcr_max_amplicon).range(1..=10_000_000));
            });
        }
        if self.selected_capability == "variant.filter.v1" {
            ui.add_space(8.0);
            let pass_only_label = self.text("仅保留 PASS", "PASS only");
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("最低 QUAL", "Minimum QUAL"));
                ui.add(
                    egui::DragValue::new(&mut self.variant_filter_min_qual)
                        .range(-1_000.0..=1_000_000.0),
                );
                ui.checkbox(&mut self.variant_filter_pass_only, pass_only_label);
                ui.label(self.text("最低 INFO/DP（0=不限制）", "Minimum INFO/DP (0=off)"));
                ui.add(egui::DragValue::new(&mut self.variant_filter_min_dp));
            });
        }
        if self.selected_capability == "expression.normalize.v1" {
            ui.add_space(8.0);
            let method_label = self.text("标准化方法", "Normalization method");
            ui.horizontal(|ui| {
                ui.label(method_label);
                egui::ComboBox::from_id_salt("expression-normalization-method")
                    .selected_text(&self.expression_normalization_method)
                    .show_ui(ui, |ui| {
                        for method in ["cpm", "log2-cpm", "median-ratio"] {
                            ui.selectable_value(
                                &mut self.expression_normalization_method,
                                method.to_owned(),
                                method,
                            );
                        }
                    });
                if self.expression_normalization_method == "log2-cpm" {
                    ui.label(self.text("伪计数", "Pseudocount"));
                    ui.add(
                        egui::DragValue::new(&mut self.expression_pseudocount)
                            .range(0.0..=1_000_000.0),
                    );
                }
            });
        }
        if matches!(
            self.selected_capability.as_str(),
            "expression.differential.v1" | "medical.bulk-rnaseq.v1"
        ) {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("特征 ID 列", "Feature ID column"));
                ui.text_edit_singleline(&mut self.differential_feature_id_column);
                ui.label(self.text("样本 ID 列", "Sample ID column"));
                ui.text_edit_singleline(&mut self.differential_sample_id_column);
                ui.label(self.text("条件列", "Condition column"));
                ui.text_edit_singleline(&mut self.differential_condition_column);
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("参考水平", "Reference level"));
                ui.text_edit_singleline(&mut self.differential_reference_level);
                ui.label(self.text("对比水平", "Contrast level"));
                ui.text_edit_singleline(&mut self.differential_contrast_level);
                ui.label("alpha");
                ui.add(egui::DragValue::new(&mut self.differential_alpha).range(0.000001..=1.0));
                ui.label(self.text("最低总计数", "Minimum total count"));
                ui.add(egui::DragValue::new(&mut self.differential_min_total_count));
            });
            if self.selected_capability == "medical.bulk-rnaseq.v1" {
                ui.small(self.text(
                    "仅限科研，不提供诊断、治疗建议或临床解释。",
                    "Research use only; no diagnosis, treatment advice, or clinical interpretation.",
                ));
            }
        }
        if self.selected_capability == "expression.pca.v1" {
            ui.add_space(8.0);
            let scale_label = self.text("按特征标准化", "Scale features");
            ui.horizontal(|ui| {
                ui.label(self.text("主成分数", "Components"));
                ui.add(egui::DragValue::new(&mut self.expression_pca_components).range(1..=50));
                ui.checkbox(&mut self.expression_pca_scale, scale_label);
            });
        }
        if self.selected_capability == "expression.cluster.v1" {
            ui.add_space(8.0);
            let scale_label = self.text("按特征标准化", "Scale features");
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("样本簇", "Sample clusters"));
                ui.add(egui::DragValue::new(&mut self.expression_sample_clusters).range(1..=100));
                ui.label(self.text("特征簇", "Feature clusters"));
                ui.add(egui::DragValue::new(&mut self.expression_feature_clusters).range(1..=100));
                ui.checkbox(&mut self.expression_cluster_scale, scale_label);
            });
        }
        if self.selected_capability == "expression.heatmap.v1" {
            ui.add_space(8.0);
            let scale_label = self.text("按行 z-score", "Row z-score");
            ui.horizontal(|ui| {
                ui.label(self.text("高变特征数", "Top variable features"));
                ui.add(
                    egui::DragValue::new(&mut self.expression_heatmap_top_features).range(1..=200),
                );
                ui.checkbox(&mut self.expression_heatmap_scale, scale_label);
            });
        }
        if self.selected_capability == "structure.pdb.summary.v1" {
            ui.add_space(8.0);
            ui.checkbox(
                &mut self.interpret_pdb_b_factors_as_plddt,
                self.language.text(
                    "明确将 B-factor 解释为 AlphaFold pLDDT",
                    "Explicitly interpret B-factor as AlphaFold pLDDT",
                ),
            );
        }
        if self.selected_capability == "structure.contact-map.v1" {
            ui.add_space(8.0);
            let include_inter_chain = self.text("包含链间接触", "Include inter-chain contacts");
            ui.horizontal_wrapped(|ui| {
                ui.label(self.text("距离阈值（埃）", "Distance cutoff (angstrom)"));
                ui.add(
                    egui::DragValue::new(&mut self.structure_contact_cutoff)
                        .range(0.1..=1_000.0)
                        .speed(0.1),
                );
                ui.label(self.text("代表原子", "Representative atom"));
                ui.text_edit_singleline(&mut self.structure_contact_atom);
                ui.checkbox(
                    &mut self.structure_contact_include_inter_chain,
                    include_inter_chain,
                );
            });
        }
        if self.selected_capability == "structure.geometry.v1" {
            ui.add_space(8.0);
            let distance_label = self.text("距离（2 原子）", "Distance (2 atoms)");
            let angle_label = self.text("角度（3 原子）", "Angle (3 atoms)");
            let dihedral_label = self.text("扭转角（4 原子）", "Dihedral (4 atoms)");
            ui.horizontal(|ui| {
                ui.label(self.text("测量类型", "Measurement"));
                egui::ComboBox::from_id_salt("structure-geometry-count")
                    .selected_text(match self.structure_geometry_atom_count {
                        2 => distance_label,
                        3 => angle_label,
                        _ => dihedral_label,
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.structure_geometry_atom_count,
                            2,
                            distance_label,
                        );
                        ui.selectable_value(
                            &mut self.structure_geometry_atom_count,
                            3,
                            angle_label,
                        );
                        ui.selectable_value(
                            &mut self.structure_geometry_atom_count,
                            4,
                            dihedral_label,
                        );
                    });
            });
            ui.small(self.text(
                "选择器格式：链/残基/原子，或 模型/链/残基/原子",
                "Selector: CHAIN/RESIDUE/ATOM or MODEL/CHAIN/RESIDUE/ATOM",
            ));
            for index in 0..self.structure_geometry_atom_count {
                ui.horizontal(|ui| {
                    ui.label(format!("{} {}", self.text("原子", "Atom"), index + 1));
                    ui.text_edit_singleline(&mut self.structure_geometry_atoms[index]);
                });
            }
        }
        if self.selected_capability == "structure.superpose.v1" {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(self.text("匹配原子", "Matched atom"));
                ui.text_edit_singleline(&mut self.structure_superpose_atom);
            });
            ui.small(self.text(
                "仅按链、残基编号和原子名匹配，不执行序列比对。",
                "Matches chain, residue ID, and atom name only; no sequence alignment.",
            ));
        }

        ui.add_space(10.0);
        egui::Grid::new("analysis-settings")
            .striped(true)
            .min_col_width(150.0)
            .show(ui, |ui| {
                ui.label(self.text("执行位置", "Execution"));
                ui.label(self.text("本机 CPU", "Local CPU"));
                ui.end_row();
                ui.label(self.text("数据上传", "Data upload"));
                ui.label(self.text("无", "None"));
                ui.end_row();
                ui.label(self.text("结果契约", "Result contract"));
                ui.monospace("AnalysisResult");
                ui.end_row();
                if capability_output_extension(&self.selected_capability).is_some()
                    || matches!(
                        self.selected_capability.as_str(),
                        "expression.differential.v1" | "medical.bulk-rnaseq.v1"
                    )
                {
                    ui.label(self.text("输出文件", "Output file"));
                    ui.label(self.text(
                        "自动写入输入文件同目录，不覆盖已有文件",
                        "Auto-written next to the input without overwriting",
                    ));
                    ui.end_row();
                }
            });

        let secondary_ready = !requires_secondary || self.secondary_dataset.is_some();
        if !dataset_ready || !capability_matches || !secondary_ready {
            ui.add_space(8.0);
            let message = if !dataset_ready {
                self.text(
                    "只有检查通过的数据才能运行本地分析。",
                    "Local analysis requires a dataset that passed inspection.",
                )
                .to_owned()
            } else if !secondary_ready {
                if secondary_format == Some("structure") {
                    self.text(
                        "结构叠合需要再导入一个检查通过的 PDB 或 mmCIF 文件。",
                        "Structure superposition requires another validated PDB or mmCIF file.",
                    )
                    .to_owned()
                } else if secondary_format == Some("fasta") {
                    self.text(
                        "注释序列提取需要再导入一个检查通过的参考 FASTA。",
                        "Annotation extraction requires another validated reference FASTA.",
                    )
                    .to_owned()
                } else if secondary_format == Some("blast") {
                    self.text(
                        "双向最佳命中需要再导入一个检查通过的反向 BLAST 结果。",
                        "Reciprocal best-hit analysis requires another validated reverse BLAST result.",
                    )
                    .to_owned()
                } else {
                    self.text(
                        "区间相交或扣除需要再导入一个检查通过的 BED 文件。",
                        "Interval intersection or subtraction requires another validated BED file.",
                    )
                    .to_owned()
                }
            } else if let Some(route) = route {
                match self.language {
                    Language::ZhCn => format!(
                        "{} 数据应使用 {}。",
                        format.to_uppercase(),
                        route.capability
                    ),
                    Language::EnUs => format!(
                        "{} data requires {}.",
                        format.to_uppercase(),
                        route.capability
                    ),
                }
            } else {
                self.text(
                    "当前格式还没有可执行的本地分析能力。",
                    "No executable local analysis capability supports this format yet.",
                )
                .to_owned()
            };
            ui.colored_label(DatasetState::Warning.color(), message);
        }
        ui.add_space(12.0);
        let can_run =
            dataset_ready && capability_matches && secondary_ready && !self.analysis_running;
        if ui
            .add_enabled(
                can_run,
                egui::Button::new(self.text("运行本地分析", "Run local analysis")),
            )
            .clicked()
        {
            self.start_selected_analysis();
        }
    }

    fn show_results_workspace(&mut self, ui: &mut egui::Ui) {
        section_title(ui, self.text("分析结果", "Analysis results"));
        if self.analysis_running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&self.analysis_status);
            });
        } else {
            ui.label(&self.analysis_status);
        }

        let Some(result) = self.analysis_result.clone() else {
            ui.add_space(18.0);
            self.show_job_history(ui);
            return;
        };
        let payload = result.get("result").unwrap_or(&result);
        let language = self.language;

        ui.add_space(12.0);
        result_panel(
            ui,
            self.text("统计摘要", "Statistics summary"),
            Some(self.text(
                "关键数值与文件级摘要，来自本次分析输出。",
                "Key numeric and file-level summaries from this analysis output.",
            )),
            |ui| render_metrics(ui, payload, language),
        );

        ui.add_space(14.0);
        result_panel(
            ui,
            self.text("图表预览", "Chart preview"),
            Some(self.text(
                "火山图、热图、共线性、点图、模体与进化树等图形结果统一在此容器中展示。",
                "Volcano, heatmap, synteny, dotplot, motif, and tree figures all share this container.",
            )),
            |ui| {
                let capability = result.get("capability").and_then(Value::as_str);
                if !visualization::show_analysis_charts(
                    ui,
                    payload,
                    capability,
                    language == Language::ZhCn,
                ) {
                    ui.small(self.text(
                        "当前结果没有可绘制的数值序列。",
                        "This result has no plottable numeric series.",
                    ));
                }
            },
        );

        ui.add_space(14.0);
        result_panel(
            ui,
            self.text("导出", "Export"),
            Some(self.text(
                "导出当前分析结果；格式与扩展名一一对应。",
                "Export the current analysis result; each format matches its extension.",
            )),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("CSV").clicked() {
                        self.export_analysis(&result, ExportFormat::Csv);
                    }
                    if ui.button("TSV").clicked() {
                        self.export_analysis(&result, ExportFormat::Tsv);
                    }
                    if ui.button("JSON").clicked() {
                        self.export_analysis(&result, ExportFormat::Json);
                    }
                    if ui.button("XLSX").clicked() {
                        self.export_analysis(&result, ExportFormat::Xlsx);
                    }
                    ui.add_enabled(false, egui::Button::new("Parquet"))
                        .on_hover_text(self.text("计划能力", "Planned capability"));
                });
                if !self.export_status.is_empty() {
                    ui.small(&self.export_status);
                }
            },
        );

        if self.user_mode == UserMode::Expert {
            ui.add_space(12.0);
            egui::CollapsingHeader::new(self.text("原始结果 JSON", "Raw result JSON")).show(
                ui,
                |ui| {
                    ui.monospace(pretty_json(&result));
                },
            );
        }
        ui.add_space(18.0);
        self.show_job_history(ui);
    }

    fn show_job_history(&self, ui: &mut egui::Ui) {
        section_title(ui, self.text("任务", "Jobs"));
        if self.job_history.is_empty() {
            ui.small(self.text("暂无任务", "No jobs yet"));
            return;
        }
        egui::Grid::new("job-history")
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                ui.strong(self.text("能力", "Capability"));
                ui.strong(self.text("数据", "Dataset"));
                ui.strong(self.text("状态", "Status"));
                ui.end_row();
                for job in self.job_history.iter().rev() {
                    ui.monospace(&job.capability);
                    ui.label(&job.dataset_name);
                    let color = match job.state {
                        JobState::Running => DatasetState::Inspecting.color(),
                        JobState::Completed => DatasetState::Ready.color(),
                        JobState::Failed => DatasetState::Invalid.color(),
                    };
                    ui.colored_label(color, job.state.label(self.language))
                        .on_hover_text(&job.message);
                    ui.end_row();
                }
            });
    }

    fn show_workspace_context(&mut self, ui: &mut egui::Ui) {
        ui.strong(self.text("当前数据", "CURRENT DATA"));
        ui.add_space(6.0);
        if let Some(dataset) = self.selected_dataset() {
            ui.label(egui::RichText::new(&dataset.name).strong());
            ui.monospace(dataset_detected_format(dataset).to_uppercase());
            ui.colored_label(dataset.state.color(), dataset.state.label(self.language));
        } else {
            ui.small(self.text("未选择", "None selected"));
        }
        ui.add_space(18.0);
        ui.separator();
        ui.strong(self.text("本地执行", "LOCAL EXECUTION"));
        ui.add_space(6.0);
        if self.environment_running {
            ui.spinner();
        }
        ui.small(&self.environment_status);
        if ui
            .link(self.text("查看环境详情", "Open environment details"))
            .clicked()
        {
            self.page = Page::Environment;
        }
        ui.add_space(18.0);
        ui.separator();
        ui.strong(self.text("文档", "DOCUMENTATION"));
        ui.add_space(6.0);
        if ui
            .link(capability_title(&self.selected_capability, self.language))
            .clicked()
        {
            self.document_capability = self.selected_capability.clone();
            self.page = Page::Documentation;
        }
        ui.small(self.text("文档随应用离线提供", "Bundled for offline use"));
    }

    fn export_analysis(&mut self, result: &Value, format: ExportFormat) {
        let extension = format.extension();
        let basename = analysis_export_basename(result);
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.text("导出分析结果", "Export analysis result"))
            .set_file_name(format!("{basename}.{extension}"))
            .add_filter(format.label(), &[extension])
            .save_file()
        else {
            return;
        };
        self.export_status = match export_value(result, &path) {
            Ok(_) => match self.language {
                Language::ZhCn => format!("已导出到 {}", path.display()),
                Language::EnUs => format!("Exported to {}", path.display()),
            },
            Err(error) => match self.language {
                Language::ZhCn => format!("导出失败：{error}"),
                Language::EnUs => format!("Export failed: {error}"),
            },
        };
    }

    fn show_structure_viewer(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("蛋白质结构查看器", "Protein structure viewer"));
        ui.label(self.text(
            "在本机解析并显示 PDB 或 mmCIF 坐标；文件不会上传。",
            "Parse and display PDB or mmCIF coordinates locally; files are never uploaded.",
        ));
        ui.add_space(8.0);

        let selected_structure = self.selected_dataset().and_then(|dataset| {
            matches!(dataset.format_hint.as_str(), "pdb" | "mmcif")
                .then(|| (dataset.name.clone(), PathBuf::from(&dataset.path)))
        });
        let mut selected_to_open = None;
        let mut picked_to_open = None;
        let mut snapshot_to_save = None;
        let structure_loading = self.structure_viewer.is_loading();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !structure_loading,
                    egui::Button::new(self.text("打开结构文件…", "Open structure file…")),
                )
                .clicked()
            {
                picked_to_open = rfd::FileDialog::new()
                    .set_title(self.text("打开蛋白质结构", "Open protein structure"))
                    .add_filter("Protein structure", &["pdb", "cif", "mmcif", "gz", "bgz"])
                    .pick_file();
            }
            if let Some((name, path)) = &selected_structure
                && ui
                    .add_enabled(
                        !structure_loading,
                        egui::Button::new(match self.language {
                            Language::ZhCn => format!("查看已选文件：{name}"),
                            Language::EnUs => format!("View selected: {name}"),
                        }),
                    )
                    .clicked()
            {
                selected_to_open = Some(path.clone());
            }
            if ui
                .add_enabled(
                    self.structure_viewer.has_model(),
                    egui::Button::new(self.text("导出 PNG", "Export PNG")),
                )
                .clicked()
            {
                snapshot_to_save = rfd::FileDialog::new()
                    .set_title(self.text("导出结构视图", "Export structure view"))
                    .set_file_name(self.structure_viewer.suggested_snapshot_name())
                    .add_filter("PNG image", &["png"])
                    .save_file();
            }
        });
        let zh_cn = self.language == Language::ZhCn;
        if let Some(path) = picked_to_open.or(selected_to_open) {
            self.structure_viewer.load_path(path, zh_cn);
        }
        if let Some(path) = snapshot_to_save {
            self.structure_viewer.save_png(&path, zh_cn);
        }
        ui.add_space(6.0);
        self.structure_viewer.show(ui, zh_cn);
    }

    fn show_environment(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("本地运行环境", "Local environment"));
        ui.label(self.text(
            "审计本机工具并生成可复核的事务预览。执行安装仍未开放。",
            "Audit local tools and build a reviewable transaction preview. Installation remains disabled.",
        ));
        ui.add_space(8.0);

        let run_audit = ui
            .add_enabled(
                !self.environment_running,
                egui::Button::new(self.language.text("重新审计", "Refresh audit")),
            )
            .clicked();
        ui.add_space(10.0);
        ui.strong(self.text("工作负载", "Workload"));
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("environment-profile")
                .selected_text(&self.environment_profile)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "local-core".to_owned(),
                        "local-core",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "scripting".to_owned(),
                        "scripting",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "managed-runtimes".to_owned(),
                        "managed-runtimes",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "containers".to_owned(),
                        "containers",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "sequence-search".to_owned(),
                        "sequence-search",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "multiple-sequence-alignment".to_owned(),
                        "multiple-sequence-alignment",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "genomics-cli".to_owned(),
                        "genomics-cli",
                    );
                    ui.selectable_value(
                        &mut self.environment_profile,
                        "full-local".to_owned(),
                        "full-local",
                    );
                });
        });
        ui.add_space(8.0);
        ui.strong(self.text("环境模式", "Environment mode"));
        ui.horizontal_wrapped(|ui| {
            for mode in [
                EnvironmentPlanMode::UseExisting,
                EnvironmentPlanMode::ManagedUser,
                EnvironmentPlanMode::ProjectIsolated,
                EnvironmentPlanMode::SystemMissingOnly,
            ] {
                ui.selectable_value(&mut self.environment_mode, mode, mode.label(self.language));
            }
        });
        ui.small(environment_mode_description(
            self.environment_mode,
            self.language,
        ));
        if self.environment_mode == EnvironmentPlanMode::ProjectIsolated {
            ui.add_space(6.0);
            ui.label(self.text("项目根目录", "Project root"));
            ui.add(
                egui::TextEdit::singleline(&mut self.environment_project_root)
                    .desired_width(f32::INFINITY)
                    .hint_text("C:\\work\\project or /work/project"),
            );
        }
        ui.add_space(8.0);
        let project_root_ready = self.environment_mode != EnvironmentPlanMode::ProjectIsolated
            || !self.environment_project_root.trim().is_empty();
        let build_plan = ui
            .add_enabled(
                !self.environment_running && project_root_ready,
                egui::Button::new(
                    self.language
                        .text("生成事务预览", "Build transaction preview"),
                ),
            )
            .clicked();
        if run_audit {
            self.start_environment_job(EnvironmentJob::Audit);
        }
        if build_plan {
            self.start_environment_job(EnvironmentJob::Plan);
        }
        if self.environment_running {
            ui.spinner();
        }
        ui.label(&self.environment_status);
        ui.separator();

        let Some(result) = &self.environment_result else {
            return;
        };
        let capability = result
            .get("capability")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let payload = result.get("result").unwrap_or(&Value::Null);
        match capability {
            "environment.audit.v1" => show_environment_audit(ui, payload, self.language),
            "environment.plan.v1" => show_environment_plan(ui, payload, self.language),
            _ => {}
        }
        ui.add_space(8.0);
        egui::CollapsingHeader::new(self.text("原始环境 JSON", "Raw environment JSON")).show(
            ui,
            |ui| {
                ui.monospace(
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()),
                );
            },
        );
    }

    fn show_documentation(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("离线能力文档", "Offline capability documentation"));
        ui.label(self.text(
            "这些文档随应用一起提供，不需要网络连接。",
            "These documents are bundled with the application and require no network connection.",
        ));
        ui.add_space(8.0);

        egui::ComboBox::from_id_salt("documentation-capability")
            .selected_text(document_title(&self.document_capability, self.language))
            .width(320.0)
            .show_ui(ui, |ui| {
                for capability in DOCUMENTED_CAPABILITIES {
                    ui.selectable_value(
                        &mut self.document_capability,
                        (*capability).to_owned(),
                        document_title(capability, self.language),
                    );
                }
            });
        ui.separator();

        if let Some(document) = capability_document(&self.document_capability, self.language) {
            render_markdown_document(ui, &document);
        } else {
            ui.colored_label(
                theme::DANGER_DEEP,
                self.text("文档未找到。", "Documentation was not found."),
            );
        }
    }

    fn show_licenses(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("许可证与第三方组件", "Licenses and third-party components"));
        ui.label(self.text(
            "以下文本属于当前安装包，可离线审查。",
            "These texts belong to the current installation and are available offline.",
        ));
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for document in [
                LegalDocument::ProjectLicense,
                LegalDocument::ThirdPartyPolicy,
                LegalDocument::RustDependencies,
                LegalDocument::FontLicense,
            ] {
                ui.selectable_value(
                    &mut self.legal_document,
                    document,
                    document.label(self.language),
                );
            }
        });
        ui.separator();

        match self.legal_document {
            LegalDocument::ProjectLicense => {
                render_plain_document(ui, include_str!("../../../LICENSE"))
            }
            LegalDocument::ThirdPartyPolicy => {
                render_markdown_document(ui, include_str!("../../../THIRD_PARTY.md"))
            }
            LegalDocument::FontLicense => {
                render_plain_document(ui, include_str!("../../../licenses/NotoSansCJK-OFL.txt"))
            }
            LegalDocument::RustDependencies => match &self.dependency_notices {
                Ok(notices) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(match self.language {
                            Language::ZhCn => {
                                format!("当前平台发行闭包：{} 个第三方包", notices.package_count)
                            }
                            Language::EnUs => format!(
                                "Current platform release closure: {} third-party packages",
                                notices.package_count
                            ),
                        });
                        ui.monospace(notices.directory.display().to_string());
                    });
                    ui.add_space(6.0);
                    render_dependency_notices(ui, &notices.lines);
                }
                Err(error) => {
                    ui.colored_label(
                        theme::WARNING,
                        self.text(
                            "开发构建旁未找到平台依赖 NOTICE；正式发行包会在 staging 时生成。",
                            "No platform dependency NOTICE was found beside this development build; release staging generates it.",
                        ),
                    );
                    ui.monospace(error);
                }
            },
        }
    }
}

impl eframe::App for BioApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped_paths = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect::<Vec<_>>()
        });
        self.queue_paths(dropped_paths);
        self.poll_inspection_jobs();
        self.poll_analysis_job();
        self.poll_environment_job();
        if self.analysis_running
            || self.environment_running
            || self.active_inspections > 0
            || !self.inspection_queue.is_empty()
            || self
                .datasets
                .iter()
                .any(|dataset| dataset.state == DatasetState::Inspecting)
        {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(4.0);
            self.show_top_bar(ui);
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(
                    ui.available_width(),
                    (ui.available_height() - theme::STATUS_BAR_HEIGHT).max(160.0),
                ),
                egui::Layout::left_to_right(egui::Align::TOP),
                |ui| {
                    let height = ui.available_height();
                    ui.allocate_ui_with_layout(
                        egui::vec2(theme::SIDEBAR_WIDTH, height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| self.show_navigation(ui),
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| match self.page {
                            Page::Workspace => self.show_workspace(ui),
                            Page::Structure => {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.show_structure_viewer(ui));
                            }
                            Page::Environment => {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.show_environment(ui));
                            }
                            Page::Documentation => {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.show_documentation(ui));
                            }
                            Page::Licenses => {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.show_licenses(ui));
                            }
                        },
                    );
                },
            );
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(2.0);
            self.show_status_bar(ui);
            ui.add_space(4.0);
        });
    }
}

fn analysis_route_for_format(format: &str) -> Option<AnalysisRoute> {
    match format.trim().to_ascii_lowercase().as_str() {
        "fasta" => Some(AnalysisRoute {
            capability: "sequence.stats.v1",
            input_role: "fasta",
        }),
        "fastq" => Some(AnalysisRoute {
            capability: "fastq.qc.v1",
            input_role: "fastq",
        }),
        "sam" => Some(AnalysisRoute {
            capability: "alignment.qc.v1",
            input_role: "sam",
        }),
        "gff3" | "gtf" => Some(AnalysisRoute {
            capability: "annotation.gxf.stats.v1",
            input_role: "annotation",
        }),
        "bed" => Some(AnalysisRoute {
            capability: "interval.intersect.v1",
            input_role: "left-bed",
        }),
        "csv" | "tsv" => Some(AnalysisRoute {
            capability: "expression.matrix.qc.v1",
            input_role: "matrix",
        }),
        "vcf" => Some(AnalysisRoute {
            capability: "variant.stats.v1",
            input_role: "vcf",
        }),
        "pdb" => Some(AnalysisRoute {
            capability: "structure.pdb.summary.v1",
            input_role: "pdb",
        }),
        "mmcif" => Some(AnalysisRoute {
            capability: "structure.mmcif.summary.v1",
            input_role: "structure",
        }),
        "blast-tabular" | "blast-xml" => Some(AnalysisRoute {
            capability: "similarity.blast.parse.v1",
            input_role: "blast",
        }),
        "protein-domains" => Some(AnalysisRoute {
            capability: "protein.domain.parse.v1",
            input_role: "domains",
        }),
        "hmm-profile" => Some(AnalysisRoute {
            capability: "similarity.hmmer.v1",
            input_role: "profile",
        }),
        "newick" => Some(AnalysisRoute {
            capability: "phylogeny.tree.transform.v1",
            input_role: "tree",
        }),
        "axt" => Some(AnalysisRoute {
            capability: "comparative.kaks.v1",
            input_role: "codon-alignment",
        }),
        "mzml" => Some(AnalysisRoute {
            capability: "medical.metabolomics.v1",
            input_role: "mzml",
        }),
        "sdf" => Some(AnalysisRoute {
            capability: "chemistry.descriptors.v1",
            input_role: "molecules",
        }),
        _ => None,
    }
}

fn analysis_route_for_capability(capability: &str, format: &str) -> Option<AnalysisRoute> {
    let format = format.trim().to_ascii_lowercase();
    match (capability, format.as_str()) {
        ("sequence.stats.v1", "fasta") => Some(AnalysisRoute {
            capability: "sequence.stats.v1",
            input_role: "fasta",
        }),
        ("sequence.kmer.count.v1", "fasta") => Some(AnalysisRoute {
            capability: "sequence.kmer.count.v1",
            input_role: "fasta",
        }),
        ("primer.epcr.v1", "fasta") => Some(AnalysisRoute {
            capability: "primer.epcr.v1",
            input_role: "fasta",
        }),
        ("fastq.qc.v1", "fastq") => Some(AnalysisRoute {
            capability: "fastq.qc.v1",
            input_role: "fastq",
        }),
        ("fastq.trim.v1", "fastq") => Some(AnalysisRoute {
            capability: "fastq.trim.v1",
            input_role: "fastq",
        }),
        ("fastq.adapter.v1", "fastq") => Some(AnalysisRoute {
            capability: "fastq.adapter.v1",
            input_role: "fastq",
        }),
        ("fastq.deduplicate.v1", "fastq") => Some(AnalysisRoute {
            capability: "fastq.deduplicate.v1",
            input_role: "fastq",
        }),
        ("alignment.qc.v1", "sam") => Some(AnalysisRoute {
            capability: "alignment.qc.v1",
            input_role: "sam",
        }),
        (
            "annotation.gxf.stats.v1"
            | "annotation.gxf.normalize.v1"
            | "annotation.gene-position.v1"
            | "annotation.sequence.extract.v1"
            | "annotation.structure.visualize.v1"
            | "genome.gene-density.v1",
            "gff3" | "gtf",
        ) => Some(AnalysisRoute {
            capability: match capability {
                "annotation.gxf.stats.v1" => "annotation.gxf.stats.v1",
                "annotation.gxf.normalize.v1" => "annotation.gxf.normalize.v1",
                "annotation.gene-position.v1" => "annotation.gene-position.v1",
                "annotation.sequence.extract.v1" => "annotation.sequence.extract.v1",
                "annotation.structure.visualize.v1" => "annotation.structure.visualize.v1",
                _ => "genome.gene-density.v1",
            },
            input_role: "annotation",
        }),
        ("interval.intersect.v1", "bed") => Some(AnalysisRoute {
            capability: "interval.intersect.v1",
            input_role: "left-bed",
        }),
        ("interval.merge.v1", "bed") => Some(AnalysisRoute {
            capability: "interval.merge.v1",
            input_role: "bed",
        }),
        ("interval.subtract.v1", "bed") => Some(AnalysisRoute {
            capability: "interval.subtract.v1",
            input_role: "left-bed",
        }),
        ("interval.closest.v1", "bed") => Some(AnalysisRoute {
            capability: "interval.closest.v1",
            input_role: "query-bed",
        }),
        (
            "expression.matrix.qc.v1"
            | "expression.normalize.v1"
            | "expression.pca.v1"
            | "expression.cluster.v1"
            | "expression.heatmap.v1",
            "csv" | "tsv",
        ) => Some(AnalysisRoute {
            capability: match capability {
                "expression.matrix.qc.v1" => "expression.matrix.qc.v1",
                "expression.normalize.v1" => "expression.normalize.v1",
                "expression.pca.v1" => "expression.pca.v1",
                "expression.cluster.v1" => "expression.cluster.v1",
                _ => "expression.heatmap.v1",
            },
            input_role: "matrix",
        }),
        ("expression.volcano.v1", "csv") => Some(AnalysisRoute {
            capability: "expression.volcano.v1",
            input_role: "differential",
        }),
        ("expression.differential.v1" | "medical.bulk-rnaseq.v1", "csv" | "tsv") => {
            Some(AnalysisRoute {
                capability: if capability == "expression.differential.v1" {
                    "expression.differential.v1"
                } else {
                    "medical.bulk-rnaseq.v1"
                },
                input_role: "counts",
            })
        }
        ("comparative.synteny.visualize.v1", "tsv") => Some(AnalysisRoute {
            capability: "comparative.synteny.visualize.v1",
            input_role: "anchors",
        }),
        ("comparative.mcscanx.v1", "tsv") => Some(AnalysisRoute {
            capability: "comparative.mcscanx.v1",
            input_role: "gene-positions",
        }),
        ("comparative.kaks.v1", "axt") => Some(AnalysisRoute {
            capability: "comparative.kaks.v1",
            input_role: "codon-alignment",
        }),
        ("medical.cohort-table.qc.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "medical.cohort-table.qc.v1",
            input_role: "cohort",
        }),
        ("medical.pathway-ruo.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "medical.pathway-ruo.v1",
            input_role: "genes",
        }),
        ("medical.variant-cohort.v1", "vcf") => Some(AnalysisRoute {
            capability: "medical.variant-cohort.v1",
            input_role: "vcf",
        }),
        ("medical.single-cell-qc.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "medical.single-cell-qc.v1",
            input_role: "matrix",
        }),
        ("motif.visualize.v1", "meme-text") => Some(AnalysisRoute {
            capability: "motif.visualize.v1",
            input_role: "meme",
        }),
        ("alignment.bam-to-bigwig.v1", "bam" | "cram") => Some(AnalysisRoute {
            capability: "alignment.bam-to-bigwig.v1",
            input_role: "alignment",
        }),
        ("annotation.go.normalize.v1" | "annotation.eggnog.normalize.v1", "csv" | "tsv") => {
            Some(AnalysisRoute {
                capability: if capability == "annotation.go.normalize.v1" {
                    "annotation.go.normalize.v1"
                } else {
                    "annotation.eggnog.normalize.v1"
                },
                input_role: "annotations",
            })
        }
        (
            "enrichment.overrepresentation.v1" | "enrichment.go.v1" | "enrichment.kegg.v1",
            "csv" | "tsv",
        ) => Some(AnalysisRoute {
            capability: match capability {
                "enrichment.overrepresentation.v1" => "enrichment.overrepresentation.v1",
                "enrichment.go.v1" => "enrichment.go.v1",
                _ => "enrichment.kegg.v1",
            },
            input_role: "genes",
        }),
        ("enrichment.gsea.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "enrichment.gsea.v1",
            input_role: "ranked",
        }),
        ("enrichment.visualize.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "enrichment.visualize.v1",
            input_role: "genes",
        }),
        ("set.venn.v1" | "set.upset.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: if capability == "set.venn.v1" {
                "set.venn.v1"
            } else {
                "set.upset.v1"
            },
            input_role: "table",
        }),
        ("protein.properties.v1", "fasta") => Some(AnalysisRoute {
            capability: "protein.properties.v1",
            input_role: "fasta",
        }),
        ("similarity.blast.local.v1", "fasta") => Some(AnalysisRoute {
            capability: "similarity.blast.local.v1",
            input_role: "query",
        }),
        ("similarity.diamond.v1", "fasta") => Some(AnalysisRoute {
            capability: "similarity.diamond.v1",
            input_role: "query",
        }),
        ("similarity.hmmer.v1", "hmm-profile") => Some(AnalysisRoute {
            capability: "similarity.hmmer.v1",
            input_role: "profile",
        }),
        ("similarity.blast.parse.v1", "blast-tabular" | "blast-xml") => Some(AnalysisRoute {
            capability: "similarity.blast.parse.v1",
            input_role: "blast",
        }),
        ("similarity.reciprocal.v1", "blast-tabular" | "blast-xml") => Some(AnalysisRoute {
            capability: "similarity.reciprocal.v1",
            input_role: "forward",
        }),
        ("protein.domain.parse.v1", "protein-domains") => Some(AnalysisRoute {
            capability: "protein.domain.parse.v1",
            input_role: "domains",
        }),
        ("protein.domain.visualize.v1", "protein-domains") => Some(AnalysisRoute {
            capability: "protein.domain.visualize.v1",
            input_role: "domains",
        }),
        ("phylogeny.tree.transform.v1", "newick") => Some(AnalysisRoute {
            capability: "phylogeny.tree.transform.v1",
            input_role: "tree",
        }),
        ("msa.muscle.v1", "fasta") => Some(AnalysisRoute {
            capability: "msa.muscle.v1",
            input_role: "fasta",
        }),
        ("msa.trimal.v1", "fasta") => Some(AnalysisRoute {
            capability: "msa.trimal.v1",
            input_role: "alignment",
        }),
        ("phylogeny.iqtree.v1", "fasta") => Some(AnalysisRoute {
            capability: "phylogeny.iqtree.v1",
            input_role: "alignment",
        }),
        ("motif.meme.v1", "fasta") => Some(AnalysisRoute {
            capability: "motif.meme.v1",
            input_role: "fasta",
        }),
        ("table.manipulate.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "table.manipulate.v1",
            input_role: "table",
        }),
        ("variant.stats.v1" | "variant.filter.v1" | "variant.normalize.v1", "vcf") => {
            Some(AnalysisRoute {
                capability: match capability {
                    "variant.stats.v1" => "variant.stats.v1",
                    "variant.filter.v1" => "variant.filter.v1",
                    _ => "variant.normalize.v1",
                },
                input_role: "vcf",
            })
        }
        ("structure.pdb.summary.v1", "pdb") => Some(AnalysisRoute {
            capability: "structure.pdb.summary.v1",
            input_role: "pdb",
        }),
        ("structure.mmcif.summary.v1", "mmcif") => Some(AnalysisRoute {
            capability: "structure.mmcif.summary.v1",
            input_role: "structure",
        }),
        (
            "structure.sequence.extract.v1" | "structure.contact-map.v1" | "structure.geometry.v1",
            "pdb" | "mmcif",
        ) => Some(AnalysisRoute {
            capability: match capability {
                "structure.sequence.extract.v1" => "structure.sequence.extract.v1",
                "structure.contact-map.v1" => "structure.contact-map.v1",
                _ => "structure.geometry.v1",
            },
            input_role: "structure",
        }),
        ("structure.superpose.v1", "pdb" | "mmcif") => Some(AnalysisRoute {
            capability: "structure.superpose.v1",
            input_role: "reference",
        }),
        ("protein.secondary-structure.v1", "pdb" | "mmcif") => Some(AnalysisRoute {
            capability: "protein.secondary-structure.v1",
            input_role: "structure",
        }),
        ("medical.pharmacogenomics.v1", "vcf") => Some(AnalysisRoute {
            capability: "medical.pharmacogenomics.v1",
            input_role: "vcf",
        }),
        ("medical.metabolomics.v1", "mzml") => Some(AnalysisRoute {
            capability: "medical.metabolomics.v1",
            input_role: "mzml",
        }),
        ("medical.spatial-transcriptomics.v1", "mtx") => Some(AnalysisRoute {
            capability: "medical.spatial-transcriptomics.v1",
            input_role: "matrix",
        }),
        ("medical.microbiome.v1" | "metagenomics.classify.v1", "fasta" | "fastq") => {
            Some(AnalysisRoute {
                capability: if capability == "medical.microbiome.v1" {
                    "medical.microbiome.v1"
                } else {
                    "metagenomics.classify.v1"
                },
                input_role: "reads",
            })
        }
        ("medical.survival.v1", "csv" | "tsv") => Some(AnalysisRoute {
            capability: "medical.survival.v1",
            input_role: "cohort",
        }),
        ("chemistry.descriptors.v1", "sdf") => Some(AnalysisRoute {
            capability: "chemistry.descriptors.v1",
            input_role: "molecules",
        }),
        _ => None,
    }
}

fn capability_requires_secondary(capability: &str) -> bool {
    secondary_input_format(capability).is_some()
}

fn secondary_input_format(capability: &str) -> Option<&'static str> {
    match capability {
        "interval.intersect.v1" | "interval.subtract.v1" | "interval.closest.v1" => Some("bed"),
        "annotation.sequence.extract.v1" => Some("fasta"),
        "variant.normalize.v1" => Some("fasta"),
        "primer.epcr.v1" => Some("tsv"),
        "structure.superpose.v1" => Some("structure"),
        "similarity.reciprocal.v1" => Some("blast"),
        "comparative.mcscanx.v1" => Some("blast-tabular"),
        "expression.differential.v1" | "medical.bulk-rnaseq.v1" => Some("table"),
        "similarity.blast.local.v1" | "similarity.diamond.v1" | "similarity.hmmer.v1" => {
            Some("fasta")
        }
        "enrichment.overrepresentation.v1"
        | "enrichment.go.v1"
        | "enrichment.kegg.v1"
        | "enrichment.gsea.v1"
        | "medical.pathway-ruo.v1"
        | "enrichment.visualize.v1" => Some("table"),
        "medical.spatial-transcriptomics.v1" => Some("tsv"),
        _ => None,
    }
}

fn secondary_input_matches(capability: &str, format: &str) -> bool {
    match secondary_input_format(capability) {
        Some("structure") => matches!(format.trim().to_ascii_lowercase().as_str(), "pdb" | "mmcif"),
        Some("blast") => matches!(
            format.trim().to_ascii_lowercase().as_str(),
            "blast-tabular" | "blast-xml"
        ),
        Some("table") => matches!(format.trim().to_ascii_lowercase().as_str(), "csv" | "tsv"),
        Some(required) => format.trim().eq_ignore_ascii_case(required),
        None => false,
    }
}

fn secondary_input_role(capability: &str) -> Option<&'static str> {
    match capability {
        "interval.intersect.v1" | "interval.subtract.v1" => Some("right-bed"),
        "interval.closest.v1" => Some("target-bed"),
        "annotation.sequence.extract.v1" => Some("fasta"),
        "variant.normalize.v1" => Some("reference"),
        "primer.epcr.v1" => Some("primers"),
        "structure.superpose.v1" => Some("mobile"),
        "similarity.reciprocal.v1" => Some("reverse"),
        "comparative.mcscanx.v1" => Some("similarity-hits"),
        "expression.differential.v1" | "medical.bulk-rnaseq.v1" => Some("sample_metadata"),
        "similarity.blast.local.v1" | "similarity.diamond.v1" => Some("reference"),
        "similarity.hmmer.v1" => Some("sequences"),
        "enrichment.overrepresentation.v1"
        | "enrichment.go.v1"
        | "enrichment.kegg.v1"
        | "medical.pathway-ruo.v1"
        | "enrichment.visualize.v1" => Some("associations"),
        "enrichment.gsea.v1" => Some("gene-sets"),
        "medical.spatial-transcriptomics.v1" => Some("features"),
        _ => None,
    }
}

fn tertiary_input_format(capability: &str) -> Option<&'static str> {
    match capability {
        "medical.spatial-transcriptomics.v1" => Some("tsv"),
        _ => None,
    }
}

fn tertiary_input_role(capability: &str) -> Option<&'static str> {
    match capability {
        "medical.spatial-transcriptomics.v1" => Some("barcodes"),
        _ => None,
    }
}

fn capability_output_extension(capability: &str) -> Option<&'static str> {
    match capability {
        "fastq.trim.v1" | "fastq.adapter.v1" | "fastq.deduplicate.v1" => Some("fastq"),
        "interval.merge.v1" | "interval.subtract.v1" => Some("bed"),
        "interval.closest.v1" => Some("tsv"),
        "table.manipulate.v1" => Some("tsv"),
        "annotation.gxf.normalize.v1" => Some("gff3"),
        "annotation.gene-position.v1" => Some("tsv"),
        "annotation.sequence.extract.v1" => Some("fasta"),
        "annotation.go.normalize.v1" | "annotation.eggnog.normalize.v1" => Some("tsv"),
        "comparative.mcscanx.v1" => Some("collinearity"),
        "comparative.kaks.v1" => Some("tsv"),
        "sequence.kmer.count.v1" | "primer.epcr.v1" => Some("tsv"),
        "expression.normalize.v1" => Some("tsv"),
        "variant.filter.v1" | "variant.normalize.v1" => Some("vcf"),
        "phylogeny.tree.transform.v1" => Some("nwk"),
        "similarity.blast.local.v1" | "similarity.diamond.v1" => Some("tsv"),
        "similarity.hmmer.v1" => Some("domtblout"),
        "msa.muscle.v1" => Some("fasta"),
        "msa.trimal.v1" => Some("fasta"),
        "phylogeny.iqtree.v1" => Some("nwk"),
        "motif.meme.v1" => Some("meme"),
        "alignment.bam-to-bigwig.v1" => Some("bw"),
        "protein.secondary-structure.v1" => Some("dssp"),
        "annotation.structure.visualize.v1"
        | "comparative.synteny.visualize.v1"
        | "enrichment.visualize.v1"
        | "protein.domain.visualize.v1"
        | "expression.volcano.v1"
        | "motif.visualize.v1" => Some("svg"),
        "medical.pharmacogenomics.v1"
        | "medical.spatial-transcriptomics.v1"
        | "medical.metabolomics.v1"
        | "medical.microbiome.v1"
        | "metagenomics.classify.v1" => Some("tsv"),
        _ => None,
    }
}

fn derived_analysis_output_path(input_path: &str, capability: &str, extension: &str) -> PathBuf {
    let input = Path::new(input_path);
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("linxira-output");
    let operation = capability
        .strip_suffix(".v1")
        .unwrap_or(capability)
        .replace('.', "-");
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    parent.join(format!("{stem}.{operation}.{millis}.{extension}"))
}

fn build_analysis_request(job_id: &str, route: AnalysisRoute, input_path: &str) -> JobRequest {
    let mut inputs = BTreeMap::new();
    inputs.insert(route.input_role.to_owned(), input_path.to_owned());
    JobRequest {
        schema_version: SCHEMA_VERSION.to_owned(),
        job_id: job_id.to_owned(),
        capability: route.capability.to_owned(),
        inputs,
        execution: ExecutionRequest {
            mode: ExecutionMode::LocalCpu,
        },
        parameters: serde_json::json!({}),
    }
}

fn run_inspection_task(task: InspectionTask) -> InspectionMessage {
    let mut inputs = BTreeMap::new();
    inputs.insert("file".to_owned(), task.path);
    let request = JobRequest {
        schema_version: SCHEMA_VERSION.to_owned(),
        job_id: new_job_id(),
        capability: "dataset.inspect.v1".to_owned(),
        inputs,
        execution: ExecutionRequest {
            mode: ExecutionMode::LocalCpu,
        },
        parameters: serde_json::json!({
            "dataset_id": task.dataset_id,
            "max_preview_records": 200,
            "max_preview_bytes": 10_485_760_u64,
        }),
    };
    InspectionMessage {
        generation: task.generation,
        dataset_id: task.dataset_id,
        result: run_worker_request(request),
    }
}

fn run_worker_request(request: JobRequest) -> UiJobResult {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute_request(request, Path::new(".")).map_err(|error| error.to_string())
    })) {
        Ok(result) => result,
        Err(_) => Err("background worker panicked".to_owned()),
    }
}

fn generation_matches(message_generation: u64, project_generation: u64) -> bool {
    message_generation == project_generation
}

fn analysis_result_matches(result: &Value, job_id: &str, capability: &str) -> bool {
    result.get("job_id").and_then(Value::as_str) == Some(job_id)
        && result.get("capability").and_then(Value::as_str) == Some(capability)
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Csv,
    Tsv,
    Json,
    Xlsx,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Json => "json",
            Self::Xlsx => "xlsx",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Json => "JSON",
            Self::Xlsx => "XLSX",
        }
    }
}

fn configure_style(context: &egui::Context) {
    context.set_theme(egui::ThemePreference::Light);
    let mut style = (*context.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size = egui::vec2(38.0, 30.0);
    style.spacing.combo_width = 180.0;

    // Quiet scrollbars: they appear on hover, use a neutral handle color,
    // and keep a consistent width across every scroll area.
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating_allocated_width = 4.0;
    style.spacing.scroll.handle_min_length = 24.0;
    style.spacing.scroll.foreground_color = false;

    let visuals = &mut style.visuals;
    visuals.panel_fill = theme::PANEL_BG;
    visuals.window_fill = theme::WINDOW_BG;
    visuals.faint_bg_color = theme::FAINT_BG;
    visuals.extreme_bg_color = theme::ELEVATED_BG;
    visuals.code_bg_color = theme::CODE_BG;
    visuals.selection.bg_fill = theme::ACCENT_SOFT;
    visuals.hyperlink_color = theme::LINK;
    visuals.warn_fg_color = theme::WARNING;
    visuals.error_fg_color = theme::DANGER;
    visuals.widgets.inactive.corner_radius = theme::CORNER_RADIUS;
    visuals.widgets.hovered.corner_radius = theme::CORNER_RADIUS;
    visuals.widgets.active.corner_radius = theme::CORNER_RADIUS;
    visuals.widgets.open.corner_radius = theme::CORNER_RADIUS;
    visuals.widgets.inactive.weak_bg_fill = theme::WIDGET_BG;
    visuals.widgets.hovered.weak_bg_fill = theme::WIDGET_BG_HOVER;
    context.set_style_of(egui::Theme::Light, style);
}

fn nav_button(ui: &mut egui::Ui, page: &mut Page, target: Page, label: &str) {
    let selected = *page == target;
    if ui
        .add_sized(
            [ui.available_width(), theme::NAV_BUTTON_HEIGHT],
            egui::Button::new(label).selected(selected),
        )
        .clicked()
    {
        *page = target;
    }
}

fn workspace_tab_button(
    ui: &mut egui::Ui,
    tab: &mut WorkspaceTab,
    target: WorkspaceTab,
    label: &str,
) {
    if ui
        .add_sized(
            [ui.available_width(), theme::TAB_BUTTON_HEIGHT],
            egui::Button::new(label).selected(*tab == target),
        )
        .clicked()
    {
        *tab = target;
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        let bar = ui
            .allocate_exact_size(egui::vec2(3.0, 16.0), egui::Sense::hover())
            .0;
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), theme::ACCENT_STRONG);
        ui.label(egui::RichText::new(title).strong().size(theme::TITLE_SIZE));
    });
}

/// A titled, uniformly padded result container. Every result section
/// (statistics, chart preview, export) shares this panel so figures and
/// summaries look coherent regardless of the capability that produced them.
fn result_panel(
    ui: &mut egui::Ui,
    title: &str,
    caption: Option<&str>,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(theme::ELEVATED_BG)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(theme::CORNER_RADIUS)
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .size(theme::SUBTITLE_SIZE),
            );
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(6.0);
            content(ui);
            if let Some(caption) = caption {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(caption)
                        .size(theme::SMALL_SIZE)
                        .color(theme::TEXT_MUTED),
                );
            }
        });
}

fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    action: &str,
    tab: &mut WorkspaceTab,
    target: WorkspaceTab,
) {
    ui.add_space(24.0);
    egui::Frame::NONE
        .fill(theme::PANEL_BG)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(theme::CORNER_RADIUS)
        .inner_margin(egui::Margin::symmetric(24, 28))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .size(theme::SUBTITLE_SIZE)
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(10.0);
                if ui.button(action).clicked() {
                    *tab = target;
                }
            });
        });
}

fn inspection_payload(result: &Value) -> &Value {
    let payload = result.get("result").unwrap_or(result);
    payload.get("manifest").unwrap_or(payload)
}

fn first_diagnostic_message(result: &Value) -> Option<String> {
    let payload = inspection_payload(result);
    ["errors", "warnings"]
        .iter()
        .find_map(|key| {
            payload
                .get(*key)
                .and_then(Value::as_array)
                .and_then(|issues| issues.first())
                .and_then(|issue| issue.get("message"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            payload
                .get("validation")
                .and_then(|validation| validation.get("diagnostics"))
                .and_then(Value::as_array)
                .and_then(|diagnostics| diagnostics.first())
                .and_then(|diagnostic| diagnostic.get("message"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            result
                .get("diagnostics")
                .and_then(Value::as_array)
                .and_then(|diagnostics| diagnostics.first())
                .and_then(|diagnostic| diagnostic.get("message"))
                .and_then(Value::as_str)
        })
        .map(str::to_owned)
}

fn inspection_state(result: &Value) -> DatasetState {
    let payload = inspection_payload(result);
    let has_errors = payload
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|issues| !issues.is_empty());
    if has_errors || payload.get("support").and_then(Value::as_str) == Some("unknown") {
        return DatasetState::Invalid;
    }
    if payload.get("support").and_then(Value::as_str) != Some("supported") {
        return DatasetState::Warning;
    }
    let has_warnings = payload
        .get("warnings")
        .and_then(Value::as_array)
        .is_some_and(|issues| !issues.is_empty());
    if has_warnings {
        DatasetState::Warning
    } else {
        DatasetState::Ready
    }
}

fn inspection_is_runnable(result: &Value) -> bool {
    let payload = inspection_payload(result);
    payload.get("support").and_then(Value::as_str) == Some("supported")
        && payload
            .get("errors")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
}

fn lookup_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn lookup_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn files_array(payload: &Value) -> Option<&Vec<Value>> {
    payload.get("files").and_then(Value::as_array).or_else(|| {
        payload
            .get("dataset")
            .and_then(|dataset| dataset.get("files"))
            .and_then(Value::as_array)
    })
}

fn first_file_field<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    files_array(payload)
        .and_then(|files| files.first())
        .and_then(|file| file.get(key))
        .and_then(Value::as_str)
}

fn first_file_u64(payload: &Value, key: &str) -> Option<u64> {
    files_array(payload)
        .and_then(|files| files.first())
        .and_then(|file| file.get(key))
        .and_then(Value::as_u64)
}

fn detected_format(payload: &Value) -> Option<&str> {
    lookup_string(payload, &["format", "detected_format", "data_format"])
        .or_else(|| first_file_field(payload, "format"))
}

fn dataset_detected_format(dataset: &DatasetEntry) -> &str {
    dataset
        .inspection
        .as_ref()
        .and_then(|inspection| detected_format(inspection_payload(inspection)))
        .unwrap_or(&dataset.format_hint)
}

fn find_preview(payload: &Value) -> Option<&Value> {
    ["preview", "records", "rows", "sample"]
        .iter()
        .find_map(|key| payload.get(*key))
        .or_else(|| {
            payload
                .get("summary")
                .and_then(|summary| summary.get("preview"))
        })
}

fn show_diagnostics(ui: &mut egui::Ui, payload: &Value, language: Language) {
    let direct_issues = [
        ("errors", DatasetState::Invalid.color()),
        ("warnings", DatasetState::Warning.color()),
    ];
    let has_direct_issues = direct_issues.iter().any(|(key, _)| {
        payload
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|issues| !issues.is_empty())
    });
    let diagnostics = payload
        .get("validation")
        .and_then(|validation| validation.get("diagnostics"))
        .and_then(Value::as_array)
        .or_else(|| payload.get("diagnostics").and_then(Value::as_array));
    if !has_direct_issues && diagnostics.is_none_or(Vec::is_empty) {
        return;
    }
    ui.add_space(12.0);
    section_title(ui, language.text("检查信息", "Diagnostics"));
    for (key, color) in direct_issues {
        if let Some(issues) = payload.get(key).and_then(Value::as_array) {
            for issue in issues {
                let message = issue.get("message").and_then(Value::as_str).unwrap_or("-");
                let line = issue.get("line").and_then(Value::as_u64);
                let message = line
                    .map(|line| format!("{message} ({line})"))
                    .unwrap_or_else(|| message.to_owned());
                ui.colored_label(color, message);
            }
        }
    }
    for diagnostic in diagnostics.into_iter().flatten() {
        let severity = diagnostic
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or("info");
        let color = match severity {
            "error" => DatasetState::Invalid.color(),
            "warning" => DatasetState::Warning.color(),
            _ => DatasetState::Inspecting.color(),
        };
        let message = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let location = diagnostic
            .get("line")
            .and_then(Value::as_u64)
            .map(|line| format!("{} {line}: ", language.text("行", "line")))
            .or_else(|| {
                diagnostic
                    .get("record")
                    .and_then(Value::as_u64)
                    .map(|record| format!("{} {record}: ", language.text("记录", "record")))
            })
            .unwrap_or_default();
        ui.colored_label(color, format!("{location}{message}"));
        if let Some(hint) = diagnostic.get("hint").and_then(Value::as_str) {
            ui.small(hint);
        }
    }
}

fn render_value_preview(ui: &mut egui::Ui, value: &Value, language: Language) {
    match value {
        Value::Array(rows) if rows.is_empty() => {
            ui.small(language.text("没有可预览记录", "No preview records"));
        }
        Value::Array(rows) if rows.iter().all(Value::is_object) => {
            let mut columns = Vec::<String>::new();
            for row in rows.iter().take(20) {
                if let Some(object) = row.as_object() {
                    for key in object.keys() {
                        if !columns.contains(key) && columns.len() < 8 {
                            columns.push(key.clone());
                        }
                    }
                }
            }
            egui::ScrollArea::horizontal()
                .id_salt("dataset-preview-table")
                .show(ui, |ui| {
                    egui::Grid::new("dataset-preview-grid")
                        .striped(true)
                        .min_col_width(110.0)
                        .show(ui, |ui| {
                            for column in &columns {
                                ui.strong(column);
                            }
                            ui.end_row();
                            for row in rows.iter().take(20) {
                                for column in &columns {
                                    ui.label(compact_value(&row[column], 80));
                                }
                                ui.end_row();
                            }
                        });
                });
            if rows.len() > 20 {
                ui.small(format!(
                    "{} 20 / {}",
                    language.text("预览", "Preview"),
                    rows.len()
                ));
            }
        }
        Value::Array(rows) => {
            egui::Grid::new("dataset-preview-list")
                .striped(true)
                .show(ui, |ui| {
                    for (index, row) in rows.iter().take(20).enumerate() {
                        ui.monospace(format!("{}", index + 1));
                        ui.label(compact_value(row, 160));
                        ui.end_row();
                    }
                });
        }
        Value::Object(object) => {
            egui::Grid::new("dataset-preview-object")
                .striped(true)
                .min_col_width(150.0)
                .show(ui, |ui| {
                    for (key, item) in object.iter().take(24) {
                        ui.label(key);
                        ui.label(compact_value(item, 180));
                        ui.end_row();
                    }
                });
        }
        Value::Null => {
            ui.small(language.text("没有可预览内容", "No preview available"));
        }
        _ => {
            ui.monospace(compact_value(value, 300));
        }
    }
}

fn render_metrics(ui: &mut egui::Ui, payload: &Value, language: Language) {
    let Some(values) = payload.as_object() else {
        render_value_preview(ui, payload, language);
        return;
    };
    egui::Grid::new("analysis-metrics")
        .striped(true)
        .min_col_width(180.0)
        .show(ui, |ui| {
            for (key, value) in values {
                ui.label(metric_label(key, language));
                ui.monospace(compact_value(value, 120));
                ui.end_row();
            }
        });
}

fn compact_value(value: &Value, max_chars: usize) -> String {
    let rendered = match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    };
    if rendered.chars().count() <= max_chars {
        rendered
    } else {
        let mut truncated = rendered
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>();
        truncated.push('…');
        truncated
    }
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn importable_file_path(path: PathBuf) -> Result<PathBuf, (PathBuf, ImportPathIssue)> {
    let drive_relative = looks_like_drive_relative_path(&path);
    let resolved = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if resolved.is_file() {
        if resolved.to_str().is_none() {
            Err((resolved, ImportPathIssue::NonUtf8))
        } else {
            Ok(resolved)
        }
    } else if resolved.is_dir() {
        Err((resolved, ImportPathIssue::Directory))
    } else if drive_relative {
        Err((path, ImportPathIssue::DriveRelative))
    } else {
        Err((path, ImportPathIssue::Unreadable))
    }
}

fn looks_like_drive_relative_path(path: &Path) -> bool {
    let value = path.as_os_str().to_string_lossy();
    let bytes = value.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && !matches!(bytes[2], b'\\' | b'/')
}

fn import_path_error(path: &Path, issue: ImportPathIssue, language: Language) -> String {
    match (language, issue) {
        (Language::ZhCn, ImportPathIssue::DriveRelative) => format!(
            "无法导入：路径 {} 缺少盘符后的分隔符，或反斜杠已被 shell 转义。请使用“选择文件”，或用双引号包住完整 Windows 路径。",
            path.display()
        ),
        (Language::EnUs, ImportPathIssue::DriveRelative) => format!(
            "Cannot import: {} is missing the separator after its drive letter, or its backslashes were consumed by the shell. Use Choose files, or quote the complete Windows path.",
            path.display()
        ),
        (Language::ZhCn, ImportPathIssue::Directory) => format!(
            "无法导入：{} 是目录；请选择 tests\\fixtures 下的具体数据文件。",
            path.display()
        ),
        (Language::EnUs, ImportPathIssue::Directory) => format!(
            "Cannot import: {} is a directory; select a specific data file under tests\\fixtures.",
            path.display()
        ),
        (Language::ZhCn, ImportPathIssue::NonUtf8) => format!(
            "无法导入：路径 {} 不是有效的 UTF-8。当前项目和执行协议使用 JSON 路径；请先将文件重命名或移动到 UTF-8 路径。",
            path.display()
        ),
        (Language::EnUs, ImportPathIssue::NonUtf8) => format!(
            "Cannot import: {} is not valid UTF-8. Project files and the execution protocol use JSON paths; rename or move the file to a UTF-8 path first.",
            path.display()
        ),
        (Language::ZhCn, ImportPathIssue::Unreadable) => {
            format!("无法导入：{} 不是可读取文件。", path.display())
        }
        (Language::EnUs, ImportPathIssue::Unreadable) => {
            format!("Cannot import: {} is not a readable file.", path.display())
        }
    }
}

fn format_hint(path: &Path) -> &'static str {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let uncompressed = name
        .strip_suffix(".gz")
        .or_else(|| name.strip_suffix(".bgz"))
        .unwrap_or(&name);
    let extension = uncompressed
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("");
    match extension {
        "fa" | "fasta" | "fna" | "ffn" | "faa" | "frn" => "fasta",
        "fq" | "fastq" => "fastq",
        "csv" => "csv",
        "tsv" | "tab" => "tsv",
        "bed" => "bed",
        "gff" | "gff3" => "gff3",
        "gtf" => "gtf",
        "vcf" => "vcf",
        "sam" => "sam",
        "bam" => "bam",
        "bcf" => "bcf",
        "cram" => "cram",
        "h5ad" => "h5ad",
        "loom" => "loom",
        "rds" => "rds",
        "pdb" => "pdb",
        "cif" | "mmcif" => "mmcif",
        "blast" | "m8" => "blast-tabular",
        "xml" => "blast-xml",
        "domtblout" => "protein-domains",
        "hmm" => "hmm-profile",
        "axt" => "axt",
        "collinearity" => "mcscanx-collinearity",
        "nwk" | "newick" | "tree" | "tre" => "newick",
        "xlsx" => "xlsx",
        "zip" => "zip",
        _ => "unknown",
    }
}

fn capability_title(capability: &str, language: Language) -> &'static str {
    match capability {
        "sequence.stats.v1" => language.text("FASTA 序列统计", "FASTA sequence statistics"),
        "sequence.kmer.count.v1" => language.text("精确 k-mer 计数", "Exact k-mer counting"),
        "primer.epcr.v1" => language.text("简单电子 PCR", "Simple electronic PCR"),
        "fastq.qc.v1" => language.text("FASTQ 质量控制", "FASTQ quality control"),
        "fastq.trim.v1" => language.text("FASTQ 质量裁剪", "FASTQ quality trimming"),
        "fastq.adapter.v1" => language.text("FASTQ 接头去除", "FASTQ adapter removal"),
        "fastq.deduplicate.v1" => language.text("FASTQ 精确去重", "FASTQ exact deduplication"),
        "interval.intersect.v1" => language.text("BED 区间相交", "BED interval intersection"),
        "interval.merge.v1" => language.text("BED 区间合并", "BED interval merge"),
        "interval.subtract.v1" => language.text("BED 区间扣除", "BED interval subtraction"),
        "interval.closest.v1" => language.text("BED 最近区间", "BED nearest interval"),
        "variant.stats.v1" => language.text("变异统计", "Variant statistics"),
        "variant.filter.v1" => language.text("VCF 基础过滤", "Basic VCF filtering"),
        "variant.normalize.v1" => {
            language.text("VCF 参考规范化", "Reference-guided VCF normalization")
        }
        "alignment.qc.v1" => language.text("比对质量控制", "Alignment quality control"),
        "alignment.bam-to-bigwig.v1" => language.text("BAM/CRAM 转 BigWig", "BAM/CRAM to BigWig"),
        "annotation.gxf.stats.v1" => language.text("注释统计", "Annotation statistics"),
        "annotation.gxf.normalize.v1" => language.text("注释规范化", "Annotation normalization"),
        "annotation.gene-position.v1" => language.text("基因位置表", "Gene position table"),
        "annotation.sequence.extract.v1" => {
            language.text("按注释提取序列", "Annotation-guided extraction")
        }
        "annotation.structure.visualize.v1" => {
            language.text("注释结构图", "Annotation structure plot")
        }
        "comparative.synteny.visualize.v1" => language.text("共线性锚点图", "Synteny anchor plot"),
        "comparative.mcscanx.v1" => {
            language.text("基因组共线性分析", "Genome collinearity analysis")
        }
        "comparative.kaks.v1" => language.text("Ka/Ks 计算", "Ka/Ks calculation"),
        "annotation.go.normalize.v1" => {
            language.text("GO 注释规范化", "GO annotation normalization")
        }
        "annotation.eggnog.normalize.v1" => {
            language.text("eggNOG 注释规范化", "eggNOG annotation normalization")
        }
        "enrichment.overrepresentation.v1" => {
            language.text("通用过度富集分析", "Generic over-representation analysis")
        }
        "enrichment.go.v1" => language.text("GO 富集分析", "GO enrichment analysis"),
        "enrichment.kegg.v1" => language.text("KEGG 富集分析", "KEGG enrichment analysis"),
        "enrichment.gsea.v1" => language.text("预排序 GSEA", "Preranked GSEA"),
        "enrichment.visualize.v1" => language.text("富集结果绘图", "Enrichment visualization"),
        "genome.gene-density.v1" => language.text("基因组特征密度", "Genome feature density"),
        "expression.matrix.qc.v1" => language.text("表达矩阵", "Expression matrix"),
        "expression.differential.v1" => {
            language.text("批量 RNA-seq 差异表达", "Bulk differential expression")
        }
        "medical.bulk-rnaseq.v1" => {
            language.text("批量 RNA-seq 科研分析", "Bulk RNA-seq research workflow")
        }
        "medical.cohort-table.qc.v1" => {
            language.text("研究队列表质量控制", "Research cohort table QC")
        }
        "medical.pathway-ruo.v1" => {
            language.text("研究队列通路分析", "Research cohort pathway analysis")
        }
        "medical.variant-cohort.v1" => {
            language.text("研究队列变异汇总", "Research cohort variant aggregation")
        }
        "medical.single-cell-qc.v1" => {
            language.text("单细胞计数矩阵质量控制", "Single-cell count matrix QC")
        }
        "expression.normalize.v1" => language.text("表达矩阵标准化", "Expression normalization"),
        "expression.pca.v1" => language.text("表达矩阵 PCA", "Expression PCA"),
        "expression.cluster.v1" => language.text("表达矩阵聚类", "Expression clustering"),
        "expression.heatmap.v1" => language.text("聚类表达热图", "Clustered expression heatmap"),
        "expression.volcano.v1" => {
            language.text("差异表达火山图", "Differential expression volcano plot")
        }
        "set.venn.v1" => language.text("2–6 集合 Venn 分析", "Two-to-six-set Venn analysis"),
        "set.upset.v1" => language.text("多集合 UpSet 分析", "Multi-set UpSet analysis"),
        "protein.properties.v1" => language.text("蛋白理化性质", "Protein properties"),
        "similarity.blast.local.v1" => language.text("本地 BLAST+ 搜索", "Local BLAST+ search"),
        "similarity.diamond.v1" => language.text("DIAMOND 相似性搜索", "DIAMOND similarity search"),
        "similarity.hmmer.v1" => language.text("HMMER profile 搜索", "HMMER profile search"),
        "similarity.blast.parse.v1" => language.text("BLAST 结果解析", "BLAST result parsing"),
        "similarity.reciprocal.v1" => language.text("双向最佳命中", "Reciprocal best hits"),
        "protein.domain.parse.v1" => {
            language.text("蛋白结构域结果解析", "Protein domain result parsing")
        }
        "protein.domain.visualize.v1" => {
            language.text("蛋白结构域架构图", "Protein domain architecture plot")
        }
        "phylogeny.tree.transform.v1" => {
            language.text("系统发育树转换", "Phylogeny tree transform")
        }
        "phylogeny.iqtree.v1" => {
            language.text("IQ-TREE 系统发育推断", "IQ-TREE phylogeny inference")
        }
        "msa.muscle.v1" => language.text("MUSCLE 多序列比对", "MUSCLE multiple sequence alignment"),
        "msa.trimal.v1" => language.text("trimAl 比对裁剪", "trimAl alignment trimming"),
        "motif.meme.v1" => language.text("MEME 基序发现", "MEME motif discovery"),
        "table.manipulate.v1" => language.text("表格处理", "Table manipulation"),
        "structure.pdb.summary.v1" => language.text("PDB 结构摘要", "PDB structure summary"),
        "structure.viewer.v1" => language.text("交互式结构查看器", "Interactive structure viewer"),
        "structure.mmcif.summary.v1" => language.text("mmCIF 结构摘要", "mmCIF structure summary"),
        "structure.sequence.extract.v1" => {
            language.text("坐标序列提取", "Coordinate sequence extraction")
        }
        "structure.contact-map.v1" => language.text("残基接触图", "Residue contact map"),
        "structure.geometry.v1" => language.text("结构几何测量", "Structure geometry"),
        "structure.superpose.v1" => language.text("结构刚体叠合", "Structure superposition"),
        "protein.secondary-structure.v1" => {
            language.text("DSSP 二级结构", "DSSP secondary structure")
        }
        "medical.pharmacogenomics.v1" => language.text(
            "药物基因组等位基因解读",
            "Pharmacogenomic allele interpretation",
        ),
        "medical.spatial-transcriptomics.v1" => {
            language.text("空间转录组矩阵汇总", "Spatial transcriptomics summary")
        }
        "medical.microbiome.v1" => language.text("微生物组 α 多样性", "Microbiome alpha diversity"),
        "metagenomics.classify.v1" => language.text("宏基因组分类", "Metagenomic classification"),
        "medical.metabolomics.v1" => language.text("代谢组峰值检测", "Metabolomics peak detection"),
        "medical.survival.v1" => language.text("生存分析 (Cox)", "Survival analysis (Cox)"),
        "chemistry.descriptors.v1" => language.text("分子理化描述符", "Molecular descriptors"),
        _ => language.text("未知能力", "Unknown capability"),
    }
}

fn new_dataset_id(index: usize) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("dataset-{millis}-{index}")
}

fn show_environment_audit(ui: &mut egui::Ui, audit: &Value, language: Language) {
    if let Some(platform) = audit.get("platform") {
        ui.label(format!(
            "{}: {} {}",
            language.text("平台", "Platform"),
            platform
                .get("family")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            platform
                .get("arch")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
    }
    if let Some(backends) = audit.get("execution_backends") {
        let ready = backends
            .get("ready")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let available = backends
            .get("available")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        ui.horizontal(|ui| {
            ui.strong(language.text("执行后端", "Execution backend"));
            ui.label(if ready {
                language.text("已就绪", "Ready")
            } else {
                language.text("未就绪", "Not ready")
            });
            if !available.is_empty() {
                ui.monospace(available);
            }
        });
        ui.small(match language {
            Language::ZhCn if cfg!(target_os = "windows") => {
                "Windows 需要 WSL Arch、WSL Debian 或 Docker 中的任意一个"
            }
            Language::ZhCn => "Linux 分别检查 Docker 和 Podman，任意一个可作为本地容器后端",
            Language::EnUs => backends
                .get("policy")
                .and_then(Value::as_str)
                .unwrap_or("Unknown backend policy"),
        });
    }
    if let Some(conda) = audit.get("conda").filter(|value| !value.is_null()) {
        ui.add_space(6.0);
        ui.strong(language.text("Conda / Bioconda", "Conda / Bioconda"));
        let distribution = conda
            .get("distribution")
            .and_then(Value::as_str)
            .unwrap_or("conda");
        let version = conda.get("version").and_then(Value::as_str).unwrap_or("-");
        ui.label(format!("{distribution} {version}"));
        ui.monospace(
            conda
                .get("root_prefix")
                .and_then(Value::as_str)
                .unwrap_or("-"),
        );
        let bioconda = conda
            .get("bioconda_configured")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let strict = conda
            .get("strict_channel_priority")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let order_valid = conda
            .get("channel_order_valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let native_supported = conda
            .get("bioconda_native_supported")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Bioconda: {}",
                if bioconda {
                    language.text("已配置", "configured")
                } else {
                    language.text("未配置", "missing")
                }
            ));
            ui.separator();
            ui.label(format!(
                "{}: {}",
                language.text("严格通道优先级", "strict channel priority"),
                if strict {
                    language.text("是", "yes")
                } else {
                    language.text("否", "no")
                }
            ));
            ui.separator();
            ui.label(format!(
                "{}: {}",
                language.text("通道顺序", "channel order"),
                if order_valid {
                    language.text("正确", "valid")
                } else {
                    language.text("需修复", "invalid")
                }
            ));
        });
        if !native_supported {
            ui.colored_label(
                theme::WARNING_AMBER,
                language.text(
                    "Bioconda 不提供原生 Windows 包；请通过 WSL Arch 或 WSL Debian 运行。",
                    "Bioconda does not publish native Windows packages; use WSL Arch or WSL Debian.",
                ),
            );
        }
    }
    ui.add_space(6.0);
    egui::Grid::new("environment-audit-tools")
        .striped(true)
        .min_col_width(140.0)
        .show(ui, |ui| {
            ui.strong(language.text("工具", "Tool"));
            ui.strong(language.text("状态", "Status"));
            ui.strong(language.text("版本", "Version"));
            ui.end_row();
            if let Some(tools) = audit.get("tools").and_then(Value::as_array) {
                for tool in tools {
                    ui.label(
                        tool.get("display_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown"),
                    );
                    let available = tool
                        .get("available")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let discovered = tool
                        .get("discovered_outside_path")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    ui.label(if available && discovered {
                        language.text("已定位（未进 PATH）", "Located (not on PATH)")
                    } else if available {
                        language.text("可用", "Available")
                    } else {
                        language.text("缺失", "Missing")
                    });
                    ui.monospace(tool.get("version").and_then(Value::as_str).unwrap_or("-"));
                    ui.end_row();
                }
            }
        });

    if let Some(warnings) = audit.get("warnings").and_then(Value::as_array) {
        for warning in warnings.iter().filter_map(Value::as_str) {
            ui.colored_label(theme::WARNING_AMBER, warning);
        }
    }
}

fn show_environment_plan(ui: &mut egui::Ui, plan: &Value, language: Language) {
    let profile = plan
        .get("profile")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mode = plan
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("managed-user");
    ui.horizontal_wrapped(|ui| {
        ui.strong(language.text("工作负载", "Workload"));
        ui.monospace(profile);
        ui.separator();
        ui.strong(language.text("模式", "Mode"));
        ui.label(localized_environment_mode(mode, language));
        ui.separator();
        ui.label(language.text("只读预览", "Read-only preview"));
    });
    if let Some(description) = localized_profile_description(profile, language)
        .or_else(|| plan.get("description").and_then(Value::as_str))
    {
        ui.label(description);
    }
    ui.add_space(6.0);
    egui::ScrollArea::horizontal()
        .id_salt("environment-plan-actions-scroll")
        .show(ui, |ui| {
            egui::Grid::new("environment-plan-actions")
                .striped(true)
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.strong(language.text("工具", "Tool"));
                    ui.strong(language.text("操作", "Action"));
                    ui.strong(language.text("执行后端", "Provider"));
                    ui.strong(language.text("方式", "Method"));
                    ui.strong(language.text("包/运行时", "Package"));
                    ui.end_row();
                    if let Some(actions) = plan.get("actions").and_then(Value::as_array) {
                        for action in actions {
                            ui.label(
                                action
                                    .get("display_name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown"),
                            );
                            ui.label(
                                match action
                                    .get("state")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown")
                                {
                                    "available" => language.text("已可用", "available"),
                                    "install" => language.text("需安装", "install"),
                                    "alternative" => language.text("备选后端", "backend option"),
                                    "missing" => {
                                        language.text("缺失（不安装）", "missing (no install)")
                                    }
                                    "unsupported" => language.text("不支持", "unsupported"),
                                    _ => language.text("未知", "unknown"),
                                },
                            );
                            ui.label(
                                action
                                    .get("execution_provider")
                                    .and_then(Value::as_str)
                                    .unwrap_or("-"),
                            );
                            ui.label(
                                action
                                    .get("strategy")
                                    .and_then(Value::as_str)
                                    .unwrap_or("-"),
                            );
                            ui.monospace(
                                action.get("package").and_then(Value::as_str).unwrap_or("-"),
                            );
                            ui.end_row();
                        }
                    }
                });
        });

    if let Some(transaction) = plan.get("transaction") {
        ui.add_space(10.0);
        ui.separator();
        ui.strong(language.text("事务边界", "Transaction boundary"));
        egui::ScrollArea::horizontal()
            .id_salt("environment-transaction-boundary-scroll")
            .show(ui, |ui| {
                egui::Grid::new("environment-transaction-boundary")
                    .num_columns(2)
                    .min_col_width(150.0)
                    .show(ui, |ui| {
                        for (label_zh, label_en, key) in [
                            ("目标目录", "Target root", "target_root"),
                            ("共享缓存", "Shared cache", "cache_root"),
                            ("运行时锁", "Runtime lock", "lock_path"),
                            ("校验策略", "Checksum policy", "checksum_policy"),
                            ("许可证策略", "License policy", "license_policy"),
                            ("激活策略", "Activation policy", "activation_policy"),
                        ] {
                            ui.label(language.text(label_zh, label_en));
                            ui.monospace(
                                transaction.get(key).and_then(Value::as_str).unwrap_or("-"),
                            );
                            ui.end_row();
                        }
                        ui.label(language.text("保留现有环境", "Preserve existing"));
                        ui.label(localized_boolean(
                            transaction
                                .get("preserves_existing")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            language,
                        ));
                        ui.end_row();
                        ui.label(language.text("系统级变更", "System mutation"));
                        ui.label(localized_boolean(
                            transaction
                                .get("system_mutation")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            language,
                        ));
                        ui.end_row();
                        ui.label(language.text("需要管理员权限", "Administrator required"));
                        ui.label(localized_boolean(
                            transaction
                                .get("requires_admin")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            language,
                        ));
                        ui.end_row();
                    });
            });

        if let Some(stages) = transaction.get("stages").and_then(Value::as_array)
            && !stages.is_empty()
        {
            ui.add_space(6.0);
            ui.strong(language.text("计划阶段", "Planned stages"));
            ui.horizontal_wrapped(|ui| {
                for stage in stages {
                    ui.monospace(stage.get("id").and_then(Value::as_str).unwrap_or("unknown"));
                }
            });
        }
        if let Some(blockers) = transaction.get("blockers").and_then(Value::as_array) {
            for blocker in blockers.iter().filter_map(Value::as_str) {
                ui.colored_label(theme::DANGER_DEEP, blocker);
            }
        }
    }

    if let Some(warnings) = plan.get("warnings").and_then(Value::as_array) {
        for warning in warnings.iter().filter_map(Value::as_str) {
            ui.colored_label(theme::WARNING_AMBER, warning);
        }
    }
}

fn environment_mode_description(mode: EnvironmentPlanMode, language: Language) -> &'static str {
    match mode {
        EnvironmentPlanMode::UseExisting => language.text(
            "只报告现有工具和缺失项，不提出安装。",
            "Report existing and missing tools without proposing installation.",
        ),
        EnvironmentPlanMode::ManagedUser => language.text(
            "默认模式；缺失组件进入用户目录，不覆盖现有工具。",
            "Default; place missing components under the user directory and preserve existing tools.",
        ),
        EnvironmentPlanMode::ProjectIsolated => language.text(
            "为当前项目生成独立目录和运行时锁。",
            "Create an isolated directory and runtime lock for this project.",
        ),
        EnvironmentPlanMode::SystemMissingOnly => language.text(
            "仅规划系统中缺失的组件；需要明确确认和相应权限。",
            "Plan only components missing from the system; explicit approval and privileges are required.",
        ),
    }
}

fn localized_environment_mode(mode: &str, language: Language) -> &'static str {
    match mode {
        "use-existing" => language.text("仅使用现有", "Use existing"),
        "managed-user" => language.text("用户隔离", "Managed user"),
        "project-isolated" => language.text("项目隔离", "Project isolated"),
        "system-missing-only" => language.text("系统缺失项", "System missing only"),
        _ => language.text("未知", "Unknown"),
    }
}

fn localized_boolean(value: bool, language: Language) -> &'static str {
    if value {
        language.text("是", "yes")
    } else {
        language.text("否", "no")
    }
}

fn localized_profile_description(profile: &str, language: Language) -> Option<&'static str> {
    if language == Language::EnUs {
        return None;
    }
    match profile {
        "local-core" => Some("无需外部工具的内置 Rust 能力"),
        "scripting" => Some("用于兼容分析流程的 Python、R 和 Java 运行时"),
        "managed-runtimes" => Some("用户级运行时管理器及其 Python、R 和 Java 运行时"),
        "containers" => Some("适用于当前平台的本地容器和 Unix 执行后端"),
        "sequence-search" => Some("本地核酸与蛋白质数据库搜索"),
        "genomics-cli" => Some("常用比对、变异、区间和序列映射工具"),
        "full-local" => Some("当前登记的全部本地运行时和生物信息学工具"),
        _ => None,
    }
}

fn metric_label(key: &str, language: Language) -> &str {
    if language == Language::EnUs {
        return key;
    }
    match key {
        "sequence_count" => "序列条数",
        "total_bases" => "总碱基数",
        "min_length" => "最短长度",
        "max_length" => "最长长度",
        "mean_length" => "平均长度",
        "n50" => "N50",
        "l50" => "L50",
        "au_n" => "auN",
        "gc_percent" => "GC 百分比",
        "n_count" => "N 数量",
        "n_percent" => "N 百分比",
        "read_count" => "读段数",
        "input_read_count" => "输入读段数",
        "output_read_count" => "输出读段数",
        "discarded_read_count" => "丢弃读段数",
        "trimmed_read_count" => "被裁剪读段数",
        "mean_quality" => "平均质量值",
        "q20_percent" => "Q20 百分比",
        "q30_percent" => "Q30 百分比",
        "quality_encoding" => "质量编码",
        "applied_quality_offset" => "采用的质量偏移",
        "per_cycle" => "逐循环指标",
        "warnings" => "警告",
        "record_count" => "记录数",
        "directive_count" => "指令数",
        "comment_count" => "注释行数",
        "sequence_region_count" => "序列区域数",
        "records_with_id" => "含标识符记录数",
        "records_with_parent" => "含父级记录数",
        "min_start" => "最小起点",
        "max_end" => "最大终点",
        "feature_type_counts" => "各特征类型记录数",
        "sequence_counts" => "各序列记录数",
        "source_counts" => "各来源记录数",
        "strand_counts" => "各链方向记录数",
        "input_record_count" => "输入记录数",
        "output_record_count" => "输出记录数",
        "converted_gtf_attribute_records" => "转换 GTF 属性记录数",
        "sorted" => "已排序",
        "missing_identifier_count" => "缺失标识符数",
        "annotation_record_count" => "注释记录数",
        "matched_feature_count" => "匹配特征数",
        "output_sequence_count" => "输出序列数",
        "output_base_count" => "输出碱基数",
        "missing_reference_count" => "缺失参考数",
        "skipped_feature_count" => "跳过特征数",
        "feature_type" => "特征类型",
        "promoter_length" => "启动子长度",
        "header_line_count" => "表头行数",
        "primary_record_count" => "主要比对记录数",
        "secondary_record_count" => "次要比对记录数",
        "supplementary_record_count" => "补充比对记录数",
        "mapped_record_count" => "已比对记录数",
        "unmapped_record_count" => "未比对记录数",
        "mapped_percent" => "比对百分比",
        "paired_record_count" => "配对记录数",
        "proper_pair_record_count" => "正确配对记录数",
        "read1_record_count" => "Read 1 记录数",
        "read2_record_count" => "Read 2 记录数",
        "duplicate_record_count" => "重复记录数",
        "duplicate_read_count" => "重复读段数",
        "strategy" => "策略",
        "umi_length" => "UMI 长度",
        "qc_fail_record_count" => "QC 失败记录数",
        "zero_mapq_record_count" => "零 MAPQ 记录数",
        "mean_mapq" => "平均 MAPQ",
        "reference_counts" => "各参考序列记录数",
        "sample_count" => "样本数",
        "feature_count" => "特征数",
        "total_value_count" => "总单元格数",
        "numeric_value_count" => "数值单元格数",
        "missing_value_count" => "缺失值数",
        "zero_value_count" => "零值数",
        "negative_value_count" => "负值数",
        "zero_percent" => "零值百分比",
        "duplicate_feature_id_count" => "重复特征标识数",
        "samples" => "各样本指标",
        "method" => "方法",
        "pseudocount" => "伪计数",
        "input_total" => "输入总量",
        "output_total" => "输出总量",
        "scale_factor" => "缩放因子",
        "scaled_features" => "已按特征标准化",
        "total_variance" => "总方差",
        "components" => "主成分",
        "component" => "主成分编号",
        "eigenvalue" => "特征值",
        "explained_variance_percent" => "解释方差百分比",
        "top_positive_loadings" => "最强正载荷",
        "top_negative_loadings" => "最强负载荷",
        "scores" => "主成分得分",
        "features" => "各特征聚类",
        "requested_clusters" => "请求簇数",
        "populated_clusters" => "非空簇数",
        "iterations" => "迭代次数",
        "converged" => "已收敛",
        "within_cluster_sum_squares" => "簇内平方和",
        "cluster_sizes" => "各簇大小",
        "assignments" => "聚类分配",
        "distance_to_centroid" => "到质心距离",
        "input_feature_count" => "输入特征数",
        "selected_feature_count" => "所选特征数",
        "scaled_rows" => "已按行标准化",
        "minimum_value" => "最小值",
        "maximum_value" => "最大值",
        "row_labels" => "行标签",
        "column_labels" => "列标签",
        "values" => "热图数值",
        "set_count" => "集合数",
        "union_size" => "并集大小",
        "set_sizes" => "各集合大小",
        "intersection_count" => "精确交集总数",
        "reported_intersection_count" => "已返回交集数",
        "omitted_intersection_count" => "省略交集数",
        "intersections" => "精确交集",
        "total_residues" => "总残基数",
        "standard_residue_count" => "标准残基数",
        "ambiguous_residue_count" => "歧义/非标准残基数",
        "composition" => "残基组成",
        "molecular_weight_da" => "分子量（Da）",
        "isoelectric_point" => "理论等电点",
        "charge_at_ph7" => "pH 7 净电荷",
        "aromaticity_percent" => "芳香性百分比",
        "gravy" => "GRAVY",
        "extinction_coefficient_reduced" => "还原态消光系数",
        "extinction_coefficient_oxidized" => "氧化态消光系数",
        "input_rows" => "输入行数",
        "output_rows" => "输出行数",
        "skipped_rows" => "跳过行数",
        "filtered_rows" => "过滤行数",
        "input_columns" => "输入列数",
        "output_columns" => "输出列数",
        "input_delimiter" => "输入分隔符",
        "output_delimiter" => "输出分隔符",
        "selected_columns" => "保留列",
        "dropped_columns" => "删除列",
        "left_interval_count" => "左侧区间数",
        "right_interval_count" => "右侧区间数",
        "query_interval_count" => "查询区间数",
        "target_interval_count" => "目标区间数",
        "matched_query_count" => "已匹配查询数",
        "unmatched_query_count" => "未匹配查询数",
        "input_interval_count" => "输入区间数",
        "output_interval_count" => "输出区间数",
        "merged_interval_count" => "被合并区间数",
        "input_bases" => "输入碱基数",
        "output_bases" => "输出碱基数",
        "quality_trimmed_bases" => "质量裁剪碱基数",
        "adapter_trimmed_bases" => "接头裁剪碱基数",
        "max_gap" => "最大间隔",
        "affected_left_interval_count" => "受影响左侧区间数",
        "removed_bases" => "被扣除碱基数",
        "overlap_pair_count" => "重叠对数",
        "left_overlapped_count" => "左侧已重叠区间数",
        "right_overlapped_count" => "右侧已重叠区间数",
        "total_overlap_bases" => "累计重叠碱基数",
        "ranked_gene_count" => "排序基因数",
        "input_gene_set_count" => "输入基因集数",
        "tested_gene_set_count" => "检验基因集数",
        "permutation_count" => "置换次数",
        "nominal_p_value" => "名义 p 值",
        "fdr_bh" => "BH FDR",
        "enrichment_score" => "富集分数",
        "leading_edge_genes" => "Leading edge 基因",
        "contigs" => "各区域统计",
        "model_count" => "模型数",
        "chain_count" => "链数",
        "residue_count" => "残基数",
        "atom_count" => "原子数",
        "polymer_atom_count" => "聚合物原子数",
        "hetero_atom_count" => "异质原子数",
        "alphafold_confidence" => "AlphaFold 置信度",
        "coordinate_units" => "坐标单位",
        "element_counts" => "元素计数",
        "bounds" => "坐标边界",
        "models" => "模型摘要",
        "model_id" => "模型编号",
        "chains" => "链序列",
        "atom_name" => "原子名",
        "cutoff_angstrom" => "距离阈值（埃）",
        "representative_residue_count" => "代表残基数",
        "contact_count" => "接触数",
        "contacts" => "接触明细",
        "measurement" => "测量类型",
        "units" => "单位",
        "value" => "测量值",
        "atoms" => "所选原子",
        "reference_format" => "参考结构格式",
        "mobile_format" => "移动结构格式",
        "reference_model_id" => "参考模型编号",
        "mobile_model_id" => "移动模型编号",
        "matched_atom_count" => "匹配原子数",
        "rmsd_before_angstrom" => "拟合前 RMSD（埃）",
        "rmsd_after_angstrom" => "拟合后 RMSD（埃）",
        "rotation" => "旋转矩阵",
        "translation" => "平移向量",
        "pass_record_count" => "PASS 记录数",
        "filtered_record_count" => "过滤记录数",
        "snp_count" => "SNP 等位基因数",
        "indel_count" => "indel 等位基因数",
        "mnv_count" => "MNV 等位基因数",
        "symbolic_count" => "符号等位基因数",
        "multiallelic_record_count" => "多等位记录数",
        "transition_count" => "转换数",
        "transversion_count" => "颠换数",
        "ti_tv_ratio" => "转换/颠换比",
        "missing_genotype_count" => "缺失基因型数",
        "called_genotype_count" => "已检出基因型数",
        "missing_genotype_rate" => "基因型缺失率",
        "contig_counts" => "各染色体记录数",
        "k" => "k-mer 长度",
        "canonical" => "合并反向互补",
        "total_windows" => "候选窗口数",
        "counted_windows" => "已计数窗口数",
        "skipped_ambiguous_windows" => "跳过歧义窗口数",
        "distinct_kmers" => "不同 k-mer 数",
        "top_kmers" => "高频 k-mer",
        "primer_pair_count" => "引物对数",
        "matched_primer_pair_count" => "命中引物对数",
        "amplicon_count" => "扩增子数",
        "min_amplicon" => "最小扩增子长度",
        "max_amplicon" => "最大扩增子长度",
        "input_records" => "输入记录数",
        "output_records" => "输出记录数",
        "rejected_by_qual" => "QUAL 淘汰数",
        "rejected_by_filter" => "FILTER 淘汰数",
        "rejected_by_contig" => "染色体淘汰数",
        "rejected_by_info_dp" => "INFO/DP 淘汰数",
        "changed_records" => "已规范化记录数",
        "left_aligned_records" => "已左对齐记录数",
        "reference_validated_records" => "参考验证记录数",
        _ => key,
    }
}

fn document_title(capability: &str, language: Language) -> &'static str {
    match capability {
        "dataset.inspect.v1" => language.text("数据集检查", "Dataset inspection"),
        "table.export.v1" => language.text("表格导出", "Table export"),
        "sequence.stats.v1" => language.text("FASTA 序列统计", "FASTA sequence statistics"),
        "sequence.kmer.count.v1" => language.text("精确 k-mer 计数", "Exact k-mer counting"),
        "primer.epcr.v1" => language.text("简单电子 PCR", "Simple electronic PCR"),
        "fastq.qc.v1" => language.text("FASTQ 质量控制", "FASTQ quality control"),
        "fastq.trim.v1" => language.text("FASTQ 质量裁剪", "FASTQ quality trimming"),
        "fastq.adapter.v1" => language.text("FASTQ 接头去除", "FASTQ adapter removal"),
        "fastq.deduplicate.v1" => language.text("FASTQ 精确去重", "FASTQ exact deduplication"),
        "alignment.qc.v1" => language.text("SAM 比对质量控制", "SAM alignment quality control"),
        "alignment.bam-to-bigwig.v1" => language.text("BAM/CRAM 转 BigWig", "BAM/CRAM to BigWig"),
        "annotation.gxf.stats.v1" => {
            language.text("GFF/GTF 注释统计", "GFF/GTF annotation statistics")
        }
        "annotation.gxf.normalize.v1" => {
            language.text("GFF/GTF 注释规范化", "GFF/GTF annotation normalization")
        }
        "annotation.gene-position.v1" => language.text("基因位置表", "Gene position table"),
        "annotation.sequence.extract.v1" => {
            language.text("按注释提取序列", "Annotation-guided sequence extraction")
        }
        "annotation.structure.visualize.v1" => {
            language.text("注释结构可视化", "Annotation structure visualization")
        }
        "comparative.synteny.visualize.v1" => {
            language.text("共线性锚点可视化", "Synteny anchor visualization")
        }
        "comparative.mcscanx.v1" => {
            language.text("基因组共线性分析", "Genome collinearity analysis")
        }
        "comparative.kaks.v1" => language.text("Ka/Ks 计算", "Ka/Ks calculation"),
        "annotation.go.normalize.v1" => {
            language.text("GO 注释规范化", "GO annotation normalization")
        }
        "annotation.eggnog.normalize.v1" => {
            language.text("eggNOG 注释规范化", "eggNOG annotation normalization")
        }
        "enrichment.overrepresentation.v1" => {
            language.text("通用过度富集分析", "Generic over-representation analysis")
        }
        "enrichment.go.v1" => language.text("GO 富集分析", "GO enrichment analysis"),
        "enrichment.kegg.v1" => language.text("KEGG 富集分析", "KEGG enrichment analysis"),
        "enrichment.gsea.v1" => language.text("预排序 GSEA", "Preranked GSEA"),
        "enrichment.visualize.v1" => language.text("富集结果可视化", "Enrichment visualization"),
        "genome.gene-density.v1" => language.text("基因组特征密度", "Genome feature density"),
        "interval.intersect.v1" => language.text("BED 区间相交", "BED interval intersection"),
        "interval.merge.v1" => language.text("BED 区间合并", "BED interval merge"),
        "interval.subtract.v1" => language.text("BED 区间扣除", "BED interval subtraction"),
        "interval.closest.v1" => language.text("BED 最近区间", "BED nearest interval"),
        "expression.matrix.qc.v1" => {
            language.text("表达矩阵质量控制", "Expression matrix quality control")
        }
        "medical.cohort-table.qc.v1" => {
            language.text("研究队列表质量控制", "Research cohort table QC")
        }
        "medical.pathway-ruo.v1" => {
            language.text("研究队列通路分析", "Research cohort pathway analysis")
        }
        "medical.variant-cohort.v1" => {
            language.text("研究队列变异汇总", "Research cohort variant aggregation")
        }
        "medical.single-cell-qc.v1" => {
            language.text("单细胞计数矩阵质量控制", "Single-cell count matrix QC")
        }
        "expression.normalize.v1" => language.text("表达矩阵标准化", "Expression normalization"),
        "expression.pca.v1" => language.text("表达矩阵 PCA", "Expression PCA"),
        "expression.cluster.v1" => language.text("表达矩阵聚类", "Expression clustering"),
        "expression.heatmap.v1" => language.text("聚类表达热图", "Clustered expression heatmap"),
        "set.venn.v1" => language.text("2–6 集合 Venn 分析", "Two-to-six-set Venn analysis"),
        "set.upset.v1" => language.text("多集合 UpSet 分析", "Multi-set UpSet analysis"),
        "protein.properties.v1" => language.text("蛋白理化性质", "Protein properties"),
        "similarity.blast.local.v1" => language.text("本地 BLAST+ 搜索", "Local BLAST+ search"),
        "similarity.diamond.v1" => language.text("DIAMOND 相似性搜索", "DIAMOND similarity search"),
        "similarity.hmmer.v1" => language.text("HMMER profile 搜索", "HMMER profile search"),
        "similarity.blast.parse.v1" => language.text("BLAST 结果解析", "BLAST result parsing"),
        "similarity.reciprocal.v1" => language.text("双向最佳命中", "Reciprocal best hits"),
        "protein.domain.parse.v1" => {
            language.text("蛋白结构域结果解析", "Protein domain result parsing")
        }
        "protein.domain.visualize.v1" => language.text(
            "蛋白结构域架构图",
            "Protein domain architecture visualization",
        ),
        "phylogeny.tree.transform.v1" => {
            language.text("系统发育树转换", "Phylogeny tree transform")
        }
        "phylogeny.iqtree.v1" => {
            language.text("IQ-TREE 系统发育推断", "IQ-TREE phylogeny inference")
        }
        "msa.muscle.v1" => language.text("MUSCLE 多序列比对", "MUSCLE multiple sequence alignment"),
        "msa.trimal.v1" => language.text("trimAl 比对裁剪", "trimAl alignment trimming"),
        "motif.meme.v1" => language.text("MEME 基序发现", "MEME motif discovery"),
        "table.manipulate.v1" => language.text("表格处理", "Table manipulation"),
        "variant.stats.v1" => language.text("VCF 变异统计", "VCF variant statistics"),
        "variant.filter.v1" => language.text("VCF 基础过滤", "Basic VCF filtering"),
        "variant.normalize.v1" => {
            language.text("VCF 参考规范化", "Reference-guided VCF normalization")
        }
        "structure.pdb.summary.v1" => language.text("PDB 结构摘要", "PDB structure summary"),
        "structure.viewer.v1" => language.text("交互式结构查看器", "Interactive structure viewer"),
        "structure.mmcif.summary.v1" => language.text("mmCIF 结构摘要", "mmCIF structure summary"),
        "structure.sequence.extract.v1" => {
            language.text("坐标序列提取", "Coordinate sequence extraction")
        }
        "structure.contact-map.v1" => language.text("残基接触图", "Residue contact map"),
        "structure.geometry.v1" => language.text("结构几何测量", "Structure geometry"),
        "structure.superpose.v1" => language.text("结构刚体叠合", "Structure superposition"),
        "protein.secondary-structure.v1" => {
            language.text("DSSP 二级结构", "DSSP secondary structure")
        }
        "environment.audit.v1" => language.text("环境审计", "Environment audit"),
        "environment.plan.v1" => language.text("环境计划", "Environment plan"),
        "runtime.catalog.v1" => language.text("运行时目录", "Runtime catalog"),
        "system.doctor.v1" => language.text("系统诊断", "System doctor"),
        "system.worker.v1" => language.text("本地任务 Worker", "Local job worker"),
        _ => language.text("未知能力", "Unknown capability"),
    }
}

fn capability_document(capability: &str, language: Language) -> Option<String> {
    capability_document_from_root(docs_root().as_deref(), capability, language).or_else(|| {
        docs_snapshot::embedded_document(capability, language.locale()).map(str::to_owned)
    })
}

/// Read a capability document from an explicit documentation root.
fn capability_document_from_root(
    root: Option<&Path>,
    capability: &str,
    language: Language,
) -> Option<String> {
    let root = root?;
    let path = root
        .join("capabilities")
        .join(capability)
        .join(format!("{}.md", language.locale()));
    fs::read_to_string(path).ok()
}

/// Resolve the documentation root: `LINXIRA_BIO_DOCS_ROOT` when set, then
/// exe-relative locations, then the development tree.
fn docs_root() -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os("LINXIRA_BIO_DOCS_ROOT")
        && !configured.is_empty()
    {
        return Some(PathBuf::from(configured));
    }
    if let Ok(executable) = std::env::current_exe() {
        for candidate in [
            executable.parent()?.join("docs"),
            executable.parent()?.join("resources/docs"),
            executable.parent()?.join("../share/linxira-bio/docs"),
        ] {
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs");
    if development.is_dir() {
        return Some(development);
    }
    None
}

mod docs_snapshot {
    include!("docs_snapshot.rs");
}

fn analysis_export_basename(result: &Value) -> String {
    const MAX_BASENAME_LENGTH: usize = 80;
    let Some(capability) = result.get("capability").and_then(Value::as_str) else {
        return "analysis-result".to_owned();
    };
    let mut basename = String::with_capacity(capability.len().min(MAX_BASENAME_LENGTH));
    let mut separator_pending = false;
    for byte in capability.bytes() {
        if byte.is_ascii_alphanumeric() {
            if separator_pending && !basename.is_empty() && basename.len() + 1 < MAX_BASENAME_LENGTH
            {
                basename.push('-');
            }
            separator_pending = false;
            if basename.len() < MAX_BASENAME_LENGTH {
                basename.push(char::from(byte.to_ascii_lowercase()));
            }
        } else if !basename.is_empty() {
            separator_pending = true;
        }
    }
    if basename.is_empty() {
        return "analysis-result".to_owned();
    }
    if matches!(
        basename.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    ) {
        basename.insert_str(0, "analysis-");
    }
    basename
}

fn load_packaged_dependency_notices() -> Result<DependencyNotices, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot locate the running executable: {error}"))?;
    let directory = executable.parent().ok_or_else(|| {
        format!(
            "executable has no parent directory: {}",
            executable.display()
        )
    })?;
    load_dependency_notices_from(directory)
}

fn load_dependency_notices_from(directory: &Path) -> Result<DependencyNotices, String> {
    const TEXT_NAME: &str = "THIRD_PARTY_DEPENDENCIES.txt";
    const JSON_NAME: &str = "THIRD_PARTY_DEPENDENCIES.json";
    let text_path = directory.join(TEXT_NAME);
    let json_path = directory.join(JSON_NAME);
    let text = fs::read_to_string(&text_path)
        .map_err(|error| format!("{}: {error}", text_path.display()))?;
    if text.trim().is_empty() {
        return Err(format!("{} is empty", text_path.display()));
    }
    let report_text = fs::read_to_string(&json_path)
        .map_err(|error| format!("{}: {error}", json_path.display()))?;
    let report: Value = serde_json::from_str(&report_text)
        .map_err(|error| format!("{}: {error}", json_path.display()))?;
    if report.get("schema_version").and_then(Value::as_str) != Some("1") {
        return Err(format!("{} has an unsupported schema", json_path.display()));
    }
    let platform = report_string(&report, "platform")
        .map_err(|error| format!("{}: {error}", json_path.display()))?;
    let target_triple = report_string(&report, "target_triple")
        .map_err(|error| format!("{}: {error}", json_path.display()))?;
    if !notice_platform_target_pair_is_valid(platform, target_triple) {
        return Err(format!(
            "{} has an invalid platform/target_triple pair: {platform}/{target_triple}",
            json_path.display()
        ));
    }
    if !notice_target_matches_current_build(platform, target_triple) {
        return Err(format!(
            "{} targets {platform}/{target_triple}, not this application build",
            json_path.display()
        ));
    }
    let package_count = report
        .get("dependency_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| format!("{} lacks dependency_count", json_path.display()))?;
    let dependency_count = report
        .get("dependencies")
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| format!("{} lacks dependencies", json_path.display()))?;
    if package_count != dependency_count {
        return Err(format!(
            "{} dependency_count does not match dependencies",
            json_path.display()
        ));
    }
    // The generator's text report is deterministic, so rebuilding it binds both staged files.
    let expected_text = render_dependency_notice_report(&report)
        .map_err(|error| format!("{}: {error}", json_path.display()))?;
    if text != expected_text {
        return Err(format!(
            "{} does not match the report in {}",
            text_path.display(),
            json_path.display()
        ));
    }
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    Ok(DependencyNotices {
        lines,
        directory: directory.to_owned(),
        package_count,
    })
}

fn notice_platform_target_pair_is_valid(platform: &str, target_triple: &str) -> bool {
    matches!(
        (platform, target_triple),
        ("windows", "x86_64-pc-windows-gnu") | ("debian" | "arch", "x86_64-unknown-linux-gnu")
    )
}

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"))]
fn notice_target_matches_current_build(platform: &str, target_triple: &str) -> bool {
    platform == "windows" && target_triple == "x86_64-pc-windows-gnu"
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
fn notice_target_matches_current_build(platform: &str, target_triple: &str) -> bool {
    matches!(platform, "debian" | "arch") && target_triple == "x86_64-unknown-linux-gnu"
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
)))]
fn notice_target_matches_current_build(_platform: &str, _target_triple: &str) -> bool {
    false
}

fn render_dependency_notice_report(report: &Value) -> Result<String, String> {
    let platform = report_string(report, "platform")?;
    let target_triple = report_string(report, "target_triple")?;
    let cargo_version = report_string(report, "cargo_version")?;
    let cargo_lock_sha256 = report_string(report, "cargo_lock_sha256")?;
    let dependency_count = report
        .get("dependency_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| "dependency notice report lacks dependency_count".to_owned())?;
    let release_roots = report_array(report, "release_roots")?;
    let dependencies = report_array(report, "dependencies")?;
    let license_texts = report_array(report, "license_texts")?;
    let mut lines = vec![
        "Linxira Bio SDK Third-Party Cargo Dependency Notices".to_owned(),
        "====================================================".to_owned(),
        String::new(),
        format!("Release platform: {platform}"),
        format!("Rust target: {target_triple}"),
        format!("Cargo: {cargo_version}"),
        format!("Cargo.lock SHA-256: {cargo_lock_sha256}"),
        format!("External dependency count: {dependency_count}"),
        String::new(),
        "This file is generated deterministically from the locked, target-filtered".to_owned(),
        "Cargo release dependency graph. Project-owned code remains licensed under".to_owned(),
        "AGPL-3.0-or-later. Third-party terms below are not replaced or relicensed.".to_owned(),
        String::new(),
        "Release roots".to_owned(),
        "-------------".to_owned(),
    ];
    for root in release_roots {
        lines.push(format!(
            "- {} {}",
            report_string(root, "name")?,
            report_string(root, "version")?
        ));
    }
    lines.extend([
        String::new(),
        "Dependencies".to_owned(),
        "------------".to_owned(),
    ]);

    let mut users = BTreeMap::<String, BTreeSet<String>>::new();
    let mut sources = BTreeMap::<String, BTreeSet<String>>::new();
    for dependency in dependencies {
        let name = report_string(dependency, "name")?;
        let version = report_string(dependency, "version")?;
        let source = report_string(dependency, "source")?;
        let expression = optional_report_string(dependency, "license_expression")?
            .filter(|value| !value.is_empty())
            .unwrap_or("license_file");
        lines.push(format!("- {name} {version} [{expression}]"));
        lines.push(format!("  Source: {source}"));
        if let Some(repository) = optional_report_string(dependency, "repository")? {
            lines.push(format!("  Repository: {repository}"));
        }
        if let Some(vcs) = dependency.get("vcs") {
            lines.push(format!(
                "  VCS revision: {}",
                report_string(vcs, "revision")?
            ));
        }
        if let Some(reason) = optional_report_string(dependency, "override_reason")? {
            lines.push(format!("  Verified override: {reason}"));
        }
        if let Some(pointers) = optional_report_array(dependency, "replaced_package_pointers")? {
            let pointers = pointers
                .iter()
                .map(|pointer| {
                    pointer.as_str().ok_or_else(|| {
                        "dependency notice report has a non-string replaced pointer".to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            lines.push(format!(
                "  Replaced package pointer(s): {}",
                pointers.join(", ")
            ));
        }
        let package_name = format!("{name} {version}");
        for document in report_array(dependency, "documents")? {
            let origin = report_string(document, "origin")?;
            let path = report_string(document, "path")?;
            let digest = report_string(document, "sha256")?;
            lines.push(format!("  Notice: {origin}:{path} sha256:{digest}"));
            users
                .entry(digest.to_owned())
                .or_default()
                .insert(package_name.clone());
            sources
                .entry(digest.to_owned())
                .or_default()
                .insert(format!("{origin}:{path}"));
        }
    }

    lines.extend([
        String::new(),
        "Retained license and notice texts".to_owned(),
        "=================================".to_owned(),
    ]);
    for license_text in license_texts {
        let digest = report_string(license_text, "sha256")?;
        let retained_text = report_string(license_text, "text")?.trim_end_matches(['\r', '\n']);
        let used_by = users
            .get(digest)
            .map(|items| items.iter().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let documents = sources
            .get(digest)
            .map(|items| items.iter().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        lines.extend([
            String::new(),
            format!("SHA-256: {digest}"),
            format!("Used by: {used_by}"),
            format!("Documents: {documents}"),
            "------------------------------------------------------------------------".to_owned(),
            retained_text.to_owned(),
            "------------------------------------------------------------------------".to_owned(),
        ]);
    }
    Ok(lines.join("\n") + "\n")
}

fn report_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("dependency notice report lacks string field {key}"))
}

fn optional_report_string<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(item)) => Ok(Some(item)),
        Some(_) => Err(format!(
            "dependency notice report field {key} is not a string or null"
        )),
    }
}

fn report_array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("dependency notice report lacks array field {key}"))
}

fn optional_report_array<'a>(value: &'a Value, key: &str) -> Result<Option<&'a [Value]>, String> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::Array(items)) => Ok(Some(items.as_slice())),
        Some(_) => Err(format!(
            "dependency notice report field {key} is not an array"
        )),
    }
}

fn render_markdown_document(ui: &mut egui::Ui, document: &str) {
    let mut in_code_block = false;
    for line in document.lines() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            ui.monospace(line);
        } else if let Some(heading) = line.strip_prefix("# ") {
            ui.heading(heading);
        } else if let Some(heading) = line.strip_prefix("## ") {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(heading).strong().size(18.0));
        } else if line.is_empty() {
            ui.add_space(4.0);
        } else {
            ui.label(line);
        }
    }
}

fn render_plain_document(ui: &mut egui::Ui, document: &str) {
    ui.add(
        egui::Label::new(egui::RichText::new(document).monospace())
            .wrap()
            .selectable(true),
    );
}

fn render_dependency_notices(ui: &mut egui::Ui, lines: &[String]) {
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    egui::ScrollArea::both()
        .id_salt("dependency-license-texts")
        .max_height(620.0)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, lines.len(), |ui, visible| {
            for line in &lines[visible] {
                let line = if line.is_empty() { " " } else { line };
                ui.add(
                    egui::Label::new(egui::RichText::new(line).monospace())
                        .extend()
                        .selectable(true),
                );
            }
        });
}

fn install_cjk_font(context: &egui::Context) {
    let font_data = include_bytes!("../assets/fonts/NotoSansSC-Regular.otf").to_vec();
    let mut fonts = egui::FontDefinitions::default();
    let font_name = "linxira-cjk".to_owned();
    fonts.font_data.insert(
        font_name.clone(),
        Arc::new(egui::FontData::from_owned(font_data)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push(font_name.clone());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(font_name);
    context.set_fonts(fonts);
}

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

fn new_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed);
    format!("ui-{millis}-{sequence}")
}

#[cfg(test)]
mod tests {
    use super::{
        AnalysisRoute, DOCUMENTED_CAPABILITIES, DatasetState, ImportPathIssue, Language,
        analysis_export_basename, analysis_result_matches, analysis_route_for_capability,
        analysis_route_for_format, build_analysis_request, capability_document,
        capability_document_from_root, capability_output_extension, capability_requires_secondary,
        derived_analysis_output_path, format_hint, generation_matches, importable_file_path,
        inspection_is_runnable, inspection_state, load_dependency_notices_from,
        looks_like_drive_relative_path, new_job_id, notice_platform_target_pair_is_valid,
        render_dependency_notice_report, secondary_input_matches, secondary_input_role,
        tertiary_input_format, tertiary_input_role,
    };
    use linxira_bio_protocol::ExecutionMode;
    use serde_json::json;
    use std::{collections::HashSet, fs, path::PathBuf};

    #[test]
    fn detects_windows_paths_with_consumed_separators() {
        assert!(looks_like_drive_relative_path(&PathBuf::from(
            "C:UsersETPauDocumentsGITHUBbio-codingtestsfixtures"
        )));
        assert!(!looks_like_drive_relative_path(&PathBuf::from(
            r"C:\Users\ETPau\Documents\GITHUB\bio-coding\tests\fixtures"
        )));
        assert!(!looks_like_drive_relative_path(&PathBuf::from(
            "C:/Users/ETPau/Documents/GITHUB/bio-coding/tests/fixtures"
        )));
    }

    #[test]
    fn import_path_rejects_directories_separately_from_missing_files() {
        let directory = std::env::temp_dir();
        let (_, directory_issue) =
            importable_file_path(directory).expect_err("directory must not import");
        assert_eq!(directory_issue, ImportPathIssue::Directory);

        let missing = std::env::temp_dir().join(format!("missing-{}", new_job_id()));
        let (_, missing_issue) =
            importable_file_path(missing).expect_err("missing path must not import");
        assert_eq!(missing_issue, ImportPathIssue::Unreadable);
    }

    #[cfg(unix)]
    #[test]
    fn import_path_rejects_non_utf8_before_the_json_boundary() {
        use std::os::unix::ffi::OsStringExt;

        let mut name = format!("linxira-non-utf8-{}-", new_job_id()).into_bytes();
        name.push(0xff);
        let path = std::env::temp_dir().join(std::ffi::OsString::from_vec(name));
        fs::write(&path, b"test").expect("write non-UTF-8 fixture");

        let (_, issue) = importable_file_path(path.clone()).expect_err("path must be rejected");

        assert_eq!(issue, ImportPathIssue::NonUtf8);
        fs::remove_file(path).expect("remove non-UTF-8 fixture");
    }

    #[test]
    fn supported_formats_route_to_their_native_capabilities() {
        assert_eq!(
            analysis_route_for_format("FASTA"),
            Some(AnalysisRoute {
                capability: "sequence.stats.v1",
                input_role: "fasta",
            })
        );
        assert_eq!(
            analysis_route_for_format("fastq"),
            Some(AnalysisRoute {
                capability: "fastq.qc.v1",
                input_role: "fastq",
            })
        );
        assert_eq!(
            analysis_route_for_format(" vcf "),
            Some(AnalysisRoute {
                capability: "variant.stats.v1",
                input_role: "vcf",
            })
        );
        assert_eq!(
            analysis_route_for_format("sam"),
            Some(AnalysisRoute {
                capability: "alignment.qc.v1",
                input_role: "sam",
            })
        );
        assert_eq!(
            analysis_route_for_format("gff3"),
            Some(AnalysisRoute {
                capability: "annotation.gxf.stats.v1",
                input_role: "annotation",
            })
        );
        assert_eq!(
            analysis_route_for_format("bed"),
            Some(AnalysisRoute {
                capability: "interval.intersect.v1",
                input_role: "left-bed",
            })
        );
        assert_eq!(
            analysis_route_for_format("tsv"),
            Some(AnalysisRoute {
                capability: "expression.matrix.qc.v1",
                input_role: "matrix",
            })
        );
        assert_eq!(
            analysis_route_for_format("pdb"),
            Some(AnalysisRoute {
                capability: "structure.pdb.summary.v1",
                input_role: "pdb",
            })
        );
        assert_eq!(
            analysis_route_for_format("mmcif"),
            Some(AnalysisRoute {
                capability: "structure.mmcif.summary.v1",
                input_role: "structure",
            })
        );
        assert_eq!(
            analysis_route_for_format("blast-tabular"),
            Some(AnalysisRoute {
                capability: "similarity.blast.parse.v1",
                input_role: "blast",
            })
        );
        assert_eq!(
            analysis_route_for_format("blast-xml"),
            Some(AnalysisRoute {
                capability: "similarity.blast.parse.v1",
                input_role: "blast",
            })
        );
        assert_eq!(
            analysis_route_for_format("protein-domains"),
            Some(AnalysisRoute {
                capability: "protein.domain.parse.v1",
                input_role: "domains",
            })
        );
        assert_eq!(
            analysis_route_for_format("newick"),
            Some(AnalysisRoute {
                capability: "phylogeny.tree.transform.v1",
                input_role: "tree",
            })
        );
        assert_eq!(analysis_route_for_format("bam"), None);
    }

    #[test]
    fn comparative_native_routes_enforce_formats_and_roles() {
        assert_eq!(
            analysis_route_for_format("axt"),
            Some(AnalysisRoute {
                capability: "comparative.kaks.v1",
                input_role: "codon-alignment",
            })
        );
        assert_eq!(
            analysis_route_for_capability("comparative.mcscanx.v1", "tsv"),
            Some(AnalysisRoute {
                capability: "comparative.mcscanx.v1",
                input_role: "gene-positions",
            })
        );
        assert!(analysis_route_for_capability("comparative.mcscanx.v1", "gff3").is_none());
        assert!(capability_requires_secondary("comparative.mcscanx.v1"));
        assert!(secondary_input_matches(
            "comparative.mcscanx.v1",
            "blast-tabular"
        ));
        assert!(!secondary_input_matches(
            "comparative.mcscanx.v1",
            "blast-xml"
        ));
        assert_eq!(
            secondary_input_role("comparative.mcscanx.v1"),
            Some("similarity-hits")
        );
        assert_eq!(
            capability_output_extension("comparative.mcscanx.v1"),
            Some("collinearity")
        );

        assert_eq!(
            analysis_route_for_capability("comparative.kaks.v1", "axt"),
            Some(AnalysisRoute {
                capability: "comparative.kaks.v1",
                input_role: "codon-alignment",
            })
        );
        assert!(analysis_route_for_capability("comparative.kaks.v1", "fasta").is_none());
        assert!(!capability_requires_secondary("comparative.kaks.v1"));
        assert_eq!(
            capability_output_extension("comparative.kaks.v1"),
            Some("tsv")
        );
    }

    #[test]
    fn comparative_file_extensions_have_specific_format_hints() {
        assert_eq!(format_hint(&PathBuf::from("pairs.axt")), "axt");
        assert_eq!(
            format_hint(&PathBuf::from("blocks.collinearity")),
            "mcscanx-collinearity"
        );
    }

    #[test]
    fn differential_expression_routes_require_sample_metadata() {
        for capability in ["expression.differential.v1", "medical.bulk-rnaseq.v1"] {
            assert_eq!(
                analysis_route_for_capability(capability, "csv"),
                Some(AnalysisRoute {
                    capability,
                    input_role: "counts",
                })
            );
            assert_eq!(
                analysis_route_for_capability(capability, "tsv"),
                Some(AnalysisRoute {
                    capability,
                    input_role: "counts",
                })
            );
            assert!(capability_requires_secondary(capability));
            assert!(secondary_input_matches(capability, "csv"));
            assert!(secondary_input_matches(capability, "tsv"));
            assert!(!secondary_input_matches(capability, "fasta"));
            assert_eq!(secondary_input_role(capability), Some("sample_metadata"));
            assert_eq!(capability_output_extension(capability), None);
        }
    }

    #[test]
    fn similarity_domain_density_and_tree_routes_enforce_their_contracts() {
        assert_eq!(
            analysis_route_for_capability("similarity.blast.parse.v1", "blast-xml"),
            Some(AnalysisRoute {
                capability: "similarity.blast.parse.v1",
                input_role: "blast",
            })
        );
        assert_eq!(
            analysis_route_for_capability("similarity.reciprocal.v1", "blast-tabular"),
            Some(AnalysisRoute {
                capability: "similarity.reciprocal.v1",
                input_role: "forward",
            })
        );
        assert_eq!(
            analysis_route_for_capability("protein.domain.parse.v1", "protein-domains"),
            Some(AnalysisRoute {
                capability: "protein.domain.parse.v1",
                input_role: "domains",
            })
        );
        assert_eq!(
            analysis_route_for_capability("genome.gene-density.v1", "gff3"),
            Some(AnalysisRoute {
                capability: "genome.gene-density.v1",
                input_role: "annotation",
            })
        );
        assert_eq!(
            analysis_route_for_capability("phylogeny.tree.transform.v1", "newick"),
            Some(AnalysisRoute {
                capability: "phylogeny.tree.transform.v1",
                input_role: "tree",
            })
        );

        assert!(capability_requires_secondary("similarity.reciprocal.v1"));
        assert!(secondary_input_matches(
            "similarity.reciprocal.v1",
            "blast-xml"
        ));
        assert!(!secondary_input_matches("similarity.reciprocal.v1", "tsv"));
        assert!(!capability_requires_secondary("protein.domain.parse.v1"));
        assert_eq!(
            capability_output_extension("phylogeny.tree.transform.v1"),
            Some("nwk")
        );

        let output =
            derived_analysis_output_path("trees/input.nwk", "phylogeny.tree.transform.v1", "nwk");
        let output_name = output
            .file_name()
            .and_then(|name| name.to_str())
            .expect("derived Newick output name");
        assert!(output_name.starts_with("input.phylogeny-tree-transform."));
        assert!(output_name.ends_with(".nwk"));
    }

    #[test]
    fn native_search_and_alignment_routes_enforce_their_contracts() {
        for capability in ["similarity.blast.local.v1", "similarity.diamond.v1"] {
            assert_eq!(
                analysis_route_for_capability(capability, "fasta"),
                Some(AnalysisRoute {
                    capability,
                    input_role: "query",
                })
            );
            assert!(capability_requires_secondary(capability));
            assert!(secondary_input_matches(capability, "fasta"));
            assert_eq!(secondary_input_role(capability), Some("reference"));
            assert_eq!(capability_output_extension(capability), Some("tsv"));
        }

        assert_eq!(
            analysis_route_for_format("hmm-profile"),
            Some(AnalysisRoute {
                capability: "similarity.hmmer.v1",
                input_role: "profile",
            })
        );
        assert!(capability_requires_secondary("similarity.hmmer.v1"));
        assert!(secondary_input_matches("similarity.hmmer.v1", "fasta"));
        assert_eq!(
            secondary_input_role("similarity.hmmer.v1"),
            Some("sequences")
        );
        assert_eq!(
            capability_output_extension("similarity.hmmer.v1"),
            Some("domtblout")
        );

        assert_eq!(
            analysis_route_for_capability("msa.muscle.v1", "fasta"),
            Some(AnalysisRoute {
                capability: "msa.muscle.v1",
                input_role: "fasta",
            })
        );
        assert!(!capability_requires_secondary("msa.muscle.v1"));
        assert_eq!(capability_output_extension("msa.muscle.v1"), Some("fasta"));

        for (capability, role, extension) in [
            ("msa.trimal.v1", "alignment", "fasta"),
            ("phylogeny.iqtree.v1", "alignment", "nwk"),
            ("motif.meme.v1", "fasta", "meme"),
        ] {
            assert_eq!(
                analysis_route_for_capability(capability, "fasta"),
                Some(AnalysisRoute {
                    capability,
                    input_role: role,
                })
            );
            assert!(!capability_requires_secondary(capability));
            assert_eq!(capability_output_extension(capability), Some(extension));
        }

        for format in ["bam", "cram"] {
            assert_eq!(
                analysis_route_for_capability("alignment.bam-to-bigwig.v1", format),
                Some(AnalysisRoute {
                    capability: "alignment.bam-to-bigwig.v1",
                    input_role: "alignment",
                })
            );
        }
        assert_eq!(
            capability_output_extension("alignment.bam-to-bigwig.v1"),
            Some("bw")
        );

        for format in ["pdb", "mmcif"] {
            assert_eq!(
                analysis_route_for_capability("protein.secondary-structure.v1", format),
                Some(AnalysisRoute {
                    capability: "protein.secondary-structure.v1",
                    input_role: "structure",
                })
            );
        }
        assert_eq!(
            capability_output_extension("protein.secondary-structure.v1"),
            Some("dssp")
        );
    }

    #[test]
    fn bed_capability_routes_include_set_operations() {
        assert_eq!(
            analysis_route_for_capability("interval.merge.v1", "bed"),
            Some(AnalysisRoute {
                capability: "interval.merge.v1",
                input_role: "bed",
            })
        );
        assert_eq!(
            analysis_route_for_capability("interval.subtract.v1", "bed"),
            Some(AnalysisRoute {
                capability: "interval.subtract.v1",
                input_role: "left-bed",
            })
        );
        assert_eq!(
            analysis_route_for_capability("interval.merge.v1", "fasta"),
            None
        );
        assert_eq!(
            analysis_route_for_capability("table.manipulate.v1", "tsv"),
            Some(AnalysisRoute {
                capability: "table.manipulate.v1",
                input_role: "table",
            })
        );
        assert_eq!(
            analysis_route_for_capability("annotation.sequence.extract.v1", "gtf"),
            Some(AnalysisRoute {
                capability: "annotation.sequence.extract.v1",
                input_role: "annotation",
            })
        );
        assert!(!capability_requires_secondary("interval.merge.v1"));
        assert!(capability_requires_secondary("interval.subtract.v1"));
        assert!(capability_requires_secondary(
            "annotation.sequence.extract.v1"
        ));
        assert_eq!(
            capability_output_extension("interval.merge.v1"),
            Some("bed")
        );
        assert_eq!(
            capability_output_extension("table.manipulate.v1"),
            Some("tsv")
        );
        assert_eq!(
            capability_output_extension("annotation.gxf.normalize.v1"),
            Some("gff3")
        );
        assert_eq!(capability_output_extension("sequence.stats.v1"), None);

        let output =
            derived_analysis_output_path("data/regions.bed", "interval.subtract.v1", "bed");
        let output_name = output
            .file_name()
            .and_then(|name| name.to_str())
            .expect("derived output name");
        assert!(output_name.starts_with("regions.interval-subtract."));
        assert!(output_name.ends_with(".bed"));
    }

    #[test]
    fn set_and_protein_capabilities_route_only_supported_inputs() {
        assert_eq!(
            analysis_route_for_capability("set.venn.v1", "csv"),
            Some(AnalysisRoute {
                capability: "set.venn.v1",
                input_role: "table",
            })
        );
        assert_eq!(
            analysis_route_for_capability("set.upset.v1", "tsv"),
            Some(AnalysisRoute {
                capability: "set.upset.v1",
                input_role: "table",
            })
        );
        assert_eq!(
            analysis_route_for_capability("protein.properties.v1", "fasta"),
            Some(AnalysisRoute {
                capability: "protein.properties.v1",
                input_role: "fasta",
            })
        );
        assert_eq!(analysis_route_for_capability("set.venn.v1", "fasta"), None);
        assert_eq!(
            analysis_route_for_capability("protein.properties.v1", "csv"),
            None
        );
        assert!(!capability_requires_secondary("set.venn.v1"));
        assert!(!capability_requires_secondary("set.upset.v1"));
        assert!(!capability_requires_secondary("protein.properties.v1"));
    }

    #[test]
    fn functional_annotation_and_enrichment_routes_use_explicit_roles() {
        for capability in [
            "annotation.go.normalize.v1",
            "annotation.eggnog.normalize.v1",
        ] {
            assert_eq!(
                analysis_route_for_capability(capability, "tsv"),
                Some(AnalysisRoute {
                    capability,
                    input_role: "annotations",
                })
            );
            assert_eq!(capability_output_extension(capability), Some("tsv"));
        }

        for capability in [
            "enrichment.overrepresentation.v1",
            "enrichment.go.v1",
            "enrichment.kegg.v1",
        ] {
            assert_eq!(
                analysis_route_for_capability(capability, "csv"),
                Some(AnalysisRoute {
                    capability,
                    input_role: "genes",
                })
            );
            assert!(capability_requires_secondary(capability));
            assert!(secondary_input_matches(capability, "csv"));
            assert!(secondary_input_matches(capability, "tsv"));
            assert_eq!(secondary_input_role(capability), Some("associations"));
            assert_eq!(capability_output_extension(capability), None);
        }

        assert!(analysis_route_for_capability("annotation.go.normalize.v1", "fasta").is_none());
        assert!(!secondary_input_matches("enrichment.go.v1", "fasta"));
    }

    #[test]
    fn scientific_visualization_routes_produce_svg_outputs() {
        assert_eq!(
            analysis_route_for_capability("annotation.structure.visualize.v1", "gff3"),
            Some(AnalysisRoute {
                capability: "annotation.structure.visualize.v1",
                input_role: "annotation",
            })
        );
        assert_eq!(
            analysis_route_for_capability("protein.domain.visualize.v1", "protein-domains"),
            Some(AnalysisRoute {
                capability: "protein.domain.visualize.v1",
                input_role: "domains",
            })
        );
        assert_eq!(
            analysis_route_for_capability("enrichment.visualize.v1", "tsv"),
            Some(AnalysisRoute {
                capability: "enrichment.visualize.v1",
                input_role: "genes",
            })
        );
        assert_eq!(
            analysis_route_for_capability("comparative.synteny.visualize.v1", "tsv"),
            Some(AnalysisRoute {
                capability: "comparative.synteny.visualize.v1",
                input_role: "anchors",
            })
        );
        assert!(capability_requires_secondary("enrichment.visualize.v1"));
        assert_eq!(
            secondary_input_role("enrichment.visualize.v1"),
            Some("associations")
        );
        for capability in [
            "annotation.structure.visualize.v1",
            "comparative.synteny.visualize.v1",
            "enrichment.visualize.v1",
            "protein.domain.visualize.v1",
        ] {
            assert_eq!(capability_output_extension(capability), Some("svg"));
        }
        assert!(
            analysis_route_for_capability("annotation.structure.visualize.v1", "fasta").is_none()
        );
        assert!(analysis_route_for_capability("protein.domain.visualize.v1", "csv").is_none());
    }

    #[test]
    fn coordinate_structure_capabilities_route_pdb_and_mmcif() {
        for format in ["pdb", "mmcif"] {
            for capability in [
                "structure.sequence.extract.v1",
                "structure.contact-map.v1",
                "structure.geometry.v1",
                "structure.superpose.v1",
            ] {
                assert!(
                    analysis_route_for_capability(capability, format).is_some(),
                    "{capability} should accept {format}"
                );
            }
        }
        assert!(analysis_route_for_capability("structure.mmcif.summary.v1", "mmcif").is_some());
        assert!(analysis_route_for_capability("structure.mmcif.summary.v1", "pdb").is_none());
        assert!(capability_requires_secondary("structure.superpose.v1"));
        assert!(secondary_input_matches("structure.superpose.v1", "pdb"));
        assert!(secondary_input_matches("structure.superpose.v1", "mmcif"));
        assert!(!secondary_input_matches("structure.superpose.v1", "fasta"));
    }

    #[test]
    fn v1_planned_medical_and_chemistry_capabilities_route_and_export_tsv() {
        for (capability, format, role) in [
            ("medical.pharmacogenomics.v1", "vcf", "vcf"),
            ("medical.metabolomics.v1", "mzml", "mzml"),
            ("medical.microbiome.v1", "fasta", "reads"),
        ] {
            let route = analysis_route_for_capability(capability, format)
                .unwrap_or_else(|| panic!("{capability} should route {format}"));
            assert_eq!(route.capability, capability);
            assert_eq!(route.input_role, role);
            assert_eq!(capability_output_extension(capability), Some("tsv"));
            assert!(!capability_requires_secondary(capability));
        }
        // survival and descriptors emit output directories (worker output_directory contract)
        for (capability, format, role) in [
            ("medical.survival.v1", "csv", "cohort"),
            ("chemistry.descriptors.v1", "sdf", "molecules"),
        ] {
            let route = analysis_route_for_capability(capability, format)
                .unwrap_or_else(|| panic!("{capability} should route {format}"));
            assert_eq!(route.capability, capability);
            assert_eq!(route.input_role, role);
            assert_eq!(capability_output_extension(capability), None);
            assert!(!capability_requires_secondary(capability));
        }
        // spatial-transcriptomics needs three inputs: matrix + features + barcodes
        let route = analysis_route_for_capability("medical.spatial-transcriptomics.v1", "mtx")
            .expect("spatial matrix route");
        assert_eq!(route.input_role, "matrix");
        assert_eq!(
            capability_output_extension("medical.spatial-transcriptomics.v1"),
            Some("tsv")
        );
        assert!(capability_requires_secondary(
            "medical.spatial-transcriptomics.v1"
        ));
        assert!(secondary_input_matches(
            "medical.spatial-transcriptomics.v1",
            "tsv"
        ));
        assert_eq!(
            secondary_input_role("medical.spatial-transcriptomics.v1"),
            Some("features")
        );
        assert_eq!(
            tertiary_input_role("medical.spatial-transcriptomics.v1"),
            Some("barcodes")
        );
        assert_eq!(
            tertiary_input_format("medical.spatial-transcriptomics.v1"),
            Some("tsv")
        );
        // negative checks
        assert!(analysis_route_for_capability("medical.survival.v1", "fasta").is_none());
        assert!(analysis_route_for_capability("chemistry.descriptors.v1", "csv").is_none());
        assert!(analysis_route_for_capability("medical.pharmacogenomics.v1", "vcf").is_some());
        assert!(
            analysis_route_for_capability("medical.spatial-transcriptomics.v1", "csv").is_none()
        );
    }

    #[test]
    fn format_based_routes_default_to_new_medical_and_chemistry_capabilities() {
        assert_eq!(
            analysis_route_for_format("mzml").map(|route| route.capability),
            Some("medical.metabolomics.v1")
        );
        assert_eq!(
            analysis_route_for_format("sdf").map(|route| route.capability),
            Some("chemistry.descriptors.v1")
        );
        assert_eq!(
            analysis_route_for_format("fasta").map(|route| route.capability),
            Some("sequence.stats.v1")
        );
    }

    #[test]
    fn analysis_request_preserves_job_route_and_input() {
        let route = analysis_route_for_format("fastq").expect("FASTQ route");
        let request = build_analysis_request("ui-job-exact", route, "reads/sample.fastq");

        assert_eq!(request.job_id, "ui-job-exact");
        assert_eq!(request.capability, "fastq.qc.v1");
        assert_eq!(
            request.inputs.get("fastq").map(String::as_str),
            Some("reads/sample.fastq")
        );
        assert_eq!(request.execution.mode, ExecutionMode::LocalCpu);
    }

    #[test]
    fn stale_background_messages_do_not_match_a_new_project() {
        assert!(generation_matches(7, 7));
        assert!(!generation_matches(6, 7));
    }

    #[test]
    fn analysis_result_must_match_request_identifiers() {
        let result = json!({
            "job_id": "ui-job-exact",
            "capability": "variant.stats.v1"
        });

        assert!(analysis_result_matches(
            &result,
            "ui-job-exact",
            "variant.stats.v1"
        ));
        assert!(!analysis_result_matches(
            &result,
            "ui-job-other",
            "variant.stats.v1"
        ));
    }

    #[test]
    fn analysis_export_names_follow_and_sanitize_the_capability() {
        assert_eq!(
            analysis_export_basename(&json!({"capability": "sequence.stats.v1"})),
            "sequence-stats-v1"
        );
        assert_eq!(
            analysis_export_basename(&json!({
                "capability": "../../Structure.PDB Summary.v1\\result"
            })),
            "structure-pdb-summary-v1-result"
        );
        assert_eq!(
            analysis_export_basename(&json!({"capability": "CON"})),
            "analysis-con"
        );
        assert_eq!(analysis_export_basename(&json!({})), "analysis-result");
    }

    #[test]
    fn dependency_notice_platforms_map_to_their_release_targets() {
        assert!(notice_platform_target_pair_is_valid(
            "windows",
            "x86_64-pc-windows-gnu"
        ));
        assert!(notice_platform_target_pair_is_valid(
            "debian",
            "x86_64-unknown-linux-gnu"
        ));
        assert!(notice_platform_target_pair_is_valid(
            "arch",
            "x86_64-unknown-linux-gnu"
        ));
        assert!(!notice_platform_target_pair_is_valid(
            "windows",
            "x86_64-unknown-linux-gnu"
        ));
        assert!(!notice_platform_target_pair_is_valid(
            "debian",
            "x86_64-pc-windows-gnu"
        ));
    }

    #[test]
    fn rapidly_created_job_ids_are_unique() {
        let ids = (0..1_000).map(|_| new_job_id()).collect::<HashSet<_>>();
        assert_eq!(ids.len(), 1_000);
    }

    #[test]
    fn every_document_menu_entry_has_both_locales() {
        for capability in DOCUMENTED_CAPABILITIES {
            assert!(capability_document(capability, Language::ZhCn).is_some());
            assert!(capability_document(capability, Language::EnUs).is_some());
        }
    }

    #[test]
    fn documentation_loads_from_an_external_root_with_embedded_fallback() {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-ui-docs-{}-{}",
            std::process::id(),
            super::new_job_id()
        ));
        let directory = root.join("capabilities/sequence.stats.v1");
        fs::create_dir_all(&directory).expect("create external docs directory");
        fs::write(directory.join("en-US.md"), "# External documentation\n").expect("write doc");

        let document =
            capability_document_from_root(Some(&root), "sequence.stats.v1", Language::EnUs)
                .expect("external document");
        assert_eq!(document, "# External documentation\n");
        assert!(
            capability_document_from_root(Some(&root), "sequence.stats.v1", Language::ZhCn)
                .is_none(),
            "missing locale in the external root must not leak the embedded copy"
        );
        let fallback =
            capability_document("dataset.inspect.v1", Language::EnUs).expect("document available");
        assert!(fallback.starts_with("# "));
        fs::remove_dir_all(root).expect("remove external docs directory");
    }

    #[test]
    fn malformed_dataset_is_never_runnable() {
        let result = json!({
            "result": {
                "support": "supported",
                "warnings": [],
                "errors": [{"code": "truncated", "message": "truncated FASTQ"}]
            }
        });

        assert_eq!(inspection_state(&result), DatasetState::Invalid);
        assert!(!inspection_is_runnable(&result));
    }

    #[test]
    fn recognized_unsupported_dataset_stays_non_runnable() {
        let result = json!({
            "result": {
                "support": "recognized-unsupported",
                "warnings": [{"code": "planned", "message": "not implemented"}],
                "errors": []
            }
        });

        assert_eq!(inspection_state(&result), DatasetState::Warning);
        assert!(!inspection_is_runnable(&result));
    }

    #[test]
    fn supported_dataset_with_warning_can_run() {
        let result = json!({
            "result": {
                "support": "supported",
                "warnings": [{"code": "extension", "message": "extension mismatch"}],
                "errors": []
            }
        });

        assert_eq!(inspection_state(&result), DatasetState::Warning);
        assert!(inspection_is_runnable(&result));
    }

    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
    ))]
    #[test]
    fn loads_a_matching_packaged_dependency_notice_pair() {
        let root = notice_test_directory();
        fs::create_dir_all(&root).expect("notice directory");
        let (platform, target_triple) = current_notice_identity();
        let report = sample_notice_report(platform, target_triple);
        let text = render_dependency_notice_report(&report).expect("render notice report");
        fs::write(root.join("THIRD_PARTY_DEPENDENCIES.txt"), text).expect("text notice");
        fs::write(
            root.join("THIRD_PARTY_DEPENDENCIES.json"),
            serde_json::to_vec(&report).expect("JSON notice"),
        )
        .expect("write JSON notice");

        let notices = load_dependency_notices_from(&root).expect("notice pair");

        assert_eq!(notices.package_count, 1);
        assert!(
            notices
                .lines
                .iter()
                .any(|line| line.contains("Third-Party Cargo Dependency Notices"))
        );
        fs::remove_dir_all(root).expect("remove notice directory");
    }

    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
    ))]
    #[test]
    fn rejects_dependency_notice_text_from_a_different_report() {
        let root = notice_test_directory();
        fs::create_dir_all(&root).expect("notice directory");
        let (platform, target_triple) = current_notice_identity();
        let mut report = sample_notice_report(platform, target_triple);
        let text = render_dependency_notice_report(&report).expect("render notice report");
        report["cargo_version"] = json!("cargo 9.99.0 (different report)");
        fs::write(root.join("THIRD_PARTY_DEPENDENCIES.txt"), text).expect("text notice");
        fs::write(
            root.join("THIRD_PARTY_DEPENDENCIES.json"),
            serde_json::to_vec(&report).expect("JSON notice"),
        )
        .expect("write JSON notice");

        let error = load_dependency_notices_from(&root).expect_err("unbound pair must fail");

        assert!(error.contains("does not match the report"));
        fs::remove_dir_all(root).expect("remove notice directory");
    }

    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
    ))]
    #[test]
    fn rejects_dependency_notice_for_another_build_target() {
        let root = notice_test_directory();
        fs::create_dir_all(&root).expect("notice directory");
        let (platform, target_triple) = if cfg!(target_os = "windows") {
            ("debian", "x86_64-unknown-linux-gnu")
        } else {
            ("windows", "x86_64-pc-windows-gnu")
        };
        let report = sample_notice_report(platform, target_triple);
        let text = render_dependency_notice_report(&report).expect("render notice report");
        fs::write(root.join("THIRD_PARTY_DEPENDENCIES.txt"), text).expect("text notice");
        fs::write(
            root.join("THIRD_PARTY_DEPENDENCIES.json"),
            serde_json::to_vec(&report).expect("JSON notice"),
        )
        .expect("write JSON notice");

        let error = load_dependency_notices_from(&root).expect_err("wrong target must fail");

        assert!(error.contains("not this application build"));
        fs::remove_dir_all(root).expect("remove notice directory");
    }

    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
    ))]
    fn notice_test_directory() -> PathBuf {
        std::env::temp_dir().join(format!("linxira-bio-ui-notices-{}", new_job_id()))
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"))]
    fn current_notice_identity() -> (&'static str, &'static str) {
        ("windows", "x86_64-pc-windows-gnu")
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    fn current_notice_identity() -> (&'static str, &'static str) {
        ("debian", "x86_64-unknown-linux-gnu")
    }

    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64", target_env = "gnu"),
        all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")
    ))]
    fn sample_notice_report(platform: &str, target_triple: &str) -> serde_json::Value {
        json!({
            "$schema": "https://linxira.org/schemas/bio/third-party-dependencies.v1.json",
            "schema_version": "1",
            "generator_version": "1",
            "cargo_version": "cargo 1.92.0 (000000000 2026-01-01)",
            "platform": platform,
            "target_triple": target_triple,
            "cargo_lock_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "override_manifest_sha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "release_roots": [{"name": "linxira-bio-ui", "version": "0.1.0"}],
            "dependency_count": 1,
            "dependencies": [{
                "name": "example",
                "version": "1.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "repository": "https://example.invalid/example",
                "license_expression": "MIT",
                "active_features": ["default"],
                "documents": [{
                    "origin": "crate-package",
                    "path": "LICENSE-MIT",
                    "sha256": "2222222222222222222222222222222222222222222222222222222222222222",
                    "source_url": "https://crates.io/crates/example/1.0.0"
                }],
                "vcs": {
                    "revision": "3333333333333333333333333333333333333333",
                    "path": "crates/example"
                }
            }],
            "license_texts": [{
                "sha256": "2222222222222222222222222222222222222222222222222222222222222222",
                "text": "Example license text\n"
            }]
        })
    }
}
