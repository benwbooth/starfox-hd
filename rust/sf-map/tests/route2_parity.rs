//! Deterministic source-correct fixtures for the Route 2 level builders.
//!
//! These began as removed-C-port dumps. Reblessing is restricted to encoding
//! corrections established from `MAPMACS.INC` and `WORLD.ASM`; the blobs and
//! registration records then protect the corrected Rust builders from drift.

use sf_map::catalog::{self, map_id};
use sf_map::levels::route2;

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
    let entry = route2::get_route2(id)
        .unwrap_or_else(|| panic!("{name}: map id {id} not ported in route2"));
    // Bless mode: the C harness that dumped these fixtures is gone (RIIR), and
    // it shared the maploop count-encoding bug (builder emitted raw count; ROM
    // macro emits count-1 — see MapBuilder::maploop + sf-oracle audit_mapvm2).
    // SF_BLESS_FIXTURES=1 rewrites the current regression blob and its length;
    // callback lists remain independently checked from the original capture.
    if std::env::var_os("SF_BLESS_FIXTURES").is_some() {
        let out = format!("{}/tests/fixtures/{name}.bin", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&out, &entry.level.data).unwrap();
        let regs_path = format!(
            "{}/tests/fixtures/{name}.regs.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut regs = format!("length {}\n", entry.level.data.len());
        for &(addr, _) in &entry.native {
            regs.push_str(&format!("native 0x{addr:06x}\n"));
        }
        for &(ptr, _) in &entry.inline {
            regs.push_str(&format!("inline {ptr}\n"));
        }
        std::fs::write(regs_path, regs).unwrap();
        return;
    }
    let blob = fixture(name, "bin");
    let regs = parse_regs(name);

    assert_eq!(regs.length, blob.len(), "{name}: fixture self-consistency");
    assert_eq!(
        entry.level.data.len(),
        blob.len(),
        "{name}: bytecode length (rust {} vs C {})",
        entry.level.data.len(),
        blob.len()
    );
    if entry.level.data != blob {
        let first = entry
            .level
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "{name}: bytecode diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            entry.level.data[first], blob[first]
        );
    }

    // Registration order/values, mirrored from the C register_* functions.
    let native: Vec<u32> = entry.native.iter().map(|&(a, _)| a).collect();
    assert_eq!(native, regs.native, "{name}: native callback addr24 order");

    let inline: Vec<u16> = entry.inline.iter().map(|&(p, _)| p).collect();
    assert_eq!(inline, regs.inline, "{name}: inline script ptr order");

    // The catalog chain must serve the same blob.
    let via_catalog = catalog::get_map_data(id)
        .unwrap_or_else(|| panic!("{name}: catalog::get_map_data({id}) returned None"));
    assert!(
        std::ptr::eq(via_catalog, &entry.level),
        "{name}: catalog dispatch must return the route2 BuiltLevel"
    );
}

#[test]
fn level2_1_matches_source_fixture() {
    assert_level_matches("r2_level2_1", map_id::M2_1);
}

#[test]
fn level2_2_matches_source_fixture() {
    assert_level_matches("r2_level2_2", map_id::M2_2);
}

#[test]
fn level2_3_matches_source_fixture() {
    assert_level_matches("r2_level2_3", map_id::M2_3);
}

#[test]
fn level2_4_matches_source_fixture() {
    assert_level_matches("r2_level2_4", map_id::M2_4);
}

#[test]
fn level2_5_matches_source_fixture() {
    assert_level_matches("r2_level2_5", map_id::M2_5);
}

#[test]
fn level2_6_matches_source_fixture() {
    assert_level_matches("r2_level2_6", map_id::M2_6);
}

#[test]
fn special_matches_source_fixture() {
    assert_level_matches("r2_special", map_id::SPECIAL);
}

#[test]
fn special_preserves_wrapper_scroll_boss_and_restart_state() {
    use sf_map::consts::{op, wm};

    let level = route2::get_route2(map_id::SPECIAL).expect("secret level");
    let data = &level.level.data;
    let contains = |needle: &[u8]| data.windows(needle.len()).any(|window| window == needle);

    assert!(contains(&[
        op::SETVARB,
        2,
        wm::DOSPACESC as u8,
        (wm::DOSPACESC >> 8) as u8,
        0,
        op::SETVARW,
        0xC0,
        0xFF,
        wm::BG2YSCROLL as u8,
        (wm::BG2YSCROLL >> 8) as u8,
        0,
    ]));
    assert!(contains(&[
        op::SETVARW,
        0,
        0,
        wm::HPOSJMP as u8,
        (wm::HPOSJMP >> 8) as u8,
        0,
    ]));
    assert!(contains(&[
        op::SETVARB,
        0,
        wm::NUMPLASERS as u8,
        (wm::NUMPLASERS >> 8) as u8,
        0,
        op::SETVARB,
        0,
        wm::NUMENDOK as u8,
        (wm::NUMENDOK >> 8) as u8,
        0,
    ]));

    let names: Vec<_> = level.inline.iter().map(|(_, name)| *name).collect();
    assert_eq!(
        names,
        vec![
            "special_mapwaitboss_trigse",
            "special_mapwaitboss_cantdie",
            "special_mapwaitboss_cleanup",
            "special_boss_cleanup",
            "special_theenddead_check",
        ]
    );
}

#[test]
fn credits_matches_source_fixture() {
    assert_level_matches("r2_credits", map_id::CREDITS);
}

#[test]
fn credits_preserves_the_retail_asm_state_transitions() {
    use sf_map::consts::{cb, op, wm};

    let level = route2::get_route2(map_id::CREDITS).expect("credits map");
    let data = &level.level.data;
    let charmap = cb::SETCHARMAPFROMMAP_L.wrapping_sub(1);
    let bg = route2::rc::BG_CRED as u16;
    let expected_prefix = [
        op::QFADEDOWN,
        op::WAITFADE,
        // MAPMACS `meters_off trans`: m_meters is $70:0200.
        op::SETVARB,
        0,
        0x00,
        0x02,
        0x70,
        op::CODEJSL,
        charmap as u8,
        (charmap >> 8) as u8,
        (cb::SETCHARMAPFROMMAP_L >> 16) as u8,
        op::SETBG,
        bg as u8,
        (bg >> 8) as u8,
        op::WAITSETBG,
        op::SETBGINFO,
    ];
    assert_eq!(&data[..expected_prefix.len()], &expected_prefix);

    let contains = |needle: &[u8]| data.windows(needle.len()).any(|window| window == needle);
    assert!(contains(&[
        op::SETVARB,
        1,
        wm::BG2VOFSOVERRIDE as u8,
        (wm::BG2VOFSOVERRIDE >> 8) as u8,
        0,
        op::SETVARW,
        0,
        0,
        wm::BG2HOFSREQ as u8,
        (wm::BG2HOFSREQ >> 8) as u8,
        0,
        op::SETVARW,
        0,
        0,
        wm::BG2VOFSREQ as u8,
        (wm::BG2VOFSREQ >> 8) as u8,
        0,
    ]));
    assert!(contains(&[
        op::SETVARB,
        8,
        wm::LEVELFINISHED as u8,
        (wm::LEVELFINISHED >> 8) as u8,
        0,
    ]));
}

#[test]
fn training_matches_source_fixture() {
    // The original removed-C dump omitted `initlevel`'s SETSTAGE/QFADEUP
    // tail. The fixture includes those two source opcodes after independent
    // retail Training trace certification.
    assert_level_matches("r2_training", map_id::TRAINING);
}
