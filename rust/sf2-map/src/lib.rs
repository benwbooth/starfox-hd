//! Executable Star Fox 2 map-script dispatcher.
//!
//! This crate executes only the 22 opcodes mechanically proven reachable
//! from the retail ROM's 25 script roots.  All engine-visible effects are
//! delegated to [`Sf2MapHost`]; there are no default no-op implementations.
//! The instruction pointer and yield behavior mirror the bank-03 retail
//! dispatcher at `$03:8FC9`.

use sf2_data::map::{
    ExternalPhaseGate, InlineAction, InlineExit, InlineProgram, MapAddress, MapCommand,
    SpawnRecord, EXTERNAL_PHASE_GATES, INLINE_EXITS, INLINE_PROGRAMS, MAP_COMMANDS, SCRIPT_ROOTS,
};
use sf2_data::map_vm::ReachableMapOp;

/// The host interface required by every reachable SF2 map operation.
///
/// Object allocation and the retail `$1651` "current object" value remain
/// host-owned.  In particular, `set_current_object_*` must implement the
/// retail rule that a zero/absent current object makes the write a no-op.
pub trait Sf2MapHost {
    type Error;

    fn request_stage_load(&mut self, table_offset: u16) -> Result<(), Self::Error>;
    fn set_current_object_byte(&mut self, field: u16, value: u8) -> Result<(), Self::Error>;
    fn display_ready(&self) -> bool;
    fn set_f3(&mut self, value: i8) -> Result<(), Self::Error>;
    fn write_long_byte(&mut self, address: u32, value: u8) -> Result<(), Self::Error>;
    fn write_long_word(&mut self, address: u32, value: u16) -> Result<(), Self::Error>;
    fn read_long_byte(&self, address: u32) -> Result<u8, Self::Error>;
    fn read_long_word(&self, address: u32) -> Result<u16, Self::Error>;
    fn load_table_idle(&self) -> bool;
    fn request_post_load(&mut self) -> Result<(), Self::Error>;

    /// Invoke one of the explicit external 65816 routines used by the map.
    /// `accumulator` contains the exact 8-bit immediate loaded by an inline
    /// block, or `None` when the caller does not establish A.
    fn call_65816(&mut self, target: u32, accumulator: Option<u8>) -> Result<(), Self::Error>;
    fn spawn_object(&mut self, record: SpawnRecord) -> Result<(), Self::Error>;
    fn set_current_object_path(&mut self, stream_offset: u16) -> Result<(), Self::Error>;
    fn spawn_aux_object(
        &mut self,
        field_04: u16,
        field_06: u16,
        field_08: u16,
        field_0b: u8,
        field_0d: u16,
        field_0f: u16,
    ) -> Result<(), Self::Error>;
    fn configure_slot(
        &mut self,
        slot: u8,
        bit_7_set: bool,
        params: [u8; 7],
    ) -> Result<(), Self::Error>;
    fn set_gsu_word_01bc(&mut self, value: u16) -> Result<(), Self::Error>;
    fn mode(&self) -> u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStop {
    /// The persistent stop opcode left the stream pointer on itself.
    Stopped,
    /// A delay or spawn installed a nonzero `$1655` value and returned.
    CounterSet(u16),
    /// Display state was not ready; the same opcode must be retried.
    WaitingForDisplay,
    /// The selected async load-table entry was nonzero; retry this opcode.
    WaitingForLoadTable,
    /// The caller's instruction budget was exhausted before a retail yield.
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunReport {
    pub commands_executed: usize,
    pub stop: RunStop,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MapVmError<E> {
    InvalidRoot(usize),
    CommandNotRecovered(MapAddress),
    MissingInlineExit(MapAddress),
    MissingInlineProgram(MapAddress),
    InvalidInlineContinuation { entry: MapAddress, returned: u16 },
    Host(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapVm {
    cursor: MapAddress,
    /// Exact last value written to retail `$1655`.  Counter consumption is an
    /// upstream engine concern and is deliberately not guessed here.
    counter: u16,
}

impl MapVm {
    pub fn new(root: MapAddress) -> Self {
        Self {
            cursor: root,
            counter: 0,
        }
    }

    pub fn from_root(root_index: usize) -> Result<Self, MapVmError<std::convert::Infallible>> {
        let root = SCRIPT_ROOTS
            .get(root_index)
            .ok_or(MapVmError::InvalidRoot(root_index))?;
        Ok(Self::new(root.address))
    }

    pub fn cursor(&self) -> MapAddress {
        self.cursor
    }

    pub fn counter(&self) -> u16 {
        self.counter
    }

    /// Supply the counter value produced by the surrounding retail movement
    /// system.  The map dispatcher itself only writes this value; it does not
    /// own the decrement rule.
    pub fn set_counter(&mut self, value: u16) {
        self.counter = value;
    }

    /// Release a retail `delay $1388; jump self` phase boundary.
    ///
    /// These boundaries are not ordinary map control flow: the surrounding
    /// stage state machine advances `$1657` from the parked jump to the byte
    /// following it.  The recovered gate table is exact and this method only
    /// succeeds when the VM is currently parked on one of those jumps.
    pub fn release_external_phase(&mut self) -> Option<ExternalPhaseGate> {
        let index = EXTERNAL_PHASE_GATES
            .binary_search_by_key(&self.cursor, |gate| gate.parked)
            .ok()?;
        let gate = EXTERNAL_PHASE_GATES[index];
        self.cursor = gate.continuation;
        self.counter = 0;
        Some(gate)
    }

    /// Execute instant commands until the retail handler would `RTS`, or
    /// until `command_budget` prevents an accidental infinite host loop.
    pub fn run<H: Sf2MapHost>(
        &mut self,
        host: &mut H,
        command_budget: usize,
    ) -> Result<RunReport, MapVmError<H::Error>> {
        let mut executed = 0usize;
        while executed < command_budget {
            let command =
                command_at(self.cursor).ok_or(MapVmError::CommandNotRecovered(self.cursor))?;
            let op = command
                .decode_reachable()
                .ok_or(MapVmError::CommandNotRecovered(self.cursor))?;
            executed += 1;

            match op {
                ReachableMapOp::Stop => {
                    return Ok(RunReport {
                        commands_executed: executed,
                        stop: RunStop::Stopped,
                    });
                }
                ReachableMapOp::Delay { ticks } => {
                    self.counter = ticks;
                    self.advance(command.size);
                    if ticks != 0 {
                        return Ok(RunReport {
                            commands_executed: executed,
                            stop: RunStop::CounterSet(ticks),
                        });
                    }
                }
                ReachableMapOp::RequestStageLoad { table_offset } => {
                    host.request_stage_load(table_offset)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::JumpBank { target } => self.cursor = target,
                ReachableMapOp::SetCurrentObjectByte { field, value } => {
                    host.set_current_object_byte(field, value)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::WaitForDisplayReady => {
                    if host.display_ready() {
                        self.advance(command.size);
                    } else {
                        // `$03:99D1` writes word value 1 to `$1655` and keeps
                        // `$1657` on the current opcode before returning.
                        self.counter = 1;
                        return Ok(RunReport {
                            commands_executed: executed,
                            stop: RunStop::WaitingForDisplay,
                        });
                    }
                }
                ReachableMapOp::SetF3 { value } => {
                    host.set_f3(value).map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::WriteLongByte { address, value } => {
                    host.write_long_byte(address, value)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::WriteLongWord { address, value } => {
                    host.write_long_word(address, value)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::WaitForLoadTableIdle => {
                    if host.load_table_idle() {
                        self.advance(command.size);
                    } else {
                        return Ok(RunReport {
                            commands_executed: executed,
                            stop: RunStop::WaitingForLoadTable,
                        });
                    }
                }
                ReachableMapOp::RequestPostLoad => {
                    host.request_post_load().map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::CallInline65816 => {
                    let inline = inline_exit_at(self.cursor)
                        .ok_or(MapVmError::MissingInlineExit(self.cursor))?;
                    let program = inline_program_at(self.cursor)
                        .ok_or(MapVmError::MissingInlineProgram(self.cursor))?;
                    let continuation = execute_inline(program.action, host)?;
                    if !inline.continuations.contains(&continuation) {
                        return Err(MapVmError::InvalidInlineContinuation {
                            entry: self.cursor,
                            returned: continuation,
                        });
                    }
                    self.cursor.address = continuation;
                }
                ReachableMapOp::Call65816 { target } => {
                    host.call_65816(target, None).map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::SpawnObject(record) => {
                    self.counter = record.delay;
                    host.spawn_object(record).map_err(MapVmError::Host)?;
                    self.advance(command.size);
                    if record.delay != 0 {
                        return Ok(RunReport {
                            commands_executed: executed,
                            stop: RunStop::CounterSet(record.delay),
                        });
                    }
                }
                ReachableMapOp::SetCurrentObjectPath { stream_offset } => {
                    host.set_current_object_path(stream_offset)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::SpawnAuxObject {
                    field_04,
                    field_06,
                    field_08,
                    field_0b,
                    field_0d,
                    field_0f,
                } => {
                    host.spawn_aux_object(
                        field_04, field_06, field_08, field_0b, field_0d, field_0f,
                    )
                    .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::ConfigureSlot {
                    slot,
                    bit_7_set,
                    params,
                } => {
                    host.configure_slot(slot, bit_7_set, params)
                        .map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::SetGsuWord01bc { value } => {
                    host.set_gsu_word_01bc(value).map_err(MapVmError::Host)?;
                    self.advance(command.size);
                }
                ReachableMapOp::BranchIfModeEq { mode, target } => {
                    if host.mode() == mode {
                        self.cursor = target;
                    } else {
                        self.advance(command.size);
                    }
                }
                ReachableMapOp::BranchIfModeBit0 { target } => {
                    if host.mode() & 1 != 0 {
                        self.cursor = target;
                    } else {
                        self.advance(command.size);
                    }
                }
                ReachableMapOp::BranchIfExternalE087Bit0400 { target } => {
                    if host.read_long_word(0x7E_E087).map_err(MapVmError::Host)? & 0x0400 != 0 {
                        self.cursor = target;
                    } else {
                        self.advance(command.size);
                    }
                }
            }
        }

        Ok(RunReport {
            commands_executed: executed,
            stop: RunStop::BudgetExhausted,
        })
    }

    fn advance(&mut self, size: u8) {
        self.cursor.address = self.cursor.address.wrapping_add(size as u16);
    }
}

pub fn command_at(address: MapAddress) -> Option<&'static MapCommand> {
    MAP_COMMANDS
        .binary_search_by_key(&address, |command| command.address)
        .ok()
        .map(|index| &MAP_COMMANDS[index])
}

pub fn inline_exit_at(address: MapAddress) -> Option<&'static InlineExit> {
    INLINE_EXITS
        .binary_search_by_key(&address, |inline| inline.address)
        .ok()
        .map(|index| &INLINE_EXITS[index])
}

pub fn inline_program_at(address: MapAddress) -> Option<&'static InlineProgram> {
    INLINE_PROGRAMS
        .binary_search_by_key(&address, |inline| inline.address)
        .ok()
        .map(|index| &INLINE_PROGRAMS[index])
}

fn execute_inline<H: Sf2MapHost>(
    action: InlineAction,
    host: &mut H,
) -> Result<u16, MapVmError<H::Error>> {
    match action {
        InlineAction::Call {
            target,
            accumulator,
            continuation,
        } => {
            host.call_65816(target, accumulator)
                .map_err(MapVmError::Host)?;
            Ok(continuation)
        }
        InlineAction::WordBits {
            address,
            mask,
            set_bits,
            continuation,
        } => {
            let old = host.read_long_word(address).map_err(MapVmError::Host)?;
            let value = if set_bits { old | mask } else { old & !mask };
            host.write_long_word(address, value)
                .map_err(MapVmError::Host)?;
            Ok(continuation)
        }
        InlineAction::BranchWordBits {
            address,
            mask,
            if_clear,
            if_set,
        } => {
            let value = host.read_long_word(address).map_err(MapVmError::Host)?;
            Ok(if value & mask == 0 { if_clear } else { if_set })
        }
        InlineAction::SetPilotLinkedFlag { continuation } => {
            set_pilot_linked_flag(host, 0x7E12C3)?;
            // Retail skips the second pilot only for the `$00C0` mode.
            if host.read_long_word(0x7E1916).map_err(MapVmError::Host)? != 0x00C0 {
                set_pilot_linked_flag(host, 0x7E12C5)?;
            }
            Ok(continuation)
        }
        InlineAction::SelectGsuProgram { continuation } => {
            let access = host.read_long_byte(0x00005E).map_err(MapVmError::Host)?;
            let disabled = access & !0x08;
            host.write_long_byte(0x00005E, disabled)
                .map_err(MapVmError::Host)?;
            host.write_long_byte(0x00303A, disabled)
                .map_err(MapVmError::Host)?;

            let selector = host.read_long_word(0x7E1B9C).map_err(MapVmError::Host)?;
            let entry = if selector & 0x0020 == 0 {
                0x8F44
            } else {
                0x8F48
            };
            host.write_long_word(0x700050, entry)
                .map_err(MapVmError::Host)?;

            let enabled = disabled | 0x08;
            host.write_long_byte(0x00005E, enabled)
                .map_err(MapVmError::Host)?;
            host.write_long_byte(0x00303A, enabled)
                .map_err(MapVmError::Host)?;
            Ok(continuation)
        }
    }
}

fn set_pilot_linked_flag<H: Sf2MapHost>(
    host: &mut H,
    pilot_pointer_address: u32,
) -> Result<(), MapVmError<H::Error>> {
    let pilot = host
        .read_long_word(pilot_pointer_address)
        .map_err(MapVmError::Host)?;
    // `LDY $2B,X` is direct-page indexed, hence bank-$00 WRAM mirror.
    let link_address = u32::from(pilot.wrapping_add(0x002B));
    let linked = host
        .read_long_word(link_address)
        .map_err(MapVmError::Host)?;
    // DBR=$7E for the absolute indexed `$6BEC,Y` read/write.
    let flag_address = 0x7E0000 | u32::from(0x6BECu16.wrapping_add(linked));
    let flags = host
        .read_long_byte(flag_address)
        .map_err(MapVmError::Host)?;
    host.write_long_byte(flag_address, flags | 0x40)
        .map_err(MapVmError::Host)
}
