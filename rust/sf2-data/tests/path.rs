#![cfg(feature = "oracle-data")]

use std::collections::BTreeSet;

use sf2_data::map::{MapAddress, MAP_COMMANDS, SPAWN_RECORDS};
use sf2_data::path::*;

const MIRAGE_DRAGON_MAP_BANK: u8 = 0x05;
const MIRAGE_DRAGON_SPAWN_ADDRESS: u16 = 0xE916;
const MIRAGE_DRAGON_PATH_INSTALL_ADDRESS: u16 = 0xE92C;
const MIRAGE_DRAGON_HEAD_SHAPE: u16 = 0xE1B0;
const MIRAGE_DRAGON_BODY_SHAPE: u16 = 0xE1E8;
const MIRAGE_DRAGON_TAIL_SHAPE: u16 = 0xE220;
const MIRAGE_DRAGON_ROOT_PATH: u16 = 0xF5B4;
const MIRAGE_DRAGON_CHAIN_GOSUB: u16 = 0xF5E9;
const MIRAGE_DRAGON_CHAIN_ENTRY: u16 = 0xE680;
const MIRAGE_DRAGON_CHAIN_RESUME: u16 = 0xF5EC;
const MIRAGE_DRAGON_HEAD_CHILD_SPAWN: u16 = 0xE682;
const MIRAGE_DRAGON_FIRST_BODY_PATH: u16 = 0xF837;
const MIRAGE_DRAGON_DESCENDANT_PATH: u16 = 0xF84D;
const MIRAGE_DRAGON_CHAIN_INITIALIZER: u16 = 0xF986;
const MIRAGE_DRAGON_CHAIN_INVERT: u16 = 0xF989;
const MIRAGE_DRAGON_CHAIN_LIMIT: u16 = 0xF98A;
const MIRAGE_DRAGON_CHAIN_RETURN: u16 = 0xF98F;
const MIRAGE_DRAGON_DESCENDANT_SPAWN: u16 = 0xF990;
const MIRAGE_DRAGON_TAIL_SHAPE_COMMAND: u16 = 0xF856;
const MIRAGE_DRAGON_INITIAL_FOLLOW: u16 = 0xF863;
const MIRAGE_DRAGON_PREDECESSOR_LINK_ENTER: u16 = 0xF886;
const MIRAGE_DRAGON_LOOP_FOLLOW: u16 = 0xF88A;
const MIRAGE_DRAGON_PREDECESSOR_LINK_EXIT: u16 = 0xF8A3;
const MIRAGE_DRAGON_PART_COUNT: u8 = 9;
const MIRAGE_DRAGON_PART_INDEX_VARIABLE: u8 = 0xA1;
const MIRAGE_DRAGON_RELATIVE_DEPTH_VARIABLE: u8 = 0x92;
const MIRAGE_DRAGON_CHILD_NUMBER: u8 = 80;
const MIRAGE_DRAGON_FAMILY_LINK_FIELD: u8 = 0x06;
const MIRAGE_DRAGON_PREDECESSOR_LINK_FIELD: u8 = 0x1C;
const MIRAGE_DRAGON_FIRST_PART_DEPTH: i16 = -45;
const MIRAGE_DRAGON_LATER_PART_DEPTH: i16 = -100;

fn path_command(address: u16) -> &'static PathCommand {
    PATH_COMMANDS
        .iter()
        .find(|command| command.address.offset == address)
        .unwrap_or_else(|| panic!("missing retail path command ${address:04X}"))
}

fn path_semantic(address: u16) -> PathSemantic {
    let command = path_command(address);
    PATH_HANDLERS
        .iter()
        .find(|handler| handler.opcode == command.opcode)
        .and_then(|handler| handler.semantic)
        .unwrap_or_else(|| panic!("missing reviewed semantic at ${address:04X}"))
}

#[test]
fn reachable_path_catalog_is_closed_and_self_consistent() {
    assert_eq!(PATH_ROOT_COUNT, 106);
    assert_eq!(PATH_COMMAND_COUNT, 14_220);
    assert_eq!(PATH_HANDLER_COUNT, 279);

    let handlers: BTreeSet<u16> = PATH_HANDLERS.iter().map(|handler| handler.opcode).collect();
    assert_eq!(handlers.len(), PATH_HANDLER_COUNT);
    assert!(PATH_COMMANDS.iter().all(|command| {
        handlers.contains(&command.opcode)
            && command.raw_len > command.prefix_size
            && command.raw_len as usize <= command.raw.len()
            && command.handler_address
                == PATH_HANDLERS
                    .iter()
                    .find(|handler| handler.opcode == command.opcode)
                    .unwrap()
                    .address
    }));
}

#[test]
fn map_derived_roots_match_retail_and_exclude_unreachable_aliases() {
    assert_eq!(PATH_ROOTS.first().unwrap().offset, 0x00BC);
    assert_eq!(PATH_ROOTS.last().unwrap().offset, 0xF5B4);

    assert!(PATH_COMMANDS.iter().all(|command| command.opcode != 0x180));
}

#[test]
fn handler_flow_preserves_wait_yield_and_dynamic_transitions() {
    let handler = |opcode| {
        PATH_HANDLERS
            .iter()
            .find(|handler| handler.opcode == opcode)
            .unwrap()
    };

    assert_eq!(
        handler(0x003).effects,
        &[
            FlowEffect {
                kind: FlowKind::Advance,
                value: Some(2),
                yields: false,
                resets_counter: true,
            },
            FlowEffect {
                kind: FlowKind::Hold,
                value: None,
                yields: true,
                resets_counter: false,
            },
        ]
    );
    assert_eq!(
        handler(0x041).effects,
        &[FlowEffect {
            kind: FlowKind::Call,
            value: Some(1),
            yields: false,
            resets_counter: false,
        }]
    );
    assert_eq!(
        handler(0x042).effects,
        &[FlowEffect {
            kind: FlowKind::Return,
            value: None,
            yields: false,
            resets_counter: false,
        }]
    );
}

#[test]
fn shared_spawn_handler_preserves_each_retail_record_width() {
    let alias = PATH_COMMANDS
        .iter()
        .find(|command| command.address.offset == 0x42B8)
        .unwrap();
    assert_eq!(alias.opcode, 0x033);
    assert_eq!(alias.raw_len, 17);
    assert_eq!(alias.successors, &[PathAddress { offset: 0x42C9 }]);

    let spawn = PATH_COMMANDS
        .iter()
        .find(|command| command.address.offset == 0x421F)
        .unwrap();
    assert_eq!(spawn.opcode, 0x0F5);
    assert_eq!(spawn.raw_len, 14);
    assert_eq!(spawn.successors, &[PathAddress { offset: 0x422D }]);
}

#[test]
fn only_reviewed_handler_semantics_are_named() {
    let named: Vec<_> = PATH_HANDLERS
        .iter()
        .filter_map(|handler| handler.semantic.map(|semantic| (handler.opcode, semantic)))
        .collect();
    assert_eq!(named.len(), PATH_HANDLER_COUNT);
    assert!(named.contains(&(0x003, PathSemantic::Wait)));
    assert!(named.contains(&(0x016, PathSemantic::Goto)));
    assert!(named.contains(&(0x07A, PathSemantic::ImportByteIndexed)));
    assert!(named.contains(&(0x041, PathSemantic::Gosub)));
    assert!(named.contains(&(0x042, PathSemantic::Return)));
    assert!(named.contains(&(0x02D, PathSemantic::IfBetweenWord)));

    assert!(PATH_HANDLERS.iter().all(|entry| entry.semantic.is_some()));
}

#[test]
fn force_trigger_targets_are_closed_over_the_retail_graph() {
    let addresses: BTreeSet<u16> = PATH_COMMANDS
        .iter()
        .map(|command| command.address.offset)
        .collect();

    let meteor = PATH_COMMANDS
        .iter()
        .find(|command| command.address.offset == 0x54F2)
        .unwrap();
    assert_eq!(meteor.opcode, 0x04C);
    assert_eq!(
        meteor.successors,
        &[
            PathAddress { offset: 0x54F5 },
            PathAddress { offset: 0x54F6 },
        ]
    );

    for command in PATH_COMMANDS
        .iter()
        .filter(|command| command.opcode == 0x04C)
    {
        let operand = usize::from(command.prefix_size) + 1;
        let target = u16::from_le_bytes([command.raw[operand], command.raw[operand + 1]]);
        assert!(addresses.contains(&target), "missing target ${target:04X}");
    }
}

#[test]
fn mirage_dragon_map_installs_the_retail_head_path() {
    let spawn_address = MapAddress {
        bank: MIRAGE_DRAGON_MAP_BANK,
        address: MIRAGE_DRAGON_SPAWN_ADDRESS,
    };
    let spawn = SPAWN_RECORDS
        .iter()
        .find(|spawn| spawn.address == spawn_address)
        .expect("Mirage Dragon spawn is reachable");
    assert_eq!(spawn.shape, MIRAGE_DRAGON_HEAD_SHAPE);
    assert_eq!((spawn.x, spawn.y, spawn.z), (0, 0, 0));

    let install = MAP_COMMANDS
        .iter()
        .find(|command| {
            command.address
                == MapAddress {
                    bank: MIRAGE_DRAGON_MAP_BANK,
                    address: MIRAGE_DRAGON_PATH_INSTALL_ADDRESS,
                }
        })
        .expect("Mirage Dragon path install is reachable");
    assert_eq!(
        &install.raw[..usize::from(install.raw_len)],
        &[
            install.opcode,
            MIRAGE_DRAGON_ROOT_PATH as u8,
            (MIRAGE_DRAGON_ROOT_PATH >> 8) as u8,
        ]
    );
    assert!(PATH_ROOTS
        .iter()
        .any(|root| root.offset == MIRAGE_DRAGON_ROOT_PATH));
}

#[test]
fn mirage_dragon_path_proves_the_linked_eight_body_and_tail_chain() {
    let chain_gosub = path_command(MIRAGE_DRAGON_CHAIN_GOSUB);
    assert_eq!(
        path_semantic(MIRAGE_DRAGON_CHAIN_GOSUB),
        PathSemantic::Gosub
    );
    assert_eq!(
        chain_gosub.successors,
        &[
            PathAddress {
                offset: MIRAGE_DRAGON_CHAIN_ENTRY,
            },
            PathAddress {
                offset: MIRAGE_DRAGON_CHAIN_RESUME,
            },
        ]
    );

    let head_child = path_command(MIRAGE_DRAGON_HEAD_CHILD_SPAWN);
    assert_eq!(
        path_semantic(MIRAGE_DRAGON_HEAD_CHILD_SPAWN),
        PathSemantic::SpawnChild
    );
    assert_eq!(
        &head_child.raw[..usize::from(head_child.raw_len)],
        &[
            head_child.opcode as u8,
            MIRAGE_DRAGON_BODY_SHAPE as u8,
            (MIRAGE_DRAGON_BODY_SHAPE >> 8) as u8,
            MIRAGE_DRAGON_FIRST_BODY_PATH as u8,
            (MIRAGE_DRAGON_FIRST_BODY_PATH >> 8) as u8,
            1,
            1,
            0,
            0,
            0,
            0,
            MIRAGE_DRAGON_FIRST_PART_DEPTH as u8,
            (MIRAGE_DRAGON_FIRST_PART_DEPTH >> 8) as u8,
            MIRAGE_DRAGON_CHILD_NUMBER,
        ]
    );

    for first_command in [MIRAGE_DRAGON_FIRST_BODY_PATH, MIRAGE_DRAGON_DESCENDANT_PATH] {
        assert_eq!(path_semantic(first_command), PathSemantic::Gosub);
        assert_eq!(
            path_command(first_command).successors,
            &[
                PathAddress {
                    offset: first_command + 3,
                },
                PathAddress {
                    offset: MIRAGE_DRAGON_CHAIN_INITIALIZER,
                },
            ]
        );
    }

    assert_eq!(
        path_semantic(MIRAGE_DRAGON_CHAIN_INVERT),
        PathSemantic::IfNot
    );
    let chain_limit = path_command(MIRAGE_DRAGON_CHAIN_LIMIT);
    assert_eq!(
        path_semantic(MIRAGE_DRAGON_CHAIN_LIMIT),
        PathSemantic::IfSameByte
    );
    assert_eq!(
        &chain_limit.raw[..usize::from(chain_limit.raw_len)],
        &[
            chain_limit.opcode as u8,
            MIRAGE_DRAGON_PART_INDEX_VARIABLE,
            MIRAGE_DRAGON_PART_COUNT,
            MIRAGE_DRAGON_DESCENDANT_SPAWN as u8,
            (MIRAGE_DRAGON_DESCENDANT_SPAWN >> 8) as u8,
        ]
    );
    assert_eq!(
        chain_limit.successors,
        &[
            PathAddress {
                offset: MIRAGE_DRAGON_CHAIN_RETURN,
            },
            PathAddress {
                offset: MIRAGE_DRAGON_DESCENDANT_SPAWN,
            },
        ]
    );

    let descendant_spawn = path_command(MIRAGE_DRAGON_DESCENDANT_SPAWN);
    assert_eq!(
        path_semantic(MIRAGE_DRAGON_DESCENDANT_SPAWN),
        PathSemantic::SpawnChild
    );
    assert_eq!(
        &descendant_spawn.raw[..usize::from(descendant_spawn.raw_len)],
        &[
            descendant_spawn.opcode as u8,
            MIRAGE_DRAGON_BODY_SHAPE as u8,
            (MIRAGE_DRAGON_BODY_SHAPE >> 8) as u8,
            MIRAGE_DRAGON_DESCENDANT_PATH as u8,
            (MIRAGE_DRAGON_DESCENDANT_PATH >> 8) as u8,
            1,
            1,
            0,
            0,
            0,
            0,
            MIRAGE_DRAGON_LATER_PART_DEPTH as u8,
            (MIRAGE_DRAGON_LATER_PART_DEPTH >> 8) as u8,
            MIRAGE_DRAGON_CHILD_NUMBER,
        ]
    );

    let tail_shape = path_command(MIRAGE_DRAGON_TAIL_SHAPE_COMMAND);
    assert_eq!(
        path_semantic(MIRAGE_DRAGON_TAIL_SHAPE_COMMAND),
        PathSemantic::SetWord
    );
    assert_eq!(
        &tail_shape.raw[..usize::from(tail_shape.raw_len)],
        &[
            tail_shape.opcode as u8,
            MIRAGE_DRAGON_TAIL_SHAPE as u8,
            (MIRAGE_DRAGON_TAIL_SHAPE >> 8) as u8,
            4,
        ]
    );

    for follow in [MIRAGE_DRAGON_INITIAL_FOLLOW, MIRAGE_DRAGON_LOOP_FOLLOW] {
        let command = path_command(follow);
        assert_eq!(
            path_semantic(follow),
            PathSemantic::PositionRelativeToLinkedVariable
        );
        assert_eq!(
            &command.raw[..usize::from(command.raw_len)],
            &[
                0,
                command.opcode as u8,
                MIRAGE_DRAGON_RELATIVE_DEPTH_VARIABLE
            ]
        );
    }

    for swap in [
        MIRAGE_DRAGON_PREDECESSOR_LINK_ENTER,
        MIRAGE_DRAGON_PREDECESSOR_LINK_EXIT,
    ] {
        let command = path_command(swap);
        assert_eq!(path_semantic(swap), PathSemantic::SwapVariableWords);
        assert_eq!(
            &command.raw[..usize::from(command.raw_len)],
            &[
                0,
                command.opcode as u8,
                MIRAGE_DRAGON_FAMILY_LINK_FIELD,
                MIRAGE_DRAGON_PREDECESSOR_LINK_FIELD,
            ]
        );
    }
}
