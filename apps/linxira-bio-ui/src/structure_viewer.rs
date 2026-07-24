use eframe::egui;
use flate2::read::MultiGzDecoder;
use linxira_bio_export::write_atomic_bytes;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread;

const MAX_COMPRESSED_STRUCTURE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STRUCTURE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ATOMS: usize = 100_000;
const MAX_BONDS: usize = 250_000;
const MAX_INTERACTIVE_ATOMS: usize = 25_000;
const MAX_INTERACTIVE_EDGES: usize = 50_000;
const SNAPSHOT_WIDTH: u32 = 1_600;
const SNAPSHOT_HEIGHT: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Representation {
    Backbone,
    BallAndStick,
    SpaceFilling,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorScheme {
    Element,
    Confidence,
    Chain,
}

#[derive(Debug, Clone)]
struct Atom {
    name: String,
    residue_name: String,
    residue_key: String,
    chain: String,
    element: String,
    position: [f32; 3],
    b_factor: f32,
    hetero: bool,
}

#[derive(Debug)]
struct StructureModel {
    atoms: Vec<Atom>,
    bonds: Vec<(usize, usize)>,
    backbone: Vec<(usize, usize)>,
    center: [f32; 3],
    radius: f32,
    chain_count: usize,
    residue_count: usize,
    confidence_available: bool,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedAtom {
    index: usize,
    position: egui::Pos2,
    depth: f32,
    perspective: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RenderGeometryKey {
    rect: egui::Rect,
    representation: Representation,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    show_hetero: bool,
    show_hydrogen: bool,
}

struct RenderGeometry {
    key: RenderGeometryKey,
    atoms: Vec<ProjectedAtom>,
    edges: Vec<(ProjectedAtom, ProjectedAtom)>,
    scale: f32,
    total_atom_count: usize,
    total_edge_count: usize,
}

type StructureLoadMessage = (PathBuf, Result<StructureModel, String>);

pub struct StructureViewer {
    model: Option<Arc<StructureModel>>,
    path: Option<PathBuf>,
    status: String,
    load_receiver: Option<Receiver<StructureLoadMessage>>,
    pending_load_path: Option<PathBuf>,
    export_receiver: Option<Receiver<Result<(PathBuf, u64), String>>>,
    export_running: bool,
    render_geometry: Option<RenderGeometry>,
    representation: Representation,
    color_scheme: ColorScheme,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    viewport_size: egui::Vec2,
    show_hetero: bool,
    show_hydrogen: bool,
    interpret_b_factors_as_plddt: bool,
}

impl Default for StructureViewer {
    fn default() -> Self {
        Self {
            model: None,
            path: None,
            status: String::new(),
            load_receiver: None,
            pending_load_path: None,
            export_receiver: None,
            export_running: false,
            render_geometry: None,
            representation: Representation::Backbone,
            color_scheme: ColorScheme::Confidence,
            yaw: -0.45,
            pitch: 0.3,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            viewport_size: egui::Vec2::ZERO,
            show_hetero: true,
            show_hydrogen: false,
            interpret_b_factors_as_plddt: false,
        }
    }
}

impl StructureViewer {
    pub fn has_model(&self) -> bool {
        self.model.is_some()
    }

    pub fn is_loading(&self) -> bool {
        self.load_receiver.is_some()
    }

    pub fn suggested_snapshot_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str())
            .map(|stem| format!("{stem}-structure.png"))
            .unwrap_or_else(|| "structure.png".to_owned())
    }

    pub fn save_png(&mut self, path: &Path, zh_cn: bool) {
        if self.export_running {
            self.status =
                localized(zh_cn, "PNG 正在导出。", "PNG export is already running.").to_owned();
            return;
        }
        let Some(model) = self.model.clone() else {
            self.status = localized(
                zh_cn,
                "PNG 导出失败：请先打开结构。",
                "PNG export failed: open a structure first.",
            )
            .to_owned();
            return;
        };
        let destination = path.to_owned();
        let thread_destination = destination.clone();
        let representation = self.representation;
        let color_scheme = self.color_scheme;
        let yaw = self.yaw;
        let pitch = self.pitch;
        let zoom = self.zoom;
        let pan = self.pan;
        let viewport_size = self.viewport_size;
        let show_hetero = self.show_hetero;
        let show_hydrogen = self.show_hydrogen;
        let (sender, receiver) = mpsc::channel();
        let spawn_result = thread::Builder::new()
            .name("linxira-structure-png-export".to_owned())
            .spawn(move || {
                let result = encode_snapshot_png(
                    SNAPSHOT_WIDTH,
                    SNAPSHOT_HEIGHT,
                    &model,
                    representation,
                    color_scheme,
                    yaw,
                    pitch,
                    zoom,
                    pan,
                    viewport_size,
                    show_hetero,
                    show_hydrogen,
                )
                .and_then(|bytes| {
                    write_atomic_bytes(&thread_destination, &bytes)
                        .map(|size| (thread_destination, size))
                        .map_err(|error| error.to_string())
                });
                let _ = sender.send(result);
            });
        match spawn_result {
            Ok(_) => {
                self.export_receiver = Some(receiver);
                self.export_running = true;
                self.status = if zh_cn {
                    format!("正在导出 PNG：{}", destination.display())
                } else {
                    format!("Exporting PNG: {}", destination.display())
                };
            }
            Err(error) => {
                self.status = if zh_cn {
                    format!("PNG 导出失败：无法启动后台任务：{error}")
                } else {
                    format!("PNG export failed: could not start background task: {error}")
                };
            }
        }
    }

    pub fn load_path(&mut self, path: impl AsRef<Path>, zh_cn: bool) {
        let path = path.as_ref().to_owned();
        let worker_path = path.clone();
        let (sender, receiver) = mpsc::channel();
        let spawn_result = thread::Builder::new()
            .name("linxira-structure-load".to_owned())
            .spawn(move || {
                let result = load_structure(&worker_path);
                let _ = sender.send((worker_path, result));
            });
        match spawn_result {
            Ok(_) => {
                self.load_receiver = Some(receiver);
                self.pending_load_path = Some(path.clone());
                self.status = if zh_cn {
                    format!("正在后台载入结构：{}", path.display())
                } else {
                    format!("Loading structure in the background: {}", path.display())
                };
            }
            Err(error) => {
                self.status = if zh_cn {
                    format!("结构载入失败：无法启动后台任务：{error}")
                } else {
                    format!("Failed to load structure: could not start background task: {error}")
                };
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, zh_cn: bool) {
        self.poll_load(zh_cn);
        self.poll_export(zh_cn);
        if self.is_loading() || self.export_running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        if self.model.is_none() {
            ui.add_space(22.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(localized(
                        zh_cn,
                        "打开本地 PDB 或 mmCIF 结构",
                        "Open a local PDB or mmCIF structure",
                    ))
                    .strong()
                    .size(16.0),
                );
                ui.small(localized(
                    zh_cn,
                    "AlphaFold 结果需要载入后明确启用 pLDDT 解释。",
                    "Explicitly enable pLDDT interpretation after loading an AlphaFold result.",
                ));
            });
            return;
        }
        let confidence_values_available = self
            .model
            .as_ref()
            .is_some_and(|model| model.confidence_available);

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.strong(localized(zh_cn, "表示", "Representation"));
            ui.selectable_value(
                &mut self.representation,
                Representation::Backbone,
                localized(zh_cn, "主链/线条", "Backbone / lines"),
            );
            ui.selectable_value(
                &mut self.representation,
                Representation::BallAndStick,
                localized(zh_cn, "球棍", "Ball-and-stick"),
            );
            ui.selectable_value(
                &mut self.representation,
                Representation::SpaceFilling,
                localized(zh_cn, "空间填充", "Space-filling"),
            );
            ui.separator();
            ui.strong(localized(zh_cn, "着色", "Color"));
            ui.selectable_value(
                &mut self.color_scheme,
                ColorScheme::Element,
                localized(zh_cn, "元素", "Element"),
            );
            ui.add_enabled_ui(
                confidence_values_available && self.interpret_b_factors_as_plddt,
                |ui| {
                    ui.selectable_value(&mut self.color_scheme, ColorScheme::Confidence, "pLDDT");
                },
            );
            ui.selectable_value(
                &mut self.color_scheme,
                ColorScheme::Chain,
                localized(zh_cn, "链", "Chain"),
            );
        });
        ui.horizontal_wrapped(|ui| {
            let confidence_response = ui.add_enabled(
                confidence_values_available,
                egui::Checkbox::new(
                    &mut self.interpret_b_factors_as_plddt,
                    localized(
                        zh_cn,
                        "这是 AlphaFold pLDDT",
                        "Interpret as AlphaFold pLDDT",
                    ),
                ),
            );
            confidence_response.on_hover_text(localized(
                zh_cn,
                "仅对 AlphaFold 输出启用；普通 PDB 的 B-factor 不是 pLDDT。",
                "Enable only for AlphaFold output; ordinary PDB B-factors are not pLDDT.",
            ));
            if self.interpret_b_factors_as_plddt {
                self.color_scheme = ColorScheme::Confidence;
            } else if self.color_scheme == ColorScheme::Confidence {
                self.color_scheme = ColorScheme::Element;
            }
            ui.checkbox(
                &mut self.show_hetero,
                localized(zh_cn, "显示配体/水", "Show ligands/water"),
            );
            ui.checkbox(
                &mut self.show_hydrogen,
                localized(zh_cn, "显示氢", "Show hydrogen"),
            );
            if ui
                .button(localized(zh_cn, "重置视角", "Reset view"))
                .clicked()
            {
                self.reset_view();
            }
            ui.small(localized(
                zh_cn,
                "左键旋转 · 右键平移 · 滚轮缩放 · 双击重置",
                "Drag to rotate · right-drag to pan · wheel to zoom · double-click to reset",
            ));
        });

        let width = ui.available_width().max(360.0);
        let height = (width * 0.58).clamp(390.0, 660.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
        self.viewport_size = rect.size();
        if response.double_clicked() {
            self.reset_view();
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = ui.input(|input| input.pointer.delta());
            self.yaw += delta.x * 0.012;
            self.pitch = (self.pitch + delta.y * 0.012).clamp(-1.5, 1.5);
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.pan += ui.input(|input| input.pointer.delta());
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(0.15, 12.0);
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(245, 248, 247));
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(192, 202, 198)),
            egui::StrokeKind::Inside,
        );
        let model = self.model.as_ref().expect("model checked above").clone();
        let key = RenderGeometryKey {
            rect,
            representation: self.representation,
            yaw: self.yaw,
            pitch: self.pitch,
            zoom: self.zoom,
            pan: self.pan,
            show_hetero: self.show_hetero,
            show_hydrogen: self.show_hydrogen,
        };
        if self
            .render_geometry
            .as_ref()
            .is_none_or(|geometry| geometry.key != key)
        {
            self.render_geometry = Some(build_render_geometry(
                key,
                &model,
                MAX_INTERACTIVE_ATOMS,
                MAX_INTERACTIVE_EDGES,
            ));
        }
        let geometry = self
            .render_geometry
            .as_ref()
            .expect("render geometry was initialized");
        render_model(&painter, &model, self.color_scheme, geometry);
        let lod_counts = (
            geometry.atoms.len(),
            geometry.total_atom_count,
            geometry.edges.len(),
            geometry.total_edge_count,
        );
        response.on_hover_cursor(egui::CursorIcon::Grab);

        ui.add_space(5.0);
        if lod_counts.0 < lod_counts.1 || lod_counts.2 < lod_counts.3 {
            ui.small(if zh_cn {
                format!(
                    "交互预览已启用 LOD：显示 {}/{} 个可见原子、{}/{} 条连接；PNG 导出仍使用完整结构。",
                    lod_counts.0, lod_counts.1, lod_counts.2, lod_counts.3
                )
            } else {
                format!(
                    "Interactive LOD: showing {}/{} visible atoms and {}/{} edges; PNG export still uses the complete structure.",
                    lod_counts.0, lod_counts.1, lod_counts.2, lod_counts.3
                )
            });
        }
        show_legend(ui, self.color_scheme, zh_cn);
        if let Some(path) = &self.path {
            ui.small(path.display().to_string());
        }
    }

    fn reset_view(&mut self) {
        self.yaw = -0.45;
        self.pitch = 0.3;
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
    }

    fn poll_load(&mut self, zh_cn: bool) {
        let Some(receiver) = &self.load_receiver else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(message) => Some(message),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some((
                self.pending_load_path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from("structure")),
                Err("load worker disconnected".to_owned()),
            )),
        };
        let Some((path, result)) = message else {
            return;
        };
        self.load_receiver = None;
        self.pending_load_path = None;
        match result {
            Ok(model) => {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_else(|| "structure".into());
                self.status = if zh_cn {
                    format!(
                        "已载入 {name}：{} 个原子，{} 个残基，{} 条链",
                        model.atoms.len(),
                        model.residue_count,
                        model.chain_count
                    )
                } else {
                    format!(
                        "Loaded {name}: {} atoms, {} residues, {} chains",
                        model.atoms.len(),
                        model.residue_count,
                        model.chain_count
                    )
                };
                self.color_scheme = ColorScheme::Element;
                self.interpret_b_factors_as_plddt = false;
                self.path = Some(path);
                self.model = Some(Arc::new(model));
                self.render_geometry = None;
                self.reset_view();
            }
            Err(error) => {
                self.status = if zh_cn {
                    format!("结构载入失败：{error}")
                } else {
                    format!("Failed to load structure: {error}")
                };
            }
        }
    }

    #[cfg(test)]
    fn snapshot_png(&self, width: u32, height: u32) -> Result<Vec<u8>, String> {
        if width < 64 || height < 64 {
            return Err("snapshot dimensions must be at least 64 by 64 pixels".to_owned());
        }
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| "open a structure before exporting a snapshot".to_owned())?;
        encode_snapshot_png(
            width,
            height,
            model,
            self.representation,
            self.color_scheme,
            self.yaw,
            self.pitch,
            self.zoom,
            self.pan,
            self.viewport_size,
            self.show_hetero,
            self.show_hydrogen,
        )
    }

    fn poll_export(&mut self, zh_cn: bool) {
        let Some(receiver) = &self.export_receiver else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("export worker disconnected".to_owned())),
        };
        let Some(result) = result else {
            return;
        };
        self.export_running = false;
        self.export_receiver = None;
        self.status = match result {
            Ok((path, size)) if zh_cn => {
                format!("已导出 PNG：{}（{size} 字节）", path.display())
            }
            Ok((path, size)) => format!("Exported PNG: {} ({size} bytes)", path.display()),
            Err(error) if zh_cn => format!("PNG 导出失败：{error}"),
            Err(error) => format!("PNG export failed: {error}"),
        };
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_snapshot_png(
    width: u32,
    height: u32,
    model: &StructureModel,
    representation: Representation,
    color_scheme: ColorScheme,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    viewport_size: egui::Vec2,
    show_hetero: bool,
    show_hydrogen: bool,
) -> Result<Vec<u8>, String> {
    if width < 64 || height < 64 {
        return Err("snapshot dimensions must be at least 64 by 64 pixels".to_owned());
    }
    let target_size = egui::vec2(width as f32 - 48.0, height as f32 - 48.0);
    let scaled_pan = scale_pan_for_snapshot(pan, viewport_size, target_size);
    let pixels = render_snapshot(
        width,
        height,
        model,
        representation,
        color_scheme,
        yaw,
        pitch,
        zoom,
        scaled_pan,
        show_hetero,
        show_hydrogen,
    );
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("PNG header: {error}"))?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| format!("PNG image data: {error}"))?;
    }
    Ok(encoded)
}

fn render_model(
    painter: &egui::Painter,
    model: &StructureModel,
    color_scheme: ColorScheme,
    geometry: &RenderGeometry,
) {
    let representation = geometry.key.representation;
    for &(left, right) in &geometry.edges {
        let middle = left.position.lerp(right.position, 0.5);
        let width = if representation == Representation::Backbone {
            2.3
        } else {
            1.5
        };
        painter.line_segment(
            [left.position, middle],
            egui::Stroke::new(width, atom_color(&model.atoms[left.index], color_scheme)),
        );
        painter.line_segment(
            [middle, right.position],
            egui::Stroke::new(width, atom_color(&model.atoms[right.index], color_scheme)),
        );
    }

    for atom in &geometry.atoms {
        let source = &model.atoms[atom.index];
        let radius = atom_radius(source, representation, atom.perspective, geometry.scale);
        let color = atom_color(source, color_scheme);
        painter.circle_filled(atom.position, radius, color);
        if representation != Representation::Backbone && radius >= 2.8 {
            painter.circle_stroke(
                atom.position,
                radius,
                egui::Stroke::new(0.7, color.gamma_multiply(0.55)),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_render_geometry(
    key: RenderGeometryKey,
    model: &StructureModel,
    atom_limit: usize,
    edge_limit: usize,
) -> RenderGeometry {
    let (projected, scale, total_atom_count) = project_atoms(
        key.rect,
        model,
        key.representation,
        key.yaw,
        key.pitch,
        key.zoom,
        key.pan,
        key.show_hetero,
        key.show_hydrogen,
        None,
    );
    let atom_stride = lod_stride(total_atom_count, atom_limit);
    let mut atoms = projected
        .iter()
        .step_by(atom_stride)
        .copied()
        .collect::<Vec<_>>();
    let mut by_index = vec![None; model.atoms.len()];
    for atom in &projected {
        by_index[atom.index] = Some(*atom);
    }

    let source_edges =
        if key.representation == Representation::Backbone && !model.backbone.is_empty() {
            &model.backbone
        } else {
            &model.bonds
        };
    let total_edge_count = source_edges
        .iter()
        .filter(|&&(left, right)| by_index[left].is_some() && by_index[right].is_some())
        .count();
    let edge_stride = lod_stride(total_edge_count, edge_limit);
    let mut edge_ordinal = 0_usize;
    let mut edges = Vec::with_capacity(total_edge_count.min(edge_limit));
    for &(left, right) in source_edges {
        let (Some(left), Some(right)) = (by_index[left], by_index[right]) else {
            continue;
        };
        if edge_ordinal.is_multiple_of(edge_stride) {
            edges.push((left, right));
        }
        edge_ordinal += 1;
    }
    edges.sort_by(|left, right| {
        (right.0.depth + right.1.depth).total_cmp(&(left.0.depth + left.1.depth))
    });
    atoms.sort_by(|left, right| right.depth.total_cmp(&left.depth));

    RenderGeometry {
        key,
        atoms,
        edges,
        scale,
        total_atom_count,
        total_edge_count,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_atoms(
    rect: egui::Rect,
    model: &StructureModel,
    representation: Representation,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    show_hetero: bool,
    show_hydrogen: bool,
    atom_limit: Option<usize>,
) -> (Vec<ProjectedAtom>, f32, usize) {
    let scale = 0.42 * rect.width().min(rect.height()) / model.radius.max(0.1) * zoom;
    let center = rect.center() + pan;
    let camera_distance = model.radius.max(1.0) * 4.0;
    let sin_yaw = yaw.sin();
    let cos_yaw = yaw.cos();
    let sin_pitch = pitch.sin();
    let cos_pitch = pitch.cos();
    let total_atom_count = model
        .atoms
        .iter()
        .filter(|atom| {
            atom_visible_for_representation(atom, representation, show_hetero, show_hydrogen)
        })
        .count();
    let atom_limit = atom_limit.unwrap_or(total_atom_count.max(1));
    let atom_stride = lod_stride(total_atom_count, atom_limit);
    let mut projected = Vec::with_capacity(total_atom_count.min(atom_limit));
    let mut atom_ordinal = 0_usize;

    for (index, atom) in model.atoms.iter().enumerate() {
        if !atom_visible_for_representation(atom, representation, show_hetero, show_hydrogen) {
            continue;
        }
        let selected = atom_ordinal.is_multiple_of(atom_stride);
        atom_ordinal += 1;
        if !selected {
            continue;
        }
        let x = atom.position[0] - model.center[0];
        let y = atom.position[1] - model.center[1];
        let z = atom.position[2] - model.center[2];
        let rotated_x = cos_yaw * x + sin_yaw * z;
        let yaw_z = -sin_yaw * x + cos_yaw * z;
        let rotated_y = cos_pitch * y - sin_pitch * yaw_z;
        let rotated_z = sin_pitch * y + cos_pitch * yaw_z;
        let perspective = (camera_distance / (camera_distance + rotated_z)).clamp(0.55, 1.8);
        projected.push(ProjectedAtom {
            index,
            position: egui::pos2(
                center.x + rotated_x * scale * perspective,
                center.y - rotated_y * scale * perspective,
            ),
            depth: rotated_z,
            perspective,
        });
    }
    (projected, scale, total_atom_count)
}

fn lod_stride(item_count: usize, limit: usize) -> usize {
    item_count.div_ceil(limit.max(1)).max(1)
}

fn atom_visible_for_representation(
    atom: &Atom,
    representation: Representation,
    show_hetero: bool,
    show_hydrogen: bool,
) -> bool {
    atom_visible(atom, show_hetero, show_hydrogen)
        && (representation != Representation::Backbone || matches!(atom.name.as_str(), "CA" | "P"))
}

fn atom_radius(atom: &Atom, representation: Representation, perspective: f32, scale: f32) -> f32 {
    match representation {
        Representation::Backbone => 2.4 * perspective.sqrt(),
        Representation::BallAndStick => (3.2 * perspective.sqrt()).clamp(1.6, 7.0),
        Representation::SpaceFilling => {
            (van_der_waals_radius(&atom.element) * scale * 0.34 * perspective).clamp(2.0, 34.0)
        }
    }
}

fn scale_pan_for_snapshot(
    pan: egui::Vec2,
    source_size: egui::Vec2,
    target_size: egui::Vec2,
) -> egui::Vec2 {
    if source_size.x.is_finite()
        && source_size.y.is_finite()
        && target_size.x.is_finite()
        && target_size.y.is_finite()
        && source_size.x > 0.0
        && source_size.y > 0.0
        && target_size.x > 0.0
        && target_size.y > 0.0
    {
        egui::vec2(
            pan.x * target_size.x / source_size.x,
            pan.y * target_size.y / source_size.y,
        )
    } else {
        pan
    }
}

#[allow(clippy::too_many_arguments)]
fn render_snapshot(
    width: u32,
    height: u32,
    model: &StructureModel,
    representation: Representation,
    color_scheme: ColorScheme,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    show_hetero: bool,
    show_hydrogen: bool,
) -> Vec<u8> {
    let mut canvas = PixelCanvas::new(width, height, egui::Color32::from_rgb(245, 248, 247));
    let inset = 24.0;
    let rect = egui::Rect::from_min_max(
        egui::pos2(inset, inset),
        egui::pos2(width as f32 - inset, height as f32 - inset),
    );
    let (mut projected, scale, _) = project_atoms(
        rect,
        model,
        representation,
        yaw,
        pitch,
        zoom,
        pan,
        show_hetero,
        show_hydrogen,
        None,
    );
    let mut by_index = vec![None; model.atoms.len()];
    for atom in &projected {
        by_index[atom.index] = Some(*atom);
    }

    let edges = if representation == Representation::Backbone && !model.backbone.is_empty() {
        &model.backbone
    } else {
        &model.bonds
    };
    let mut visible_edges = edges
        .iter()
        .filter_map(|&(left, right)| Some((by_index[left]?, by_index[right]?)))
        .collect::<Vec<_>>();
    visible_edges.sort_by(|left, right| {
        (right.0.depth + right.1.depth).total_cmp(&(left.0.depth + left.1.depth))
    });
    let line_width = if representation == Representation::Backbone {
        3
    } else {
        2
    };
    for (left, right) in visible_edges {
        let middle = left.position.lerp(right.position, 0.5);
        canvas.draw_line(
            left.position,
            middle,
            line_width,
            atom_color(&model.atoms[left.index], color_scheme),
        );
        canvas.draw_line(
            middle,
            right.position,
            line_width,
            atom_color(&model.atoms[right.index], color_scheme),
        );
    }

    projected.sort_by(|left, right| right.depth.total_cmp(&left.depth));
    for atom in projected {
        let source = &model.atoms[atom.index];
        let radius = atom_radius(source, representation, atom.perspective, scale)
            .round()
            .max(2.0) as i32;
        let color = atom_color(source, color_scheme);
        if representation != Representation::Backbone && radius >= 3 {
            canvas.draw_disc(atom.position, radius, color.gamma_multiply(0.55));
            canvas.draw_disc(atom.position, radius - 1, color);
        } else {
            canvas.draw_disc(atom.position, radius, color);
        }
    }
    canvas.pixels
}

struct PixelCanvas {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

impl PixelCanvas {
    fn new(width: u32, height: u32, background: egui::Color32) -> Self {
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[background.r(), background.g(), background.b(), 255]);
        }
        Self {
            width: width as i32,
            height: height as i32,
            pixels,
        }
    }

    fn draw_line(&mut self, start: egui::Pos2, end: egui::Pos2, width: i32, color: egui::Color32) {
        let mut x0 = start.x.round() as i32;
        let mut y0 = start.y.round() as i32;
        let x1 = end.x.round() as i32;
        let y1 = end.y.round() as i32;
        let dx = (x1 - x0).abs();
        let step_x = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let step_y = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.draw_disc(egui::pos2(x0 as f32, y0 as f32), width / 2, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let doubled = 2 * error;
            if doubled >= dy {
                error += dy;
                x0 += step_x;
            }
            if doubled <= dx {
                error += dx;
                y0 += step_y;
            }
        }
    }

    fn draw_disc(&mut self, center: egui::Pos2, radius: i32, color: egui::Color32) {
        let center_x = center.x.round() as i32;
        let center_y = center.y.round() as i32;
        let radius = radius.max(0);
        let squared_radius = radius * radius;
        let minimum_y = (center_y - radius).max(0);
        let maximum_y = (center_y + radius).min(self.height - 1);
        let minimum_x = (center_x - radius).max(0);
        let maximum_x = (center_x + radius).min(self.width - 1);
        for y in minimum_y..=maximum_y {
            for x in minimum_x..=maximum_x {
                let dx = x - center_x;
                let dy = y - center_y;
                if dx * dx + dy * dy <= squared_radius {
                    let offset = ((y * self.width + x) * 4) as usize;
                    self.pixels[offset..offset + 4].copy_from_slice(&[
                        color.r(),
                        color.g(),
                        color.b(),
                        255,
                    ]);
                }
            }
        }
    }
}

fn atom_visible(atom: &Atom, show_hetero: bool, show_hydrogen: bool) -> bool {
    (show_hetero || !atom.hetero) && (show_hydrogen || atom.element != "H")
}

fn show_legend(ui: &mut egui::Ui, scheme: ColorScheme, zh_cn: bool) {
    ui.horizontal_wrapped(|ui| match scheme {
        ColorScheme::Confidence => {
            ui.small(localized(zh_cn, "pLDDT：", "pLDDT:"));
            for (color, label) in [
                (egui::Color32::from_rgb(0, 83, 214), ">= 90"),
                (egui::Color32::from_rgb(44, 167, 224), "70-<90"),
                (egui::Color32::from_rgb(247, 211, 69), "50-<70"),
                (egui::Color32::from_rgb(239, 111, 48), "< 50"),
            ] {
                ui.colored_label(color, format!("● {label}"));
            }
        }
        ColorScheme::Element => {
            ui.small(localized(zh_cn, "元素：", "Elements:"));
            for (element, color) in [
                ("C", element_color("C")),
                ("N", element_color("N")),
                ("O", element_color("O")),
                ("S", element_color("S")),
                ("P", element_color("P")),
            ] {
                ui.colored_label(color, format!("● {element}"));
            }
        }
        ColorScheme::Chain => {
            ui.small(localized(
                zh_cn,
                "每条链使用稳定的离散颜色",
                "Each chain uses a stable discrete color",
            ));
        }
    });
}

fn load_structure(path: &Path) -> Result<StructureModel, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let mut prefix = [0_u8; 2];
    let prefix_length = fs::File::open(path)
        .and_then(|mut file| file.read(&mut prefix))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let gzip = prefix_length == prefix.len() && prefix == [0x1f, 0x8b];
    if gzip && metadata.len() > MAX_COMPRESSED_STRUCTURE_BYTES {
        return Err(format!(
            "compressed file is larger than the {} MiB viewer limit",
            MAX_COMPRESSED_STRUCTURE_BYTES / 1024 / 1024
        ));
    }
    if !gzip && metadata.len() > MAX_STRUCTURE_BYTES {
        return Err(format!(
            "file is larger than the {} MiB viewer limit",
            MAX_STRUCTURE_BYTES / 1024 / 1024
        ));
    }
    let file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let source: Box<dyn Read> = if gzip {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    source
        .take(MAX_STRUCTURE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_STRUCTURE_BYTES {
        return Err(format!(
            "decompressed structure is larger than the {} MiB viewer limit",
            MAX_STRUCTURE_BYTES / 1024 / 1024
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| "structure is not UTF-8 text".to_owned())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let uncompressed_name = name
        .strip_suffix(".gz")
        .or_else(|| name.strip_suffix(".bgz"))
        .unwrap_or(&name);
    let atoms = if (uncompressed_name.ends_with(".cif") || uncompressed_name.ends_with(".mmcif"))
        || text.trim_start().starts_with("data_")
    {
        parse_mmcif(&text)?
    } else {
        parse_pdb(&text)?
    };
    build_model(atoms)
}

fn parse_pdb(text: &str) -> Result<Vec<Atom>, String> {
    let mut atoms = Vec::new();
    let mut saw_model = false;
    for line in text.lines() {
        let record = field(line, 0, 6);
        if record == "MODEL" {
            if saw_model {
                break;
            }
            saw_model = true;
            continue;
        }
        if record == "ENDMDL" && saw_model {
            break;
        }
        let hetero = match record {
            "ATOM" => false,
            "HETATM" => true,
            _ => continue,
        };
        let alternate = field(line, 16, 17);
        if !matches!(alternate, "" | "A" | "." | "?") {
            continue;
        }
        let x = parse_f32(field(line, 30, 38), "PDB x coordinate")?;
        let y = parse_f32(field(line, 38, 46), "PDB y coordinate")?;
        let z = parse_f32(field(line, 46, 54), "PDB z coordinate")?;
        let name = field(line, 12, 16).to_owned();
        let element_field = field(line, 76, 78);
        let element = normalize_element(if element_field.is_empty() {
            infer_element(&name)
        } else {
            element_field
        });
        let chain = field(line, 21, 22).to_owned();
        let residue_number = field(line, 22, 27);
        atoms.push(Atom {
            name,
            residue_name: field(line, 17, 20).to_owned(),
            residue_key: residue_number.to_owned(),
            chain,
            element,
            position: [x, y, z],
            b_factor: parse_optional_f32(field(line, 60, 66)).unwrap_or(0.0),
            hetero,
        });
        if atoms.len() > MAX_ATOMS {
            return Err(format!(
                "structure exceeds the {MAX_ATOMS} atom viewer limit"
            ));
        }
    }
    if atoms.is_empty() {
        Err("no ATOM or HETATM coordinates were found".to_owned())
    } else {
        Ok(atoms)
    }
}

fn parse_mmcif(text: &str) -> Result<Vec<Atom>, String> {
    let mut tokens = CifTokenCursor::new(text);
    while let Some(token) = tokens.next_token()? {
        if token != "loop_" {
            continue;
        }
        let mut headers = Vec::new();
        let first_value = loop {
            match tokens.next_token()? {
                Some(header) if header.starts_with('_') => headers.push(header),
                value => break value,
            }
        };
        if headers
            .iter()
            .any(|header| header.starts_with("_atom_site."))
        {
            return parse_atom_site_loop(&mut tokens, &headers, first_value);
        }
        skip_loop_values(&mut tokens, first_value)?;
    }
    Err("mmCIF does not contain an _atom_site coordinate loop".to_owned())
}

fn parse_atom_site_loop<'a>(
    tokens: &mut CifTokenCursor<'a>,
    headers: &[&str],
    mut first_value: Option<&'a str>,
) -> Result<Vec<Atom>, String> {
    let x = required_column(headers, "_atom_site.Cartn_x")?;
    let y = required_column(headers, "_atom_site.Cartn_y")?;
    let z = required_column(headers, "_atom_site.Cartn_z")?;
    let group = column(headers, &["_atom_site.group_PDB"]);
    let name = column(
        headers,
        &["_atom_site.auth_atom_id", "_atom_site.label_atom_id"],
    );
    let residue = column(
        headers,
        &["_atom_site.auth_comp_id", "_atom_site.label_comp_id"],
    );
    let chain = column(
        headers,
        &["_atom_site.auth_asym_id", "_atom_site.label_asym_id"],
    );
    let residue_sequence = column(
        headers,
        &["_atom_site.auth_seq_id", "_atom_site.label_seq_id"],
    );
    let insertion_code = column(headers, &["_atom_site.pdbx_PDB_ins_code"]);
    let element = column(headers, &["_atom_site.type_symbol"]);
    let b_factor = column(headers, &["_atom_site.B_iso_or_equiv"]);
    let alternate = column(
        headers,
        &["_atom_site.label_alt_id", "_atom_site.auth_alt_id"],
    );
    let model_number = column(headers, &["_atom_site.pdbx_PDB_model_num"]);
    let width = headers.len();
    let mut atoms = Vec::new();
    let mut first_model = None::<&str>;

    loop {
        let next = match first_value.take() {
            Some(value) => Some(value),
            None => tokens.next_token()?,
        };
        let Some(first) = next else {
            break;
        };
        if is_loop_boundary(first) {
            tokens.put_back(first)?;
            break;
        }
        let mut row = Vec::with_capacity(width);
        row.push(first);
        for _ in 1..width {
            let Some(value) = tokens.next_token()? else {
                return Err(format!(
                    "incomplete mmCIF _atom_site row: found {} of {width} values",
                    row.len()
                ));
            };
            row.push(value);
        }
        if let Some(index) = model_number {
            let model = row[index];
            if *first_model.get_or_insert(model) != model {
                continue;
            }
        }
        if alternate
            .map(|index| row[index])
            .is_some_and(|value| !matches!(value, "." | "?" | "A" | "1"))
        {
            continue;
        }
        let atom_name = value_at(&row, name).unwrap_or("?").to_owned();
        let element = normalize_element(
            value_at(&row, element)
                .filter(|value| !matches!(*value, "." | "?"))
                .unwrap_or_else(|| infer_element(&atom_name)),
        );
        let sequence = value_at(&row, residue_sequence).unwrap_or("?");
        let insertion = value_at(&row, insertion_code).filter(|value| !matches!(*value, "." | "?"));
        let residue_key = insertion
            .map(|insertion| format!("{sequence}{insertion}"))
            .unwrap_or_else(|| sequence.to_owned());
        atoms.push(Atom {
            name: atom_name,
            residue_name: value_at(&row, residue).unwrap_or("UNK").to_owned(),
            residue_key,
            chain: value_at(&row, chain).unwrap_or("?").to_owned(),
            element,
            position: [
                parse_f32(row[x], "mmCIF x coordinate")?,
                parse_f32(row[y], "mmCIF y coordinate")?,
                parse_f32(row[z], "mmCIF z coordinate")?,
            ],
            b_factor: value_at(&row, b_factor)
                .and_then(parse_optional_f32)
                .unwrap_or(0.0),
            hetero: value_at(&row, group).is_some_and(|value| value == "HETATM"),
        });
        if atoms.len() > MAX_ATOMS {
            return Err(format!(
                "structure exceeds the {MAX_ATOMS} atom viewer limit"
            ));
        }
    }
    if atoms.is_empty() {
        Err("mmCIF _atom_site loop did not contain coordinates".to_owned())
    } else {
        Ok(atoms)
    }
}

fn build_model(atoms: Vec<Atom>) -> Result<StructureModel, String> {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut chains = BTreeSet::new();
    let mut residues = BTreeSet::new();
    let mut confidence_values = 0_usize;
    for atom in &atoms {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(atom.position[axis]);
            maximum[axis] = maximum[axis].max(atom.position[axis]);
        }
        chains.insert(atom.chain.clone());
        residues.insert((
            atom.chain.clone(),
            atom.residue_key.clone(),
            atom.residue_name.clone(),
        ));
        if (0.0..=100.0).contains(&atom.b_factor) && atom.b_factor > 0.0 {
            confidence_values += 1;
        }
    }
    let center = [
        (minimum[0] + maximum[0]) * 0.5,
        (minimum[1] + maximum[1]) * 0.5,
        (minimum[2] + maximum[2]) * 0.5,
    ];
    let radius = atoms
        .iter()
        .map(|atom| squared_distance(atom.position, center).sqrt())
        .fold(0.0_f32, f32::max)
        .max(1.0);
    let bonds = infer_bonds(&atoms);
    let backbone = infer_backbone(&atoms);
    Ok(StructureModel {
        confidence_available: confidence_values * 2 >= atoms.len(),
        atoms,
        bonds,
        backbone,
        center,
        radius,
        chain_count: chains.len(),
        residue_count: residues.len(),
    })
}

fn infer_bonds(atoms: &[Atom]) -> Vec<(usize, usize)> {
    const CELL: f32 = 2.5;
    let mut cells = HashMap::<(i32, i32, i32), Vec<usize>>::new();
    let mut bonds = Vec::new();
    for (index, atom) in atoms.iter().enumerate() {
        let cell = (
            (atom.position[0] / CELL).floor() as i32,
            (atom.position[1] / CELL).floor() as i32,
            (atom.position[2] / CELL).floor() as i32,
        );
        'neighbors: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(candidates) = cells.get(&(cell.0 + dx, cell.1 + dy, cell.2 + dz)) {
                        for &other in candidates {
                            let limit = covalent_radius(&atom.element)
                                + covalent_radius(&atoms[other].element)
                                + 0.45;
                            let distance_squared =
                                squared_distance(atom.position, atoms[other].position);
                            if distance_squared > 0.16 && distance_squared <= limit * limit {
                                bonds.push((other, index));
                                if bonds.len() >= MAX_BONDS {
                                    break 'neighbors;
                                }
                            }
                        }
                    }
                }
            }
        }
        cells.entry(cell).or_default().push(index);
        if bonds.len() >= MAX_BONDS {
            break;
        }
    }
    bonds
}

fn infer_backbone(atoms: &[Atom]) -> Vec<(usize, usize)> {
    let mut representatives = Vec::<usize>::new();
    let mut last_residue = None::<(String, String)>;
    for (index, atom) in atoms.iter().enumerate() {
        if atom.hetero || !matches!(atom.name.as_str(), "CA" | "P") {
            continue;
        }
        let residue = (atom.chain.clone(), atom.residue_key.clone());
        if last_residue.as_ref() != Some(&residue) {
            representatives.push(index);
            last_residue = Some(residue);
        }
    }
    representatives
        .windows(2)
        .filter_map(|pair| {
            let left = &atoms[pair[0]];
            let right = &atoms[pair[1]];
            (left.chain == right.chain
                && squared_distance(left.position, right.position) <= 8.5_f32.powi(2))
            .then_some((pair[0], pair[1]))
        })
        .collect()
}

struct CifTokenCursor<'a> {
    text: &'a str,
    position: usize,
    pending: Option<&'a str>,
}

impl<'a> CifTokenCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            position: 0,
            pending: None,
        }
    }

    fn next_token(&mut self) -> Result<Option<&'a str>, String> {
        if let Some(token) = self.pending.take() {
            return Ok(Some(token));
        }
        let bytes = self.text.as_bytes();
        loop {
            while self.position < bytes.len() && bytes[self.position].is_ascii_whitespace() {
                self.position += 1;
            }
            if self.position >= bytes.len() {
                return Ok(None);
            }
            if bytes[self.position] != b'#' {
                break;
            }
            while self.position < bytes.len() && bytes[self.position] != b'\n' {
                self.position += 1;
            }
        }

        if bytes[self.position] == b';' && (self.position == 0 || bytes[self.position - 1] == b'\n')
        {
            let start = self.position + 1;
            let mut delimiter = start;
            while delimiter < bytes.len()
                && !(bytes[delimiter] == b';' && bytes[delimiter - 1] == b'\n')
            {
                delimiter += 1;
            }
            if delimiter >= bytes.len() {
                return Err("unterminated semicolon-delimited mmCIF value".to_owned());
            }
            self.position = delimiter + 1;
            while self.position < bytes.len() && bytes[self.position] != b'\n' {
                self.position += 1;
            }
            return Ok(Some(
                self.text[start..delimiter].trim_end_matches(['\r', '\n']),
            ));
        }

        if matches!(bytes[self.position], b'\'' | b'"') {
            let quote = bytes[self.position];
            self.position += 1;
            let start = self.position;
            while self.position < bytes.len() && bytes[self.position] != quote {
                self.position += 1;
            }
            if self.position >= bytes.len() {
                return Err("unterminated quoted mmCIF value".to_owned());
            }
            let token = &self.text[start..self.position];
            self.position += 1;
            return Ok(Some(token));
        }

        let start = self.position;
        while self.position < bytes.len()
            && !bytes[self.position].is_ascii_whitespace()
            && bytes[self.position] != b'#'
        {
            self.position += 1;
        }
        Ok(Some(&self.text[start..self.position]))
    }

    fn put_back(&mut self, token: &'a str) -> Result<(), String> {
        if self.pending.replace(token).is_some() {
            Err("internal mmCIF tokenizer pushback overflow".to_owned())
        } else {
            Ok(())
        }
    }
}

fn skip_loop_values<'a>(
    tokens: &mut CifTokenCursor<'a>,
    mut value: Option<&'a str>,
) -> Result<(), String> {
    loop {
        let next = match value.take() {
            Some(value) => Some(value),
            None => tokens.next_token()?,
        };
        let Some(token) = next else {
            return Ok(());
        };
        if is_loop_boundary(token) {
            tokens.put_back(token)?;
            return Ok(());
        }
    }
}

fn is_loop_boundary(token: &str) -> bool {
    token == "loop_"
        || token == "stop_"
        || token.starts_with('_')
        || token.starts_with("data_")
        || token.starts_with("save_")
}

fn required_column(headers: &[&str], name: &str) -> Result<usize, String> {
    headers
        .iter()
        .position(|header| *header == name)
        .ok_or_else(|| format!("mmCIF atom loop is missing {name}"))
}

fn column(headers: &[&str], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|name| headers.iter().position(|header| *header == *name))
}

fn value_at<'a>(row: &[&'a str], index: Option<usize>) -> Option<&'a str> {
    index.and_then(|index| row.get(index)).copied()
}

fn field(line: &str, start: usize, end: usize) -> &str {
    line.as_bytes()
        .get(start..end.min(line.len()))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or("")
        .trim()
}

fn parse_f32(value: &str, context: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| format!("invalid {context}: {value}"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(format!("non-finite {context}: {value}"))
    }
}

fn parse_optional_f32(value: &str) -> Option<f32> {
    value.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn infer_element(atom_name: &str) -> &str {
    let name = atom_name.trim_start_matches(|character: char| character.is_ascii_digit());
    match name.as_bytes().first().copied() {
        Some(b'H' | b'h') => "H",
        Some(b'C' | b'c') => "C",
        Some(b'N' | b'n') => "N",
        Some(b'O' | b'o') => "O",
        Some(b'S' | b's') => "S",
        Some(b'P' | b'p') => "P",
        _ => "X",
    }
}

fn normalize_element(element: &str) -> String {
    element
        .trim()
        .chars()
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

fn squared_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2)
}

fn covalent_radius(element: &str) -> f32 {
    match element {
        "H" => 0.31,
        "C" => 0.76,
        "N" => 0.71,
        "O" => 0.66,
        "F" => 0.57,
        "P" => 1.07,
        "S" => 1.05,
        "CL" => 1.02,
        "FE" => 1.24,
        "ZN" => 1.22,
        _ => 0.9,
    }
}

fn van_der_waals_radius(element: &str) -> f32 {
    match element {
        "H" => 1.2,
        "C" => 1.7,
        "N" => 1.55,
        "O" => 1.52,
        "F" => 1.47,
        "P" => 1.8,
        "S" => 1.8,
        "CL" => 1.75,
        "FE" => 1.8,
        "ZN" => 1.39,
        _ => 1.7,
    }
}

fn atom_color(atom: &Atom, scheme: ColorScheme) -> egui::Color32 {
    match scheme {
        ColorScheme::Element => element_color(&atom.element),
        ColorScheme::Confidence => confidence_color(atom.b_factor),
        ColorScheme::Chain => chain_color(&atom.chain),
    }
}

fn element_color(element: &str) -> egui::Color32 {
    match element {
        "H" => egui::Color32::from_rgb(224, 227, 226),
        "C" => egui::Color32::from_rgb(84, 96, 93),
        "N" => egui::Color32::from_rgb(45, 92, 194),
        "O" => egui::Color32::from_rgb(205, 56, 59),
        "S" => egui::Color32::from_rgb(221, 170, 38),
        "P" => egui::Color32::from_rgb(214, 104, 36),
        "F" | "CL" => egui::Color32::from_rgb(55, 158, 91),
        "FE" => egui::Color32::from_rgb(172, 91, 49),
        "ZN" => egui::Color32::from_rgb(101, 112, 160),
        _ => egui::Color32::from_rgb(137, 109, 151),
    }
}

fn confidence_color(confidence: f32) -> egui::Color32 {
    if confidence >= 90.0 {
        egui::Color32::from_rgb(0, 83, 214)
    } else if confidence >= 70.0 {
        egui::Color32::from_rgb(44, 167, 224)
    } else if confidence >= 50.0 {
        egui::Color32::from_rgb(247, 211, 69)
    } else {
        egui::Color32::from_rgb(239, 111, 48)
    }
}

fn chain_color(chain: &str) -> egui::Color32 {
    const COLORS: [egui::Color32; 8] = [
        egui::Color32::from_rgb(35, 127, 105),
        egui::Color32::from_rgb(50, 105, 166),
        egui::Color32::from_rgb(190, 118, 32),
        egui::Color32::from_rgb(177, 66, 77),
        egui::Color32::from_rgb(116, 85, 156),
        egui::Color32::from_rgb(39, 143, 159),
        egui::Color32::from_rgb(116, 132, 55),
        egui::Color32::from_rgb(173, 87, 137),
    ];
    let hash = chain.bytes().fold(0_usize, |value, byte| {
        value.wrapping_mul(31).wrapping_add(usize::from(byte))
    });
    COLORS[hash % COLORS.len()]
}

fn localized<'a>(zh_cn: bool, zh: &'a str, en: &'a str) -> &'a str {
    if zh_cn { zh } else { en }
}

#[cfg(test)]
mod tests {
    use super::{
        RenderGeometryKey, Representation, StructureViewer, build_model, build_render_geometry,
        confidence_color, load_structure, lod_stride, parse_f32, parse_mmcif, parse_pdb,
        scale_pan_for_snapshot,
    };
    use eframe::egui;
    use flate2::{Compression, write::GzEncoder};
    use std::fs;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_first_pdb_model_and_confidence() {
        let pdb = concat!(
            "MODEL        1\n",
            "ATOM      1  CA  ALA A   1      11.104  13.207   8.100  1.00 95.50           C  \n",
            "ATOM      2  N   ALA A   1      10.000  13.000   8.000  1.00 88.00           N  \n",
            "ENDMDL\n",
            "MODEL        2\n",
            "ATOM      3  CA  ALA A   1       0.000   0.000   0.000  1.00 10.00           C  \n",
            "ENDMDL\n"
        );
        let atoms = parse_pdb(pdb).expect("valid PDB");
        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0].name, "CA");
        assert_eq!(atoms[0].b_factor, 95.5);
    }

    #[test]
    fn parses_alphafold_style_mmcif_atom_loop() {
        let cif = r#"data_model
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.B_iso_or_equiv
_atom_site.pdbx_PDB_model_num
ATOM 1 C CA ALA A 1 1.0 2.0 3.0 92.0 1
ATOM 2 N N ALA A 1 2.0 2.0 3.0 88.0 1
#
"#;
        let atoms = parse_mmcif(cif).expect("valid mmCIF");
        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0].chain, "A");
        assert_eq!(atoms[0].position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn streams_past_non_atom_mmcif_loops_and_keeps_insertion_codes() {
        let cif = r#"data_model
loop_
_audit.id
_audit.note
1
;multiline
metadata
;
loop_
_atom_site.group_PDB
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM C CA ALA '_A' 10 A 1.0 2.0 3.0
ATOM C CA GLY '_A' 10 B 2.0 2.0 3.0
#
"#;

        let atoms = parse_mmcif(cif).expect("valid mmCIF");

        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0].chain, "_A");
        assert_eq!(atoms[0].residue_key, "10A");
        assert_eq!(atoms[1].residue_key, "10B");
        assert_eq!(build_model(atoms).expect("model").residue_count, 2);
    }

    #[test]
    fn rejects_non_finite_structure_coordinates() {
        assert!(parse_f32("NaN", "test coordinate").is_err());
        assert!(parse_f32("inf", "test coordinate").is_err());
        let cif = r#"data_model
loop_
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
NaN 1.0 2.0
#
"#;
        assert!(parse_mmcif(cif).is_err());
    }

    #[test]
    fn loads_gzip_compressed_pdb_by_content() {
        let path = temporary_path("pdb.gz");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(
                b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 92.00           C  \n",
            )
            .expect("compress PDB");
        fs::write(&path, encoder.finish().expect("finish gzip")).expect("write gzip PDB");

        let model = load_structure(&path).expect("load gzip PDB");

        assert_eq!(model.atoms.len(), 1);
        fs::remove_file(path).expect("remove gzip PDB");
    }

    #[test]
    fn structure_loading_completes_through_the_background_channel() {
        let path = temporary_path("pdb");
        fs::write(
            &path,
            b"ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 92.00           C  \n",
        )
        .expect("write PDB");
        let mut viewer = StructureViewer::default();

        viewer.load_path(&path, false);

        assert!(viewer.is_loading());
        for _ in 0..200 {
            viewer.poll_load(false);
            if !viewer.is_loading() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!viewer.is_loading());
        assert_eq!(viewer.model.as_ref().expect("loaded model").atoms.len(), 1);
        fs::remove_file(path).expect("remove PDB");
    }

    #[test]
    fn interactive_geometry_limits_and_sorts_work_once_per_view_key() {
        let pdb = concat!(
            "ATOM      1  C   ALA A   1       0.000   0.000   0.000  1.00 92.00           C  \n",
            "ATOM      2  C   ALA A   1       1.400   0.000   0.000  1.00 92.00           C  \n",
            "ATOM      3  C   ALA A   1       2.800   0.000   0.000  1.00 92.00           C  \n",
            "ATOM      4  C   ALA A   1       4.200   0.000   0.000  1.00 92.00           C  \n",
        );
        let model = build_model(parse_pdb(pdb).expect("PDB")).expect("model");
        let key = RenderGeometryKey {
            rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            representation: Representation::BallAndStick,
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            show_hetero: true,
            show_hydrogen: false,
        };

        let geometry = build_render_geometry(key, &model, 2, 1);

        assert_eq!(geometry.total_atom_count, 4);
        assert_eq!(geometry.atoms.len(), 2);
        assert!(geometry.total_edge_count > 1);
        assert_eq!(geometry.edges.len(), 1);
        assert!(
            geometry
                .atoms
                .windows(2)
                .all(|pair| pair[0].depth >= pair[1].depth)
        );
        assert_eq!(geometry.key, key);
        assert_eq!(lod_stride(10, 3), 4);
    }

    #[test]
    fn confidence_thresholds_match_the_summary_capability() {
        assert_eq!(confidence_color(90.0), egui::Color32::from_rgb(0, 83, 214));
        assert_eq!(
            confidence_color(70.0),
            egui::Color32::from_rgb(44, 167, 224)
        );
        assert_eq!(
            confidence_color(50.0),
            egui::Color32::from_rgb(247, 211, 69)
        );
    }

    #[test]
    fn snapshot_pan_scales_with_the_export_canvas() {
        assert_eq!(
            scale_pan_for_snapshot(
                egui::vec2(25.0, -10.0),
                egui::vec2(500.0, 250.0),
                egui::vec2(1_000.0, 500.0)
            ),
            egui::vec2(50.0, -20.0)
        );
    }

    #[test]
    fn exports_a_png_from_the_current_structure_view() {
        let pdb = concat!(
            "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 92.00           C  \n",
            "ATOM      2  CA  GLY A   2       3.800   0.000   0.000  1.00 75.00           C  \n",
            "ATOM      3  O   GLY A   2       4.500   1.000   0.000  1.00 75.00           O  \n",
        );
        let viewer = StructureViewer {
            model: Some(Arc::new(
                build_model(parse_pdb(pdb).expect("PDB")).expect("model"),
            )),
            ..StructureViewer::default()
        };

        let png = viewer.snapshot_png(320, 200).expect("PNG snapshot");

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_be_bytes(png[16..20].try_into().expect("width")),
            320
        );
        assert_eq!(
            u32::from_be_bytes(png[20..24].try_into().expect("height")),
            200
        );
        assert!(png.len() > 500);
    }

    #[test]
    fn refuses_to_export_without_an_open_structure() {
        let error = StructureViewer::default()
            .snapshot_png(320, 200)
            .expect_err("missing structure must fail");
        assert!(error.contains("open a structure"));
    }

    fn temporary_path(suffix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "linxira-structure-viewer-{}-{nonce}.{suffix}",
            std::process::id()
        ))
    }
}
