//! OBJ, glTF, GLB and FBX mesh import. Imported geometry is embedded in the .astra scene.
use astra_scene::MeshGeometry;
use std::{fs, path::Path};

pub fn load_mesh(path: &Path) -> Result<MeshGeometry, String> {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    let source = path.file_name().and_then(|value| value.to_str()).unwrap_or("Imported Mesh").to_owned();
    match extension.as_str() {
        "obj" => load_obj(path, source),
        "gltf" | "glb" => load_gltf(path, source),
        "fbx" => load_fbx(path, source),
        other => Err(format!("Unsupported mesh format: .{other}")),
    }
}

fn load_obj(path: &Path, source: String) -> Result<MeshGeometry, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("Cannot read OBJ: {error}"))?;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut parts = line.split_whitespace();
        match parts.next().unwrap_or("") {
            "v" => {
                let values: Result<Vec<f32>, _> = parts.take(3).map(str::parse::<f32>).collect();
                let values = values.map_err(|_| format!("Invalid vertex at OBJ line {}", line_index + 1))?;
                if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
                    return Err(format!("Invalid vertex at OBJ line {}", line_index + 1));
                }
                positions.push([values[0], values[1], values[2]]);
            }
            "f" => {
                let face: Result<Vec<u32>, String> = parts.map(|token| {
                    let raw = token.split('/').next().unwrap_or("");
                    let index: i64 = raw.parse().map_err(|_| format!("Invalid face index at OBJ line {}", line_index + 1))?;
                    if index == 0 { return Err(format!("OBJ indices are 1-based at line {}", line_index + 1)); }
                    let resolved = if index > 0 { index - 1 } else { positions.len() as i64 + index };
                    if resolved < 0 || resolved as usize >= positions.len() {
                        return Err(format!("Face index out of range at OBJ line {}", line_index + 1));
                    }
                    Ok(resolved as u32)
                }).collect();
                let face = face?;
                if face.len() < 3 { return Err(format!("Face has fewer than 3 vertices at OBJ line {}", line_index + 1)); }
                for corner in 1..face.len() - 1 {
                    indices.extend([face[0], face[corner], face[corner + 1]]);
                }
            }
            _ => {}
        }
    }
    MeshGeometry::new(positions, indices, source).map_err(str::to_owned)
}

fn load_gltf(path: &Path, source: String) -> Result<MeshGeometry, String> {
    let (document, buffers, _images) = gltf::import(path).map_err(|error| format!("glTF import failed: {error}"))?;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();

    if let Some(scene) = document.default_scene().or_else(|| document.scenes().next()) {
        for node in scene.nodes() {
            append_gltf_node(node, glam::Mat4::IDENTITY, &buffers, &mut positions, &mut indices)?;
        }
    } else {
        for mesh in document.meshes() {
            append_gltf_mesh(mesh, glam::Mat4::IDENTITY, &buffers, &mut positions, &mut indices)?;
        }
    }
    MeshGeometry::new(positions, indices, source).map_err(str::to_owned)
}

fn append_gltf_node(
    node: gltf::Node<'_>,
    parent_transform: glam::Mat4,
    buffers: &[gltf::buffer::Data],
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
) -> Result<(), String> {
    let local_transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let world_transform = parent_transform * local_transform;
    if let Some(mesh) = node.mesh() {
        append_gltf_mesh(mesh, world_transform, buffers, positions, indices)?;
    }
    for child in node.children() {
        append_gltf_node(child, world_transform, buffers, positions, indices)?;
    }
    Ok(())
}

fn append_gltf_mesh(
    mesh: gltf::Mesh<'_>,
    transform: glam::Mat4,
    buffers: &[gltf::buffer::Data],
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
) -> Result<(), String> {
    for primitive in mesh.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles { continue; }
        let reader = primitive.reader(|buffer| Some(buffers[buffer.index()].0.as_slice()));
        let primitive_positions: Vec<[f32; 3]> = reader.read_positions()
            .ok_or_else(|| format!("glTF mesh '{}' has no POSITION attribute", mesh.name().unwrap_or("unnamed")))?
            .map(|position| transform.transform_point3(glam::Vec3::from_array(position)).to_array())
            .collect();
        if primitive_positions.is_empty() { continue; }
        let primitive_indices: Vec<u32> = reader.read_indices()
            .map(|values| values.into_u32().collect())
            .unwrap_or_else(|| (0..primitive_positions.len() as u32).collect());
        if primitive_indices.len() % 3 != 0 || primitive_indices.iter().any(|index| *index as usize >= primitive_positions.len()) {
            return Err("glTF triangle primitive has invalid indices".to_owned());
        }
        let base = positions.len() as u32;
        positions.extend(primitive_positions);
        indices.extend(primitive_indices.into_iter().map(|index| base + index));
    }
    Ok(())
}

fn load_fbx(path: &Path, source: String) -> Result<MeshGeometry, String> {
    let filename = path.to_str().ok_or_else(|| "FBX path is not valid UTF-8".to_owned())?;
    let options = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        generate_missing_normals: true,
        ..Default::default()
    };
    let scene = ufbx::load_file(filename, options)
        .map_err(|error| format!("FBX import failed: {error}"))?;

    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();

    for node in &scene.nodes {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let matrix = node.geometry_to_world;
        let transform = glam::Mat4::from_cols_array(&[
            matrix.m00 as f32, matrix.m10 as f32, matrix.m20 as f32, 0.0,
            matrix.m01 as f32, matrix.m11 as f32, matrix.m21 as f32, 0.0,
            matrix.m02 as f32, matrix.m12 as f32, matrix.m22 as f32, 0.0,
            matrix.m03 as f32, matrix.m13 as f32, matrix.m23 as f32, 1.0,
        ]);

        let mut triangulated = vec![0u32; mesh.max_face_triangles.saturating_mul(3)];
        for face in &mesh.faces {
            if face.num_indices < 3 { continue; }
            let triangle_count = mesh.triangulate_face(&mut triangulated, *face) as usize;
            let corner_count = triangle_count.saturating_mul(3);
            if corner_count > triangulated.len() {
                return Err("FBX triangulation returned an invalid index count".to_owned());
            }
            for &vertex_index in &triangulated[..corner_count] {
                let vertex = mesh.vertex_position[vertex_index as usize];
                let point = transform.transform_point3(glam::Vec3::new(
                    vertex.x as f32, vertex.y as f32, vertex.z as f32,
                ));
                let next_index = u32::try_from(positions.len())
                    .map_err(|_| "FBX mesh exceeds the supported vertex count".to_owned())?;
                positions.push(point.to_array());
                indices.push(next_index);
            }
        }
    }

    if indices.is_empty() {
        return Err("FBX file contains no triangle mesh geometry".to_owned());
    }
    MeshGeometry::new(positions, indices, source).map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_import_triangulates_faces_and_supports_negative_indices() {
        let path = std::env::temp_dir().join(format!("astra-import-{}.obj", std::process::id()));
        fs::write(&path, "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf -4 -3 -2 -1\n").unwrap();
        let mesh = load_obj(&path, "quad.obj".to_owned()).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(mesh.bounds_max, [1.0, 1.0, 0.0]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn obj_import_rejects_out_of_range_face_indices() {
        let path = std::env::temp_dir().join(format!("astra-invalid-{}.obj", std::process::id()));
        fs::write(&path, "v 0 0 0\nf 1 2 3\n").unwrap();
        assert!(load_obj(&path, "invalid.obj".to_owned()).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn fbx_import_rejects_invalid_files_without_panicking() {
        let path = std::env::temp_dir().join(format!("astra-invalid-{}.fbx", std::process::id()));
        fs::write(&path, "not an FBX file").unwrap();
        assert!(load_fbx(&path, "invalid.fbx".to_owned()).is_err());
        let _ = fs::remove_file(path);
    }
}
