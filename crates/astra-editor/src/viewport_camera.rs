use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::FRAC_PI_4;

/// Builds the same perspective camera matrix for rendering and ray picking.
pub fn view_projection(
    width: i32,
    height: i32,
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
) -> Mat4 {
    let pitch = pitch.clamp(-1.35, 1.35);
    let distance = distance.clamp(2.5, 80.0);
    let eye_offset = Vec3::new(
        distance * pitch.cos() * yaw.sin(),
        distance * pitch.sin(),
        distance * pitch.cos() * yaw.cos(),
    );
    let eye = target + eye_offset;
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let projection = Mat4::perspective_rh_gl(
        FRAC_PI_4,
        width.max(1) as f32 / height.max(1) as f32,
        0.05,
        200.0,
    );
    projection * view
}

/// Moves the camera target in the camera plane.
pub fn pan_target(
    target: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    pointer_delta: Vec2,
    viewport_height: f32,
) -> Vec3 {
    let pitch = pitch.clamp(-1.35, 1.35);
    let view_direction = Vec3::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
    .normalize_or_zero();
    let right = Vec3::Y.cross(view_direction).normalize_or_zero();
    let up = view_direction.cross(right).normalize_or_zero();
    let world_per_pixel =
        (2.0 * distance.clamp(2.5, 80.0) * FRAC_PI_4.tan()) / viewport_height.max(1.0);

    target - right * pointer_delta.x * world_per_pixel
        + up * pointer_delta.y * world_per_pixel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_looks_at_target_instead_of_world_origin() {
        let target = Vec3::new(4.0, 2.0, -3.0);
        let matrix = view_projection(800, 600, 0.7, 0.4, 10.0, target);
        let clip = matrix * target.extend(1.0);
        assert!(clip.is_finite());
        assert!(clip.x.abs() < 1.0e-5);
        assert!(clip.y.abs() < 1.0e-5);
    }

    #[test]
    fn panning_changes_target_and_scales_with_viewport_height() {
        let target = Vec3::ZERO;
        let small = pan_target(target, 0.0, 0.0, 10.0, Vec2::new(20.0, 0.0), 500.0);
        let large = pan_target(target, 0.0, 0.0, 10.0, Vec2::new(20.0, 0.0), 1000.0);
        assert!(small.x < 0.0);
        assert!((small.x.abs() - large.x.abs() * 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn zero_pointer_delta_does_not_move_camera_target() {
        let target = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(
            pan_target(target, 0.5, 0.3, 8.0, Vec2::ZERO, 700.0),
            target
        );
    }
}
