//! Path one-shot cue routing and placement (`$7F:A412..A5B5`).
//!
//! These distance bands differ from positional engine loops. The caller
//! resolves the selected listener/marker and supplies its live heading;
//! direct cues retain authored IDs/parameters in the shared semantic queue.
//! PCM interpretation and asset availability remain separate audio concerns.

use super::{path_math, Angle, StereoPosition, Vector3};

use super::path_control::PlayerTarget;

/// Identity classification of the currently selected sound listener. The
/// fallback listener is outside the ordinary actor pool in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueListener {
    PrimaryPlayer,
    PrimaryFallback,
    Other,
}

/// Decoded authored cue data. The seven parameter bits are kept intact:
/// interpreting them for PCM playback is a separate audio-bank concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredCue {
    pub id: u8,
    parameter: u8,
    pub target: PlayerTarget,
}

impl AuthoredCue {
    pub const fn new(id: u8, parameter: u8, target: PlayerTarget) -> Self {
        assert!(
            parameter & 0x80 == 0,
            "routing is separate from cue parameters"
        );
        Self {
            id,
            parameter,
            target,
        }
    }

    pub const fn parameter(self) -> u8 {
        self.parameter
    }

    /// Source $7F:A439 ORs secondary routing; primary identities do not
    /// clear a secondary route already supplied by the authored cue.
    pub const fn for_listener(mut self, listener: CueListener) -> Self {
        if matches!(listener, CueListener::Other) {
            self.target = PlayerTarget::Secondary;
        }
        self
    }
}

/// Borrow the same queue used by gameplay sounds, never a path-local queue.
/// Identity observations are supplied for both selected-player slots so
/// callback entry can change selection without retaining a stale listener.
pub struct PathAudio<'a> {
    pub events: &'a mut super::AudioState,
    pub listeners: [CueListener; 2],
}

impl PathAudio<'_> {
    pub fn queue(&mut self, cue: AuthoredCue, selected: PlayerTarget) {
        let index = match selected {
            PlayerTarget::Primary => 0,
            PlayerTarget::Secondary => 1,
        };
        self.events.queue(super::SoundEvent::Authored(
            cue.for_listener(self.listeners[index]),
        ));
    }
}

const CLOSE_DISTANCE_EXCLUSIVE: u16 = 800;
const MIDDLE_DISTANCE_EXCLUSIVE: u16 = 1_300;
const FRONT_CENTER_END: u8 = 16;
const RIGHT_SECTOR_END: u8 = 112;
const REAR_CENTER_END: u8 = 144;
const LEFT_SECTOR_END: u8 = 240;
const FORWARD_RIGHT_END: u8 = 64;
const FORWARD_LEFT_START: u8 = 192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSoundClass {
    /// Close-range center cue, omitted at distance 800 and beyond.
    NearbyOnly,
    /// Three distance bands, always centered.
    DistanceOnly,
    /// Stereo placement within the first two distance bands.
    Positioned,
    /// In the first two bands, reject the rear half-plane.
    ForwardPositioned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueDistance {
    Close,
    Middle,
    Far,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CuePlacement {
    pub distance: CueDistance,
    pub position: StereoPosition,
}

/// Symmetric front/rear dead zones (`$7F:A570..A5B4`). Positive quarter
/// bearings are right; negative quarter bearings are left. Used by both
/// this path service and the native positional-loop selector.
pub fn stereo_position(relative_bearing: u8) -> StereoPosition {
    if (FRONT_CENTER_END..RIGHT_SECTOR_END).contains(&relative_bearing) {
        StereoPosition::Right
    } else if (REAR_CENTER_END..LEFT_SECTOR_END).contains(&relative_bearing) {
        StereoPosition::Left
    } else {
        StereoPosition::Center
    }
}

/// Classify a cue, evaluating the bearing only on paths that actually need
/// it. Far cues are centered even for the forward-only class: the source
/// jumps straight to enqueue before reaching the angle/rear suppression.
pub fn select_placement(
    class: PathSoundClass,
    distance: u16,
    relative_bearing: impl FnOnce() -> u8,
) -> Option<CuePlacement> {
    let distance = if distance < CLOSE_DISTANCE_EXCLUSIVE {
        CueDistance::Close
    } else if class == PathSoundClass::NearbyOnly {
        return None;
    } else if distance < MIDDLE_DISTANCE_EXCLUSIVE {
        CueDistance::Middle
    } else {
        CueDistance::Far
    };
    let position = if matches!(
        class,
        PathSoundClass::NearbyOnly | PathSoundClass::DistanceOnly
    ) || distance == CueDistance::Far
    {
        StereoPosition::Center
    } else {
        let bearing = relative_bearing();
        if class == PathSoundClass::ForwardPositioned
            && (FORWARD_RIGHT_END..FORWARD_LEFT_START).contains(&bearing)
        {
            return None;
        }
        stereo_position(bearing)
    };
    Some(CuePlacement { distance, position })
}

pub fn placement_at(
    class: PathSoundClass,
    source: Vector3,
    listener: Vector3,
    listener_bearing: Angle,
) -> Option<CuePlacement> {
    let delta = Vector3 {
        x: source.x.wrapping_sub(listener.x),
        y: 0,
        z: source.z.wrapping_sub(listener.z),
    };
    select_placement(class, path_math::vector_length(delta), || {
        let bearing = (sf_core::aim_angle::sf2_atan16(delta.x, delta.z) >> 8) as u8;
        bearing.wrapping_sub(listener_bearing.units())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_routing_preserves_every_cue_and_parameter_including_preselected_secondary() {
        for id in 0..=u8::MAX {
            for parameter in 0..0x80 {
                for target in [PlayerTarget::Primary, PlayerTarget::Secondary] {
                    let cue = AuthoredCue::new(id, parameter, target);
                    for listener in [
                        CueListener::PrimaryPlayer,
                        CueListener::PrimaryFallback,
                        CueListener::Other,
                    ] {
                        let routed = cue.for_listener(listener);
                        assert_eq!(routed.id, id);
                        assert_eq!(routed.parameter(), parameter);
                        assert_eq!(
                            routed.target,
                            if listener == CueListener::Other {
                                PlayerTarget::Secondary
                            } else {
                                target
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn authored_and_named_events_share_order_and_source_overflow() {
        use super::super::{AudioState, SoundEvent};
        let mut audio = AudioState::default();
        let cue = AuthoredCue::new(18, 0, PlayerTarget::Primary);
        for count in [15, 16, 17, 31, 32, 33] {
            let mut expected = Vec::new();
            for index in 0..count {
                let event = if index % 2 == 0 {
                    SoundEvent::RapidLaser
                } else {
                    SoundEvent::Authored(cue)
                };
                audio.queue(event);
                expected.push(event);
            }
            assert_eq!(
                audio
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
                expected[count - count % 16..]
            );
            assert!(audio.take_events().iter().all(Option::is_none));
        }
    }

    #[test]
    fn exact_distance_edges_and_non_positional_paths_do_not_query_bearing() {
        for (distance, expected) in [
            (0, CueDistance::Close),
            (799, CueDistance::Close),
            (800, CueDistance::Middle),
            (1_299, CueDistance::Middle),
            (1_300, CueDistance::Far),
            (u16::MAX, CueDistance::Far),
        ] {
            assert_eq!(
                select_placement(PathSoundClass::DistanceOnly, distance, || panic!()),
                Some(CuePlacement {
                    distance: expected,
                    position: StereoPosition::Center
                })
            );
            assert_eq!(
                select_placement(PathSoundClass::NearbyOnly, distance, || panic!()).is_some(),
                distance < 800
            );
        }
    }

    #[test]
    fn all_bearings_preserve_half_open_stereo_and_forward_rejection_sectors() {
        for bearing in 0..=u8::MAX {
            let expected = match bearing {
                16..=111 => StereoPosition::Right,
                144..=239 => StereoPosition::Left,
                _ => StereoPosition::Center,
            };
            assert_eq!(
                select_placement(PathSoundClass::Positioned, 500, || bearing),
                Some(CuePlacement {
                    distance: CueDistance::Close,
                    position: expected
                })
            );
            let forward = select_placement(PathSoundClass::ForwardPositioned, 800, || bearing);
            assert_eq!(forward.is_none(), (64..192).contains(&bearing));
            if let Some(forward) = forward {
                assert_eq!(forward.distance, CueDistance::Middle);
                assert_eq!(forward.position, expected);
            }
        }
    }

    #[test]
    fn far_forward_cues_are_not_suppressed_or_panned() {
        for class in [
            PathSoundClass::Positioned,
            PathSoundClass::ForwardPositioned,
        ] {
            assert_eq!(
                select_placement(class, 1_300, || panic!("far branch skips direction")),
                Some(CuePlacement {
                    distance: CueDistance::Far,
                    position: StereoPosition::Center
                })
            );
        }
    }

    #[test]
    fn world_sampling_wraps_coordinates_ignores_altitude_and_subtracts_listener_heading() {
        let source = Vector3 {
            x: -32_760,
            y: i16::MIN,
            z: 100,
        };
        let listener = Vector3 {
            x: 32_760,
            y: i16::MAX,
            z: 100,
        };
        assert_eq!(
            placement_at(PathSoundClass::Positioned, source, listener, Angle::ZERO),
            Some(CuePlacement {
                distance: CueDistance::Close,
                position: StereoPosition::Right
            })
        );
        assert_eq!(
            placement_at(
                PathSoundClass::Positioned,
                source,
                listener,
                Angle::from_units(64)
            ),
            Some(CuePlacement {
                distance: CueDistance::Close,
                position: StereoPosition::Center
            })
        );
    }
}
