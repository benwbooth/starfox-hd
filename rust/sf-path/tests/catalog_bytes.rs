//! Byte-equality of the path catalog against its source oracle.
//!
//! When the user-owned assembled catalog is available, compare its complete
//! PATHDATA/DPATHDAT/KPATHDAT range directly with the reference build ROM.
//! Without that catalog, retain the old fixture check for the source-level
//! fallback builder.

use sf_core::shape::resolve_shape_word;
use sf_path::builder::{PAL_SHAPE, PATH_MISSING_OFFSET};
use sf_path::ids::{
    PATH_ID_CALL_FOL, PATH_ID_CARRIEDLOG, PATH_ID_CUTCREDS, PATH_ID_E_DOSUN, PATH_ID_E_KURURI,
    PATH_ID_E_KURURI2, PATH_ID_FOLOW, PATH_ID_ITADOSUN, PATH_ID_MINICASTANET,
    PATH_ID_MINICASTANETLR, PATH_ID_TOW_0, PATH_ID_TOW_1,
};
use sf_path::opcodes::*;
use sf_path::{literals, rom_catalog_data};

const SHAPE_TOWER_CHILD: u16 = 447;
const SHAPE_NONSOLID_PILLAR: u16 = 452;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> Vec<u8> {
    let path = fixture_path(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn fallback_path(catalog: &sf_path::catalog::PathCatalog, path_id: u16) -> &[u8] {
    let start = catalog.offsets[path_id as usize] as usize;
    let end = catalog
        .offsets
        .iter()
        .copied()
        .filter(|&offset| offset != PATH_MISSING_OFFSET && offset as usize > start)
        .min()
        .map_or(catalog.data.len(), usize::from);
    &catalog.data[start..end]
}

fn canonical_source_program(
    catalog: &sf_path::catalog::PathCatalog,
    path_id: u16,
    path_operands_are_offsets: bool,
) -> Vec<u8> {
    let start = catalog.offsets[path_id as usize] as usize;
    let mut program = fallback_path(catalog, path_id).to_vec();
    let mut ip = 0usize;

    while ip < program.len() {
        let opcode = program[ip];
        let (len, branch_operand) = match opcode {
            P_RELTOPLAYERON | P_RELTOPLAYEROFF | P_ALWAYSGENVECSON | P_FACEPLAYER
            | P_WAITFACEPLAYER | P_END | P_REMOVE | P_EXPLODE | P_SPACESHIPON | P_INVINCIBLEON
            | P_ZREMOVEON | P_ZREMOVEOFF | P_FIRE | P_RETURN | P_NEXT | P_COLLISIONSON
            | P_COLLISIONSOFF | P_WAIT1 => (1, None),
            P_WAIT | P_SETVEL | P_INITANIM | P_ADDROTX | P_ADDROTY | P_ADDROTZ | P_ADDWORLDX
            | P_ADDWORLDY | P_ADDWORLDZ | P_WEAPON | P_DOQ | P_SET0B | P_SET0W | P_INCW
            | P_SOUND | P_SOUND2 => (2, None),
            P_ADDB | P_SETB | P_ACHASEB | P_SETACCEL | P_DO => (3, None),
            P_SETW | P_ADDW => (4, None),
            P_GOTO | P_IGOTO | P_LEFTOFPLAYER | P_BEHINDPLAYER | P_ALWAYSOFF | P_FORCE => {
                (3, Some(1))
            }
            P_ALWAYS => (4, Some(1)),
            P_DISTLESS => (5, Some(3)),
            P_LOOP | P_IFZEROB | P_IFZEROW => (4, Some(2)),
            P_IFSAMEW | P_IFBETWEENB => (6, Some(4)),
            P_IFBETWEENW => (8, Some(6)),
            P_QSPAWN => {
                let shape_operand = ip + 1;
                if path_operands_are_offsets {
                    let shape =
                        u16::from_le_bytes([program[shape_operand], program[shape_operand + 1]]);
                    program[shape_operand..shape_operand + 2]
                        .copy_from_slice(&resolve_shape_word(shape).to_le_bytes());
                }
                let operand = ip + 3;
                let raw = u16::from_le_bytes([program[operand], program[operand + 1]]);
                let canonical_id = if path_operands_are_offsets {
                    [PATH_ID_FOLOW, PATH_ID_E_KURURI2]
                        .into_iter()
                        .find(|&id| catalog.offsets[id as usize] == raw)
                        .unwrap_or_else(|| panic!("unmapped assembled QSPAWN target {raw:#06x}"))
                } else {
                    raw
                };
                program[operand..operand + 2].copy_from_slice(&canonical_id.to_le_bytes());
                (7, None)
            }
            _ => panic!("unhandled opcode {opcode:#04x} at byte {ip} in source path {path_id}"),
        };

        if let Some(operand_offset) = branch_operand {
            let operand = ip + operand_offset;
            let target = u16::from_le_bytes([program[operand], program[operand + 1]]) as usize;
            assert!(
                (start..start + program.len()).contains(&target),
                "path {path_id} branch target {target:#06x} is outside its source program"
            );
            let relative_target = (target - start) as u16;
            program[operand..operand + 2].copy_from_slice(&relative_target.to_le_bytes());
        }

        ip += len;
    }
    assert_eq!(ip, program.len(), "path {path_id} instruction boundary");
    program
}

#[test]
fn fallback_uses_exact_path_only_shape_meshes() {
    let catalog = literals::build_fallback();

    let tower = fallback_path(&catalog, PATH_ID_TOW_0);
    let spawn = tower
        .windows(13)
        .find(|bytes| {
            bytes[0] == P_SPAWNLINK && u16::from_le_bytes([bytes[3], bytes[4]]) == PATH_ID_TOW_1
        })
        .expect("tow_0 must spawn its linked tow_1 child");
    assert_eq!(
        u16::from_le_bytes([spawn[1], spawn[2]]),
        SHAPE_TOWER_CHILD,
        "tow_1 must use its own flat mesh instead of the tow_0 body",
    );

    let carried_log = fallback_path(&catalog, PATH_ID_CARRIEDLOG);
    let set_shape = [
        P_SETW,
        SHAPE_NONSOLID_PILLAR as u8,
        (SHAPE_NONSOLID_PILLAR >> 8) as u8,
        PAL_SHAPE as u8,
    ];
    assert!(
        carried_log
            .windows(set_shape.len())
            .any(|bytes| bytes == set_shape),
        "carried pillar must switch to the exact non-solid pillar mesh",
    );
}

#[test]
fn catalog_matches_source_oracle() {
    let catalog = literals::get_catalog();

    if catalog.data.len() == rom_catalog_data::ROM_PATH_CATALOG_SIZE {
        let rom_path = format!("{}/../sf-oracle/data/sf.sfc", env!("CARGO_MANIFEST_DIR"));
        let rom = std::fs::read(&rom_path).unwrap_or_else(|e| panic!("read {rom_path}: {e}"));
        let bank = 4usize;
        let rom_start = bank * 0x8000 + (rom_catalog_data::ROM_PATH_CPU_BASE - 0x8000);
        let rom_end = bank * 0x8000 + (rom_catalog_data::ROM_PATH_CPU_END - 0x8000);

        assert_eq!(
            &catalog.data,
            &rom[rom_start..rom_end],
            "assembled path bank differs from the reference ROM",
        );
        assert_eq!(
            catalog
                .offsets
                .iter()
                .filter(|&&offset| offset != PATH_MISSING_OFFSET)
                .count(),
            rom_catalog_data::ROM_PATH_MAPPED_IDS,
            "symbol-derived path mapping count",
        );
        assert_eq!(
            catalog.offsets[PATH_ID_CUTCREDS as usize], PATH_MISSING_OFFSET,
            "CUTCREDS is a map subroutine, not path bytecode",
        );
        for &offset in catalog
            .offsets
            .iter()
            .filter(|&&offset| offset != PATH_MISSING_OFFSET)
        {
            assert!(
                (offset as usize) < rom_catalog_data::ROM_PATH_CATALOG_SIZE,
                "mapped path offset {offset:#06x} is outside the assembled path section",
            );
        }
        return;
    }

    if std::env::var("SF_BLESS_FIXTURES").is_ok() {
        let offsets_raw: Vec<u8> = catalog
            .offsets
            .iter()
            .flat_map(|o| o.to_le_bytes())
            .collect();
        std::fs::write(fixture_path("path_blob.bin"), &catalog.data).unwrap();
        std::fs::write(fixture_path("path_offsets.bin"), &offsets_raw).unwrap();
        std::fs::write(
            fixture_path("path_meta.txt"),
            format!("{} {}\n", catalog.data.len(), catalog.offsets.len()),
        )
        .unwrap();
        eprintln!("blessed path catalog fixtures");
        return;
    }
    let blob = fixture("path_blob.bin");
    let offsets_raw = fixture("path_offsets.bin");
    let meta = String::from_utf8(fixture("path_meta.txt")).unwrap();
    let mut meta_it = meta.split_ascii_whitespace();
    let blob_len: usize = meta_it.next().unwrap().parse().unwrap();
    let offset_count: usize = meta_it.next().unwrap().parse().unwrap();
    assert_eq!(blob_len, blob.len(), "fixture self-consistency");
    assert_eq!(
        offset_count * 2,
        offsets_raw.len(),
        "fixture self-consistency"
    );

    let offsets: Vec<u16> = offsets_raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    assert_eq!(
        catalog.data.len(),
        blob.len(),
        "blob length (rust {} vs C {})",
        catalog.data.len(),
        blob.len()
    );
    if catalog.data != blob {
        let first = catalog
            .data
            .iter()
            .zip(blob.iter())
            .position(|(a, b)| a != b)
            .unwrap();
        panic!(
            "blob diverges at offset {first}: rust {:#04x} vs C {:#04x}",
            catalog.data[first], blob[first]
        );
    }

    assert_eq!(catalog.offsets, offsets, "path offset table");
}

#[test]
fn fallback_minicastanet_programs_match_the_assembled_rom_bytes() {
    let fallback = literals::build_fallback();
    let assembled = literals::get_catalog();
    assert_eq!(
        assembled.data.len(),
        rom_catalog_data::ROM_PATH_CATALOG_SIZE,
        "this parity test needs the user-owned extracted path catalog"
    );

    let fallback_start = fallback.offsets[PATH_ID_MINICASTANET as usize] as usize;
    let fallback_lr = fallback.offsets[PATH_ID_MINICASTANETLR as usize] as usize;
    let assembled_start = assembled.offsets[PATH_ID_MINICASTANET as usize] as usize;
    let assembled_lr = assembled.offsets[PATH_ID_MINICASTANETLR as usize] as usize;
    fn assert_rebased_equal(fallback: &[u8], assembled: &[u8], delta: u16, name: &str) {
        assert_eq!(fallback.len(), assembled.len(), "{name} byte length");
        let mut i = 0;
        while i < fallback.len() {
            if fallback[i] == assembled[i] {
                i += 1;
                continue;
            }
            assert!(i + 1 < fallback.len(), "{name}: lone mismatch at {i}");
            let f = u16::from_le_bytes([fallback[i], fallback[i + 1]]);
            let a = u16::from_le_bytes([assembled[i], assembled[i + 1]]);
            assert_eq!(
                a,
                f.wrapping_add(delta),
                "{name}: non-fixup mismatch at byte {i}"
            );
            i += 2;
        }
    }

    assert_rebased_equal(
        &fallback.data[fallback_start..fallback_lr],
        &assembled.data[assembled_start..assembled_lr],
        (assembled_start as u16).wrapping_sub(fallback_start as u16),
        "minicastanet fallback transcription",
    );

    // These two programs are appended at the end of the fallback catalog, so
    // its remaining tail is exactly the LR program. Compare the same number
    // of bytes from the symbol-derived ROM start (including the terminal END).
    let lr_len = fallback.data.len() - fallback_lr;
    assert_rebased_equal(
        &fallback.data[fallback_lr..],
        &assembled.data[assembled_lr..assembled_lr + lr_len],
        (assembled_lr as u16).wrapping_sub(fallback_lr as u16),
        "minicastanetLR fallback transcription",
    );
}

#[test]
fn fallback_sector_z_and_venom_paths_match_the_assembled_rom_bytes() {
    let fallback = literals::build_fallback();
    let assembled = literals::get_catalog();
    assert_eq!(
        assembled.data.len(),
        rom_catalog_data::ROM_PATH_CATALOG_SIZE,
        "this parity test needs the user-owned extracted path catalog"
    );

    for (path_id, name) in [
        (PATH_ID_CALL_FOL, "call_fol"),
        (PATH_ID_FOLOW, "folow"),
        (PATH_ID_E_KURURI, "e_kururi"),
        (PATH_ID_E_KURURI2, "e_kururi2"),
        (PATH_ID_E_DOSUN, "e_dosun"),
        (PATH_ID_ITADOSUN, "itadosun"),
    ] {
        assert_eq!(
            canonical_source_program(&fallback, path_id, false),
            canonical_source_program(&assembled, path_id, true),
            "{name} fallback transcription differs from PATHDATA.ASM"
        );
    }
}
