//! Parity test: the generated Rust shape tables must match the generated C
//! header (`src/renderer/shape_data.h`) exactly. Both files are emitted by
//! `tools/shape_compiler.py`; this test parses the C output and compares
//! every shape id, name, vertex component and face record byte-for-byte
//! (f32 values parsed from the identical decimal literals).

use sf_render::shape_data::{SHAPE_DATA, SHAPE_DATA_COUNT};
use std::collections::BTreeMap;
use std::path::PathBuf;

struct CShape {
    verts: Vec<[f32; 3]>,
    faces: Vec<([u16; 12], u8, u8)>,
    name: Option<String>,
}

fn c_header_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/renderer/shape_data.h")
}

fn parse_c_header() -> (BTreeMap<u16, CShape>, Vec<(u16, String)>, Vec<(String, u16)>) {
    let text = std::fs::read_to_string(c_header_path())
        .expect("read src/renderer/shape_data.h");

    let mut shapes: BTreeMap<u16, CShape> = BTreeMap::new();
    let mut table: Vec<(u16, String)> = Vec::new();
    let mut ext_defines: Vec<(String, u16)> = Vec::new();

    let mut lines = text.lines().peekable();
    let mut in_table = false;

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("#define SHAPE_EXT_") {
            let mut it = rest.split_whitespace();
            let name = it.next().expect("ext name").to_string();
            let id: u16 = it.next().expect("ext id").parse().expect("ext id parse");
            ext_defines.push((name, id));
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("static const ShapeVertex shape_") {
            let id: u16 = rest
                .split("_verts")
                .next()
                .unwrap()
                .parse()
                .expect("vertex array shape id");
            let mut verts = Vec::new();
            for vline in lines.by_ref() {
                let v = vline.trim();
                if v == "};" {
                    break;
                }
                // `{ 0.0f, -32.0f, 8.0f },` (trailing comma optional)
                let inner = v
                    .trim_end_matches(',')
                    .trim_start_matches('{')
                    .trim_end_matches('}');
                let comps: Vec<f32> = inner
                    .split(',')
                    .map(|c| {
                        c.trim()
                            .trim_end_matches('f')
                            .parse::<f32>()
                            .expect("vertex component")
                    })
                    .collect();
                assert_eq!(comps.len(), 3, "vertex arity in shape {id}");
                verts.push([comps[0], comps[1], comps[2]]);
            }
            shapes
                .entry(id)
                .or_insert_with(|| CShape { verts: Vec::new(), faces: Vec::new(), name: None })
                .verts = verts;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("static const ShapeFace shape_") {
            let id: u16 = rest
                .split("_faces")
                .next()
                .unwrap()
                .parse()
                .expect("face array shape id");
            let mut faces = Vec::new();
            for fline in lines.by_ref() {
                let f = fline.trim();
                if f == "};" {
                    break;
                }
                // `{ .vertex_indices = {0, 1, 2, ...}, .num_verts = 3, .color_index = 5 },`
                let idx_start = f.find(".vertex_indices = {").expect("indices") + 19;
                let idx_end = idx_start + f[idx_start..].find('}').expect("indices end");
                let mut indices = [0u16; 12];
                for (i, tok) in f[idx_start..idx_end].split(',').enumerate() {
                    indices[i] = tok.trim().parse().expect("vertex index");
                }
                let nv_start = f.find(".num_verts = ").expect("num_verts") + 13;
                let nv_end = nv_start + f[nv_start..].find(',').expect("num_verts end");
                let num_verts: u8 = f[nv_start..nv_end].trim().parse().expect("num_verts value");
                let ci_start = f.find(".color_index = ").expect("color_index") + 15;
                let ci_end = ci_start + f[ci_start..].find('}').expect("color_index end");
                let color_index: u8 =
                    f[ci_start..ci_end].trim().parse().expect("color_index value");
                faces.push((indices, num_verts, color_index));
            }
            shapes
                .entry(id)
                .or_insert_with(|| CShape { verts: Vec::new(), faces: Vec::new(), name: None })
                .faces = faces;
            continue;
        }

        if trimmed.starts_with("static const ShapeDataEntry g_shape_data[]") {
            in_table = true;
            continue;
        }
        if in_table {
            if trimmed == "};" {
                in_table = false;
                continue;
            }
            // `{ 2, shape_2_verts, sizeof(...), shape_2_faces, sizeof(...), "myship_4" },`
            let inner = trimmed.trim_start_matches('{');
            let id: u16 = inner
                .split(',')
                .next()
                .unwrap()
                .trim()
                .parse()
                .expect("table shape id");
            let name_start = trimmed.find('"').expect("table name") + 1;
            let name_end = name_start + trimmed[name_start..].find('"').expect("name end");
            let name = trimmed[name_start..name_end].to_string();
            shapes.get_mut(&id).expect("table id has arrays").name = Some(name.clone());
            table.push((id, name));
        }
    }

    (shapes, table, ext_defines)
}

#[test]
fn shape_data_matches_c_header() {
    let (c_shapes, c_table, c_ext) = parse_c_header();

    // Totals.
    assert_eq!(c_table.len(), 277, "C header shape count");
    assert_eq!(SHAPE_DATA_COUNT, c_table.len(), "Rust shape count");
    assert_eq!(SHAPE_DATA.len(), SHAPE_DATA_COUNT);
    assert_eq!(c_shapes.len(), c_table.len(), "C arrays vs table entries");

    // Same ids in the same (sorted) order, same names, same geometry.
    for (i, entry) in SHAPE_DATA.iter().enumerate() {
        let (c_id, c_name) = &c_table[i];
        assert_eq!(entry.shape_id, *c_id, "shape id at table index {i}");
        assert_eq!(entry.name, c_name, "shape name for id {c_id}");

        let c_shape = &c_shapes[c_id];
        assert_eq!(
            entry.vertices.len(),
            c_shape.verts.len(),
            "vertex count for shape {c_id}"
        );
        assert_eq!(
            entry.faces.len(),
            c_shape.faces.len(),
            "face count for shape {c_id}"
        );

        for (vi, (rv, cv)) in entry.vertices.iter().zip(&c_shape.verts).enumerate() {
            // Exact bit equality: both sides come from the same decimal
            // literal, so correctly-rounded parsing must agree.
            assert_eq!(
                [rv.x.to_bits(), rv.y.to_bits(), rv.z.to_bits()],
                [cv[0].to_bits(), cv[1].to_bits(), cv[2].to_bits()],
                "vertex {vi} of shape {c_id}"
            );
        }

        for (fi, (rf, (c_idx, c_nv, c_ci))) in
            entry.faces.iter().zip(&c_shape.faces).enumerate()
        {
            assert_eq!(rf.vertex_indices, *c_idx, "face {fi} indices, shape {c_id}");
            assert_eq!(rf.num_verts, *c_nv, "face {fi} num_verts, shape {c_id}");
            assert_eq!(rf.color_index, *c_ci, "face {fi} color_index, shape {c_id}");
        }
    }

    // Extended-bank consts: same set of defines, spot-checked against the
    // Rust consts (all 29 are generated by the same emitter loop).
    assert_eq!(c_ext.len(), 29, "SHAPE_EXT_* define count");
    let ext: BTreeMap<&str, u16> =
        c_ext.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    use sf_render::shape_data as sd;
    assert_eq!(ext["MYBASE_0"], sd::SHAPE_EXT_MYBASE_0);
    assert_eq!(ext["MY_BIRD"], sd::SHAPE_EXT_MY_BIRD);
    assert_eq!(ext["BIG_METEOR"], sd::SHAPE_EXT_BIG_METEOR);
    assert_eq!(ext["OP_0"], sd::SHAPE_EXT_OP_0);
    assert_eq!(ext["OP_2"], sd::SHAPE_EXT_OP_2);
}

#[test]
fn wireframe_segments_present() {
    // Face2 wireframe shapes (op_0 runway rails, shyper ring) must keep
    // their num_verts == 2 line segments.
    for id in [508u16, 268u16] {
        let entry = SHAPE_DATA
            .iter()
            .find(|e| e.shape_id == id)
            .unwrap_or_else(|| panic!("shape {id} missing"));
        assert!(
            entry.faces.iter().any(|f| f.num_verts == 2),
            "shape {id} should contain Face2 line segments"
        );
    }
}
