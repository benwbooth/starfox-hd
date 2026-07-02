//! Byte-equality of the route2 level builders against the C oracle.
//!
//! Fixtures (`r2_<name>.bin` / `r2_<name>.regs.txt`) were dumped from the C
//! MapBuilder (`src/map/levels.c`) by the route2 lane's standalone harness
//! (`r2_dump.c`): the `.bin` is the emitted bytecode blob and `.regs.txt`
//! records `length`, the `native` MAP_CB_* addr24s and the `inline`
//! CODE65816 script ptrs in C registration-call order.

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
    let entry = route2::get_route2(id)
        .unwrap_or_else(|| panic!("{name}: map id {id} not ported in route2"));
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
fn level2_1_matches_c() {
    assert_level_matches("r2_level2_1", map_id::M2_1);
}

#[test]
fn level2_2_matches_c() {
    assert_level_matches("r2_level2_2", map_id::M2_2);
}

#[test]
fn level2_3_matches_c() {
    assert_level_matches("r2_level2_3", map_id::M2_3);
}

#[test]
fn level2_4_matches_c() {
    assert_level_matches("r2_level2_4", map_id::M2_4);
}

#[test]
fn level2_5_matches_c() {
    assert_level_matches("r2_level2_5", map_id::M2_5);
}

#[test]
fn level2_6_matches_c() {
    assert_level_matches("r2_level2_6", map_id::M2_6);
}

#[test]
fn special_matches_c() {
    assert_level_matches("r2_special", map_id::SPECIAL);
}

#[test]
fn credits_matches_c() {
    assert_level_matches("r2_credits", map_id::CREDITS);
}

#[test]
fn training_matches_c() {
    assert_level_matches("r2_training", map_id::TRAINING);
}
