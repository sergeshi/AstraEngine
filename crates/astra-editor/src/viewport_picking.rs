use crate::viewport_camera;
use astra_scene::{EntityId, PrimitiveMesh, Scene};
use egui::{Pos2, Rect};
use glam::{Mat4, Vec3, Vec4};

/// Returns the closest cube intersected by the pointer ray in the viewport.
pub fn pick_entity(
    scene: &Scene,
    pointer: Pos2,
    viewport: Rect,
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: [f32; 3],
) -> Option<EntityId> {
    if viewport.width() <= 0.0 || viewport.height() <= 0.0 || !viewport.contains(pointer) {
        return None;
    }

    let view_projection = viewport_camera::view_projection(
        viewport.width() as i32,
        viewport.height() as i32,
        yaw,
        pitch,
        distance,
        Vec3::from_array(target),
    );
    let inverse_view_projection = view_projection.inverse();

    let ndc_x = ((pointer.x - viewport.left()) / viewport.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pointer.y - viewport.top()) / viewport.height()) * 2.0;
    let near = unproject(inverse_view_projection, Vec4::new(ndc_x, ndc_y, -1.0, 1.0))?;
    let far = unproject(inverse_view_projection, Vec4::new(ndc_x, ndc_y, 1.0, 1.0))?;
    let ray_direction = (far - near).normalize_or_zero();

    if ray_direction.length_squared() < 0.99 || !ray_direction.is_finite() {
        return None;
    }

    let mut closest: Option<(f32, EntityId)> = None;

    for entity in scene.iter() {
        if entity.mesh != Some(PrimitiveMesh::Cube) {
            continue;
        }

        let Some(transform) = scene.world_matrix(entity.id) else {
            continue;
        };
        let determinant = transform.determinant();
        if !determinant.is_finite() || determinant.abs() < 1.0e-8 {
            continue;
        }

        let inverse_transform = transform.inverse();
        let local_origin = inverse_transform.transform_point3(near);
        // Do not normalize this vector: keeping its scale preserves the world-ray parameter.
        let local_direction = inverse_transform.transform_vector3(ray_direction);
        let Some(hit_distance) = intersect_unit_cube(local_origin, local_direction) else {
            continue;
        };

        if closest.is_none_or(|(best_distance, _)| hit_distance < best_distance) {
            closest = Some((hit_distance, entity.id));
        }
    }

    closest.map(|(_, id)| id)
}

fn unproject(inverse_view_projection: Mat4, point: Vec4) -> Option<Vec3> {
    let world = inverse_view_projection * point;
    if !world.is_finite() || world.w.abs() < 1.0e-8 {
        return None;
    }

    let result = world.truncate() / world.w;
    result.is_finite().then_some(result)
}

/// Slab intersection against the cube's local-space bounds [-1, 1].
fn intersect_unit_cube(origin: Vec3, direction: Vec3) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;

    for axis in 0..3 {
        let origin_axis = origin[axis];
        let direction_axis = direction[axis];

        if direction_axis.abs() < 1.0e-7 {
            if !(-1.0..=1.0).contains(&origin_axis) {
                return None;
            }
            continue;
        }

        let first = (-1.0 - origin_axis) / direction_axis;
        let second = (1.0 - origin_axis) / direction_axis;
        near = near.max(first.min(second));
        far = far.min(first.max(second));

        if near > far {
            return None;
        }
    }

    (far >= 0.0).then_some(near)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center_of(rect: Rect) -> Pos2 {
        rect.center()
    }

    #[test]
    fn center_pointer_picks_cube_at_scene_origin() {
        let mut scene = Scene::new();
        let cube = scene.spawn_cube("Cube");
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));

        let picked = pick_entity(
            &scene,
            center_of(viewport),
            viewport,
            0.0,
            0.0,
            9.0,
            [0.0; 3],
        );

        assert_eq!(picked, Some(cube));
    }

    #[test]
    fn picking_returns_nearest_cube_not_first_created_cube() {
        let mut scene = Scene::new();
        let farther = scene.spawn_cube("Farther");
        scene.get_mut(farther).unwrap().transform.translation = [0.0, 0.0, -2.0];
        let nearer = scene.spawn_cube("Nearer");
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));

        let picked = pick_entity(
            &scene,
            center_of(viewport),
            viewport,
            0.0,
            0.0,
            9.0,
            [0.0; 3],
        );

        assert_eq!(picked, Some(nearer));
    }

    #[test]
    fn picking_works_after_camera_target_moves_away_from_origin() {
        let mut scene = Scene::new();
        let cube = scene.spawn_cube("Offset Cube");
        scene.get_mut(cube).unwrap().transform.translation = [4.0, 0.0, 0.0];
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));

        let picked = pick_entity(
            &scene,
            center_of(viewport),
            viewport,
            0.0,
            0.0,
            9.0,
            [4.0, 0.0, 0.0],
        );

        assert_eq!(picked, Some(cube));
    }

    #[test]
    fn picking_ignores_entities_without_meshes() {
        let mut scene = Scene::new();
        scene.spawn("Camera");
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));

        let picked = pick_entity(
            &scene,
            center_of(viewport),
            viewport,
            0.0,
            0.0,
            9.0,
            [0.0; 3],
        );

        assert_eq!(picked, None);
    }

    #[test]
    fn pointer_outside_viewport_does_not_pick() {
        let mut scene = Scene::new();
        scene.spawn_cube("Cube");
        let viewport = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));

        let picked = pick_entity(
            &scene,
            Pos2::new(120.0, 50.0),
            viewport,
            0.0,
            0.0,
            9.0,
            [0.0; 3],
        );

        assert_eq!(picked, None);
    }
}
