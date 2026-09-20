//! Scene-owned coordination shared by independently scheduled authored paths.

use super::path_fields::{ByteField, ByteOperand};
use super::Object;

/// Live campaign-node flags ($D7F6). Node loading replaces the low byte,
/// while authored paths and campaign writeback read and replace the full word.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveNodeFlags {
    pub bits: u16,
}

/// Live per-objective completion bits, distinct from the active-node flags.
/// Space-node entry and writeback copy the entire campaign word; authored
/// query/record helpers read and replace it through their actor's scratch word.
/// Sources: $04:B2A8, $04:B2E9, $44:87D3 and $44:87E5.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveCompletion {
    pub bits: u16,
}

/// Live objective accounting shared by encounter paths and campaign writeback.
/// Planet entry initializes both low bytes to the same count. Space entry
/// retains the packed campaign byte in `node_record`, and publishes the sum
/// of its nibbles into the low byte of `remaining_word`. Path byte operations
/// preserve the companion byte; campaign writeback reads the complete word.
/// Sources: $04:B218, $04:B283, $04:B2B1; path counters $D7F4/$D7A1.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterObjectiveCounts {
    pub remaining_word: u16,
    pub node_record: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveCountField {
    Remaining,
    NodeRecord,
}

impl EncounterObjectiveCounts {
    pub fn apply(
        &mut self,
        actor: &mut Object,
        field: ObjectiveCountField,
        command: CoordinationCommand,
    ) {
        let previous = match field {
            ObjectiveCountField::Remaining => self.remaining_word as u8,
            ObjectiveCountField::NodeRecord => self.node_record,
        };
        let value = match command {
            CoordinationCommand::CopyTo(destination) => {
                destination.write(actor, previous);
                return;
            }
            CoordinationCommand::Assign(source) => source.read(actor),
            CoordinationCommand::Increment => previous.wrapping_add(1),
            CoordinationCommand::Decrement => previous.wrapping_sub(1),
        };
        match field {
            ObjectiveCountField::Remaining => {
                const COMPANION_BYTE: u16 = 0xFF00;
                self.remaining_word = (self.remaining_word & COMPANION_BYTE) | u16::from(value);
            }
            ObjectiveCountField::NodeRecord => self.node_record = value,
        }
    }
}

/// Encounter exit publication ($1D74, $1D88/$1D8C, $1D8E). The player
/// transition consumer copies this horizontal anchor into its actor, with
/// a separately chosen height. Publishing does not perform that transition.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterHandoff {
    pub player_flags: u8,
    pub x: i16,
    pub z: i16,
    /// The producer replaces only the yaw byte. The scene consumer at
    /// $07:9E4B reads the whole word, so its companion byte is retained.
    pub heading_word: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffCommand {
    Request,
    StoreX(super::path_fields::WordOperand),
    StoreZ(super::path_fields::WordOperand),
    StoreHeading(ByteOperand),
}

impl EncounterHandoff {
    pub fn apply(&mut self, actor: &Object, command: HandoffCommand) {
        const HANDOFF_REQUEST: u8 = 0x40;
        const HEADING_COMPANION: u16 = 0xFF00;
        match command {
            HandoffCommand::Request => self.player_flags |= HANDOFF_REQUEST,
            HandoffCommand::StoreX(source) => self.x = source.read(actor) as i16,
            HandoffCommand::StoreZ(source) => self.z = source.read(actor) as i16,
            HandoffCommand::StoreHeading(source) => {
                self.heading_word =
                    (self.heading_word & HEADING_COMPANION) | u16::from(source.read(actor));
            }
        }
    }
}

/// World-space point published by encounter paths for the camera target.
/// Source path producer $7F:C2B3; camera consumer $07:A14A.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EncounterCameraFocus {
    pub position: super::Vector3,
}

/// Actor selected by paths for camera-follow orientation ($1DFF), distinct
/// from the encounter's world-space focus point and player selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraTrackingTarget {
    pub actor: Option<super::ObjectId>,
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
    /// Transition-script publication ($D7D5): cleared on encounter/player
    /// initialization, published by $44:B92A and polled by $44:872C.
    /// Keep the whole byte; waiting tests nonzero, not equality with one.
    pub transition_ready: u8,
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
    TransitionReady,
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
            CoordinationField::TransitionReady => &mut self.transition_ready,
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
