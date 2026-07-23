#![cfg(feature = "oracle-data")]

use std::collections::BTreeSet;

use sf2_data::path::*;

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
