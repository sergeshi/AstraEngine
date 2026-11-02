//! OBJ, glTF, GLB and FBX mesh import. Imported geometry is embedded in the .astra scene.
use astra_scene::{Material, MeshGeometry};
use image::GenericImageView;
use std::{fs, path::Path, sync::atomic::{AtomicU64, Ordering}};

static NEXT_IMPORTED_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);

pub fn load_mesh(path: &Path, asset_root: Option<&Path>) -> Result<MeshGeometry, String> {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    let source = path.file_name().and_then(|value| value.to_str()).unwrap_or("Imported Mesh").to_owned();
    match extension.as_str() {
        "obj" => load_obj(path, source, asset_root),
        "gltf" | "glb" => load_gltf(path, source, asset_root),
        "fbx" => load_fbx(path, source, asset_root),
        other => Err(format!("Unsupported mesh format: .{other}")),
    }
}

fn load_obj(path: &Path, source: String, asset_root: Option<&Path>) -> Result<MeshGeometry, String> {
    use std::collections::HashMap;
    let text = fs::read_to_string(path).map_err(|error| format!("Cannot read OBJ: {error}"))?;
    let mut source_positions = Vec::<[f32; 3]>::new();
    let mut source_uvs = Vec::<[f32; 2]>::new();
    let mut source_normals = Vec::<[f32; 3]>::new();
    let mut positions = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    let mut vertex_map = HashMap::<(usize, Option<usize>, Option<usize>), u32>::new();
    let mut has_all_uvs = true;
    let mut has_all_normals = true;
    let mut material_library: Option<String> = None;

    let resolve = |raw: &str, count: usize, line: usize, kind: &str| -> Result<usize, String> {
        let index: i64 = raw.parse().map_err(|_| format!("Invalid OBJ {kind} index at line {line}"))?;
        if index == 0 { return Err(format!("OBJ indices are 1-based at line {line}")); }
        let resolved = if index > 0 { index - 1 } else { count as i64 + index };
        if resolved < 0 || resolved as usize >= count { return Err(format!("OBJ {kind} index out of range at line {line}")); }
        Ok(resolved as usize)
    };

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        let mut parts = line.split_whitespace();
        match parts.next().unwrap_or("") {
            "v" => {
                let values: Result<Vec<f32>, _> = parts.take(3).map(str::parse::<f32>).collect();
                let v = values.map_err(|_| format!("Invalid vertex at OBJ line {line_number}"))?;
                if v.len() != 3 || v.iter().any(|n| !n.is_finite()) { return Err(format!("Invalid vertex at OBJ line {line_number}")); }
                source_positions.push([v[0], v[1], v[2]]);
            }
            "vt" => {
                let values: Result<Vec<f32>, _> = parts.take(2).map(str::parse::<f32>).collect();
                let v = values.map_err(|_| format!("Invalid UV at OBJ line {line_number}"))?;
                if v.len() < 2 || v.iter().any(|n| !n.is_finite()) { return Err(format!("Invalid UV at OBJ line {line_number}")); }
                source_uvs.push([v[0], v[1]]);
            }
            "vn" => {
                let values: Result<Vec<f32>, _> = parts.take(3).map(str::parse::<f32>).collect();
                let v = values.map_err(|_| format!("Invalid normal at OBJ line {line_number}"))?;
                if v.len() != 3 || v.iter().any(|n| !n.is_finite()) { return Err(format!("Invalid normal at OBJ line {line_number}")); }
                source_normals.push([v[0], v[1], v[2]]);
            }
            "mtllib" => { material_library = parts.next().map(str::to_owned); }
            "f" => {
                let mut face = Vec::<u32>::new();
                for token in parts {
                    let mut fields = token.split('/');
                    let p = resolve(fields.next().unwrap_or(""), source_positions.len(), line_number, "position")?;
                    let uv_raw = fields.next().unwrap_or("");
                    let n_raw = fields.next().unwrap_or("");
                    let uv = if uv_raw.is_empty() { None } else { Some(resolve(uv_raw, source_uvs.len(), line_number, "UV")?) };
                    let normal = if n_raw.is_empty() { None } else { Some(resolve(n_raw, source_normals.len(), line_number, "normal")?) };
                    has_all_uvs &= uv.is_some();
                    has_all_normals &= normal.is_some();
                    let key = (p, uv, normal);
                    let index = if let Some(index) = vertex_map.get(&key) { *index } else {
                        let index = u32::try_from(positions.len()).map_err(|_| "OBJ exceeds supported vertex count".to_owned())?;
                        positions.push(source_positions[p]);
                        uvs.push(uv.map(|i| source_uvs[i]).unwrap_or([0.0, 0.0]));
                        normals.push(normal.map(|i| source_normals[i]).unwrap_or([0.0, 0.0, 0.0]));
                        vertex_map.insert(key, index);
                        index
                    };
                    face.push(index);
                }
                if face.len() < 3 { return Err(format!("Face has fewer than 3 vertices at OBJ line {line_number}")); }
                for corner in 1..face.len()-1 { indices.extend([face[0], face[corner], face[corner+1]]); }
            }
            _ => {}
        }
    }
    if positions.is_empty() || indices.is_empty() { return Err("OBJ contains no triangle mesh geometry".to_owned()); }
    if !has_all_uvs { uvs.clear(); }
    if !has_all_normals { normals.clear(); }
    let mut geometry = MeshGeometry::new_with_attributes(positions, indices, normals, uvs, source).map_err(str::to_owned)?;
    if let Some(library) = material_library {
        let mtl_path = path.parent().unwrap_or(Path::new(".")).join(library);
        if let Ok(material) = load_mtl_material(&mtl_path, asset_root) { geometry.imported_material = Some(material); }
    }
    Ok(geometry)
}

fn safe_asset_stem(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn copy_texture_file(source: &Path, root: &Path, name: &str) -> Option<String> {
    let directory = root.join("Assets").join("Textures");
    fs::create_dir_all(&directory).ok()?;
    let extension = source.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let stem = safe_asset_stem(Path::new(name).file_stem()?.to_str()?);
    let sequence = NEXT_IMPORTED_TEXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{stem}-{}-{sequence}.{}", std::process::id(), extension);
    fs::copy(source, directory.join(&filename)).ok()?;
    Some(format!("Assets/Textures/{filename}"))
}

fn save_texture_bytes(bytes: &[u8], width: u32, height: u32, color: image::ColorType, root: &Path, name: &str) -> Option<String> {
    let directory = root.join("Assets").join("Textures");
    fs::create_dir_all(&directory).ok()?;
    let sequence = NEXT_IMPORTED_TEXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{}-{sequence}.png", safe_asset_stem(name));
    let destination = directory.join(&filename);
    image::save_buffer(&destination, bytes, width, height, color).ok()?;
    Some(format!("Assets/Textures/{filename}"))
}

fn save_encoded_texture(bytes: &[u8], root: &Path, name: &str) -> Option<String> {
    let directory = root.join("Assets").join("Textures");
    fs::create_dir_all(&directory).ok()?;
    let filename = format!("{}.png", safe_asset_stem(Path::new(name).file_stem()?.to_str()?));
    let destination = directory.join(&filename);
    image::load_from_memory(bytes).ok()?.save(&destination).ok()?;
    Some(format!("Assets/Textures/{filename}"))
}

fn load_gltf(path: &Path, source: String, asset_root: Option<&Path>) -> Result<MeshGeometry, String> {
    let (document, buffers, images) = gltf::import(path).map_err(|error| format!("glTF import failed: {error}"))?;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut imported_material = None;

    if let Some(scene) = document.default_scene().or_else(|| document.scenes().next()) {
        for node in scene.nodes() {
            append_gltf_node(node, glam::Mat4::IDENTITY, &buffers, &mut positions, &mut indices, &mut normals, &mut uvs, &mut imported_material, &images, asset_root)?;
        }
    } else {
        for mesh in document.meshes() {
            append_gltf_mesh(mesh, glam::Mat4::IDENTITY, &buffers, &mut positions, &mut indices, &mut normals, &mut uvs, &mut imported_material, &images, asset_root)?;
        }
    }
    let mut geometry = MeshGeometry::new_with_attributes(positions, indices, normals, uvs, source).map_err(str::to_owned)?;
    geometry.imported_material = imported_material;
    Ok(geometry)
}

fn append_gltf_node(
    node: gltf::Node<'_>,
    parent_transform: glam::Mat4,
    buffers: &[gltf::buffer::Data],
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    imported_material: &mut Option<Material>,
    images: &[gltf::image::Data],
    asset_root: Option<&Path>,
) -> Result<(), String> {
    let local_transform = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let world_transform = parent_transform * local_transform;
    if let Some(mesh) = node.mesh() {
        append_gltf_mesh(mesh, world_transform, buffers, positions, indices, normals, uvs, imported_material, images, asset_root)?;
    }
    for child in node.children() {
        append_gltf_node(child, world_transform, buffers, positions, indices, normals, uvs, imported_material, images, asset_root)?;
    }
    Ok(())
}

fn append_gltf_mesh(
    mesh: gltf::Mesh<'_>,
    transform: glam::Mat4,
    buffers: &[gltf::buffer::Data],
    positions: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    imported_material: &mut Option<Material>,
    images: &[gltf::image::Data],
    asset_root: Option<&Path>,
) -> Result<(), String> {
    for primitive in mesh.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles { continue; }
        if imported_material.is_none() {
            let pbr = primitive.material().pbr_metallic_roughness();
            let mut material = Material { base_color: pbr.base_color_factor(), metallic: pbr.metallic_factor(), roughness: pbr.roughness_factor(), base_color_texture: None, metallic_roughness_texture: None };
            if let (Some(root), Some(info)) = (asset_root, pbr.base_color_texture()) {
                let image_index = info.texture().source().index();
                if let Some(data) = images.get(image_index) {
                    let color_type = match data.format { gltf::image::Format::R8 => image::ColorType::L8, gltf::image::Format::R8G8 => image::ColorType::La8, gltf::image::Format::R8G8B8 => image::ColorType::Rgb8, gltf::image::Format::R8G8B8A8 => image::ColorType::Rgba8, _ => image::ColorType::Rgba8 };
                    if let Some(relative) = save_texture_bytes(&data.pixels, data.width, data.height, color_type, root, &format!("gltf-base-color-{image_index}")) { material.base_color_texture = Some(relative); }
                }
            }
            if let (Some(root), Some(info)) = (asset_root, pbr.metallic_roughness_texture()) {
                let image_index = info.texture().source().index();
                if let Some(data) = images.get(image_index) {
                    let color_type = match data.format { gltf::image::Format::R8 => image::ColorType::L8, gltf::image::Format::R8G8 => image::ColorType::La8, gltf::image::Format::R8G8B8 => image::ColorType::Rgb8, gltf::image::Format::R8G8B8A8 => image::ColorType::Rgba8, _ => image::ColorType::Rgba8 };
                    if let Some(relative) = save_texture_bytes(&data.pixels, data.width, data.height, color_type, root, &format!("gltf-metallic-roughness-{image_index}")) { material.metallic_roughness_texture = Some(relative); }
                }
            }
            *imported_material = Some(material);
        }
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
        let primitive_normals: Vec<[f32; 3]> = reader.read_normals()
            .map(|values| values.map(|n| transform.inverse().transpose().transform_vector3(glam::Vec3::from_array(n)).normalize_or_zero().to_array()).collect())
            .unwrap_or_default();
        let primitive_uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
            .map(|values| values.into_f32().collect())
            .unwrap_or_default();
        let base = positions.len() as u32;
        let vertex_count = primitive_positions.len();
        positions.extend(primitive_positions);
        indices.extend(primitive_indices.into_iter().map(|index| base + index));
        if primitive_normals.len() == vertex_count { normals.extend(primitive_normals); } else { normals.clear(); }
        if primitive_uvs.len() == vertex_count { uvs.extend(primitive_uvs); } else { uvs.clear(); }
    }
    Ok(())
}

fn load_ufbx_texture_image(texture: &ufbx::Texture) -> Option<image::DynamicImage> {
    let external = Path::new(texture.absolute_filename.as_ref());
    if external.is_file() { image::open(external).ok() } else { image::load_from_memory(&texture.content).ok() }
}

fn save_packed_mr_texture(roughness: Option<image::DynamicImage>, metalness: Option<image::DynamicImage>, root: &Path, name: &str) -> Option<String> {
    let reference = roughness.as_ref().or(metalness.as_ref())?;
    let (width, height) = reference.dimensions();
    let roughness = roughness.map(|image| image.to_luma8()).map(|image| image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle));
    let metalness = metalness.map(|image| image.to_luma8()).map(|image| image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle));
    let packed = image::RgbaImage::from_fn(width, height, |x, y| {
        let r = roughness.as_ref().map(|image| image.get_pixel(x, y).0[0]).unwrap_or(255);
        let m = metalness.as_ref().map(|image| image.get_pixel(x, y).0[0]).unwrap_or(255);
        image::Rgba([255, r, m, 255])
    });
    let directory = root.join("Assets").join("Textures");
    fs::create_dir_all(&directory).ok()?;
    let sequence = NEXT_IMPORTED_TEXTURE_ID.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{}-{sequence}.png", safe_asset_stem(name));
    packed.save(directory.join(&filename)).ok()?;
    Some(format!("Assets/Textures/{filename}"))
}

fn load_fbx(path: &Path, source: String, asset_root: Option<&Path>) -> Result<MeshGeometry, String> {
    let filename = path.to_str().ok_or_else(|| "FBX path is not valid UTF-8".to_owned())?;
    let options = ufbx::LoadOpts { target_axes: ufbx::CoordinateAxes::right_handed_y_up(), target_unit_meters: 1.0, generate_missing_normals: true, normalize_normals: true, ..Default::default() };
    let scene = ufbx::load_file(filename, options).map_err(|error| format!("FBX import failed: {error:?}"))?;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut imported_material = None;
    for node in &scene.nodes {
        if imported_material.is_none() {
            if let Some(source_material) = node.materials.first() {
                let pbr = &source_material.pbr;
                let base = pbr.base_color.value_vec4;
                let base_color = if pbr.base_color.has_value { [base.x as f32, base.y as f32, base.z as f32, base.w as f32] } else { [0.72, 0.74, 0.78, 1.0] };
                let metallic = if pbr.metalness.has_value { pbr.metalness.value_vec4.x as f32 } else { 0.0 };
                let roughness = if pbr.roughness.has_value { pbr.roughness.value_vec4.x as f32 } else { 0.65 };
                let mut material = Material { base_color, metallic: metallic.clamp(0.0, 1.0), roughness: roughness.clamp(0.0, 1.0), base_color_texture: None, metallic_roughness_texture: None };
                if let (Some(texture), Some(root)) = (pbr.base_color.texture.as_ref(), asset_root) {
                    let texture_name = texture.filename.as_ref();
                    let texture_path = Path::new(texture.absolute_filename.as_ref());
                    if texture_path.is_file() {
                        if let Some(relative) = copy_texture_file(texture_path, root, texture_name) { material.base_color_texture = Some(relative); }
                    } else if !texture.content.is_empty() {
                        if let Some(relative) = save_encoded_texture(&texture.content, root, texture_name) { material.base_color_texture = Some(relative); }
                    }
                }
                if let Some(root) = asset_root {
                    let roughness = pbr.roughness.texture.as_ref().and_then(|texture| load_ufbx_texture_image(texture));
                    let metalness = pbr.metalness.texture.as_ref().and_then(|texture| load_ufbx_texture_image(texture));
                    if roughness.is_some() || metalness.is_some() {
                        if let Some(relative) = save_packed_mr_texture(roughness, metalness, root, "fbx-metallic-roughness") {
                            material.metallic_roughness_texture = Some(relative);
                        }
                    }
                }
                imported_material = Some(material);
            }
        }
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let matrix = node.geometry_to_world;
        let transform = glam::Mat4::from_cols_array(&[
            matrix.m00 as f32, matrix.m10 as f32, matrix.m20 as f32, 0.0,
            matrix.m01 as f32, matrix.m11 as f32, matrix.m21 as f32, 0.0,
            matrix.m02 as f32, matrix.m12 as f32, matrix.m22 as f32, 0.0,
            matrix.m03 as f32, matrix.m13 as f32, matrix.m23 as f32, 1.0,
        ]);
        let normal_transform = transform.inverse().transpose();
        let mut triangulated = vec![0u32; mesh.max_face_triangles.saturating_mul(3)];
        for face in &mesh.faces {
            if face.num_indices < 3 { continue; }
            let triangle_count = mesh.triangulate_face(&mut triangulated, *face) as usize;
            let corner_count = triangle_count.saturating_mul(3);
            if corner_count > triangulated.len() { return Err("FBX triangulation returned an invalid index count".to_owned()); }
            for &vertex_index in &triangulated[..corner_count] {
                let vertex = mesh.vertex_position[vertex_index as usize];
                let point = transform.transform_point3(glam::Vec3::new(vertex.x as f32, vertex.y as f32, vertex.z as f32));
                let next_index = u32::try_from(positions.len()).map_err(|_| "FBX mesh exceeds the supported vertex count".to_owned())?;
                positions.push(point.to_array()); indices.push(next_index);
                let n = ufbx::get_vertex_vec3(&mesh.vertex_normal, vertex_index as usize);
                normals.push(normal_transform.transform_vector3(glam::Vec3::new(n.x as f32, n.y as f32, n.z as f32)).normalize_or_zero().to_array());
                let uv = ufbx::get_vertex_vec2(&mesh.vertex_uv, vertex_index as usize);
                uvs.push([uv.x as f32, uv.y as f32]);
            }
        }
    }
    if indices.is_empty() { return Err("FBX file contains no triangle mesh geometry".to_owned()); }
    let mut geometry = MeshGeometry::new_with_attributes(positions, indices, normals, uvs, source).map_err(str::to_owned)?;
    geometry.imported_material = imported_material;
    Ok(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_import_triangulates_faces_and_supports_negative_indices() {
        let path = std::env::temp_dir().join(format!("astra-import-{}.obj", std::process::id()));
        fs::write(&path, "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf -4 -3 -2 -1\n").unwrap();
        let mesh = load_obj(&path, "quad.obj".to_owned(), None).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(mesh.bounds_max, [1.0, 1.0, 0.0]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn obj_import_preserves_vertex_normals_and_uvs() {
        let path = std::env::temp_dir().join(format!("astra-attrs-{}.obj", std::process::id()));
        fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\n").unwrap();
        let mesh = load_obj(&path, "attrs.obj".to_owned(), None).unwrap();
        assert_eq!(mesh.uvs, vec![[0.0,0.0],[1.0,0.0],[0.0,1.0]]);
        assert_eq!(mesh.normals, vec![[0.0,0.0,1.0];3]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn obj_mtl_import_preserves_pbr_factors_and_copies_base_color_map() {
        let root = std::env::temp_dir().join(format!("astra-mtl-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let obj = root.join("asset.obj");
        let mtl = root.join("asset.mtl");
        let texture = root.join("albedo.png");
        fs::write(&texture, b"not decoded here").unwrap();
        fs::write(&mtl, "newmtl Sample\nKd 0.2 0.4 0.6\nPm 0.7\nPr 0.3\nmap_Kd albedo.png\n").unwrap();
        fs::write(&obj, "mtllib asset.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let mesh = load_obj(&obj, "asset.obj".to_owned(), Some(&root)).unwrap();
        let material = mesh.imported_material.unwrap();
        assert_eq!(material.base_color[..3], [0.2,0.4,0.6]);
        assert!((material.metallic - 0.7).abs() < 1.0e-6);
        assert!((material.roughness - 0.3).abs() < 1.0e-6);
        assert!(material.base_color_texture.as_deref().is_some_and(|p| root.join(p).exists()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn obj_import_rejects_out_of_range_face_indices() {
        let path = std::env::temp_dir().join(format!("astra-invalid-{}.obj", std::process::id()));
        fs::write(&path, "v 0 0 0\nf 1 2 3\n").unwrap();
        assert!(load_obj(&path, "invalid.obj".to_owned(), None).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn fbx_import_rejects_invalid_files_without_panicking() {
        let path = std::env::temp_dir().join(format!("astra-invalid-{}.fbx", std::process::id()));
        fs::write(&path, "not an FBX file").unwrap();
        assert!(load_fbx(&path, "invalid.fbx".to_owned(), None).is_err());
        let _ = fs::remove_file(path);
    }
}
