//! Engine-owned scene document model. Rendering and editor UI stay outside this crate.

/// Stable identifier for an entity within a scene document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// Three-dimensional translation, rotation (Euler degrees), and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation_degrees: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

/// A named scene entity. Hierarchy storage will be added with scene document support.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub transform: Transform,
}

impl Entity {
    pub fn new(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            transform: Transform::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entity_starts_with_identity_transform() {
        let entity = Entity::new(EntityId(1), "Camera");
        assert_eq!(entity.transform, Transform::default());
        assert_eq!(entity.name, "Camera");
    }
}
