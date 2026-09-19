//! Exercise original flat material selection without replacing source data.
use sf2_game::intro_material::{DepthGroup, FlatMaterial};
use sf_oracle::gsu::Gsu;

fn put(source: &mut Gsu, address: usize, value: u16) {
    source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn word(source: &Gsu, address: usize) -> u16 {
    u16::from_le_bytes(source.ram[address..address + 2].try_into().unwrap())
}

#[test]
fn depth_groups_match_original_scene_and_object_threshold_selection() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = Gsu::new(rom.clone());
    let groups = [
        DepthGroup::Near,
        DepthGroup::Middle,
        DepthGroup::Far,
        DepthGroup::Farthest,
    ];
    let mut seen = [false; 4];
    let mut overflow_cases = 0;
    for row in 0..14u16 {
        let address = 0x8F1C + row * 4;
        let thresholds = std::array::from_fn(|axis| rom[usize::from(address) + axis] as i8);
        for depth in i16::MIN..=i16::MAX {
            // Alternate the two original owners. A nonzero high byte must
            // not turn a zero low-byte selector into an object override.
            let override_enabled = depth as u16 & 1 != 0;
            put(
                &mut source,
                0x50,
                if override_enabled { 0x8F28 } else { address },
            );
            put(
                &mut source,
                0x1B6,
                0xA500 | if override_enabled { row + 1 } else { 0 },
            );
            put(&mut source, 0x2A, depth as u16);
            put(&mut source, 0x4E, 0x8B0C);
            source.watch_execution(1, 0x9527);
            source.start(1, 0x94C8);
            while !source.execution_watch_hit()
                && source.is_running()
                && source.last_run_steps < 200
            {
                source.run_slice(1);
            }
            assert!(source.execution_watch_hit(), "row={row} depth={depth}");
            let group = DepthGroup::for_camera_depth(depth, thresholds);
            let bank = groups
                .iter()
                .position(|candidate| *candidate == group)
                .unwrap();
            seen[bank] = true;
            assert_eq!(
                word(&source, 0x4C),
                0x8AAC + bank as u16 * 24,
                "shade row={row} depth={depth}"
            );
            assert_eq!(
                word(&source, 0x52),
                0x8B0C + bank as u16 * 32,
                "depth row={row} depth={depth}"
            );
            let widened_bank = thresholds
                .iter()
                .position(|threshold| i32::from(depth) + (i32::from(*threshold) << 8) < 0)
                .unwrap_or(3);
            overflow_cases += usize::from(widened_bank != bank);
        }
    }
    assert!(seen.into_iter().all(|visited| visited));
    assert!(
        overflow_cases > 0,
        "must distinguish wrapping from widened comparisons"
    );
}

#[test]
fn flat_palette_pairs_match_original_material_branches() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = Gsu::new(rom.clone());
    // Unmodified retail instructions set the data bank to one.
    source.start(1, 0x94C8);
    source.run_slice(3);
    let groups = [
        DepthGroup::Near,
        DepthGroup::Middle,
        DepthGroup::Far,
        DepthGroup::Farthest,
    ];
    let mut random = 0x9E85_8AACu32;
    let mut cases = 0;
    for word in 0..=u16::MAX {
        let Some(material) = FlatMaterial::from_word(word) else {
            continue;
        };
        for (bank, group) in groups.into_iter().enumerate() {
            for enabled in [false, true] {
                random ^= random << 13;
                random ^= random >> 17;
                random ^= random << 5;
                let address = 0x8000 + (random as usize % 32765);
                let normal = std::array::from_fn(|axis| rom[address + axis] as i8);
                let light = [random as i8, (random >> 8) as i8, (random >> 16) as i8];
                put(&mut source, 0x1C, 1);
                put(&mut source, 0x4C, 0x8AAC + bank as u16 * 24);
                put(&mut source, 0x52, 0x8B0C + bank as u16 * 32);
                put(&mut source, 0x54, if enabled { 0x8000 } else { 0 });
                for (axis, component) in light.into_iter().enumerate() {
                    put(&mut source, 0x106 + axis * 2, component as i16 as u16);
                }
                source.r[2] = address as u16 - 1;
                source.r[3] = word;
                source.watch_execution(1, 0x9EFD);
                source.start(1, 0x9E85);
                while !source.execution_watch_hit()
                    && source.is_running()
                    && source.last_run_steps < 500
                {
                    source.run_slice(1);
                }
                assert!(source.execution_watch_hit(), "word={word:04X}");
                assert_eq!(
                    u16::from(material.palette_pair(group, enabled, normal, light)),
                    source.r[0],
                    "word={word:04X} bank={bank} enabled={enabled}"
                );
                assert_eq!(
                    u16::from(material.palette_pair_at_depth(
                        bank as i16 * 4096,
                        [-16, -32, -48],
                        enabled,
                        normal,
                        light,
                    )),
                    source.r[0],
                    "depth-selected word={word:04X} bank={bank} enabled={enabled}"
                );
                // The watch executed ALT1. Consume its LMS before restarting
                // another independent material, preserving clean prefix state.
                source.run_slice(1);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, (12 * 256 + 32 + 256) * 4 * 2);
}

#[test]
fn non_flat_and_invalid_table_words_require_other_dispatch() {
    for word in [
        0x0C00, 0x3DFF, 0x3E20, 0x3EFF, 0x4000, 0x8000, 0xC000, 0xFFFF,
    ] {
        assert!(FlatMaterial::from_word(word).is_none(), "word={word:04X}");
    }
}
