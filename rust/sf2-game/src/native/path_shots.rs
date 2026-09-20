//! Player-owned live shot count, shared by the launch gate and linked paths.
//! Source: `$06:A9E6..AA19`, `$7F:BDC9..BDF7`, auxiliary byte 6C03.

use super::path_fields::{ByteField, ByteOperand};
use super::{ObjectId, ObjectStore};

const SHOT_LIMIT: u8 = 8;

/// Scene-owned projectile flight override (D7D8). Scene initialization clears
/// it; the large encounter controller at path 9492 publishes one. The rapid
/// shots use exactly one to retain their extended-flight route; other codes
/// fall back to the complete surface-mode byte. Keep the source byte intact.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectileFlightOverride {
    pub code: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlightOverrideCommand {
    CopyTo(ByteField),
    Assign(ByteOperand),
}

impl ProjectileFlightOverride {
    pub fn apply(&mut self, actor: &mut super::Object, command: FlightOverrideCommand) {
        match command {
            FlightOverrideCommand::CopyTo(field) => field.write(actor, self.code),
            FlightOverrideCommand::Assign(value) => self.code = value.read(actor),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveShots(u8);

impl ActiveShots {
    pub const fn from_count(count: u8) -> Self {
        Self(count)
    }

    pub const fn count(self) -> u8 {
        self.0
    }

    /// Source CMP/BPL tests the wrapped subtraction's sign, not carry.
    pub const fn admits_launch(self) -> bool {
        (self.0.wrapping_sub(SHOT_LIMIT) as i8) < 0
    }

    pub fn apply(&mut self, command: ShotCountCommand) {
        self.0 = match command {
            ShotCountCommand::Increment => self.0.wrapping_add(1),
            ShotCountCommand::Decrement => self.0.saturating_sub(1),
        };
    }

    /// Player initialization (`$06:DB4D`) and the gated reset at `$06:9CBC`
    /// clear the entire byte. Their enclosing player services own the gates.
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotCountCommand {
    Increment,
    Decrement,
}

pub struct LinkedShotCount<'a> {
    pub owner: ObjectId,
    pub state: &'a mut ActiveShots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotCountError {
    MissingActor(ObjectId),
    MissingAttachment,
    WrongLinkedOwner {
        expected: ObjectId,
        supplied: ObjectId,
    },
}

/// Resolve the projectile's attachment, never the fresh selected player.
/// Validate every identity before modifying the borrowed player record.
pub fn apply_linked(
    objects: &ObjectStore,
    projectile: ObjectId,
    linked: &mut LinkedShotCount<'_>,
    command: ShotCountCommand,
) -> Result<(), ShotCountError> {
    let owner = objects
        .get(projectile)
        .ok_or(ShotCountError::MissingActor(projectile))?
        .base
        .attachment
        .ok_or(ShotCountError::MissingAttachment)?;
    objects
        .get(owner)
        .ok_or(ShotCountError::MissingActor(owner))?;
    if linked.owner != owner {
        return Err(ShotCountError::WrongLinkedOwner {
            expected: owner,
            supplied: linked.owner,
        });
    }
    linked.state.apply(command);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ShapeId};

    #[test]
    fn all_counts_preserve_wrap_saturation_signed_gate_and_reset() {
        for count in 0..=u8::MAX {
            let initial = ActiveShots::from_count(count);
            assert_eq!(initial.count(), count);
            assert_eq!(initial.admits_launch(), count < 8 || count >= 136);
            let mut state = initial;
            state.apply(ShotCountCommand::Increment);
            assert_eq!(state.count(), count.wrapping_add(1));
            state = initial;
            state.apply(ShotCountCommand::Decrement);
            assert_eq!(state.count(), if count == 0 { 0 } else { count - 1 });
            state = initial;
            state.clear();
            assert_eq!(state, ActiveShots::default());
        }
    }

    #[test]
    fn linked_identity_errors_are_atomic_and_never_change_objects() {
        let mut objects = ObjectStore::new();
        let actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        let projectile = objects.allocate(actor.clone()).unwrap();
        let player = objects.allocate(actor.clone()).unwrap();
        let unrelated = objects.allocate(actor).unwrap();
        for case in 0..5 {
            objects.get_mut(projectile).unwrap().base.attachment =
                if case == 0 { None } else { Some(player) };
            let mut objects = objects.clone();
            if case == 2 {
                objects.remove(player).unwrap();
                objects.get_mut(projectile).unwrap().base.attachment = Some(player);
            }
            if case == 3 {
                objects.remove(projectile).unwrap();
            }
            let before = objects.clone();
            let mut count = ActiveShots::from_count(255);
            let result = apply_linked(
                &objects,
                projectile,
                &mut LinkedShotCount {
                    owner: if case == 1 { unrelated } else { player },
                    state: &mut count,
                },
                ShotCountCommand::Increment,
            );
            let expected = match case {
                0 => Err(ShotCountError::MissingAttachment),
                1 => Err(ShotCountError::WrongLinkedOwner {
                    expected: player,
                    supplied: unrelated,
                }),
                2 => Err(ShotCountError::MissingActor(player)),
                3 => Err(ShotCountError::MissingActor(projectile)),
                _ => Ok(()),
            };
            assert_eq!(result, expected);
            assert_eq!(count.count(), if case == 4 { 0 } else { 255 });
            assert_eq!(objects, before);
        }
    }
}
