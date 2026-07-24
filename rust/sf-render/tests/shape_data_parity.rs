//! Shape-table integrity checks. The generated Rust tables in
//! `sf-render/src/shape_data.rs` are emitted by `tools/shape_compiler.py`.
//!
//! (The former `shape_data_matches_c_header` cross-check compared these
//! tables against the generated C header `src/renderer/shape_data.h`. That
//! C tree — and its header oracle — has been removed, so the check is gone;
//! `shape_compiler.py` now emits the Rust table as the single source.)

use sf_render::shape_data::{
    SHAPE_DATA, SHAPE_EXT_DEBOSS_0, SHAPE_EXT_DEBOSS_2, SHAPE_EXT_ROBOT_0, SHAPE_EXT_ZACO_0,
    SHAPE_EXT_ZACO_7P, SHAPE_EXT_ZACO_8P,
};

#[test]
fn shape_headers_retain_their_rom_color_tables() {
    let named = |name: &str| {
        SHAPE_DATA
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("shape {name} missing"))
    };
    assert_eq!(named("asteroid1").default_color_table, "asteroid_c");
    assert_eq!(named("andross").default_color_table, "andross_c");
    assert_eq!(named("smoke").default_color_table, "smoke_c");
    assert_eq!(named("fireball").default_color_table, "fireball_c");
}

#[test]
fn generated_geometry_contains_every_source_animation_frame() {
    let named = |name: &str| {
        SHAPE_DATA
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("shape {name} missing"))
    };

    for (name, frame_count) in [
        ("flower", 13),
        ("leaf", 7),
        ("walk_4_l", 24),
        ("walk_4_r", 24),
        ("whale", 12),
        // The exported header says Frames 32 but provides 20 concrete rows;
        // both retail paper paths clamp their animation counter to 0..19.
        ("paper_1", 20),
    ] {
        let entry = named(name);
        assert_eq!(entry.animation_frames.len(), frame_count, "{name}");
        assert!(
            entry
                .animation_frames
                .iter()
                .all(|frame| frame.len() == entry.vertices.len()),
            "{name} frame vertex count drifted"
        );
        assert!(
            entry
                .animation_frames
                .windows(2)
                .any(|pair| pair[0] != pair[1]),
            "{name} animation collapsed to repeated frame zero"
        );
    }

    for entry in &SHAPE_DATA {
        let Some(max_index) = entry
            .faces
            .iter()
            .flat_map(|face| face.vertex_indices[..usize::from(face.num_verts)].iter())
            .max()
        else {
            continue;
        };
        for frame in entry.animation_frames {
            assert!(
                usize::from(*max_index) < frame.len(),
                "{} animation frame cannot satisfy its face indices",
                entry.name
            );
        }
    }
}

#[test]
fn source_visibility_triangles_are_resolved_and_in_bounds() {
    let mut one_sided_faces = 0usize;
    let mut two_sided_faces = 0usize;
    for entry in &SHAPE_DATA {
        for face in entry.faces {
            match face.visibility_vertices {
                Some(indices) => {
                    one_sided_faces += 1;
                    for index in indices {
                        assert!(
                            usize::from(index) < entry.vertices.len(),
                            "{} visibility vertex {} is out of bounds",
                            entry.name,
                            index
                        );
                    }
                    for frame in entry.animation_frames {
                        for index in indices {
                            assert!(
                                usize::from(index) < frame.len(),
                                "{} animated visibility vertex {} is out of bounds",
                                entry.name,
                                index
                            );
                        }
                    }
                }
                None => two_sided_faces += 1,
            }
        }
    }

    assert!(
        one_sided_faces > 5_000,
        "visibility metadata was not retained"
    );
    assert!(two_sided_faces > 0, "intentional two-sided faces were lost");
}

#[test]
fn authored_face_normals_are_retained_in_gl_coordinates() {
    let mut authored = 0usize;
    let mut zero_normals = 0usize;
    for entry in &SHAPE_DATA {
        for face in entry.faces {
            assert!(
                face.normal
                    .iter()
                    .all(|component| (-128..=128).contains(component)),
                "{} has a face normal outside signed-byte conversion range",
                entry.name
            );
            if face.normal == [0, 0, 0] {
                zero_normals += 1;
            } else {
                authored += 1;
            }
        }
    }

    assert!(authored > 5_000, "authored normals were not retained");
    assert!(zero_normals > 0, "zero-normal unlit records were lost");

    let arwing = SHAPE_DATA
        .iter()
        .find(|entry| entry.name == "myship_4")
        .expect("Arwing source mesh");
    assert_eq!(arwing.faces[0].normal, [-90, -90, 9]);

    let laser = SHAPE_DATA
        .iter()
        .find(|entry| entry.name == "elaser2")
        .expect("player laser source mesh");
    assert_eq!(laser.faces[0].normal, [0, -127, 0]);
}

#[test]
fn runtime_craft_laser_and_commander_use_complete_source_meshes() {
    let named = |name: &str| {
        SHAPE_DATA
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("shape {name} missing"))
    };

    let arwing = named("myship_4");
    assert_eq!(
        (arwing.shape_id, arwing.vertices.len(), arwing.faces.len()),
        (2, 16, 20)
    );

    let laser = named("elaser2");
    assert_eq!(laser.shape_id, 511);
    assert_eq!((laser.vertices.len(), laser.faces.len()), (6, 6));
    assert_eq!(laser.animation_frames.len(), 9);

    for (name, vertices, faces, frames) in [
        ("boss_7_0", 20, 17, 9),
        ("boss_7_1", 44, 44, 0),
        ("boss_7_1o", 26, 22, 0),
        ("boss_7_2", 10, 8, 0),
        ("boss_7_3", 11, 13, 10),
        ("boss_7_4", 11, 13, 10),
    ] {
        let entry = named(name);
        assert_eq!(entry.vertices.len(), vertices, "{name} vertices");
        assert_eq!(entry.faces.len(), faces, "{name} faces");
        assert_eq!(entry.animation_frames.len(), frames, "{name} frames");
    }
}

#[test]
fn every_reachable_texture_material_has_supported_face_arity() {
    use sf_render::color_data::{animation_frames, table_id_by_name, COLOR_TABLES};

    for entry in &SHAPE_DATA {
        let table_id = table_id_by_name(entry.default_color_table)
            .unwrap_or_else(|| panic!("unknown table {}", entry.default_color_table));
        let table = COLOR_TABLES[table_id as usize].entries;
        for face in entry.faces {
            let root = table[face.color_index as usize];
            let mut materials: Vec<u16> = vec![root];
            if root & 0xC000 == 0x8000 {
                if let Some(frames) = animation_frames(root & 0x3FFF) {
                    materials.extend_from_slice(frames);
                }
            }
            for material in materials {
                if material & 0xC000 == 0x4000 {
                    assert!(
                        (3..=4).contains(&face.num_verts),
                        "{} texture face has {} vertices",
                        entry.name,
                        face.num_verts
                    );
                }
            }
        }
    }
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

#[test]
fn intro_commander_side_shells_have_source_geometry() {
    for (id, name) in [
        (SHAPE_EXT_DEBOSS_0, "deboss_0"),
        (SHAPE_EXT_DEBOSS_2, "deboss_2"),
    ] {
        let entry = SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == id)
            .unwrap_or_else(|| panic!("{name} missing from shape catalog"));
        assert_eq!(entry.name, name);
        assert!(!entry.vertices.is_empty(), "{name} vertices");
        assert!(!entry.faces.is_empty(), "{name} faces");
    }
}

/// Tick 167: `zaco_8p` extended-bank extract (SHAPES2.ASM; szaco2 debris).
#[test]
fn zaco_8p_extended_mesh_matches_asm() {
    assert_eq!(SHAPE_EXT_ZACO_8P, 283);
    let entry = SHAPE_DATA
        .iter()
        .find(|e| e.shape_id == SHAPE_EXT_ZACO_8P)
        .expect("zaco_8p missing from SHAPE_DATA");
    assert_eq!(entry.name, "zaco_8p");
    // ShapeHdr shift=1; Points: (-12,0,-15), (-9,-9,-15), (11,8,0), (-12,0,15)
    // with HD Y-flip → ×2 world units.
    assert_eq!(entry.vertices.len(), 4);
    assert_eq!(
        (
            entry.vertices[0].x,
            entry.vertices[0].y,
            entry.vertices[0].z
        ),
        (-24.0, 0.0, -30.0)
    );
    assert_eq!(
        (
            entry.vertices[1].x,
            entry.vertices[1].y,
            entry.vertices[1].z
        ),
        (-18.0, 18.0, -30.0)
    );
    assert_eq!(
        (
            entry.vertices[2].x,
            entry.vertices[2].y,
            entry.vertices[2].z
        ),
        (22.0, -16.0, 0.0)
    );
    assert_eq!(
        (
            entry.vertices[3].x,
            entry.vertices[3].y,
            entry.vertices[3].z
        ),
        (-24.0, 0.0, 30.0)
    );
    assert_eq!(entry.faces.len(), 4);
    assert!(entry.faces.iter().all(|f| f.num_verts == 3));
}

/// Black Hole's native shape-shuffler must be able to display every direct
/// mesh in its source catalog, including the three shapes that are not map
/// shape rows.
#[test]
fn damyscr_direct_meshes_have_stable_native_ids() {
    let expected = [
        (SHAPE_EXT_ZACO_0, 418, "zaco_0"),
        (SHAPE_EXT_ZACO_7P, 419, "zaco_7p"),
        (SHAPE_EXT_ROBOT_0, 420, "robot_0"),
    ];

    for (shape_id, stable_id, name) in expected {
        assert_eq!(shape_id, stable_id);
        let entry = SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == shape_id)
            .unwrap_or_else(|| panic!("shape {name} missing"));
        assert_eq!(entry.name, name);
        assert!(!entry.vertices.is_empty());
        assert!(!entry.faces.is_empty());
    }
}

/// The catalog declaration itself is not a shape. This guards the exact
/// source numbering used by native object fields and prevents a global
/// one-row renderer shift from returning.
#[test]
fn source_shape_ids_select_the_named_meshes() {
    let expected = [
        (1, "exitlight"),
        (2, "myship_4"),
        (17, "round_0"),
        (55, "boss_7_1"),
        (105, "zaco_4"),
        (227, "tadpole"),
        (230, "zaco_1"),
        (240, "font_t"),
        (241, "font_h"),
        (242, "font_e"),
        (243, "font_n"),
        (244, "font_d"),
        (245, "gamesh"),
    ];

    for (shape_id, name) in expected {
        let entry = SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == shape_id)
            .unwrap_or_else(|| panic!("shape id {shape_id} missing"));
        assert_eq!(entry.name, name, "source shape id {shape_id} drifted");
    }
}

/// Runtime-visible ShapeHdr records added to the stable extended bank must
/// remain backed by real geometry. These ids are an ABI shared by sf-map,
/// sf-strat, sf-game, and the renderer.
#[test]
fn runtime_extended_mesh_catalog_is_complete() {
    let expected = [
        (322, "cockpit"),
        (323, "old_type"),
        (324, "item_0"),
        (325, "r_but_2"),
        (326, "walk_4_0"),
        (327, "arm"),
        (328, "bulge"),
        (329, "boss_e_0"),
        (330, "boss_e_1"),
        (331, "boss_e_1a"),
        (332, "boss_e_3"),
        (333, "boss_e_4"),
        (334, "ringlaser"),
        (335, "snake_0"),
        (336, "snake_3"),
        (337, "snake_4"),
        (338, "smark"),
        (339, "mmark"),
        (340, "lmark"),
        (341, "escapee"),
        (342, "lfdie"),
        (343, "andross"),
        (344, "androsscube"),
        (345, "face_0_1"),
        (346, "face_1"),
        (347, "face_box"),
        (348, "sface_b"),
        (349, "sface2_b"),
        (350, "para_1"),
        (351, "my_w"),
        (352, "my_r_w"),
        (353, "my_l_w"),
        (354, "my_b_w"),
        (355, "up1_man"),
        (356, "f_dra_1"),
        (357, "fire"),
        (358, "smoke"),
        (359, "ssplash"),
        (360, "splash"),
        (361, "pexplod"),
        (362, "boostshape"),
        (363, "firebreath"),
        (364, "lsmoke"),
        (365, "folsmoke"),
        (366, "androsshole"),
        (367, "spexplod"),
        (368, "myship_r"),
        (369, "myship_l"),
        (370, "myship_b"),
        (371, "my_up"),
        (372, "bmyship_4"),
        (373, "bmyship_r"),
        (374, "bmyship_l"),
        (375, "bmyship_b"),
        (376, "myzoom_4"),
        (377, "myzoom_r"),
        (378, "myzoom_l"),
        (379, "myzoom_b"),
        (380, "line"),
        (381, "boss_d_0"),
        (382, "boss_d_2"),
        (383, "neck"),
        (384, "grabber"),
        (385, "grabber2"),
        (386, "egg"),
        (387, "boss_d_8"),
        (388, "boss_d_9"),
        (389, "boss_d_6"),
        (390, "boss_d_7"),
        (391, "boss_9_0"),
        (392, "barrier"),
        (393, "fireface_b"),
        (394, "boss_a_3"),
        (395, "boss_a_4"),
        (396, "boss_a_5"),
        (397, "boss_b_l"),
        (398, "boss_b_r"),
        (399, "boss_b_h"),
        (400, "round0p"),
        (401, "ripair_w"),
        (402, "fireball"),
        (403, "missile"),
        (404, "ironball"),
        (405, "bouncyball"),
        (406, "shelpball"),
        (407, "nuke"),
        (408, "hyper"),
        (409, "hou_3"),
        (410, "my_demobs"),
        (411, "my_demos"),
        (412, "big_m"),
        (413, "boss_f_b"),
        (414, "walker_r"),
        (415, "playerbeam"),
        (416, "ovalbeam"),
        (417, "c_miss"),
        (426, "boss_a_6"),
        (427, "boss_f_8"),
        (428, "boss_f_9"),
        (429, "boss_f_8a"),
        (430, "boss_f_9a"),
        (431, "face_0"),
        (442, "flower"),
        (443, "big_bird"),
        (444, "leaf"),
        (445, "walk_4_l"),
        (446, "walk_4_r"),
        (447, "tow_1"),
        (448, "slot_1"),
        (449, "slot_2"),
        (450, "slot_3"),
        (451, "slot_4"),
        (452, "pillar3_ns"),
        (453, "laserline"),
        (454, "warp_1"),
        (455, "warp_2"),
        (456, "warp_3"),
        (457, "wall_l"),
        (458, "wall_r"),
    ];

    for (shape_id, name) in expected {
        let entry = SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == shape_id)
            .unwrap_or_else(|| panic!("shape {shape_id} ({name}) missing"));
        assert_eq!(entry.name, name, "stable id {shape_id} was reassigned");
        assert!(!entry.vertices.is_empty(), "shape {name} has no vertices");
        assert!(!entry.faces.is_empty(), "shape {name} has no faces");
    }
}
