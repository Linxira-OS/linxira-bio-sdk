#![forbid(unsafe_code)]

use eframe::egui;
use linxira_bio_protocol::{ExecutionMode, ExecutionRequest, JobRequest, SCHEMA_VERSION};
use linxira_bio_worker::execute_request;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{
    Arc,
    mpsc::{self, Receiver},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_100.0, 720.0])
            .with_min_inner_size([820.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Linxira Bio SDK",
        options,
        Box::new(|context| Ok(Box::new(BioApp::new(context)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Environment,
    SequenceStatistics,
    Documentation,
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

type UiJobResult = Result<String, String>;

const DOCUMENTED_CAPABILITIES: &[&str] = &[
    "sequence.stats.v1",
    "environment.audit.v1",
    "environment.plan.v1",
    "runtime.catalog.v1",
    "system.doctor.v1",
    "system.worker.v1",
];

struct BioApp {
    language: Language,
    cjk_font_loaded: bool,
    page: Page,
    input_path: String,
    sequence_status: String,
    sequence_result: Option<Value>,
    sequence_receiver: Option<Receiver<UiJobResult>>,
    sequence_running: bool,
    environment_status: String,
    environment_result: Option<Value>,
    environment_receiver: Option<Receiver<(EnvironmentJob, UiJobResult)>>,
    environment_running: bool,
    environment_profile: String,
    document_capability: String,
}

impl BioApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::light());
        let cjk_font_loaded = install_cjk_font(&context.egui_ctx);
        let mut app = Self {
            language: Language::ZhCn,
            cjk_font_loaded,
            page: Page::Environment,
            input_path: String::new(),
            sequence_status: "已准备好进行本地分析。".to_owned(),
            sequence_result: None,
            sequence_receiver: None,
            sequence_running: false,
            environment_status: "正在审计本地环境...".to_owned(),
            environment_result: None,
            environment_receiver: None,
            environment_running: false,
            environment_profile: "full-local".to_owned(),
            document_capability: "sequence.stats.v1".to_owned(),
        };
        app.start_environment_job(EnvironmentJob::Audit);
        app
    }

    fn text(&self, zh_cn: &'static str, en_us: &'static str) -> &'static str {
        self.language.text(zh_cn, en_us)
    }

    fn start_sequence_statistics(&mut self) {
        let input_path = self.input_path.trim().to_owned();
        if input_path.is_empty() || self.sequence_running {
            return;
        }

        let (sender, receiver) = mpsc::channel();
        self.sequence_receiver = Some(receiver);
        self.sequence_running = true;
        self.sequence_result = None;
        self.sequence_status = self
            .text(
                "正在本地运行 sequence.stats.v1...",
                "Running sequence.stats.v1 locally...",
            )
            .to_owned();

        thread::spawn(move || {
            let mut inputs = BTreeMap::new();
            inputs.insert("fasta".to_owned(), input_path);
            let request = JobRequest {
                schema_version: SCHEMA_VERSION.to_owned(),
                job_id: new_job_id(),
                capability: "sequence.stats.v1".to_owned(),
                inputs,
                execution: ExecutionRequest {
                    mode: ExecutionMode::LocalCpu,
                },
                parameters: serde_json::json!({}),
            };
            let result =
                execute_request(request, Path::new(".")).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
    }

    fn start_environment_job(&mut self, kind: EnvironmentJob) {
        if self.environment_running {
            return;
        }

        let profile = self.environment_profile.clone();
        let (capability, parameters) = match kind {
            EnvironmentJob::Audit => ("environment.audit.v1", serde_json::json!({})),
            EnvironmentJob::Plan => (
                "environment.plan.v1",
                serde_json::json!({"profile": profile}),
            ),
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
                self.text("正在生成安装计划...", "Building an installation plan...")
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

    fn poll_sequence_job(&mut self) {
        let message = self
            .sequence_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some(message) = message else {
            return;
        };

        self.sequence_receiver = None;
        self.sequence_running = false;
        match message {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(result) => {
                    self.sequence_result = Some(result);
                    self.sequence_status =
                        self.text("分析已完成。", "Analysis completed.").to_owned();
                }
                Err(error) => {
                    self.sequence_status = match self.language {
                        Language::ZhCn => format!("Worker 返回了无效 JSON：{error}"),
                        Language::EnUs => format!("Worker returned invalid JSON: {error}"),
                    };
                }
            },
            Err(error) => {
                self.sequence_status = match self.language {
                    Language::ZhCn => format!("分析失败：{error}"),
                    Language::EnUs => format!("Analysis failed: {error}"),
                };
            }
        }
    }

    fn poll_environment_job(&mut self) {
        let message = self
            .environment_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let Some((kind, message)) = message else {
            return;
        };

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
                            "安装计划已生成，未对系统进行任何更改。",
                            "Installation plan completed. No changes applied.",
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
        ui.heading("Linxira Bio");
        ui.label(self.text("本地生物信息学工作台", "Native bioinformatics workbench"));
        ui.add_space(6.0);
        let selected_language = match self.language {
            Language::ZhCn => "简体中文",
            Language::EnUs => "English",
        };
        egui::ComboBox::from_id_salt("interface-language")
            .selected_text(selected_language)
            .width(120.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.language, Language::ZhCn, "简体中文");
                ui.selectable_value(&mut self.language, Language::EnUs, "English");
            });
        if self.language == Language::ZhCn && !self.cjk_font_loaded {
            ui.colored_label(egui::Color32::from_rgb(170, 70, 40), "未找到系统中文字体");
        }
        ui.separator();
        ui.selectable_value(
            &mut self.page,
            Page::Environment,
            self.language.text("运行环境", "Environment"),
        );
        ui.selectable_value(
            &mut self.page,
            Page::SequenceStatistics,
            self.language.text("序列统计", "Sequence statistics"),
        );
        ui.selectable_value(
            &mut self.page,
            Page::Documentation,
            self.language.text("离线文档", "Offline documentation"),
        );
        ui.add_enabled(
            false,
            egui::Button::new(
                self.language
                    .text("FASTQ 质量控制", "FASTQ quality control"),
            ),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.language.text("基因组区间", "Genome intervals")),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.language.text("比对文件", "Alignment files")),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.language.text("变异分析", "Variants")),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.language.text("表达矩阵", "Expression matrices")),
        );
        ui.add_enabled(
            false,
            egui::Button::new(self.language.text("蛋白质结构", "Protein structures")),
        );
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("AGPL-3.0-or-later");
            ui.small(self.language.text(
                "Windows 优先 | Debian | Arch",
                "Windows first | Debian | Arch",
            ));
        });
    }

    fn show_environment(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("本地运行环境", "Local environment"));
        ui.label(self.text(
            "检查本机工具并生成只读安装计划。安装功能尚未开放。",
            "Audit local tools and build a read-only installation plan. Installation is not yet enabled.",
        ));
        ui.add_space(8.0);

        let mut run_audit = false;
        let mut build_plan = false;
        ui.horizontal_wrapped(|ui| {
            run_audit = ui
                .add_enabled(
                    !self.environment_running,
                    egui::Button::new(self.language.text("重新审计", "Refresh audit")),
                )
                .clicked();
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
            build_plan = ui
                .add_enabled(
                    !self.environment_running,
                    egui::Button::new(self.language.text("生成安装计划", "Build install plan")),
                )
                .clicked();
        });
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

    fn show_sequence_statistics(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("FASTA 序列统计", "FASTA sequence statistics"));
        ui.label(self.text(
            "在本地计算记录数、长度、N50/L50、auN、GC 比例和 N 含量。",
            "Compute record count, lengths, N50/L50, auN, GC percentage, and N content locally.",
        ));
        ui.add_space(12.0);

        ui.label(self.text("输入 FASTA 路径", "Input FASTA path"));
        ui.add(
            egui::TextEdit::singleline(&mut self.input_path)
                .desired_width(f32::INFINITY)
                .hint_text("C:\\data\\assembly.fasta or /data/assembly.fasta"),
        );
        ui.add_space(8.0);

        let can_run = !self.sequence_running && !self.input_path.trim().is_empty();
        if ui
            .add_enabled(
                can_run,
                egui::Button::new(self.language.text("运行本地分析", "Run local analysis")),
            )
            .clicked()
        {
            self.start_sequence_statistics();
        }
        if self.sequence_running {
            ui.spinner();
        }
        ui.label(&self.sequence_status);
        ui.separator();

        if let Some(result) = &self.sequence_result {
            ui.heading(self.text("结果", "Result"));
            if let Some(values) = result.get("result").and_then(Value::as_object) {
                egui::Grid::new("sequence-statistics-result")
                    .striped(true)
                    .min_col_width(180.0)
                    .show(ui, |ui| {
                        for (key, value) in values {
                            ui.label(metric_label(key, self.language));
                            ui.monospace(value.to_string());
                            ui.end_row();
                        }
                    });
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new(self.text("原始结果 JSON", "Raw result JSON")).show(
                ui,
                |ui| {
                    ui.monospace(
                        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()),
                    );
                },
            );
        }
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
        self.poll_sequence_job();
        self.poll_environment_job();
        if self.sequence_running || self.environment_running {
            context.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::left_to_right(egui::Align::TOP),
                |ui| {
                    let height = ui.available_height();
                    ui.allocate_ui_with_layout(
                        egui::vec2(230.0, height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| self.show_navigation(ui),
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| match self.page {
                                Page::Environment => self.show_environment(ui),
                                Page::SequenceStatistics => self.show_sequence_statistics(ui),
                                Page::Documentation => self.show_documentation(ui),
                            });
                        },
                    );
                },
            );
        });
    }
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
                "Windows 需要 WSL Debian 或 Docker 中的任意一个"
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
                    "Bioconda 不提供原生 Windows 包；请通过 WSL Debian 运行 Bioconda 环境。",
                    "Bioconda does not publish native Windows packages; use WSL Debian for Bioconda environments.",
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
    if let Some(description) = localized_profile_description(profile, language)
        .or_else(|| plan.get("description").and_then(Value::as_str))
    {
        ui.label(description);
    }
    ui.add_space(6.0);
    egui::Grid::new("environment-plan-actions")
        .striped(true)
        .min_col_width(120.0)
        .show(ui, |ui| {
            ui.strong(language.text("工具", "Tool"));
            ui.strong(language.text("操作", "Action"));
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
                            "unsupported" => language.text("不支持", "unsupported"),
                            _ => language.text("未知", "unknown"),
                        },
                    );
                    ui.label(
                        action
                            .get("strategy")
                            .and_then(Value::as_str)
                            .unwrap_or("-"),
                    );
                    ui.monospace(action.get("package").and_then(Value::as_str).unwrap_or("-"));
                    ui.end_row();
                }
            }
        });

    if let Some(warnings) = plan.get("warnings").and_then(Value::as_array) {
        for warning in warnings.iter().filter_map(Value::as_str) {
            ui.colored_label(egui::Color32::from_rgb(160, 90, 0), warning);
        }
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
        _ => key,
    }
}

fn document_title(capability: &str, language: Language) -> &'static str {
    match capability {
        "sequence.stats.v1" => language.text("FASTA 序列统计", "FASTA sequence statistics"),
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
        ("sequence.stats.v1", Language::ZhCn) => Some(include_str!(
            "../../../docs/capabilities/sequence.stats.v1/zh-CN.md"
        )),
        ("sequence.stats.v1", Language::EnUs) => Some(include_str!(
            "../../../docs/capabilities/sequence.stats.v1/en-US.md"
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

fn install_cjk_font(context: &egui::Context) -> bool {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    let Some(font_data) = CANDIDATES.iter().find_map(|path| fs::read(path).ok()) else {
        return false;
    };

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
    true
}

fn new_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("ui-{millis}")
}
