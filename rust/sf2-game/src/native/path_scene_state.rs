//! Scene-owned coordination shared by independently scheduled authored paths.

use super::path_fields::{ByteField, ByteOperand};
use super::Object;

/// These bytes are script-owned, not actor counters. Their precise stage and
/// bit assignments vary by encounter. Keep their complete wrapping values.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterCoordination {
    /// Primary progression ($D787); also observed by player service $06:9E03.
    pub progress: u8,
    /// Secondary progression ($D788), advanced by part-death paths and waited
    /// on by $44:1168/$44:1388 (including the full-byte sentinel 255).
    pub secondary_progress: u8,
    /// Completed-part bits ($D789), published by $44:38FE.
    pub completed_parts: u8,
    /// Concurrent message-service bits ($D78A), raised and cleared around
    /// the counted waits at $44:88B8/$44:88DB.
    pub active_messages: u8,
    /// Shared handshake ($D78B). Some paths use bits; others increment and
    /// decrement the whole byte. It is intentionally not a boolean set.
    pub handshake: u8,
    /// Persistent authored actor-retirement bits ($D78D). Actor paths retain
    /// their initial identity byte and set its bit through the shared helper.
    pub retired_actors: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationField {
    Progress,
    SecondaryProgress,
    CompletedParts,
    ActiveMessages,
    Handshake,
    RetiredActors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationCommand {
    CopyTo(ByteField),
    Assign(ByteOperand),
    Increment,
    Decrement,
}

impl EncounterCoordination {
    pub fn apply(
        &mut self,
        actor: &mut Object,
        field: CoordinationField,
        command: CoordinationCommand,
    ) {
        let value = match field {
            CoordinationField::Progress => &mut self.progress,
            CoordinationField::SecondaryProgress => &mut self.secondary_progress,
            CoordinationField::CompletedParts => &mut self.completed_parts,
            CoordinationField::ActiveMessages => &mut self.active_messages,
            CoordinationField::Handshake => &mut self.handshake,
            CoordinationField::RetiredActors => &mut self.retired_actors,
        };
        match command {
            CoordinationCommand::CopyTo(destination) => destination.write(actor, *value),
            CoordinationCommand::Assign(source) => *value = source.read(actor),
            CoordinationCommand::Increment => *value = value.wrapping_add(1),
            CoordinationCommand::Decrement => *value = value.wrapping_sub(1),
        }
    }
}

/// Global path latch word ($CF33), distinct from encounter signals $D77D.
/// The reset service clears it through the authored one-based mask lookup.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PathLatches {
    pub raised: u16,
}

/// Pending supplementary sound-bank selection ($1BBB). Source $03:E2D9
/// consumes it during sound upload and clears it after a nonempty upload.
/// Publishing the request does not itself play a cue or consume the byte.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SoundBankRequest {
    pub selection: u8,
}
