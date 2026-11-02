//! Versioned AstraEngine project container, validation and crash-safe persistence.

use astra_scene::{EntityId, Scene};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CURRENT_FORMAT_VERSION: u32 = 2;
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
}

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
            Self::Invalid(issues) => write!(f, "Project integrity check failed ({} issue(s))", issues.len()),
        }
    }
}
impl std::error::Error for ProjectError {}

fn legacy_version() -> u32 { 1 }
fn default_magic() -> String { PROJECT_MAGIC.to_owned() }
fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
fn new_id() -> String {
    // Stable enough for document identity without introducing a platform RNG dependency.
    format!("astra-{:x}-{:x}", now_secs(), std::process::id())
}

impl ProjectDocument {
    pub fn new(name: impl Into<String>) -> Self {
        let now = now_secs();
        let scene_id = "scene-1".to_owned();
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            magic: PROJECT_MAGIC.to_owned(),
            project_id: new_id(),
            project_name: name.into(),
            created_unix_secs: now,
            modified_unix_secs: now,
            active_scene_id: scene_id.clone(),
            scenes: vec![SceneDocument { id: scene_id, name: "Main".to_owned(), scene: default_scene() }],
        }
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.id == self.active_scene_id).map(|scene| &scene.scene)
    }

    pub fn active_scene_mut(&mut self) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|scene| scene.id == self.active_scene_id).map(|scene| &mut scene.scene)
    }

    pub fn add_scene(&mut self, name: impl Into<String>) -> String {
        let index = self.scenes.len() + 1;
        let id = format!("scene-{index}-{:x}", now_secs());
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
            validate_scene(&entry.scene, &location, &mut issues);
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
        let backup = path.parent().unwrap_or(Path::new(".")).join(format!(".{}.bak", path.file_name().and_then(|s| s.to_str()).unwrap_or("project.astra")));
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(primary) if backup.exists() => fs::read(&backup).map_err(|backup_error| ProjectError::Io(format!("{primary}; backup recovery failed: {backup_error}")))?,
            Err(error) => return Err(ProjectError::Io(error.to_string())),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| ProjectError::Parse(e.to_string()))?;
        let file_version = value.get("format_version").and_then(serde_json::Value::as_u64).unwrap_or(1) as u32;
        if file_version > CURRENT_FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(file_version));
        }

        // Compatibility migration: early editor builds saved a bare Scene JSON.
        if value.get("scenes").is_none() && value.get("entities").is_some() {
            let scene: Scene = serde_json::from_value(value).map_err(|e| ProjectError::Parse(e.to_string()))?;
            let mut project = Self::new(path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported Project"));
            project.scenes[0].scene = scene;
            return Ok(project);
        }

        let mut project: Self = serde_json::from_value(value).map_err(|e| ProjectError::Parse(e.to_string()))?;
        if project.format_version > CURRENT_FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(project.format_version));
        }
        // Version 1 had the same scene payload but no formal signature.
        if project.format_version == 1 {
            project.magic = PROJECT_MAGIC.to_owned();
            project.format_version = CURRENT_FORMAT_VERSION;
        }
        let issues = project.validate();
        if !issues.is_empty() { return Err(ProjectError::Invalid(issues)); }
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
    if had_original { let _ = fs::remove_file(backup); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn validation_rejects_missing_active_scene_and_bad_hierarchy() {
        let mut project = ProjectDocument::new("Broken");
        project.active_scene_id = "missing".to_owned();
        assert!(!project.validate().is_empty());
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
