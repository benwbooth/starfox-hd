//! Shared authored countdown used by the invisible service path. The world
//! owns one record; actor-local copies are snapshots, not independent clocks.

use super::path_fields::{ByteField, ByteOperand};
use super::Object;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PathCountdown {
    pub remaining: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountdownCommand {
    CopyTo(ByteField),
    Assign(ByteOperand),
    Increment,
    Decrement,
}

impl PathCountdown {
    pub fn apply(&mut self, actor: &mut Object, command: CountdownCommand) {
        match command {
            CountdownCommand::CopyTo(field) => field.write(actor, self.remaining),
            CountdownCommand::Assign(value) => self.remaining = value.read(actor),
            CountdownCommand::Increment => self.remaining = self.remaining.wrapping_add(1),
            // Only the service path's branch guards zero. An ordinary
            // authored decrement still wraps exactly like any byte write.
            CountdownCommand::Decrement => self.remaining = self.remaining.wrapping_sub(1),
        }
    }
}

/// Path-only shared bytes: authored paths store, count and hand values
/// through them, and no 65816 service reads them. Each cell is named by
/// its source byte because no wider meaning is shared by its users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScratchCell {
    /// D766.
    D766,
    /// D77B.
    D77B,
    /// D77C.
    D77C,
    /// D79C.
    D79C,
    /// D7D2: an attacker quota in some families, a hand-off in others.
    D7D2,
    /// D7D6.
    D7D6,
    /// D7EB.
    D7EB,
    /// D7E8..D7EA: health retained per node presentation variant (1E09)
    /// between a rival's encounters; campaign start clears them (`$0D:F627`).
    RetainedRivalHealth(u8),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PathScratchBytes {
    cells: [PathCountdown; 10],
}

impl ScratchCell {
    fn slot(self) -> usize {
        match self {
            Self::D766 => 0,
            Self::D77C => 1,
            Self::D79C => 2,
            Self::D7D2 => 3,
            Self::D7D6 => 4,
            Self::D77B => 5,
            Self::D7EB => 6,
            Self::RetainedRivalHealth(variant) => 7 + usize::from(variant.min(2)),
        }
    }
}

impl PathScratchBytes {
    pub fn apply(&mut self, cell: ScratchCell, actor: &mut Object, command: CountdownCommand) {
        self.cells[cell.slot()].apply(actor, command);
    }

    pub fn get(&self, cell: ScratchCell) -> u8 {
        self.cells[cell.slot()].remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    #[test]
    fn all_countdown_operations_preserve_width_live_operands_and_other_actor_state() {
        for value in 0..=u8::MAX {
            let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
            actor.base.wait_timer = 91;
            actor.extension.surface_contact.group = value ^ 0xA5;
            for (command, remaining, part) in [
                (CountdownCommand::CopyTo(ByteField::Part), value, value),
                (
                    CountdownCommand::Assign(ByteOperand::Actor(ByteField::Part)),
                    value ^ 0xA5,
                    value ^ 0xA5,
                ),
                (
                    CountdownCommand::Assign(ByteOperand::Literal(255)),
                    255,
                    value ^ 0xA5,
                ),
                (
                    CountdownCommand::Increment,
                    value.wrapping_add(1),
                    value ^ 0xA5,
                ),
                (
                    CountdownCommand::Decrement,
                    value.wrapping_sub(1),
                    value ^ 0xA5,
                ),
            ] {
                let mut actual = actor.clone();
                let mut expected = actor.clone();
                expected.extension.surface_contact.group = part;
                let mut countdown = PathCountdown { remaining: value };
                countdown.apply(&mut actual, command);
                assert_eq!(actual, expected);
                assert_eq!(countdown.remaining, remaining);
            }
        }
    }
}
