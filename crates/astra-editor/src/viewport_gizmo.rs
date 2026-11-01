use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};
use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode { Move, Rotate, Scale }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space { World, Local }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis { X, Y, Z }
impl Axis {
    pub fn index(self) -> usize { match self { Self::X => 0, Self::Y => 1, Self::Z => 2 } }
    fn vector(self) -> Vec3 { match self { Self::X => Vec3::X, Self::Y => Vec3::Y, Self::Z => Vec3::Z } }
    fn color(self) -> Color32 { match self { Self::X => Color32::from_rgb(235, 75, 70), Self::Y => Color32::from_rgb(80, 210, 115), Self::Z => Color32::from_rgb(80, 145, 245) } }
}

pub fn project(matrix: Mat4, rect: Rect, point: Vec3) -> Option<Pos2> {
    let clip = matrix * point.extend(1.0);
    if !clip.is_finite() || clip.w <= 1.0e-5 { return None; }
    let ndc = clip.truncate() / clip.w;
    Some(Pos2::new(rect.center().x + ndc.x * rect.width() * 0.5, rect.center().y - ndc.y * rect.height() * 0.5))
}

fn paths(mode: Mode, matrix: Mat4, rect: Rect, origin: Vec3, length: f32, orientation: Quat) -> Vec<(Axis, Vec<Pos2>)> {
    let mut result = Vec::new();
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let points: Vec<Pos2> = if mode == Mode::Rotate {
            let (u, v) = match axis { Axis::X => (Vec3::Y, Vec3::Z), Axis::Y => (Vec3::X, Vec3::Z), Axis::Z => (Vec3::X, Vec3::Y) };
            (0..=64).filter_map(|i| {
                let a = i as f32 / 64.0 * std::f32::consts::TAU;
                project(matrix, rect, origin + orientation * (u * a.cos() + v * a.sin()) * length)
            }).collect()
        } else {
            [origin, origin + orientation * axis.vector() * length].into_iter().filter_map(|p| project(matrix, rect, p)).collect()
        };
        if points.len() > 1 { result.push((axis, points)); }
    }
    result
}

pub fn draw(painter: &Painter, mode: Mode, matrix: Mat4, rect: Rect, origin: Vec3, length: f32, orientation: Quat) {
    let center = project(matrix, rect, origin);
    for (axis, points) in paths(mode, matrix, rect, origin, length, orientation) {
        let color = axis.color();
        painter.add(Shape::line(points.clone(), Stroke::new(2.6_f32, color)));
        if mode != Mode::Rotate {
            if let (Some(end), Some(center)) = (points.last(), center) {
                painter.circle_filled(*end, if mode == Mode::Scale { 6.0 } else { 4.5 }, color);
                painter.circle_stroke(center, 3.0, Stroke::new(1.0_f32, Color32::WHITE));
            }
        }
    }
}

pub fn hit_test(mode: Mode, matrix: Mat4, rect: Rect, origin: Vec3, length: f32, orientation: Quat, pointer: Pos2) -> Option<Axis> {
    let mut best: Option<(f32, Axis)> = None;
    for (axis, points) in paths(mode, matrix, rect, origin, length, orientation) {
        for pair in points.windows(2) {
            let distance = distance_to_segment(pointer, pair[0], pair[1]);
            if distance <= 10.0 && best.is_none_or(|(d, _)| distance < d) { best = Some((distance, axis)); }
        }
    }
    best.map(|(_, axis)| axis)
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let denom = ab.length_sq();
    if denom <= 1.0e-5 { return p.distance(a); }
    let t = ((p - a).dot(ab) / denom).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub fn matrix(width: f32, height: f32, yaw: f32, pitch: f32, distance: f32, target: Vec3) -> Mat4 {
    crate::viewport_camera::view_projection(width as i32, height as i32, yaw, pitch, distance, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projected_center_is_viewport_center() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let matrix = crate::viewport_camera::view_projection(800, 600, 0.0, 0.0, 9.0, Vec3::ZERO);
        let p = project(matrix, rect, Vec3::ZERO).unwrap();
        assert!(p.distance(rect.center()) < 0.01);
    }
    #[test]
    fn move_axis_hit_test_detects_visible_axis() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let matrix = crate::viewport_camera::view_projection(800, 600, 0.0, 0.0, 9.0, Vec3::ZERO);
        let origin = project(matrix, rect, Vec3::ZERO).unwrap();
        let end = project(matrix, rect, Vec3::X * 1.5).unwrap();
        let pointer = origin + (end - origin) * 0.7;
        assert_eq!(hit_test(Mode::Move, matrix, rect, Vec3::ZERO, 1.5, Quat::IDENTITY, pointer), Some(Axis::X));
    }
    #[test]
    fn local_orientation_rotates_gizmo_axes() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let matrix = crate::viewport_camera::view_projection(800, 600, 0.0, 0.0, 9.0, Vec3::ZERO);
        let orientation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let start = project(matrix, rect, Vec3::ZERO).unwrap();
        let end = project(matrix, rect, orientation * Vec3::X * 1.5).unwrap();
        let pointer = start + (end - start) * 0.7;
        assert_eq!(hit_test(Mode::Move, matrix, rect, Vec3::ZERO, 1.5, orientation, pointer), Some(Axis::X));
    }
}
