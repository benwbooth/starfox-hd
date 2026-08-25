//! 3D draw-list pass with obj_id-keyed interpolation and drop shadows.
//!
//! Port (C oracle): `src/renderer/draw_list.c` plus the `DrawListEntry`
//! struct and `DL_FLAG_*` constants from `src/types.h`.

use crate::font::Font;
use crate::gpu::{Gpu, TextureId};
use crate::shapes_gl::ShapeStore;
use crate::source_projection::SourcePose;
use crate::source_raster::{SourceBitmapRect, SourceRaster};
use crate::transform::Transform;

pub const MAX_OBJECTS: usize = 128;
pub const MAX_DRAW_LIST: usize = 128;

pub const DL_FLAG_VISIBLE: u8 = 0x01;
pub const DL_FLAG_SHADOW: u8 = 0x02;
pub const DL_FLAG_HIGHLIGHT: u8 = 0x04;
pub const DL_FLAG_TEXT: u8 = 0x10;
pub const DL_FLAG_SCALED_SPRITE: u8 = 0x20;

/// Typed camera used to project a completed source-resolution scene when its
/// bitmap is presented under a later display state. The normal application
/// leaves this unset and uses the renderer's current camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSceneCamera {
    pub position: [i32; 3],
    pub rotation: [u16; 3],
}

/// Project the model origin through the column-major GPU matrices. Returns
/// NDC x/y and positive camera depth (`clip.w`) for an in-front point.
fn project_model_origin(
    proj: &[f32; 16],
    view: &[f32; 16],
    model: &[f32; 16],
) -> Option<(f32, f32, f32)> {
    let mut pv = [0.0f32; 16];
    crate::transform::multiply(&mut pv, proj, view);
    let mut pvm = [0.0f32; 16];
    crate::transform::multiply(&mut pvm, &pv, model);
    let w = pvm[15];
    if !w.is_finite() || w <= 0.0 {
        return None;
    }
    let x = pvm[12] / w;
    let y = pvm[13] / w;
    if x.is_finite() && y.is_finite() {
        Some((x, y, w))
    } else {
        None
    }
}

/// Project one stable draw-list object identity for an object-anchored
/// presentation effect. Position interpolation follows the ordinary object
/// pass, so the overlay remains attached between 20 Hz game ticks.
pub(crate) fn project_draw_object_origin(
    transform: &Transform,
    prev: &[DrawListEntry],
    curr: &[DrawListEntry],
    object_id: u16,
    alpha: f32,
) -> Option<(f32, f32)> {
    if object_id == 0 {
        return None;
    }
    let current = curr.iter().find(|entry| entry.obj_id == object_id)?;
    let entry = prev
        .iter()
        .find(|entry| entry.obj_id == object_id && entry.shape_id == current.shape_id)
        .map_or(*current, |previous| {
            interpolate_entry(previous, current, alpha)
        });

    project_world_origin(transform, entry.x, entry.y, entry.z)
}

/// Project one flat FP16.16 world position through the current interpolated
/// camera.
pub(crate) fn project_world_origin(
    transform: &Transform,
    x: i32,
    y: i32,
    z: i32,
) -> Option<(f32, f32)> {
    let mut model = [0.0f32; 16];
    transform.build_model_matrix(&mut model, x, y, z, 0, 0, 0);
    project_model_origin(transform.projection(), transform.view(), &model).map(|(x, y, _)| (x, y))
}

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

fn source_sort_depth(entry: &DrawListEntry, camera: SourceSceneCamera) -> i16 {
    let view_matrix = sf_core::snes_trig::zxy_matrix_q15_fine(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
    );
    let world_position = [
        (entry.x >> 16) as i16,
        (entry.y >> 16) as i16,
        (entry.z >> 16) as i16,
    ];
    let camera_position = camera
        .position
        .map(|coordinate| (coordinate >> 16) as i16);
    let relative = [
        world_position[0].wrapping_sub(camera_position[0]),
        world_position[1].wrapping_sub(camera_position[1]),
        world_position[2].wrapping_sub(camera_position[2]),
    ];
    let view_position = sf_core::snes_trig::matrix_rotate_q15(
        view_matrix,
        relative[0],
        relative[1],
        relative[2],
    );
    let shape_sort_depth = sf_core::sf1_shape_metrics::sf1_shape_metrics(entry.shape_id)
        .map_or(0, |metrics| metrics.sort_depth);
    view_position
        .2
        .wrapping_add(entry.sort_z)
        .wrapping_add(shape_sort_depth)
}

fn source_painter_order(
    entries: &[DrawListEntry],
    camera: SourceSceneCamera,
) -> Vec<usize> {
    let mut order: Vec<_> = (0..entries.len()).collect();
    // MDRAWLIS.MC `mallrotzsort` links the farthest object first. Its compare
    // decrements the existing depth before testing, so a later entry with the
    // same depth is inserted before the earlier entry.
    order.sort_by_key(|index| {
        (
            std::cmp::Reverse(source_sort_depth(&entries[*index], camera)),
            std::cmp::Reverse(*index),
        )
    });
    order
}

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

/// Fractional-precision counterpart of `lerp_angle8`: the integer version
/// truncates `diff*t` to 0 for the common |diff| <= 1-unit per-tick rotation
/// (so a banking ship's roll snaps once per 20 Hz tick even though its
/// position glides). Returns the un-truncated wrapped angle in [0, 256) for
/// feeding `Transform::build_model_matrix_f`.
fn lerp_angle8_f(from: i16, to: i16, t: f32) -> f32 {
    let a8 = (from & 0xFF) as i32;
    let b8 = (to & 0xFF) as i32;
    let mut diff = b8 - a8;
    if diff > 127 {
        diff -= 256;
    }
    if diff < -128 {
        diff += 256;
    }
    (a8 as f32 + diff as f32 * t).rem_euclid(256.0)
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

/// Build the camera-facing basis used by MARIO's `mssprite` path, then apply
/// its signed source-size adjustment. Scaled sprites are screen-facing visual
/// objects, so their object rotation is intentionally ignored.
fn apply_scaled_sprite_model(
    model: &mut [f32; 16],
    view: &[f32; 16],
    shape_id: u16,
    size_adjustment: u8,
) {
    // The view basis is orthonormal (including the SNES-to-GL reflection), so
    // its transpose is the exact inverse. `view * model` therefore has an
    // identity rotation and the source XY sprite plane always faces camera.
    model[0] = view[0];
    model[1] = view[4];
    model[2] = view[8];
    model[3] = 0.0;
    model[4] = view[1];
    model[5] = view[5];
    model[6] = view[9];
    model[7] = 0.0;
    model[8] = view[2];
    model[9] = view[6];
    model[10] = view[10];
    model[11] = 0.0;

    let scale = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape_id).map_or(1.0, |metrics| {
        let adjustment = i32::from(size_adjustment as i8) << u32::from(metrics.coordinate_shift);
        let adjusted_extent = (i32::from(metrics.visual_extent) + adjustment).max(1);
        adjusted_extent as f32 / f32::from(metrics.visual_extent.max(1))
    });
    for index in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
        model[index] *= scale;
    }
}

pub struct DrawListRenderer {
    source_texture: Option<TextureId>,
    last_source_indices: Vec<u8>,
    last_source_rgba: Vec<u8>,
    last_source_owners: Vec<u16>,
    last_source_faces: Vec<u16>,
}

impl DrawListRenderer {
    pub fn new() -> Self {
        DrawListRenderer {
            source_texture: None,
            last_source_indices: vec![0; crate::source_raster::WIDTH * crate::source_raster::HEIGHT],
            last_source_rgba: vec![
                0;
                crate::source_raster::WIDTH * crate::source_raster::HEIGHT * 4
            ],
            last_source_owners: vec![0; crate::source_raster::WIDTH * crate::source_raster::HEIGHT],
            last_source_faces: vec![
                crate::source_raster::NO_FACE;
                crate::source_raster::WIDTH * crate::source_raster::HEIGHT
            ],
        }
    }

    pub fn source_bitmap_indices(&self) -> &[u8] {
        &self.last_source_indices
    }

    pub fn source_bitmap_rgba(&self) -> &[u8] {
        &self.last_source_rgba
    }

    pub fn source_bitmap_owners(&self) -> &[u16] {
        &self.last_source_owners
    }

    pub fn source_bitmap_faces(&self) -> &[u16] {
        &self.last_source_faces
    }

    /// Mirror of `RenderShadow` (MARIO/MDRAWLIS.MC shadow pass): project the
    /// object's own mesh onto the ground plane by flattening the world-space
    /// Y row of the model matrix. `shadow_height` is the per-level BGS.ASM
    /// `shadowheight` (MDRAWLIS.MC:1416-1432 rotates the shadow at
    /// y = shadowheight - viewposy): SNES world Y of the ground plane,
    /// +down, 0 for planet surfaces, nucleusheight (400) inside the Nucleus.
    #[allow(clippy::too_many_arguments)]
    fn render_shadow(
        &self,
        gpu: &mut Gpu,
        proj: &[f32; 16],
        view: &[f32; 16],
        shapes: &ShapeStore,
        transform: &Transform,
        e: &DrawListEntry,
        shadow_height: f32,
    ) {
        let Some(shape) = shapes.get(e.shape_id) else {
            return;
        };
        if shape.num_triangles <= 0 {
            return;
        }
        // FP16.16 ground-plane height in the entry's coordinate space.
        let ground_fp = (shadow_height * 65536.0) as i32;
        if e.y > ground_fp {
            return; // below ground (SNES +Y is down): no shadow
        }

        let mut model = [0.0f32; 16];
        transform.build_model_matrix(&mut model, e.x, ground_fp, e.z, e.rx, e.ry, e.rz);
        // Flatten world-space Y (row 1 of the column-major matrix); lift
        // slightly off the ground plane to avoid coplanar artifacts.
        model[1] = 0.0;
        model[5] = 0.0;
        model[9] = 0.0;
        model[13] += 0.5;

        // Alpha-blended, non-depth-writing (was GL SRC_ALPHA blend + depth
        // mask off) so the shadow tints the ground instead of occluding it.
        gpu.push_flat_tris_alpha(&shape.tri_verts, proj, view, &model, [0.0, 0.0, 0.0, 0.40]);
    }

    /// Mirror of `DrawList_Render`. `shape_palette` is the frame's decoded
    /// BGS-selected polygon palette.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &mut self,
        gpu: &mut Gpu,
        shapes: &ShapeStore,
        transform: &mut Transform,
        prev: &[DrawListEntry],
        curr: &[DrawListEntry],
        alpha: f32,
        shadow_height: f32,
        shape_palette: &crate::shapes::ShapePaletteRgb,
        font: &mut Font,
        source_presentation_offset: Option<[i16; 2]>,
        source_bitmap_clear: Option<SourceBitmapRect>,
        source_scene_camera: Option<SourceSceneCamera>,
    ) {
        // At the exact fixed-update boundary the source still presents the
        // preceding complete draw snapshot. Iterating `curr` here made newly
        // born objects pop in one presentation frame early and removed
        // objects disappear early, even though matched objects correctly
        // interpolated from `prev` at alpha zero.
        let presented = if alpha <= 0.0 { prev } else { curr };
        if presented.is_empty() {
            return;
        }

        // The game tick is 20Hz but rendering is uncapped: rebuild the view
        // matrix from the interpolated camera so the camera glides with the
        // interpolated objects instead of stepping once per tick.
        transform.set_view_lerp(alpha);

        // Snapshot the interpolated view/projection; both are stable across
        // the pass (set_view_lerp is called once) and passed per draw.
        let view = *transform.view();
        let proj = *transform.projection();

        // Interpolated entries that want a drop shadow (collected during the
        // main pass, drawn afterwards as a translucent overlay).
        let mut shadow_list: Vec<DrawListEntry> = Vec::new();
        let mut source_raster = SourceRaster::new();

        // Pair current entries with the previous frame's entries by stable
        // object id (alien index), not by list position.
        let mut prev_by_id = [-1i16; MAX_OBJECTS + 1];
        for (i, p) in prev.iter().enumerate() {
            if (p.obj_id as usize) <= MAX_OBJECTS {
                prev_by_id[p.obj_id as usize] = i as i16;
            }
        }

        let source_camera = source_presentation_offset.map(|_| {
            source_scene_camera.unwrap_or_else(|| {
                let (camera, rotation) = transform.source_camera();
                SourceSceneCamera {
                    position: [camera.x, camera.y, camera.z],
                    rotation,
                }
            })
        });
        let presented_order = source_camera.map_or_else(
            || (0..presented.len()).collect(),
            |camera| source_painter_order(presented, camera),
        );

        // The source builds every dithered ground shadow into the indexed
        // bitmap before its normal-object pass. Keep this strict path
        // separate from the smooth translucent HD shadow pass below.
        if let Some(camera) = source_camera.filter(|_| matches!(alpha, 0.0 | 1.0)) {
            for &entry_index in &presented_order {
                let entry = &presented[entry_index];
                if entry.flags & (DL_FLAG_VISIBLE | DL_FLAG_SHADOW)
                    != (DL_FLAG_VISIBLE | DL_FLAG_SHADOW)
                    || entry.explosion_cnt != 0
                {
                    continue;
                }
                let previous_index = if entry.obj_id != 0 && (entry.obj_id as usize) <= MAX_OBJECTS {
                    prev_by_id[entry.obj_id as usize]
                } else {
                    -1
                };
                let shadow = if previous_index >= 0
                    && prev[previous_index as usize].shape_id == entry.shape_id
                {
                    interpolate_entry(&prev[previous_index as usize], entry, alpha)
                } else {
                    *entry
                };
                let ground = shadow_height as i16;
                if (shadow.y >> 16) as i16 > ground {
                    continue;
                }
                source_raster.set_owner(shadow.obj_id);
                shapes.render_source_shadow(
                    &mut source_raster,
                    shadow.shape_id,
                    shadow.anim_frame,
                    SourcePose {
                        world_position: [(shadow.x >> 16) as i16, ground, (shadow.z >> 16) as i16],
                        rotation: [shadow.rx as u8, shadow.ry as u8, shadow.rz as u8],
                        view_position: camera
                            .position
                            .map(|coordinate| (coordinate >> 16) as i16),
                        view_rotation: camera.rotation,
                    },
                    shape_palette,
                );
            }
        }

        for entry_index in presented_order {
            let entry = &presented[entry_index];
            if entry.flags & DL_FLAG_VISIBLE == 0 {
                continue;
            }

            // Interpolate if we have a matching previous entry.
            let prev_idx = if entry.obj_id != 0 && (entry.obj_id as usize) <= MAX_OBJECTS {
                prev_by_id[entry.obj_id as usize]
            } else {
                -1
            };
            let interpolating = prev_idx >= 0 && prev[prev_idx as usize].shape_id == entry.shape_id;
            let interp = if interpolating {
                interpolate_entry(&prev[prev_idx as usize], entry, alpha)
            } else {
                *entry
            };

            // Fractional interpolated rotation for a jitter-free model build
            // (interp.rx/ry/rz are truncated to whole SNES units and are still
            // used for the flat shadow pass, where the error is invisible).
            let (frx, fry, frz) = if interpolating {
                let p = &prev[prev_idx as usize];
                (
                    lerp_angle8_f(p.rx, entry.rx, alpha),
                    lerp_angle8_f(p.ry, entry.ry, alpha),
                    lerp_angle8_f(p.rz, entry.rz, alpha),
                )
            } else {
                (entry.rx as f32, entry.ry as f32, entry.rz as f32)
            };

            // Build model matrix.
            let mut model = [0.0f32; 16];
            transform.build_model_matrix_f(&mut model, interp.x, interp.y, interp.z, frx, fry, frz);

            if interp.flags & DL_FLAG_SCALED_SPRITE != 0 {
                apply_scaled_sprite_model(&mut model, &view, interp.shape_id, interp.tscroll_x);
            }

            // MARIO MDRAWLIS.MC handles ASF_TEXTOBJ before shape lookup.
            // The message pointer is in coltab, color in depth, and signed
            // size adjustment in tscrollx. `msprint` projects 127+size at
            // the object's camera depth and centers the fixed-width string.
            if interp.flags & DL_FLAG_TEXT != 0 {
                if let (Some(text), Some((ndc_x, ndc_y, depth))) = (
                    crate::text3d::message_text(interp.color_table),
                    project_model_origin(&proj, &view, &model),
                ) {
                    let size = interp.tscroll_x as i8 as i16;
                    let cell_ref = (127 + size) as f32 * 256.0 / depth;
                    let mut color_index = interp.depth_offset & 0x0f;
                    if color_index == 15 {
                        color_index = 14u8.saturating_sub(((depth as u32) >> 10).min(5) as u8);
                    }
                    let rgb = shape_palette[color_index as usize];
                    font.draw_string_scaled_centered_ndc(
                        gpu,
                        ndc_x,
                        ndc_y,
                        text.as_ref(),
                        cell_ref,
                        [rgb[0], rgb[1], rgb[2], 1.0],
                    );
                }
                continue;
            }

            // Retail wireframe objects are dedicated Face2-only shapes, so
            // they take the same exact material-aware shape path as every
            // other object.
            let source_pose = (interp.flags & DL_FLAG_SCALED_SPRITE == 0
                && matches!(alpha, 0.0 | 1.0))
            .then(|| source_camera)
            .flatten()
            .map(|camera| {
                SourcePose {
                    world_position: [
                        (interp.x >> 16) as i16,
                        (interp.y >> 16) as i16,
                        (interp.z >> 16) as i16,
                    ],
                    rotation: [interp.rx as u8, interp.ry as u8, interp.rz as u8],
                    view_position: camera
                        .position
                        .map(|coordinate| (coordinate >> 16) as i16),
                    view_rotation: camera.rotation,
                }
            });
            source_raster.set_owner(interp.obj_id);
            if interp.flags & DL_FLAG_SCALED_SPRITE != 0 {
                if let Some(camera) = source_camera.filter(|_| matches!(alpha, 0.0 | 1.0)) {
                    let pose = SourcePose {
                        world_position: [
                            (interp.x >> 16) as i16,
                            (interp.y >> 16) as i16,
                            (interp.z >> 16) as i16,
                        ],
                        rotation: [0; 3],
                        view_position: camera
                            .position
                            .map(|coordinate| (coordinate >> 16) as i16),
                        view_rotation: camera.rotation,
                    };
                    shapes.render_source_scaled_sprite(
                        &mut source_raster,
                        interp.shape_id,
                        interp.col_frame,
                        interp.color_table,
                        interp.depth_offset,
                        interp.tscroll_x,
                        pose,
                        shape_palette,
                    );
                }
            }
            shapes.render(
                gpu,
                &mut source_raster,
                transform,
                interp.shape_id,
                interp.anim_frame,
                interp.col_frame,
                interp.color_table,
                interp.explosion_cnt,
                &model,
                source_pose,
                shape_palette,
            );

            // Queue the drop shadow (skip exploding objects).
            if interp.flags & DL_FLAG_SHADOW != 0
                && interp.explosion_cnt == 0
                && shadow_list.len() < MAX_DRAW_LIST
            {
                shadow_list.push(interp);
            }
        }

        let (output_width, output_height) = gpu.size();
        if let Some(rect) = source_bitmap_clear {
            source_raster.clear_rect(rect);
        }
        self.last_source_indices
            .clone_from_slice(source_raster.indices());
        self.last_source_rgba.clone_from_slice(source_raster.rgba());
        self.last_source_owners
            .clone_from_slice(source_raster.owners());
        self.last_source_faces
            .clone_from_slice(source_raster.faces());
        source_raster.submit(
            gpu,
            &mut self.source_texture,
            output_width,
            output_height,
            source_presentation_offset.unwrap_or([0; 2]),
        );

        // --- Shadow pass (after the opaque pass so depth testing hides
        // shadow fragments behind solid geometry). The retained flat pipeline
        // has no alpha blend / depth-mask toggle, so shadows draw as opaque
        // black tris (see parity note). ---
        if source_presentation_offset.is_none() {
            for e in &shadow_list {
                self.render_shadow(gpu, &proj, &view, shapes, transform, e, shadow_height);
            }
        }
    }
}

impl Default for DrawListRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    const MEDIUM_EXPLOSION_SPRITE_SHAPE: u16 = 462;
    const PLAYER_SPRITE_SCALE_ADJUSTMENT: u8 = 253;
    const CAMERA_PITCH: i16 = 23;
    const CAMERA_YAW: i16 = 71;
    const CAMERA_ROLL: i16 = 9;
    const DESTROYED_PITCH: i16 = 41;
    const DESTROYED_YAW: i16 = 117;
    const DESTROYED_ROLL: i16 = 202;
    const DESTROYED_X: i32 = 100 << 16;
    const DESTROYED_Y: i32 = -50 << 16;
    const DESTROYED_Z: i32 = 500 << 16;
    const EXPECTED_PLAYER_SPRITE_SCALE: f32 = 0.25;
    const MATRIX_EPSILON: f32 = 0.000_01;

    #[test]
    fn source_painter_orders_far_to_near_and_reverses_equal_depths() {
        let at_depth = |depth: i32| DrawListEntry {
            z: depth << 16,
            shape_id: crate::shapes::SHAPE_ARWING,
            ..DrawListEntry::default()
        };
        let entries = [at_depth(167), at_depth(479), at_depth(479)];
        let camera = SourceSceneCamera {
            position: [0; 3],
            rotation: [0; 3],
        };

        assert_eq!(source_painter_order(&entries, camera), [2, 1, 0]);
    }

    #[test]
    fn projects_model_origin_with_gpu_matrix_order() {
        let mut proj = [0.0; 16];
        let mut view = [0.0; 16];
        let mut model = [0.0; 16];
        crate::transform::identity(&mut proj);
        crate::transform::identity(&mut view);
        crate::transform::identity(&mut model);
        // A minimal perspective transform with w = -z.
        proj[11] = -1.0;
        proj[15] = 0.0;
        model[12] = 2.0;
        model[13] = -1.0;
        model[14] = -4.0;
        let (x, y, depth) = project_model_origin(&proj, &view, &model).unwrap();
        assert_eq!(depth, 4.0);
        assert_eq!(x, 0.5);
        assert_eq!(y, -0.25);
    }

    #[test]
    fn rejects_points_behind_camera() {
        let mut proj = [0.0; 16];
        let mut view = [0.0; 16];
        let mut model = [0.0; 16];
        crate::transform::identity(&mut proj);
        crate::transform::identity(&mut view);
        crate::transform::identity(&mut model);
        proj[11] = -1.0;
        proj[15] = 0.0;
        model[14] = 4.0;
        assert!(project_model_origin(&proj, &view, &model).is_none());
    }

    #[test]
    fn scaled_sprite_cancels_a_rotated_camera_basis() {
        let mut transform = Transform::new();
        transform.set_camera(0, 0, 0, CAMERA_PITCH, CAMERA_YAW, CAMERA_ROLL);
        let view = *transform.view();
        let mut model = [0.0; 16];
        transform.build_model_matrix(
            &mut model,
            DESTROYED_X,
            DESTROYED_Y,
            DESTROYED_Z,
            DESTROYED_PITCH,
            DESTROYED_YAW,
            DESTROYED_ROLL,
        );

        apply_scaled_sprite_model(
            &mut model,
            &view,
            MEDIUM_EXPLOSION_SPRITE_SHAPE,
            PLAYER_SPRITE_SCALE_ADJUSTMENT,
        );
        let mut camera_space = [0.0; 16];
        crate::transform::multiply(&mut camera_space, &view, &model);

        for (index, expected) in [
            (0, EXPECTED_PLAYER_SPRITE_SCALE),
            (1, 0.0),
            (2, 0.0),
            (4, 0.0),
            (5, EXPECTED_PLAYER_SPRITE_SCALE),
            (6, 0.0),
            (8, 0.0),
            (9, 0.0),
            (10, EXPECTED_PLAYER_SPRITE_SCALE),
        ] {
            assert!(
                (camera_space[index] - expected).abs() <= MATRIX_EPSILON,
                "matrix[{index}]={} expected {expected}",
                camera_space[index]
            );
        }
    }
}
