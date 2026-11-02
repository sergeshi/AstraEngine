#![allow(unsafe_code)]

use crate::viewport_camera;
use astra_scene::{EntityId, MeshComponent, PrimitiveMesh, Scene};
use eframe::glow::{self, HasContext};
use glam::Vec3;
use std::{collections::HashMap, path::Path};

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    metallic: f32,
    roughness: f32,
}

pub struct GpuViewport {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    view_projection: glow::UniformLocation,
    line_vertices: i32,
    base_color_texture: glow::UniformLocation,
    has_texture: glow::UniformLocation,
    metallic_roughness_texture: glow::UniformLocation,
    has_metallic_roughness_texture: glow::UniformLocation,
    textures: HashMap<String, glow::Texture>,
}

impl GpuViewport {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        let vertices = build_grid_vertices();
        unsafe {
            let vertex_shader = compile_shader(gl, glow::VERTEX_SHADER, VERTEX_SHADER)?;
            let fragment_shader = compile_shader(gl, glow::FRAGMENT_SHADER, FRAGMENT_SHADER)?;
            let program = gl.create_program().map_err(|e| e.to_string())?;
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);
            gl.link_program(program);
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);

            if !gl.get_program_link_status(program) {
                let message = gl.get_program_info_log(program);
                gl.delete_program(program);
                return Err(message);
            }

            let vertex_array = gl.create_vertex_array().map_err(|e| e.to_string())?;
            let vertex_buffer = gl.create_buffer().map_err(|e| e.to_string())?;
            gl.bind_vertex_array(Some(vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            let bytes = std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                vertices.len() * std::mem::size_of::<Vertex>(),
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);

            let stride = std::mem::size_of::<Vertex>() as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(2, 3, glow::FLOAT, false, stride, 24);
            gl.enable_vertex_attrib_array(3);
            gl.vertex_attrib_pointer_f32(3, 2, glow::FLOAT, false, stride, 36);
            gl.enable_vertex_attrib_array(4);
            gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, stride, 44);
            gl.enable_vertex_attrib_array(5);
            gl.vertex_attrib_pointer_f32(5, 1, glow::FLOAT, false, stride, 48);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            let view_projection = gl.get_uniform_location(program, "u_view_projection").ok_or_else(|| "Missing u_view_projection uniform".to_owned())?;
            let base_color_texture = gl.get_uniform_location(program, "u_base_color_texture").ok_or_else(|| "Missing u_base_color_texture uniform".to_owned())?;
            let has_texture = gl.get_uniform_location(program, "u_has_texture").ok_or_else(|| "Missing u_has_texture uniform".to_owned())?;
            let metallic_roughness_texture = gl.get_uniform_location(program, "u_metallic_roughness_texture").ok_or_else(|| "Missing u_metallic_roughness_texture uniform".to_owned())?;
            let has_metallic_roughness_texture = gl.get_uniform_location(program, "u_has_metallic_roughness_texture").ok_or_else(|| "Missing u_has_metallic_roughness_texture uniform".to_owned())?;

            Ok(Self {
                program,
                vertex_array,
                vertex_buffer,
                view_projection,
                line_vertices: build_line_vertex_count(),
                base_color_texture,
                has_texture,
                metallic_roughness_texture,
                has_metallic_roughness_texture,
                textures: HashMap::new(),
            })
        }
    }

    pub fn paint(
        &mut self,
        gl: &glow::Context,
        width: i32,
        height: i32,
        yaw: f32,
        pitch: f32,
        distance: f32,
        target: [f32; 3],
        scene: &Scene,
        selected_entity: Option<EntityId>,
        texture_root: Option<&Path>,
    ) {
        let grid = build_grid_vertices();
        let matrix = viewport_camera::view_projection(width, height, yaw, pitch, distance, Vec3::from_array(target));
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_color(0.105, 0.125, 0.16, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(Some(&self.view_projection), false, &matrix.to_cols_array());
            gl.uniform_1_i32(Some(&self.base_color_texture), 0);
            gl.uniform_1_i32(Some(&self.has_texture), 0);
            gl.uniform_1_i32(Some(&self.metallic_roughness_texture), 1);
            gl.uniform_1_i32(Some(&self.has_metallic_roughness_texture), 0);
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            upload_vertices(gl, &grid);
            gl.draw_arrays(glow::LINES, 0, self.line_vertices);
            for entity in scene.iter() {
                let vertices = build_entity_vertices(scene, entity.id, selected_entity);
                if vertices.is_empty() { continue; }
                upload_vertices(gl, &vertices);
                let material = entity.material.as_ref();
                let base_color_path = if selected_entity == Some(entity.id) { None } else { material.and_then(|m| m.base_color_texture.as_deref()) };
                let metallic_roughness_path = if selected_entity == Some(entity.id) { None } else { material.and_then(|m| m.metallic_roughness_texture.as_deref()) };
                let base_color = base_color_path.and_then(|relative| texture_root.and_then(|root| self.texture_for_path(gl, root, relative, true)));
                let metallic_roughness = metallic_roughness_path.and_then(|relative| texture_root.and_then(|root| self.texture_for_path(gl, root, relative, false)));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, base_color);
                gl.uniform_1_i32(Some(&self.has_texture), if base_color.is_some() { 1 } else { 0 });
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, metallic_roughness);
                gl.uniform_1_i32(Some(&self.has_metallic_roughness_texture), if metallic_roughness.is_some() { 1 } else { 0 });
                gl.draw_arrays(glow::TRIANGLES, 0, vertices.len() as i32);
            }
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    fn texture_for_path(&mut self, gl: &glow::Context, root: &Path, relative: &str, srgb: bool) -> Option<glow::Texture> {
        let path = root.join(relative);
        let key = format!("{}|srgb={srgb}", path.to_string_lossy());
        if let Some(texture) = self.textures.get(&key) { return Some(*texture); }
        let image = image::open(&path).ok()?.flipv().to_rgba8();
        let (width, height) = image.dimensions();
        let pixels = image.into_raw();
        unsafe {
            let texture = gl.create_texture().ok()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR_MIPMAP_LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::REPEAT as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::REPEAT as i32);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            let internal_format = if srgb { glow::SRGB8_ALPHA8 } else { glow::RGBA8 };
            gl.tex_image_2d(glow::TEXTURE_2D, 0, internal_format as i32, width as i32, height as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(&pixels));
            gl.generate_mipmap(glow::TEXTURE_2D);
            gl.bind_texture(glow::TEXTURE_2D, None);
            self.textures.insert(key, texture);
            Some(texture)
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_buffer(self.vertex_buffer);
            gl.delete_vertex_array(self.vertex_array);
            for texture in self.textures.values() { gl.delete_texture(*texture); }
            gl.delete_program(self.program);
        }
    }
}

unsafe fn compile_shader(
    gl: &glow::Context,
    kind: u32,
    source: &str,
) -> Result<glow::Shader, String> {
    let shader = unsafe { gl.create_shader(kind).map_err(|e| e.to_string())? };
    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let message = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(message);
        }
    }
    Ok(shader)
}

fn build_grid_vertices() -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let minor = [0.22, 0.25, 0.30];
    let major = [0.31, 0.35, 0.41];

    for i in -20..=20 {
        let n = i as f32;
        let color = if i % 5 == 0 { major } else { minor };
        push_line(&mut vertices, [-20.0, 0.0, n], [20.0, 0.0, n], color);
        push_line(&mut vertices, [n, 0.0, -20.0], [n, 0.0, 20.0], color);
    }

    // World axes: X red, Y green, Z blue.
    push_line(
        &mut vertices,
        [-20.0, 0.002, 0.0],
        [20.0, 0.002, 0.0],
        [0.85, 0.24, 0.22],
    );
    push_line(
        &mut vertices,
        [0.0, -20.0, 0.0],
        [0.0, 20.0, 0.0],
        [0.25, 0.82, 0.42],
    );
    push_line(
        &mut vertices,
        [0.0, 0.002, -20.0],
        [0.0, 0.002, 20.0],
        [0.25, 0.48, 0.95],
    );
    vertices
}

fn upload_vertices(gl: &glow::Context, vertices: &[Vertex]) {
    let bytes = unsafe { std::slice::from_raw_parts(vertices.as_ptr().cast::<u8>(), vertices.len() * std::mem::size_of::<Vertex>()) };
    unsafe { gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW); }
}

fn build_entity_vertices(scene: &Scene, entity_id: EntityId, selected_entity: Option<EntityId>) -> Vec<Vertex> {
    let Some(entity) = scene.get(entity_id) else { return Vec::new(); };
    let Some(points) = entity.mesh_component.as_ref().map(MeshComponent::triangle_vertices).or_else(|| entity.mesh.map(PrimitiveMesh::triangle_vertices)) else { return Vec::new(); };
    let Some(transform) = scene.world_matrix(entity.id) else { return Vec::new(); };
    let color = if selected_entity == Some(entity.id) { [0.98, 0.78, 0.28] }
        else if let Some(material) = entity.material.as_ref() { [material.base_color[0], material.base_color[1], material.base_color[2]] }
        else if entity.mesh_component.as_ref().is_some_and(|component| matches!(component, MeshComponent::Imported(_))) { [0.66, 0.72, 0.82] }
        else { match entity.mesh.unwrap_or(PrimitiveMesh::Cube) {
            PrimitiveMesh::Cube => [0.55, 0.63, 0.78], PrimitiveMesh::Sphere => [0.30, 0.72, 0.68],
            PrimitiveMesh::Plane => [0.72, 0.62, 0.36], PrimitiveMesh::Cylinder => [0.62, 0.48, 0.78], PrimitiveMesh::Capsule => [0.78, 0.48, 0.42],
        }};
    let geometry = entity.mesh_component.as_ref().and_then(|c| match c { MeshComponent::Imported(g) => Some(g), _ => None });
    let material = entity.material.as_ref();
    let mut vertices = Vec::with_capacity(points.len());
    for (corner, point) in points.iter().enumerate() {
        let position = transform.transform_point3(Vec3::from_array(*point) - Vec3::from_array(entity.pivot));
        let local_normal = if let Some(g) = geometry {
            let index = g.indices.get(corner).copied().unwrap_or(corner as u32) as usize;
            g.normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0])
        } else {
            let base = (corner / 3) * 3;
            let a=Vec3::from_array(points[base]); let b=Vec3::from_array(points[base+1]); let c=Vec3::from_array(points[base+2]);
            (b-a).cross(c-a).normalize_or_zero().to_array()
        };
        let normal = transform.inverse().transpose().transform_vector3(Vec3::from_array(local_normal)).normalize_or_zero();
        let uv = geometry.and_then(|g| g.indices.get(corner).and_then(|i| g.uvs.get(*i as usize))).copied().unwrap_or_else(|| primitive_uv(points, corner));
        vertices.push(Vertex { position: position.to_array(), color, normal: normal.to_array(), uv,
            metallic: material.map(|m| m.metallic).unwrap_or(0.0), roughness: material.map(|m| m.roughness).unwrap_or(0.65) });
    }
    vertices
}

fn primitive_uv(points: &[[f32; 3]], corner: usize) -> [f32; 2] {
    let base = (corner / 3) * 3;
    if base + 2 >= points.len() { return [0.0, 0.0]; }
    let a = Vec3::from_array(points[base]);
    let b = Vec3::from_array(points[base + 1]);
    let c = Vec3::from_array(points[base + 2]);
    let n = (b - a).cross(c - a).abs();
    let p = points[corner];
    if n.x >= n.y && n.x >= n.z { [p[2] + 0.5, p[1] + 0.5] }
    else if n.y >= n.z { [p[0] + 0.5, p[2] + 0.5] }
    else { [p[0] + 0.5, p[1] + 0.5] }
}

fn push_line(vertices: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], color: [f32; 3]) {
    vertices.push(Vertex { position: a, color, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], metallic: 0.0, roughness: 1.0 });
    vertices.push(Vertex { position: b, color, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], metallic: 0.0, roughness: 1.0 });
}

fn build_line_vertex_count() -> i32 {
    // 41 grid lines in each direction, plus three world axes.
    (41 * 2 * 2 + 3 * 2) as i32
}

const VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_color;
layout (location = 2) in vec3 a_normal;
layout (location = 3) in vec2 a_uv;
layout (location = 4) in float a_metallic;
layout (location = 5) in float a_roughness;
uniform mat4 u_view_projection;
out vec3 v_color;
out vec3 v_normal;
out vec2 v_uv;
out float v_metallic;
out float v_roughness;
void main() {
    v_color = a_color;
    v_normal = a_normal;
    v_uv = a_uv;
    v_metallic = a_metallic;
    v_roughness = a_roughness;
    gl_Position = u_view_projection * vec4(a_position, 1.0);
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 v_color;
in vec3 v_normal;
in vec2 v_uv;
in float v_metallic;
in float v_roughness;
uniform sampler2D u_base_color_texture;
uniform bool u_has_texture;
uniform sampler2D u_metallic_roughness_texture;
uniform bool u_has_metallic_roughness_texture;
out vec4 out_color;

const float PI = 3.14159265359;

float distribution_ggx(vec3 n, vec3 h, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float ndoth = max(dot(n, h), 0.0);
    float ndoth2 = ndoth * ndoth;
    float denominator = ndoth2 * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denominator * denominator, 0.0001);
}

float geometry_schlick_ggx(float ndotv, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return ndotv / max(ndotv * (1.0 - k) + k, 0.0001);
}

float geometry_smith(vec3 n, vec3 v, vec3 l, float roughness) {
    return geometry_schlick_ggx(max(dot(n, v), 0.0), roughness) *
           geometry_schlick_ggx(max(dot(n, l), 0.0), roughness);
}

vec3 fresnel_schlick(float cos_theta, vec3 f0) {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

void main() {
    vec4 texel = u_has_texture ? texture(u_base_color_texture, v_uv) : vec4(1.0);
    vec3 albedo = v_color * texel.rgb;
    vec4 mr_sample = u_has_metallic_roughness_texture ? texture(u_metallic_roughness_texture, v_uv) : vec4(1.0);
    float metallic = clamp(v_metallic * mr_sample.b, 0.0, 1.0);
    float roughness = clamp(v_roughness * mr_sample.g, 0.045, 1.0);
    vec3 n = normalize(v_normal);
    vec3 v = normalize(vec3(0.0, 2.0, 5.0));
    vec3 l = normalize(vec3(0.45, 0.82, 0.36));
    vec3 h = normalize(v + l);
    vec3 radiance = vec3(3.0, 2.95, 2.8);

    float ndotv = max(dot(n, v), 0.0);
    float ndotl = max(dot(n, l), 0.0);
    vec3 f0 = mix(vec3(0.04), albedo, metallic);
    vec3 f = fresnel_schlick(max(dot(h, v), 0.0), f0);
    float d = distribution_ggx(n, h, roughness);
    float g = geometry_smith(n, v, l, roughness);
    vec3 specular = (d * g * f) / max(4.0 * ndotv * ndotl, 0.001);
    vec3 kd = (vec3(1.0) - f) * (1.0 - metallic);
    vec3 direct = (kd * albedo / PI + specular) * radiance * ndotl;
    vec3 ambient = vec3(0.035) * albedo;
    vec3 color = ambient + direct;
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0 / 2.2));
    out_color = vec4(color, texel.a);
}
"#;
