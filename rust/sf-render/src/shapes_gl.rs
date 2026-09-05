//! GPU shape store: upload, per-face flat-color rendering, GSU shading.
//!
//! Port (C oracle): the runtime half of `src/renderer/shapes.c`
//! (`Shapes_Register`/`UploadShape`/`BuildFaceNormals`/`Shapes_Render`/
//! `Shapes_RegisterBuiltins`). The pure color math (material resolution,
//! shade index, depth banks) lives in [`crate::shapes`] and is wired in
//! here exactly like the C `Shapes_Render` loop.

use crate::gpu::{Gpu, TextureId, Vertex3, Vertex3Tex};
use crate::light_data::SHADE_TABLE_LEN;
use crate::shape_data::{self, ShapeFace, ShapePainterNode, ShapeVertex};
use crate::shapes::{self, DEPTHZ_COUNT, DEPTHZ_NORMAL, SHAPE_COLTAB_ID_0};
use crate::source_projection::{self, ProjectedPoint, SourcePose};
use crate::source_raster::SourceRaster;
use crate::transform::Transform;
use sf_core::scene::{DepthColors, SceneStyle};
use std::collections::HashMap;

pub const MAX_SHAPES: usize = 512;

/// Source scaled sprites are screen-facing images, not polygons selected
/// by a model-space visibility triangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeRenderMode {
    Polygons,
    ScaledSprite,
}

/// Render-only endpoints. The game and reference raster keep integer frames.
#[derive(Clone, Copy, Debug)]
pub struct MeshAnimationBlend {
    pub previous_frame: u8,
    pub current_frame: u8,
    pub amount: f32,
}
const SOURCE_LOCAL_FACE_INDICES: [u16; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const SOURCE_LOD_DEPTH_THRESHOLDS: [i16; 3] = [1_000, 2_000, 3_000];
const SOURCE_ANIMATION_COUNTER_MASK: u8 = 63;

/// A scaled sprite is one upright square image. MDSPRITE derives both
/// dimensions from the low byte of the texture mask, ignoring polygon UV
/// layout and scroll fields (the latter carry the sprite size adjustment).
fn scaled_sprite_mesh(
    shape: &GpuShape,
    source_size: u16,
    texture_offset: u16,
) -> Option<[Vertex3Tex; 6]> {
    let first = shape.vertices.first()?;
    let mut left = first.x;
    let mut right = first.x;
    let mut bottom = first.y;
    let mut top = first.y;
    for vertex in &shape.vertices {
        left = left.min(vertex.x);
        right = right.max(vertex.x);
        bottom = bottom.min(vertex.y);
        top = top.max(vertex.y);
    }
    if left == right || bottom == top || source_size == 0 {
        return None;
    }
    let edge = f32::from(source_size);
    let coordinate_mask = source_size - 1;
    let square_mask = coordinate_mask | (coordinate_mask << 8);
    let corners = [
        ([left, top, 0.0], [0.0, 0.0]),
        ([right, top, 0.0], [edge, 0.0]),
        ([right, bottom, 0.0], [edge, edge]),
        ([left, bottom, 0.0], [0.0, edge]),
    ];
    Some([0, 1, 2, 0, 2, 3].map(|index| {
        let (pos, [x, y]) = corners[index];
        Vertex3Tex {
            pos,
            tex_info: [x, y, f32::from(texture_offset), f32::from(square_mask)],
        }
    }))
}

fn sf1_animation_frame_index(animation_frame: u8, frame_count: usize) -> usize {
    usize::from(animation_frame & SOURCE_ANIMATION_COUNTER_MASK) % frame_count
}

fn source_lod_index(object_depth: i16) -> Option<usize> {
    SOURCE_LOD_DEPTH_THRESHOLDS
        .iter()
        .rposition(|threshold| object_depth >= *threshold)
}

const SOURCE_TEX_BANK_01: &[u8; 32768] =
    include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_01.BIN");
const TEX_BANK_23: &[u8; 32768] =
    include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_23.BIN");
const RETAIL_REVISION_2_PILLAR_TEXEL_ADDRESS: usize = 0x0CC5;
const RETAIL_REVISION_2_PILLAR_TEXEL: u8 = 0x59;

const fn retail_revision_2_texture_bank_01() -> [u8; 32_768] {
    let mut texture = *SOURCE_TEX_BANK_01;
    texture[RETAIL_REVISION_2_PILLAR_TEXEL_ADDRESS] = RETAIL_REVISION_2_PILLAR_TEXEL;
    texture
}

/// The Rev-2 retail ROM differs from the reconstructed source asset at one
/// packed pillar texel. Keep that observed target revision explicit while the
/// reference tree remains an unmodified static-analysis input.
const TEX_BANK_01: [u8; 32_768] = retail_revision_2_texture_bank_01();

#[derive(Clone, Copy)]
struct TextureLayout {
    mask: u16,
    coords: [[u8; 2]; 4],
}

/// Byte-exact `DEFSPR.ASM` texturexy0..8 records, in polygon-vertex order.
/// `MDRAWP.MC` applies the mask to interpolated local coordinates before it
/// adds the sprite's linear ROM address.
const TEXTURE_XY: [TextureLayout; 9] = [
    TextureLayout {
        mask: 0x1f1f,
        coords: [[0, 0], [31, 0], [31, 31], [0, 31]],
    },
    TextureLayout {
        mask: 0x3f3f,
        coords: [[63, 0], [0, 0], [0, 63], [63, 63]],
    },
    TextureLayout {
        mask: 0x0f7f,
        coords: [[0, 0], [0, 15], [15, 15], [15, 0]],
    },
    TextureLayout {
        mask: 0x1f1f,
        coords: [[0, 0], [127, 0], [127, 31], [0, 31]],
    },
    TextureLayout {
        mask: 0x0f3f,
        coords: [[0, 0], [63, 0], [63, 15], [0, 15]],
    },
    TextureLayout {
        mask: 0x0f3f,
        coords: [[63, 0], [0, 0], [0, 15], [63, 15]],
    },
    TextureLayout {
        mask: 0x3f3f,
        coords: [[0, 0], [63, 0], [63, 63], [0, 63]],
    },
    TextureLayout {
        mask: 0x077f,
        coords: [[31, 0], [0, 0], [0, 7], [31, 7]],
    },
    TextureLayout {
        mask: 0x1f1f,
        coords: [[31, 0], [0, 0], [0, 31], [31, 31]],
    },
];

/// CPU mirror of `merge; and rmask; add rspdata`, used by the tests to keep
/// the shader's address arithmetic pinned to the Super FX routine.
#[cfg(test)]
fn texture_address(base: u16, mask: u16, local_x: i32, local_y: i32) -> u16 {
    let x = (local_x & i32::from(mask & 0xff)) as u16;
    let y = (local_y & i32::from(mask >> 8)) as u16;
    base.wrapping_add(y << 8).wrapping_add(x) & 0x7fff
}

/// Decode the two games' distinct packed-texture control-bit layouts into
/// ordinary flat asset indices. SF1 stores high-nibble mode in material bit
/// 13; SF2 stores it in sprite-index bit 7 and retains six layout bits.
const fn texture_material_fields(is_sf2: bool, material: u16) -> (usize, usize, bool) {
    if is_sf2 {
        (
            ((material >> 8) & 0x3F) as usize,
            (material & 0xFF) as usize,
            material & 0x0080 != 0,
        )
    } else {
        (
            ((material >> 8) & 0x1F) as usize,
            (material & 0xFF) as usize,
            material & 0x2000 != 0,
        )
    }
}

/// A registered shape with CPU-side vertex buffers (C `Shape` with
/// `gpu_ready`). Instead of a VBO, triangle and line vertices are kept as
/// plain `Vertex3` vectors and pushed to the retained `Gpu` per draw.
#[derive(Clone)]
pub struct GpuShape {
    /// Generated ShapeHdr `sh_col_ptr`, represented by color_data table id.
    pub default_color_table: u16,
    /// SF2 stores an address into its bank-$01 color data; SF1 stores a
    /// generated table id. This selects the matching material resolver.
    pub is_sf2: bool,
    pub vertices: Vec<ShapeVertex>,
    pub faces: Vec<ShapeFace>,
    /// Authored back-to-front hierarchy for source-resolution rendering.
    pub painter_nodes: Vec<ShapePainterNode>,
    /// Starts of adjacent vertex pairs sharing an authored X reflection.
    pub reflected_pair_starts: Vec<u16>,
    /// Derived flat normals, one per face (zero for degenerate faces).
    pub face_normals: Vec<[f32; 3]>,
    /// All fan-triangulated triangle vertices (3 per triangle), face order.
    pub tri_verts: Vec<Vertex3>,
    /// All Face2 line-segment vertices (2 per segment), face order.
    pub line_verts: Vec<Vertex3>,
    pub num_triangles: i32,
    pub face_tri_start: Vec<i32>,
    pub face_tri_count: Vec<i32>,
    pub num_line_verts: i32,
    /// Per-face first line vertex within `line_verts`, -1 = not a line face.
    pub face_line_first: Vec<i32>,
}

struct SourceLodShape {
    base: GpuShape,
    animation_frames: Vec<GpuShape>,
}

impl SourceLodShape {
    fn frame(&self, animation_frame: u8) -> &GpuShape {
        if self.animation_frames.is_empty() {
            &self.base
        } else {
            &self.animation_frames
                [sf1_animation_frame_index(animation_frame, self.animation_frames.len())]
        }
    }
}

/// Mirror of `BuildFaceNormals`: cross-product normals flipped to point
/// away from the mesh centroid.
fn build_face_normals(vertices: &[ShapeVertex], faces: &[ShapeFace]) -> Vec<[f32; 3]> {
    let mut centroid = [0.0f32; 3];
    for v in vertices {
        centroid[0] += v.x;
        centroid[1] += v.y;
        centroid[2] += v.z;
    }
    if !vertices.is_empty() {
        let inv = 1.0 / vertices.len() as f32;
        centroid[0] *= inv;
        centroid[1] *= inv;
        centroid[2] *= inv;
    }

    let mut normals = vec![[0.0f32; 3]; faces.len()];
    for (i, face) in faces.iter().enumerate() {
        if face.num_verts < 3 {
            continue;
        }
        let (i0, i1, i2) = (
            face.vertex_indices[0] as usize,
            face.vertex_indices[1] as usize,
            face.vertex_indices[2] as usize,
        );
        if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() {
            continue;
        }
        let (v0, v1, v2) = (&vertices[i0], &vertices[i1], &vertices[i2]);
        let (ux, uy, uz) = (v1.x - v0.x, v1.y - v0.y, v1.z - v0.z);
        let (vx, vy, vz) = (v2.x - v0.x, v2.y - v0.y, v2.z - v0.z);
        let nx = uy * vz - uz * vy;
        let ny = uz * vx - ux * vz;
        let nz = ux * vy - uy * vx;
        let length = (nx * nx + ny * ny + nz * nz).sqrt();
        if length <= 0.0001 {
            continue;
        }
        let mut n = [nx / length, ny / length, nz / length];

        let fc = [
            (v0.x + v1.x + v2.x) / 3.0,
            (v0.y + v1.y + v2.y) / 3.0,
            (v0.z + v1.z + v2.z) / 3.0,
        ];
        if (fc[0] - centroid[0]) * n[0]
            + (fc[1] - centroid[1]) * n[1]
            + (fc[2] - centroid[2]) * n[2]
            < 0.0
        {
            n = [-n[0], -n[1], -n[2]];
        }
        normals[i] = n;
    }
    normals
}

/// Normalize each authored source normal, deriving geometry only for the
/// deliberate zero-normal records used by unlit faces and line primitives.
fn resolve_face_normals(vertices: &[ShapeVertex], faces: &[ShapeFace]) -> Vec<[f32; 3]> {
    let derived = build_face_normals(vertices, faces);
    faces
        .iter()
        .enumerate()
        .map(|(index, face)| {
            let normal = face.normal.map(f32::from);
            let length =
                (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            if length <= 0.0001 {
                derived[index]
            } else {
                [normal[0] / length, normal[1] / length, normal[2] / length]
            }
        })
        .collect()
}

fn source_explosion_face_indices(face: &ShapeFace, base_points: &[ProjectedPoint]) -> Vec<u16> {
    let mut indices = face.vertex_indices[..usize::from(face.num_verts)].to_vec();
    if face
        .visibility_vertices
        .is_some_and(|visibility| !source_projection::face_is_visible(base_points, visibility))
    {
        indices.reverse();
    }
    indices
}

/// Mirror of `Shapes_BuildExplodedModelMatrix` (`mexpfacesinit`).
fn build_exploded_model_matrix(
    base_model: &[f32; 16],
    face_normal: &[f32; 3],
    explosion_state: u8,
) -> [f32; 16] {
    let rotated_x = base_model[0] * face_normal[0]
        + base_model[4] * face_normal[1]
        + base_model[8] * face_normal[2];
    let rotated_y = base_model[1] * face_normal[0]
        + base_model[5] * face_normal[1]
        + base_model[9] * face_normal[2];
    let rotated_z = base_model[2] * face_normal[0]
        + base_model[6] * face_normal[1]
        + base_model[10] * face_normal[2];
    let scale = (explosion_state as f32 * 127.0) / 4.0;

    let mut out = *base_model;
    out[12] += rotated_x * scale;
    out[13] += -rotated_y.abs() * scale;
    out[14] += rotated_z * scale;
    out
}

/// Rotate the fixed world light into object space (`m_rotlightx/y/z`,
/// MOBJ.MC:905-922). The model matrix is pure rotation+translation, so the
/// transpose of its upper 3x3 is the inverse rotation.
fn compute_object_light(model_matrix: &[f32; 16]) -> [f32; 3] {
    let l = shapes::LIGHT_DIR;
    let mut out = [0.0f32; 3];
    for i in 0..3 {
        out[i] = model_matrix[i * 4] * l[0]
            + model_matrix[i * 4 + 1] * l[1]
            + model_matrix[i * 4 + 2] * l[2];
    }
    out
}

/// Keep homogeneous coordinates for face orientation. Dividing by depth
/// changes winding when a selector crosses the eye plane; accepting every
/// such face instead draws back faces over the inside of a corridor.
fn project_visibility_point(vertex: &ShapeVertex, pvm: &[f32; 16]) -> Option<[f64; 3]> {
    let clip_x = pvm[0] * vertex.x + pvm[4] * vertex.y + pvm[8] * vertex.z + pvm[12];
    let clip_y = pvm[1] * vertex.x + pvm[5] * vertex.y + pvm[9] * vertex.z + pvm[13];
    let clip_w = pvm[3] * vertex.x + pvm[7] * vertex.y + pvm[11] * vertex.z + pvm[15];
    if !clip_x.is_finite() || !clip_y.is_finite() || !clip_w.is_finite() {
        return None;
    }

    Some([f64::from(clip_x), f64::from(clip_y), f64::from(clip_w)])
}

/// Source `msh_vizis` records one independently selected triangle per face.
/// The source visibility result includes the front-of-camera bit, so an
/// in-front source polygon is visible when its projected winding is positive.
/// The renderer's complete source-to-clip basis preserves that sign; applying
/// an extra screen-space reflection here selects the retail back faces.
fn face_is_camera_visible(shape: &GpuShape, face: &ShapeFace, pvm: &[f32; 16]) -> bool {
    let Some(indices) = face.visibility_vertices else {
        return true;
    };
    let Some(a) = shape
        .vertices
        .get(usize::from(indices[0]))
        .and_then(|vertex| project_visibility_point(vertex, pvm))
    else {
        return true;
    };
    let Some(b) = shape
        .vertices
        .get(usize::from(indices[1]))
        .and_then(|vertex| project_visibility_point(vertex, pvm))
    else {
        return true;
    };
    let Some(c) = shape
        .vertices
        .get(usize::from(indices[2]))
        .and_then(|vertex| project_visibility_point(vertex, pvm))
    else {
        return true;
    };

    // This determinant has the same sign as projected winding when all
    // points are in front, and remains defined at zero or mixed-sign depth.
    // GPU clipping still decides which part of the selected face is visible.
    let signed_area = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0]);
    signed_area > 0.0
}

fn resolve_registered_face_material(
    shape: &GpuShape,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
) -> Option<u16> {
    if shape.is_sf2 {
        let table = if color_table == 0 {
            shape.default_color_table
        } else {
            color_table
        };
        let material = sf2_data::colors::material_at(table, face_color_index)?;
        sf2_data::colors::resolve_animated_material(material, col_frame)
    } else {
        shapes::resolve_face_material_from_table(
            shape.default_color_table,
            face_color_index,
            col_frame,
            color_table,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_registered_face_color(
    shape: &GpuShape,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
    depth_colors: DepthColors,
    palette: &shapes::ShapePaletteRgb,
) -> [f32; 4] {
    if shape.is_sf2 {
        let Some(material) =
            resolve_registered_face_material(shape, face_color_index, col_frame, color_table)
        else {
            return shapes::DEBUG_MATERIAL_COLOR;
        };
        shapes::resolve_sf2_material_color_in(material, col_frame, shade_index, depth_bank, palette)
    } else {
        shapes::resolve_face_color_from_table_for_scene(
            shape.default_color_table,
            face_color_index,
            col_frame,
            color_table,
            shade_index,
            depth_bank,
            depth_colors,
            palette,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_registered_face_palette_pair(
    shape: &GpuShape,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
    depth_colors: DepthColors,
) -> Option<shapes::PalettePair> {
    let material =
        resolve_registered_face_material(shape, face_color_index, col_frame, color_table)?;
    if shape.is_sf2 {
        shapes::resolve_sf2_material_palette_pair(material, col_frame, shade_index, depth_bank)
    } else {
        shapes::resolve_material_palette_pair_for_scene(
            material,
            col_frame,
            shade_index,
            depth_bank,
            depth_colors,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_registered_face_smooth_color(
    shape: &GpuShape,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_blend: shapes::DepthBankBlend,
    depth_colors: DepthColors,
    palette: &shapes::ShapePaletteRgb,
) -> [f32; 4] {
    let color_at = |depth_bank: shapes::DepthBank| {
        let depth_bank = depth_bank.source_index();
        resolve_registered_face_palette_pair(
            shape,
            face_color_index,
            col_frame,
            color_table,
            shade_index,
            depth_bank,
            depth_colors,
        )
        .map(|pair| shapes::decode_palette_pair_in(pair.packed(), palette))
        .unwrap_or_else(|| {
            resolve_registered_face_color(
                shape,
                face_color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
                depth_colors,
                palette,
            )
        })
    };
    let near = color_at(depth_blend.near_bank);
    if depth_blend.near_bank == depth_blend.far_bank {
        return near;
    }
    let far = color_at(depth_blend.far_bank);
    std::array::from_fn(|channel| {
        near[channel] + (far[channel] - near[channel]) * depth_blend.amount
    })
}

pub struct ShapeStore {
    shapes: Vec<Option<GpuShape>>,
    /// Fully decompiled SF1 vertex streams, keyed by the flat native shape
    /// id. Each vector includes frame zero and follows the source Frames-table
    /// period; no source addresses or shape-language state reach rendering.
    sf1_animation_frames: HashMap<u16, Vec<GpuShape>>,
    sf1_source_lods: HashMap<u16, [Option<SourceLodShape>; 3]>,
    sf2_shapes: HashMap<u16, GpuShape>,
    sf2_animation_frames: HashMap<u16, Vec<GpuShape>>,
    depthz_table: usize,
    depth_colors: DepthColors,
    sf1_texture_banks: [Option<TextureId>; 2],
    sf2_texture_banks: [Option<TextureId>; 3],
}

impl ShapeStore {
    pub fn new() -> Self {
        let mut shapes = Vec::with_capacity(MAX_SHAPES);
        shapes.resize_with(MAX_SHAPES, || None);
        ShapeStore {
            shapes,
            sf1_animation_frames: HashMap::new(),
            sf1_source_lods: HashMap::new(),
            sf2_shapes: HashMap::new(),
            sf2_animation_frames: HashMap::new(),
            depthz_table: DEPTHZ_NORMAL,
            depth_colors: DepthColors::Night,
            sf1_texture_banks: [None, None],
            sf2_texture_banks: [None, None, None],
        }
    }

    /// Mirror of `Shapes_SetDepthTable` (`set_zdepthtable` map macro).
    pub fn set_depth_table(&mut self, table: usize) {
        self.depthz_table = if table >= DEPTHZ_COUNT {
            DEPTHZ_NORMAL
        } else {
            table
        };
    }

    pub fn set_scene_style(&mut self, style: SceneStyle) {
        self.depthz_table = shapes::depth_threshold_index(style.depth_thresholds);
        self.depth_colors = style.depth_colors;
    }

    /// Mirror of `Shapes_Get` (resolves raw shape words like the C).
    pub fn get(&self, shape_id: u16) -> Option<&GpuShape> {
        if let Some(shape) = self.sf2_shapes.get(&shape_id) {
            return Some(shape);
        }
        let id = shapes::resolve_shape_word(shape_id) as usize;
        if id >= MAX_SHAPES {
            return None;
        }
        self.shapes[id].as_ref()
    }

    /// Per-axis collision half-extents, matching the source `ShapeHdr`
    /// `sh_xmax`/`sh_ymax`/`sh_zmax` fields for SF1. Mesh-derived bounds are
    /// retained only for native/runtime shapes outside that catalog. Resolves
    /// raw shape words like [`ShapeStore::get`].
    pub fn shape_half_extents(&self, shape_id: u16) -> Option<(i16, i16, i16)> {
        if !self.sf2_shapes.contains_key(&shape_id) {
            let flat_shape_id = shapes::resolve_shape_word(shape_id);
            if let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(flat_shape_id) {
                return Some((
                    metrics.half_extents[0],
                    metrics.half_extents[1],
                    metrics.half_extents[2],
                ));
            }
        }
        let shape = self.get(shape_id)?;
        if shape.vertices.is_empty() {
            return None;
        }
        let (mut mx, mut my, mut mz) = (0.0f32, 0.0f32, 0.0f32);
        let mut include = |candidate: &GpuShape| {
            for vertex in &candidate.vertices {
                mx = mx.max(vertex.x.abs());
                my = my.max(vertex.y.abs());
                mz = mz.max(vertex.z.abs());
            }
        };
        include(shape);

        if let Some(frames) = self.sf2_animation_frames.get(&shape_id) {
            for frame in frames {
                include(frame);
            }
        } else {
            let flat_shape_id = shapes::resolve_shape_word(shape_id);
            if let Some(frames) = self.sf1_animation_frames.get(&flat_shape_id) {
                for frame in frames {
                    include(frame);
                }
            }
        }
        let clamp = |f: f32| f.round().clamp(0.0, i16::MAX as f32) as i16;
        Some((clamp(mx), clamp(my), clamp(mz)))
    }

    /// Build a table of collision half-extents for every registered shape,
    /// keyed by internal shape id (0..MAX_SHAPES). Injected into the game's
    /// collision system (C `load_collision_extents`); shapes absent from the
    /// table keep the coldet 20/20/20 fallback.
    pub fn all_shape_half_extents(&self) -> std::collections::HashMap<u16, (i16, i16, i16)> {
        let mut table = shapes::sf1_shape_half_extents();
        for (id, slot) in self.shapes.iter().enumerate() {
            if slot.is_some() {
                if let Some(extents) = self.shape_half_extents(id as u16) {
                    table.insert(id as u16, extents);
                }
            }
        }
        for &id in self.sf2_shapes.keys() {
            if let Some(extents) = self.shape_half_extents(id) {
                table.insert(id, extents);
            }
        }
        table
    }

    /// Mirror of `Shapes_Register` + `UploadShape`: fan-triangulate faces and
    /// collect Face2 line segments into CPU vertex buffers (`tri_verts` then
    /// `line_verts`) for the retained `Gpu` draw path.
    pub fn register(&mut self, shape_id: u16, verts: &[ShapeVertex], faces: &[ShapeFace]) -> bool {
        self.register_with_color(shape_id, verts, faces, SHAPE_COLTAB_ID_0 as u16)
    }

    fn build_gpu_shape(
        verts: &[ShapeVertex],
        faces: &[ShapeFace],
        painter_nodes: &[ShapePainterNode],
        reflected_pair_starts: &[u16],
        default_color_table: u16,
        is_sf2: bool,
    ) -> GpuShape {
        let vertices: Vec<ShapeVertex> = verts.to_vec();
        let faces: Vec<ShapeFace> = faces.to_vec();
        let face_normals = resolve_face_normals(&vertices, &faces);

        // Count total triangles (fan triangulation: n-gon -> n-2 triangles).
        let mut face_tri_start = Vec::with_capacity(faces.len());
        let mut face_tri_count = Vec::with_capacity(faces.len());
        let mut total_tris: i32 = 0;
        for face in &faces {
            face_tri_start.push(total_tris);
            let ntris = (face.num_verts as i32 - 2).max(0);
            face_tri_count.push(ntris);
            total_tris += ntris;
        }

        // Wireframe (Face2) segments: index within `line_verts` (0-based).
        let mut face_line_first = Vec::with_capacity(faces.len());
        let mut line_count: i32 = 0;
        for face in &faces {
            if face.num_verts == 2 {
                face_line_first.push(line_count);
                line_count += 2;
            } else {
                face_line_first.push(-1);
            }
        }

        // Expand fan triangles (3 verts/tri).
        let mut tri_verts: Vec<Vertex3> = Vec::with_capacity((total_tris * 3) as usize);
        for (fi, face) in faces.iter().enumerate() {
            if face.num_verts < 3 {
                continue;
            }
            let v0 = face.vertex_indices[0] as usize;
            for t in 0..face_tri_count[fi] {
                let v1 = face.vertex_indices[t as usize + 1] as usize;
                let v2 = face.vertex_indices[t as usize + 2] as usize;
                if v0 >= vertices.len() || v1 >= vertices.len() || v2 >= vertices.len() {
                    // Skip invalid face (keep triangle slot as degenerate).
                    tri_verts.push(Vertex3 {
                        pos: [0.0, 0.0, 0.0],
                    });
                    tri_verts.push(Vertex3 {
                        pos: [0.0, 0.0, 0.0],
                    });
                    tri_verts.push(Vertex3 {
                        pos: [0.0, 0.0, 0.0],
                    });
                    continue;
                }
                for &vi in &[v0, v1, v2] {
                    let v = &vertices[vi];
                    tri_verts.push(Vertex3 {
                        pos: [v.x, v.y, v.z],
                    });
                }
            }
        }

        // Line segments (2 verts/segment), same order as `face_line_first`.
        let mut line_verts: Vec<Vertex3> = Vec::with_capacity(line_count as usize);
        for face in &faces {
            if face.num_verts != 2 {
                continue;
            }
            let mut lv0 = face.vertex_indices[0] as usize;
            let mut lv1 = face.vertex_indices[1] as usize;
            if lv0 >= vertices.len() || lv1 >= vertices.len() {
                lv0 = 0;
                lv1 = 0;
            }
            for &vi in &[lv0, lv1] {
                let v = &vertices[vi];
                line_verts.push(Vertex3 {
                    pos: [v.x, v.y, v.z],
                });
            }
        }

        GpuShape {
            default_color_table,
            is_sf2,
            vertices,
            faces,
            painter_nodes: painter_nodes.to_vec(),
            reflected_pair_starts: reflected_pair_starts.to_vec(),
            face_normals,
            tri_verts,
            line_verts,
            num_triangles: total_tris,
            face_tri_start,
            face_tri_count,
            num_line_verts: line_count,
            face_line_first,
        }
    }

    /// Deform matching authored vertices, never pixels or simulation state.
    /// Discontinuous animation changes and incompatible topology hold their
    /// ordinary discrete frame instead of inventing intermediate geometry.
    fn blended_sf2_animation(&self, shape_id: u16, blend: MeshAnimationBlend) -> Option<GpuShape> {
        if !blend.amount.is_finite() || blend.amount <= 0.0 || blend.amount >= 1.0 {
            return None;
        }
        let frames = self.sf2_animation_frames.get(&shape_id)?;
        let previous_index = usize::from(blend.previous_frame) % frames.len();
        let current_index = usize::from(blend.current_frame) % frames.len();
        if previous_index == current_index
            || ((previous_index + 1) % frames.len() != current_index
                && (current_index + 1) % frames.len() != previous_index)
        {
            return None;
        }
        let previous = &frames[previous_index];
        let current = &frames[current_index];
        if previous.vertices.len() != current.vertices.len()
            || previous.faces.len() != current.faces.len()
            || previous.faces.iter().zip(&current.faces).any(|(a, b)| {
                a.num_verts != b.num_verts
                    || a.vertex_indices != b.vertex_indices
                    || a.visibility_vertices != b.visibility_vertices
                    || a.color_index != b.color_index
            })
        {
            return None;
        }
        let vertices: Vec<_> = previous
            .vertices
            .iter()
            .zip(&current.vertices)
            .map(|(a, b)| ShapeVertex {
                x: a.x + (b.x - a.x) * blend.amount,
                y: a.y + (b.y - a.y) * blend.amount,
                z: a.z + (b.z - a.z) * blend.amount,
            })
            .collect();
        Some(Self::build_gpu_shape(
            &vertices,
            &previous.faces,
            &previous.painter_nodes,
            &previous.reflected_pair_starts,
            previous.default_color_table,
            true,
        ))
    }

    pub(crate) fn sf2_animation_geometry(
        &self,
        shape_id: u16,
        frame: u8,
        blend: Option<MeshAnimationBlend>,
    ) -> Option<std::borrow::Cow<'_, GpuShape>> {
        let frames = self.sf2_animation_frames.get(&shape_id)?;
        Some(
            match blend.and_then(|blend| self.blended_sf2_animation(shape_id, blend)) {
                Some(shape) => std::borrow::Cow::Owned(shape),
                None => std::borrow::Cow::Borrowed(&frames[usize::from(frame) % frames.len()]),
            },
        )
    }

    pub fn register_with_color(
        &mut self,
        shape_id: u16,
        verts: &[ShapeVertex],
        faces: &[ShapeFace],
        default_color_table: u16,
    ) -> bool {
        let id = shape_id as usize;
        if id >= MAX_SHAPES {
            return false;
        }
        self.shapes[id] = Some(Self::build_gpu_shape(
            verts,
            faces,
            &[],
            &[],
            default_color_table,
            false,
        ));
        true
    }

    /// Register all exact SF2 ShapeHdr meshes by their native bank-$00
    /// address tokens. Coordinates are expanded with the header's fixed-point
    /// shift, and authored face normals are retained for GSU lighting.
    pub fn register_sf2_shapes(&mut self) {
        self.sf2_shapes.clear();
        self.sf2_animation_frames.clear();
        for entry in &sf2_data::shape_data::SHAPE_DATA {
            let scale = (1u32 << entry.shift) as f32;
            let convert_vertices = |vertices: &[sf2_data::shape_data::ShapeVertex]| {
                vertices
                    .iter()
                    .map(|v| ShapeVertex {
                        x: f32::from(v.x) * scale,
                        // Source shape coordinates use screen-down Y; the
                        // shared renderer mesh convention is screen-up Y.
                        y: -f32::from(v.y) * scale,
                        z: f32::from(v.z) * scale,
                    })
                    .collect::<Vec<_>>()
            };
            let vertices = convert_vertices(entry.vertices);
            let faces: Vec<ShapeFace> = entry
                .faces
                .iter()
                .map(|face| ShapeFace {
                    vertex_indices: face.vertex_indices,
                    num_verts: face.num_verts,
                    color_index: face.color_index,
                    normal: [
                        i16::from(face.normal[0]),
                        -i16::from(face.normal[1]),
                        i16::from(face.normal[2]),
                    ],
                    visibility_vertices: face.visibility_vertices,
                })
                .collect();
            let shape = Self::build_gpu_shape(&vertices, &faces, &[], &[], entry.color_table, true);
            let flat_shape_id = sf_core::shape::sf2_shape_id(entry.header_index);
            self.sf2_shapes.insert(flat_shape_id, shape);
            if !entry.animation_frames.is_empty() {
                let frames = entry
                    .animation_frames
                    .iter()
                    .map(|vertices| {
                        Self::build_gpu_shape(
                            &convert_vertices(vertices),
                            &faces,
                            &[],
                            &[],
                            entry.color_table,
                            true,
                        )
                    })
                    .collect();
                self.sf2_animation_frames.insert(flat_shape_id, frames);
            }
        }
    }

    /// Load every compiled ASM shape and its complete typed animation frames.
    /// Geometry and face visibility come directly from the retail sources;
    /// no hand-authored runtime mesh overrides are involved.
    pub fn register_builtins(&mut self, gpu: &mut Gpu) {
        self.sf1_texture_banks = [
            Some(gpu.create_texture_r8(256, 128, &TEX_BANK_01)),
            Some(gpu.create_texture_r8(256, 128, TEX_BANK_23)),
        ];
        self.sf2_texture_banks = [
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_0)),
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_1)),
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_2)),
        ];
        self.sf1_animation_frames.clear();
        self.sf1_source_lods.clear();
        for entry in shape_data::SHAPE_DATA.iter() {
            let table = crate::color_data::table_id_by_name(entry.default_color_table)
                .unwrap_or(SHAPE_COLTAB_ID_0 as u16);
            let id = usize::from(entry.shape_id);
            if id < MAX_SHAPES {
                self.shapes[id] = Some(Self::build_gpu_shape(
                    entry.vertices,
                    entry.faces,
                    entry.painter_nodes,
                    entry.reflected_pair_starts,
                    table,
                    false,
                ));
            }
            if !entry.animation_frames.is_empty() {
                let frames = entry
                    .animation_frames
                    .iter()
                    .map(|vertices| {
                        Self::build_gpu_shape(
                            vertices,
                            entry.faces,
                            entry.painter_nodes,
                            entry.reflected_pair_starts,
                            table,
                            false,
                        )
                    })
                    .collect();
                self.sf1_animation_frames.insert(entry.shape_id, frames);
            }
            if entry.source_lods.iter().any(Option::is_some) {
                let source_lods = entry.source_lods.map(|lod| {
                    lod.map(|lod| SourceLodShape {
                        base: Self::build_gpu_shape(
                            lod.vertices,
                            lod.faces,
                            lod.painter_nodes,
                            lod.reflected_pair_starts,
                            table,
                            false,
                        ),
                        animation_frames: lod
                            .animation_frames
                            .iter()
                            .map(|vertices| {
                                Self::build_gpu_shape(
                                    vertices,
                                    lod.faces,
                                    lod.painter_nodes,
                                    lod.reflected_pair_starts,
                                    table,
                                    false,
                                )
                            })
                            .collect(),
                    })
                });
                self.sf1_source_lods.insert(entry.shape_id, source_lods);
            }
        }
        self.register_sf2_shapes();
    }

    fn object_depth(view: &[f32; 16], model_matrix: &[f32; 16]) -> f32 {
        let x = model_matrix[12];
        let y = model_matrix[13];
        let z = model_matrix[14];
        // Camera looks down -Z in view space, so depth is the negated view Z.
        -(view[2] * x + view[6] * y + view[10] * z + view[14])
    }

    fn textured_face_vertices(
        &self,
        shape: &GpuShape,
        face: &ShapeFace,
        material: u16,
        texture_scroll: [u8; 2],
    ) -> Option<(Vec<Vertex3Tex>, TextureId, bool)> {
        let (layout_index, descriptor_index, high_nibble) =
            texture_material_fields(shape.is_sf2, material);
        let (mask, coords, offset, texture) = if shape.is_sf2 {
            let layout = sf2_data::textures::TEXTURE_LAYOUTS.get(layout_index)?;
            let descriptor = sf2_data::textures::TEXTURE_SPRITES.get(descriptor_index)?;
            if descriptor.bank == sf2_data::textures::UNUSED_TEXTURE_BANK {
                return None;
            }
            let texture = *self
                .sf2_texture_banks
                .get(descriptor.bank as usize)?
                .as_ref()?;
            (layout.mask, layout.coords, descriptor.offset, texture)
        } else {
            let layout = TEXTURE_XY.get(layout_index)?;
            let descriptor = crate::color_data::TEXTURE_SPRITES.get(descriptor_index)?;
            let texture = *self
                .sf1_texture_banks
                .get(descriptor.bank as usize)?
                .as_ref()?;
            (layout.mask, layout.coords, descriptor.offset, texture)
        };
        let n = face.num_verts as usize;
        if !(3..=4).contains(&n) {
            return None;
        }
        let mut out = Vec::with_capacity((n - 2) * 3);
        for tri in 0..n - 2 {
            for ordinal in [0, tri + 1, tri + 2] {
                let vi = face.vertex_indices[ordinal] as usize;
                let v = shape.vertices.get(vi)?;
                let [x, y] = coords[ordinal];
                // Keep the scroll in the continuous vertex domain. The
                // fragment shader applies the authored mask after linear
                // interpolation; wrapping each endpoint as u8 first turns a
                // small seam-crossing span (e.g. 250..258) into a long
                // backward ramp (250..2), which makes textured faces shimmer
                // as the scroll crosses a byte boundary.
                let x = continuous_texture_coord(x, texture_scroll[0]);
                let y = continuous_texture_coord(y, texture_scroll[1]);
                out.push(Vertex3Tex {
                    pos: [v.x, v.y, v.z],
                    tex_info: [x, y, offset as f32, mask as f32],
                });
            }
        }
        Some((out, texture, high_nibble))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_source_textured_face(
        &self,
        raster: &mut SourceRaster,
        points: &[ProjectedPoint],
        indices: &[u16],
        face: &ShapeFace,
        material: u16,
        texture_scroll: [u8; 2],
        palette: &[[f32; 4]; 16],
    ) -> bool {
        let (layout_index, descriptor_index, high_nibble) =
            texture_material_fields(false, material);
        let Some(layout) = TEXTURE_XY.get(layout_index) else {
            return false;
        };
        let Some(descriptor) = crate::color_data::TEXTURE_SPRITES.get(descriptor_index) else {
            return false;
        };
        let texture = match descriptor.bank {
            0 => TEX_BANK_01.as_slice(),
            1 => TEX_BANK_23.as_slice(),
            _ => return false,
        };
        let vertex_count = usize::from(face.num_verts);
        if !(3..=4).contains(&vertex_count) {
            return false;
        }
        raster.draw_textured_polygon(
            points,
            indices,
            &layout.coords[..vertex_count],
            texture,
            descriptor.offset,
            layout.mask,
            high_nibble,
            texture_scroll,
            palette,
        );
        true
    }

    fn source_face_order(shape: &GpuShape, points: &[ProjectedPoint]) -> Vec<usize> {
        if shape.painter_nodes.is_empty() {
            return (0..shape.faces.len()).collect();
        }

        fn append_group(order: &mut Vec<usize>, face_start: u16, face_count: u16, limit: usize) {
            let start = usize::from(face_start).min(limit);
            let end = start.saturating_add(usize::from(face_count)).min(limit);
            order.extend(start..end);
        }

        fn visit(
            shape: &GpuShape,
            points: &[ProjectedPoint],
            node_index: u16,
            depth: usize,
            order: &mut Vec<usize>,
        ) {
            if depth >= shape.painter_nodes.len() {
                return;
            }
            let Some(node) = shape.painter_nodes.get(usize::from(node_index)) else {
                return;
            };
            match *node {
                ShapePainterNode::Leaf {
                    face_start,
                    face_count,
                } => append_group(order, face_start, face_count, shape.faces.len()),
                ShapePainterNode::Partition {
                    visibility_vertices,
                    face_start,
                    face_count,
                    left,
                    right,
                } => {
                    let visible = visibility_vertices
                        .is_none_or(|indices| source_projection::face_is_visible(points, indices));
                    let visit_child = |child: Option<u16>, order: &mut Vec<usize>| {
                        if let Some(child) = child {
                            visit(shape, points, child, depth + 1, order);
                        }
                    };
                    if visible {
                        visit_child(left, order);
                        append_group(order, face_start, face_count, shape.faces.len());
                        visit_child(right, order);
                    } else {
                        visit_child(right, order);
                        visit_child(left, order);
                    }
                }
            }
        }

        let mut order = Vec::with_capacity(shape.faces.len());
        visit(shape, points, 0, 0, &mut order);
        order
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_source_scaled_sprite(
        &self,
        raster: &mut SourceRaster,
        shape_id: u16,
        col_frame: u8,
        color_table: u16,
        material_index: u8,
        size_adjustment: u8,
        pose: SourcePose,
        palette: &shapes::ShapePaletteRgb,
    ) -> bool {
        let flat_shape_id = shapes::resolve_shape_word(shape_id);
        let Some(shape) = self
            .shapes
            .get(usize::from(flat_shape_id))
            .and_then(Option::as_ref)
        else {
            return false;
        };
        if shape.is_sf2 {
            return false;
        }
        let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(flat_shape_id) else {
            return false;
        };
        let Some(projected) = source_projection::project_scaled_sprite(
            pose,
            metrics.visual_extent,
            metrics.coordinate_shift,
            size_adjustment,
        ) else {
            return false;
        };
        let Some(material) =
            resolve_registered_face_material(shape, material_index, col_frame, color_table)
        else {
            return false;
        };
        let (layout_index, descriptor_index, high_nibble) =
            texture_material_fields(false, material);
        let Some(layout) = TEXTURE_XY.get(layout_index) else {
            return false;
        };
        let Some(descriptor) = crate::color_data::TEXTURE_SPRITES.get(descriptor_index) else {
            return false;
        };
        let texture = match descriptor.bank {
            0 => TEX_BANK_01.as_slice(),
            1 => TEX_BANK_23.as_slice(),
            _ => return false,
        };
        let source_size = (layout.mask & 255) + 1;
        raster.set_face(crate::source_raster::NO_FACE);
        let texture_palette: [[f32; 4]; 16] = std::array::from_fn(|index| {
            [palette[index][0], palette[index][1], palette[index][2], 1.0]
        });
        raster.draw_scaled_sprite(
            projected.top_left,
            projected.size,
            source_size,
            texture,
            descriptor.offset,
            high_nibble,
            &texture_palette,
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn render_hd_scaled_sprite(
        &self,
        gpu: &mut Gpu,
        shape: &GpuShape,
        col_frame: u8,
        color_table: u16,
        material_index: u8,
        projection: &[f32; 16],
        view: &[f32; 16],
        model: &[f32; 16],
        palette: &[[f32; 4]; 16],
    ) {
        let Some(material) =
            resolve_registered_face_material(shape, material_index, col_frame, color_table)
        else {
            return;
        };
        let (layout_index, descriptor_index, high_nibble) =
            texture_material_fields(false, material);
        let Some(layout) = TEXTURE_XY.get(layout_index) else {
            return;
        };
        let Some(descriptor) = crate::color_data::TEXTURE_SPRITES.get(descriptor_index) else {
            return;
        };
        let Some(texture) = self
            .sf1_texture_banks
            .get(usize::from(descriptor.bank))
            .copied()
            .flatten()
        else {
            return;
        };
        let source_size = (layout.mask & u16::from(u8::MAX)) + 1;
        let Some(vertices) = scaled_sprite_mesh(shape, source_size, descriptor.offset) else {
            return;
        };
        gpu.push_textured_tris(
            &vertices,
            projection,
            view,
            model,
            palette,
            high_nibble,
            texture,
        );
    }

    fn source_shape_for_depth<'a>(
        &'a self,
        shape_id: u16,
        object_depth: i16,
        animation_frame: u8,
        base: &'a GpuShape,
    ) -> &'a GpuShape {
        let Some(lod_index) = source_lod_index(object_depth) else {
            return base;
        };
        self.sf1_source_lods
            .get(&shape_id)
            .and_then(|lods| lods[lod_index].as_ref())
            .map_or(base, |lod| lod.frame(animation_frame))
    }

    fn render_source_projected(
        &self,
        raster: &mut SourceRaster,
        shape_id: u16,
        shape: &GpuShape,
        animation_frame: u8,
        col_frame: u8,
        color_table: u16,
        object_depth_table: u8,
        texture_scroll: [u8; 2],
        pose: SourcePose,
        palette: &shapes::ShapePaletteRgb,
    ) -> bool {
        if shape.is_sf2 {
            return false;
        }
        let flat_shape_id = shapes::resolve_shape_word(shape_id);
        let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(flat_shape_id) else {
            return false;
        };
        let base_projection = source_projection::project_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        let shape = self.source_shape_for_depth(
            flat_shape_id,
            base_projection.object_depth,
            animation_frame,
            shape,
        );
        let projected = source_projection::project_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        if source_projection::shape_is_outside_playfield(&projected.points) {
            return true;
        }
        let depth_bank = shapes::select_object_depth_bank(
            f32::from(projected.object_depth),
            self.depthz_table,
            object_depth_table,
        );

        let texture_palette: [[f32; 4]; 16] = std::array::from_fn(|index| {
            [palette[index][0], palette[index][1], palette[index][2], 1.0]
        });
        for face_index in Self::source_face_order(shape, &projected.points) {
            let face = &shape.faces[face_index];
            if face.visibility_vertices.is_some_and(|indices| {
                !source_projection::face_is_visible(&projected.points, indices)
            }) {
                continue;
            }
            raster.set_face(face_index as u16);
            let authored_indices = &face.vertex_indices[..usize::from(face.num_verts)];
            let material =
                resolve_registered_face_material(shape, face.color_index, col_frame, color_table);
            let crosses_near_plane = authored_indices.iter().any(|index| {
                projected.view_points[usize::from(*index)][2] < source_projection::MIN_FRONT_DEPTH
            });
            if crosses_near_plane
                && material
                    .is_some_and(|material| material & 0xC000 == shapes::MATERIAL_COLTEXT_FLAG)
            {
                continue;
            }
            let clipped_points = crosses_near_plane.then(|| {
                source_projection::project_near_clipped_face(
                    &projected.view_points,
                    authored_indices,
                )
            });
            let (face_points, indices) = clipped_points.as_ref().map_or_else(
                || (projected.points.as_slice(), authored_indices),
                |points| {
                    (
                        points.as_slice(),
                        &SOURCE_LOCAL_FACE_INDICES[..points.len()],
                    )
                },
            );
            let minimum_vertices = if face.num_verts == 2 { 2 } else { 3 };
            if indices.len() < minimum_vertices {
                continue;
            }
            if let Some(material) =
                material.filter(|material| material & 0xC000 == shapes::MATERIAL_COLTEXT_FLAG)
            {
                self.render_source_textured_face(
                    raster,
                    face_points,
                    indices,
                    face,
                    material,
                    texture_scroll,
                    &texture_palette,
                );
                continue;
            }
            let shade_index =
                shapes::compute_quantized_shade_index(face.normal, projected.object_light);
            if let Some(pair) = resolve_registered_face_palette_pair(
                shape,
                face.color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
                self.depth_colors,
            ) {
                if face.num_verts == 2 {
                    raster.draw_palette_line(
                        face_points,
                        indices,
                        &texture_palette,
                        [pair.low, pair.high],
                    );
                } else {
                    raster.draw_palette_pair(
                        face_points,
                        indices,
                        &texture_palette,
                        [pair.low, pair.high],
                    );
                }
            } else {
                let color = resolve_registered_face_color(
                    shape,
                    face.color_index,
                    col_frame,
                    color_table,
                    shade_index,
                    depth_bank,
                    self.depth_colors,
                    palette,
                );
                if face.num_verts == 2 {
                    raster.draw_solid_line(face_points, indices, color);
                } else {
                    raster.draw_solid(face_points, indices, color);
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn render_source_exploded(
        &self,
        raster: &mut SourceRaster,
        shape_id: u16,
        shape: &GpuShape,
        animation_frame: u8,
        col_frame: u8,
        color_table: u16,
        object_depth_table: u8,
        texture_scroll: [u8; 2],
        explosion_state: u8,
        pose: SourcePose,
        palette: &shapes::ShapePaletteRgb,
    ) -> bool {
        if shape.is_sf2 {
            return false;
        }
        let flat_shape_id = shapes::resolve_shape_word(shape_id);
        let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(flat_shape_id) else {
            return false;
        };
        let base_projection = source_projection::project_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        let shape = self.source_shape_for_depth(
            flat_shape_id,
            base_projection.object_depth,
            animation_frame,
            shape,
        );
        let base_projection = source_projection::project_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        let depth_bank = shapes::select_object_depth_bank(
            f32::from(base_projection.object_depth),
            self.depthz_table,
            object_depth_table,
        );
        let texture_palette: [[f32; 4]; 16] = std::array::from_fn(|index| {
            [palette[index][0], palette[index][1], palette[index][2], 1.0]
        });

        for face_index in Self::source_face_order(shape, &base_projection.points) {
            let face = &shape.faces[face_index];
            let authored_indices = source_explosion_face_indices(face, &base_projection.points);
            let projected = source_projection::project_exploded_face(
                &shape.vertices,
                &shape.reflected_pair_starts,
                &authored_indices,
                face.normal,
                metrics.coordinate_shift,
                explosion_state,
                pose,
            );
            if projected
                .points
                .iter()
                .any(|point| point.depth < source_projection::MIN_FRONT_DEPTH)
            {
                continue;
            }
            let indices = &SOURCE_LOCAL_FACE_INDICES[..usize::from(face.num_verts)];
            raster.set_face(face_index as u16);
            let material =
                resolve_registered_face_material(shape, face.color_index, col_frame, color_table);
            if let Some(material) =
                material.filter(|material| material & 0xC000 == shapes::MATERIAL_COLTEXT_FLAG)
            {
                self.render_source_textured_face(
                    raster,
                    &projected.points,
                    indices,
                    face,
                    material,
                    texture_scroll,
                    &texture_palette,
                );
                continue;
            }
            let shade_index =
                shapes::compute_quantized_shade_index(face.normal, base_projection.object_light);
            if let Some(pair) = resolve_registered_face_palette_pair(
                shape,
                face.color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
                self.depth_colors,
            ) {
                if face.num_verts == 2 {
                    raster.draw_palette_line(
                        &projected.points,
                        indices,
                        &texture_palette,
                        [pair.low, pair.high],
                    );
                } else {
                    raster.draw_palette_pair(
                        &projected.points,
                        indices,
                        &texture_palette,
                        [pair.low, pair.high],
                    );
                }
            } else {
                let color = resolve_registered_face_color(
                    shape,
                    face.color_index,
                    col_frame,
                    color_table,
                    shade_index,
                    depth_bank,
                    self.depth_colors,
                    palette,
                );
                if face.num_verts == 2 {
                    raster.draw_solid_line(&projected.points, indices, color);
                } else {
                    raster.draw_solid(&projected.points, indices, color);
                }
            }
        }
        true
    }

    /// Rasterize the retail fixed-point shadow pass into the same indexed
    /// bitmap as the normal polygons. Ordinary source shadows reuse the
    /// object's mesh, flatten its height contribution, and alternate shadow
    /// color nine with cleared pixels on the source checkerboard.
    pub fn render_source_shadow(
        &self,
        raster: &mut SourceRaster,
        shape_id: u16,
        anim_frame: u8,
        explosion_state: u8,
        pose: SourcePose,
        palette: &shapes::ShapePaletteRgb,
    ) -> bool {
        let shape = if let Some(frames) = self.sf2_animation_frames.get(&shape_id) {
            &frames[usize::from(anim_frame) % frames.len()]
        } else if let Some(shape) = self.sf2_shapes.get(&shape_id) {
            shape
        } else {
            let flat_shape_id = shapes::resolve_shape_word(shape_id);
            if let Some(frames) = self.sf1_animation_frames.get(&flat_shape_id) {
                &frames[sf1_animation_frame_index(anim_frame, frames.len())]
            } else {
                let Some(shape) = self.shapes[flat_shape_id as usize].as_ref() else {
                    return false;
                };
                shape
            }
        };
        if shape.is_sf2 {
            return false;
        }
        let flat_shape_id = shapes::resolve_shape_word(shape_id);
        let Some(metrics) = sf_core::sf1_shape_metrics::sf1_shape_metrics(flat_shape_id) else {
            return false;
        };
        let base_projection = source_projection::project_shadow_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        let shape = self.source_shape_for_depth(
            flat_shape_id,
            base_projection.object_depth,
            anim_frame,
            shape,
        );
        let projected = source_projection::project_shadow_shape(
            &shape.vertices,
            &shape.reflected_pair_starts,
            metrics.coordinate_shift,
            pose,
        );
        if projected
            .points
            .iter()
            .any(|point| point.depth < source_projection::MIN_FRONT_DEPTH)
            || source_projection::shape_is_outside_playfield(&projected.points)
        {
            return true;
        }
        let texture_palette: [[f32; 4]; 16] = std::array::from_fn(|index| {
            [palette[index][0], palette[index][1], palette[index][2], 1.0]
        });
        const SOURCE_SHADOW_PAIR: [u8; 2] = [9, 0];
        for face_index in Self::source_face_order(shape, &projected.points) {
            let face = &shape.faces[face_index];
            if explosion_state == 0
                && face.visibility_vertices.is_some_and(|indices| {
                    !source_projection::face_is_visible(&projected.points, indices)
                })
            {
                continue;
            }
            raster.set_face(face_index as u16);
            let authored_indices = &face.vertex_indices[..usize::from(face.num_verts)];
            let explosion_indices = (explosion_state != 0)
                .then(|| source_explosion_face_indices(face, &projected.points));
            let exploded = (explosion_state != 0).then(|| {
                source_projection::project_exploded_shadow_face(
                    &shape.vertices,
                    &shape.reflected_pair_starts,
                    explosion_indices
                        .as_deref()
                        .expect("exploded shadow indices"),
                    face.normal,
                    metrics.coordinate_shift,
                    explosion_state,
                    pose,
                )
            });
            let (face_points, face_indices) = exploded.as_ref().map_or_else(
                || (projected.points.as_slice(), authored_indices),
                |exploded| {
                    (
                        exploded.points.as_slice(),
                        &SOURCE_LOCAL_FACE_INDICES[..usize::from(face.num_verts)],
                    )
                },
            );
            if face.num_verts == 2 {
                raster.draw_palette_line(
                    face_points,
                    face_indices,
                    &texture_palette,
                    SOURCE_SHADOW_PAIR,
                );
            } else {
                raster.draw_palette_pair(
                    face_points,
                    face_indices,
                    &texture_palette,
                    SOURCE_SHADOW_PAIR,
                );
            }
        }
        true
    }

    /// Mirror of `Shapes_Render`, pushing per-face flat triangles/lines to the
    /// retained `Gpu` with `transform`'s current proj/view. `palette` is the
    /// frame's decoded BGS-selected polygon palette.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        gpu: &mut Gpu,
        source_raster: &mut SourceRaster,
        transform: &Transform,
        shape_id: u16,
        anim_frame: u8,
        animation_blend: Option<MeshAnimationBlend>,
        col_frame: u8,
        color_table: u16,
        object_depth_table: u8,
        texture_scroll: [u8; 2],
        explosion_state: u8,
        model_matrix: &[f32; 16],
        depth_layer: u8,
        render_mode: ShapeRenderMode,
        source_pose: Option<SourcePose>,
        palette: &shapes::ShapePaletteRgb,
        palette_pair_style: shapes::PalettePairStyle,
    ) {
        let shape = if let Some(frames) = self.sf2_animation_frames.get(&shape_id) {
            &frames[usize::from(anim_frame) % frames.len()]
        } else if let Some(shape) = self.sf2_shapes.get(&shape_id) {
            shape
        } else {
            let flat_shape_id = shapes::resolve_shape_word(shape_id);
            if flat_shape_id as usize >= MAX_SHAPES {
                return;
            }
            if let Some(frames) = self.sf1_animation_frames.get(&flat_shape_id) {
                &frames[sf1_animation_frame_index(anim_frame, frames.len())]
            } else {
                let Some(shape) = self.shapes[flat_shape_id as usize].as_ref() else {
                    return;
                };
                shape
            }
        };

        // Source-resolution output and scaled sprites remain discrete. SF1
        // animation semantics are deliberately unaffected by this SF2 change.
        let blended_shape = if source_pose.is_none() && render_mode == ShapeRenderMode::Polygons {
            animation_blend.and_then(|blend| self.blended_sf2_animation(shape_id, blend))
        } else {
            None
        };
        let shape = blended_shape.as_ref().unwrap_or(shape);

        let mut proj = *transform.projection();
        // Sequential launch-corridor pieces deliberately overlap at their
        // seams. MARIO resolves those coplanar faces by painter order; a
        // depth buffer otherwise produces moving moire strips as the camera
        // enters a seam. Keep screen projection unchanged and give the later
        // painter layer a tiny, deterministic normalized-depth preference.
        apply_depth_layer_bias(&mut proj, depth_layer);
        let view = *transform.view();
        let mut projection_view = [0.0; 16];
        crate::transform::multiply(&mut projection_view, &proj, &view);
        let mut projection_view_model = [0.0; 16];
        crate::transform::multiply(&mut projection_view_model, &projection_view, model_matrix);
        let object_depth = Self::object_depth(&view, model_matrix);
        let depth_bank =
            shapes::select_object_depth_bank(object_depth, self.depthz_table, object_depth_table);
        // Both games use the same HD distance-color interpolation. An
        // authored object override remains discrete; game identity is not
        // a reason to bypass smooth shading. The retail branch below still
        // consumes the exact selected bank, independently of this blend.
        let depth_blend = if object_depth_table != 0 {
            shapes::DepthBankBlend {
                near_bank: shapes::DepthBank::from_source_index(depth_bank),
                far_bank: shapes::DepthBank::from_source_index(depth_bank),
                amount: 0.0,
            }
        } else {
            shapes::blend_depth_banks(object_depth, self.depthz_table)
        };
        let light_object = compute_object_light(model_matrix);
        if let Some(pose) = source_pose {
            let rendered = if explosion_state == 0 {
                self.render_source_projected(
                    source_raster,
                    shape_id,
                    shape,
                    anim_frame,
                    col_frame,
                    color_table,
                    object_depth_table,
                    texture_scroll,
                    pose,
                    palette,
                )
            } else {
                self.render_source_exploded(
                    source_raster,
                    shape_id,
                    shape,
                    anim_frame,
                    col_frame,
                    color_table,
                    object_depth_table,
                    texture_scroll,
                    explosion_state,
                    pose,
                    palette,
                )
            };
            if rendered {
                return;
            }
        }
        let texture_palette: [[f32; 4]; 16] =
            std::array::from_fn(|i| [palette[i][0], palette[i][1], palette[i][2], 1.0]);

        // MDRAWLIS dispatches scaled sprites before the polygon face loop.
        // depth_offset selects the sprite material, and tscroll_x controls
        // size only. Neither is a polygon depth/scroll value on this path.
        if render_mode == ShapeRenderMode::ScaledSprite && !shape.is_sf2 {
            self.render_hd_scaled_sprite(
                gpu,
                shape,
                col_frame,
                color_table,
                object_depth_table,
                &proj,
                &view,
                model_matrix,
                &texture_palette,
            );
            return;
        }

        for (i, face) in shape.faces.iter().enumerate() {
            let tri_start = shape.face_tri_start[i];
            let tri_count = shape.face_tri_count[i];

            // Both visible and hidden source faces become separate shards
            // during an explosion (`mexpfacesvis` / `mexpfacesnvis`).
            if explosion_state == 0
                && render_mode == ShapeRenderMode::Polygons
                && tri_count > 0
                && !face_is_camera_visible(shape, face, &projection_view_model)
            {
                continue;
            }

            if tri_count <= 0 {
                // Wireframe (Face2) segment: draw as a line with the face's
                // material color, full-bright (the SNES does not shade lines).
                if face.num_verts == 2 && shape.face_line_first[i] >= 0 {
                    let model = if explosion_state != 0 {
                        build_exploded_model_matrix(
                            model_matrix,
                            &shape.face_normals[i],
                            explosion_state,
                        )
                    } else {
                        *model_matrix
                    };
                    let first = shape.face_line_first[i] as usize;
                    if palette_pair_style == shapes::PalettePairStyle::Smooth {
                        let color = resolve_registered_face_smooth_color(
                            shape,
                            face.color_index,
                            col_frame,
                            color_table,
                            SHADE_TABLE_LEN as i32 - 1,
                            depth_blend,
                            self.depth_colors,
                            palette,
                        );
                        gpu.push_flat_lines(
                            &shape.line_verts[first..first + 2],
                            &proj,
                            &view,
                            &model,
                            color,
                        );
                    } else if let Some(pair) = resolve_registered_face_palette_pair(
                        shape,
                        face.color_index,
                        col_frame,
                        color_table,
                        SHADE_TABLE_LEN as i32 - 1,
                        depth_bank,
                        self.depth_colors,
                    ) {
                        gpu.push_palette_pair_lines(
                            &shape.line_verts[first..first + 2],
                            &proj,
                            &view,
                            &model,
                            &texture_palette,
                            [pair.low, pair.high],
                        );
                    } else {
                        let color = resolve_registered_face_color(
                            shape,
                            face.color_index,
                            col_frame,
                            color_table,
                            SHADE_TABLE_LEN as i32 - 1,
                            depth_bank,
                            self.depth_colors,
                            palette,
                        );
                        gpu.push_flat_lines(
                            &shape.line_verts[first..first + 2],
                            &proj,
                            &view,
                            &model,
                            color,
                        );
                    }
                }
                continue;
            }

            let model = if explosion_state != 0 {
                // `mexpfacesinit` rotates each authored face normal, forces
                // the Y component downward, then applies `(n * expcnt) >> 2`.
                build_exploded_model_matrix(model_matrix, &shape.face_normals[i], explosion_state)
            } else {
                *model_matrix
            };

            let shade_index = shapes::compute_shade_index(face.normal, light_object);

            let material =
                resolve_registered_face_material(shape, face.color_index, col_frame, color_table);
            if material.is_some_and(|word| word & 0xC000 == shapes::MATERIAL_COLTEXT_FLAG) {
                if let Some((verts, texture, high_nibble)) =
                    self.textured_face_vertices(shape, face, material.unwrap(), texture_scroll)
                {
                    // BU_7 embeds its logo inside a larger coplanar wall;
                    // MYBASE_0 uses matching outlines. Keep the backing
                    // (texel zero is transparent),
                    // but give only the later decal a tiny normalized-depth
                    // preference so camera quantization cannot alternate
                    // ownership between the two surfaces.
                    let decal_over_backing = explosion_state == 0
                        && i > 0
                        && coplanar_decal_is_contained(&shape.vertices, face, &shape.faces[i - 1])
                        && resolve_registered_face_material(
                            shape,
                            shape.faces[i - 1].color_index,
                            col_frame,
                            color_table,
                        )
                        .is_some_and(|word| word & 0xC000 != shapes::MATERIAL_COLTEXT_FLAG);
                    let mut decal_proj = proj;
                    if decal_over_backing {
                        apply_depth_layer_bias(&mut decal_proj, 1);
                    }
                    gpu.push_textured_tris(
                        &verts,
                        &decal_proj,
                        &view,
                        &model,
                        &texture_palette,
                        high_nibble,
                        texture,
                    );
                    continue;
                }
            }

            let start = (tri_start * 3) as usize;
            let count = (tri_count * 3) as usize;
            if palette_pair_style == shapes::PalettePairStyle::Smooth {
                let color = resolve_registered_face_smooth_color(
                    shape,
                    face.color_index,
                    col_frame,
                    color_table,
                    shade_index,
                    depth_blend,
                    self.depth_colors,
                    palette,
                );
                gpu.push_flat_tris(
                    &shape.tri_verts[start..start + count],
                    &proj,
                    &view,
                    &model,
                    color,
                );
            } else if let Some(pair) = resolve_registered_face_palette_pair(
                shape,
                face.color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
                self.depth_colors,
            ) {
                gpu.push_palette_pair_tris(
                    &shape.tri_verts[start..start + count],
                    &proj,
                    &view,
                    &model,
                    &texture_palette,
                    [pair.low, pair.high],
                );
            } else {
                let color = resolve_registered_face_color(
                    shape,
                    face.color_index,
                    col_frame,
                    color_table,
                    shade_index,
                    depth_bank,
                    self.depth_colors,
                    palette,
                );
                gpu.push_flat_tris(
                    &shape.tri_verts[start..start + count],
                    &proj,
                    &view,
                    &model,
                    color,
                );
            }
        }
    }
}

#[inline]
fn continuous_texture_coord(authored: u8, scroll: u8) -> f32 {
    f32::from(authored) + f32::from(scroll)
}

fn coplanar_decal_is_contained(
    vertices: &[ShapeVertex],
    decal: &ShapeFace,
    backing: &ShapeFace,
) -> bool {
    if decal.normal != backing.normal
        || !(3..=4).contains(&decal.num_verts)
        || !(3..=4).contains(&backing.num_verts)
    {
        return false;
    }
    fn subtract(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| a[axis] - b[axis])
    }
    fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    let point = |index: u16| {
        let vertex = vertices[usize::from(index)];
        [
            f64::from(vertex.x),
            f64::from(vertex.y),
            f64::from(vertex.z),
        ]
    };
    let wall = &backing.vertex_indices[..usize::from(backing.num_verts)];
    let logo = &decal.vertex_indices[..usize::from(decal.num_verts)];
    if wall
        .iter()
        .chain(logo)
        .any(|&index| usize::from(index) >= vertices.len())
    {
        return false;
    }
    let origin = point(wall[0]);
    let normal = cross(
        subtract(point(wall[1]), origin),
        subtract(point(wall[2]), origin),
    );
    if normal == [0.0; 3] {
        return false;
    }
    // Generated source coordinates are exact integers. Test the authored
    // plane before camera transforms, without a distance-based tolerance
    // that could incorrectly promote a nearby but separate surface.
    logo.iter().all(|&index| {
        let vertex = point(index);
        dot(normal, subtract(vertex, origin)) == 0.0
            && (0..wall.len()).all(|edge| {
                let start = point(wall[edge]);
                let end = point(wall[(edge + 1) % wall.len()]);
                dot(normal, cross(subtract(end, start), subtract(vertex, start))) >= 0.0
            })
    })
}

/// Give later source painter layers a constant normalized-depth preference.
/// For the perspective matrix used by [`Transform`], `clip.w = -z` and the
/// coefficient at index 10 multiplies the same camera-space z.  Adjusting
/// that coefficient therefore changes `clip.z / clip.w` by exactly the same
/// amount at every distance, unlike adding a constant to clip-z.
const DEPTH_LAYER_STEP: f32 = 0.000_001;

fn apply_depth_layer_bias(projection: &mut [f32; 16], depth_layer: u8) {
    projection[10] += f32::from(depth_layer) * DEPTH_LAYER_STEP;
}

impl Default for ShapeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_depth_layer_bias, face_is_camera_visible, resolve_registered_face_material,
        resolve_registered_face_palette_pair, sf1_animation_frame_index,
        source_explosion_face_indices, source_lod_index, texture_address, texture_material_fields,
        MeshAnimationBlend, ShapeStore, DEPTH_LAYER_STEP, RETAIL_REVISION_2_PILLAR_TEXEL,
        RETAIL_REVISION_2_PILLAR_TEXEL_ADDRESS, SOURCE_TEX_BANK_01, TEXTURE_XY, TEX_BANK_01,
    };
    use super::{continuous_texture_coord, coplanar_decal_is_contained};
    use crate::shape_data::{ShapeFace, ShapeVertex};
    use crate::shapes;
    use sf_core::shape::sf2_shape_id;

    #[test]
    fn source_lod_thresholds_match_the_authored_distance_bands() {
        assert_eq!(source_lod_index(999), None);
        assert_eq!(source_lod_index(1_000), Some(0));
        assert_eq!(source_lod_index(1_999), Some(0));
        assert_eq!(source_lod_index(2_000), Some(1));
        assert_eq!(source_lod_index(2_999), Some(1));
        assert_eq!(source_lod_index(3_000), Some(2));
    }

    #[test]
    fn sf1_animation_selection_masks_before_reducing_the_frame_table() {
        const ROBOT_FRAME_COUNT: usize = 12;

        assert_eq!(sf1_animation_frame_index(71, ROBOT_FRAME_COUNT), 7);
        assert_eq!(sf1_animation_frame_index(63, ROBOT_FRAME_COUNT), 3);
        assert_eq!(sf1_animation_frame_index(64, ROBOT_FRAME_COUNT), 0);
    }

    #[test]
    fn retail_revision_two_pillar_texel_is_an_explicit_source_correction() {
        assert_eq!(
            SOURCE_TEX_BANK_01[RETAIL_REVISION_2_PILLAR_TEXEL_ADDRESS],
            0x5A
        );
        assert_eq!(RETAIL_REVISION_2_PILLAR_TEXEL, 0x59);
        assert_eq!(
            TEX_BANK_01[RETAIL_REVISION_2_PILLAR_TEXEL_ADDRESS],
            RETAIL_REVISION_2_PILLAR_TEXEL,
        );
    }

    #[test]
    fn hidden_explosion_face_reverses_its_authored_vertex_stream() {
        let debris = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 465)
            .expect("compiled Training debris");
        let face = &debris.faces[2];
        let points = [
            crate::source_projection::ProjectedPoint {
                x: 96,
                y: 151,
                depth: 186,
            },
            crate::source_projection::ProjectedPoint {
                x: 109,
                y: 144,
                depth: 186,
            },
            crate::source_projection::ProjectedPoint {
                x: 123,
                y: 150,
                depth: 186,
            },
            crate::source_projection::ProjectedPoint {
                x: 85,
                y: 149,
                depth: 186,
            },
        ];

        assert_eq!(source_explosion_face_indices(face, &points), [2, 1, 3]);
    }

    fn visibility_test_shape(selector: Option<[u16; 3]>) -> super::GpuShape {
        let vertices = [
            ShapeVertex {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            ShapeVertex {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            ShapeVertex {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        let faces = [ShapeFace {
            vertex_indices: [0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            num_verts: 3,
            color_index: 0,
            normal: [0, 127, 0],
            visibility_vertices: selector,
        }];
        ShapeStore::build_gpu_shape(&vertices, &faces, &[], &[], 0, false)
    }

    #[test]
    fn corneria_building_depth_material_resolves_from_the_authored_table() {
        let source = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|shape| shape.shape_id == 61)
            .expect("Corneria building shape");
        let table = crate::color_data::table_id_by_name(source.default_color_table)
            .expect("building color table");
        let shape = ShapeStore::build_gpu_shape(
            source.vertices,
            source.faces,
            source.painter_nodes,
            source.reflected_pair_starts,
            table,
            false,
        );
        let face = &shape.faces[5];
        assert_eq!(
            resolve_registered_face_material(&shape, face.color_index, 58, 0),
            Some(0x3E06)
        );
        assert_eq!(
            resolve_registered_face_palette_pair(
                &shape,
                face.color_index,
                58,
                0,
                0,
                0,
                sf_core::scene::DepthColors::Night,
            ),
            Some(shapes::PalettePair { low: 12, high: 12 })
        );
    }

    #[test]
    fn source_visibility_winding_is_preserved_in_clip_coordinates() {
        let identity = [
            1.0, 0.0, 0.0, 0.0, // column 0
            0.0, 1.0, 0.0, 0.0, // column 1
            0.0, 0.0, 1.0, 0.0, // column 2
            0.0, 0.0, 0.0, 1.0, // column 3
        ];

        let visible = visibility_test_shape(Some([0, 1, 2]));
        assert_eq!(visible.face_normals[0], [0.0, 1.0, 0.0]);
        assert!(face_is_camera_visible(
            &visible,
            &visible.faces[0],
            &identity
        ));

        let hidden = visibility_test_shape(Some([0, 2, 1]));
        assert!(!face_is_camera_visible(
            &hidden,
            &hidden.faces[0],
            &identity
        ));

        let two_sided = visibility_test_shape(None);
        assert!(face_is_camera_visible(
            &two_sided,
            &two_sided.faces[0],
            &identity
        ));
    }

    #[test]
    fn corridor_back_face_stays_culled_when_selector_crosses_eye_plane() {
        // Side wall: moving past its end must not make both sides visible.
        let mut front = visibility_test_shape(Some([0, 1, 2]));
        front.vertices = vec![
            ShapeVertex {
                x: -1.0,
                y: -1.0,
                z: 1.0,
            },
            ShapeVertex {
                x: -1.0,
                y: 1.0,
                z: 1.0,
            },
            ShapeVertex {
                x: -1.0,
                y: 1.0,
                z: 3.0,
            },
        ];
        let mut back = front.faces[0].clone();
        back.visibility_vertices = Some([0, 2, 1]);
        for eye_depth in [0.0, 0.5, 1.0, 1.5, 2.0] {
            let projection = [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
                -eye_depth,
            ];
            let front_visible = face_is_camera_visible(&front, &front.faces[0], &projection);
            let back_visible = face_is_camera_visible(&front, &back, &projection);
            assert_ne!(
                front_visible, back_visible,
                "opposite side-wall selectors must not both draw at eye depth {eye_depth}"
            );
            assert!(!front_visible);
            assert!(back_visible);
        }
    }

    #[test]
    fn sf2_mesh_animation_blends_authored_vertices_without_mutating_them() {
        const ANIMATED_SHAPE: u16 = sf2_shape_id(51);
        let mut store = ShapeStore::new();
        store.register_sf2_shapes();
        let frames = &store.sf2_animation_frames[&ANIMATED_SHAPE];
        let original = frames[0].vertices.clone();
        let blend = |amount| MeshAnimationBlend {
            previous_frame: 0,
            current_frame: 1,
            amount,
        };
        for amount in [0.25, 0.5, 0.75] {
            let mesh = store
                .sf2_animation_geometry(ANIMATED_SHAPE, 0, Some(blend(amount)))
                .expect("authored SF2 animation");
            for ((actual, previous), current) in mesh
                .vertices
                .iter()
                .zip(&frames[0].vertices)
                .zip(&frames[1].vertices)
            {
                assert_eq!(actual.x, previous.x + (current.x - previous.x) * amount);
                assert_eq!(actual.y, previous.y + (current.y - previous.y) * amount);
                assert_eq!(actual.z, previous.z + (current.z - previous.z) * amount);
            }
            assert_ne!(mesh.vertices, frames[0].vertices);
            assert_ne!(mesh.vertices, frames[1].vertices);
            assert_eq!(mesh.faces, frames[0].faces);
        }
        for (amount, frame) in [(0.0, 0), (1.0, 1)] {
            let mesh = store
                .sf2_animation_geometry(ANIMATED_SHAPE, frame, Some(blend(amount)))
                .expect("authored SF2 animation endpoint");
            assert_eq!(mesh.vertices, frames[usize::from(frame)].vertices);
        }
        assert_eq!(
            store.sf2_animation_frames[&ANIMATED_SHAPE][0].vertices,
            original
        );
    }

    #[test]
    fn sf2_mesh_animation_holds_discontinuities_and_rejects_changed_topology() {
        const ANIMATED_SHAPE: u16 = sf2_shape_id(51);
        let mut store = ShapeStore::new();
        store.register_sf2_shapes();
        for (previous_frame, current_frame) in [(0, 0), (0, 3)] {
            assert!(store
                .blended_sf2_animation(
                    ANIMATED_SHAPE,
                    MeshAnimationBlend {
                        previous_frame,
                        current_frame,
                        amount: 0.5,
                    }
                )
                .is_none());
        }
        let adjacent = MeshAnimationBlend {
            previous_frame: 0,
            current_frame: 1,
            amount: 0.5,
        };
        for amount in [f32::NAN, f32::INFINITY, -1.0, 2.0] {
            assert!(store
                .blended_sf2_animation(ANIMATED_SHAPE, MeshAnimationBlend { amount, ..adjacent })
                .is_none());
        }
        assert!(store
            .blended_sf2_animation(
                ANIMATED_SHAPE,
                MeshAnimationBlend {
                    previous_frame: 1,
                    current_frame: 0,
                    ..adjacent
                }
            )
            .is_some());
        assert!(store
            .blended_sf2_animation(
                ANIMATED_SHAPE,
                MeshAnimationBlend {
                    previous_frame: 15,
                    current_frame: 0,
                    ..adjacent
                }
            )
            .is_some());
        let frames = store.sf2_animation_frames.get_mut(&ANIMATED_SHAPE).unwrap();
        frames[1].faces[0].vertex_indices.swap(0, 1);
        assert!(store
            .blended_sf2_animation(ANIMATED_SHAPE, adjacent)
            .is_none());
    }

    #[test]
    fn sf2_catalog_uses_native_tokens_scale_and_material_pointers() {
        let mut store = ShapeStore::new();
        store.register_sf2_shapes();
        assert_eq!(store.sf2_shapes.len(), 577);
        assert_eq!(store.sf2_animation_frames.len(), 135);

        let animated = store
            .sf2_animation_frames
            .get(&sf2_shape_id(51))
            .expect("first animated SF2 ShapeHdr");
        assert_eq!(animated.len(), 16);
        assert_ne!(animated[0].vertices, animated[1].vertices);

        let craft = store
            .get(sf2_shape_id(415))
            .expect("SF2 craft catalog mesh");
        assert!(craft.is_sf2);
        assert_eq!(craft.default_color_table, 0x81F4);
        assert_eq!(craft.vertices.len(), 18);
        assert_eq!(craft.faces.len(), 26);
        assert_eq!(
            store.shape_half_extents(sf2_shape_id(415)),
            Some((592, 848, 1104))
        );
        assert!(craft.face_normals[0][0] < -0.8);

        let source = sf2_data::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.header_index == 415)
            .expect("SF2 craft source entry");
        let (vertex_index, source_vertex) = source
            .vertices
            .iter()
            .enumerate()
            .find(|(_, vertex)| vertex.y != 0)
            .expect("SF2 craft has a nonzero Y coordinate");
        let scale = (1u32 << source.shift) as f32;
        assert_eq!(
            craft.vertices[vertex_index].y,
            -f32::from(source_vertex.y) * scale,
            "SF2 vertices must use the shared GL-up flat-coordinate model"
        );

        let (face_index, source_face) = source
            .faces
            .iter()
            .enumerate()
            .find(|(_, face)| face.normal[1] != 0)
            .expect("SF2 craft has a nonzero authored normal Y component");
        assert_eq!(
            craft.faces[face_index].normal,
            [
                i16::from(source_face.normal[0]),
                -i16::from(source_face.normal[1]),
                i16::from(source_face.normal[2]),
            ]
        );
        assert_eq!(
            craft.face_normals[face_index][1].signum(),
            -f32::from(source_face.normal[1]).signum()
        );

        assert!(store.get(sf2_shape_id(577)).is_none());
    }

    #[test]
    fn null_shape_uses_its_source_authored_bounds() {
        let store = ShapeStore::new();
        assert_eq!(
            store.shape_half_extents(shapes::SHAPE_NULL),
            Some(shapes::SHAPE_NULL_HALF_EXTENTS)
        );
        assert_eq!(
            store
                .all_shape_half_extents()
                .get(&shapes::SHAPE_NULL)
                .copied(),
            Some(shapes::SHAPE_NULL_HALF_EXTENTS)
        );
        assert_eq!(
            shapes::sf1_shape_half_extents()
                .get(&shapes::SHAPE_NULL)
                .copied(),
            Some(shapes::SHAPE_NULL_HALF_EXTENTS)
        );
    }

    #[test]
    fn texture_address_preserves_linear_row_carry() {
        // Adding X after masking carries through byte 0 exactly as the GSU
        // `add rspdata` does.  The old atlas-UV implementation returned
        // (0x17, row 0) instead of address 0x0117 here.
        assert_eq!(texture_address(0x00f8, 0x1f1f, 31, 0), 0x0117);
    }

    #[test]
    fn hd_texture_scroll_keeps_vertex_span_continuous_across_byte_wrap() {
        assert_eq!(continuous_texture_coord(0, 250), 250.0);
        assert_eq!(continuous_texture_coord(8, 250), 258.0);
        assert_eq!(
            continuous_texture_coord(8, 250) - continuous_texture_coord(0, 250),
            8.0
        );
    }

    #[test]
    fn authored_logo_decals_recognize_contained_and_matching_backings() {
        const LOGO_PAIRS: &[(u16, &[usize])] = &[
            (crate::shape_data::SHAPE_EXT_MYBASE_0, &[48, 50]),
            (sf_map::consts::sh::BU_7, &[7]),
        ];
        for &(shape_id, backing_indices) in LOGO_PAIRS {
            let shape = crate::shape_data::SHAPE_DATA
                .iter()
                .find(|shape| shape.shape_id == shape_id)
                .unwrap();
            for &index in backing_indices {
                let backing = &shape.faces[index];
                let decal = &shape.faces[index + 1];
                assert!(coplanar_decal_is_contained(shape.vertices, decal, backing));
                let mut displaced = shape.vertices.to_vec();
                displaced[usize::from(decal.vertex_indices[0])].z += 1.0;
                assert!(!coplanar_decal_is_contained(&displaced, decal, backing));
            }
        }
    }

    #[test]
    fn texture_layout_masks_match_defspr_records() {
        assert_eq!(
            TEXTURE_XY.map(|layout| layout.mask),
            [0x1f1f, 0x3f3f, 0x0f7f, 0x1f1f, 0x0f3f, 0x0f3f, 0x3f3f, 0x077f, 0x1f1f,]
        );
        assert_eq!(texture_address(0x0000, 0x0f7f, 127, 31), 0x0f7f);
        assert_eq!(texture_address(0x7ff0, 0x1f1f, 31, 0), 0x000f);
    }

    #[test]
    fn texture_control_bits_follow_each_games_retail_encoding() {
        assert_eq!(texture_material_fields(false, 0x6004), (0, 4, true));
        assert_eq!(texture_material_fields(false, 0x4504), (5, 4, false));
        assert_eq!(texture_material_fields(true, 0x409A), (0, 154, true));
        assert_eq!(texture_material_fields(true, 0x401E), (0, 30, false));
        assert_eq!(texture_material_fields(true, 0x45A0), (5, 160, true));
    }

    #[test]
    fn tunnel_depth_bias_is_constant_in_normalized_perspective_depth() {
        let mut projection = [0.0; 16];
        projection[10] = -1.0;
        projection[11] = -1.0;
        projection[14] = -1.0;
        let base = projection;
        apply_depth_layer_bias(&mut projection, 3);

        for camera_z in [-64.0f32, -512.0, -4096.0] {
            let base_ndc = (base[10] * camera_z + base[14]) / (base[11] * camera_z);
            let biased_ndc =
                (projection[10] * camera_z + projection[14]) / (projection[11] * camera_z);
            assert!((biased_ndc - base_ndc + 3.0 * DEPTH_LAYER_STEP).abs() < 1e-6);
        }
    }

    #[test]
    fn tunnel_depth_bias_orders_later_coplanar_layer_first() {
        let mut far_layer = [0.0; 16];
        let mut near_layer = [0.0; 16];
        apply_depth_layer_bias(&mut far_layer, 1);
        apply_depth_layer_bias(&mut near_layer, 2);
        assert!(near_layer[10] > far_layer[10]);
        // With clip.w = -z, the larger coefficient produces the smaller
        // normalized depth and wins the GPU Less comparison.
        let z = -100.0;
        let far_ndc = far_layer[10] * z / -z;
        let near_ndc = near_layer[10] * z / -z;
        assert!(near_ndc < far_ndc);
    }
}
