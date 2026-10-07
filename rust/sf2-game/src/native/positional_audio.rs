//! Strategy-time positional-loop selection ($7F:365A..3763).
//!
//! Candidates are sampled immediately after their strategy, before retirement.
//! The scene resets the nearest-distance accumulator at epoch start and
//! publishes it at the source's later audio boundary ($03:815A). Rescanning
//! surviving actors after cleanup loses both this ordering and retired sounds.

use super::{Angle, Object, ObjectId, SpatialDistance, SpatialSound, StereoPosition, Vector3};

const CLOSE_LIMIT: u16 = 400;
const NEAR_LIMIT: u16 = 1_000;
const FAR_LIMIT: u16 = 2_000;
const FRONT_CENTER_END: u8 = 16;
const RIGHT_END: u8 = 112;
const REAR_CENTER_END: u8 = 144;
const LEFT_END: u8 = 240;

/// Fixed primary view marker, not whichever player a path currently selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopListener {
    pub position: Vector3,
    /// Source marker byte 15, independently owned from the actor's yaw byte.
    pub bearing: Angle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopSelection {
    pub sound: SpatialSound,
    pub distance: u16,
    /// Original control OR distance/stereo bits. Preserve overlapping bits
    /// already authored in the control; do not rebuild it from a masked ID.
    pub control: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingLoopListener;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PositionalAudio {
    nearest: Option<LoopSelection>,
    published: Option<LoopSelection>,
    /// Output gate is independent from the retained identity/distance. Entry
    /// reset silences this byte alone; a frozen publication preserves silence.
    published_control: u8,
}

impl PositionalAudio {
    /// Source $03:C193 resets distance only. Prior published output remains
    /// audible until the publication boundary, even if no new actor qualifies.
    pub fn begin_epoch(&mut self) {
        self.nearest = None;
    }

    pub fn pending(&self) -> Option<LoopSelection> {
        self.nearest
    }

    pub fn published(&self) -> Option<LoopSelection> {
        if self.published_control == 0 {
            None
        } else {
            self.published
        }
    }

    pub fn retained_selection(&self) -> Option<LoopSelection> {
        self.published
    }

    pub fn published_control(&self) -> u8 {
        self.published_control
    }

    /// Source $06:843A preserves selected sound, owner and distance, as well
    /// as the independent nearest-actor accumulator for the next publication.
    pub fn silence_published(&mut self) {
        self.published_control = 0;
    }

    pub fn observe(
        &mut self,
        owner: ObjectId,
        actor: &Object,
        suppressed: bool,
        listener: Option<LoopListener>,
    ) -> Result<(), MissingLoopListener> {
        // Global source 1CD3 bit 01 and the actor's zero control both bypass
        // marker reads. Deferred removal is deliberately not a filter here.
        if suppressed {
            return Ok(());
        }
        let Some(sound) = actor.extension.spatial_loop else {
            return Ok(());
        };
        let listener = listener.ok_or(MissingLoopListener)?;
        let x = actor.base.position.x.wrapping_sub(listener.position.x);
        let z = actor.base.position.z.wrapping_sub(listener.position.z);
        let distance = xz_distance(x, z);
        if self
            .nearest
            .is_some_and(|previous| distance >= previous.distance)
        {
            return Ok(());
        }
        let band = distance_band(distance);
        let bearing = (sf_core::aim_angle::sf2_atan16(x, z) >> 8) as u8;
        let position = stereo_position(bearing.wrapping_sub(listener.bearing.units()));
        self.nearest = Some(LoopSelection {
            sound: SpatialSound {
                source: owner,
                sound,
                distance: band,
                position,
            },
            distance,
            control: sound.authored_control() | distance_bits(band) | stereo_bits(position),
        });
        Ok(())
    }

    /// Source 1B84 bit 01 freezes output separately from strategy pause.
    /// An unfrozen epoch with no candidate silences the positional channel.
    pub fn publish(&mut self, frozen: bool) {
        if !frozen {
            self.published = self.nearest;
            self.published_control = self.nearest.map_or(0, |selection| selection.control);
        }
    }
}

fn xz_distance(x: i16, z: i16) -> u16 {
    // ADC/ROR retains the carry: two -32768 deltas produce 32768, not zero.
    ((u32::from(x.unsigned_abs()) + u32::from(z.unsigned_abs())) / 2) as u16
}

fn distance_band(distance: u16) -> SpatialDistance {
    // The original signed-result CMP branches are equivalent over the
    // distance routine's complete [0, 32768] output domain.
    match distance {
        value if value < CLOSE_LIMIT => SpatialDistance::Close,
        value if value < NEAR_LIMIT => SpatialDistance::Near,
        value if value < FAR_LIMIT => SpatialDistance::Far,
        _ => SpatialDistance::Distant,
    }
}

fn stereo_position(angle: u8) -> StereoPosition {
    match angle {
        FRONT_CENTER_END..RIGHT_END => StereoPosition::Right,
        REAR_CENTER_END..LEFT_END => StereoPosition::Left,
        _ => StereoPosition::Center,
    }
}

fn distance_bits(distance: SpatialDistance) -> u8 {
    match distance {
        SpatialDistance::Close => 0,
        SpatialDistance::Near => 0x10,
        SpatialDistance::Far => 0x20,
        SpatialDistance::Distant => 0x30,
    }
}

fn stereo_bits(position: StereoPosition) -> u8 {
    match position {
        StereoPosition::Left => 0x40,
        StereoPosition::Center => 0x80,
        StereoPosition::Right => 0xC0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ObjectStore, ShapeId, SpatialLoop};

    fn listener() -> LoopListener {
        LoopListener {
            position: Vector3::default(),
            bearing: Angle::ZERO,
        }
    }

    #[test]
    fn silencing_retains_identity_and_pending_candidate_across_frozen_publication() {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Effect,
            ))
            .unwrap();
        for control in 1..=u8::MAX {
            let mut audio = PositionalAudio::default();
            let actor = objects.get_mut(owner).unwrap();
            actor.extension.spatial_loop = SpatialLoop::from_authored_control(control);
            actor.base.position.z = 100;
            audio
                .observe(owner, actor, false, Some(listener()))
                .unwrap();
            audio.publish(false);
            let selected = audio.published().unwrap();
            audio.begin_epoch();
            actor.base.position.z = 4000;
            audio
                .observe(owner, actor, false, Some(listener()))
                .unwrap();
            let pending = audio.pending().unwrap();
            audio.silence_published();
            assert_eq!(audio.published(), None);
            assert_eq!(audio.published_control(), 0);
            assert_eq!(audio.retained_selection(), Some(selected));
            assert_eq!(audio.pending(), Some(pending));
            audio.publish(true);
            assert_eq!(audio.published(), None);
            assert_eq!(audio.retained_selection(), Some(selected));
            audio.publish(false);
            assert_eq!(audio.published(), Some(pending));
            assert_eq!(audio.published_control(), pending.control);
            audio.begin_epoch();
            audio.silence_published();
            audio.publish(false);
            assert_eq!(audio.retained_selection(), None);
            assert_eq!(audio.published_control(), 0);
        }
    }

    #[test]
    fn arithmetic_preserves_full_word_delta_carry_and_all_distance_boundaries() {
        for x in [
            i16::MIN,
            -32767,
            -4000,
            -2000,
            -800,
            -1,
            0,
            1,
            799,
            800,
            1999,
            2000,
            3999,
            4000,
            i16::MAX,
        ] {
            for z in [i16::MIN, -32767, -1, 0, 1, i16::MAX] {
                assert_eq!(
                    xz_distance(x, z),
                    ((i32::from(x).abs() + i32::from(z).abs()) >> 1) as u16
                );
            }
        }
        assert_eq!(xz_distance(i16::MIN, i16::MIN), 32768);
        for value in 0..=32768u16 {
            let expected = if (value.wrapping_sub(400) as i16) < 0 {
                SpatialDistance::Close
            } else if (value.wrapping_sub(1000) as i16) < 0 {
                SpatialDistance::Near
            } else if (value.wrapping_sub(2000) as i16) < 0 {
                SpatialDistance::Far
            } else {
                SpatialDistance::Distant
            };
            assert_eq!(distance_band(value), expected);
        }
        for angle in 0..=u8::MAX {
            let expected = if angle < 16 || angle >= 240 || (112..144).contains(&angle) {
                StereoPosition::Center
            } else if angle < 112 {
                StereoPosition::Right
            } else {
                StereoPosition::Left
            };
            assert_eq!(stereo_position(angle), expected);
        }
    }

    #[test]
    fn equal_distance_keeps_first_strategy_snapshot_even_after_actor_changes_or_retirement() {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
        actor.extension.spatial_loop = Some(SpatialLoop::CapitalEngine);
        actor.base.position.z = 100;
        actor.base.flags.remove_after_tick = true;
        let first = objects.allocate(actor.clone()).unwrap();
        let second = objects.allocate(actor).unwrap();
        let mut audio = PositionalAudio::default();
        audio.begin_epoch();
        audio
            .observe(first, objects.get(first).unwrap(), false, Some(listener()))
            .unwrap();
        let saved = audio.pending().unwrap();
        assert_eq!(saved.distance, 50);
        assert_eq!(saved.control, 0x8B);
        audio
            .observe(
                second,
                objects.get(second).unwrap(),
                false,
                Some(listener()),
            )
            .unwrap();
        assert_eq!(audio.pending(), Some(saved));
        objects.get_mut(first).unwrap().base.position.z = 20000;
        objects.remove(first).unwrap();
        audio.publish(false);
        assert_eq!(audio.published(), Some(saved));
        audio.begin_epoch();
        assert_eq!(audio.published(), Some(saved));
        assert_eq!(audio.pending(), None);
        audio.publish(true);
        assert_eq!(audio.published(), Some(saved));
        audio.publish(false);
        assert_eq!(audio.published(), None);
    }

    #[test]
    fn suppression_and_silent_actors_skip_marker_reads_and_encoded_controls_keep_their_bits() {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Effect,
            ))
            .unwrap();
        let mut audio = PositionalAudio::default();
        assert_eq!(
            audio.observe(owner, objects.get(owner).unwrap(), false, None),
            Ok(())
        );
        for control in 1..=u8::MAX {
            let actor = objects.get_mut(owner).unwrap();
            actor.extension.spatial_loop = SpatialLoop::from_authored_control(control);
            actor.base.position.z = 2000;
            audio.begin_epoch();
            assert_eq!(audio.observe(owner, actor, true, None), Ok(()));
            assert_eq!(audio.pending(), None);
            assert_eq!(
                audio.observe(owner, actor, false, None),
                Err(MissingLoopListener)
            );
            for (bearing, expected_position, expected_bits) in [
                (0, StereoPosition::Center, 0x80),
                (64, StereoPosition::Left, 0x40),
                (192, StereoPosition::Right, 0xC0),
            ] {
                audio.begin_epoch();
                let listener = LoopListener {
                    bearing: Angle::from_units(bearing),
                    ..listener()
                };
                audio.observe(owner, actor, false, Some(listener)).unwrap();
                let actual = audio.pending().unwrap();
                assert_eq!(actual.sound.position, expected_position);
                assert_eq!(actual.control, control | 0x20 | expected_bits);
                assert_eq!(actual.sound.sound.authored_control(), control);
            }
        }
    }
}
