#![forbid(unsafe_code)]

use eframe::egui;
use linxira_bio_export::export_value;
use linxira_bio_protocol::{ExecutionMode, ExecutionRequest, JobRequest, SCHEMA_VERSION};
use linxira_bio_worker::execute_request;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender, TryRecvError},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: preferred_renderer(),
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Linxira Bio SDK",
        options,
        Box::new(|context| Ok(Box::new(BioApp::new(context)))),
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
    Environment,
    Documentation,
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
            Self::Inspecting => egui::Color32::from_rgb(49, 103, 158),
            Self::Ready => egui::Color32::from_rgb(32, 116, 86),
            Self::Warning => egui::Color32::from_rgb(176, 104, 24),
            Self::Invalid => egui::Color32::from_rgb(174, 57, 57),
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
    "sequence.stats.v1",
    "fastq.qc.v1",
    "variant.stats.v1",
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
    project_generation: u64,
    inspection_sender: Sender<InspectionMessage>,
    inspection_receiver: Receiver<InspectionMessage>,
    inspection_queue: VecDeque<InspectionTask>,
    active_inspections: usize,
    selected_capability: String,
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
}

impl BioApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
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
            project_generation: 0,
            inspection_sender,
            inspection_receiver,
            inspection_queue: VecDeque::new(),
            active_inspections: 0,
            selected_capability: "sequence.stats.v1".to_owned(),
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
        };
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

    fn queue_path(&mut self, path: PathBuf) {
        let path = fs::canonicalize(&path).unwrap_or(path);
        if !path.is_file() {
            self.import_status = match self.language {
                Language::ZhCn => format!("无法导入：{} 不是可读取文件。", path.display()),
                Language::EnUs => {
                    format!("Cannot import: {} is not a readable file.", path.display())
                }
            };
            return;
        }

        let normalized = path.to_string_lossy().into_owned();
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
            return;
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
        let Some(route) = analysis_route_for_format(&format) else {
            return;
        };
        if self.analysis_running || !runnable || self.selected_capability != route.capability {
            return;
        }

        let job_id = new_job_id();
        let request = build_analysis_request(&job_id, route, &dataset_path);
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
                .color(egui::Color32::from_rgb(23, 81, 72)),
        );
        ui.small(self.text("本地分析工作台", "Local analysis workbench"));
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
            Page::Environment,
            language.text("运行环境", "Environment"),
        );
        nav_button(
            ui,
            &mut self.page,
            Page::Documentation,
            language.text("离线文档", "Offline docs"),
        );

        ui.add_space(18.0);
        ui.weak(self.text("项目数据", "PROJECT DATA"));
        ui.add_space(4.0);
        if self.datasets.is_empty() {
            ui.small(self.text("尚未导入数据", "No datasets imported"));
        } else {
            egui::ScrollArea::vertical()
                .id_salt("navigation-datasets")
                .max_height(280.0)
                .show(ui, |ui| {
                    for index in 0..self.datasets.len() {
                        let dataset = &self.datasets[index];
                        let selected = self.selected_dataset == Some(index);
                        let label = egui::RichText::new(&dataset.name).size(13.0);
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
                egui::Color32::from_rgb(49, 103, 158)
            } else if self.environment_result.is_some() {
                egui::Color32::from_rgb(32, 116, 86)
            } else {
                egui::Color32::from_rgb(176, 104, 24)
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
                let context_width = 270.0;
                let content_width = (ui.available_width() - context_width - 18.0).max(420.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| self.show_workspace_tab(ui),
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(context_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| self.show_workspace_context(ui),
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
            egui::Color32::from_rgb(226, 241, 237)
        } else {
            egui::Color32::from_rgb(246, 248, 247)
        };
        egui::Frame::NONE
            .fill(drop_fill)
            .stroke(egui::Stroke::new(
                1.0,
                if hovered {
                    egui::Color32::from_rgb(40, 126, 108)
                } else {
                    egui::Color32::from_rgb(190, 199, 196)
                },
            ))
            .corner_radius(egui::CornerRadius::same(6))
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
                                    "fa", "fasta", "fna", "faa", "fq", "fastq", "csv", "tsv",
                                    "bed", "gff", "gff3", "gtf", "vcf", "sam", "bam", "gz",
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
            self.queue_path(path);
            self.import_path.clear();
        }
        ui.colored_label(egui::Color32::from_rgb(73, 88, 83), &self.import_status);

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
                ui.label("FASTA, FASTQ, CSV/TSV, BED, GFF3/GTF, VCF, SAM");
                ui.label(".gz / BGZF");
                ui.end_row();
                ui.colored_label(
                    DatasetState::Warning.color(),
                    self.text("识别但暂不分析", "Recognize only"),
                );
                ui.label("BAM, BCF, CRAM, HDF5/H5AD, LOOM, RDS, PDB/mmCIF");
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
            route.is_some_and(|route| route.capability == self.selected_capability.as_str());

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
                for capability in ["sequence.stats.v1", "fastq.qc.v1", "variant.stats.v1"] {
                    ui.selectable_value(
                        &mut self.selected_capability,
                        capability.to_owned(),
                        capability_title(capability, self.language),
                    );
                }
                ui.separator();
                for capability in [
                    "interval.intersect.v1",
                    "alignment.qc.v1",
                    "expression.matrix.qc.v1",
                ] {
                    ui.add_enabled(
                        false,
                        egui::Button::new(format!(
                            "{}  [{}]",
                            capability_title(capability, self.language),
                            self.text("计划中", "planned")
                        )),
                    );
                }
            });

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
            });

        if !dataset_ready || !capability_matches {
            ui.add_space(8.0);
            let message = if !dataset_ready {
                self.text(
                    "只有检查通过的数据才能运行本地分析。",
                    "Local analysis requires a dataset that passed inspection.",
                )
                .to_owned()
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
        let can_run = dataset_ready && capability_matches && !self.analysis_running;
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
        ui.add_space(12.0);
        section_title(ui, self.text("统计摘要", "Statistics summary"));
        render_metrics(ui, payload, self.language);

        ui.add_space(14.0);
        section_title(ui, self.text("导出", "Export"));
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
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.text("导出分析结果", "Export analysis result"))
            .set_file_name(format!("sequence-statistics.{extension}"))
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
            render_markdown_document(ui, document);
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(170, 70, 40),
                self.text("文档未找到。", "Documentation was not found."),
            );
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
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::left_to_right(egui::Align::TOP),
                |ui| {
                    let height = ui.available_height();
                    ui.allocate_ui_with_layout(
                        egui::vec2(190.0, height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| self.show_navigation(ui),
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| match self.page {
                            Page::Workspace => self.show_workspace(ui),
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
                        },
                    );
                },
            );
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
        "vcf" => Some(AnalysisRoute {
            capability: "variant.stats.v1",
            input_role: "vcf",
        }),
        _ => None,
    }
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

    let visuals = &mut style.visuals;
    visuals.panel_fill = egui::Color32::from_rgb(249, 250, 249);
    visuals.window_fill = egui::Color32::from_rgb(252, 253, 252);
    visuals.faint_bg_color = egui::Color32::from_rgb(238, 242, 240);
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.code_bg_color = egui::Color32::from_rgb(235, 239, 237);
    visuals.selection.bg_fill = egui::Color32::from_rgb(42, 123, 105);
    visuals.hyperlink_color = egui::Color32::from_rgb(32, 101, 145);
    visuals.warn_fg_color = egui::Color32::from_rgb(176, 104, 24);
    visuals.error_fg_color = egui::Color32::from_rgb(174, 57, 57);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(4);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(239, 243, 241);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(224, 236, 232);
    context.set_style_of(egui::Theme::Light, style);
}

fn nav_button(ui: &mut egui::Ui, page: &mut Page, target: Page, label: &str) {
    let selected = *page == target;
    if ui
        .add_sized(
            [ui.available_width(), 36.0],
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
            [ui.available_width(), 34.0],
            egui::Button::new(label).selected(*tab == target),
        )
        .clicked()
    {
        *tab = target;
    }
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).strong().size(18.0));
}

fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    action: &str,
    tab: &mut WorkspaceTab,
    target: WorkspaceTab,
) {
    ui.add_space(36.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(title).strong().size(18.0));
        ui.add_space(8.0);
        if ui.button(action).clicked() {
            *tab = target;
        }
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
        "xlsx" => "xlsx",
        "zip" => "zip",
        _ => "unknown",
    }
}

fn capability_title(capability: &str, language: Language) -> &'static str {
    match capability {
        "sequence.stats.v1" => language.text("FASTA 序列统计", "FASTA sequence statistics"),
        "fastq.qc.v1" => language.text("FASTQ 质量控制", "FASTQ quality control"),
        "interval.intersect.v1" => language.text("基因组区间", "Genome intervals"),
        "variant.stats.v1" => language.text("变异统计", "Variant statistics"),
        "alignment.qc.v1" => language.text("比对质量控制", "Alignment quality control"),
        "expression.matrix.qc.v1" => language.text("表达矩阵", "Expression matrix"),
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
                egui::Color32::from_rgb(160, 90, 0),
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
            ui.colored_label(egui::Color32::from_rgb(160, 90, 0), warning);
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
                ui.colored_label(egui::Color32::from_rgb(160, 70, 40), blocker);
            }
        }
    }

    if let Some(warnings) = plan.get("warnings").and_then(Value::as_array) {
        for warning in warnings.iter().filter_map(Value::as_str) {
            ui.colored_label(egui::Color32::from_rgb(160, 90, 0), warning);
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
        "mean_quality" => "平均质量值",
        "q20_percent" => "Q20 百分比",
        "q30_percent" => "Q30 百分比",
        "quality_encoding" => "质量编码",
        "applied_quality_offset" => "采用的质量偏移",
        "per_cycle" => "逐循环指标",
        "warnings" => "警告",
        "record_count" => "记录数",
        "sample_count" => "样本数",
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
        _ => key,
    }
}

fn document_title(capability: &str, language: Language) -> &'static str {
    match capability {
        "dataset.inspect.v1" => language.text("数据集检查", "Dataset inspection"),
        "table.export.v1" => language.text("表格导出", "Table export"),
        "sequence.stats.v1" => language.text("FASTA 序列统计", "FASTA sequence statistics"),
        "fastq.qc.v1" => language.text("FASTQ 质量控制", "FASTQ quality control"),
        "variant.stats.v1" => language.text("VCF 变异统计", "VCF variant statistics"),
        "environment.audit.v1" => language.text("环境审计", "Environment audit"),
        "environment.plan.v1" => language.text("环境计划", "Environment plan"),
        "runtime.catalog.v1" => language.text("运行时目录", "Runtime catalog"),
        "system.doctor.v1" => language.text("系统诊断", "System doctor"),
        "system.worker.v1" => language.text("本地任务 Worker", "Local job worker"),
        _ => language.text("未知能力", "Unknown capability"),
    }
}

fn capability_document(capability: &str, language: Language) -> Option<&'static str> {
    match (capability, language) {
        ("dataset.inspect.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/dataset.inspect.v1/zh-CN.md"
        )),
        ("dataset.inspect.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/dataset.inspect.v1/en-US.md"
        )),
        ("table.export.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/table.export.v1/zh-CN.md"
        )),
        ("table.export.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/table.export.v1/en-US.md"
        )),
        ("sequence.stats.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/sequence.stats.v1/zh-CN.md"
        )),
        ("sequence.stats.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/sequence.stats.v1/en-US.md"
        )),
        ("fastq.qc.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/fastq.qc.v1/zh-CN.md"
        )),
        ("fastq.qc.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/fastq.qc.v1/en-US.md"
        )),
        ("variant.stats.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/variant.stats.v1/zh-CN.md"
        )),
        ("variant.stats.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/variant.stats.v1/en-US.md"
        )),
        ("environment.audit.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/environment.audit.v1/zh-CN.md"
        )),
        ("environment.audit.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/environment.audit.v1/en-US.md"
        )),
        ("environment.plan.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/environment.plan.v1/zh-CN.md"
        )),
        ("environment.plan.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/environment.plan.v1/en-US.md"
        )),
        ("runtime.catalog.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/runtime.catalog.v1/zh-CN.md"
        )),
        ("runtime.catalog.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/runtime.catalog.v1/en-US.md"
        )),
        ("system.doctor.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/system.doctor.v1/zh-CN.md"
        )),
        ("system.doctor.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/system.doctor.v1/en-US.md"
        )),
        ("system.worker.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/system.worker.v1/zh-CN.md"
        )),
        ("system.worker.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/system.worker.v1/en-US.md"
        )),
        _ => None,
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
        AnalysisRoute, DOCUMENTED_CAPABILITIES, DatasetState, Language, analysis_result_matches,
        analysis_route_for_format, build_analysis_request, capability_document, generation_matches,
        inspection_is_runnable, inspection_state, new_job_id,
    };
    use linxira_bio_protocol::ExecutionMode;
    use serde_json::json;
    use std::collections::HashSet;

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
        assert_eq!(analysis_route_for_format("bam"), None);
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
}
