//! Scene data model. Rendering and editor UI are intentionally kept out of this crate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable identifier for an entity within a scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

/// Local-space transform represented without a graphics-backend dependency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: [f32; 3],
    /// Euler angles in radians.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// Built-in primitive geometry that can be rendered by the editor viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveMesh {
    Cube,
}

/// Minimal scene graph node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub parent: Option<EntityId>,
    pub transform: Transform,
    pub mesh: Option<PrimitiveMesh>,
}

/// Serializable in-memory scene model with deterministic entity iteration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    next_id: u64,
    entities: BTreeMap<EntityId, Entity>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, name: impl Into<String>) -> EntityId {
        self.next_id = self.next_id.max(self.entities.keys().map(|id| id.0).max().unwrap_or(0)) + 1;
        let id = EntityId(self.next_id);
        self.entities.insert(
            id,
            Entity {
                id,
                name: name.into(),
                parent: None,
                transform: Transform::default(),
                mesh: None,
            },
        );
        id
    }

    pub fn spawn_cube(&mut self, name: impl Into<String>) -> EntityId {
        let id = self.spawn(name);
        if let Some(entity) = self.get_mut(id) {
            entity.mesh = Some(PrimitiveMesh::Cube);
        }
        id
    }

    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        let removed = self.entities.remove(&id)?;
        for entity in self.entities.values_mut() {
            if entity.parent == Some(id) {
                entity.parent = None;
            }
        }
        Some(removed)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawned_entities_receive_unique_ids_and_default_transforms() {
        let mut scene = Scene::new();
        let first = scene.spawn("Camera");
        let second = scene.spawn("Light");

        assert_ne!(first, second);
        assert_eq!(scene.get(first).expect("entity exists").name, "Camera");
        assert_eq!(
            scene.get(second).expect("entity exists").transform,
            Transform::default()
        );
    }

    #[test]
    fn removing_parent_detaches_children() {
        let mut scene = Scene::new();
        let parent = scene.spawn("Parent");
        let child = scene.spawn("Child");
        scene.get_mut(child).expect("child exists").parent = Some(parent);

        scene.remove(parent);

        assert_eq!(scene.get(child).expect("child remains").parent, None);
    }

    #[test]
    fn scene_round_trip_preserves_entities_and_next_identifier() {
        let mut scene = Scene::new();
        let cube = scene.spawn("Cube");
        scene.get_mut(cube).expect("cube exists").transform.translation = [1.0, 2.0, 3.0];

        let encoded = serde_json::to_string(&scene).expect("scene serializes");
        let mut decoded: Scene = serde_json::from_str(&encoded).expect("scene deserializes");
        assert_eq!(decoded, scene);
        let next = decoded.spawn("Sphere");
        assert!(next.0 > cube.0);
    }
}
