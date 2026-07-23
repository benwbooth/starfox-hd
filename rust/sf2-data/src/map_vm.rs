//! Decoded semantics for every map opcode reachable from the retail SF2
//! script roots.
//!
//! The names here describe the handlers' observed side effects.  They do not
//! borrow SF1 opcode names: SF2 reordered the map language and several
//! commands manipulate SF2-only pools and WRAM state.

use crate::map::{MapAddress, MapCommand, SpawnRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReachableMapOp {
    /// Save the current stream pointer and return, so this record remains the
    /// permanent end/hold point.
    Stop,
    /// Install a nonzero delay and resume after this record when it expires.
    /// A zero value falls through immediately.
    Delay { ticks: u16 },
    /// Schedule the three-byte routine-table entry at this byte offset.
    RequestStageLoad { table_offset: u16 },
    /// Replace both the script bank and stream pointer.
    JumpBank { target: MapAddress },
    /// Write a byte to `current_object + field`, if a current object exists.
    SetCurrentObjectByte { field: u16, value: u8 },
    /// Wait until `$F4 == 0` and the RAM-resident display state byte is `$80`.
    WaitForDisplayReady,
    /// Write one of the two observed signed control values to direct page `$F3`.
    SetF3 { value: i8 },
    /// Write through an exact 24-bit address carried by the script.
    WriteLongByte { address: u32, value: u8 },
    /// Write through an exact 24-bit address carried by the script.
    WriteLongWord { address: u32, value: u16 },
    /// Yield while the routine-table entry selected by `$1642` is nonzero.
    WaitForLoadTableIdle,
    /// Set bit `$0008` in the async-work flags at `$1D27`.
    RequestPostLoad,
    /// Execute 65816 code beginning immediately after the opcode.  The code
    /// returns with the next stream offset in X.
    CallInline65816,
    /// Call an explicit 24-bit 65816 target with the current object in X.
    Call65816 { target: u32 },
    /// Allocate and initialize a main object from the full 14-byte record.
    SpawnObject(SpawnRecord),
    /// Install the bank-relative path stream offset in current object `+$2B`.
    SetCurrentObjectPath { stream_offset: u16 },
    /// Allocate an auxiliary-pool record.  These bytes are copied to its
    /// `+$04,+$06,+$08,+$0B,+$0D,+$0F` fields, respectively.
    SpawnAuxObject {
        field_04: u16,
        field_06: u16,
        field_08: u16,
        field_0b: u8,
        field_0d: u16,
        field_0f: u16,
    },
    /// Populate one 16-byte slot at `$7E:686A + slot * $10`.
    ConfigureSlot {
        slot: u8,
        bit_7_set: bool,
        params: [u8; 7],
    },
    /// Store the operand at the GSU-visible word `$70:01BC`.
    SetGsuWord01bc { value: u16 },
    /// Branch within the current script bank when `$1BA5` equals `mode`.
    BranchIfModeEq { mode: u8, target: MapAddress },
    /// Branch within the current script bank when bit 0 of `$1BA5` is set.
    BranchIfModeBit0 { target: MapAddress },
    /// Branch within the current script bank when bit `$0400` of the
    /// retail state word `$7E:E087` is set.
    BranchIfExternalE087Bit0400 { target: MapAddress },
}

#[inline]
fn word(raw: &[u8; 16], at: usize) -> u16 {
    u16::from_le_bytes([raw[at], raw[at + 1]])
}

#[inline]
fn script_target(command: &MapCommand, at: usize) -> MapAddress {
    MapAddress {
        bank: command.address.bank,
        address: 0x8000u16.wrapping_add(word(&command.raw, at)),
    }
}

#[inline]
fn long(raw: &[u8; 16], word_at: usize, bank_at: usize) -> u32 {
    ((raw[bank_at] as u32) << 16) | word(raw, word_at) as u32
}

impl MapCommand {
    /// Decode a command that is reachable from the 25 proven retail roots.
    ///
    /// `None` is deliberately returned for the 62 table entries which the
    /// current retail root graph never executes.  This prevents a future
    /// runtime from silently assigning guessed behavior to dead byte values.
    pub fn decode_reachable(&self) -> Option<ReachableMapOp> {
        Some(match self.opcode {
            0x02 => ReachableMapOp::Stop,
            0x10 => ReachableMapOp::RequestStageLoad {
                table_offset: word(&self.raw, 1),
            },
            0x12 => ReachableMapOp::Delay {
                ticks: word(&self.raw, 1),
            },
            0x2E => ReachableMapOp::JumpBank {
                target: MapAddress {
                    bank: self.raw[3],
                    address: 0x8000u16.wrapping_add(word(&self.raw, 1)),
                },
            },
            0x36 => ReachableMapOp::SetCurrentObjectByte {
                field: word(&self.raw, 1),
                value: self.raw[3],
            },
            0x4C => ReachableMapOp::WaitForDisplayReady,
            0x4E => ReachableMapOp::SetF3 { value: 2 },
            0x50 => ReachableMapOp::SetF3 { value: -2 },
            0x5C => ReachableMapOp::WriteLongByte {
                address: long(&self.raw, 2, 4),
                value: self.raw[1],
            },
            0x5E => ReachableMapOp::WriteLongWord {
                address: long(&self.raw, 3, 5),
                value: word(&self.raw, 1),
            },
            0x64 => ReachableMapOp::WaitForLoadTableIdle,
            0x66 => ReachableMapOp::RequestPostLoad,
            0x78 => ReachableMapOp::CallInline65816,
            0x7A => ReachableMapOp::Call65816 {
                target: long(&self.raw, 1, 3),
            },
            0x86 => ReachableMapOp::SpawnObject(SpawnRecord {
                address: self.address,
                opcode: self.opcode,
                delay: word(&self.raw, 1),
                x: word(&self.raw, 3) as i16,
                y: word(&self.raw, 5) as i16,
                z: word(&self.raw, 7) as i16,
                shape: word(&self.raw, 9),
                strategy: long(&self.raw, 11, 13),
                linked_object: None,
            }),
            0x8C => ReachableMapOp::SetCurrentObjectPath {
                stream_offset: word(&self.raw, 1),
            },
            0x90 => ReachableMapOp::SpawnAuxObject {
                field_04: word(&self.raw, 1),
                field_06: word(&self.raw, 3),
                field_08: word(&self.raw, 5),
                field_0b: self.raw[7],
                field_0d: word(&self.raw, 8),
                field_0f: word(&self.raw, 10),
            },
            0x94 => ReachableMapOp::ConfigureSlot {
                slot: self.raw[1] & 0x7F,
                bit_7_set: self.raw[1] & 0x80 != 0,
                params: self.raw[2..9].try_into().expect("fixed map record"),
            },
            0x9A => ReachableMapOp::SetGsuWord01bc {
                value: word(&self.raw, 1),
            },
            0x9E => ReachableMapOp::BranchIfModeEq {
                mode: self.raw[1],
                target: script_target(self, 2),
            },
            0xA2 => ReachableMapOp::BranchIfModeBit0 {
                target: script_target(self, 1),
            },
            0xA4 => ReachableMapOp::BranchIfExternalE087Bit0400 {
                target: script_target(self, 1),
            },
            _ => return None,
        })
    }
}
