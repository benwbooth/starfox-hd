//! Byte-equality of the route-3 level builders against the C oracle.
//!
//! Fixtures (`r3_*.bin` / `r3_*.regs.txt`) were dumped by a standalone C
//! harness that includes `src/map/levels.c`, calls `Levels_GetMapData(id)`
//! and records the `World_Register*` calls made while the map is selected
//! (same format as `level1_1.regs.txt`).
//!
//! Registration parity is checked against `Route3Level::{inline,native}_regs`
//! (raw ptr/name pairs) because the shared callback enums are off-limits to
//! the route-3 lane; see `levels/route3/mod.rs` for the consolidation TODO.

use sf_map::builder::MapBuilder;
use sf_map::catalog::{self, map_id};
use sf_map::consts::{BGM_BOSS1, BGM_FADEOUT, MEDPSPEED};
use sf_map::levels::route3;

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
    let mut regs = Regs {
        length: 0,
        native: Vec::new(),
        inline: Vec::new(),
    };
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
    let level = route3::get_level(id)
        .unwrap_or_else(|| panic!("{name}: map id {id} not in the route3 lane"));
    // Bless mode: the C harness that dumped these fixtures is gone (RIIR), and
    // it shared the maploop count-encoding bug (builder emitted raw count; ROM
    // macro emits count-1 — see MapBuilder::maploop + sf-oracle audit_mapvm2).
    // SF_BLESS_FIXTURES=1 rewrites the current regression blob and its length;
    // callback lists remain independently checked from the original capture.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        let out = format!("{}/tests/fixtures/{name}.bin", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&out, &level.built.data).unwrap();
        let regs_path = format!(
            "{}/tests/fixtures/{name}.regs.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut regs = format!("length {}\n", level.built.data.len());
        for &(addr, _) in &level.native_regs {
            regs.push_str(&format!("native 0x{addr:06x}\n"));
        }
        for &(ptr, _) in &level.inline_regs {
            regs.push_str(&format!("inline {ptr}\n"));
        }
        std::fs::write(regs_path, regs).unwrap();
        return;
    }
    let blob = fixture(name, "bin");
    let regs = parse_regs(name);

    assert_eq!(regs.length, blob.len(), "{name}: fixture self-consistency");
    assert_eq!(
        level.built.data.len(),
        blob.len(),
        "{name}: bytecode length (rust {} vs C {})",
        level.built.data.len(),
        blob.len()
    );
    if level.built.data != blob {
        let first = level
            .built
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "{name}: bytecode diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            level.built.data[first], blob[first]
        );
    }

    let native: Vec<u32> = level.native_regs.iter().map(|&(a, _)| a).collect();
    assert_eq!(native, regs.native, "{name}: native callback addr24 order");

    let inline: Vec<u16> = level.inline_regs.iter().map(|&(p, _)| p).collect();
    assert_eq!(inline, regs.inline, "{name}: inline script ptr order");

    // The catalog must serve the same blob through the lane dispatch.
    let via_catalog = catalog::get_map_data(id)
        .unwrap_or_else(|| panic!("{name}: catalog does not dispatch map id {id}"));
    assert!(
        std::ptr::eq(via_catalog, &level.built),
        "{name}: catalog dispatch returns a different level"
    );
}

#[test]
fn level3_1_matches_c() {
    assert_level_matches("r3_level3_1", map_id::M3_1);
}

#[test]
fn level3_2_matches_c() {
    assert_level_matches("r3_level3_2", map_id::M3_2);
}

#[test]
fn level3_3_matches_c() {
    assert_level_matches("r3_level3_3", map_id::M3_3);
}

#[test]
fn level3_4_matches_c() {
    assert_level_matches("r3_level3_4", map_id::M3_4);
}

#[test]
fn level3_5_matches_c() {
    assert_level_matches("r3_level3_5", map_id::M3_5);
}

#[test]
fn level3_5_uses_the_retail_boss_music_transition() {
    let level = route3::get_level(map_id::M3_5).expect("route 3 level 5");

    // MAPMACS.INC `fadeoutbgm` followed immediately by MAP3_5.ASM
    // `setbgm 5`. The intervening MSU-1 fade and 2,000-unit wait are
    // assembled out because CONFIG/ROM.INC defines MSU1 as zero.
    let mut expected = MapBuilder::new();
    expected.setbgm(BGM_FADEOUT);
    expected.mapwait(MEDPSPEED * 30);
    expected.setbgm(BGM_BOSS1);
    let (expected, _) = expected.finish();

    let occurrences = level
        .built
        .data
        .windows(expected.len())
        .filter(|window| *window == expected.as_slice())
        .count();
    assert_eq!(
        occurrences, 1,
        "retail boss transition must occur exactly once without the MSU-1-only wait"
    );
}

#[test]
fn level3_6_matches_c() {
    assert_level_matches("r3_level3_6", map_id::M3_6);
}

#[test]
fn level3_7_matches_c() {
    assert_level_matches("r3_level3_7", map_id::M3_7);
}

#[test]
fn final_matches_c() {
    assert_level_matches("r3_final", map_id::FINAL);
}
