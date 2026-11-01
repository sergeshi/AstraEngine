#![allow(unsafe_code)]

use eframe::glow::{self, HasContext};
use astra_scene::{EntityId, PrimitiveMesh, Scene};
use glam::{EulerRot, Mat4, Quat, Vec3};
use crate::viewport_camera;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
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
    push_line(&mut vertices, [-20.0, 0.002, 0.0], [20.0, 0.002, 0.0], [0.85, 0.24, 0.22]);
    push_line(&mut vertices, [0.0, -20.0, 0.0], [0.0, 20.0, 0.0], [0.25, 0.82, 0.42]);
    push_line(&mut vertices, [0.0, 0.002, -20.0], [0.0, 0.002, 20.0], [0.25, 0.48, 0.95]);
    vertices
}

fn build_scene_vertices(scene: &Scene, selected_entity: Option<EntityId>) -> Vec<Vertex> {
    let mut vertices = build_grid_vertices();
    let cube_faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.78, 0.31, 0.27], [[-1.0,-1.0, 1.0],[1.0,-1.0, 1.0],[1.0,1.0,1.0],[-1.0,1.0,1.0]]),
        ([0.30, 0.66, 0.39], [[1.0,-1.0,-1.0],[-1.0,-1.0,-1.0],[-1.0,1.0,-1.0],[1.0,1.0,-1.0]]),
        ([0.28, 0.48, 0.82], [[-1.0,-1.0,-1.0],[-1.0,-1.0,1.0],[-1.0,1.0,1.0],[-1.0,1.0,-1.0]]),
        ([0.83, 0.68, 0.28], [[1.0,-1.0,1.0],[1.0,-1.0,-1.0],[1.0,1.0,-1.0],[1.0,1.0,1.0]]),
        ([0.65, 0.40, 0.78], [[-1.0,1.0,1.0],[1.0,1.0,1.0],[1.0,1.0,-1.0],[-1.0,1.0,-1.0]]),
        ([0.30, 0.68, 0.73], [[-1.0,-1.0,-1.0],[1.0,-1.0,-1.0],[1.0,-1.0,1.0],[-1.0,-1.0,1.0]]),
    ];

    for entity in scene.iter() {
        if entity.mesh != Some(PrimitiveMesh::Cube) {
            continue;
        }
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from_array(entity.transform.scale),
            Quat::from_euler(
                EulerRot::XYZ,
                entity.transform.rotation[0],
                entity.transform.rotation[1],
                entity.transform.rotation[2],
            ),
            Vec3::from_array(entity.transform.translation),
        );
        let selected = selected_entity == Some(entity.id);
        for (face_color, corners) in cube_faces {
            let color = if selected { [0.98, 0.78, 0.28] } else { face_color };
            for index in [0, 1, 2, 0, 2, 3] {
                let position = transform.transform_point3(Vec3::from_array(corners[index]));
                vertices.push(Vertex { position: position.to_array(), color });
            }
        }
    }
    vertices
}

fn push_line(vertices: &mut Vec<Vertex>, a: [f32; 3], b: [f32; 3], color: [f32; 3]) {
    vertices.push(Vertex { position: a, color });
    vertices.push(Vertex { position: b, color });
}

fn build_line_vertex_count() -> i32 {
    // 41 grid lines in each direction, plus three world axes.
    (41 * 2 * 2 + 3 * 2) as i32
}

const VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec3 a_position;
layout (location = 1) in vec3 a_color;
uniform mat4 u_view_projection;
out vec3 v_color;
void main() {
    v_color = a_color;
    gl_Position = u_view_projection * vec4(a_position, 1.0);
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 v_color;
out vec4 out_color;
void main() {
    out_color = vec4(v_color, 1.0);
}
"#;
