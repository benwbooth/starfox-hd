//! Acceptance tests for the SF2 phase-1 data extraction (SF2_RECON.md).
//!
//! These validate the *structure* of the generated tables (self-contained; no
//! ROM needed at test time). Where possible they cross-check against the SF1
//! analogues the SF2 formats were matched to.

use sf2_data::{audio, colors, shape_data, text};

// --------------------------------------------------------------------------
// (a) Audio: first blob is the driver (dest $0400), chain terminates cleanly.
// --------------------------------------------------------------------------
#[test]
fn audio_driver_blob_and_chain() {
    assert!(audio::AUDIO_BLOB_COUNT > 0);
    assert_eq!(audio::AUDIO_BLOBS.len(), audio::AUDIO_BLOB_COUNT);

    // First blob is the driver: flagged, exec = driver entry, and it uploads a
    // block to $0400 (the driver code) — exactly what SF1's SGSOUND0 does and
    // how rust/sf-audio/src/boot.rs jumps to SPC_DRIVER_ENTRY after upload.
    let driver = &audio::AUDIO_BLOBS[0];
    assert!(driver.is_driver, "blob 0 must be the driver");
    assert_eq!(driver.exec, audio::SPC_DRIVER_ENTRY);
    assert_eq!(audio::SPC_DRIVER_ENTRY, 0x0400);
    assert!(
        driver.blocks.iter().any(|b| b.dest == 0x0400),
        "driver blob must upload code to $0400"
    );

    // The chain starts at the recon offset and is contiguous: every blob's
    // rom_off equals the previous blob's rom_end (the [00 00][exec] terminator
    // is included in each blob), and the last ends at the chain end. This is
    // what makes the whole chain re-parse identically to SF1's uploader.
    assert_eq!(audio::AUDIO_CHAIN_START, 0xCBE1E);
    assert_eq!(driver.rom_off, audio::AUDIO_CHAIN_START);
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
// (b) Shapes: candidate catalog is present with plausible vertex counts.
//     (Real geometry is deferred — see shape_data.rs docs.)
// --------------------------------------------------------------------------
#[test]
fn shape_candidates_plausible() {
    assert!(shape_data::SHAPE_DATA_COUNT > 0, "expected >0 shape candidates");
    assert_eq!(shape_data::SHAPE_DATA.len(), shape_data::SHAPE_DATA_COUNT);
    for s in &shape_data::SHAPE_DATA {
        // Point blocks parse to 2..=48 vertices per the strict SF1 grammar.
        assert!(
            (2..=48).contains(&s.vertices.len()),
            "candidate {} has implausible vertex count {}",
            s.shape_id,
            s.vertices.len()
        );
        // Faces are empty for now (deferred); indices, if any, stay in range.
        for f in s.faces {
            assert!((2..=12).contains(&f.num_verts));
            for k in 0..f.num_verts as usize {
                assert!((f.vertex_indices[k] as usize) < s.vertices.len());
            }
        }
        assert!(s.rom_off >= 0x90000 && s.rom_off < 0xC0000);
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
    assert!(colnorm > 0 && coldepth > 0, "expected COLNORM and COLDEPTH words");

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
}

// --------------------------------------------------------------------------
// (d) Text: located name/label tables extracted with expected landmark strings.
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
