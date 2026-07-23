use std::collections::HashMap;
use std::convert::Infallible;

use sf2_data::map::{
    InlineAction, MapAddress, SpawnRecord, EXTERNAL_PHASE_GATES, INLINE_EXITS, INLINE_PROGRAMS,
    SCRIPT_ROOTS,
};
use sf2_map::{MapVm, MapVmError, RunStop, Sf2MapHost};

#[derive(Default)]
struct RecordingHost {
    display_ready: bool,
    load_idle: bool,
    mode: u8,
    spawns: Vec<SpawnRecord>,
    calls: Vec<(u32, Option<u8>)>,
    writes: Vec<(u32, u16, bool)>,
    memory: HashMap<u32, u8>,
}

impl Sf2MapHost for RecordingHost {
    type Error = Infallible;

    fn request_stage_load(&mut self, table_offset: u16) -> Result<(), Self::Error> {
        self.calls.push((0xFF_0000 | u32::from(table_offset), None));
        Ok(())
    }
    fn set_current_object_byte(&mut self, field: u16, value: u8) -> Result<(), Self::Error> {
        self.writes
            .push((u32::from(field), u16::from(value), false));
        Ok(())
    }
    fn display_ready(&self) -> bool {
        self.display_ready
    }
    fn set_f3(&mut self, value: i8) -> Result<(), Self::Error> {
        self.writes.push((0xF3, value as u8 as u16, false));
        Ok(())
    }
    fn write_long_byte(&mut self, address: u32, value: u8) -> Result<(), Self::Error> {
        self.writes.push((address, u16::from(value), false));
        self.memory.insert(address, value);
        Ok(())
    }
    fn write_long_word(&mut self, address: u32, value: u16) -> Result<(), Self::Error> {
        self.writes.push((address, value, true));
        let [low, high] = value.to_le_bytes();
        self.memory.insert(address, low);
        self.memory.insert(address + 1, high);
        Ok(())
    }
    fn read_long_byte(&self, address: u32) -> Result<u8, Self::Error> {
        Ok(*self.memory.get(&address).unwrap_or(&0))
    }
    fn read_long_word(&self, address: u32) -> Result<u16, Self::Error> {
        Ok(u16::from_le_bytes([
            *self.memory.get(&address).unwrap_or(&0),
            *self.memory.get(&(address + 1)).unwrap_or(&0),
        ]))
    }
    fn load_table_idle(&self) -> bool {
        self.load_idle
    }
    fn request_post_load(&mut self) -> Result<(), Self::Error> {
        self.calls.push((0xFF_0008, None));
        Ok(())
    }
    fn call_65816(&mut self, target: u32, accumulator: Option<u8>) -> Result<(), Self::Error> {
        self.calls.push((target, accumulator));
        Ok(())
    }
    fn spawn_object(&mut self, record: SpawnRecord) -> Result<(), Self::Error> {
        self.spawns.push(record);
        Ok(())
    }
    fn set_current_object_path(&mut self, stream_offset: u16) -> Result<(), Self::Error> {
        self.writes.push((0x2B, stream_offset, true));
        Ok(())
    }
    fn spawn_aux_object(
        &mut self,
        field_04: u16,
        field_06: u16,
        field_08: u16,
        field_0b: u8,
        field_0d: u16,
        field_0f: u16,
    ) -> Result<(), Self::Error> {
        self.calls.push((
            u32::from(field_04)
                ^ u32::from(field_06)
                ^ u32::from(field_08)
                ^ u32::from(field_0b)
                ^ u32::from(field_0d)
                ^ u32::from(field_0f),
            None,
        ));
        Ok(())
    }
    fn configure_slot(
        &mut self,
        slot: u8,
        bit_7_set: bool,
        params: [u8; 7],
    ) -> Result<(), Self::Error> {
        self.calls.push((
            u32::from(slot) | (u32::from(bit_7_set) << 8) | (u32::from(params[0]) << 16),
            None,
        ));
        Ok(())
    }
    fn set_gsu_word_01bc(&mut self, value: u16) -> Result<(), Self::Error> {
        self.writes.push((0x70_01BC, value, true));
        Ok(())
    }
    fn mode(&self) -> u8 {
        self.mode
    }
}

#[test]
fn initial_root_spawns_exact_first_object_then_follows_inline_exit() {
    let mut vm = MapVm::from_root(0).unwrap();
    let mut host = RecordingHost::default();
    let report = vm.run(&mut host, 2).unwrap();

    assert_eq!(report.commands_executed, 2);
    assert_eq!(report.stop, RunStop::BudgetExhausted);
    assert_eq!(host.spawns.len(), 1);
    assert_eq!(host.spawns[0].address, SCRIPT_ROOTS[0].address);
    assert_eq!(host.spawns[0].shape, 0xBC9C);
    assert!(INLINE_EXITS[0].continuations.contains(&vm.cursor().address));
}

#[test]
fn nonzero_delay_advances_then_yields_with_exact_counter() {
    let mut vm = MapVm::new(MapAddress {
        bank: 0x05,
        address: 0xFC20,
    });
    let mut host = RecordingHost::default();
    let report = vm.run(&mut host, 8).unwrap();
    assert_eq!(report.stop, RunStop::CounterSet(0x1388));
    assert_eq!(vm.counter(), 0x1388);
    assert_eq!(vm.cursor().address, 0xFC23);
}

#[test]
fn display_wait_retries_the_same_opcode_until_ready() {
    // Use the catalog's mechanically known first occurrence instead of
    // baking a stream location into the test.
    let wait = sf2_data::map::MAP_COMMANDS
        .iter()
        .find(|command| command.opcode == 0x4C)
        .unwrap();
    let mut vm = MapVm::new(wait.address);
    let original = vm.cursor();
    let mut host = RecordingHost::default();
    assert_eq!(
        vm.run(&mut host, 8).unwrap().stop,
        RunStop::WaitingForDisplay
    );
    assert_eq!(vm.cursor(), original);
    assert_eq!(vm.counter(), 1);

    host.display_ready = true;
    let report = vm.run(&mut host, 1).unwrap();
    assert_eq!(report.stop, RunStop::BudgetExhausted);
    assert_eq!(vm.cursor().address, original.address + 1);
}

#[test]
fn every_inline_program_matches_the_proven_exit_catalog() {
    assert_eq!(INLINE_PROGRAMS.len(), INLINE_EXITS.len());
    for (program, exits) in INLINE_PROGRAMS.iter().zip(INLINE_EXITS.iter()) {
        assert_eq!(program.address, exits.address);
        let continuations: Vec<u16> = match program.action {
            InlineAction::Call { continuation, .. }
            | InlineAction::WordBits { continuation, .. }
            | InlineAction::SetPilotLinkedFlag { continuation }
            | InlineAction::SelectGsuProgram { continuation } => vec![continuation],
            InlineAction::BranchWordBits {
                if_clear, if_set, ..
            } => vec![if_clear, if_set],
        };
        assert!(continuations
            .iter()
            .all(|continuation| exits.continuations.contains(continuation)));
    }
}

#[test]
fn typed_inline_call_preserves_the_retail_accumulator_argument() {
    let program = INLINE_PROGRAMS
        .iter()
        .find(|program| {
            matches!(
                program.action,
                InlineAction::Call {
                    accumulator: Some(1),
                    ..
                }
            )
        })
        .unwrap();
    let (target, continuation) = match program.action {
        InlineAction::Call {
            target,
            continuation,
            ..
        } => (target, continuation),
        _ => unreachable!(),
    };
    let mut vm = MapVm::new(program.address);
    let mut host = RecordingHost::default();
    assert_eq!(vm.run(&mut host, 1).unwrap().stop, RunStop::BudgetExhausted);
    assert_eq!(host.calls, vec![(target, Some(1))]);
    assert_eq!(vm.cursor().address, continuation);
}

#[test]
fn typed_inline_word_change_reads_modifies_and_writes_exact_wram_word() {
    let program = INLINE_PROGRAMS
        .iter()
        .find(|program| {
            matches!(
                program.action,
                InlineAction::WordBits { set_bits: true, .. }
            )
        })
        .unwrap();
    let (address, mask, continuation) = match program.action {
        InlineAction::WordBits {
            address,
            mask,
            continuation,
            ..
        } => (address, mask, continuation),
        _ => unreachable!(),
    };
    let mut vm = MapVm::new(program.address);
    let mut host = RecordingHost::default();
    host.write_long_word(address, 0x0042).unwrap();
    host.writes.clear();
    vm.run(&mut host, 1).unwrap();
    assert_eq!(host.read_long_word(address).unwrap(), 0x0042 | mask);
    assert_eq!(host.writes, vec![(address, 0x0042 | mask, true)]);
    assert_eq!(vm.cursor().address, continuation);
}

#[test]
fn typed_inline_pilot_link_update_uses_the_retail_indirection() {
    let program = INLINE_PROGRAMS
        .iter()
        .find(|program| matches!(program.action, InlineAction::SetPilotLinkedFlag { .. }))
        .unwrap();
    let mut vm = MapVm::new(program.address);
    let mut host = RecordingHost::default();
    host.write_long_word(0x7E12C3, 0x1300).unwrap();
    host.write_long_word(0x00132B, 0x0020).unwrap();
    host.write_long_byte(0x7E6C0C, 0x01).unwrap();
    // `$00C0` suppresses the second-pilot path.
    host.write_long_word(0x7E1916, 0x00C0).unwrap();
    host.writes.clear();
    vm.run(&mut host, 1).unwrap();
    assert_eq!(host.read_long_byte(0x7E6C0C).unwrap(), 0x41);
    assert_eq!(host.writes, vec![(0x7E6C0C, 0x41, false)]);
}

#[test]
fn typed_inline_gsu_selection_reproduces_access_gate_and_entry_write() {
    let program = INLINE_PROGRAMS
        .iter()
        .find(|program| matches!(program.action, InlineAction::SelectGsuProgram { .. }))
        .unwrap();
    let mut vm = MapVm::new(program.address);
    let mut host = RecordingHost::default();
    host.write_long_byte(0x00005E, 0x1F).unwrap();
    host.write_long_word(0x7E1B9C, 0x0020).unwrap();
    host.writes.clear();
    vm.run(&mut host, 1).unwrap();
    assert_eq!(host.read_long_word(0x700050).unwrap(), 0x8F48);
    assert_eq!(host.read_long_byte(0x00005E).unwrap(), 0x1F);
    assert_eq!(
        host.writes,
        vec![
            (0x00005E, 0x17, false),
            (0x00303A, 0x17, false),
            (0x700050, 0x8F48, true),
            (0x00005E, 0x1F, false),
            (0x00303A, 0x1F, false),
        ]
    );
}

#[test]
fn every_recovered_root_can_enter_the_runtime() {
    for (index, root) in SCRIPT_ROOTS.iter().enumerate() {
        let vm = MapVm::from_root(index).unwrap();
        assert_eq!(vm.cursor(), root.address);
    }
    assert!(matches!(
        MapVm::from_root(SCRIPT_ROOTS.len()),
        Err(MapVmError::InvalidRoot(_))
    ));
}

#[test]
fn host_can_release_only_an_exact_recovered_phase_gate() {
    let gate = EXTERNAL_PHASE_GATES
        .iter()
        .find(|gate| gate.parked.address == 0xE055)
        .copied()
        .unwrap();
    let mut vm = MapVm::new(gate.parked);
    vm.set_counter(0x1388);
    assert_eq!(vm.release_external_phase(), Some(gate));
    assert_eq!(vm.cursor(), gate.continuation);
    assert_eq!(vm.counter(), 0);
    assert_eq!(vm.release_external_phase(), None);
}
