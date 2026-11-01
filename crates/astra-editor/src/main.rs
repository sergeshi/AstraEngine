mod gpu_viewport;

use astra_core::EngineInfo;
use astra_scene::{EntityId, Scene};
use egui::{Id, Sense, Ui, WidgetText};
use std::sync::{Arc, Mutex};
use gpu_viewport::GpuViewport;
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use serde::{Deserialize, Serialize};

const DOCK_STATE_KEY: &str = "astra_editor_dock_state";
const EDITOR_STATE_KEY: &str = "astra_editor_state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum EditorTab {
    Hierarchy,
    Viewport,
    Inspector,
    ContentBrowser,
    Console,
}

impl EditorTab {
    fn title(self) -> &'static str {
        match self {
            Self::Hierarchy => "Hierarchy",
            Self::Viewport => "Viewport",
            Self::Inspector => "Inspector",
            Self::ContentBrowser => "Content Browser",
            Self::Console => "Console",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EditorState {
    scene: Scene,
    selected_entity: Option<EntityId>,
    console_lines: Vec<String>,
}

impl Default for EditorState {
    fn default() -> Self {
        let mut scene = Scene::new();
        scene.spawn("Camera");
        scene.spawn("Directional Light");
        let cube = scene.spawn("Cube");

        Self {
            scene,
            selected_entity: Some(cube),
            console_lines: vec![
                "AstraEngine Editor 0.4 initialized.".to_owned(),
                "Hierarchy and Inspector use the shared scene model.".to_owned(),
            ],
        }
    }
}

fn default_dock_state() -> DockState<EditorTab> {
    let mut dock = DockState::new(vec![EditorTab::Viewport]);
    let tree = dock.main_surface_mut();

    let [center, _hierarchy] =
        tree.split_left(NodeIndex::root(), 0.20, vec![EditorTab::Hierarchy]);
    let [center, _inspector] =
        tree.split_right(center, 0.76, vec![EditorTab::Inspector]);
    tree.split_below(
        center,
        0.72,
        vec![EditorTab::ContentBrowser, EditorTab::Console],
    );

    dock
}

struct AstraEditor {
    dock_state: DockState<EditorTab>,
    state: EditorState,
    gpu_viewport: Option<Arc<Mutex<GpuViewport>>>,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
}

impl AstraEditor {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let dock_state = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, DOCK_STATE_KEY))
            .unwrap_or_else(default_dock_state);
        let mut state: EditorState = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, EDITOR_STATE_KEY))
            .unwrap_or_default();

        let gpu_viewport = cc.gl.as_ref().and_then(|gl| match GpuViewport::new(gl) {
            Ok(renderer) => {
                state.console_lines.push("OpenGL viewport renderer initialized.".to_owned());
                Some(Arc::new(Mutex::new(renderer)))
            }
            Err(error) => {
                state.console_lines.push(format!("GPU viewport initialization failed: {error}"));
                None
            }
        });

        Self {
            dock_state,
            state,
            gpu_viewport,
            camera_yaw: 0.75,
            camera_pitch: 0.48,
            camera_distance: 9.0,
        }
    }

    fn log(&mut self, message: impl Into<String>) {
        self.state.console_lines.push(message.into());
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("main_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        self.log("New Scene: scene document workflow is not implemented yet.");
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.log("Save requested: scene serialization is scheduled for a later milestone.");
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    ui.add_enabled(false, egui::Button::new("Undo"));
                    ui.add_enabled(false, egui::Button::new("Redo"));
                });
                ui.menu_button("Window", |ui| {
                    if ui.button("Reset Layout").clicked() {
                        self.dock_state = default_dock_state();
                        self.log("Editor panel layout reset.");
                        ui.close_menu();
                    }
                    ui.separator();
                    for tab in [
                        EditorTab::Hierarchy,
                        EditorTab::Viewport,
                        EditorTab::Inspector,
                        EditorTab::ContentBrowser,
                        EditorTab::Console,
                    ] {
                        if ui.button(format!("Show {}", tab.title())).clicked() {
                            self.dock_state.push_to_focused_leaf(tab);
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.label(format!("{} {}", EngineInfo::CURRENT.name, EngineInfo::CURRENT.version));
                    ui.label("AstraEngine standalone editor prototype");
                });
            });
        });
    }

    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("＋ New").clicked() {
                    self.log("New project command selected (project wizard is not implemented yet).");
                }
                if ui.button("Save").clicked() {
                    self.log("Save requested (scene serialization is not implemented yet).");
                }
                ui.separator();
                if ui.button("▶ Play").clicked() {
                    self.log("Play requested (game launch is not implemented yet).");
                }
                if ui.button("■ Stop").clicked() {
                    self.log("Stop requested.");
                }
                ui.separator();
                ui.label("Scene: Untitled");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Windows · Editor 0.4");
                });
            });
        });
    }
}

impl eframe::App for AstraEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.menu_bar(ctx);
        self.toolbar(ctx);

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.small("Ready");
                    ui.separator();
                    ui.small(format!(
                        "Selected: {}",
                        self.state.selected_entity
                            .and_then(|id| self.state.scene.get(id))
                            .map(|entity| entity.name.as_str())
                            .unwrap_or("None")
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small("AstraEngine");
                    });
                });
            });

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                let mut viewer = EditorTabViewer {
                    state: &mut self.state,
                    gpu_viewport: self.gpu_viewport.clone(),
                    camera_yaw: &mut self.camera_yaw,
                    camera_pitch: &mut self.camera_pitch,
                    camera_distance: &mut self.camera_distance,
                };
                DockArea::new(&mut self.dock_state)
                    .style(Style::from_egui(ui.style()))
                    .show_inside(ui, &mut viewer);
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, DOCK_STATE_KEY, &self.dock_state);
        eframe::set_value(storage, EDITOR_STATE_KEY, &self.state);
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let (Some(renderer), Some(gl)) = (&self.gpu_viewport, gl) {
            if let Ok(renderer) = renderer.lock() {
                renderer.destroy(gl);
            }
        }
    }
}

struct EditorTabViewer<'a> {
    state: &'a mut EditorState,
    gpu_viewport: Option<Arc<Mutex<GpuViewport>>>,
    camera_yaw: &'a mut f32,
    camera_pitch: &'a mut f32,
    camera_distance: &'a mut f32,
}

impl TabViewer for EditorTabViewer<'_> {
    type Tab = EditorTab;

    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        Id::new(*tab as u8)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            EditorTab::Hierarchy => self.hierarchy_ui(ui),
            EditorTab::Viewport => self.viewport_ui(ui),
            EditorTab::Inspector => self.inspector_ui(ui),
            EditorTab::ContentBrowser => self.content_browser_ui(ui),
            EditorTab::Console => self.console_ui(ui),
        }
    }
}

impl EditorTabViewer<'_> {
    fn hierarchy_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("World");
            if ui.button("＋ Add Cube").clicked() {
                let id = self.state.scene.spawn("Cube");
                self.state.selected_entity = Some(id);
                self.state.console_lines.push(format!("Created Cube ({})", id.0));
            }
            if ui.button("− Delete").clicked() {
                if let Some(id) = self.state.selected_entity {
                    if self.state.scene.remove(id).is_some() {
                        self.state.selected_entity = self.state.scene.iter().next().map(|entity| entity.id);
                        self.state.console_lines.push(format!("Deleted entity ({})", id.0));
                    }
                }
            }
        });
        ui.separator();

        let entities: Vec<(EntityId, String)> = self.state.scene
            .iter()
            .map(|entity| (entity.id, entity.name.clone()))
            .collect();
        for (id, name) in entities {
            let selected = self.state.selected_entity == Some(id);
            if ui.selectable_label(selected, format!("◇  {name}")).clicked() {
                self.state.selected_entity = Some(id);
            }
        }
        if self.state.scene.is_empty() {
            ui.weak("Scene is empty. Add an object to begin.");
        }
    }

    fn viewport_ui(&mut self, ui: &mut Ui) {
        let available = ui.available_size().max(egui::Vec2::splat(80.0));
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            *self.camera_yaw -= delta.x * 0.008;
            *self.camera_pitch = (*self.camera_pitch + delta.y * 0.008).clamp(-1.3, 1.3);
            ui.ctx().request_repaint();
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                *self.camera_distance = (*self.camera_distance * (1.0 - scroll * 0.001))
                    .clamp(2.5, 40.0);
                ui.ctx().request_repaint();
            }
        }

        if let Some(renderer) = &self.gpu_viewport {
            let renderer = Arc::clone(renderer);
            let yaw = *self.camera_yaw;
            let pitch = *self.camera_pitch;
            let distance = *self.camera_distance;
            let callback = egui::PaintCallback {
                rect,
                callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                    if let Ok(renderer) = renderer.lock() {
                        let viewport = info.viewport_in_pixels();
                        renderer.paint(
                            painter.gl(),
                            viewport.width_px,
                            viewport.height_px,
                            yaw,
                            pitch,
                            distance,
                        );
                    }
                })),
            };
            ui.painter().add(callback);
        } else {
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(27, 31, 39));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "GPU renderer unavailable",
                egui::FontId::proportional(16.0),
                egui::Color32::LIGHT_RED,
            );
        }

        let painter = ui.painter();
        painter.text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            "PERSPECTIVE · LIT 3D SCENE",
            egui::FontId::monospace(12.0),
            egui::Color32::from_rgb(195, 205, 220),
        );
        painter.text(
            rect.left_bottom() + egui::vec2(14.0, -12.0),
            egui::Align2::LEFT_BOTTOM,
            "LMB drag: orbit   ·   Wheel: zoom",
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(150, 164, 183),
        );
    }

    fn inspector_ui(&mut self, ui: &mut Ui) {
        ui.heading("Details");
        ui.separator();

        let Some(selected) = self.state.selected_entity else {
            ui.weak("Select an entity in the Hierarchy.");
            return;
        };
        let Some(entity) = self.state.scene.get_mut(selected) else {
            ui.weak("Selected entity no longer exists.");
            return;
        };

        ui.label(format!("Selected Object: {}", entity.name));
        ui.small(format!("Entity ID: {}", entity.id.0));
        ui.add_space(8.0);

        egui::CollapsingHeader::new("Transform")
            .default_open(true)
            .show(ui, |ui| {
                vector3_editor(ui, "Location", &mut entity.transform.translation);
                vector3_editor(ui, "Rotation", &mut entity.transform.rotation);
                vector3_editor(ui, "Scale", &mut entity.transform.scale);
            });

        egui::CollapsingHeader::new("Components")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Transform");
                ui.weak("Additional components are not implemented yet.");
            });
    }

    fn content_browser_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("Content");
            ui.separator();
            ui.label("AstraProject / Content");
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for (icon, name) in [
                ("📁", "Meshes"),
                ("📁", "Materials"),
                ("📁", "Textures"),
                ("📁", "Scenes"),
                ("📁", "Audio"),
            ] {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(icon);
                        ui.label(name);
                    });
                });
            }
        });
        ui.add_space(8.0);
        ui.weak("Asset indexing and file operations will be added with the project system.");
    }

    fn console_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong("Output Log");
            if ui.button("Clear").clicked() {
                self.state.console_lines.clear();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.state.console_lines {
                    ui.monospace(line);
                }
            });
    }
}

fn vector3_editor(ui: &mut Ui, label: &str, values: &mut [f32; 3]) {
    ui.horizontal(|ui| {
        ui.label(label);
        for (axis, value) in ["X", "Y", "Z"].into_iter().zip(values.iter_mut()) {
            ui.label(axis);
            ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(3));
        }
    });
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AstraEngine Editor 0.4")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 680.0]),
        renderer: eframe::Renderer::Glow,
        depth_buffer: 24,
        ..Default::default()
    };

    eframe::run_native(
        "AstraEngine Editor 0.4",
        options,
        Box::new(|cc| {
            let mut editor = AstraEditor::new(cc);
            let runtime = astra_runtime::GameRuntime::default();
            editor.log(format!("Runtime state: {:?}", runtime.state()));
            Ok(Box::new(editor))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_contains_all_five_editor_panels() {
        let dock = default_dock_state();
        let tabs: Vec<EditorTab> = dock
            .main_surface()
            .iter_all_tabs()
            .copied()
            .collect();

        for expected in [
            EditorTab::Hierarchy,
            EditorTab::Viewport,
            EditorTab::Inspector,
            EditorTab::ContentBrowser,
            EditorTab::Console,
        ] {
            assert!(tabs.contains(&expected), "missing panel: {expected:?}");
        }
    }

    #[test]
    fn editor_state_has_a_default_selection() {
        let state = EditorState::default();
        let selected = state.selected_entity.expect("default entity is selected");
        assert_eq!(state.scene.get(selected).expect("selected entity exists").name, "Cube");
        assert_eq!(state.scene.len(), 3);
    }
}
