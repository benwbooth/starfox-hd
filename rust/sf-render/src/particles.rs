//! Additive-blended billboard particles.
//!
//! Port (C oracle): `src/renderer/particles.c`.

use glow::HasContext;

use crate::gl_backend::{self, GlBackend};
use crate::transform::Transform;

pub const MAX_PARTICLES: usize = 256;

#[derive(Debug, Clone, Copy, Default)]
struct Particle {
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    life: f32,
    max_life: f32,
    ptype: u8,
    active: bool,
}

/// Build a translation + uniform-scale model matrix (column-major 4x4).
fn build_particle_model(x: f32, y: f32, z: f32, scale: f32) -> [f32; 16] {
    let mut m = [0.0f32; 16];
    m[0] = scale;
    m[5] = scale;
    m[10] = scale;
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m[15] = 1.0;
    m
}

pub struct Particles {
    particles: [Particle; MAX_PARTICLES],
    quad_vao: Option<glow::VertexArray>,
    quad_vbo: Option<glow::Buffer>,
}

impl Particles {
    pub fn new(gl: &glow::Context) -> Self {
        // Unit quad centred at origin, in XY plane (GL_TRIANGLE_FAN order).
        let quad_verts: [f32; 12] = [
            -0.5, -0.5, 0.0,
            0.5, -0.5, 0.0,
            0.5, 0.5, 0.0,
            -0.5, 0.5, 0.0,
        ];
        let (vao, vbo) = unsafe {
            let vao = gl.create_vertex_array().ok();
            let vbo = gl.create_buffer().ok();
            gl.bind_vertex_array(vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, vbo);
            let bytes: &[u8] = core::slice::from_raw_parts(
                quad_verts.as_ptr() as *const u8,
                quad_verts.len() * 4,
            );
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 3 * 4, 0);
            gl.bind_vertex_array(None);
            (vao, vbo)
        };
        Particles {
            particles: [Particle::default(); MAX_PARTICLES],
            quad_vao: vao,
            quad_vbo: vbo,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(&mut self, x: f32, y: f32, z: f32, vx: f32, vy: f32, vz: f32, life: f32, ptype: u8) {
        for p in self.particles.iter_mut() {
            if !p.active {
                *p = Particle {
                    x,
                    y,
                    z,
                    vx,
                    vy,
                    vz,
                    life,
                    max_life: life,
                    ptype,
                    active: true,
                };
                return;
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for p in self.particles.iter_mut() {
            if !p.active {
                continue;
            }
            p.life -= dt;
            if p.life <= 0.0 {
                p.active = false;
                continue;
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.z += p.vz * dt;
        }
    }

    /// Mirror of `Particles_Render`.
    pub fn render(&self, gl: &glow::Context, backend: &GlBackend, transform: &Transform) {
        if !self.particles.iter().any(|p| p.active) {
            return;
        }

        unsafe {
            gl.use_program(Some(backend.flat));
        }
        gl_backend::set_mat4(gl, backend.flat, "uView", transform.view());
        gl_backend::set_mat4(gl, backend.flat, "uProj", transform.projection());

        // Particles are additive-blended, depth-tested but not depth-written
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE);
            gl.depth_mask(false);
            gl.bind_vertex_array(self.quad_vao);
        }

        for p in &self.particles {
            if !p.active {
                continue;
            }
            let alpha = if p.max_life > 0.0 { p.life / p.max_life } else { 0.0 };
            if alpha <= 0.0 {
                continue;
            }

            // Colour by type
            let (r, g, b) = match p.ptype {
                1 => (1.0, 1.0, 0.2), // spark (yellow)
                2 => (1.0, 1.0, 1.0), // flash (white)
                3 => (0.3, 0.5, 1.0), // energy (blue)
                _ => (1.0, 0.4, 0.1), // fire (orange/red)
            };

            let model = build_particle_model(p.x, p.y, p.z, 2.0);
            gl_backend::set_mat4(gl, backend.flat, "uModel", &model);
            gl_backend::set_vec4(gl, backend.flat, "uColor", r, g, b, alpha);
            unsafe {
                gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
            }
        }

        unsafe {
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
            gl.disable(glow::BLEND);
        }
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(b) = self.quad_vbo.take() {
                gl.delete_buffer(b);
            }
            if let Some(v) = self.quad_vao.take() {
                gl.delete_vertex_array(v);
            }
        }
        self.particles = [Particle::default(); MAX_PARTICLES];
    }
}
