#![allow(unsafe_code)]

use crate::viewport_camera;
use astra_scene::{EntityId, MeshComponent, PrimitiveMesh, Scene};
use eframe::glow::{self, HasContext};
use glam::Vec3;

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

            let view_projection = gl
                .get_uniform_location(program, "u_view_projection")
                .ok_or_else(|| "Missing u_view_projection uniform".to_owned())?;

            Ok(Self {
                program,
                vertex_array,
                vertex_buffer,
                view_projection,
                line_vertices: build_line_vertex_count(),
            })
        }
    }

    pub fn paint(
        &self,
        gl: &glow::Context,
        width: i32,
        height: i32,
        yaw: f32,
        pitch: f32,
        distance: f32,
        target: [f32; 3],
        scene: &Scene,
        selected_entity: Option<EntityId>,
    ) {
        let vertices = build_scene_vertices(scene, selected_entity);
        let total_vertices = vertices.len() as i32;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                vertices.len() * std::mem::size_of::<Vertex>(),
            )
        };
        let matrix = viewport_camera::view_projection(
            width,
            height,
            yaw,
            pitch,
            distance,
            Vec3::from_array(target),
        );

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.clear_color(0.105, 0.125, 0.16, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                Some(&self.view_projection),
                false,
                &matrix.to_cols_array(),
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.line_width(1.0);
            gl.draw_arrays(glow::LINES, 0, self.line_vertices);
            gl.draw_arrays(
                glow::TRIANGLES,
                self.line_vertices,
                total_vertices - self.line_vertices,
            );
            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_buffer(self.vertex_buffer);
            gl.delete_vertex_array(self.vertex_array);
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

fn build_scene_vertices(scene: &Scene, selected_entity: Option<EntityId>) -> Vec<Vertex> {
    let mut vertices = build_grid_vertices();
    for entity in scene.iter() {
        let Some(points) = entity.mesh_component.as_ref().map(MeshComponent::triangle_vertices)
            .or_else(|| entity.mesh.map(PrimitiveMesh::triangle_vertices)) else { continue; };
        let Some(transform) = scene.world_matrix(entity.id) else { continue; };
        let color = if selected_entity == Some(entity.id) {
            [0.98, 0.78, 0.28]
        } else if let Some(material) = entity.material.as_ref() {
            [material.base_color[0], material.base_color[1], material.base_color[2]]
        } else if entity.mesh_component.as_ref().is_some_and(|component| matches!(component, MeshComponent::Imported(_))) {
            [0.66, 0.72, 0.82]
        } else {
            match entity.mesh.unwrap_or(PrimitiveMesh::Cube) {
                PrimitiveMesh::Cube => [0.55, 0.63, 0.78],
                PrimitiveMesh::Sphere => [0.30, 0.72, 0.68],
                PrimitiveMesh::Plane => [0.72, 0.62, 0.36],
                PrimitiveMesh::Cylinder => [0.62, 0.48, 0.78],
                PrimitiveMesh::Capsule => [0.78, 0.48, 0.42],
            }
        };
        let geometry_data = entity.mesh_component.as_ref().and_then(|component| match component { MeshComponent::Imported(g) => Some(g), _ => None });
        let material = entity.material.as_ref();
        for (corner, point) in points.iter().enumerate() {
            let pivot = Vec3::from_array(entity.pivot);
            let position = transform.transform_point3(Vec3::from_array(*point) - pivot);
            let local_normal = if let Some(g) = geometry_data {
                let index = g.indices.get(corner).copied().unwrap_or(corner as u32) as usize;
                g.normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0])
            } else {
                let a = Vec3::from_array(points[(corner / 3) * 3]);
                let b = Vec3::from_array(points[(corner / 3) * 3 + 1]);
                let c = Vec3::from_array(points[(corner / 3) * 3 + 2]);
                (b-a).cross(c-a).normalize_or_zero().to_array()
            };
            let normal = transform.inverse().transpose().transform_vector3(Vec3::from_array(local_normal)).normalize_or_zero();
            let uv = geometry_data.and_then(|g| g.indices.get(corner).and_then(|i| g.uvs.get(*i as usize))).copied().unwrap_or([0.0, 0.0]);
            vertices.push(Vertex { position: position.to_array(), color, normal: normal.to_array(), uv,
                metallic: material.map(|m| m.metallic).unwrap_or(0.0),
                roughness: material.map(|m| m.roughness).unwrap_or(0.65) });
        }
    }
    vertices
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
out vec4 out_color;
void main() {
    vec3 n = normalize(v_normal);
    vec3 l = normalize(vec3(0.45, 0.82, 0.36));
    vec3 v = normalize(vec3(0.0, 2.0, 5.0));
    vec3 h = normalize(l + v);
    float ndotl = max(dot(n, l), 0.0);
    float exponent = mix(128.0, 3.0, clamp(v_roughness, 0.0, 1.0));
    float specular = pow(max(dot(n, h), 0.0), exponent);
    vec3 f0 = mix(vec3(0.04), v_color, clamp(v_metallic, 0.0, 1.0));
    vec3 diffuse = v_color * (1.0 - clamp(v_metallic, 0.0, 1.0)) * (0.24 + 0.76 * ndotl);
    vec3 color = diffuse + f0 * specular * (1.0 - 0.65 * v_roughness);
    out_color = vec4(color, 1.0);
}
"#;
