//! GPU shape store: upload, per-face flat-color rendering, GSU shading.
//!
//! Port (C oracle): the runtime half of `src/renderer/shapes.c`
//! (`Shapes_Register`/`UploadShape`/`BuildFaceNormals`/`Shapes_Render`/
//! `Shapes_RegisterBuiltins`). The pure color math (material resolution,
//! shade index, depth banks) lives in [`crate::shapes`] and is wired in
//! here exactly like the C `Shapes_Render` loop.

use crate::builtin_shapes as bs;
use crate::gpu::{Gpu, Vertex3};
use crate::light_data::SHADE_TABLE_LEN;
use crate::shape_data::{self, ShapeFace, ShapeVertex};
use crate::shapes::{
    self, DEPTHZ_COUNT, DEPTHZ_NORMAL, SHAPE_BOSS7_0, SHAPE_BOSS7_1, SHAPE_BOSS7_1O,
    SHAPE_BOSS7_2, SHAPE_BOSS7_3, SHAPE_BOSS7_4, SHAPE_MYSHIP_4,
};
use crate::transform::Transform;

pub const MAX_SHAPES: usize = 512;

const SHAPE_INTERNAL_BOSS7_0_FRAME1: u16 = 480;
const SHAPE_INTERNAL_BOSS7_3_FRAME1: u16 = 488;
const SHAPE_INTERNAL_BOSS7_4_FRAME1: u16 = 497;

/// Mirror of `Shapes_ResolveBoss7Frame`: map an animated boss7 shape +
/// anim_frame onto the internal per-frame mesh slots.
pub fn resolve_boss7_frame(shape_id: u16, anim_frame: u8) -> u16 {
    match shape_id {
        SHAPE_BOSS7_0 => {
            if anim_frame == 0 {
                SHAPE_BOSS7_0
            } else {
                let f = anim_frame.min(8);
                SHAPE_INTERNAL_BOSS7_0_FRAME1 + (f as u16 - 1)
            }
        }
        SHAPE_BOSS7_3 => {
            if anim_frame == 0 {
                SHAPE_BOSS7_3
            } else {
                let f = anim_frame.min(9);
                SHAPE_INTERNAL_BOSS7_3_FRAME1 + (f as u16 - 1)
            }
        }
        SHAPE_BOSS7_4 => {
            if anim_frame == 0 {
                SHAPE_BOSS7_4
            } else {
                let f = anim_frame.min(9);
                SHAPE_INTERNAL_BOSS7_4_FRAME1 + (f as u16 - 1)
            }
        }
        _ => shape_id,
    }
}

/// A registered shape with CPU-side vertex buffers (C `Shape` with
/// `gpu_ready`). Instead of a VBO, triangle and line vertices are kept as
/// plain `Vertex3` vectors and pushed to the retained `Gpu` per draw.
pub struct GpuShape {
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
        if (fc[0] - centroid[0]) * n[0] + (fc[1] - centroid[1]) * n[1]
            + (fc[2] - centroid[2]) * n[2]
            < 0.0
        {
            n = [-n[0], -n[1], -n[2]];
        }
        normals[i] = n;
    }
    normals
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

pub struct ShapeStore {
    shapes: Vec<Option<GpuShape>>,
    depthz_table: usize,
}

impl ShapeStore {
    pub fn new() -> Self {
        let mut shapes = Vec::with_capacity(MAX_SHAPES);
        shapes.resize_with(MAX_SHAPES, || None);
        ShapeStore {
            shapes,
            depthz_table: DEPTHZ_NORMAL,
        }
    }

    /// Mirror of `Shapes_SetDepthTable` (`set_zdepthtable` map macro).
    pub fn set_depth_table(&mut self, table: usize) {
        self.depthz_table = if table >= DEPTHZ_COUNT { DEPTHZ_NORMAL } else { table };
    }

    /// Mirror of `Shapes_Get` (resolves raw shape words like the C).
    pub fn get(&self, shape_id: u16) -> Option<&GpuShape> {
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
        for v in &shape.vertices {
            mx = mx.max(v.x.abs());
            my = my.max(v.y.abs());
            mz = mz.max(v.z.abs());
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
            let Some(shape) = slot else { continue };
            if shape.vertices.is_empty() {
                continue;
            }
            let (mut mx, mut my, mut mz) = (0.0f32, 0.0f32, 0.0f32);
            for v in &shape.vertices {
                mx = mx.max(v.x.abs());
                my = my.max(v.y.abs());
                mz = mz.max(v.z.abs());
            }
            let clamp = |f: f32| f.round().clamp(0.0, i16::MAX as f32) as i16;
            table.insert(id as u16, (clamp(mx), clamp(my), clamp(mz)));
        }
        table
    }

    /// Mirror of `Shapes_Register` + `UploadShape`: fan-triangulate faces and
    /// collect Face2 line segments into CPU vertex buffers (`tri_verts` then
    /// `line_verts`) for the retained `Gpu` draw path.
    pub fn register(
        &mut self,
        shape_id: u16,
        verts: &[ShapeVertex],
        faces: &[ShapeFace],
    ) -> bool {
        let id = shape_id as usize;
        if id >= MAX_SHAPES {
            return false;
        }

        let vertices: Vec<ShapeVertex> = verts.to_vec();
        let faces: Vec<ShapeFace> = faces.to_vec();
        let face_normals = build_face_normals(&vertices, &faces);

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
                    tri_verts.push(Vertex3 { pos: [0.0, 0.0, 0.0] });
                    tri_verts.push(Vertex3 { pos: [0.0, 0.0, 0.0] });
                    tri_verts.push(Vertex3 { pos: [0.0, 0.0, 0.0] });
                    continue;
                }
                for &vi in &[v0, v1, v2] {
                    let v = &vertices[vi];
                    tri_verts.push(Vertex3 { pos: [v.x, v.y, v.z] });
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
                line_verts.push(Vertex3 { pos: [v.x, v.y, v.z] });
            }
        }

        self.shapes[id] = Some(GpuShape {
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
        });
        true
    }

    /// Mirror of `Shapes_RegisterBuiltins`: load every compiled ASM shape
    /// from [`crate::shape_data`], then overwrite the hand-tuned builtins
    /// (Arwing at 2, Boss7 family incl. internal frame slots).
    pub fn register_builtins(&mut self, _gpu: &mut Gpu) {
        for entry in shape_data::SHAPE_DATA.iter() {
            self.register(entry.shape_id, entry.vertices, entry.faces);
        }

        self.register(SHAPE_MYSHIP_4, &bs::ARWING_VERTS, &bs::ARWING_FACES);
        self.register(SHAPE_BOSS7_1, &bs::BOSS7_1_VERTS, &bs::BOSS7_1_FACES);
        self.register(SHAPE_BOSS7_0, &bs::BOSS7_0_VERTS, &bs::BOSS7_0_FACES);
        self.register_frame_variants(
            SHAPE_INTERNAL_BOSS7_0_FRAME1,
            &bs::BOSS7_0_VERTS,
            16,
            bs::BOSS7_0_FRAME_VERTS.iter().map(|f| &f[..]),
            &bs::BOSS7_0_FACES,
        );
        self.register(SHAPE_BOSS7_1O, &bs::BOSS7_1O_VERTS, &bs::BOSS7_1O_FACES);
        self.register(SHAPE_BOSS7_2, &bs::BOSS7_2_VERTS, &bs::BOSS7_2_FACES);
        self.register(SHAPE_BOSS7_3, &bs::BOSS7_3_VERTS, &bs::BOSS7_3_FACES);
        self.register_frame_variants(
            SHAPE_INTERNAL_BOSS7_3_FRAME1,
            &bs::BOSS7_3_VERTS,
            8,
            bs::BOSS7_3_FRAME_VERTS.iter().map(|f| &f[..]),
            &bs::BOSS7_3_FACES,
        );
        self.register(SHAPE_BOSS7_4, &bs::BOSS7_4_VERTS, &bs::BOSS7_4_FACES);
        self.register_frame_variants(
            SHAPE_INTERNAL_BOSS7_4_FRAME1,
            &bs::BOSS7_4_VERTS,
            8,
            bs::BOSS7_4_FRAME_VERTS.iter().map(|f| &f[..]),
            &bs::BOSS7_4_FACES,
        );
    }

    /// Mirror of `register_shape_frame_variants`.
    fn register_frame_variants<'a>(
        &mut self,
        first_shape_id: u16,
        frame0_verts: &[ShapeVertex],
        static_verts: usize,
        frames: impl Iterator<Item = &'a [ShapeVertex]>,
        faces: &[ShapeFace],
    ) {
        for (i, frame) in frames.enumerate() {
            let mut verts = frame0_verts.to_vec();
            verts[static_verts..static_verts + frame.len()].copy_from_slice(frame);
            self.register(first_shape_id + i as u16, &verts, faces);
        }
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

    /// Mirror of `Shapes_Render`, pushing per-face flat triangles/lines to the
    /// retained `Gpu` with `transform`'s current proj/view.
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
    ) {
        let shape_id = shapes::resolve_shape_word(shape_id);
        let shape_id = resolve_boss7_frame(shape_id, anim_frame);
        if shape_id as usize >= MAX_SHAPES {
            return;
        }
        let Some(shape) = self.shapes[shape_id as usize].as_ref() else {
            return;
        };

        let proj = *transform.projection();
        let view = *transform.view();
        let depth_bank = self.select_depth_bank(&view, model_matrix);
        let light_obj = compute_object_light(model_matrix);

        for (i, face) in shape.faces.iter().enumerate() {
            let tri_start = shape.face_tri_start[i];
            let tri_count = shape.face_tri_count[i];

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
                    let color = shapes::resolve_face_color(
                        shape_id,
                        face.color_index,
                        col_frame,
                        color_table,
                        SHADE_TABLE_LEN as i32 - 1,
                        depth_bank,
                    );
                    let first = shape.face_line_first[i] as usize;
                    gpu.push_flat_lines(
                        &shape.line_verts[first..first + 2],
                        &proj,
                        &view,
                        &model,
                        color,
                    );
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

            let color = shapes::resolve_face_color(
                shape_id,
                face.color_index,
                col_frame,
                color_table,
                shade_index,
                depth_bank,
            );
            let start = (tri_start * 3) as usize;
            let count = (tri_count * 3) as usize;
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

impl Default for ShapeStore {
    fn default() -> Self {
        Self::new()
    }
}
