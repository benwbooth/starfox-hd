//! Nearby obstacle warning scan (`$06:A647..A84D`). Candidate range is
//! player-relative, but projection uses the fixed primary view marker. The
//! two best candidates are ranked by signed lateral plus forward offset,
//! interpreted as an unsigned word, not by Euclidean distance.

use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::{Angle, AudioState, ObjectId, ObjectStore, ShapeId, SoundEvent, Vector3};

const MOVEMENT_CLASS_MASK: u8 = 0xF0;
const FLIGHT_CLASS: u8 = 0x10;
const TRANSITION_PAIR_MASK: u8 = 0xFE;
const GUARDED_TRANSITION: u8 = 2;
const HEIGHT_LIMIT: u16 = 300;
const RANGE_LIMIT: u16 = 500;
const FORWARD_LIMIT: i16 = 150;
const INITIAL_SCORE: u16 = 32_767;
const CENTER_WIDTH: u16 = 50;
const OUTER_WIDTH: u16 = 130;
const LARGE_SHAPE_SIZE: u16 = 256;
const RIGHT: u8 = 1;
const CENTER: u8 = 2;
const LEFT: u8 = 4;
const WARNING_CUE: u8 = 171;
const RIGHT_PARAMETER: u8 = 0x20;
const LEFT_PARAMETER: u8 = 0x10;

/// Live entry conditions, sampled from shared mode and player controls.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WarningControl {
    /// Shared game mode bit 02 ($1B84).
    pub globally_disabled: bool,
    /// Player movement class ($6AA0); low variant bits do not affect entry.
    pub movement_mode: u8,
    /// Player auxiliary bit 20 ($6B77).
    pub inhibited: bool,
    /// Player transition mode ($6BFF), tested with its low bit removed.
    pub transition_mode: u8,
    /// Player auxiliary bit 40 ($6B77) admits the guarded transition pair.
    pub transition_ready: bool,
}

impl WarningControl {
    pub fn enabled(self) -> bool {
        !self.globally_disabled
            && self.movement_mode & MOVEMENT_CLASS_MASK == FLIGHT_CLASS
            && !self.inhibited
            && (self.transition_mode & TRANSITION_PAIR_MASK != GUARDED_TRANSITION
                || self.transition_ready)
    }
}

/// `$06:A56D` uses marker $033F and its bearing byte, even for player two.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WarningView {
    pub position: Vector3,
    pub bearing: Angle,
}

impl WarningView {
    pub fn project(self, position: Vector3) -> (i16, i16) {
        sf_core::snes_trig::rotate_16xz(
            self.bearing.units(),
            position.x.wrapping_sub(self.position.x),
            position.z.wrapping_sub(self.position.z),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningError {
    MissingPlayer(ObjectId),
    UnknownShape { object: ObjectId, shape: ShapeId },
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    object: ObjectId,
    lateral: i16,
    score: u16,
}

fn direction(lateral: i16, nearest_lateral: i16) -> u8 {
    match lateral.unsigned_abs() {
        0..CENTER_WIDTH => CENTER,
        CENTER_WIDTH..OUTER_WIDTH => {
            // The SECOND candidate also uses the NEAREST one's sign at
            // $06:A7BD. Do not replace this with its own lateral sign.
            if nearest_lateral < 0 {
                LEFT
            } else {
                RIGHT
            }
        }
        _ => 0,
    }
}

/// Scan active-list order, update one-shot warning latches and enqueue the
/// source cue. Noncandidates retain their latch. A latched in-range object
/// is ignored; leaving height/range/forward bounds rearms it. Visibility,
/// collision, strategy suspension and health are not additional filters.
/// Missing native shape metadata fails before changing latches or audio.
pub fn update(
    objects: &mut ObjectStore,
    player: ObjectId,
    primary_player: Option<ObjectId>,
    control: WarningControl,
    view: WarningView,
    audio: &mut AudioState,
) -> Result<(), WarningError> {
    if !control.enabled() {
        return Ok(());
    }
    let player_position = objects
        .get(player)
        .ok_or(WarningError::MissingPlayer(player))?
        .base
        .position;
    let mut rearm = Vec::new();
    let mut best: [Option<Candidate>; 2] = [None, None];
    for &id in objects.active_ids() {
        let actor = objects.get(id).expect("active warning candidate");
        if !actor.base.flags.proximity_warning_source {
            continue;
        }
        let position = actor.base.position;
        let height = player_position.y.wrapping_sub(position.y).unsigned_abs();
        let range = sf_core::aim_angle::sf2_xz_angle_distance(
            player_position.x.wrapping_sub(position.x),
            player_position.z.wrapping_sub(position.z),
        ) as u16;
        let (lateral, forward) = view.project(position);
        if height >= HEIGHT_LIMIT || range >= RANGE_LIMIT || !(0..FORWARD_LIMIT).contains(&forward)
        {
            rearm.push(id);
            continue;
        }
        if actor.base.flags.proximity_warning_latched {
            continue;
        }
        let score = forward.wrapping_add(lateral) as u16;
        if score >= best[1].map_or(INITIAL_SCORE, |candidate| candidate.score) {
            continue;
        }
        let candidate = Candidate {
            object: id,
            lateral,
            score,
        };
        if score < best[0].map_or(INITIAL_SCORE, |candidate| candidate.score) {
            best[1] = best[0];
            best[0] = Some(candidate);
        } else {
            best[1] = Some(candidate);
        }
    }
    let mut latch = Vec::new();
    let mut sides = 0;
    if let Some(nearest) = best[0] {
        for candidate in best.into_iter().flatten() {
            let side = direction(candidate.lateral, nearest.lateral);
            if side == 0 {
                continue;
            }
            sides |= side;
            let actor = objects
                .get(candidate.object)
                .expect("live warning selection");
            let shape = actor
                .base
                .shape
                .catalog_entry()
                .ok_or(WarningError::UnknownShape {
                    object: candidate.object,
                    shape: actor.base.shape,
                })?;
            // $06:A837 tests the high byte of ShapeHdr.size, not its bounds.
            if shape.size >= LARGE_SHAPE_SIZE {
                sides = CENTER;
            }
            latch.push(candidate.object);
        }
    }
    for id in rearm {
        objects
            .get_mut(id)
            .expect("live warning rearm")
            .base
            .flags
            .proximity_warning_latched = false;
    }
    for id in latch {
        objects
            .get_mut(id)
            .expect("live warning latch")
            .base
            .flags
            .proximity_warning_latched = true;
    }
    if sides != 0 {
        let parameter = if sides & CENTER != 0 {
            0
        } else if sides & LEFT != 0 {
            LEFT_PARAMETER
        } else {
            RIGHT_PARAMETER
        };
        audio.queue(SoundEvent::Authored(AuthoredCue::new(
            WARNING_CUE,
            parameter,
            if Some(player) == primary_player {
                PlayerTarget::Primary
            } else {
                PlayerTarget::Secondary
            },
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind};

    const ENABLED: WarningControl = WarningControl {
        globally_disabled: false,
        movement_mode: 16,
        inhibited: false,
        transition_mode: 0,
        transition_ready: false,
    };

    fn shape(large: bool) -> ShapeId {
        let index = sf2_data::shape_data::SHAPE_DATA
            .iter()
            .position(|shape| (shape.size >= 256) == large)
            .unwrap();
        ShapeId::from_catalog_index(index as u16)
    }

    fn object(position: Vector3, large: bool) -> Object {
        let mut object = Object::new(ObjectKind::Enemy, shape(large), Behavior::FollowPath);
        object.base.position = position;
        object.base.flags.proximity_warning_source = true;
        object
    }

    fn cues(audio: &mut AudioState) -> Vec<SoundEvent> {
        audio.take_events().into_iter().flatten().collect()
    }

    fn multiply(value: i16, factor: i8) -> i16 {
        let magnitude = value.unsigned_abs();
        let factor_magnitude = u16::from(factor.unsigned_abs().wrapping_mul(2));
        let high_product = (magnitude >> 8) * factor_magnitude;
        let low_product = ((magnitude & 255) * factor_magnitude) >> 8;
        let result = high_product.wrapping_add(low_product);
        if (value < 0) != (factor < 0) {
            result.wrapping_neg() as i16
        } else {
            result as i16
        }
    }

    fn projection(position: Vector3, view: WarningView) -> (i16, i16) {
        let dx = position.x.wrapping_sub(view.position.x);
        let dz = position.z.wrapping_sub(view.position.z);
        let sin = sf_core::snes_trig::SINTAB[usize::from(view.bearing.units())];
        let cos = sf_core::snes_trig::COSTAB[usize::from(view.bearing.units())];
        (
            multiply(dx, cos).wrapping_sub(multiply(dz, sin)),
            multiply(dx, sin).wrapping_add(multiply(dz, cos)),
        )
    }

    fn distance(a: Vector3, b: Vector3) -> u16 {
        let x = (a.x.wrapping_sub(b.x).unsigned_abs() as i16) >> 1;
        let z = (a.z.wrapping_sub(b.z).unsigned_abs() as i16) >> 1;
        let larger = if z.wrapping_sub(x) < 0 { x } else { z };
        let sum = x.wrapping_add(z).wrapping_mul(2).wrapping_add(larger);
        ((sum >> 1).wrapping_add(sum) >> 2) as u16
    }

    /// Independent reference uses a stable sorted candidate list instead of
    /// the native two-slot insertion. It is arithmetic over source rules,
    /// not execution of the original program or recorded gameplay.
    fn expected(objects: &mut ObjectStore, player: ObjectId, view: WarningView) -> Option<u8> {
        let origin = objects.get(player).unwrap().base.position;
        let mut candidates = Vec::new();
        for id in objects.active_ids().to_vec() {
            let actor = objects.get_mut(id).unwrap();
            if !actor.base.flags.proximity_warning_source {
                continue;
            }
            let (x, z) = projection(actor.base.position, view);
            if origin.y.wrapping_sub(actor.base.position.y).unsigned_abs() >= 300
                || distance(origin, actor.base.position) >= 500
                || z < 0
                || z >= 150
            {
                actor.base.flags.proximity_warning_latched = false;
            } else if !actor.base.flags.proximity_warning_latched {
                let score = x.wrapping_add(z) as u16;
                if score < 32767 {
                    candidates.push((score, x, id));
                }
            }
        }
        candidates.sort_by_key(|&(score, _, _)| score);
        let nearest = candidates.first().map_or(0, |candidate| candidate.1);
        let mut sides = 0;
        for &(_, lateral, id) in candidates.iter().take(2) {
            if lateral.unsigned_abs() >= 130 {
                continue;
            }
            sides |= if lateral.unsigned_abs() < 50 {
                2
            } else if nearest < 0 {
                4
            } else {
                1
            };
            let actor = objects.get_mut(id).unwrap();
            actor.base.flags.proximity_warning_latched = true;
            if actor.base.shape.catalog_entry().unwrap().size > 255 {
                sides = 2;
            }
        }
        if sides == 0 {
            None
        } else if sides & 2 != 0 {
            Some(0)
        } else if sides & 4 != 0 {
            Some(16)
        } else {
            Some(32)
        }
    }

    #[test]
    fn entry_conditions_preserve_all_mode_variants_and_do_not_consume_disabled_state() {
        for mode in 0..=255 {
            for transition in 0..=255 {
                for flags in 0..8 {
                    let control = WarningControl {
                        globally_disabled: flags & 1 != 0,
                        inhibited: flags & 2 != 0,
                        transition_ready: flags & 4 != 0,
                        movement_mode: mode,
                        transition_mode: transition,
                    };
                    let enabled = (16..32).contains(&mode)
                        && flags & 3 == 0
                        && (!matches!(transition, 2 | 3) || flags & 4 != 0);
                    assert_eq!(control.enabled(), enabled);
                }
            }
        }
        let mut objects = ObjectStore::new();
        let missing = objects.allocate(object(Vector3::default(), false)).unwrap();
        objects.remove(missing).unwrap();
        let mut audio = AudioState::default();
        assert_eq!(
            update(
                &mut objects,
                missing,
                None,
                WarningControl::default(),
                WarningView::default(),
                &mut audio
            ),
            Ok(())
        );
        assert_eq!(
            update(
                &mut objects,
                missing,
                None,
                ENABLED,
                WarningView::default(),
                &mut audio
            ),
            Err(WarningError::MissingPlayer(missing))
        );
        assert!(cues(&mut audio).is_empty());
    }

    #[test]
    fn view_projection_matches_byte_product_truncation_for_every_bearing_and_word() {
        for angle in 0..=255 {
            let view = WarningView {
                position: Vector3 {
                    x: 32700,
                    y: -1200,
                    z: -32600,
                },
                bearing: Angle::from_units(angle),
            };
            for word in 0..=u16::MAX {
                let position = Vector3 {
                    x: word as i16,
                    y: word as i16,
                    z: word.wrapping_mul(257).wrapping_add(12345) as i16,
                };
                assert_eq!(view.project(position), projection(position, view));
            }
        }
    }

    #[test]
    fn candidate_ranking_sides_latches_and_routing_match_source_boundaries() {
        let lateral = [
            -32768, -500, -131, -130, -129, -51, -50, -49, 0, 49, 50, 51, 129, 130, 131, 500, 32767,
        ];
        for first in lateral {
            for second in lateral {
                for depth in [-1, 0, 50, 99, 149, 150, 152, 500] {
                    for configuration in 0..8 {
                        let mut objects = ObjectStore::new();
                        let player = objects
                            .allocate(Object::new(
                                ObjectKind::Player,
                                ShapeId::EMPTY,
                                Behavior::PlayerFlight,
                            ))
                            .unwrap();
                        let other = objects
                            .allocate(object(
                                Vector3 {
                                    x: second,
                                    y: 0,
                                    z: depth,
                                },
                                configuration & 1 != 0,
                            ))
                            .unwrap();
                        let owner = objects
                            .allocate(object(
                                Vector3 {
                                    x: first,
                                    y: 0,
                                    z: depth,
                                },
                                configuration & 2 != 0,
                            ))
                            .unwrap();
                        let actor = objects.get_mut(owner).unwrap();
                        actor.base.flags.visible = false;
                        actor.base.flags.collision_disabled = true;
                        actor.base.flags.strategy_suspended = true;
                        actor.base.hit_points = 0;
                        actor.base.flags.proximity_warning_latched = configuration & 4 != 0;
                        let mut reference = objects.clone();
                        let parameter = expected(&mut reference, player, WarningView::default());
                        let mut audio = AudioState::default();
                        let primary = if configuration & 4 == 0 {
                            Some(player)
                        } else {
                            Some(other)
                        };
                        update(
                            &mut objects,
                            player,
                            primary,
                            ENABLED,
                            WarningView::default(),
                            &mut audio,
                        )
                        .unwrap();
                        assert_eq!(
                            objects, reference,
                            "x={first},{second} z={depth} config={configuration}"
                        );
                        let target = if primary == Some(player) {
                            PlayerTarget::Primary
                        } else {
                            PlayerTarget::Secondary
                        };
                        let expected_cues: Vec<_> = parameter
                            .into_iter()
                            .map(|p| SoundEvent::Authored(AuthoredCue::new(171, p, target)))
                            .collect();
                        assert_eq!(cues(&mut audio), expected_cues);
                    }
                }
            }
        }
    }

    #[test]
    fn live_order_best_two_selection_uses_player_range_and_a_separate_fixed_view() {
        for seed in 0..1024_u16 {
            let mut objects = ObjectStore::new();
            let player_position = Vector3 {
                x: seed.wrapping_mul(257) as i16,
                y: seed.wrapping_mul(8191) as i16,
                z: seed.wrapping_mul(513) as i16,
            };
            let mut player_actor =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            player_actor.base.position = player_position;
            let player = objects.allocate(player_actor).unwrap();
            let view = WarningView {
                position: Vector3 {
                    x: player_position.x.wrapping_add((seed % 99) as i16),
                    y: 12345,
                    z: player_position.z.wrapping_sub((seed % 79) as i16),
                },
                bearing: Angle::from_units(seed as u8),
            };
            for index in 0..8_u16 {
                let position = Vector3 {
                    x: player_position.x.wrapping_add(
                        (seed.wrapping_mul(31).wrapping_add(index * 23) % 601) as i16 - 300,
                    ),
                    y: player_position.y.wrapping_add(
                        (seed.wrapping_mul(17).wrapping_add(index * 79) % 651) as i16 - 325,
                    ),
                    z: player_position.z.wrapping_add(
                        (seed.wrapping_mul(53).wrapping_add(index * 37) % 401) as i16 - 200,
                    ),
                };
                let mut actor = object(position, index % 3 == 0);
                actor.base.flags.proximity_warning_latched = (seed + index) % 5 == 0;
                actor.base.flags.proximity_warning_source = (seed + index) % 7 != 0;
                objects.allocate(actor).unwrap();
            }
            let mut reference = objects.clone();
            let parameter = expected(&mut reference, player, view);
            let mut audio = AudioState::default();
            let retained = SoundEvent::Authored(AuthoredCue::new(42, 3, PlayerTarget::Primary));
            audio.queue(retained);
            update(&mut objects, player, None, ENABLED, view, &mut audio).unwrap();
            assert_eq!(objects, reference, "seed={seed}");
            let expected_cues: Vec<_> = std::iter::once(retained)
                .chain(parameter.into_iter().map(|parameter| {
                    SoundEvent::Authored(AuthoredCue::new(171, parameter, PlayerTarget::Secondary))
                }))
                .collect();
            assert_eq!(cues(&mut audio), expected_cues, "seed={seed}");
        }
    }

    #[test]
    fn warning_rearms_only_after_bounds_exit_and_disabled_source_retains_latch() {
        let mut objects = ObjectStore::new();
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let owner = objects
            .allocate(object(Vector3 { x: 0, y: 0, z: 100 }, false))
            .unwrap();
        let mut audio = AudioState::default();
        for (height, source, expected_count, latched) in [
            (0, true, 1, true),
            (299, true, 0, true),
            (300, false, 0, true),
            (300, true, 0, false),
            (-299, true, 1, true),
            (-300, true, 0, false),
            (i16::MIN, true, 0, false),
            (0, true, 1, true),
        ] {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.position.y = height;
            actor.base.flags.proximity_warning_source = source;
            update(
                &mut objects,
                player,
                Some(player),
                ENABLED,
                WarningView::default(),
                &mut audio,
            )
            .unwrap();
            assert_eq!(cues(&mut audio).len(), expected_count);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .base
                    .flags
                    .proximity_warning_latched,
                latched
            );
        }
    }

    #[test]
    fn invalid_selected_shape_fails_before_rearming_other_actors_or_publishing_audio() {
        let mut objects = ObjectStore::new();
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let stale = objects
            .allocate(object(
                Vector3 {
                    x: 0,
                    y: 400,
                    z: 100,
                },
                false,
            ))
            .unwrap();
        objects
            .get_mut(stale)
            .unwrap()
            .base
            .flags
            .proximity_warning_latched = true;
        let selected = objects
            .allocate(object(Vector3 { x: 0, y: 0, z: 100 }, false))
            .unwrap();
        let unknown = ShapeId::from_catalog_index(u16::MAX);
        objects.get_mut(selected).unwrap().base.shape = unknown;
        let before = objects.clone();
        let mut audio = AudioState::default();
        assert_eq!(
            update(
                &mut objects,
                player,
                Some(player),
                ENABLED,
                WarningView::default(),
                &mut audio
            ),
            Err(WarningError::UnknownShape {
                object: selected,
                shape: unknown
            })
        );
        assert_eq!(objects, before);
        assert!(cues(&mut audio).is_empty());
    }
}
