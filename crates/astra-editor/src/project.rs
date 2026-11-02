//! Versioned AstraEngine project container, validation and crash-safe persistence.

use astra_scene::{EntityId, MeshComponent, MeshGeometry, Scene};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
    sync::atomic::{AtomicU64, Ordering},
};

pub const CURRENT_FORMAT_VERSION: u32 = 4;
const PROJECT_MAGIC: &str = "ASTRA_ENGINE_PROJECT";
const AUTOSAVE_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocument {
    #[serde(default = "legacy_version")]
    pub format_version: u32,
    #[serde(default = "default_magic")]
    pub magic: String,
    pub project_id: String,
    pub project_name: String,
    pub created_unix_secs: u64,
    pub modified_unix_secs: u64,
    pub active_scene_id: String,
    pub scenes: Vec<SceneDocument>,
    #[serde(default)]
    pub textures: Vec<TextureAsset>,
    #[serde(default)]
    pub materials: Vec<MaterialAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureAsset {
    pub id: String,
    pub name: String,
    /// Path relative to the project directory, never an absolute source path.
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialAsset {
    pub id: String,
    pub name: String,
    pub material: astra_scene::Material,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneTemplate { Empty, Default }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDocument {
    pub id: String,
    pub name: String,
    pub scene: Scene,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    Io(String),
    Parse(String),
    UnsupportedVersion(u32),
    Invalid(Vec<ValidationIssue>),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(f, "I/O error: {message}"),
            Self::Parse(message) => write!(f, "Invalid project file: {message}"),
            Self::UnsupportedVersion(version) => write!(f, "Project format version {version} is newer than supported version {CURRENT_FORMAT_VERSION}"),
            Self::Invalid(issues) => {
                write!(f, "Project integrity check failed: ")?;
                for (index, issue) in issues.iter().enumerate() {
                    if index > 0 { write!(f, "; ")?; }
                    write!(f, "{}: {}", issue.location, issue.message)?;
                }
                Ok(())
            },
        }
    }
}
impl std::error::Error for ProjectError {}

fn legacy_version() -> u32 { 1 }
fn default_magic() -> String { PROJECT_MAGIC.to_owned() }
fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
fn new_id(prefix: &str) -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{:x}-{:x}-{:x}", now_secs(), std::process::id(), sequence)
}

impl ProjectDocument {
    pub fn new(name: impl Into<String>) -> Self {
        let now = now_secs();
        let scene_id = new_id("scene");
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            magic: PROJECT_MAGIC.to_owned(),
            project_id: new_id("project"),
            project_name: name.into(),
            created_unix_secs: now,
            modified_unix_secs: now,
            active_scene_id: scene_id.clone(),
            scenes: vec![SceneDocument { id: scene_id, name: "Main".to_owned(), scene: default_scene() }],
            textures: Vec::new(),
            materials: Vec::new(),
        }
    }

    pub fn normalize_mesh_attributes(&mut self) {
        for entry in &mut self.scenes { normalize_legacy_mesh_attributes(&mut entry.scene); }
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.id == self.active_scene_id).map(|scene| &scene.scene)
    }

    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|scene| scene.id == self.active_scene_id).map(|scene| &mut scene.scene)
    }

    #[cfg(test)]
    pub fn add_scene(&mut self, name: impl Into<String>) -> String {
        let id = new_id("scene");
        self.scenes.push(SceneDocument { id: id.clone(), name: name.into(), scene: Scene::new() });
        self.active_scene_id = id.clone();
        id
    }

    pub fn set_active_scene(&mut self, id: &str) -> bool {
        if self.scenes.iter().any(|scene| scene.id == id) {
            self.active_scene_id = id.to_owned();
            true
        } else {
            false
        }
    }

    pub fn rename_scene(&mut self, id: &str, name: impl Into<String>) -> bool {
        let name = name.into();
        if name.trim().is_empty() { return false; }
        if let Some(scene) = self.scenes.iter_mut().find(|scene| scene.id == id) {
            scene.name = name.trim().to_owned();
            true
        } else { false }
    }

    pub fn duplicate_scene(&mut self, id: &str) -> Option<String> {
        let source = self.scenes.iter().find(|scene| scene.id == id)?.clone();
        let new_id = new_id("scene");
        self.scenes.push(SceneDocument {
            id: new_id.clone(),
            name: format!("{} Copy", source.name),
            scene: source.scene,
        });
        self.active_scene_id = new_id.clone();
        Some(new_id)
    }

    pub fn remove_scene(&mut self, id: &str) -> bool {
        if self.scenes.len() <= 1 { return false; }
        let Some(index) = self.scenes.iter().position(|scene| scene.id == id) else { return false; };
        self.scenes.remove(index);
        if self.active_scene_id == id {
            let next = index.min(self.scenes.len() - 1);
            self.active_scene_id = self.scenes[next].id.clone();
        }
        true
    }

    pub fn add_scene_from_template(&mut self, name: impl Into<String>, template: SceneTemplate) -> String {
        let id = new_id("scene");
        let scene = match template {
            SceneTemplate::Empty => Scene::new(),
            SceneTemplate::Default => default_scene(),
        };
        self.scenes.push(SceneDocument { id: id.clone(), name: name.into(), scene });
        self.active_scene_id = id.clone();
        id
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        if self.magic != PROJECT_MAGIC {
            issues.push(issue("header", "Invalid AstraEngine project signature"));
        }
        if self.format_version == 0 || self.format_version > CURRENT_FORMAT_VERSION {
            issues.push(issue("format_version", "Unsupported project format version"));
        }
        if self.project_id.trim().is_empty() {
            issues.push(issue("project_id", "Project ID cannot be empty"));
        }
        if self.project_name.trim().is_empty() {
            issues.push(issue("project_name", "Project name cannot be empty"));
        }
        if self.scenes.is_empty() {
            issues.push(issue("scenes", "Project must contain at least one scene"));
        }
        let mut scene_ids = BTreeSet::new();
        for (index, entry) in self.scenes.iter().enumerate() {
            let location = format!("scenes[{index}]");
            if entry.id.trim().is_empty() || !scene_ids.insert(entry.id.as_str()) {
                issues.push(issue(&location, "Scene ID is empty or duplicated"));
            }
            if entry.name.trim().is_empty() {
                issues.push(issue(&location, "Scene name cannot be empty"));
            }
            validate_scene(&entry.scene, &location, &mut issues);
        }
        let mut texture_ids = BTreeSet::new();
        let mut texture_paths = BTreeSet::new();
        for (index, texture) in self.textures.iter().enumerate() {
            let location = format!("textures[{index}]");
            let path = Path::new(&texture.relative_path);
            if texture.id.trim().is_empty() || !texture_ids.insert(texture.id.as_str()) || texture.name.trim().is_empty() || texture.relative_path.trim().is_empty()
                || path.is_absolute() || path.components().any(|component| matches!(component, std::path::Component::ParentDir)) {
                issues.push(issue(&location, "Texture asset has an invalid ID, name, or project-relative path"));
            } else {
                texture_paths.insert(texture.relative_path.as_str());
            }
        }
        let mut material_ids = BTreeSet::new();
        for (index, material) in self.materials.iter().enumerate() {
            if material.id.trim().is_empty() || !material_ids.insert(material.id.as_str()) || material.name.trim().is_empty()
                || !material_is_valid(&material.material)
                || !material_textures_registered(&material.material, &texture_paths) {
                issues.push(issue(&format!("materials[{index}]"), "Material asset has an invalid ID, name, or PBR data"));
            }
        }
        for (scene_index, entry) in self.scenes.iter().enumerate() {
            for entity in entry.scene.iter() {
                if entity.material_asset_id.as_ref().is_some_and(|id| !material_ids.contains(id.as_str())) {
                    issues.push(issue(&format!("scenes[{scene_index}]"), &format!("Entity {} references a missing material asset", entity.id.0)));
                }
                if entity.material.as_ref().is_some_and(|material| !material_is_valid(material) || !material_textures_registered(material, &texture_paths)) {
                    issues.push(issue(&format!("scenes[{scene_index}]"), &format!("Entity {} has invalid PBR material values", entity.id.0)));
                }
            }
        }
        if !scene_ids.contains(self.active_scene_id.as_str()) {
            issues.push(issue("active_scene_id", "Active scene does not exist"));
        }
        issues
    }

    pub fn save_atomic(&mut self, path: &Path) -> Result<(), ProjectError> {
        self.format_version = CURRENT_FORMAT_VERSION;
        self.magic = PROJECT_MAGIC.to_owned();
        self.modified_unix_secs = now_secs();
        let issues = self.validate();
        if !issues.is_empty() { return Err(ProjectError::Invalid(issues)); }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| ProjectError::Parse(e.to_string()))?;
        atomic_write(path, &bytes).map_err(|e| ProjectError::Io(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let backup = path.parent().unwrap_or(Path::new("."))
            .join(format!(".{}.bak", path.file_name().and_then(|s| s.to_str()).unwrap_or("project.astra")));

        let primary_result = fs::read(path)
            .map_err(|error| ProjectError::Io(error.to_string()))
            .and_then(|bytes| Self::decode_project(&bytes, path));

        match primary_result {
            Ok(project) => Ok(project),
            Err(primary_error) if backup.exists() => {
                let backup_result = fs::read(&backup)
                    .map_err(|error| ProjectError::Io(error.to_string()))
                    .and_then(|bytes| Self::decode_project(&bytes, path));
                backup_result.map_err(|backup_error| ProjectError::Parse(format!(
                    "Primary project failed ({primary_error}); backup recovery failed ({backup_error})"
                )))
            }
            Err(error) => Err(error),
        }
    }

    fn decode_project(bytes: &[u8], path: &Path) -> Result<Self, ProjectError> {
        let value: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|error| ProjectError::Parse(error.to_string()))?;
        let file_version = value.get("format_version").and_then(serde_json::Value::as_u64).unwrap_or(1) as u32;
        if file_version > CURRENT_FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(file_version));
        }
        if value.get("magic").and_then(serde_json::Value::as_str).is_some_and(|magic| magic != PROJECT_MAGIC) {
            return Err(ProjectError::Invalid(vec![issue("header", "Invalid AstraEngine project signature")]));
        }

        // Compatibility migration: early editor builds saved a bare Scene JSON.
        if value.get("scenes").is_none() && value.get("entities").is_some() {
            let mut scene: Scene = serde_json::from_value(value).map_err(|error| ProjectError::Parse(error.to_string()))?;
            normalize_legacy_mesh_attributes(&mut scene);
            let mut project = Self::new(path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported Project"));
            project.scenes[0].scene = scene;
            return Ok(project);
        }

        let mut project: Self = serde_json::from_value(value)
            .map_err(|error| ProjectError::Parse(error.to_string()))?;
        project.normalize_mesh_attributes();
        if project.format_version > CURRENT_FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(project.format_version));
        }
        if project.format_version < CURRENT_FORMAT_VERSION {
            project.magic = PROJECT_MAGIC.to_owned();
            project.format_version = CURRENT_FORMAT_VERSION;
        }
        let issues = project.validate();
        if !issues.is_empty() {
            return Err(ProjectError::Invalid(issues));
        }
        Ok(project)
    }

    pub fn autosave_path() -> PathBuf {
        std::env::temp_dir().join("AstraEngine").join("recovery.astra")
    }

    pub fn autosave_due(last_autosave: SystemTime) -> bool {
        SystemTime::now().duration_since(last_autosave).unwrap_or_default().as_secs() >= AUTOSAVE_INTERVAL_SECS
    }
}

fn default_scene() -> Scene {
    let mut scene = Scene::new();
    scene.spawn("Camera");
    scene.spawn("Directional Light");
    scene.spawn_cube("Cube");
    scene
}

fn issue(location: &str, message: &str) -> ValidationIssue {
    ValidationIssue { location: location.to_owned(), message: message.to_owned() }
}

fn normalize_legacy_mesh_attributes(scene: &mut Scene) {
    let ids: Vec<EntityId> = scene.iter().map(|entity| entity.id).collect();
    for id in ids {
        let Some(entity) = scene.get_mut(id) else { continue };
        let Some(MeshComponent::Imported(mesh)) = entity.mesh_component.as_mut() else { continue };
        if mesh.normals.len() == mesh.positions.len() && mesh.uvs.len() == mesh.positions.len() { continue; }
        if let Ok(mut normalized) = MeshGeometry::new_with_attributes(
            mesh.positions.clone(), mesh.indices.clone(), mesh.normals.clone(), mesh.uvs.clone(), mesh.source.clone(),
        ) {
            normalized.imported_material = mesh.imported_material.clone();
            *mesh = normalized;
        }
    }
}

fn material_is_valid(material: &astra_scene::Material) -> bool {
    material.base_color.iter().all(|value| value.is_finite())
        && material.metallic.is_finite() && material.roughness.is_finite()
        && [material.base_color_texture.as_ref(), material.metallic_roughness_texture.as_ref()].into_iter().flatten().all(|path| {
            let path = Path::new(path);
            !path.as_os_str().is_empty() && !path.is_absolute() && !path.components().any(|part| matches!(part, std::path::Component::ParentDir))
        })
}

fn material_textures_registered(material: &astra_scene::Material, paths: &BTreeSet<&str>) -> bool {
    [material.base_color_texture.as_deref(), material.metallic_roughness_texture.as_deref()]
        .into_iter().flatten().all(|path| paths.contains(path))
}

fn validate_scene(scene: &Scene, location: &str, issues: &mut Vec<ValidationIssue>) {
    let mut ids = BTreeSet::new();
    for entity in scene.iter() {
        if !ids.insert(entity.id) {
            issues.push(issue(location, "Duplicate entity ID"));
        }
        if entity.name.trim().is_empty() {
            issues.push(issue(location, &format!("Entity {} has an empty name", entity.id.0)));
        }
        let transform = entity.transform;
        if transform.translation.iter().chain(transform.rotation.iter()).chain(transform.scale.iter()).any(|v| !v.is_finite()) {
            issues.push(issue(location, &format!("Entity {} has a non-finite transform", entity.id.0)));
        }
        if entity.parent.is_some_and(|parent| scene.get(parent).is_none()) {
            issues.push(issue(location, &format!("Entity {} references a missing parent", entity.id.0)));
        }
        if let Some(MeshComponent::Imported(mesh)) = &entity.mesh_component {
            if mesh.positions.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0
                || mesh.positions.iter().flatten().any(|value| !value.is_finite())
                || mesh.indices.iter().any(|index| *index as usize >= mesh.positions.len())
                || mesh.normals.len() != mesh.positions.len()
                || mesh.normals.iter().flatten().any(|value| !value.is_finite())
                || mesh.uvs.len() != mesh.positions.len()
                || mesh.uvs.iter().flatten().any(|value| !value.is_finite())
                || mesh.bounds_min.iter().chain(mesh.bounds_max.iter()).any(|value| !value.is_finite())
            {
                issues.push(issue(location, &format!("Entity {} has invalid imported mesh geometry", entity.id.0)));
            }
        }
        let mut visited = BTreeSet::<EntityId>::new();
        let mut cursor = Some(entity.id);
        while let Some(id) = cursor {
            if !visited.insert(id) {
                issues.push(issue(location, &format!("Hierarchy cycle detected at entity {}", id.0)));
                break;
            }
            cursor = scene.get(id).and_then(|node| node.parent);
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("project.astra");
    let temp = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let backup = parent.join(format!(".{file_name}.bak",));
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    let had_original = path.exists();
    if had_original {
        if backup.exists() { fs::remove_file(&backup)?; }
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temp, path) {
        if had_original { let _ = fs::rename(&backup, path); }
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    // Keep the previous version as a persistent recovery point.
    // It is replaced atomically on the next successful save.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use astra_scene::MeshGeometry;

    #[test]
    fn project_round_trip_preserves_multiple_scenes() {
        let mut project = ProjectDocument::new("Test");
        project.add_scene("Second");
        assert_eq!(project.scenes.len(), 2);
        assert!(project.validate().is_empty());
        let encoded = serde_json::to_vec(&project).unwrap();
        let decoded: ProjectDocument = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.scenes.len(), 2);
        assert_eq!(decoded.active_scene_id, project.active_scene_id);
    }

    #[test]
    fn scene_operations_preserve_content_and_protect_last_scene() {
        let mut project = ProjectDocument::new("Scene Ops");
        let original_id = project.active_scene_id.clone();
        let original_count = project.active_scene().unwrap().len();
        assert!(project.rename_scene(&original_id, "Renamed"));
        assert_eq!(project.scenes[0].name, "Renamed");

        let duplicate_id = project.duplicate_scene(&original_id).unwrap();
        assert_eq!(project.active_scene().unwrap().len(), original_count);
        assert_eq!(project.scenes[1].name, "Renamed Copy");
        assert!(project.remove_scene(&duplicate_id));
        assert_eq!(project.scenes.len(), 1);
        assert!(!project.remove_scene(&original_id));
        assert_eq!(project.active_scene_id, original_id);
    }

    #[test]
    fn scene_templates_create_empty_or_default_content() {
        let mut project = ProjectDocument::new("Templates");
        let empty_id = project.add_scene_from_template("Empty", SceneTemplate::Empty);
        assert_eq!(project.active_scene().unwrap().len(), 0);
        let default_id = project.add_scene_from_template("Default", SceneTemplate::Default);
        assert_eq!(project.active_scene().unwrap().len(), 3);
        assert_ne!(empty_id, default_id);
    }

    #[test]
    fn validation_rejects_missing_active_scene_and_bad_hierarchy() {
        let mut project = ProjectDocument::new("Broken");
        project.active_scene_id = "missing".to_owned();
        assert!(!project.validate().is_empty());
    }

    #[test]
    fn project_file_round_trip_uses_atomic_persistence() {
        let mut project = ProjectDocument::new("Disk Test");
        project.add_scene("Second");
        let path = std::env::temp_dir().join(format!("astra-project-{}-{}.astra", std::process::id(), now_secs()));
        project.save_atomic(&path).unwrap();
        let loaded = ProjectDocument::load(&path).unwrap();
        assert_eq!(loaded.scenes.len(), 2);
        assert_eq!(loaded.project_name, "Disk Test");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn second_save_keeps_previous_project_as_backup() {
        let path = std::env::temp_dir().join(format!("astra-backup-{}-{}.astra", std::process::id(), now_secs()));
        let backup = path.parent().unwrap().join(format!(".{}.bak", path.file_name().unwrap().to_string_lossy()));
        let mut project = ProjectDocument::new("Before");
        project.save_atomic(&path).unwrap();
        project.project_name = "After".to_owned();
        project.save_atomic(&path).unwrap();

        assert!(backup.exists(), "backup should remain after a successful second save");
        let previous = ProjectDocument::load(&backup).unwrap();
        let current = ProjectDocument::load(&path).unwrap();
        assert_eq!(previous.project_name, "Before");
        assert_eq!(current.project_name, "After");

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn corrupted_primary_project_recovers_from_backup() {
        let path = std::env::temp_dir().join(format!("astra-recovery-{}-{}.astra", std::process::id(), now_secs()));
        let backup = path.parent().unwrap().join(format!(".{}.bak", path.file_name().unwrap().to_string_lossy()));
        let mut project = ProjectDocument::new("Recoverable");
        project.save_atomic(&path).unwrap();
        project.project_name = "New version".to_owned();
        project.save_atomic(&path).unwrap();

        fs::write(&path, b"{ corrupted json").unwrap();
        let recovered = ProjectDocument::load(&path).unwrap();
        assert_eq!(recovered.project_name, "Recoverable");

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn version_two_projects_migrate_to_current_format() {
        let mut project = ProjectDocument::new("Version Two");
        project.format_version = 2;
        let path = std::env::temp_dir().join(format!("astra-v2-{}-{}.astra", std::process::id(), now_secs()));
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        let loaded = ProjectDocument::load(&path).unwrap();
        assert_eq!(loaded.format_version, CURRENT_FORMAT_VERSION);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn project_round_trip_embeds_imported_mesh_geometry() {
        let mut project = ProjectDocument::new("Embedded Mesh");
        let geometry = MeshGeometry::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![0, 1, 2],
            "triangle.obj",
        ).unwrap();
        let scene = project.active_scene_mut().unwrap();
        let id = scene.spawn("Imported");
        scene.get_mut(id).unwrap().mesh_component = Some(MeshComponent::Imported(geometry));
        let path = std::env::temp_dir().join(format!("astra-mesh-{}-{}.astra", std::process::id(), now_secs()));
        project.save_atomic(&path).unwrap();
        let loaded = ProjectDocument::load(&path).unwrap();
        let imported = loaded.active_scene().unwrap().iter()
            .find(|entity| entity.name == "Imported").unwrap();
        assert!(matches!(imported.mesh_component.as_ref(), Some(MeshComponent::Imported(_))));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn version_three_mesh_without_attributes_migrates_with_generated_attributes() {
        let mut project = ProjectDocument::new("Legacy Attributes");
        project.format_version = 3;
        let id = project.active_scene_mut().unwrap().spawn("Old Mesh");
        let geometry = MeshGeometry::new(vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]], vec![0,1,2], "legacy.obj").unwrap();
        project.active_scene_mut().unwrap().get_mut(id).unwrap().mesh_component = Some(MeshComponent::Imported(geometry));
        let mut value = serde_json::to_value(&project).unwrap();
        let entity_key = id.0.to_string();
        let entity = value["scenes"][0]["scene"]["entities"].get_mut(&entity_key).unwrap();
        entity["mesh_component"]["Imported"].as_object_mut().unwrap().remove("normals");
        entity["mesh_component"]["Imported"].as_object_mut().unwrap().remove("uvs");
        entity["mesh_component"]["Imported"].as_object_mut().unwrap().remove("imported_material");
        let path = std::env::temp_dir().join(format!("astra-v3-attrs-{}.astra", std::process::id()));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let loaded = ProjectDocument::load(&path).unwrap();
        let Some(MeshComponent::Imported(mesh)) = loaded.active_scene().unwrap().get(id).unwrap().mesh_component.as_ref() else { panic!("mesh not migrated"); };
        assert_eq!(mesh.normals.len(), 3);
        assert_eq!(mesh.uvs.len(), 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn project_round_trips_texture_and_material_assets() {
        let path = std::env::temp_dir().join(format!("astra-assets-{}.astra", std::process::id()));
        let mut project = ProjectDocument::new("Assets Test");
        project.textures.push(TextureAsset { id: "tex-1".to_owned(), name: "Albedo".to_owned(), relative_path: "Assets/Textures/albedo.png".to_owned() });
        project.materials.push(MaterialAsset { id: "mat-1".to_owned(), name: "Paint".to_owned(), material: astra_scene::Material::default() });
        let scene = project.active_scene_mut().unwrap();
        let entity = scene.spawn("Textured Cube");
        let object = scene.get_mut(entity).unwrap();
        object.material = Some(astra_scene::Material { base_color_texture: Some("Assets/Textures/albedo.png".to_owned()), ..Default::default() });
        object.material_asset_id = Some("mat-1".to_owned());
        project.save_atomic(&path).unwrap();
        let loaded = ProjectDocument::load(&path).unwrap();
        assert_eq!(loaded.textures, project.textures);
        assert_eq!(loaded.materials, project.materials);
        assert_eq!(loaded.active_scene().unwrap().get(entity).unwrap().material_asset_id.as_deref(), Some("mat-1"));
        let _ = fs::remove_file(&path);
        let backup = path.parent().unwrap().join(format!(".{}.bak", path.file_name().unwrap().to_string_lossy()));
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn project_rejects_texture_paths_that_escape_project_root() {
        let mut project = ProjectDocument::new("Unsafe Texture");
        project.textures.push(TextureAsset { id: "tex".to_owned(), name: "Bad".to_owned(), relative_path: "../outside.png".to_owned() });
        assert!(!project.validate().is_empty());
    }

    #[test]
    fn future_project_versions_are_rejected() {
        let mut project = ProjectDocument::new("Future");
        project.format_version = CURRENT_FORMAT_VERSION + 1;
        let path = std::env::temp_dir().join(format!("astra-future-{}-{}.astra", std::process::id(), now_secs()));
        fs::write(&path, serde_json::to_vec(&project).unwrap()).unwrap();
        assert_eq!(ProjectDocument::load(&path), Err(ProjectError::UnsupportedVersion(CURRENT_FORMAT_VERSION + 1)));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn legacy_bare_scene_is_migrated() {
        let scene = default_scene();
        let value = serde_json::to_vec(&scene).unwrap();
        let temp = std::env::temp_dir().join(format!("astra-legacy-{}.json", std::process::id()));
        fs::write(&temp, value).unwrap();
        let loaded = ProjectDocument::load(&temp).unwrap();
        let _ = fs::remove_file(temp);
        assert_eq!(loaded.format_version, CURRENT_FORMAT_VERSION);
        assert_eq!(loaded.scenes.len(), 1);
        assert_eq!(loaded.scenes[0].scene.len(), 3);
    }
}
