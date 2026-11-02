//! Serializable scene graph with local and inherited world-space transforms.

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: [f32; 3],
    /// Euler angles in radians.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}
impl Default for Transform {
    fn default() -> Self { Self { translation: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3] } }
}
impl Transform {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_euler(EulerRot::XYZ, self.rotation[0], self.rotation[1], self.rotation[2]),
            Vec3::from_array(self.translation),
        )
    }
    pub fn from_matrix(matrix: Mat4) -> Option<Self> {
        if !matrix.is_finite() { return None; }
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        if !scale.is_finite() || !rotation.is_finite() || !translation.is_finite() { return None; }
        let rebuilt = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        if matrix.to_cols_array().iter().zip(rebuilt.to_cols_array()).any(|(a, b)| (*a - b).abs() > 1.0e-3) {
            return None;
        }
        let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
        Some(Self { translation: translation.to_array(), rotation: [x, y, z], scale: scale.to_array() })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveMesh { Cube, Sphere, Plane, Cylinder, Capsule }

impl PrimitiveMesh {
    /// Triangle-list geometry in local space, centered at the origin.
    pub fn triangle_vertices(self) -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        let mut tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| { out.extend([a, b, c]); };
        match self {
            Self::Cube => {
                let faces = [
                    [[-1.,-1.,1.],[1.,-1.,1.],[1.,1.,1.],[-1.,1.,1.]],
                    [[1.,-1.,-1.],[-1.,-1.,-1.],[-1.,1.,-1.],[1.,1.,-1.]],
                    [[-1.,-1.,-1.],[-1.,-1.,1.],[-1.,1.,1.],[-1.,1.,-1.]],
                    [[1.,-1.,1.],[1.,-1.,-1.],[1.,1.,-1.],[1.,1.,1.]],
                    [[-1.,1.,1.],[1.,1.,1.],[1.,1.,-1.],[-1.,1.,-1.]],
                    [[-1.,-1.,-1.],[1.,-1.,-1.],[1.,-1.,1.],[-1.,-1.,1.]],
                ];
                for f in faces { tri(f[0],f[1],f[2]); tri(f[0],f[2],f[3]); }
            }
            Self::Plane => {
                tri([-1.,0.,-1.],[1.,0.,-1.],[1.,0.,1.]);
                tri([-1.,0.,-1.],[1.,0.,1.],[-1.,0.,1.]);
            }
            Self::Sphere | Self::Capsule => {
                let rings = 16usize; let segments = 24usize;
                for y in 0..rings {
                    let v0 = y as f32 / rings as f32;
                    let v1 = (y+1) as f32 / rings as f32;
                    let lat0 = -std::f32::consts::FRAC_PI_2 + v0*std::f32::consts::PI;
                    let lat1 = -std::f32::consts::FRAC_PI_2 + v1*std::f32::consts::PI;
                    for x in 0..segments {
                        let u0 = x as f32 / segments as f32 * std::f32::consts::TAU;
                        let u1 = (x+1) as f32 / segments as f32 * std::f32::consts::TAU;
                        let point = |lat:f32, lon:f32| {
                            let r=lat.cos(); [r*lon.cos(),lat.sin(),r*lon.sin()]
                        };
                        let a=point(lat0,u0); let b=point(lat0,u1);
                        let c=point(lat1,u1); let d=point(lat1,u0);
                        let adjust = |mut p:[f32;3]| { if self==Self::Capsule { p[1]=p[1]*0.5+p[1].signum()*0.5; } p };
                        tri(adjust(a),adjust(b),adjust(c)); tri(adjust(a),adjust(c),adjust(d));
                    }
                }
            }
            Self::Cylinder => {
                let n=32usize;
                for i in 0..n {
                    let a=i as f32/n as f32*std::f32::consts::TAU;
                    let b=(i+1) as f32/n as f32*std::f32::consts::TAU;
                    let p=|ang:f32,y:f32| [ang.cos(),y,ang.sin()];
                    let (a0,a1,b0,b1)=(p(a,-1.),p(a,1.),p(b,-1.),p(b,1.));
                    tri(a0,b0,b1); tri(a0,b1,a1);
                    tri([0.,-1.,0.],b0,a0); tri([0.,1.,0.],a1,b1);
                }
            }
        }
        out
    }

    pub fn local_bounds(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::Plane => ([-1.0, -0.025, -1.0], [1.0, 0.025, 1.0]),
            Self::Capsule => ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]),
            _ => ([-1.0; 3], [1.0; 3]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub parent: Option<EntityId>,
    pub transform: Transform,
    /// Local-space pivot offset used by editor transform tools.
    #[serde(default)]
    pub pivot: [f32; 3],
    #[serde(default)]
    pub mesh: Option<PrimitiveMesh>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    next_id: u64,
    entities: BTreeMap<EntityId, Entity>,
}
impl Scene {
    pub fn new() -> Self { Self::default() }
    pub fn spawn(&mut self, name: impl Into<String>) -> EntityId {
        self.next_id = self.next_id.max(self.entities.keys().map(|id| id.0).max().unwrap_or(0)) + 1;
        let id = EntityId(self.next_id);
        self.entities.insert(id, Entity { id, name: name.into(), parent: None, transform: Transform::default(), pivot: [0.0; 3], mesh: None });
        id
    }
    pub fn spawn_primitive(&mut self, name: impl Into<String>, mesh: PrimitiveMesh) -> EntityId {
        let id = self.spawn(name);
        self.get_mut(id).expect("new entity exists").mesh = Some(mesh);
        id
    }
    pub fn spawn_cube(&mut self, name: impl Into<String>) -> EntityId { self.spawn_primitive(name, PrimitiveMesh::Cube) }
    pub fn spawn_sphere(&mut self, name: impl Into<String>) -> EntityId { self.spawn_primitive(name, PrimitiveMesh::Sphere) }
    pub fn spawn_plane(&mut self, name: impl Into<String>) -> EntityId { self.spawn_primitive(name, PrimitiveMesh::Plane) }
    pub fn spawn_cylinder(&mut self, name: impl Into<String>) -> EntityId { self.spawn_primitive(name, PrimitiveMesh::Cylinder) }
    pub fn spawn_capsule(&mut self, name: impl Into<String>) -> EntityId { self.spawn_primitive(name, PrimitiveMesh::Capsule) }
    pub fn get(&self, id: EntityId) -> Option<&Entity> { self.entities.get(&id) }
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> { self.entities.get_mut(&id) }
    pub fn iter(&self) -> impl Iterator<Item = &Entity> { self.entities.values() }
    pub fn len(&self) -> usize { self.entities.len() }
    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    pub fn roots(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values().filter(|e| e.parent.is_none())
    }
    pub fn children(&self, parent: EntityId) -> impl Iterator<Item = &Entity> {
        self.entities.values().filter(move |e| e.parent == Some(parent))
    }
    pub fn local_matrix(&self, id: EntityId) -> Option<Mat4> { Some(self.get(id)?.transform.matrix()) }
    pub fn world_matrix(&self, id: EntityId) -> Option<Mat4> {
        let entity = self.get(id)?;
        let local = entity.transform.matrix();
        match entity.parent {
            Some(parent) => Some(self.world_matrix(parent)? * local),
            None => Some(local),
        }
    }
    pub fn world_transform(&self, id: EntityId) -> Option<Transform> {
        Transform::from_matrix(self.world_matrix(id)?)
    }
    pub fn set_world_transform(&mut self, id: EntityId, world: Transform) -> Result<(), &'static str> {
        let parent = self.get(id).ok_or("entity does not exist")?.parent;
        let local_matrix = if let Some(parent) = parent {
            let parent_world = self.world_matrix(parent).ok_or("parent does not exist")?;
            let determinant = parent_world.determinant();
            if !determinant.is_finite() || determinant.abs() < 1.0e-8 { return Err("parent transform is not invertible"); }
            parent_world.inverse() * world.matrix()
        } else { world.matrix() };
        let local = Transform::from_matrix(local_matrix).ok_or("world transform cannot be represented as local TRS")?;
        self.get_mut(id).ok_or("entity does not exist")?.transform = local;
        Ok(())
    }
    pub fn set_parent(&mut self, id: EntityId, parent: Option<EntityId>, keep_world: bool) -> Result<(), &'static str> {
        if !self.entities.contains_key(&id) { return Err("entity does not exist"); }
        if let Some(parent_id) = parent {
            if !self.entities.contains_key(&parent_id) { return Err("parent does not exist"); }
            if parent_id == id || self.is_descendant(parent_id, id) { return Err("parenting would create a cycle"); }
        }
        if self.get(id).and_then(|e| e.parent) == parent { return Ok(()); }
        let previous_parent = self.get(id).and_then(|entity| entity.parent);
        let world = if keep_world { Some(self.world_matrix(id).ok_or("invalid existing hierarchy")?) } else { None };
        self.get_mut(id).expect("entity checked").parent = parent;
        if let Some(world) = world {
            let local_matrix = if let Some(parent_id) = parent {
                let parent_world = self.world_matrix(parent_id).ok_or("invalid parent hierarchy")?;
                let determinant = parent_world.determinant();
                if !determinant.is_finite() || determinant.abs() < 1.0e-8 {
                    self.get_mut(id).expect("entity exists").parent = previous_parent;
                    return Err("parent transform is not invertible");
                }
                parent_world.inverse() * world
            } else { world };
            let Some(local) = Transform::from_matrix(local_matrix) else {
                self.get_mut(id).expect("entity exists").parent = previous_parent;
                return Err("reparenting would require a sheared transform");
            };
            self.get_mut(id).expect("entity exists").transform = local;
        }
        Ok(())
    }
    pub fn is_descendant(&self, candidate: EntityId, ancestor: EntityId) -> bool {
        let mut cursor = self.get(candidate).and_then(|e| e.parent);
        let mut visited = BTreeSet::new();
        while let Some(id) = cursor {
            if id == ancestor { return true; }
            if !visited.insert(id) { return true; }
            cursor = self.get(id).and_then(|e| e.parent);
        }
        false
    }
    pub fn subtree_ids(&self, root: EntityId) -> Vec<EntityId> {
        if !self.entities.contains_key(&root) { return Vec::new(); }
        let mut result = vec![root];
        let mut index = 0;
        while index < result.len() {
            let parent = result[index];
            result.extend(self.children(parent).map(|e| e.id));
            index += 1;
        }
        result
    }
    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        let removed = self.entities.remove(&id)?;
        for entity in self.entities.values_mut() { if entity.parent == Some(id) { entity.parent = None; } }
        Some(removed)
    }
    pub fn remove_subtree(&mut self, id: EntityId) -> Vec<Entity> {
        let ids = self.subtree_ids(id);
        ids.into_iter().rev().filter_map(|entity_id| self.entities.remove(&entity_id)).collect()
    }
    pub fn duplicate_subtree(&mut self, id: EntityId) -> Result<EntityId, &'static str> {
        let source = self.get(id).ok_or("entity does not exist")?.clone();
        let ids = self.subtree_ids(id);
        let mut remap = BTreeMap::new();
        for old in &ids {
            let original = self.get(*old).ok_or("invalid subtree")?.clone();
            let new_id = self.spawn(original.name.clone() + " Copy");
            remap.insert(*old, new_id);
            let new = self.get_mut(new_id).expect("new entity exists");
            new.transform = original.transform;
            new.pivot = original.pivot;
            new.mesh = original.mesh;
        }
        for old in &ids {
            let old_entity = self.get(*old).ok_or("invalid subtree")?;
            let new_id = remap[old];
            let parent = if *old == id { source.parent } else { old_entity.parent.and_then(|p| remap.get(&p).copied()) };
            self.get_mut(new_id).expect("new entity exists").parent = parent;
        }
        Ok(remap[&id])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: Vec3, b: Vec3) { assert!((a-b).length() < 1.0e-4, "{a:?} != {b:?}"); }

    #[test] fn spawned_entities_have_unique_ids_and_default_transforms() {
        let mut s=Scene::new(); let a=s.spawn("Camera"); let b=s.spawn("Light");
        assert_ne!(a,b); assert_eq!(s.get(a).unwrap().transform,Transform::default());
    }
    #[test] fn spawned_cube_has_mesh() { let mut s=Scene::new(); let id=s.spawn_cube("Cube"); assert_eq!(s.get(id).unwrap().mesh,Some(PrimitiveMesh::Cube)); }
    #[test] fn all_primitives_have_geometry_and_serialize() {
        let mut scene=Scene::new();
        for mesh in [PrimitiveMesh::Cube,PrimitiveMesh::Sphere,PrimitiveMesh::Plane,PrimitiveMesh::Cylinder,PrimitiveMesh::Capsule] {
            let id=scene.spawn_primitive(format!("{mesh:?}"),mesh);
            assert!(!mesh.triangle_vertices().is_empty());
            assert_eq!(scene.get(id).unwrap().mesh,Some(mesh));
        }
        let json=serde_json::to_string(&scene).unwrap();
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(),scene);
    }
    #[test] fn world_matrix_inherits_parent_translation_rotation_and_scale() {
        let mut s=Scene::new(); let p=s.spawn("P"); let c=s.spawn_cube("C");
        s.get_mut(p).unwrap().transform=Transform{translation:[3.0,0.0,0.0],rotation:[0.0,0.0,std::f32::consts::FRAC_PI_2],scale:[2.0;3]};
        s.get_mut(c).unwrap().transform.translation=[1.0,0.0,0.0]; s.set_parent(c,Some(p),false).unwrap();
        let world=s.world_matrix(c).unwrap().transform_point3(Vec3::ZERO);
        close(world,Vec3::new(3.0,2.0,0.0));
    }
    #[test] fn reparent_keep_world_preserves_world_position() {
        let mut s=Scene::new(); let p=s.spawn("P"); let c=s.spawn("C");
        s.get_mut(p).unwrap().transform.translation=[5.0,0.0,0.0];
        s.get_mut(c).unwrap().transform.translation=[2.0,1.0,0.0];
        let before=s.world_matrix(c).unwrap().transform_point3(Vec3::ZERO);
        s.set_parent(c,Some(p),true).unwrap();
        close(s.world_matrix(c).unwrap().transform_point3(Vec3::ZERO),before);
    }
    #[test] fn parenting_rejects_self_and_cycles() {
        let mut s=Scene::new(); let a=s.spawn("A"); let b=s.spawn("B");
        s.set_parent(b,Some(a),false).unwrap();
        assert!(s.set_parent(a,Some(b),false).is_err()); assert!(s.set_parent(a,Some(a),false).is_err());
    }
    #[test] fn subtree_delete_and_duplicate_preserve_hierarchy() {
        let mut s=Scene::new(); let p=s.spawn("P"); let c=s.spawn_cube("C"); s.set_parent(c,Some(p),false).unwrap();
        let copy=s.duplicate_subtree(p).unwrap(); let copy_child=s.children(copy).next().unwrap().id;
        assert_eq!(s.get(copy_child).unwrap().mesh,Some(PrimitiveMesh::Cube));
        assert_eq!(s.remove_subtree(p).len(),2); assert!(s.get(c).is_none());
    }
    #[test] fn serialization_round_trip_keeps_parent_links_and_ids() {
        let mut s=Scene::new(); let a=s.spawn("A"); let b=s.spawn("B"); s.set_parent(b,Some(a),false).unwrap();
        let json=serde_json::to_string(&s).unwrap(); let decoded:Scene=serde_json::from_str(&json).unwrap();
        assert_eq!(decoded,s); assert_eq!(decoded.get(b).unwrap().parent,Some(a));
    }
}
