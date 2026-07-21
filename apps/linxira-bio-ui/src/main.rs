#![forbid(unsafe_code)]

use eframe::egui;
use linxira_bio_protocol::{ExecutionMode, ExecutionRequest, JobRequest, SCHEMA_VERSION};
use linxira_bio_worker::execute_request;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
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
}

#[derive(Clone, Copy)]
enum EnvironmentJob {
    Audit,
    Plan,
}

type UiJobResult = Result<String, String>;

struct BioApp {
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
}

impl BioApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::light());
        let mut app = Self {
            page: Page::Environment,
            input_path: String::new(),
            sequence_status: "Ready for a local analysis.".to_owned(),
            sequence_result: None,
            sequence_receiver: None,
            sequence_running: false,
            environment_status: "Auditing the local environment...".to_owned(),
            environment_result: None,
            environment_receiver: None,
            environment_running: false,
            environment_profile: "full-local".to_owned(),
        };
        app.start_environment_job(EnvironmentJob::Audit);
        app
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
        self.sequence_status = "Running sequence.stats.v1 locally...".to_owned();

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
        let (capability, parameters, status) = match kind {
            EnvironmentJob::Audit => (
                "environment.audit.v1",
                serde_json::json!({}),
                "Auditing the local environment...",
            ),
            EnvironmentJob::Plan => (
                "environment.plan.v1",
                serde_json::json!({"profile": profile}),
                "Building an installation plan...",
            ),
        };
        let (sender, receiver) = mpsc::channel();
        self.environment_receiver = Some(receiver);
        self.environment_running = true;
        self.environment_result = None;
        self.environment_status = status.to_owned();

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
                    self.sequence_status = "Analysis completed.".to_owned();
                }
                Err(error) => {
                    self.sequence_status = format!("Worker returned invalid JSON: {error}");
                }
            },
            Err(error) => {
                self.sequence_status = format!("Analysis failed: {error}");
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
                        EnvironmentJob::Audit => "Environment audit completed.",
                        EnvironmentJob::Plan => "Installation plan completed. No changes applied.",
                    }
                    .to_owned();
                }
                Err(error) => {
                    self.environment_status = format!("Worker returned invalid JSON: {error}");
                }
            },
            Err(error) => {
                self.environment_status = format!("Environment operation failed: {error}");
            }
        }
    }

    fn show_navigation(&mut self, ui: &mut egui::Ui) {
        ui.heading("Linxira Bio");
        ui.label("Native analysis SDK");
        ui.separator();
        ui.selectable_value(&mut self.page, Page::Environment, "Environment");
        ui.selectable_value(
            &mut self.page,
            Page::SequenceStatistics,
            "Sequence statistics",
        );
        ui.add_enabled(false, egui::Button::new("FASTQ quality control"));
        ui.add_enabled(false, egui::Button::new("Genome intervals"));
        ui.add_enabled(false, egui::Button::new("Alignment files"));
        ui.add_enabled(false, egui::Button::new("Variants"));
        ui.add_enabled(false, egui::Button::new("Expression matrices"));
        ui.add_enabled(false, egui::Button::new("Protein structures"));
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.small("AGPL-3.0-or-later");
            ui.small("Windows first | Debian | Arch");
        });
    }

    fn show_environment(&mut self, ui: &mut egui::Ui) {
        ui.heading("Local environment");
        ui.add_space(8.0);

        let mut run_audit = false;
        let mut build_plan = false;
        ui.horizontal(|ui| {
            run_audit = ui
                .add_enabled(
                    !self.environment_running,
                    egui::Button::new("Refresh audit"),
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
                    egui::Button::new("Build install plan"),
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
            "environment.audit.v1" => show_environment_audit(ui, payload),
            "environment.plan.v1" => show_environment_plan(ui, payload),
            _ => {}
        }
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Raw environment JSON").show(ui, |ui| {
            ui.monospace(
                serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()),
            );
        });
    }

    fn show_sequence_statistics(&mut self, ui: &mut egui::Ui) {
        ui.heading("FASTA sequence statistics");
        ui.label(
            "Compute record count, lengths, N50/L50, auN, GC percentage, and N content locally.",
        );
        ui.add_space(12.0);

        ui.label("Input FASTA path");
        ui.add(
            egui::TextEdit::singleline(&mut self.input_path)
                .desired_width(f32::INFINITY)
                .hint_text("C:\\data\\assembly.fasta or /data/assembly.fasta"),
        );
        ui.add_space(8.0);

        let can_run = !self.sequence_running && !self.input_path.trim().is_empty();
        if ui
            .add_enabled(can_run, egui::Button::new("Run local analysis"))
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
            ui.heading("Result");
            if let Some(values) = result.get("result").and_then(Value::as_object) {
                egui::Grid::new("sequence-statistics-result")
                    .striped(true)
                    .min_col_width(180.0)
                    .show(ui, |ui| {
                        for (key, value) in values {
                            ui.label(key);
                            ui.monospace(value.to_string());
                            ui.end_row();
                        }
                    });
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new("Raw result JSON").show(ui, |ui| {
                ui.monospace(
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string()),
                );
            });
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
                            });
                        },
                    );
                },
            );
        });
    }
}

fn show_environment_audit(ui: &mut egui::Ui, audit: &Value) {
    if let Some(platform) = audit.get("platform") {
        ui.label(format!(
            "Platform: {} {}",
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
    ui.add_space(6.0);
    egui::Grid::new("environment-audit-tools")
        .striped(true)
        .min_col_width(140.0)
        .show(ui, |ui| {
            ui.strong("Tool");
            ui.strong("Status");
            ui.strong("Version");
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
                    ui.label(if available { "Available" } else { "Missing" });
                    ui.monospace(tool.get("version").and_then(Value::as_str).unwrap_or("-"));
                    ui.end_row();
                }
            }
        });
}

fn show_environment_plan(ui: &mut egui::Ui, plan: &Value) {
    if let Some(description) = plan.get("description").and_then(Value::as_str) {
        ui.label(description);
    }
    ui.add_space(6.0);
    egui::Grid::new("environment-plan-actions")
        .striped(true)
        .min_col_width(120.0)
        .show(ui, |ui| {
            ui.strong("Tool");
            ui.strong("Action");
            ui.strong("Method");
            ui.strong("Package");
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
                        action
                            .get("state")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown"),
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

fn new_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("ui-{millis}")
}
