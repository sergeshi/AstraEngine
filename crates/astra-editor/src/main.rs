mod gpu_viewport;
mod project;
mod viewport_camera;
mod viewport_gizmo;
mod viewport_picking;

use astra_core::EngineInfo;
use astra_scene::{EntityId, PrimitiveMesh, Scene};
use egui::{Id, Sense, Ui, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use glam::{EulerRot, Quat, Vec3};
use gpu_viewport::GpuViewport;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use viewport_picking::pick_entity;
use project::{ProjectDocument, SceneTemplate};
use std::path::PathBuf;
use std::time::SystemTime;

const DOCK_STATE_KEY: &str = "astra_editor_dock_state";
const EDITOR_STATE_KEY: &str = "astra_editor_state";
const PROJECT_STATE_KEY: &str = "astra_project_state";
const PROJECT_PATH_KEY: &str = "astra_project_path";

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
    #[serde(default)]
    selected_entities: Vec<EntityId>,
    #[serde(default)]
    undo_stack: Vec<Scene>,
    #[serde(default)]
    redo_stack: Vec<Scene>,
    console_lines: Vec<String>,
    #[serde(default)]
    dirty: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        let mut scene = Scene::new();
        scene.spawn("Camera");
        scene.spawn("Directional Light");
        let cube = scene.spawn_cube("Cube");

        Self {
            scene,
            selected_entity: Some(cube),
            selected_entities: vec![cube],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            console_lines: vec![
                "AstraEngine Editor 0.4 initialized.".to_owned(),
                "Hierarchy and Inspector use the shared scene model.".to_owned(),
            ],
            dirty: false,
        }
    }
}

impl EditorState {
    fn checkpoint(&mut self) {
        self.dirty = true;
        self.undo_stack.push(self.scene.clone());
        if self.undo_stack.len() > 128 { self.undo_stack.remove(0); }
        self.redo_stack.clear();
    }
    fn undo(&mut self) {
        if let Some(previous) = self.undo_stack.pop() {
            self.redo_stack.push(self.scene.clone()); self.scene = previous; self.dirty = true;
            self.selected_entities.retain(|id| self.scene.get(*id).is_some());
            self.selected_entity = self.selected_entities.last().copied();
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.scene.clone()); self.scene = next; self.dirty = true;
            self.selected_entities.retain(|id| self.scene.get(*id).is_some());
            self.selected_entity = self.selected_entities.last().copied();
        }
    }
    fn select(&mut self, id: EntityId, additive: bool) {
        if additive {
            if self.selected_entities.contains(&id) { self.selected_entities.retain(|v| *v != id); }
            else { self.selected_entities.push(id); }
        } else { self.selected_entities = vec![id]; }
        self.selected_entity = self.selected_entities.last().copied();
    }
    fn root_selection(&self) -> Vec<EntityId> {
        let selected = &self.selected_entities;
        selected.iter().copied().filter(|id|
            !selected.iter().any(|other| other != id && self.scene.is_descendant(*id, *other))
        ).collect()
    }
    fn delete_selection(&mut self) {
        let roots = self.root_selection(); if roots.is_empty() { return; }
        self.checkpoint();
        for id in roots { self.scene.remove_subtree(id); }
        self.selected_entities.clear(); self.selected_entity = None;
    }
    fn duplicate_selection(&mut self) {
        let roots = self.root_selection(); if roots.is_empty() { return; }
        self.checkpoint();
        self.selected_entities = roots.into_iter().filter_map(|id| self.scene.duplicate_subtree(id).ok()).collect();
        self.selected_entity = self.selected_entities.last().copied();
    }
    fn group_selection(&mut self) {
        let roots = self.root_selection(); if roots.len() < 2 { return; }
        self.checkpoint();
        let group = self.scene.spawn("Group");
        for id in roots { let _ = self.scene.set_parent(id, Some(group), true); }
        self.select(group, false);
    }
    fn unparent_selection(&mut self) {
        let roots = self.root_selection();
        let ids: Vec<_> = roots.into_iter().filter(|id| self.scene.get(*id).is_some_and(|e| e.parent.is_some())).collect();
        if ids.is_empty() { return; }
        self.checkpoint();
        for id in ids { let _ = self.scene.set_parent(id, None, true); }
    }
}

fn default_dock_state() -> DockState<EditorTab> {
    let mut dock = DockState::new(vec![EditorTab::Viewport]);
    let tree = dock.main_surface_mut();

    let [center, _hierarchy] = tree.split_left(NodeIndex::root(), 0.20, vec![EditorTab::Hierarchy]);
    let [center, _inspector] = tree.split_right(center, 0.76, vec![EditorTab::Inspector]);
    tree.split_below(
        center,
        0.72,
        vec![EditorTab::ContentBrowser, EditorTab::Console],
    );

    dock
}

#[derive(Debug, Clone, Copy)]
struct GizmoDrag {
    entity: EntityId,
    axis: viewport_gizmo::Axis,
    start: egui::Pos2,
    initial: [f32; 3],
    basis: Quat,
}

#[derive(Debug, Clone)]
enum PendingAction { NewProject, OpenProject(PathBuf) }

struct AstraEditor {
    dock_state: DockState<EditorTab>,
    state: EditorState,
    gpu_viewport: Option<Arc<Mutex<GpuViewport>>>,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    camera_target: [f32; 3],
    gizmo_mode: viewport_gizmo::Mode,
    gizmo_space: viewport_gizmo::Space,
    gizmo_drag: Option<GizmoDrag>,
    project: ProjectDocument,
    project_path: Option<PathBuf>,
    project_dirty: bool,
    last_autosave: SystemTime,
    pending_action: Option<PendingAction>,
    scene_name_dialog: Option<(String, String)>,
    scene_delete_confirmation: Option<String>,
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
        if state.selected_entities.is_empty() {
            state.selected_entities = state.selected_entity.into_iter().collect();
        }
        let mut project: ProjectDocument = cc.storage
            .and_then(|storage| eframe::get_value(storage, PROJECT_STATE_KEY))
            .unwrap_or_else(|| ProjectDocument::new("Untitled"));
        if let Some(scene) = project.active_scene() {
            state.scene = scene.clone();
        } else if let Some(scene) = project.active_scene_mut() {
            *scene = state.scene.clone();
        }
        let mut project_path: Option<PathBuf> = cc.storage
            .and_then(|storage| eframe::get_value::<Option<String>>(storage, PROJECT_PATH_KEY))
            .flatten()
            .map(PathBuf::from);
        let recovery_path = ProjectDocument::autosave_path();
        let mut was_recovered = false;
        if let Ok(recovered) = ProjectDocument::load(&recovery_path) {
            if recovered.modified_unix_secs > project.modified_unix_secs {
                project = recovered;
                was_recovered = true;
                state.scene = project.active_scene().cloned().unwrap_or_default();
                project_path = None;
                state.console_lines.push("Recovered a newer autosaved project.".to_owned());
            }
        }
        let gpu_viewport = cc.gl.as_ref().and_then(|gl| match GpuViewport::new(gl) {
            Ok(renderer) => {
                state
                    .console_lines
                    .push("OpenGL viewport renderer initialized.".to_owned());
                Some(Arc::new(Mutex::new(renderer)))
            }
            Err(error) => {
                state
                    .console_lines
                    .push(format!("GPU viewport initialization failed: {error}"));
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
            camera_target: [0.0, 0.0, 0.0],
            gizmo_mode: viewport_gizmo::Mode::Move,
            gizmo_space: viewport_gizmo::Space::World,
            gizmo_drag: None,
            project,
            project_path,
            project_dirty: was_recovered,
            last_autosave: SystemTime::now(),
            pending_action: None,
            scene_name_dialog: None,
            scene_delete_confirmation: None,
        }
    }

    fn log(&mut self, message: impl Into<String>) {
        self.state.console_lines.push(message.into());
    }

    fn project_snapshot(&self) -> ProjectDocument {
        let mut project = self.project.clone();
        if let Some(scene) = project.active_scene_mut() {
            *scene = self.state.scene.clone();
        }
        project
    }

    fn is_project_dirty(&self) -> bool {
        self.project_dirty || self.state.dirty
    }

    fn reset_scene_editor_state(&mut self, scene: Scene) {
        self.state.scene = scene;
        self.state.selected_entities.clear();
        self.state.selected_entity = None;
        self.state.undo_stack.clear();
        self.state.redo_stack.clear();
        self.state.dirty = false;
        self.gizmo_drag = None;
    }

    fn create_new_project(&mut self) {
        let state = EditorState::default();
        let mut project = ProjectDocument::new("Untitled");
        if let Some(scene) = project.active_scene_mut() { *scene = state.scene.clone(); }
        self.state = state;
        self.project = project;
        self.project_path = None;
        self.project_dirty = false;
        let _ = std::fs::remove_file(ProjectDocument::autosave_path());
        self.log("Created a new AstraEngine project.");
    }

    fn request_new_project(&mut self) {
        if self.is_project_dirty() {
            self.pending_action = Some(PendingAction::NewProject);
        } else {
            self.create_new_project();
        }
    }

    fn request_open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("AstraEngine Project", &["astra"])
            .pick_file() else { return; };
        let action = PendingAction::OpenProject(path);
        if self.is_project_dirty() {
            self.pending_action = Some(action);
        } else {
            self.execute_pending_action(action);
        }
    }

    fn execute_pending_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::NewProject => self.create_new_project(),
            PendingAction::OpenProject(path) => self.open_project(&path),
        }
    }

    fn open_project(&mut self, path: &std::path::Path) {
        match ProjectDocument::load(path) {
            Ok(project) => {
                let scene = project.active_scene().cloned().unwrap_or_default();
                self.project = project;
                self.project_path = Some(path.to_path_buf());
                self.reset_scene_editor_state(scene);
                self.project_dirty = false;
                self.state.dirty = false;
                let _ = std::fs::remove_file(ProjectDocument::autosave_path());
                self.log(format!("Opened project: {}", path.display()));
            }
            Err(error) => self.log(format!("Open failed: {error}")),
        }
    }

    fn save_project(&mut self) -> bool {
        if self.project_path.is_none() {
            return self.save_project_as();
        }
        let path = self.project_path.clone().expect("path checked");
        let mut project = self.project_snapshot();
        match project.save_atomic(&path) {
            Ok(()) => {
                self.project = project;
                self.project_dirty = false;
                self.state.dirty = false;
                let _ = std::fs::remove_file(ProjectDocument::autosave_path());
                self.log(format!("Saved project: {}", path.display()));
                true
            }
            Err(error) => {
                self.log(format!("Save failed: {error}"));
                false
            }
        }
    }

    fn save_project_as(&mut self) -> bool {
        let safe_name: String = self.project.project_name.chars()
            .map(|ch| if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { ch })
            .collect();
        let default_name = format!("{safe_name}.astra");
        let Some(path) = rfd::FileDialog::new()
            .add_filter("AstraEngine Project", &["astra"])
            .set_file_name(&default_name)
            .save_file() else { return false; };
        let mut project = self.project_snapshot();
        if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
            project.project_name = name.to_owned();
        }
        match project.save_atomic(&path) {
            Ok(()) => {
                self.project = project;
                self.project_path = Some(path.clone());
                self.project_dirty = false;
                self.state.dirty = false;
                let _ = std::fs::remove_file(ProjectDocument::autosave_path());
                self.log(format!("Saved project as: {}", path.display()));
                true
            }
            Err(error) => {
                self.log(format!("Save As failed: {error}"));
                false
            }
        }
    }

    fn add_scene(&mut self) {
        self.add_scene_from_template(SceneTemplate::Default);
    }

    fn add_scene_from_template(&mut self, template: SceneTemplate) {
        self.store_active_scene();
        let next_scene_number = self.project.scenes.len() + 1;
        let name = format!("Scene {next_scene_number}");
        self.project.add_scene_from_template(name, template);
        self.project_dirty = true;
        self.reset_scene_editor_state(self.project.active_scene().cloned().unwrap_or_default());
        self.log("Added a new scene from template.");
    }

    fn store_active_scene(&mut self) {
        if let Some(scene) = self.project.active_scene_mut() {
            *scene = self.state.scene.clone();
        }
    }

    fn duplicate_active_scene(&mut self) {
        self.store_active_scene();
        let current_id = self.project.active_scene_id.clone();
        if self.project.duplicate_scene(&current_id).is_some() {
            self.project_dirty = true;
            self.reset_scene_editor_state(self.project.active_scene().cloned().unwrap_or_default());
            self.log("Duplicated scene, including its hierarchy and object transforms.");
        }
    }

    fn rename_scene(&mut self, id: &str, name: String) {
        if self.project.rename_scene(id, name) {
            self.project_dirty = true;
            self.log("Scene renamed.");
        } else {
            self.log("Scene name cannot be empty.");
        }
    }

    fn delete_scene(&mut self, id: &str) {
        self.store_active_scene();
        if self.project.remove_scene(id) {
            self.project_dirty = true;
            self.reset_scene_editor_state(self.project.active_scene().cloned().unwrap_or_default());
            self.log("Scene deleted.");
        } else {
            self.log("Cannot delete the last scene in a project.");
        }
    }

    fn scene_management_dialogs(&mut self, ctx: &egui::Context) {
        if let Some((scene_id, mut name)) = self.scene_name_dialog.take() {
            let mut apply = false;
            let mut cancel = false;
            egui::Window::new("Rename Scene").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("Scene name");
                ui.text_edit_singleline(&mut name);
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() { apply = true; }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });
            if apply { self.rename_scene(&scene_id, name); }
            else if !cancel { self.scene_name_dialog = Some((scene_id, name)); }
        }

        if let Some(scene_id) = self.scene_delete_confirmation.clone() {
            let scene_name = self.project.scenes.iter().find(|scene| scene.id == scene_id)
                .map(|scene| scene.name.clone()).unwrap_or_else(|| "Scene".to_owned());
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("Delete Scene?").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(format!("Delete '{scene_name}'? This cannot be undone."));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() { confirm = true; }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });
            if confirm {
                self.scene_delete_confirmation = None;
                self.delete_scene(&scene_id);
            } else if cancel { self.scene_delete_confirmation = None; }
        }
    }

    fn switch_scene(&mut self, id: &str) {
        if id == self.project.active_scene_id { return; }
        if let Some(scene) = self.project.active_scene_mut() { *scene = self.state.scene.clone(); }
        if self.project.set_active_scene(id) {
            self.project_dirty = true;
            self.reset_scene_editor_state(self.project.active_scene().cloned().unwrap_or_default());
            self.log(format!("Switched to scene: {}", self.project.scenes.iter().find(|s| s.id == id).map(|s| s.name.as_str()).unwrap_or(id)));
        }
    }

    fn autosave_if_needed(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(std::time::Duration::from_secs(5));
        if !self.is_project_dirty() || !ProjectDocument::autosave_due(self.last_autosave) { return; }
        let mut project = self.project_snapshot();
        match project.save_atomic(&ProjectDocument::autosave_path()) {
            Ok(()) => {
                self.last_autosave = SystemTime::now();
                self.log("Autosave recovery snapshot written.");
            }
            Err(error) => self.log(format!("Autosave failed: {error}")),
        }
    }

    fn unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
        let mut save = false;
        let mut discard = false;
        let mut cancel = false;
        if self.pending_action.is_some() {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("The project has unsaved changes. Save before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save and Continue").clicked() { save = true; }
                        if ui.button("Discard").clicked() { discard = true; }
                        if ui.button("Cancel").clicked() { cancel = true; }
                    });
                });
        }
        if cancel { self.pending_action = None; }
        if save {
            if self.save_project() {
                if let Some(action) = self.pending_action.take() { self.execute_pending_action(action); }
            }
        }
        if discard {
            if let Some(action) = self.pending_action.take() { self.execute_pending_action(action); }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() { return; }
        let (undo, redo, duplicate, delete, select_all, new_project, open_project, save, save_as) = ctx.input(|i| (
            i.modifiers.command && i.key_pressed(egui::Key::Z),
            i.modifiers.command && i.key_pressed(egui::Key::Y),
            i.modifiers.command && i.key_pressed(egui::Key::D),
            i.key_pressed(egui::Key::Delete),
            i.modifiers.command && i.key_pressed(egui::Key::A),
            i.modifiers.command && i.key_pressed(egui::Key::N),
            i.modifiers.command && i.key_pressed(egui::Key::O),
            i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S),
            i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S),
        ));
        if undo { self.state.undo(); }
        if redo { self.state.redo(); }
        if duplicate { self.state.duplicate_selection(); }
        if delete { self.state.delete_selection(); }
        if select_all {
            self.state.selected_entities = self.state.scene.iter().map(|e| e.id).collect();
            self.state.selected_entity = self.state.selected_entities.last().copied();
        }
        if new_project { self.request_new_project(); }
        if open_project { self.request_open_project(); }
        if save { self.save_project(); }
        if save_as { self.save_project_as(); }
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("main_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project  Ctrl+N").clicked() {
                        self.request_new_project();
                        ui.close_menu();
                    }
                    if ui.button("Open Project…  Ctrl+O").clicked() {
                        self.request_open_project();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save  Ctrl+S").clicked() {
                        self.save_project();
                        ui.close_menu();
                    }
                    if ui.button("Save As…  Ctrl+Shift+S").clicked() {
                        self.save_project_as();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("New Scene").clicked() {
                        self.add_scene();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(!self.state.undo_stack.is_empty(), egui::Button::new("Undo  Ctrl+Z")).clicked() { self.state.undo(); ui.close_menu(); }
                    if ui.add_enabled(!self.state.redo_stack.is_empty(), egui::Button::new("Redo  Ctrl+Y")).clicked() { self.state.redo(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("Duplicate  Ctrl+D").clicked() { self.state.duplicate_selection(); ui.close_menu(); }
                    if ui.button("Delete  Del").clicked() { self.state.delete_selection(); ui.close_menu(); }
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
                if ui.button("＋ New").clicked() { self.request_new_project(); }
                if ui.button("Open").clicked() { self.request_open_project(); }
                if ui.button("Save").clicked() { self.save_project(); }
                ui.menu_button("＋ Primitive", |ui| {
                    for (label, mesh) in [
                        ("Cube", PrimitiveMesh::Cube),
                        ("Sphere", PrimitiveMesh::Sphere),
                        ("Plane", PrimitiveMesh::Plane),
                        ("Cylinder", PrimitiveMesh::Cylinder),
                        ("Capsule", PrimitiveMesh::Capsule),
                    ] {
                        if ui.button(label).clicked() {
                            self.state.checkpoint();
                            let id = self.state.scene.spawn_primitive(label, mesh);
                            self.state.select(id, false);
                            self.project_dirty = true;
                            self.log(format!("Created {label} primitive."));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui.button("▶ Play").clicked() {
                    self.log("Play requested (game launch is not implemented yet).");
                }
                if ui.button("■ Stop").clicked() {
                    self.log("Stop requested.");
                }
                ui.separator();
                for (label, mode) in [
                    ("W Move", viewport_gizmo::Mode::Move),
                    ("E Rotate", viewport_gizmo::Mode::Rotate),
                    ("R Scale", viewport_gizmo::Mode::Scale),
                ] {
                    if ui
                        .selectable_label(self.gizmo_mode == mode, label)
                        .clicked()
                    {
                        self.gizmo_mode = mode;
                        self.gizmo_drag = None;
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(self.gizmo_space == viewport_gizmo::Space::World, "World")
                    .clicked()
                {
                    self.gizmo_space = viewport_gizmo::Space::World;
                    self.gizmo_drag = None;
                }
                if ui
                    .selectable_label(self.gizmo_space == viewport_gizmo::Space::Local, "Local")
                    .clicked()
                {
                    self.gizmo_space = viewport_gizmo::Space::Local;
                    self.gizmo_drag = None;
                }
                ui.separator();
                ui.label(format!("Project: {}", self.project.project_name));
                let scenes: Vec<(String, String)> = self.project.scenes.iter().map(|scene| (scene.id.clone(), scene.name.clone())).collect();
                let active_name = self.project.scenes.iter().find(|scene| scene.id == self.project.active_scene_id).map(|scene| scene.name.clone()).unwrap_or_else(|| "No Scene".to_owned());
                egui::ComboBox::from_id_salt("active_project_scene")
                    .selected_text(format!("Scene: {active_name}"))
                    .show_ui(ui, |ui| {
                        for (id, name) in &scenes {
                            if ui.selectable_label(*id == self.project.active_scene_id, name).clicked() {
                                self.switch_scene(id);
                            }
                        }
                    });
                ui.menu_button("⋯", |ui| {
                    let active_id = self.project.active_scene_id.clone();
                    if ui.button("Rename").clicked() {
                        self.scene_name_dialog = Some((active_id.clone(), active_name.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_active_scene();
                        ui.close_menu();
                    }
                    if ui.button("Delete").clicked() {
                        self.scene_delete_confirmation = Some(active_id);
                        ui.close_menu();
                    }
                });
                ui.menu_button("＋ Scene", |ui| {
                    if ui.button("Default Scene").clicked() {
                        self.add_scene_from_template(SceneTemplate::Default);
                        ui.close_menu();
                    }
                    if ui.button("Empty Scene").clicked() {
                        self.add_scene_from_template(SceneTemplate::Empty);
                        ui.close_menu();
                    }
                });
                if self.is_project_dirty() { ui.label("●"); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Windows · Editor 0.4");
                });
            });
        });
    }
}

impl eframe::App for AstraEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        self.autosave_if_needed(ctx);
        self.menu_bar(ctx);
        self.toolbar(ctx);
        self.unsaved_changes_dialog(ctx);
        self.scene_management_dialogs(ctx);

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.small("Ready");
                    ui.separator();
                    ui.small(format!(
                        "Selected: {}",
                        self.state
                            .selected_entity
                            .and_then(|id| self.state.scene.get(id))
                            .map(|entity| entity.name.as_str())
                            .unwrap_or("None")
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small("AstraEngine");
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut viewer = EditorTabViewer {
                state: &mut self.state,
                gpu_viewport: self.gpu_viewport.clone(),
                camera_yaw: &mut self.camera_yaw,
                camera_pitch: &mut self.camera_pitch,
                camera_distance: &mut self.camera_distance,
                camera_target: &mut self.camera_target,
                gizmo_mode: &mut self.gizmo_mode,
                gizmo_space: &mut self.gizmo_space,
                gizmo_drag: &mut self.gizmo_drag,
            };
            DockArea::new(&mut self.dock_state)
                .style(Style::from_egui(ui.style()))
                .show_inside(ui, &mut viewer);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, DOCK_STATE_KEY, &self.dock_state);
        eframe::set_value(storage, EDITOR_STATE_KEY, &self.state);
        eframe::set_value(storage, PROJECT_STATE_KEY, &self.project_snapshot());
        let stored_path = self.project_path.as_ref().map(|path| path.to_string_lossy().to_string());
        eframe::set_value(storage, PROJECT_PATH_KEY, &stored_path);
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
    camera_target: &'a mut [f32; 3],
    gizmo_mode: &'a mut viewport_gizmo::Mode,
    gizmo_space: &'a mut viewport_gizmo::Space,
    gizmo_drag: &'a mut Option<GizmoDrag>,
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
                self.state.checkpoint();
                let id = self.state.scene.spawn_cube("Cube");
                self.state.select(id, false);
                self.state.console_lines.push(format!("Created Cube ({})", id.0));
            }
            if ui.button("＋ Add Empty").clicked() {
                self.state.checkpoint();
                let id = self.state.scene.spawn("Empty");
                self.state.select(id, false);
            }
            if ui.button("Group").clicked() { self.state.group_selection(); }
            if ui.button("Unparent").clicked() { self.state.unparent_selection(); }
            if ui.button("Duplicate").clicked() { self.state.duplicate_selection(); }
            if ui.button("Delete").clicked() { self.state.delete_selection(); }
        });
        ui.separator();

        // A dedicated drop target makes moving an entity back to the scene root explicit.
        let (root_rect, root_response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 36.0),
            Sense::click_and_drag(),
        );
        let root_fill = if root_response.hovered() {
            ui.visuals().widgets.active.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        ui.painter().rect_filled(root_rect, 4.0, root_fill);
        ui.painter().text(
            root_rect.center(),
            egui::Align2::CENTER_CENTER,
            "↥  Drop here to move to scene root",
            egui::FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
        if let Some(dropped) = root_response.dnd_release_payload::<EntityId>() {
            reparent_entity(&mut self.state, *dropped, None);
        }

        let roots: Vec<EntityId> = self.state.scene.roots().map(|e| e.id).collect();
        for id in roots { hierarchy_node(ui, &mut self.state, id, 0); }
        if self.state.scene.is_empty() {
            ui.weak("Scene is empty. Add an object to begin.");
        }
    }

    fn viewport_ui(&mut self, ui: &mut Ui) {
        let available = ui.available_size().max(egui::Vec2::splat(80.0));
        let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

        if response.hovered() {
            ui.input(|input| {
                if input.key_pressed(egui::Key::W) {
                    *self.gizmo_mode = viewport_gizmo::Mode::Move;
                    *self.gizmo_drag = None;
                }
                if input.key_pressed(egui::Key::E) {
                    *self.gizmo_mode = viewport_gizmo::Mode::Rotate;
                    *self.gizmo_drag = None;
                }
                if input.key_pressed(egui::Key::R) {
                    *self.gizmo_mode = viewport_gizmo::Mode::Scale;
                    *self.gizmo_drag = None;
                }
            });
        }

        let selected_transform = self.state.selected_entity.and_then(|id| self.state.scene.world_transform(id));
        let selected_position = selected_transform.map(|transform| transform.translation);
        let selected_orientation = selected_transform
            .map(|transform| {
                Quat::from_euler(
                    EulerRot::XYZ,
                    transform.rotation[0],
                    transform.rotation[1],
                    transform.rotation[2],
                )
            })
            .unwrap_or(Quat::IDENTITY);
        let gizmo_basis = if *self.gizmo_space == viewport_gizmo::Space::Local {
            selected_orientation
        } else {
            Quat::IDENTITY
        };
        let camera_matrix = viewport_gizmo::matrix(
            rect.width(),
            rect.height(),
            *self.camera_yaw,
            *self.camera_pitch,
            *self.camera_distance,
            Vec3::from_array(*self.camera_target),
        );
        let gizmo_length = (*self.camera_distance * 0.19).clamp(0.8, 2.8);

        if response.drag_started_by(egui::PointerButton::Primary) {
            if let (Some(pointer), Some(id), Some(position)) = (
                response.interact_pointer_pos(),
                self.state.selected_entity,
                selected_position,
            ) {
                if let Some(axis) = viewport_gizmo::hit_test(
                    *self.gizmo_mode,
                    camera_matrix,
                    rect,
                    Vec3::from_array(position),
                    gizmo_length,
                    gizmo_basis,
                    pointer,
                ) {
                    self.state.checkpoint();
                    let initial = self.state.scene.world_transform(id)
                        .map(|t| match *self.gizmo_mode {
                            viewport_gizmo::Mode::Move => t.translation,
                            viewport_gizmo::Mode::Rotate => t.rotation,
                            viewport_gizmo::Mode::Scale => t.scale,
                        }).unwrap_or([0.0; 3]);
                    *self.gizmo_drag = Some(GizmoDrag {
                        entity: id,
                        axis,
                        start: pointer,
                        initial,
                        basis: gizmo_basis,
                    });
                } else {
                    *self.gizmo_drag = None;
                }
            }
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(drag), Some(pointer)) = (*self.gizmo_drag, response.interact_pointer_pos())
            {
                let delta = pointer - drag.start;
                if let Some(mut world) = self.state.scene.world_transform(drag.entity) {
                    let index = drag.axis.index();
                    match *self.gizmo_mode {
                        viewport_gizmo::Mode::Move => {
                            let axis = drag.basis * match drag.axis { viewport_gizmo::Axis::X => Vec3::X, viewport_gizmo::Axis::Y => Vec3::Y, viewport_gizmo::Axis::Z => Vec3::Z };
                            let origin = Vec3::from_array(drag.initial);
                            if let (Some(a), Some(b)) = (viewport_gizmo::project(camera_matrix, rect, origin), viewport_gizmo::project(camera_matrix, rect, origin + axis * gizmo_length)) {
                                let screen_axis = b - a;
                                let pixels_per_unit = screen_axis.length() / gizmo_length.max(0.001);
                                if pixels_per_unit > 0.01 {
                                    let amount = delta.dot(screen_axis.normalized()) / pixels_per_unit;
                                    world.translation = (origin + axis * amount).to_array();
                                }
                            }
                        }
                        viewport_gizmo::Mode::Rotate => {
                            let amount = (delta.x - delta.y) * 0.01;
                            let axis = match drag.axis { viewport_gizmo::Axis::X => Vec3::X, viewport_gizmo::Axis::Y => Vec3::Y, viewport_gizmo::Axis::Z => Vec3::Z };
                            let initial = Quat::from_euler(EulerRot::XYZ, drag.initial[0], drag.initial[1], drag.initial[2]);
                            let delta_rotation = if *self.gizmo_space == viewport_gizmo::Space::Local { Quat::from_axis_angle(axis, amount) } else { Quat::from_axis_angle(drag.basis * axis, amount) };
                            let rotation = if *self.gizmo_space == viewport_gizmo::Space::Local { initial * delta_rotation } else { delta_rotation * initial };
                            let (x, y, z) = rotation.to_euler(EulerRot::XYZ); world.rotation = [x, y, z];
                        }
                        viewport_gizmo::Mode::Scale => {
                            let mut value = drag.initial;
                            value[index] = (drag.initial[index] * (1.0 + (delta.x - delta.y) * 0.01)).max(0.01);
                            world.scale = value;
                        }
                    }
                    let _ = self.state.scene.set_world_transform(drag.entity, world);
                }
                ui.ctx().request_repaint();
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            *self.gizmo_drag = None;
        }

        if response.dragged_by(egui::PointerButton::Primary) && self.gizmo_drag.is_none() {
            let delta = ui.input(|input| input.pointer.delta());
            *self.camera_yaw -= delta.x * 0.008;
            *self.camera_pitch = (*self.camera_pitch + delta.y * 0.008).clamp(-1.3, 1.3);
            ui.ctx().request_repaint();
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = ui.input(|input| input.pointer.delta());
            *self.camera_target = viewport_camera::pan_target(
                Vec3::from_array(*self.camera_target),
                *self.camera_yaw,
                *self.camera_pitch,
                *self.camera_distance,
                glam::Vec2::new(delta.x, delta.y),
                rect.height(),
            )
            .to_array();
            ui.ctx().request_repaint();
        }
        if response.hovered() {
            let (scroll, focus_pressed) =
                ui.input(|input| (input.raw_scroll_delta.y, input.key_pressed(egui::Key::F)));
            if scroll.abs() > 0.0 {
                *self.camera_distance =
                    (*self.camera_distance * (1.0 - scroll * 0.001)).clamp(2.5, 80.0);
                ui.ctx().request_repaint();
            }
            if focus_pressed {
                if let Some(position) = selected_position {
                    *self.camera_target = position;
                    ui.ctx().request_repaint();
                }
            }
        }

        if response.clicked_by(egui::PointerButton::Primary) && self.gizmo_drag.is_none() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let picked = pick_entity(
                    &self.state.scene,
                    pointer_pos,
                    rect,
                    *self.camera_yaw,
                    *self.camera_pitch,
                    *self.camera_distance,
                    *self.camera_target,
                );
                if let Some(id) = picked {
                    let additive = ui.input(|input| input.modifiers.ctrl || input.modifiers.command);
                    self.state.select(id, additive);
                } else {
                    self.state.selected_entities.clear();
                    self.state.selected_entity = None;
                }
            }
        }

        if let Some(renderer) = &self.gpu_viewport {
            let renderer = Arc::clone(renderer);
            let yaw = *self.camera_yaw;
            let pitch = *self.camera_pitch;
            let distance = *self.camera_distance;
            let target = *self.camera_target;
            let scene = self.state.scene.clone();
            let selected_entity = self.state.selected_entity;
            ui.painter().add(egui::PaintCallback {
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
                            target,
                            &scene,
                            selected_entity,
                        );
                    }
                })),
            });
        } else {
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(27, 31, 39));
        }

        if let Some(id) = self.state.selected_entity {
            if let Some(entity) = self.state.scene.get(id) {
                if let (Some(mesh), Some(matrix)) = (entity.mesh, self.state.scene.world_matrix(id)) {
                    let (min, max) = mesh.local_bounds();
                    let pivot = Vec3::from_array(entity.pivot);
                    let corners = [
                        Vec3::new(min[0],min[1],min[2]), Vec3::new(max[0],min[1],min[2]),
                        Vec3::new(max[0],max[1],min[2]), Vec3::new(min[0],max[1],min[2]),
                        Vec3::new(min[0],min[1],max[2]), Vec3::new(max[0],min[1],max[2]),
                        Vec3::new(max[0],max[1],max[2]), Vec3::new(min[0],max[1],max[2]),
                    ];
                    let edges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)];
                    let projected: Vec<_> = corners.iter().map(|p| viewport_gizmo::project(camera_matrix, rect, matrix.transform_point3(*p-pivot))).collect();
                    for (a,b) in edges {
                        if let (Some(a),Some(b)) = (projected[a],projected[b]) {
                            ui.painter().line_segment([a,b], egui::Stroke::new(1.2, egui::Color32::from_rgb(245,190,65)));
                        }
                    }
                }
            }
        }

        if let Some(position) = selected_position {
            viewport_gizmo::draw(
                ui.painter(),
                *self.gizmo_mode,
                camera_matrix,
                rect,
                Vec3::from_array(position),
                gizmo_length,
                gizmo_basis,
            );
        }
        let painter = ui.painter();
        painter.text(
            rect.left_top() + egui::vec2(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            format!(
                "PERSPECTIVE · {:?} GIZMO · {:?}",
                self.gizmo_mode, self.gizmo_space
            )
            .to_uppercase(),
            egui::FontId::monospace(12.0),
            egui::Color32::from_rgb(195, 205, 220),
        );
        painter.text(rect.left_bottom() + egui::vec2(14.0, -12.0), egui::Align2::LEFT_BOTTOM,
            "LMB drag: orbit / gizmo   ·   MMB: pan   ·   Wheel: zoom   ·   F: focus   ·   W/E/R: tools",
            egui::FontId::proportional(12.0), egui::Color32::from_rgb(150, 164, 183));
    }

    fn inspector_ui(&mut self, ui: &mut Ui) {
        ui.heading("Details");
        ui.separator();

        let Some(selected) = self.state.selected_entity else {
            ui.weak("Select an entity in the Hierarchy.");
            return;
        };
        let before = self.state.scene.clone();
        let selected_world_matrix = self.state.scene.world_matrix(selected);
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
        if before != self.state.scene {
            self.state.dirty = true;
            self.state.undo_stack.push(before);
            self.state.redo_stack.clear();
        }

        egui::CollapsingHeader::new("Pivot")
            .default_open(false)
            .show(ui, |ui| {
                vector3_editor(ui, "Local Offset", &mut entity.pivot);
            });

        egui::CollapsingHeader::new("Mesh / Bounds")
            .default_open(true)
            .show(ui, |ui| {
                if let Some(mesh) = entity.mesh {
                    ui.label(format!("Primitive: {mesh:?}"));
                    let (min, max) = mesh.local_bounds();
                    ui.label(format!("Local Min: [{:.2}, {:.2}, {:.2}]", min[0], min[1], min[2]));
                    ui.label(format!("Local Max: [{:.2}, {:.2}, {:.2}]", max[0], max[1], max[2]));
                    if let Some(matrix) = selected_world_matrix {
                        let mut world_min = Vec3::splat(f32::INFINITY);
                        let mut world_max = Vec3::splat(f32::NEG_INFINITY);
                        for x in [min[0], max[0]] { for y in [min[1], max[1]] { for z in [min[2], max[2]] {
                            let p = matrix.transform_point3(Vec3::new(x,y,z) - Vec3::from_array(entity.pivot));
                            world_min = world_min.min(p); world_max = world_max.max(p);
                        } } }
                        ui.label(format!("World Min: [{:.2}, {:.2}, {:.2}]", world_min.x, world_min.y, world_min.z));
                        ui.label(format!("World Max: [{:.2}, {:.2}, {:.2}]", world_max.x, world_max.y, world_max.z));
                    }
                } else {
                    ui.weak("No mesh component.");
                }
            });

        egui::CollapsingHeader::new("Material")
            .default_open(true)
            .show(ui, |ui| {
                if entity.material.is_none() && ui.button("＋ Create Material").clicked() {
                    entity.material = Some(astra_scene::Material::default());
                }
                if let Some(material) = entity.material.as_mut() {
                    ui.label("Base Color");
                    for (index, channel) in ["R", "G", "B", "A"].into_iter().enumerate() {
                        ui.add(egui::Slider::new(&mut material.base_color[index], 0.0..=1.0).text(channel));
                    }
                    ui.add(egui::Slider::new(&mut material.metallic, 0.0..=1.0).text("Metallic"));
                    ui.add(egui::Slider::new(&mut material.roughness, 0.0..=1.0).text("Roughness"));
                    if ui.button("Remove Material").clicked() {
                        entity.material = None;
                    }
                } else {
                    ui.weak("No material assigned.");
                }
            });

        egui::CollapsingHeader::new("Components")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Transform");
                ui.label("Mesh");
                ui.weak("Camera, light, physics and script components are planned for later milestones.");
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

fn hierarchy_node(ui: &mut Ui, state: &mut EditorState, id: EntityId, depth: usize) {
    let Some(entity) = state.scene.get(id) else { return; };
    let name = entity.name.clone();
    let has_children = state.scene.children(id).next().is_some();
    let selected = state.selected_entities.contains(&id);

    // One response owns both click selection and drag initiation. Using
    // dnd_set_drag_payload avoids the extra Sense::drag response created by
    // Ui::dnd_drag_source, which can steal clicks from the row widget.
    let row_height = ui.spacing().interact_size.y;
    let row_width = ui.available_width().max(1.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(row_width, row_height),
        Sense::click_and_drag(),
    );
    response.dnd_set_drag_payload(id);

    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if response.hovered() {
        visuals.widgets.hovered.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 3.0, fill);
    }

    let indent = depth as f32 * 14.0;
    let icon = if has_children { "▸" } else { "◇" };
    let text_color = if selected {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };
    let font = egui::TextStyle::Body.resolve(ui.style());
    let icon_pos = egui::pos2(rect.left() + indent + 4.0, rect.center().y);
    let name_pos = egui::pos2(rect.left() + indent + 22.0, rect.center().y);
    ui.painter().text(icon_pos, egui::Align2::LEFT_CENTER, icon, font.clone(), text_color);
    ui.painter().text(
        name_pos,
        egui::Align2::LEFT_CENTER,
        format!("{name}  ({})", id.0),
        font,
        text_color,
    );

    if response.clicked() {
        let additive = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        state.select(id, additive);
    }
    if let Some(dropped) = response.dnd_release_payload::<EntityId>() {
        reparent_entity(state, *dropped, Some(id));
    }

    let children: Vec<EntityId> = state.scene.children(id).map(|e| e.id).collect();
    for child in children { hierarchy_node(ui, state, child, depth + 1); }
}

fn reparent_entity(state: &mut EditorState, entity: EntityId, parent: Option<EntityId>) {
    let can_change = state.scene.get(entity).is_some_and(|e| e.parent != parent);
    if !can_change { return; }

    let before = state.scene.clone();
    if state.scene.set_parent(entity, parent, true).is_ok() {
        state.dirty = true;
        state.undo_stack.push(before);
        if state.undo_stack.len() > 128 { state.undo_stack.remove(0); }
        state.redo_stack.clear();
        match parent {
            Some(parent) => state.console_lines.push(format!(
                "Reparented {} under {}", entity.0, parent.0
            )),
            None => state.console_lines.push(format!(
                "Moved {} to scene root", entity.0
            )),
        }
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
        let tabs: Vec<EditorTab> = dock.iter_all_tabs().map(|(_, tab)| *tab).collect();

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
        assert_eq!(
            state.scene.get(selected).expect("selected entity exists").name,
            "Cube"
        );
        assert_eq!(state.scene.len(), 3);
    }

    #[test]
    fn undo_and_redo_restore_scene_snapshots() {
        let mut state = EditorState::default();
        let before = state.scene.len();
        state.checkpoint();
        state.scene.spawn_cube("Added");
        state.undo();
        assert_eq!(state.scene.len(), before);
        state.redo();
        assert_eq!(state.scene.len(), before + 1);
    }

    #[test]
    fn hierarchy_selection_and_unparenting_keep_world_transform() {
        let mut state = EditorState::default();
        let parent = state.scene.spawn("Parent");
        let child = state.scene.spawn("Child");
        state.scene.get_mut(parent).unwrap().transform.translation = [4.0, 0.0, 0.0];
        state.scene.get_mut(child).unwrap().transform.translation = [1.0, 2.0, 0.0];
        state.scene.set_parent(child, Some(parent), true).unwrap();

        state.select(child, false);
        assert_eq!(state.selected_entity, Some(child));
        assert_eq!(state.selected_entities, vec![child]);

        let world_before = state.scene.world_matrix(child).unwrap();
        reparent_entity(&mut state, child, None);
        assert_eq!(state.scene.get(child).unwrap().parent, None);
        assert_eq!(state.scene.world_matrix(child).unwrap(), world_before);
        assert_eq!(state.undo_stack.len(), 1);
        state.undo();
        assert_eq!(state.scene.get(child).unwrap().parent, Some(parent));
    }

    #[test]
    fn grouping_selection_creates_parent_without_moving_objects() {
        let mut state = EditorState::default();
        let first = state.scene.spawn_cube("First");
        let second = state.scene.spawn_cube("Second");
        state.scene.get_mut(first).unwrap().transform.translation = [2.0, 0.0, 0.0];
        state.scene.get_mut(second).unwrap().transform.translation = [-2.0, 0.0, 0.0];
        state.selected_entities = vec![first, second];
        let first_world = state.scene.world_matrix(first).unwrap();
        let second_world = state.scene.world_matrix(second).unwrap();
        state.group_selection();
        let group = state.selected_entity.unwrap();
        assert_eq!(state.scene.get(first).unwrap().parent, Some(group));
        assert_eq!(state.scene.get(second).unwrap().parent, Some(group));
        assert_eq!(state.scene.world_matrix(first).unwrap(), first_world);
        assert_eq!(state.scene.world_matrix(second).unwrap(), second_world);
    }
}
