//! 3D draw-list pass with obj_id-keyed interpolation and drop shadows.
//!
//! Port (C oracle): `src/renderer/draw_list.c` plus the `DrawListEntry`
//! struct and `DL_FLAG_*` constants from `src/types.h`.

use crate::font::Font;
use crate::gpu::{Gpu, TextureId};
use crate::shapes::{SHAPE_ALIAS_OP_0, SHAPE_ALIAS_OP_1, SHAPE_ALIAS_OP_2};
use crate::shapes_gl::{ShapeRenderMode, ShapeStore};
use crate::source_projection::SourcePose;
use crate::source_raster::{SourceBitmapRect, SourceRaster};
use crate::transform::Transform;
use sf_core::point_field::PointPixel;

pub const MAX_OBJECTS: usize = 128;
pub const MAX_DRAW_LIST: usize = 128;

pub const DL_FLAG_VISIBLE: u8 = 0x01;
pub const DL_FLAG_SHADOW: u8 = 0x02;
pub const DL_FLAG_HIGHLIGHT: u8 = 0x04;
pub const DL_FLAG_TEXT: u8 = 0x10;
pub const DL_FLAG_SCALED_SPRITE: u8 = 0x20;

/// Presentation style for polygon shading and projected ground shadows.
/// Oracle captures select [`Self::RetailDithered`] explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowStyle {
    /// Do not draw projected ground shadows; average polygon shade pairs.
    Disabled,
    /// Draw smooth shadows and average polygon shade pairs.
    #[default]
    Smooth,
    /// Preserve retail alternating-pixel shadows and polygon shade pairs.
    RetailDithered,
}

impl ShadowStyle {
    /// Decode the documented `[Video] ShadowStyle` INI value.
    pub fn from_config_value(value: i32) -> Self {
        match value {
            1 => Self::Smooth,
            2 => Self::RetailDithered,
            _ => Self::Disabled,
        }
    }
}

const RETAIL_SHADOW_COLOR: u8 = 9;
const RETAIL_SHADOW_TRANSPARENT: u8 = 0;

const STRATEGY_COLOR_SPECIAL: u8 = 0x01;
const STRATEGY_COLOR_HIT_FLASH: u8 = 0x02;

/// The authored draw-list material override selected by an object's semantic
/// presentation flags. It is resolved before either raster path so the exact
/// and HD presentations describe the same game state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectColorPolicy {
    Inherited,
    Special,
    HitFlash,
    SpecialHitFlash,
}

impl ObjectColorPolicy {
    fn from_strategy_flags(strategy_flags: u8, scaled_sprite: bool) -> Self {
        let special = strategy_flags & STRATEGY_COLOR_SPECIAL != 0;
        let hit_flash = strategy_flags & STRATEGY_COLOR_HIT_FLASH != 0;
        match (special, hit_flash, scaled_sprite) {
            (_, true, true) | (false, false, _) => Self::Inherited,
            (true, false, _) => Self::Special,
            (false, true, false) => Self::HitFlash,
            (true, true, false) => Self::SpecialHitFlash,
        }
    }

    fn resolve(self, inherited: u16) -> u16 {
        match self {
            Self::Inherited => inherited,
            Self::Special => crate::color_data::COLOR_TABLE_ID_1_C,
            Self::HitFlash => crate::color_data::COLOR_TABLE_WHITE_C,
            Self::SpecialHitFlash => crate::color_data::COLOR_TABLE_RED_C,
        }
    }
}

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
    let previous = prev.iter().find(|entry| entry.obj_id == object_id);
    let current = curr.iter().find(|entry| entry.obj_id == object_id);
    let entry = if alpha < 1.0 {
        let previous = previous?;
        current
            .filter(|current| can_interpolate(previous, current))
            .map_or(*previous, |current| {
                let alpha = source_safe_interpolation_alpha(
                    previous,
                    current,
                    transform.source_camera_endpoints(),
                    alpha,
                );
                interpolate_entry(previous, current, alpha)
            })
    } else {
        *current?
    };

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
    /// Allocation-lifetime token paired with `obj_id` for interpolation.
    pub interpolation_id: u64,
}

fn can_interpolate(previous: &DrawListEntry, current: &DrawListEntry) -> bool {
    current.obj_id != 0
        && previous.obj_id == current.obj_id
        && previous.shape_id == current.shape_id
        && previous.interpolation_id == current.interpolation_id
}

fn interpolation_pair<'a>(
    entry: &'a DrawListEntry,
    presenting_previous: bool,
    prev: &'a [DrawListEntry],
    curr: &'a [DrawListEntry],
    prev_by_id: &[i16; MAX_OBJECTS + 1],
    curr_by_id: &[i16; MAX_OBJECTS + 1],
) -> Option<(&'a DrawListEntry, &'a DrawListEntry)> {
    let object_id = usize::from(entry.obj_id);
    if object_id == 0 || object_id > MAX_OBJECTS {
        return None;
    }
    if presenting_previous {
        let current_index = curr_by_id[object_id];
        if current_index < 0 {
            return None;
        }
        let current = &curr[current_index as usize];
        can_interpolate(entry, current).then_some((entry, current))
    } else {
        let previous_index = prev_by_id[object_id];
        if previous_index < 0 {
            return None;
        }
        let previous = &prev[previous_index as usize];
        can_interpolate(previous, entry).then_some((previous, entry))
    }
}

fn presentation_entries<'a>(
    prev: &'a [DrawListEntry],
    curr: &'a [DrawListEntry],
    alpha: f32,
) -> (bool, &'a [DrawListEntry]) {
    let presenting_previous = alpha < 1.0;
    (
        presenting_previous,
        if presenting_previous { prev } else { curr },
    )
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
    let camera_position = camera.position.map(|coordinate| (coordinate >> 16) as i16);
    let relative = [
        world_position[0].wrapping_sub(camera_position[0]),
        world_position[1].wrapping_sub(camera_position[1]),
        world_position[2].wrapping_sub(camera_position[2]),
    ];
    let view_position =
        sf_core::snes_trig::matrix_rotate_q15(view_matrix, relative[0], relative[1], relative[2]);
    let shape_sort_depth = sf_core::sf1_shape_metrics::sf1_shape_metrics(entry.shape_id)
        .map_or(0, |metrics| metrics.sort_depth);
    view_position
        .2
        .wrapping_add(entry.sort_z)
        .wrapping_add(shape_sort_depth)
}

fn source_painter_order(entries: &[DrawListEntry], camera: SourceSceneCamera) -> Vec<usize> {
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

fn launch_corridor_depth_layers(entries: &[DrawListEntry]) -> [u8; MAX_DRAW_LIST] {
    let mut tunnel_order = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            matches!(
                entry.shape_id,
                SHAPE_ALIAS_OP_0 | SHAPE_ALIAS_OP_1 | SHAPE_ALIAS_OP_2
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    // Consecutive MAP1_1A tunnel meshes overlap by twenty source units. Their
    // seam owner must remain fixed while the intro camera turns through 180
    // degrees: camera-depth sorting reverses at the midpoint and makes the
    // two coplanar surfaces exchange ownership for a frame. World depth and
    // shape identity are authored, stable tie-breakers for this one corridor.
    tunnel_order.sort_by_key(|index| {
        let entry = &entries[*index];
        // MAP1_1A creates the OP_0 outline before its OP_1 backing at equal
        // depth. MDRAWLIS inserts later equal-depth objects first, so the
        // outline is painted last. Preserve that ownership in HD depth too:
        // sorting flat shape ids upward instead buried coplanar floor lines.
        (
            entry.z,
            std::cmp::Reverse(entry.shape_id),
            entry.interpolation_id,
        )
    });
    let mut layers = [0; MAX_DRAW_LIST];
    for (layer, index) in tunnel_order.into_iter().enumerate() {
        layers[index] = (layer + 1) as u8;
    }
    layers
}

fn has_source_shadow(entry: &DrawListEntry) -> bool {
    entry.flags & (DL_FLAG_VISIBLE | DL_FLAG_SHADOW) == (DL_FLAG_VISIBLE | DL_FLAG_SHADOW)
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
    // Positions and rotations are continuous presentation values. Animation,
    // material, topology, and flags are fixed-update state and remain on the
    // preceding snapshot throughout the open interval.
    let mut out = if alpha < 1.0 { *a } else { *b };
    out.x = (a.x as f32 + (b.x as f32 - a.x as f32) * alpha) as i32;
    out.y = (a.y as f32 + (b.y as f32 - a.y as f32) * alpha) as i32;
    out.z = (a.z as f32 + (b.z as f32 - a.z as f32) * alpha) as i32;
    out.rx = lerp_angle8(a.rx, b.rx, alpha);
    out.ry = lerp_angle8(a.ry, b.ry, alpha);
    out.rz = lerp_angle8(a.rz, b.rz, alpha);
    out
}

fn source_camera_depth(
    entry: &DrawListEntry,
    camera: (crate::transform::CameraState, [u16; 3]),
) -> i16 {
    let (position, rotation) = camera;
    let relative = [
        ((entry.x.wrapping_sub(position.x)) >> 16) as i16,
        ((entry.y.wrapping_sub(position.y)) >> 16) as i16,
        ((entry.z.wrapping_sub(position.z)) >> 16) as i16,
    ];
    let matrix = sf_core::snes_trig::zxy_matrix_q15_fine(rotation[0], rotation[1], rotation[2]);
    sf_core::snes_trig::matrix_rotate_q15(matrix, relative[0], relative[1], relative[2]).2
}

/// Interpolation must not synthesize poses while an object crosses through
/// the camera plane. The retail renderer presents only the two fixed-update
/// endpoints; blending across the sign change feeds near-zero depth into the
/// perspective divide and turns corridor segments or close ships into long,
/// flickering shards. Hold the preceding source pose through that one open
/// interval, then switch at its ordinary endpoint.
fn source_safe_interpolation_alpha(
    previous: &DrawListEntry,
    current: &DrawListEntry,
    camera_endpoints: [(crate::transform::CameraState, [u16; 3]); 2],
    alpha: f32,
) -> f32 {
    let previous_depth = source_camera_depth(previous, camera_endpoints[0]);
    let current_depth = source_camera_depth(current, camera_endpoints[1]);
    if alpha < 1.0 && (previous_depth > 0) != (current_depth > 0) {
        0.0
    } else {
        alpha
    }
}

/// Build the camera-facing basis used by MARIO's `mssprite` path, then apply
/// its signed source-size adjustment. Scaled sprites are screen-facing visual
/// objects, so their object rotation is intentionally ignored.
fn apply_scaled_sprite_model(
    model: &mut [f32; 16],
    view: &[f32; 16],
    shapes: &ShapeStore,
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

    if let (Some(metrics), Some(shape)) = (
        sf_core::sf1_shape_metrics::sf1_shape_metrics(shape_id),
        shapes.get(shape_id),
    ) {
        // MDSPRITE.MC doubles sh_size before adding the signed, shifted
        // strategy adjustment. This is the complete square's width, not
        // a multiplier for the unrelated polygon mesh's authored extent.
        let adjustment =
            i16::from(size_adjustment as i8).wrapping_shl(u32::from(metrics.coordinate_shift));
        let world_size = (metrics.visual_extent as i16)
            .wrapping_mul(2)
            .wrapping_add(adjustment);
        let world_size = if world_size == 0 {
            1
        } else {
            world_size.max(0)
        };
        let bounds = shape.vertices.iter().fold(
            [
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ],
            |[left, right, top, bottom], vertex| {
                [
                    left.min(vertex.x),
                    right.max(vertex.x),
                    top.min(vertex.y),
                    bottom.max(vertex.y),
                ]
            },
        );
        for (column, span) in [(0, bounds[1] - bounds[0]), (4, bounds[3] - bounds[2])] {
            if span > 0.0 {
                let scale = f32::from(world_size) / span;
                for index in column..column + 3 {
                    model[index] *= scale;
                }
            }
        }
    }
}

pub struct DrawListRenderer {
    source_texture: Option<TextureId>,
    last_source_indices: Vec<u8>,
    last_source_rgba: Vec<u8>,
    last_source_owners: Vec<u16>,
    last_source_faces: Vec<u16>,
    last_source_workload: crate::source_raster::SourceFrameWorkload,
}

impl DrawListRenderer {
    pub fn new() -> Self {
        DrawListRenderer {
            source_texture: None,
            last_source_indices: vec![
                0;
                crate::source_raster::WIDTH * crate::source_raster::HEIGHT
            ],
            last_source_rgba: vec![
                0;
                crate::source_raster::WIDTH * crate::source_raster::HEIGHT * 4
            ],
            last_source_owners: vec![0; crate::source_raster::WIDTH * crate::source_raster::HEIGHT],
            last_source_faces: vec![
                crate::source_raster::NO_FACE;
                crate::source_raster::WIDTH * crate::source_raster::HEIGHT
            ],
            last_source_workload: crate::source_raster::SourceFrameWorkload::default(),
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

    pub fn source_frame_workload(&self) -> crate::source_raster::SourceFrameWorkload {
        self.last_source_workload
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
        shape_palette: &crate::shapes::ShapePaletteRgb,
        shadow_style: ShadowStyle,
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

        match shadow_style {
            ShadowStyle::Disabled => {}
            ShadowStyle::Smooth => {
                // Alpha-blended and non-depth-writing so the HD shadow tints
                // the ground instead of occluding it.
                gpu.push_flat_tris_alpha(
                    &shape.tri_verts,
                    proj,
                    view,
                    &model,
                    [0.0, 0.0, 0.0, 0.40],
                );
            }
            ShadowStyle::RetailDithered => {
                let palette = std::array::from_fn(|index| {
                    [
                        shape_palette[index][0],
                        shape_palette[index][1],
                        shape_palette[index][2],
                        1.0,
                    ]
                });
                gpu.push_palette_pair_tris_alpha(
                    &shape.tri_verts,
                    proj,
                    view,
                    &model,
                    &palette,
                    [RETAIL_SHADOW_COLOR, RETAIL_SHADOW_TRANSPARENT],
                );
            }
        }
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
        source_point_pixels: &[PointPixel],
        source_gameplay_meter_palette: Option<&crate::shapes::ShapePaletteRgb>,
        shadow_style: ShadowStyle,
    ) {
        // Interpolation presents the interval from `prev` to `curr`. Keep the
        // preceding snapshot's topology throughout that open interval: a
        // retired object remains until the interpolated camera reaches the
        // tick that retired it, while a newborn object enters at that same
        // endpoint. Switching to `curr` for every alpha above zero made whole
        // corridor segments disappear almost one tick before the camera
        // reached their cull boundary, exposing seams as a 20 Hz flicker.
        let (presenting_previous, presented) = presentation_entries(prev, curr, alpha);
        self.last_source_workload = crate::source_raster::SourceFrameWorkload::default();
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
        let camera_endpoints = transform.source_camera_endpoints();

        // Interpolated entries that want a drop shadow (collected during the
        // main pass, drawn afterwards as a translucent overlay).
        let mut shadow_list: Vec<DrawListEntry> = Vec::new();
        let palette_pair_style = if shadow_style == ShadowStyle::RetailDithered {
            crate::shapes::PalettePairStyle::RetailDithered
        } else {
            crate::shapes::PalettePairStyle::Smooth
        };
        let mut source_raster = SourceRaster::with_palette_pair_style(palette_pair_style);
        if source_presentation_offset.is_some() {
            source_raster.draw_point_field(source_point_pixels, shape_palette);
        }

        // Pair current entries with the previous frame's entries by stable
        // object id (alien index), not by list position.
        let mut prev_by_id = [-1i16; MAX_OBJECTS + 1];
        for (i, p) in prev.iter().enumerate() {
            if (p.obj_id as usize) <= MAX_OBJECTS {
                prev_by_id[p.obj_id as usize] = i as i16;
            }
        }
        let mut curr_by_id = [-1i16; MAX_OBJECTS + 1];
        for (i, current) in curr.iter().enumerate() {
            if (current.obj_id as usize) <= MAX_OBJECTS {
                curr_by_id[current.obj_id as usize] = i as i16;
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
        let corridor_depth_layers = launch_corridor_depth_layers(presented);

        // The source builds ground shadows into the indexed bitmap before its
        // normal-object pass. The selected presentation style controls whether
        // that bitmap uses smooth averaged colors or retail checkerboards.
        if shadow_style != ShadowStyle::Disabled {
            if let Some(camera) = source_camera.filter(|_| matches!(alpha, 0.0 | 1.0)) {
                for &entry_index in &presented_order {
                    let entry = &presented[entry_index];
                    if !has_source_shadow(entry) {
                        continue;
                    }
                    let shadow = interpolation_pair(
                        entry,
                        presenting_previous,
                        prev,
                        curr,
                        &prev_by_id,
                        &curr_by_id,
                    )
                    .map_or(*entry, |(previous, current)| {
                        interpolate_entry(previous, current, alpha)
                    });
                    let ground = shadow_height as i16;
                    if (shadow.y >> 16) as i16 > ground {
                        continue;
                    }
                    source_raster.set_owner(shadow.obj_id);
                    shapes.render_source_shadow(
                        &mut source_raster,
                        shadow.shape_id,
                        shadow.anim_frame,
                        shadow.explosion_cnt,
                        SourcePose {
                            world_position: [
                                (shadow.x >> 16) as i16,
                                ground,
                                (shadow.z >> 16) as i16,
                            ],
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
        }

        for entry_index in presented_order {
            let entry = &presented[entry_index];
            if entry.flags & DL_FLAG_VISIBLE == 0 {
                continue;
            }

            // Interpolate if we have a matching previous entry.
            let interpolation = interpolation_pair(
                entry,
                presenting_previous,
                prev,
                curr,
                &prev_by_id,
                &curr_by_id,
            );
            let interp = interpolation.map_or(*entry, |(previous, current)| {
                interpolate_entry(
                    previous,
                    current,
                    source_safe_interpolation_alpha(previous, current, camera_endpoints, alpha),
                )
            });

            // Fractional interpolated rotation for a jitter-free model build
            // (interp.rx/ry/rz are truncated to whole SNES units and are still
            // used for the flat shadow pass, where the error is invisible).
            let (frx, fry, frz) = if let Some((previous, current)) = interpolation {
                let alpha =
                    source_safe_interpolation_alpha(previous, current, camera_endpoints, alpha);
                (
                    lerp_angle8_f(previous.rx, current.rx, alpha),
                    lerp_angle8_f(previous.ry, current.ry, alpha),
                    lerp_angle8_f(previous.rz, current.rz, alpha),
                )
            } else {
                (entry.rx as f32, entry.ry as f32, entry.rz as f32)
            };

            // Build model matrix.
            let mut model = [0.0f32; 16];
            transform.build_model_matrix_f(&mut model, interp.x, interp.y, interp.z, frx, fry, frz);

            if interp.flags & DL_FLAG_SCALED_SPRITE != 0 {
                apply_scaled_sprite_model(
                    &mut model,
                    &view,
                    shapes,
                    interp.shape_id,
                    interp.tscroll_x,
                );
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

            let scaled_sprite = interp.flags & DL_FLAG_SCALED_SPRITE != 0;
            let color_table = ObjectColorPolicy::from_strategy_flags(interp.sflags, scaled_sprite)
                .resolve(interp.color_table);

            // Retail wireframe objects are dedicated Face2-only shapes, so
            // they take the same exact material-aware shape path as every
            // other object.
            let source_pose = (interp.flags & DL_FLAG_SCALED_SPRITE == 0
                && matches!(alpha, 0.0 | 1.0))
            .then(|| source_camera)
            .flatten()
            .map(|camera| SourcePose {
                world_position: [
                    (interp.x >> 16) as i16,
                    (interp.y >> 16) as i16,
                    (interp.z >> 16) as i16,
                ],
                rotation: [interp.rx as u8, interp.ry as u8, interp.rz as u8],
                view_position: camera.position.map(|coordinate| (coordinate >> 16) as i16),
                view_rotation: camera.rotation,
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
                        view_position: camera.position.map(|coordinate| (coordinate >> 16) as i16),
                        view_rotation: camera.rotation,
                    };
                    shapes.render_source_scaled_sprite(
                        &mut source_raster,
                        interp.shape_id,
                        interp.col_frame,
                        color_table,
                        interp.depth_offset,
                        interp.tscroll_x,
                        pose,
                        shape_palette,
                    );
                    // The source sprite path owns this draw, including its
                    // clipping decision. Never add a second HD quad behind
                    // its transparent pixels in strict source output.
                    continue;
                }
            }
            shapes.render(
                gpu,
                &mut source_raster,
                transform,
                interp.shape_id,
                interp.anim_frame,
                interp.col_frame,
                color_table,
                interp.depth_offset,
                [interp.tscroll_x, interp.tscroll_y],
                interp.explosion_cnt,
                &model,
                corridor_depth_layers[entry_index],
                if scaled_sprite {
                    ShapeRenderMode::ScaledSprite
                } else {
                    ShapeRenderMode::Polygons
                },
                source_pose,
                shape_palette,
                palette_pair_style,
            );

            // Queue the drop shadow (skip exploding objects).
            if shadow_style != ShadowStyle::Disabled
                && source_presentation_offset.is_none()
                && interp.flags & DL_FLAG_SHADOW != 0
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
        if let Some(palette) = source_gameplay_meter_palette {
            source_raster.apply_gameplay_meter_palette(palette);
        }
        self.last_source_indices
            .clone_from_slice(source_raster.indices());
        self.last_source_rgba.clone_from_slice(source_raster.rgba());
        self.last_source_owners
            .clone_from_slice(source_raster.owners());
        self.last_source_faces
            .clone_from_slice(source_raster.faces());
        self.last_source_workload = source_raster.workload();
        source_raster.submit(
            gpu,
            &mut self.source_texture,
            output_width,
            output_height,
            source_presentation_offset.unwrap_or([0; 2]),
        );

        // --- Optional HD shadow pass (after opaque geometry so depth testing
        // hides projected fragments behind solid objects). ---
        if source_presentation_offset.is_none() && shadow_style != ShadowStyle::Disabled {
            for e in &shadow_list {
                self.render_shadow(
                    gpu,
                    &proj,
                    &view,
                    shapes,
                    transform,
                    e,
                    shadow_height,
                    shape_palette,
                    shadow_style,
                );
            }
        }
    }
}

#[cfg(test)]
mod interpolation_tests {
    use super::{
        can_interpolate, interpolate_entry, launch_corridor_depth_layers, presentation_entries,
        source_safe_interpolation_alpha, DrawListEntry,
    };
    use crate::shapes::{SHAPE_ALIAS_OP_0, SHAPE_ALIAS_OP_1};
    use crate::transform::Transform;

    #[test]
    fn source_slot_reuse_never_interpolates_across_allocation_lifetimes() {
        const RECYCLED_TUNNEL_SHAPE: u16 = 510;
        const FIRST_ALLOCATION_ID: u64 = 41;

        let previous = DrawListEntry {
            obj_id: 1,
            shape_id: RECYCLED_TUNNEL_SHAPE,
            interpolation_id: FIRST_ALLOCATION_ID,
            ..DrawListEntry::default()
        };
        let same_object = DrawListEntry {
            interpolation_id: previous.interpolation_id,
            ..previous
        };
        let reused_slot = DrawListEntry {
            interpolation_id: previous.interpolation_id + 1,
            ..previous
        };

        assert!(can_interpolate(&previous, &same_object));
        assert!(!can_interpolate(&previous, &reused_slot));
    }

    #[test]
    fn previous_topology_is_presented_until_the_interpolation_endpoint() {
        const RETIRED_OBJECT_ID: u16 = 2;
        const NEW_OBJECT_ID: u16 = 3;
        const HALF_TICK: f32 = 0.5;

        let previous = [
            DrawListEntry {
                obj_id: 1,
                ..DrawListEntry::default()
            },
            DrawListEntry {
                obj_id: RETIRED_OBJECT_ID,
                ..DrawListEntry::default()
            },
        ];
        let current = [
            previous[0],
            DrawListEntry {
                obj_id: NEW_OBJECT_ID,
                ..DrawListEntry::default()
            },
        ];

        let (presenting_previous, between_ticks) =
            presentation_entries(&previous, &current, HALF_TICK);
        assert!(presenting_previous);
        assert!(between_ticks
            .iter()
            .any(|entry| entry.obj_id == RETIRED_OBJECT_ID));
        assert!(!between_ticks
            .iter()
            .any(|entry| entry.obj_id == NEW_OBJECT_ID));

        let (presenting_previous, at_endpoint) = presentation_entries(&previous, &current, 1.0);
        assert!(!presenting_previous);
        assert!(!at_endpoint
            .iter()
            .any(|entry| entry.obj_id == RETIRED_OBJECT_ID));
        assert!(at_endpoint
            .iter()
            .any(|entry| entry.obj_id == NEW_OBJECT_ID));
    }

    #[test]
    fn discrete_presentation_state_advances_only_at_the_endpoint() {
        const PREVIOUS_ANIMATION: u8 = 14;
        const CURRENT_ANIMATION: u8 = 15;
        const PREVIOUS_POSITION: i32 = 100;
        const CURRENT_POSITION: i32 = 200;
        const HALF_TICK: f32 = 0.5;

        let previous = DrawListEntry {
            x: PREVIOUS_POSITION,
            anim_frame: PREVIOUS_ANIMATION,
            col_frame: PREVIOUS_ANIMATION,
            ..DrawListEntry::default()
        };
        let current = DrawListEntry {
            x: CURRENT_POSITION,
            anim_frame: CURRENT_ANIMATION,
            col_frame: CURRENT_ANIMATION,
            ..previous
        };

        let between = interpolate_entry(&previous, &current, HALF_TICK);
        assert_eq!(between.x, 150);
        assert_eq!(between.anim_frame, PREVIOUS_ANIMATION);
        assert_eq!(between.col_frame, PREVIOUS_ANIMATION);

        let endpoint = interpolate_entry(&previous, &current, 1.0);
        assert_eq!(endpoint.x, CURRENT_POSITION);
        assert_eq!(endpoint.anim_frame, CURRENT_ANIMATION);
        assert_eq!(endpoint.col_frame, CURRENT_ANIMATION);
    }

    #[test]
    fn camera_plane_crossings_hold_the_preceding_source_pose() {
        const HALF_TICK: f32 = 0.5;
        const PREVIOUS_DEPTH: i32 = 40;
        const CURRENT_DEPTH: i32 = -40;

        let mut transform = Transform::new();
        transform.set_camera(0, 0, 0, 0, 0, 0);
        transform.set_camera(0, 0, 0, 0, 0, 0);
        let previous = DrawListEntry {
            z: PREVIOUS_DEPTH << 16,
            ..DrawListEntry::default()
        };
        let current = DrawListEntry {
            z: CURRENT_DEPTH << 16,
            ..previous
        };

        assert_eq!(
            source_safe_interpolation_alpha(
                &previous,
                &current,
                transform.source_camera_endpoints(),
                HALF_TICK,
            ),
            0.0
        );
        assert_eq!(
            source_safe_interpolation_alpha(
                &previous,
                &current,
                transform.source_camera_endpoints(),
                1.0,
            ),
            1.0
        );
    }

    #[test]
    fn in_front_motion_keeps_smooth_interpolation() {
        const HALF_TICK: f32 = 0.5;

        let mut transform = Transform::new();
        transform.set_camera(0, 0, 0, 0, 0, 0);
        transform.set_camera(0, 0, 0, 0, 0, 0);
        let previous = DrawListEntry {
            z: 80 << 16,
            ..DrawListEntry::default()
        };
        let current = DrawListEntry {
            z: 40 << 16,
            ..previous
        };

        assert_eq!(
            source_safe_interpolation_alpha(
                &previous,
                &current,
                transform.source_camera_endpoints(),
                HALF_TICK,
            ),
            HALF_TICK
        );
    }

    #[test]
    fn overlapping_corridor_segments_follow_stable_world_depth() {
        const FAR_DEPTH: i32 = 800;
        const NEAR_DEPTH: i32 = 200;

        let entries = [
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_0,
                z: FAR_DEPTH << 16,
                ..DrawListEntry::default()
            },
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_1,
                z: NEAR_DEPTH << 16,
                ..DrawListEntry::default()
            },
            DrawListEntry::default(),
        ];
        let layers = launch_corridor_depth_layers(&entries);

        assert_eq!(layers[0], 2);
        assert_eq!(layers[1], 1);
        assert_eq!(layers[2], 0);
    }

    #[test]
    fn later_corridor_spawns_do_not_renumber_existing_seam_owners() {
        const FIRST_DEPTH: i32 = 200;
        const SECOND_DEPTH: i32 = 800;

        let first_pair = [
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_0,
                z: FIRST_DEPTH << 16,
                interpolation_id: 1,
                ..DrawListEntry::default()
            },
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_1,
                z: FIRST_DEPTH << 16,
                interpolation_id: 2,
                ..DrawListEntry::default()
            },
        ];
        let initial = launch_corridor_depth_layers(&first_pair);
        let with_later_pair = launch_corridor_depth_layers(&[
            first_pair[0],
            first_pair[1],
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_0,
                z: SECOND_DEPTH << 16,
                interpolation_id: 3,
                ..DrawListEntry::default()
            },
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_1,
                z: SECOND_DEPTH << 16,
                interpolation_id: 4,
                ..DrawListEntry::default()
            },
        ]);

        assert_eq!(&with_later_pair[..2], &initial[..2]);
    }

    #[test]
    fn corridor_outline_keeps_source_priority_over_its_backing() {
        const SEGMENT_DEPTH: i32 = 250 << 16;
        let entries = [
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_0,
                z: SEGMENT_DEPTH,
                ..DrawListEntry::default()
            },
            DrawListEntry {
                shape_id: SHAPE_ALIAS_OP_1,
                z: SEGMENT_DEPTH,
                ..DrawListEntry::default()
            },
        ];
        let layers = launch_corridor_depth_layers(&entries);
        assert!(layers[0] > layers[1], "outline must win coplanar backing");
        let reversed = launch_corridor_depth_layers(&[entries[1], entries[0]]);
        assert_eq!(layers[0], reversed[1]);
        assert_eq!(layers[1], reversed[0]);
    }
}

#[cfg(test)]
mod shadow_style_tests {
    use super::ShadowStyle;

    #[test]
    fn default_shadow_setting_renders_smooth_shadows() {
        assert_eq!(ShadowStyle::default(), ShadowStyle::Smooth);
        assert_ne!(ShadowStyle::default(), ShadowStyle::Disabled);
    }

    #[test]
    fn shadow_opt_outs_remain_explicit() {
        assert_eq!(ShadowStyle::from_config_value(0), ShadowStyle::Disabled);
        assert_eq!(ShadowStyle::from_config_value(2), ShadowStyle::RetailDithered);
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
    const EXPECTED_PLAYER_SPRITE_SCALE: f32 = 0.625;
    const MATRIX_EPSILON: f32 = 0.000_01;

    #[test]
    fn object_color_policy_matches_semantic_draw_flags() {
        const AUTHORED_TABLE: u16 = crate::color_data::COLOR_TABLE_DEFAULT_C;

        let resolved = |strategy_flags, scaled_sprite| {
            ObjectColorPolicy::from_strategy_flags(strategy_flags, scaled_sprite)
                .resolve(AUTHORED_TABLE)
        };

        assert_eq!(resolved(0, false), AUTHORED_TABLE);
        assert_eq!(
            resolved(STRATEGY_COLOR_SPECIAL, false),
            crate::color_data::COLOR_TABLE_ID_1_C
        );
        assert_eq!(
            resolved(STRATEGY_COLOR_HIT_FLASH, false),
            crate::color_data::COLOR_TABLE_WHITE_C
        );
        assert_eq!(
            resolved(STRATEGY_COLOR_SPECIAL | STRATEGY_COLOR_HIT_FLASH, false),
            crate::color_data::COLOR_TABLE_RED_C
        );
        assert_eq!(resolved(STRATEGY_COLOR_HIT_FLASH, true), AUTHORED_TABLE);
        assert_eq!(
            resolved(STRATEGY_COLOR_SPECIAL | STRATEGY_COLOR_HIT_FLASH, true),
            AUTHORED_TABLE
        );
    }

    #[test]
    fn source_shadow_remains_visible_during_polygon_debris() {
        let debris = DrawListEntry {
            flags: DL_FLAG_VISIBLE | DL_FLAG_SHADOW,
            explosion_cnt: 7,
            ..DrawListEntry::default()
        };

        assert!(has_source_shadow(&debris));
    }

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
        let mut shapes = ShapeStore::new();
        let mesh = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|shape| shape.shape_id == MEDIUM_EXPLOSION_SPRITE_SHAPE)
            .unwrap();
        assert!(shapes.register(mesh.shape_id, mesh.vertices, mesh.faces));
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
            &shapes,
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
            (10, 1.0),
        ] {
            assert!(
                (camera_space[index] - expected).abs() <= MATRIX_EPSILON,
                "matrix[{index}]={} expected {expected}",
                camera_space[index]
            );
        }
    }

    #[test]
    fn launch_sprite_width_uses_doubled_extent_and_signed_adjustment() {
        const BOOST_SHAPE: u16 = crate::shape_data::SHAPE_EXT_BOOSTSHAPE;
        const BOOST_SIZE_CASES: [(u8, f32); 3] = [(0, 40.0), (255, 38.0), (251, 30.0)];
        let mesh = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|shape| shape.shape_id == BOOST_SHAPE)
            .unwrap();
        let mut shapes = ShapeStore::new();
        assert!(shapes.register(mesh.shape_id, mesh.vertices, mesh.faces));
        let mut view = [0.0; 16];
        crate::transform::identity(&mut view);
        for (adjustment, expected_width) in BOOST_SIZE_CASES {
            let mut model = view;
            apply_scaled_sprite_model(&mut model, &view, &shapes, BOOST_SHAPE, adjustment);
            for (axis, scale) in [(0, model[0]), (1, model[5])] {
                let coordinates: Vec<_> = mesh
                    .vertices
                    .iter()
                    .map(|vertex| if axis == 0 { vertex.x } else { vertex.y })
                    .collect();
                let span = coordinates
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max)
                    - coordinates.iter().copied().fold(f32::INFINITY, f32::min);
                assert!((span * scale - expected_width).abs() <= MATRIX_EPSILON);
            }
        }
    }
}
