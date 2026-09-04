//! HD-only point interpolation. Source pixels and simulation stay untouched.

use std::collections::HashMap;

use sf_core::point_field::{PointIdentity, PointPixel};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PresentedPoint {
    pub x: f32,
    pub y: f32,
    pub palette_index: u8,
}

impl From<&PointPixel> for PresentedPoint {
    fn from(pixel: &PointPixel) -> Self {
        Self {
            x: f32::from(pixel.x),
            y: f32::from(pixel.y),
            palette_index: pixel.palette_index,
        }
    }
}

pub(crate) fn interpolate_points(
    previous: Option<&[PointPixel]>,
    current: &[PointPixel],
    alpha: f32,
) -> Vec<PresentedPoint> {
    let alpha = alpha.clamp(0.0, 1.0);
    let Some(previous) = previous.filter(|_| alpha < 1.0) else {
        return current.iter().map(PresentedPoint::from).collect();
    };
    let current_by_identity: HashMap<_, _> = current
        .iter()
        .filter(|point| point.identity != PointIdentity::Untracked)
        .map(|point| (point.identity, point))
        .collect();
    previous
        .iter()
        .map(|point| {
            let mut presented = PresentedPoint::from(point);
            if let Some(next) = current_by_identity.get(&point.identity) {
                presented.x += (f32::from(next.x) - presented.x) * alpha;
                presented.y += (f32::from(next.y) - presented.y) * alpha;
            }
            // Palette and visibility are discrete. A clipped or respawned
            // point isn't paired with an unrelated neighbor or array index.
            presented
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID_POINT: PointIdentity = PointIdentity::Ground {
        column: 2,
        row: 3,
        lower: false,
    };

    fn point(identity: PointIdentity, x: u8) -> PointPixel {
        PointPixel {
            x,
            y: 20,
            palette_index: 14,
            identity,
        }
    }

    #[test]
    fn fractional_motion_matches_identity_after_clipping_reorders_the_list() {
        let previous = [point(PointIdentity::Untracked, 99), point(GRID_POINT, 10)];
        let current = [point(GRID_POINT, 13)];
        let middle = interpolate_points(Some(&previous), &current, 0.5);
        assert_eq!(middle[0].x, 99.0);
        assert_eq!(middle[1].x, 11.5);
        assert_eq!(
            interpolate_points(Some(&previous), &current, 0.0)[1].x,
            10.0
        );
        assert_eq!(
            interpolate_points(Some(&previous), &current, 1.0),
            current.iter().map(PresentedPoint::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn respawned_dust_does_not_streak_between_unrelated_positions() {
        let identity = |generation| PointIdentity::Dust {
            slot: 1,
            generation,
            lower: false,
        };
        let previous = [point(identity(1), 10)];
        let current = [point(identity(2), 200)];
        assert_eq!(
            interpolate_points(Some(&previous), &current, 0.5)[0].x,
            10.0
        );
        assert_eq!(
            interpolate_points(Some(&previous), &current, 1.0)[0].x,
            200.0
        );
    }

    #[test]
    fn missing_history_and_untracked_pixels_are_not_guessed() {
        let previous = [point(PointIdentity::Untracked, 10)];
        let current = [point(PointIdentity::Untracked, 200)];
        assert_eq!(interpolate_points(None, &current, 0.5)[0].x, 200.0);
        assert_eq!(
            interpolate_points(Some(&previous), &current, 0.5)[0].x,
            10.0
        );
    }
}
