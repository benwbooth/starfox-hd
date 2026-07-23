//! Additive-blended billboard particles.
//!
//! Port (C oracle): `src/renderer/particles.c`.

use crate::gpu::{Gpu, Vertex3};
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
    /// Unit quad centred at origin (XY plane), the GL_TRIANGLE_FAN expanded
    /// into a 6-vertex triangle list for the retained flat pipeline.
    quad_tris: [Vertex3; 6],
}

impl Particles {
    pub fn new(_gpu: &mut Gpu) -> Self {
        // Unit quad centred at origin, in XY plane. Fan order was
        // (v0,v1,v2,v3); triangulate as (v0,v1,v2) + (v0,v2,v3).
        let v0 = Vertex3 {
            pos: [-0.5, -0.5, 0.0],
        };
        let v1 = Vertex3 {
            pos: [0.5, -0.5, 0.0],
        };
        let v2 = Vertex3 {
            pos: [0.5, 0.5, 0.0],
        };
        let v3 = Vertex3 {
            pos: [-0.5, 0.5, 0.0],
        };
        Particles {
            particles: [Particle::default(); MAX_PARTICLES],
            quad_tris: [v0, v1, v2, v0, v2, v3],
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        vx: f32,
        vy: f32,
        vz: f32,
        life: f32,
        ptype: u8,
    ) {
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

    /// Mirror of `Particles_Render`. NOTE: the old pass was additive-blended
    /// and depth-write-disabled; the retained flat pipeline offers neither, so
    /// particles now draw as opaque depth-writing quads (see parity note).
    pub fn render(&self, gpu: &mut Gpu, transform: &Transform) {
        if !self.particles.iter().any(|p| p.active) {
            return;
        }

        let proj = *transform.projection();
        let view = *transform.view();

        for p in &self.particles {
            if !p.active {
                continue;
            }
            let alpha = if p.max_life > 0.0 {
                p.life / p.max_life
            } else {
                0.0
            };
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
            // Additive, non-depth-writing (was GL SRC_ALPHA/ONE + depth mask
            // off) so particles glow and layer instead of occluding.
            gpu.push_flat_tris_add(&self.quad_tris, &proj, &view, &model, [r, g, b, alpha]);
        }
    }
}
