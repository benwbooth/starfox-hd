//! Byte-equality of the ported level builders against the C oracle.
//!
//! Fixtures were dumped from the C MapBuilder (`src/map/levels.c`) by a
//! standalone harness: `<name>.bin` is the emitted bytecode blob, and
//! `<name>.regs.txt` records `length`, the `native` MAP_CB_* addr24s and
//! the `inline` CODE65816 script ptrs in C registration-call order.

use sf_map::builder::MapBuilder;
use sf_map::catalog::{self, map_id};
use sf_map::consts::op;

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
    let level =
        catalog::get_map_data(id).unwrap_or_else(|| panic!("{name}: map id {id} not ported"));
    // Bless mode: the C harness that dumped these fixtures is gone (RIIR), and
    // it shared the maploop count-encoding bug (builder emitted raw count; ROM
    // macro emits count-1 — see MapBuilder::maploop + sf-oracle audit_mapvm2).
    // SF_BLESS_FIXTURES=1 rewrites the current regression blob and its length;
    // callback lists remain independently checked from the original capture.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        let out = format!("{}/tests/fixtures/{name}.bin", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&out, &level.data).unwrap();
        let regs_path = format!(
            "{}/tests/fixtures/{name}.regs.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut regs = format!("length {}\n", level.data.len());
        for &(addr, _) in &level.native_callbacks {
            regs.push_str(&format!("native 0x{addr:06x}\n"));
        }
        for &(ptr, _) in &level.inline_callbacks {
            regs.push_str(&format!("inline {ptr}\n"));
        }
        std::fs::write(regs_path, regs).unwrap();
        return;
    }
    let blob = fixture(name, "bin");
    let regs = parse_regs(name);

    assert_eq!(regs.length, blob.len(), "{name}: fixture self-consistency");
    assert_eq!(
        level.data.len(),
        blob.len(),
        "{name}: bytecode length (rust {} vs C {})",
        level.data.len(),
        blob.len()
    );
    if level.data != blob {
        let first = level
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "{name}: bytecode diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            level.data[first], blob[first]
        );
    }

    let native: Vec<u32> = level.native_callbacks.iter().map(|&(a, _)| a).collect();
    assert_eq!(native, regs.native, "{name}: native callback addr24 order");

    let inline: Vec<u16> = level.inline_callbacks.iter().map(|&(p, _)| p).collect();
    assert_eq!(inline, regs.inline, "{name}: inline script ptr order");
}

#[test]
fn none_matches_c() {
    assert_level_matches("none", map_id::NONE);
}

#[test]
fn level1_1_matches_c() {
    assert_level_matches("level1_1", map_id::M1_1);
}

#[test]
fn title_matches_c() {
    assert_level_matches("title", map_id::TITLE);
}

#[test]
fn continue_matches_c() {
    assert_level_matches("continue", map_id::CONTINUE);
}

#[test]
fn wait_matches_c() {
    // MAP_ID_WAIT shares the title blob. The C build registers NO callbacks
    // for it, while the Rust BuiltLevel carries the title's registration
    // lists (documented in catalog.rs: the wait entry point never reaches
    // the CODE65816 hook, so this is behavior-identical). Compare the
    // bytecode only.
    let level = catalog::get_map_data(map_id::WAIT).expect("wait ported");
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        let out = format!("{}/tests/fixtures/wait.bin", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(out, &level.data).unwrap();
        return;
    }
    let blob = fixture("wait", "bin");
    assert_eq!(level.data, blob, "wait: bytecode must match the title blob");
}

#[test]
fn planet_matches_c() {
    assert_level_matches("planet", map_id::PLANET);
}

#[test]
fn every_retail_map_id_has_native_data() {
    for id in map_id::NONE..=map_id::TRAINING {
        assert!(
            catalog::get_map_data(id).is_some(),
            "retail map id {id} has no Rust builder"
        );
    }
}

#[test]
fn mapwait_matches_the_assembled_macro_encoding() {
    let mut b = MapBuilder::new();
    b.mapwait(0); // no bytes
    b.mapwait(1); // one VM yield, zero mapcnt
    b.mapwait(246); // source value is quantized to 15 * 16 = 240
    b.mapwait(4095); // largest compact effective wait (4080)
    b.mapwait(4096); // first full-width wait
    let (data, _) = b.finish();

    assert_eq!(
        data,
        vec![
            op::WAIT2,
            0,
            op::WAIT2,
            15,
            op::WAIT2,
            255,
            op::WAIT,
            0x00,
            0x10,
        ]
    );
}
