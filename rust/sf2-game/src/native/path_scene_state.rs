//! Scene-owned coordination shared by independently scheduled authored paths.

use super::path_fields::{ByteField, ByteOperand};
use super::Object;

/// World-space point published by encounter paths for the camera target.
/// Source path producer $7F:C2B3; camera consumer $07:A14A.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterCameraFocus {
    pub position: super::Vector3,
}

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
    /// Encounter-specific phase ($D79A), including full-byte sentinels.
    /// Fighter emitters clear it; fighters poll it to enter their abort path.
    pub phase: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationField {
    Progress,
    SecondaryProgress,
    CompletedParts,
    ActiveMessages,
    Handshake,
    RetiredActors,
    Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationCommand {
    CopyTo(ByteField),
    Assign(ByteOperand),
    Increment,
    Decrement,
}

/// Script-owned boss bar values, before the separate UI publication clamp.
/// Source D777 is a bank-three text reference, NOT a scheduled callback.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterHealthDisplay {
    pub current: u8,
    pub maximum: u8,
    pub label: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthDisplayField {
    Current,
    Maximum,
}

impl EncounterHealthDisplay {
    pub fn apply(
        &mut self,
        actor: &mut Object,
        field: HealthDisplayField,
        command: CoordinationCommand,
    ) {
        let value = match field {
            HealthDisplayField::Current => &mut self.current,
            HealthDisplayField::Maximum => &mut self.maximum,
        };
        match command {
            CoordinationCommand::CopyTo(destination) => destination.write(actor, *value),
            CoordinationCommand::Assign(source) => *value = source.read(actor),
            CoordinationCommand::Increment => *value = value.wrapping_add(1),
            CoordinationCommand::Decrement => *value = value.wrapping_sub(1),
        }
    }
}

/// One decoded connection between the four authored Gunner waypoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GunnerRoute {
    pub origin: (i16, i16),
    pub destination: (i16, i16),
    pub destination_index: u8,
    pub heading: u8,
    pub entry_heading: Option<u8>,
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
            CoordinationField::Phase => &mut self.phase,
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
