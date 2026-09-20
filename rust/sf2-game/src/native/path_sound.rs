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
    pub markers: Option<MarkerInputs>,
}

/// Fixed listener markers are not the selected actor's pose. The selected
/// actor's view-side flag (23 bit 40) chooses between them at `$7F:A3FB`.
/// That observation is independent of the path owner's player-selection
/// flag (24 bit 80), so both selected slots supply their own marker side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerInputs {
    pub selected_sides: [PlayerTarget; 2],
    pub markers: [CueMarker; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CueMarker {
    pub identity: CueListener,
    pub position: Vector3,
    /// Source marker byte 15, not the selected actor's base yaw (byte 14).
    pub bearing: Angle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerRange {
    Near,
    Wide,
}

impl MarkerRange {
    fn distance(self) -> u16 {
        match self {
            Self::Near => 800,
            Self::Wide => 5_120,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerCueMode {
    DistanceBands(PathSoundClass),
    RangeLimited(MarkerRange),
}

fn player_index(target: PlayerTarget) -> usize {
    match target {
        PlayerTarget::Primary => 0,
        PlayerTarget::Secondary => 1,
    }
}

impl PathAudio<'_> {
    pub fn queue(&mut self, cue: AuthoredCue, selected: PlayerTarget) {
        let index = player_index(selected);
        self.events.queue(super::SoundEvent::Authored(
            cue.for_listener(self.listeners[index]),
        ));
    }

    /// Missing marker observations are checked by the caller before enqueue.
    /// Routing uses the resolved marker identity, not the direct-cue listener.
    pub fn queue_marker(
        &mut self,
        id: u8,
        mode: MarkerCueMode,
        source: Vector3,
        selected: PlayerTarget,
    ) -> Result<(), MissingSoundMarkers> {
        let inputs = self.markers.ok_or(MissingSoundMarkers)?;
        let side = inputs.selected_sides[player_index(selected)];
        let marker = inputs.markers[player_index(side)];
        if let Some(cue) = marker_cue(id, mode, source, marker) {
            self.events.queue(super::SoundEvent::Authored(cue));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingSoundMarkers;

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

impl CuePlacement {
    /// `$7F:A4D5..A4EB`, `$7F:A5A0..A5B1`: stereo is added, not ORed.
    /// In particular the middle-left parameter is 0x40, not 0x30.
    fn parameter(self) -> u8 {
        let distance: u8 = match self.distance {
            CueDistance::Close => 0,
            CueDistance::Middle => 0x30,
            CueDistance::Far => 0x60,
        };
        let stereo = match self.position {
            StereoPosition::Center => 0,
            StereoPosition::Right => 0x20,
            StereoPosition::Left => 0x10,
        };
        distance.wrapping_add(stereo)
    }
}

/// The two range-limited handlers test the sign of a wrapped subtraction,
/// not unsigned less-than. Retain its high-distance wraparound behavior.
fn range_placement(
    range: MarkerRange,
    distance: u16,
    bearing: impl FnOnce() -> u8,
) -> Option<CuePlacement> {
    ((distance.wrapping_sub(range.distance()) as i16) < 0).then(|| CuePlacement {
        distance: CueDistance::Close,
        position: stereo_position(bearing()),
    })
}

pub fn marker_cue(
    id: u8,
    mode: MarkerCueMode,
    source: Vector3,
    marker: CueMarker,
) -> Option<AuthoredCue> {
    let delta = Vector3 {
        x: source.x.wrapping_sub(marker.position.x),
        y: 0,
        z: source.z.wrapping_sub(marker.position.z),
    };
    let distance = path_math::vector_length(delta);
    let bearing = || {
        let angle = (sf_core::aim_angle::sf2_atan16(delta.x, delta.z) >> 8) as u8;
        angle.wrapping_sub(marker.bearing.units())
    };
    let placement = match mode {
        MarkerCueMode::DistanceBands(class) => select_placement(class, distance, bearing),
        MarkerCueMode::RangeLimited(range) => range_placement(range, distance, bearing),
    }?;
    Some(
        AuthoredCue::new(id, placement.parameter(), PlayerTarget::Primary)
            .for_listener(marker.identity),
    )
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
    fn banded_parameters_add_stereo_for_every_bearing_and_preserve_literal_ids() {
        for id in 0..=u8::MAX {
            for bearing in 0..=u8::MAX {
                let stereo = match bearing {
                    16..=111 => 0x20,
                    144..=239 => 0x10,
                    _ => 0,
                };
                for (distance, parameter) in
                    [(0, 0), (799, 0), (800, 0x30), (1299, 0x30), (1300, 0x60)]
                {
                    for class in [PathSoundClass::DistanceOnly, PathSoundClass::Positioned] {
                        let source = Vector3 {
                            x: 0,
                            y: i16::MIN,
                            z: distance,
                        };
                        let absolute = (sf_core::aim_angle::sf2_atan16(0, distance) >> 8) as u8;
                        let marker = CueMarker {
                            identity: CueListener::Other,
                            position: Vector3 {
                                x: 0,
                                y: i16::MAX,
                                z: 0,
                            },
                            bearing: Angle::from_units(absolute.wrapping_sub(bearing)),
                        };
                        let expected = parameter
                            + if class == PathSoundClass::Positioned && distance < 1300 {
                                stereo
                            } else {
                                0
                            };
                        assert_eq!(
                            marker_cue(id, MarkerCueMode::DistanceBands(class), source, marker),
                            Some(AuthoredCue::new(id, expected, PlayerTarget::Secondary))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn limited_ranges_preserve_every_wrapped_word_comparison_and_skip_unused_bearing() {
        for (range, limit) in [(MarkerRange::Near, 800_u16), (MarkerRange::Wide, 5120_u16)] {
            for distance in 0..=u16::MAX {
                // Independent source sign-bit calculation, including the
                // high-distance accepted band beyond limit + 32767.
                let accepted = (i32::from(distance) - i32::from(limit)) & 0x8000 != 0;
                for bearing in [0, 15, 16, 111, 112, 143, 144, 239, 240, 255] {
                    let actual = range_placement(range, distance, || {
                        assert!(accepted);
                        bearing
                    });
                    let parameter = match bearing {
                        16..=111 => 0x20,
                        144..=239 => 0x10,
                        _ => 0,
                    };
                    assert_eq!(
                        actual.map(CuePlacement::parameter),
                        accepted.then_some(parameter)
                    );
                }
            }
        }
    }

    #[test]
    fn range_limited_cue_wraps_coordinates_ignores_height_and_retains_marker_identity() {
        for identity in [
            CueListener::PrimaryPlayer,
            CueListener::PrimaryFallback,
            CueListener::Other,
        ] {
            let marker = CueMarker {
                identity,
                position: Vector3 {
                    x: i16::MAX,
                    y: i16::MIN,
                    z: i16::MIN,
                },
                bearing: Angle::from_units(17),
            };
            let source = Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: i16::MAX,
            };
            let angle = ((sf_core::aim_angle::sf2_atan16(1, -1) >> 8) as u8).wrapping_sub(17);
            let expected = match angle {
                16..=111 => 0x20,
                144..=239 => 0x10,
                _ => 0,
            };
            for range in [MarkerRange::Near, MarkerRange::Wide] {
                assert_eq!(
                    marker_cue(255, MarkerCueMode::RangeLimited(range), source, marker),
                    Some(
                        AuthoredCue::new(255, expected, PlayerTarget::Primary)
                            .for_listener(identity)
                    )
                );
            }
        }
    }

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
