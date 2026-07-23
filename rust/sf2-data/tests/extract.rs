//! Acceptance tests for the SF2 phase-1 data extraction (SF2_RECON.md).
//!
//! These validate the *structure* of the generated tables (self-contained; no
//! ROM needed at test time). Where possible they cross-check against the SF1
//! analogues the SF2 formats were matched to.

#[cfg(feature = "oracle-data")]
use sf2_data::map;
use sf2_data::{audio, colors, lighting, shape_data, text, textures};

// --------------------------------------------------------------------------
// (a) Audio: exact reset driver boundary and broad catalog are both preserved.
// --------------------------------------------------------------------------
#[test]
fn audio_driver_upload_and_catalog() {
    assert!(audio::AUDIO_BLOB_COUNT > 0);
    assert_eq!(audio::AUDIO_BLOBS.len(), audio::AUDIO_BLOB_COUNT);

    // Reset passes CPU $1A:8000, file $0D0000. It is an embedded upload-file
    // boundary inside the first broader terminator-delimited catalog region.
    // Starting at $0CBE1E would upload two unrelated preceding blocks.
    assert_eq!(audio::SPC_DRIVER_ENTRY, 0x0400);
    assert_eq!(audio::DRIVER_UPLOAD_START, 0x0D0000);
    assert_eq!(audio::DRIVER_UPLOAD_END, 0x0D2E81);
    assert_eq!(audio::DRIVER_UPLOAD_FILE, "SF2DRIVER.BIN");

    let first_region = &audio::AUDIO_BLOBS[0];
    assert!(first_region.contains_driver_code);
    assert_eq!(first_region.exec, audio::SPC_DRIVER_ENTRY);
    assert!(
        first_region
            .blocks
            .iter()
            .any(|block| block.dest == audio::SPC_DRIVER_ENTRY),
        "catalog region 0 must contain the embedded driver payload"
    );

    // The chain starts at the recon offset and is contiguous: every blob's
    // rom_off equals the previous blob's rom_end (the [00 00][exec] terminator
    // is included in each blob), and the last ends at the chain end. This is
    // what makes the whole chain re-parse identically to SF1's uploader.
    assert_eq!(audio::AUDIO_CHAIN_START, 0xCBE1E);
    assert_eq!(first_region.rom_off, audio::AUDIO_CHAIN_START);
    assert!(audio::DRIVER_UPLOAD_START > first_region.rom_off);
    assert_eq!(audio::DRIVER_UPLOAD_END, first_region.rom_end);
    for pair in audio::AUDIO_BLOBS.windows(2) {
        assert_eq!(pair[1].rom_off, pair[0].rom_end, "blobs must be contiguous");
    }
    let last = audio::AUDIO_BLOBS.last().unwrap();
    assert_eq!(last.rom_end, audio::AUDIO_CHAIN_END);

    // Every blob must carry at least one upload block (a well-formed chain).
    for b in &audio::AUDIO_BLOBS {
        assert!(!b.blocks.is_empty(), "blob {} has no blocks", b.id);
        for blk in b.blocks {
            assert!(blk.size > 0);
        }
    }
}

// --------------------------------------------------------------------------
// (b) Shapes: the complete contiguous ShapeHdr table and its exact meshes.
// --------------------------------------------------------------------------
#[test]
fn complete_shape_table_is_self_consistent() {
    assert_eq!(shape_data::SHAPE_DATA_COUNT, 577);
    assert_eq!(shape_data::SHAPE_DATA.len(), shape_data::SHAPE_DATA_COUNT);
    assert_eq!(shape_data::SHAPE_DATA[0].shape_id, 0xBC9C);
    assert_eq!(shape_data::SHAPE_DATA[576].shape_id, 0xFB9C);

    let mut vertex_count = 0usize;
    let mut animation_frame_count = 0usize;
    let mut animated_shape_count = 0usize;
    let mut face_count = 0usize;
    let mut procedural_count = 0usize;
    for s in &shape_data::SHAPE_DATA {
        assert_eq!(
            s.shape_id,
            shape_data::SHAPE_HEADER_START + s.header_index * shape_data::SHAPE_HEADER_SIZE
        );
        assert_eq!(
            shape_data::shape_by_id(s.shape_id).unwrap().shape_id,
            s.shape_id
        );
        vertex_count += s.vertices.len();
        animation_frame_count += s.animation_frames.len();
        animated_shape_count += usize::from(!s.animation_frames.is_empty());
        face_count += s.faces.len();
        procedural_count += usize::from(s.procedural);
        for f in s.faces {
            assert!((2..=12).contains(&f.num_verts));
            for k in 0..f.num_verts as usize {
                assert!((f.vertex_indices[k] as usize) < s.vertices.len());
            }
        }
        for frame in s.animation_frames {
            assert_eq!(frame.len(), s.vertices.len());
            for face in s.faces {
                for index in 0..usize::from(face.num_verts) {
                    assert!(usize::from(face.vertex_indices[index]) < frame.len());
                }
            }
        }
    }
    assert_eq!(vertex_count, 11_860);
    assert_eq!(animated_shape_count, 135);
    assert_eq!(animation_frame_count, 1_342);
    assert_eq!(face_count, 10_524);
    assert_eq!(procedural_count, 2);

    assert!(shape_data::shape_by_id(0xBC9B).is_none());
    assert!(shape_data::shape_by_id(0xBC9D).is_none());
    assert!(shape_data::shape_by_id(0xFBB8).is_none());

    let craft = shape_data::shape_by_id(0xEA00).unwrap();
    assert_eq!(craft.points_address, 0x0F938D);
    assert_eq!(craft.faces_address, 0x0F93B6);
    assert_eq!(craft.shift, 4);
    assert_eq!(craft.vertices.len(), 18);
    assert_eq!(craft.faces.len(), 26);

    #[cfg(feature = "oracle-data")]
    {
        for spawn in map::SPAWN_RECORDS {
            assert!(
                shape_data::shape_by_id(spawn.shape).is_some(),
                "map spawn {:02X}:{:04X} uses unknown ShapeHdr ${:04X}",
                spawn.address.bank,
                spawn.address.address,
                spawn.shape
            );
        }
    }
}

// --------------------------------------------------------------------------
// (c) Colors: material words decode to N valid material classes.
// --------------------------------------------------------------------------
#[test]
fn colors_decode_to_material_words() {
    assert_eq!(colors::MATERIAL_WORDS.len(), colors::MATERIAL_WORD_COUNT);
    assert!(colors::MATERIAL_WORD_COUNT > 0);
    assert_eq!(colors::COLOR_TABLE_ROM_OFF, 0x806C);

    // The vast majority classify as the known SF1 material classes (COLNORM /
    // COLDEPTH / COLANIM / COLTEXT / COLLITE / COLSMOOTH); only a small tail of
    // boundary words falls in OTHER. Cross-checks the $3E/$3F signature the
    // recon used and the discriminators in sf-render's resolver.
    use colors::MaterialClass::*;
    let mut known = 0usize;
    let (mut colnorm, mut coldepth) = (0usize, 0usize);
    for &w in colors::MATERIAL_WORDS.iter() {
        match colors::classify(w) {
            Other => {}
            c => {
                known += 1;
                if c == ColNorm {
                    colnorm += 1;
                }
                if c == ColDepth {
                    coldepth += 1;
                }
            }
        }
    }
    // >90% recognized material words.
    assert!(
        known * 100 >= colors::MATERIAL_WORD_COUNT * 90,
        "only {known}/{} words classified",
        colors::MATERIAL_WORD_COUNT
    );
    assert!(
        colnorm > 0 && coldepth > 0,
        "expected COLNORM and COLDEPTH words"
    );

    // The master sub-table (first MASTER_TABLE_LEN words) is dominated by
    // COLNORM entries (the $3F signature), as sampled at 0x806C.
    let master = &colors::MATERIAL_WORDS[..colors::MASTER_TABLE_LEN];
    let master_colnorm = master
        .iter()
        .filter(|&&w| colors::classify(w) == ColNorm)
        .count();
    assert!(master_colnorm > 0);
    assert_eq!(colors::classify(0x3FEE), ColNorm);
    assert_eq!(colors::classify(0x3E00), ColDepth);
    assert_eq!(colors::classify(0x8000), ColAnim);

    // Retail tables are not necessarily word-aligned. Keep lookup tied to
    // byte addresses instead of indexing the diagnostic aligned-word view.
    assert_eq!(colors::material_at(0x81F4, 0), Some(0x0000));
    assert_eq!(colors::material_at(0x81F4, 10), Some(0x3E00));
    assert_eq!(colors::material_at(0x85C1, 0), Some(0x85C5));
    assert_eq!(colors::resolve_animated_material(0x85C5, 0), Some(0x3FEE));
    assert_eq!(colors::resolve_animated_material(0x85C5, 1), Some(0x3F88));
    assert_eq!(colors::resolve_animated_material(0x85C5, 8), Some(0x3FEE));
    assert_eq!(colors::material_at(0x806B, 0), None);
}

#[test]
fn lighting_tables_match_the_sf2_retail_variants() {
    assert_eq!(lighting::STANDARD_DEPTH_PAIRS.len(), 4);
    assert!(lighting::STANDARD_DEPTH_PAIRS
        .iter()
        .all(|bank| bank.len() == 32));
    assert_eq!(lighting::STANDARD_DEPTH_PAIRS[0][25], 0x0E);
    assert_eq!(lighting::STANDARD_DEPTH_PAIRS[1][25], 0x0D);
    assert_eq!(lighting::STANDARD_DEPTH_PAIRS[2][25], 0x0C);
    assert_eq!(lighting::STANDARD_DEPTH_PAIRS[3][25], 0x0B);

    assert_eq!(lighting::SHADE_PAIRS.len(), 4);
    assert_eq!(
        lighting::SHADE_PAIRS[0][6],
        [0x11, 0x12, 0x12, 0x22, 0x22, 0x23, 0x23, 0x33, 0x34, 0x44]
    );
    assert_eq!(lighting::SHADE_PAIRS[3][9][9], 0xAF);
}

// --------------------------------------------------------------------------
// (d) Polygon textures: exact descriptors, layouts, and source banks.
// --------------------------------------------------------------------------
#[test]
fn polygon_texture_tables_are_complete_and_cover_every_shape_reference() {
    assert_eq!(textures::TEXTURE_SPRITES.len(), 211);
    assert_eq!(textures::TEXTURE_LAYOUTS.len(), 12);
    assert_eq!(textures::TEXTURE_BANK_0.len(), 32_768);
    assert_eq!(textures::TEXTURE_BANK_1.len(), 32_768);
    assert_eq!(textures::TEXTURE_BANK_2.len(), 32_768);

    assert_eq!(
        textures::TEXTURE_SPRITES[0],
        textures::TextureSprite { bank: 0, offset: 0 }
    );
    assert_eq!(textures::TEXTURE_SPRITES[69].offset, 0x50E0);
    assert!(textures::TEXTURE_SPRITES[70..128]
        .iter()
        .all(|descriptor| descriptor.bank == textures::UNUSED_TEXTURE_BANK));
    assert_eq!(textures::TEXTURE_SPRITES[128].bank, 0);
    assert_eq!(textures::TEXTURE_SPRITES[210].offset, 0x40C0);

    assert_eq!(textures::TEXTURE_LAYOUTS[0].mask, 0x1F1F);
    assert_eq!(
        textures::TEXTURE_LAYOUTS[0].coords,
        [[31, 0], [0, 0], [0, 31], [31, 31]]
    );
    assert_eq!(textures::TEXTURE_LAYOUTS[11].mask, 0x070F);

    let mut textured_faces = 0usize;
    for shape in &shape_data::SHAPE_DATA {
        for face in shape.faces {
            let root = colors::material_at(shape.color_table, face.color_index)
                .expect("shape color index must resolve through its exact table");
            let frame_count = if root & 0xC000 == 0x8000 { 128 } else { 1 };
            for frame in 0..frame_count {
                let material = colors::resolve_animated_material(root, frame as u8).unwrap_or(root);
                if material & 0xC000 != 0x4000 {
                    continue;
                }
                textured_faces += 1;
                assert!((3..=4).contains(&face.num_verts));
                let layout = usize::from((material >> 8) & 0x1F);
                let descriptor = usize::from(material & 0xFF);
                assert!(layout < textures::TEXTURE_LAYOUTS.len());
                assert!(descriptor < textures::TEXTURE_SPRITES.len());
                assert_ne!(
                    textures::TEXTURE_SPRITES[descriptor].bank,
                    textures::UNUSED_TEXTURE_BANK
                );
            }
        }
    }
    assert!(textured_faces > 100);
}

// --------------------------------------------------------------------------
// (e) Text: located name/label tables extracted with expected landmark strings.
// --------------------------------------------------------------------------
#[test]
fn text_tables_have_landmarks() {
    assert_eq!(text::TEXT_TABLE_COUNT, 4);

    let hud: Vec<&str> = text::HUD_LABELS.iter().map(|e| e.text).collect();
    assert!(hud.contains(&"NINTENDO"));
    assert!(hud.contains(&"CORNERIA"));

    // Boss names include the regional-carryover ANDORF + ANDROSS pair.
    let boss: Vec<&str> = text::BOSS_NAMES.iter().map(|e| e.text.trim()).collect();
    assert!(boss.contains(&"HAL BIRD"));
    assert!(boss.contains(&"ANDORF"));
    assert!(boss.contains(&"ANDROSS"));

    let rivals: Vec<&str> = text::RIVAL_NAMES.iter().map(|e| e.text.trim()).collect();
    assert!(rivals.contains(&"WOLF"));
    assert!(rivals.contains(&"PIGMA"));

    assert!(!text::CREDITS.is_empty());
    // Credits carry control-byte formatting prefixes (0x04 line commands).
    assert!(text::CREDITS.iter().any(|e| e.control.contains(&0x04)));
}
