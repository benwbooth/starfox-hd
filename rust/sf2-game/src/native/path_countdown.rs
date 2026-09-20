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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    #[test]
    fn all_countdown_operations_preserve_width_live_operands_and_other_actor_state() {
        for value in 0..=u8::MAX {
            let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
            actor.base.wait_timer = 91;
            actor.extension.path_state.part = value ^ 0xA5;
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
                expected.extension.path_state.part = part;
                let mut countdown = PathCountdown { remaining: value };
                countdown.apply(&mut actual, command);
                assert_eq!(actual, expected);
                assert_eq!(countdown.remaining, remaining);
            }
        }
    }
}
