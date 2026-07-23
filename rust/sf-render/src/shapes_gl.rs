//! GPU shape store: upload, per-face flat-color rendering, GSU shading.
//!
//! Port (C oracle): the runtime half of `src/renderer/shapes.c`
//! (`Shapes_Register`/`UploadShape`/`BuildFaceNormals`/`Shapes_Render`/
//! `Shapes_RegisterBuiltins`). The pure color math (material resolution,
//! shade index, depth banks) lives in [`crate::shapes`] and is wired in
//! here exactly like the C `Shapes_Render` loop.

use crate::gpu::{Gpu, TextureId, Vertex3, Vertex3Tex};
use crate::light_data::SHADE_TABLE_LEN;
use crate::shape_data::{self, ShapeFace, ShapeVertex};
use crate::shapes::{self, DEPTHZ_COUNT, DEPTHZ_NORMAL, SHAPE_COLTAB_ID_0};
use crate::transform::Transform;
use sf_core::scene::{DepthColors, SceneStyle};
use std::collections::HashMap;

pub const MAX_SHAPES: usize = 512;

const TEX_BANK_01: &[u8; 32768] =
    include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_01.BIN");
const TEX_BANK_23: &[u8; 32768] =
    include_bytes!("../../../reference/ultrastarfox/SF/MSPRITES/TEX_23.BIN");

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
pub struct GpuShape {
    /// Generated ShapeHdr `sh_col_ptr`, represented by color_data table id.
    pub default_color_table: u16,
    /// SF2 stores an address into its bank-$01 color data; SF1 stores a
    /// generated table id. This selects the matching material resolver.
    pub is_sf2: bool,
    pub vertices: Vec<ShapeVertex>,
    pub faces: Vec<ShapeFace>,
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

/// Project one source visibility point into normalized device coordinates.
/// Faces crossing the camera plane are left to GPU clipping instead of being
/// rejected from an unstable pre-divide winding.
fn project_visibility_point(vertex: &ShapeVertex, pvm: &[f32; 16]) -> Option<[f32; 2]> {
    const MIN_POSITIVE_CLIP_W: f32 = 0.0001;

    let clip_x = pvm[0] * vertex.x + pvm[4] * vertex.y + pvm[8] * vertex.z + pvm[12];
    let clip_y = pvm[1] * vertex.x + pvm[5] * vertex.y + pvm[9] * vertex.z + pvm[13];
    let clip_w = pvm[3] * vertex.x + pvm[7] * vertex.y + pvm[11] * vertex.z + pvm[15];
    if !clip_x.is_finite()
        || !clip_y.is_finite()
        || !clip_w.is_finite()
        || clip_w <= MIN_POSITIVE_CLIP_W
    {
        return None;
    }

    let projected = [clip_x / clip_w, clip_y / clip_w];
    projected[0]
        .is_finite()
        .then_some(())
        .and_then(|()| projected[1].is_finite().then_some(projected))
}

/// Source `msh_vizis` records one independently selected triangle per face.
/// A negative source projected winding is visible. The retail billboard
/// selector `2,3,0` remains negative after the complete source-to-GL camera
/// conversion, so the equivalent NDC test is also strictly negative.
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

    let signed_area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    signed_area < 0.0
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

pub struct ShapeStore {
    shapes: Vec<Option<GpuShape>>,
    /// Fully decompiled SF1 vertex streams, keyed by the flat native shape
    /// id. Each vector includes frame zero and follows the source Frames-table
    /// period; no source addresses or shape-language state reach rendering.
    sf1_animation_frames: HashMap<u16, Vec<GpuShape>>,
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

    /// Per-axis collision half-extents for a shape's mesh, matching the C
    /// `load_collision_extents` (bounding-box half-dimensions sh_maxx/maxy/maxz):
    /// max(|x|), max(|y|), max(|z|) over the shape's vertices, clamped to i16.
    /// Resolves raw shape words like [`ShapeStore::get`]. Returns `None` when
    /// the shape is unregistered or has no vertices.
    pub fn shape_half_extents(&self, shape_id: u16) -> Option<(i16, i16, i16)> {
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
        let mut table = std::collections::HashMap::new();
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
            let shape = Self::build_gpu_shape(&vertices, &faces, entry.color_table, true);
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
            Some(gpu.create_texture_r8(256, 128, TEX_BANK_01)),
            Some(gpu.create_texture_r8(256, 128, TEX_BANK_23)),
        ];
        self.sf2_texture_banks = [
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_0)),
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_1)),
            Some(gpu.create_texture_r8(256, 128, &sf2_data::textures::TEXTURE_BANK_2)),
        ];
        self.sf1_animation_frames.clear();
        for entry in shape_data::SHAPE_DATA.iter() {
            let table = crate::color_data::table_id_by_name(entry.default_color_table)
                .unwrap_or(SHAPE_COLTAB_ID_0 as u16);
            self.register_with_color(entry.shape_id, entry.vertices, entry.faces, table);
            if !entry.animation_frames.is_empty() {
                let frames = entry
                    .animation_frames
                    .iter()
                    .map(|vertices| Self::build_gpu_shape(vertices, entry.faces, table, false))
                    .collect();
                self.sf1_animation_frames.insert(entry.shape_id, frames);
            }
        }
        self.register_sf2_shapes();
    }

    /// Pick the COLDEPTH bank from the object's view-space depth
    /// (`Shapes_SelectDepthBank`).
    fn select_depth_bank(&self, view: &[f32; 16], model_matrix: &[f32; 16]) -> u8 {
        let x = model_matrix[12];
        let y = model_matrix[13];
        let z = model_matrix[14];
        // Camera looks down -Z in view space, so depth is the negated view Z.
        let depth = -(view[2] * x + view[6] * y + view[10] * z + view[14]);
        shapes::select_depth_bank(depth, self.depthz_table)
    }

    fn textured_face_vertices(
        &self,
        shape: &GpuShape,
        face: &ShapeFace,
        material: u16,
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
                out.push(Vertex3Tex {
                    pos: [v.x, v.y, v.z],
                    tex_info: [x as f32, y as f32, offset as f32, mask as f32],
                });
            }
        }
        Some((out, texture, high_nibble))
    }

    /// Mirror of `Shapes_Render`, pushing per-face flat triangles/lines to the
    /// retained `Gpu` with `transform`'s current proj/view. `palette` is the
    /// frame's decoded BGS-selected polygon palette.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        gpu: &mut Gpu,
        transform: &Transform,
        shape_id: u16,
        anim_frame: u8,
        col_frame: u8,
        color_table: u16,
        explosion_state: u8,
        model_matrix: &[f32; 16],
        palette: &shapes::ShapePaletteRgb,
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
                &frames[usize::from(anim_frame) % frames.len()]
            } else {
                let Some(shape) = self.shapes[flat_shape_id as usize].as_ref() else {
                    return;
                };
                shape
            }
        };

        let proj = *transform.projection();
        let view = *transform.view();
        let mut projection_view = [0.0; 16];
        crate::transform::multiply(&mut projection_view, &proj, &view);
        let mut projection_view_model = [0.0; 16];
        crate::transform::multiply(&mut projection_view_model, &projection_view, model_matrix);
        let depth_bank = self.select_depth_bank(&view, model_matrix);
        let light_obj = compute_object_light(model_matrix);
        let texture_palette: [[f32; 4]; 16] =
            std::array::from_fn(|i| [palette[i][0], palette[i][1], palette[i][2], 1.0]);

        for (i, face) in shape.faces.iter().enumerate() {
            let tri_start = shape.face_tri_start[i];
            let tri_count = shape.face_tri_count[i];

            // Both visible and hidden source faces become separate shards
            // during an explosion (`mexpfacesvis` / `mexpfacesnvis`).
            if explosion_state == 0
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
                    let pair = resolve_registered_face_palette_pair(
                        shape,
                        face.color_index,
                        col_frame,
                        color_table,
                        SHADE_TABLE_LEN as i32 - 1,
                        depth_bank,
                        self.depth_colors,
                    );
                    let first = shape.face_line_first[i] as usize;
                    if let Some(pair) = pair {
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

            let shade_index = shapes::compute_shade_index(shape.face_normals[i], light_obj);

            let material =
                resolve_registered_face_material(shape, face.color_index, col_frame, color_table);
            if material.is_some_and(|word| word & 0xC000 == shapes::MATERIAL_COLTEXT_FLAG) {
                if let Some((verts, texture, high_nibble)) =
                    self.textured_face_vertices(shape, face, material.unwrap())
                {
                    gpu.push_textured_tris(
                        &verts,
                        &proj,
                        &view,
                        &model,
                        &texture_palette,
                        high_nibble,
                        texture,
                    );
                    continue;
                }
            }

            let pair = resolve_registered_face_palette_pair(
                shape,
                face.color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
                self.depth_colors,
            );
            let start = (tri_start * 3) as usize;
            let count = (tri_count * 3) as usize;
            if let Some(pair) = pair {
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

impl Default for ShapeStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        face_is_camera_visible, texture_address, texture_material_fields, ShapeStore, TEXTURE_XY,
    };
    use crate::shape_data::{ShapeFace, ShapeVertex};
    use sf_core::shape::sf2_shape_id;

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
        ShapeStore::build_gpu_shape(&vertices, &faces, 0, false)
    }

    #[test]
    fn source_visibility_winding_is_rejected_in_gl_coordinates() {
        let identity = [
            1.0, 0.0, 0.0, 0.0, // column 0
            0.0, 1.0, 0.0, 0.0, // column 1
            0.0, 0.0, 1.0, 0.0, // column 2
            0.0, 0.0, 0.0, 1.0, // column 3
        ];

        let visible = visibility_test_shape(Some([0, 2, 1]));
        assert_eq!(visible.face_normals[0], [0.0, 1.0, 0.0]);
        assert!(face_is_camera_visible(
            &visible,
            &visible.faces[0],
            &identity
        ));

        let hidden = visibility_test_shape(Some([0, 1, 2]));
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
    fn texture_address_preserves_linear_row_carry() {
        // Adding X after masking carries through byte 0 exactly as the GSU
        // `add rspdata` does.  The old atlas-UV implementation returned
        // (0x17, row 0) instead of address 0x0117 here.
        assert_eq!(texture_address(0x00f8, 0x1f1f, 31, 0), 0x0117);
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
}
