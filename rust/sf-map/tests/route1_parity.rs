//! Byte-equality of the route1 (wave-2) level builders against the C oracle.
//!
//! Fixtures were dumped from the C MapBuilder (`src/map/levels.c`) by a
//! standalone harness that compiles levels.c with recording stubs for
//! `World_RegisterNativeCallback` / `World_RegisterInlineMapCode`, calls
//! `Levels_GetMapData(id)` once per map and writes `<name>.bin` (the blob)
//! plus `<name>.regs.txt` (`length`, `native 0x......` addr24s and
//! `inline <ptr>` script ptrs in C registration-call order) — the same
//! format as the phase-1 fixtures in `tests/fixture_parity.rs`.

use sf_map::catalog::{self, map_id};
use sf_map::levels::route1;

fn fixture(name: &str, ext: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}.{ext}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

struct Regs {
    length: usize,
    native: Vec<u32>,
    inline: Vec<u16>,
}

fn parse_regs(name: &str) -> Regs {
    let text = String::from_utf8(fixture(name, "regs.txt")).unwrap();
    let mut regs = Regs { length: 0, native: Vec::new(), inline: Vec::new() };
    for line in text.lines() {
        let mut it = line.split_ascii_whitespace();
        match (it.next(), it.next()) {
            (Some("length"), Some(v)) => regs.length = v.parse().unwrap(),
            (Some("native"), Some(v)) => {
                let v = v.trim_start_matches("0x");
                regs.native.push(u32::from_str_radix(v, 16).unwrap());
            }
            (Some("inline"), Some(v)) => regs.inline.push(v.parse().unwrap()),
            _ => {}
        }
    }
    regs
}

fn assert_level_matches(name: &str, id: u32) {
    let level = route1::get_full(id)
        .unwrap_or_else(|| panic!("{name}: map id {id} not ported in route1"));
    // Bless mode: the C harness that dumped these fixtures is gone (RIIR), and
    // it shared the maploop count-encoding bug (builder emitted raw count; ROM
    // macro emits count-1 — see MapBuilder::maploop + sf-oracle audit_mapvm2).
    // SF_BLESS_FIXTURES=1 rewrites the .bin from the current builder output
    // (lengths unchanged, so .regs.txt stays valid). Regression guard.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        let out = format!("{}/tests/fixtures/{name}.bin", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&out, &level.level.data).unwrap();
        return;
    }
    let blob = fixture(name, "bin");
    let regs = parse_regs(name);

    assert_eq!(regs.length, blob.len(), "{name}: fixture self-consistency");
    assert_eq!(
        level.level.data.len(),
        blob.len(),
        "{name}: bytecode length (rust {} vs C {})",
        level.level.data.len(),
        blob.len()
    );
    if level.level.data != blob {
        let first = level
            .level
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "{name}: bytecode diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            level.level.data[first], blob[first]
        );
    }

    // Route1 levels record registrations as raw (addr/ptr, name) pairs on
    // Route1Level (see route1/mod.rs consolidation TODO); verify the
    // addr24 / script-ptr ORDER against the C dump.
    let native: Vec<u32> = level.native_regs.iter().map(|&(a, _)| a).collect();
    assert_eq!(native, regs.native, "{name}: native callback addr24 order");

    let inline: Vec<u16> = level.inline_regs.iter().map(|&(p, _)| p).collect();
    assert_eq!(inline, regs.inline, "{name}: inline script ptr order");

    // The catalog must dispatch this id to the same built level.
    let via_catalog = catalog::get_map_data(id)
        .unwrap_or_else(|| panic!("{name}: catalog does not dispatch id {id}"));
    assert!(
        std::ptr::eq(via_catalog, &level.level),
        "{name}: catalog dispatch must reach the route1 level"
    );
}

#[test]
fn level1_2_matches_c() {
    assert_level_matches("r1_level1_2", map_id::M1_2);
}

#[test]
fn level1_3_matches_c() {
    assert_level_matches("r1_level1_3", map_id::M1_3);
}

#[test]
fn level1_4_matches_c() {
    assert_level_matches("r1_level1_4", map_id::M1_4);
}

#[test]
fn level1_5_matches_c() {
    assert_level_matches("r1_level1_5", map_id::M1_5);
}

#[test]
fn level1_6_matches_c() {
    assert_level_matches("r1_level1_6", map_id::M1_6);
}

#[test]
fn blackhole_matches_c() {
    assert_level_matches("r1_blackhole", map_id::BLACKHOLE);
}

#[test]
fn intro_matches_c() {
    assert_level_matches("r1_intro", map_id::INTRO);
}
