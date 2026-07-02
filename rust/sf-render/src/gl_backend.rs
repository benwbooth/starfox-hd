//! GL program management + streaming line VBO.
//!
//! Ported from the original C reference (`renderer/gl_backend.c`). The flat
//! shader can be loaded from a caller-supplied shader directory
//! (`flat.vert.glsl` / `flat.frag.glsl`, the canonical GLSL shipped in
//! `rust/sf-render/shaders/`); when absent the embedded sources — byte-equal
//! to those files — are used. The HUD shader is embedded only.

use glow::HasContext;
use std::path::Path;

/// Embedded flat shader — byte-equal to `rust/sf-render/shaders/flat.vert.glsl`.
pub const FLAT_VERT_SRC: &str = "#version 330 core\n\
layout(location = 0) in vec3 aPos;\n\
uniform mat4 uModel;\n\
uniform mat4 uView;\n\
uniform mat4 uProj;\n\
void main() {\n\
    gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);\n\
}\n";

pub const FLAT_FRAG_SRC: &str = "#version 330 core\n\
uniform vec4 uColor;\n\
out vec4 FragColor;\n\
void main() {\n\
    FragColor = uColor;\n\
}\n";

/// HUD shader (2D overlay), identical to `hud_vert_src`/`hud_frag_src`.
pub const HUD_VERT_SRC: &str = "#version 330 core\n\
layout(location = 0) in vec2 aPos;\n\
layout(location = 1) in vec2 aTexCoord;\n\
uniform mat4 uProj;\n\
uniform mat4 uModel;\n\
out vec2 vTexCoord;\n\
void main() {\n\
    gl_Position = uProj * uModel * vec4(aPos, 0.0, 1.0);\n\
    vTexCoord = aTexCoord;\n\
}\n";

pub const HUD_FRAG_SRC: &str = "#version 330 core\n\
uniform vec4 uColor;\n\
uniform sampler2D uTexture;\n\
uniform int uUseTexture;\n\
uniform vec4 uPalette[16];\n\
in vec2 vTexCoord;\n\
out vec4 FragColor;\n\
void main() {\n\
    if (uUseTexture == 2) {\n\
        float idx_f = texture(uTexture, vTexCoord).r;\n\
        int idx = int(idx_f * 255.0 + 0.5);\n\
        if (idx == 0) discard;\n\
        FragColor = uPalette[idx];\n\
    } else if (uUseTexture == 1) {\n\
        vec4 texel = texture(uTexture, vTexCoord);\n\
        if (texel.a < 0.5) discard;\n\
        FragColor = texel * uColor;\n\
    } else {\n\
        FragColor = uColor;\n\
    }\n\
}\n";

pub fn compile_shader(
    gl: &glow::Context,
    source: &str,
    shader_type: u32,
) -> Result<glow::Shader, String> {
    unsafe {
        let shader = gl.create_shader(shader_type)?;
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("Shader compile error: {log}"));
        }
        Ok(shader)
    }
}

pub fn link_program(
    gl: &glow::Context,
    vert: glow::Shader,
    frag: glow::Shader,
) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        gl.attach_shader(program, vert);
        gl.attach_shader(program, frag);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("Shader link error: {log}"));
        }
        Ok(program)
    }
}

pub fn build_program(
    gl: &glow::Context,
    vert_src: &str,
    frag_src: &str,
) -> Result<glow::Program, String> {
    let vert = compile_shader(gl, vert_src, glow::VERTEX_SHADER)?;
    let frag = match compile_shader(gl, frag_src, glow::FRAGMENT_SHADER) {
        Ok(f) => f,
        Err(e) => {
            unsafe { gl.delete_shader(vert) };
            return Err(e);
        }
    };
    let program = link_program(gl, vert, frag);
    unsafe {
        gl.delete_shader(vert);
        gl.delete_shader(frag);
    }
    program
}

// --- Uniform helpers (GlBackend_SetMat4 / SetVec3 / SetVec4 / SetFloat) ----

pub fn set_mat4(gl: &glow::Context, program: glow::Program, name: &str, mat: &[f32; 16]) {
    unsafe {
        if let Some(loc) = gl.get_uniform_location(program, name) {
            gl.uniform_matrix_4_f32_slice(Some(&loc), false, mat);
        }
    }
}

pub fn set_vec3(gl: &glow::Context, program: glow::Program, name: &str, x: f32, y: f32, z: f32) {
    unsafe {
        if let Some(loc) = gl.get_uniform_location(program, name) {
            gl.uniform_3_f32(Some(&loc), x, y, z);
        }
    }
}

pub fn set_vec4(
    gl: &glow::Context,
    program: glow::Program,
    name: &str,
    x: f32,
    y: f32,
    z: f32,
    w: f32,
) {
    unsafe {
        if let Some(loc) = gl.get_uniform_location(program, name) {
            gl.uniform_4_f32(Some(&loc), x, y, z, w);
        }
    }
}

pub fn set_float(gl: &glow::Context, program: glow::Program, name: &str, val: f32) {
    unsafe {
        if let Some(loc) = gl.get_uniform_location(program, name) {
            gl.uniform_1_f32(Some(&loc), val);
        }
    }
}

pub fn set_int(gl: &glow::Context, program: glow::Program, name: &str, val: i32) {
    unsafe {
        if let Some(loc) = gl.get_uniform_location(program, name) {
            gl.uniform_1_i32(Some(&loc), val);
        }
    }
}

/// GL programs + streaming line buffer shared across passes
/// (`g_flat_shader` / `g_hud_shader` in the C).
pub struct GlBackend {
    pub flat: glow::Program,
    pub hud: glow::Program,
    line_vao: Option<glow::VertexArray>,
    line_vbo: Option<glow::Buffer>,
    line_capacity: usize, // bytes
}

impl GlBackend {
    /// `shader_dir`: optional directory containing `flat.vert.glsl` /
    /// `flat.frag.glsl` (canonical copies live in `rust/sf-render/shaders/`);
    /// falls back to the embedded sources when missing.
    pub fn new(gl: &glow::Context, shader_dir: Option<&Path>) -> Result<Self, String> {
        let (flat_vert, flat_frag) = match shader_dir {
            Some(dir) => {
                let v = std::fs::read_to_string(dir.join("flat.vert.glsl"));
                let f = std::fs::read_to_string(dir.join("flat.frag.glsl"));
                match (v, f) {
                    (Ok(v), Ok(f)) => (v, f),
                    _ => (FLAT_VERT_SRC.to_string(), FLAT_FRAG_SRC.to_string()),
                }
            }
            None => (FLAT_VERT_SRC.to_string(), FLAT_FRAG_SRC.to_string()),
        };

        let flat = build_program(gl, &flat_vert, &flat_frag)?;
        let hud = build_program(gl, HUD_VERT_SRC, HUD_FRAG_SRC)?;
        Ok(GlBackend {
            flat,
            hud,
            line_vao: None,
            line_vbo: None,
            line_capacity: 0,
        })
    }

    /// Mirror of `GlBackend_DrawLines`: stream `positions` (3 floats per
    /// vertex) into the shared line VBO and draw as GL_LINES.
    pub fn draw_lines(&mut self, gl: &glow::Context, positions: &[f32]) {
        let mut vertex_count = positions.len() / 3;
        if vertex_count < 2 {
            return;
        }
        vertex_count &= !1; // GL_LINES consumes pairs

        unsafe {
            if self.line_vao.is_none() {
                let vao = gl.create_vertex_array().ok();
                let vbo = gl.create_buffer().ok();
                self.line_vao = vao;
                self.line_vbo = vbo;
                gl.bind_vertex_array(vao);
                gl.bind_buffer(glow::ARRAY_BUFFER, vbo);
                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 3 * 4, 0);
                gl.enable_vertex_attrib_array(0);
            } else {
                gl.bind_vertex_array(self.line_vao);
                gl.bind_buffer(glow::ARRAY_BUFFER, self.line_vbo);
            }

            let bytes: &[u8] = core::slice::from_raw_parts(
                positions.as_ptr() as *const u8,
                vertex_count * 3 * 4,
            );
            if bytes.len() > self.line_capacity {
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
                self.line_capacity = bytes.len();
            } else {
                // Orphan-then-fill keeps the driver from stalling on reuse.
                gl.buffer_data_size(
                    glow::ARRAY_BUFFER,
                    self.line_capacity as i32,
                    glow::STREAM_DRAW,
                );
                gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
            }

            gl.draw_arrays(glow::LINES, 0, vertex_count as i32);
            gl.bind_vertex_array(None);
        }
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.flat);
            gl.delete_program(self.hud);
            if let Some(vao) = self.line_vao.take() {
                gl.delete_vertex_array(vao);
            }
            if let Some(vbo) = self.line_vbo.take() {
                gl.delete_buffer(vbo);
            }
            self.line_capacity = 0;
        }
    }
}
