//! Path one-shot cue placement (`$7F:A460..A5B5`).
//!
//! These distance bands differ from positional engine loops. The caller
//! resolves the selected listener/marker and supplies its live heading;
//! cue identity, listener routing and native PCM assets remain audio-owned.

use super::{path_math, Angle, StereoPosition, Vector3};

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
