//! 3D draw-list pass with obj_id-keyed interpolation and drop shadows.
//!
//! Port (C oracle): `src/renderer/draw_list.c` plus the `DrawListEntry`
//! struct and `DL_FLAG_*` constants from `src/types.h`.

use glow::HasContext;

use crate::gl_backend::{self, GlBackend};
use crate::shapes_gl::ShapeStore;
use crate::transform::Transform;

pub const MAX_OBJECTS: usize = 128;
pub const MAX_DRAW_LIST: usize = 128;

pub const DL_FLAG_VISIBLE: u8 = 0x01;
pub const DL_FLAG_SHADOW: u8 = 0x02;
pub const DL_FLAG_HIGHLIGHT: u8 = 0x04;
pub const DL_FLAG_WIREFRAME: u8 = 0x08;

/// Draw list entry — the bridge between game logic and renderer
/// (STRUCTS.INC `dl_` structure; wider types like the C port).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawListEntry {
    /// World position (fixed-point 16.16).
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Rotation angles (SNES 0-255 units, stored as i16).
    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
    pub shape_id: u16,
    pub color_table: u16,
    pub sort_z: i16,
    pub sflags: u8,
    pub explosion_cnt: u8,
    pub anim_frame: u8,
    pub col_frame: u8,
    pub depth_offset: u8,
    pub flags: u8,
    pub shad_x: i16,
    pub shad_y: i16,
    pub shad_z: i16,
    pub tscroll_x: u8,
    pub tscroll_y: u8,
    /// Stable source-object id (alien index + 1); 0 = no identity.
    pub obj_id: u16,
}

// Scratch buffer size for wireframe edge extraction (DL_MAX_LINE_VERTS).
const DL_MAX_LINE_VERTS: usize = 4096;

fn lerp_angle8(from: i16, to: i16, t: f32) -> i16 {
    let a8 = from & 0xFF;
    let b8 = to & 0xFF;
    let mut diff = b8 - a8;
    if diff > 127 {
        diff -= 256;
    }
    if diff < -128 {
        diff += 256;
    }
    let out8 = a8 + (diff as f32 * t) as i16;
    out8 & 0xFF
}

/// Mirror of `InterpolateEntry`.
fn interpolate_entry(a: &DrawListEntry, b: &DrawListEntry, alpha: f32) -> DrawListEntry {
    let mut out = *b;
    out.x = (a.x as f32 + (b.x as f32 - a.x as f32) * alpha) as i32;
    out.y = (a.y as f32 + (b.y as f32 - a.y as f32) * alpha) as i32;
    out.z = (a.z as f32 + (b.z as f32 - a.z as f32) * alpha) as i32;
    out.rx = lerp_angle8(a.rx, b.rx, alpha);
    out.ry = lerp_angle8(a.ry, b.ry, alpha);
    out.rz = lerp_angle8(a.rz, b.rz, alpha);
    out
}

pub struct DrawListRenderer {
    line_scratch: Vec<f32>,
}

impl DrawListRenderer {
    pub fn new() -> Self {
        DrawListRenderer {
            line_scratch: Vec::with_capacity(DL_MAX_LINE_VERTS * 3),
        }
    }

    /// Mirror of `RenderWireframeEntry`: draw a DL_FLAG_WIREFRAME entry as a
    /// single-color edge wireframe (flat light grey stand-in).
    fn render_wireframe_entry(
        &mut self,
        gl: &glow::Context,
        backend: &mut GlBackend,
        shapes: &ShapeStore,
        shape_id: u16,
        model: &[f32; 16],
    ) {
        let Some(shape) = shapes.get(shape_id) else {
            return;
        };

        self.line_scratch.clear();
        let mut vert_count = 0usize;
        'faces: for face in &shape.faces {
            if face.num_verts < 2 {
                continue;
            }
            let nv = face.num_verts as usize;
            let edge_count = if nv == 2 { 1 } else { nv };
            for e in 0..edge_count {
                let a = face.vertex_indices[e] as usize;
                let b = face.vertex_indices[(e + 1) % nv] as usize;
                if a >= shape.vertices.len() || b >= shape.vertices.len() {
                    continue;
                }
                if vert_count + 2 > DL_MAX_LINE_VERTS {
                    break 'faces;
                }
                let va = &shape.vertices[a];
                let vb = &shape.vertices[b];
                self.line_scratch
                    .extend_from_slice(&[va.x, va.y, va.z, vb.x, vb.y, vb.z]);
                vert_count += 2;
            }
        }

        if vert_count == 0 {
            return;
        }

        gl_backend::set_mat4(gl, backend.flat, "uModel", model);
        gl_backend::set_vec4(gl, backend.flat, "uColor", 0.75, 0.78, 0.82, 1.0);
        let scratch = std::mem::take(&mut self.line_scratch);
        backend.draw_lines(gl, &scratch);
        self.line_scratch = scratch;
    }

    /// Mirror of `RenderShadow` (MARIO/MDRAWLIS.MC shadow pass): project the
    /// object's own mesh onto the ground plane by flattening the world-space
    /// Y row of the model matrix.
    fn render_shadow(
        &self,
        gl: &glow::Context,
        backend: &GlBackend,
        shapes: &ShapeStore,
        transform: &Transform,
        e: &DrawListEntry,
    ) {
        let Some(shape) = shapes.get(e.shape_id) else {
            return;
        };
        if shape.num_triangles <= 0 {
            return;
        }
        if e.y > 0 {
            return; // below ground (SNES +Y is down): no shadow
        }

        let mut model = [0.0f32; 16];
        // Ground height 0 (BGS.ASM shadowheight for planet levels).
        transform.build_model_matrix(&mut model, e.x, 0, e.z, e.rx, e.ry, e.rz);
        // Flatten world-space Y (row 1 of the column-major matrix); lift
        // slightly to avoid coplanar artifacts.
        model[1] = 0.0;
        model[5] = 0.0;
        model[9] = 0.0;
        model[13] = 0.5;

        gl_backend::set_mat4(gl, backend.flat, "uModel", &model);
        gl_backend::set_vec4(gl, backend.flat, "uColor", 0.0, 0.0, 0.0, 0.40);
        unsafe {
            gl.bind_vertex_array(Some(shape.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, shape.num_triangles * 3);
            gl.bind_vertex_array(None);
        }
    }

    /// Mirror of `DrawList_Render`.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        gl: &glow::Context,
        backend: &mut GlBackend,
        shapes: &ShapeStore,
        transform: &mut Transform,
        prev: &[DrawListEntry],
        curr: &[DrawListEntry],
        alpha: f32,
    ) {
        if curr.is_empty() {
            return;
        }

        // The game tick is 20Hz but rendering is uncapped: rebuild the view
        // matrix from the interpolated camera so the camera glides with the
        // interpolated objects instead of stepping once per tick.
        transform.set_view_lerp(alpha);

        unsafe {
            gl.use_program(Some(backend.flat));
        }
        gl_backend::set_mat4(gl, backend.flat, "uView", transform.view());
        gl_backend::set_mat4(gl, backend.flat, "uProj", transform.projection());

        // Interpolated entries that want a drop shadow (collected during the
        // main pass, drawn afterwards as a translucent overlay).
        let mut shadow_list: Vec<DrawListEntry> = Vec::new();

        // Pair current entries with the previous frame's entries by stable
        // object id (alien index), not by list position.
        let mut prev_by_id = [-1i16; MAX_OBJECTS + 1];
        for (i, p) in prev.iter().enumerate() {
            if (p.obj_id as usize) <= MAX_OBJECTS {
                prev_by_id[p.obj_id as usize] = i as i16;
            }
        }

        for entry in curr {
            if entry.flags & DL_FLAG_VISIBLE == 0 {
                continue;
            }

            // Interpolate if we have a matching previous entry.
            let prev_idx = if entry.obj_id != 0 && (entry.obj_id as usize) <= MAX_OBJECTS {
                prev_by_id[entry.obj_id as usize]
            } else {
                -1
            };
            let interp = if prev_idx >= 0 && prev[prev_idx as usize].shape_id == entry.shape_id
            {
                interpolate_entry(&prev[prev_idx as usize], entry, alpha)
            } else {
                *entry
            };

            // Build model matrix.
            let mut model = [0.0f32; 16];
            transform.build_model_matrix(
                &mut model, interp.x, interp.y, interp.z, interp.rx, interp.ry, interp.rz,
            );

            // Render the shape. Wireframe-flagged entries draw as edge lines
            // instead of filled triangles.
            if interp.flags & DL_FLAG_WIREFRAME != 0 {
                self.render_wireframe_entry(gl, backend, shapes, interp.shape_id, &model);
            } else {
                shapes.render(
                    gl,
                    backend,
                    transform,
                    interp.shape_id,
                    interp.anim_frame,
                    interp.col_frame,
                    interp.color_table,
                    interp.explosion_cnt,
                    &model,
                );
            }

            // Queue the drop shadow (skip exploding objects).
            if interp.flags & DL_FLAG_SHADOW != 0
                && interp.explosion_cnt == 0
                && shadow_list.len() < MAX_DRAW_LIST
            {
                shadow_list.push(interp);
            }
        }

        // --- Shadow pass (after the opaque pass so depth testing hides
        // shadow fragments behind solid geometry) ---
        if !shadow_list.is_empty() {
            unsafe {
                gl.enable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                gl.depth_mask(false);
            }
            for e in &shadow_list {
                self.render_shadow(gl, backend, shapes, transform, e);
            }
            unsafe {
                gl.depth_mask(true);
                gl.disable(glow::BLEND);
            }
        }
    }
}

impl Default for DrawListRenderer {
    fn default() -> Self {
        Self::new()
    }
}
