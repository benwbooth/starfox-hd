#![cfg(feature = "oracle-data")]

use std::collections::{BTreeMap, BTreeSet};

use sf2_data::map::*;
use sf2_data::map_vm::ReachableMapOp;

#[test]
fn reachable_map_program_has_no_scan_only_candidates() {
    assert_eq!(SCRIPT_ROOT_COUNT, 25);
    assert_eq!(MAP_COMMAND_COUNT, 4_094);
    assert_eq!(SPAWN_RECORD_COUNT, 232);
    assert_eq!(INLINE_EXIT_COUNT, 262);
    assert_eq!(INLINE_PROGRAM_COUNT, 262);
    assert_eq!(EXTERNAL_PHASE_GATE_COUNT, 237);

    assert!(MAP_COMMANDS.iter().all(|command| {
        command.raw_len >= 1 && command.raw_len <= 16 && command.raw[0] == command.opcode
    }));
    assert!(INLINE_EXITS
        .iter()
        .all(|inline| !inline.continuations.is_empty()));
}

#[test]
fn every_inline_exit_has_a_mechanically_typed_program() {
    let mut kinds = [0usize; 5];
    for (program, exits) in INLINE_PROGRAMS.iter().zip(INLINE_EXITS.iter()) {
        assert_eq!(program.address, exits.address);
        match program.action {
            InlineAction::Call { .. } => kinds[0] += 1,
            InlineAction::WordBits { .. } => kinds[1] += 1,
            InlineAction::BranchWordBits { .. } => kinds[2] += 1,
            InlineAction::SetPilotLinkedFlag { .. } => kinds[3] += 1,
            InlineAction::SelectGsuProgram { .. } => kinds[4] += 1,
        }
    }
    assert_eq!(kinds, [236, 7, 4, 8, 7]);
}

#[test]
fn map_spawn_roster_and_first_player_record_match_retail() {
    let first = SPAWN_RECORDS.first().unwrap();
    assert_eq!(
        first.address,
        MapAddress {
            bank: 0x05,
            address: 0x8003
        }
    );
    assert_eq!(first.opcode, 0x86);
    assert_eq!((first.x, first.y, first.z), (400, -150, 0));
    assert_eq!(first.shape, 0xBC9C);
    assert_eq!(first.strategy, 0x0682F9);

    let strategies: BTreeSet<u32> = SPAWN_RECORDS.iter().map(|s| s.strategy).collect();
    assert_eq!(
        strategies,
        BTreeSet::from([0x0682ED, 0x0682F9, 0x7F7E00, 0x7F7E1E])
    );

    let mut shapes = BTreeMap::<u16, usize>::new();
    for spawn in SPAWN_RECORDS {
        *shapes.entry(spawn.shape).or_default() += 1;
    }
    assert_eq!(shapes.len(), 50);
    assert_eq!(shapes[&0xBC9C], 50);
    assert_eq!(shapes[&0xD6A4], 12);
}

#[test]
fn external_phase_gates_include_the_live_battle_transitions() {
    assert!(EXTERNAL_PHASE_GATES.contains(&ExternalPhaseGate {
        hold: MapAddress {
            bank: 0x05,
            address: 0xE052
        },
        parked: MapAddress {
            bank: 0x05,
            address: 0xE055
        },
        continuation: MapAddress {
            bank: 0x05,
            address: 0xE059
        },
    }));
    assert!(EXTERNAL_PHASE_GATES.contains(&ExternalPhaseGate {
        hold: MapAddress {
            bank: 0x05,
            address: 0xE5B4
        },
        parked: MapAddress {
            bank: 0x05,
            address: 0xE5B7
        },
        continuation: MapAddress {
            bank: 0x05,
            address: 0xE5BB
        },
    }));
}

#[test]
fn reset_copy_mapping_resolves_all_ram_strategy_targets() {
    assert_eq!(wram_code_rom_file(0x7F3596), Some(0x013596));
    assert_eq!(wram_code_rom_file(0x7F7E00), Some(0x050000));
    assert_eq!(wram_code_rom_file(0x7F7E1E), Some(0x05001E));
    assert_eq!(wram_code_rom_file(0x7FCBFF), Some(0x054DFF));
    assert_eq!(wram_code_rom_file(0x7FCC00), None);
}

#[test]
fn every_reachable_command_has_handler_derived_semantics() {
    let opcodes: BTreeSet<u8> = MAP_COMMANDS.iter().map(|command| command.opcode).collect();
    assert_eq!(
        opcodes,
        BTreeSet::from([
            0x02, 0x10, 0x12, 0x2E, 0x36, 0x4C, 0x4E, 0x50, 0x5C, 0x5E, 0x64, 0x66, 0x78, 0x7A,
            0x86, 0x8C, 0x90, 0x94, 0x9A, 0x9E, 0xA2, 0xA4,
        ])
    );
    assert!(MAP_COMMANDS
        .iter()
        .all(|command| command.decode_reachable().is_some()));
}

#[test]
fn first_stage_setup_decodes_without_sf1_opcode_assumptions() {
    let command = |address| {
        MAP_COMMANDS
            .iter()
            .find(|command| command.address.address == address)
            .unwrap()
    };

    assert_eq!(
        command(0x8003).decode_reachable(),
        Some(ReachableMapOp::SpawnObject(SPAWN_RECORDS[0]))
    );
    assert_eq!(
        command(0x8035).decode_reachable(),
        Some(ReachableMapOp::SetF3 { value: -2 })
    );
    assert_eq!(
        command(0x8037).decode_reachable(),
        Some(ReachableMapOp::RequestStageLoad {
            table_offset: 0x0003
        })
    );
    assert_eq!(
        command(0x803C).decode_reachable(),
        Some(ReachableMapOp::Call65816 { target: 0x03DD6E })
    );
}

#[test]
fn object_path_and_long_write_operands_match_retail_bytes() {
    let path = MAP_COMMANDS
        .iter()
        .find(|command| command.address.address == 0x809D)
        .unwrap();
    assert_eq!(
        path.decode_reachable(),
        Some(ReachableMapOp::SetCurrentObjectPath {
            stream_offset: 0x7442
        })
    );

    let write = MAP_COMMANDS
        .iter()
        .find(|command| command.address.address == 0x802F)
        .unwrap();
    assert_eq!(
        write.decode_reachable(),
        Some(ReachableMapOp::WriteLongByte {
            address: 0x001D57,
            value: 1
        })
    );
}
