//! Downward surface selection (`$0D:AF3A..B209`).
//!
//! The caller supplies eligible object colliders in active-list order, with
//! the probing object excluded. This service owns geometry and surface
//! selection, not collision response or object-field mutation.

use sf2_data::collision_data::CollisionProfile;

use super::{collision_math, Angle, ShapeId, Vector3};

const FULL_SEARCH_HEIGHT: i16 = 16_384;
const REDUCED_SEARCH_HEIGHT: i16 = 8_192;
const VERTICAL_MARGIN: i16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSearch {
    Full,
    Reduced,
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceCollider<'a> {
    pub position: Vector3,
    pub yaw: Angle,
    pub bounds: [u16; 3],
    pub profile: Option<&'a CollisionProfile>,
    /// A fixed animation frame; None uses the shared strategy clock byte.
    pub animation_frame: Option<u8>,
}

impl SurfaceCollider<'static> {
    /// Decode static shape geometry at the catalog boundary. Eligibility,
    /// current-object exclusion and list order remain the caller's job.
    pub fn from_shape(
        shape: ShapeId,
        position: Vector3,
        yaw: Angle,
        animation_frame: Option<u8>,
    ) -> Option<Self> {
        Some(Self {
            position,
            yaw,
            bounds: shape.catalog_entry()?.bounds,
            profile: sf2_data::collision_data::collision_profile_by_index(shape.catalog_index()),
            animation_frame,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceContact {
    pub height: i16,
    pub collider_index: Option<usize>,
    /// Compound group counted backward, as authored; ordinary boxes use 0.
    pub group: u8,
    pub flags: u8,
}

/// Source bounds admit the positive edge and reject the negative edge.
fn broad_axis_contains(center: i16, radius: u16, point: i16) -> bool {
    (center as u16)
        .wrapping_add(radius)
        .wrapping_sub(point as u16)
        < radius.wrapping_mul(2)
}

fn footprint_axis_contains(center: i16, width: u16, point: i16) -> bool {
    (width >> 1)
        .wrapping_add(center as u16)
        .wrapping_sub(point as u16)
        < width
}

pub fn query_surface(
    position: Vector3,
    colliders: &[SurfaceCollider<'_>],
    strategy_tick: u8,
    search: SurfaceSearch,
) -> SurfaceContact {
    let mut nearest = SurfaceContact {
        height: match search {
            SurfaceSearch::Full => FULL_SEARCH_HEIGHT,
            SurfaceSearch::Reduced => REDUCED_SEARCH_HEIGHT,
        },
        collider_index: None,
        group: 0,
        flags: 0,
    };
    for (index, collider) in colliders.iter().enumerate() {
        let [width, height, depth] = collider.bounds;
        if !broad_axis_contains(collider.position.x, width, position.x)
            || !broad_axis_contains(collider.position.z, depth, position.z)
        {
            continue;
        }
        let surface = if let Some(profile) = collider.profile {
            let (x, z) = collision_math::local_probe(
                collider.yaw,
                position.x.wrapping_sub(collider.position.x),
                position.z.wrapping_sub(collider.position.z),
            );
            let frame = usize::from(collider.animation_frame.unwrap_or(strategy_tick));
            profile
                .groups
                .iter()
                .enumerate()
                .find_map(|(group_index, group)| {
                    let record = group.variants.get(frame).unwrap_or(&group.variants[0]);
                    if !footprint_axis_contains(record.center_x, record.width, x)
                        || !footprint_axis_contains(record.center_z, record.depth, z)
                    {
                        return None;
                    }
                    if record.polygon.is_some_and(|polygon| {
                        !collision_math::polygon_contains(polygon.vertices, polygon.scale, x, z)
                    }) {
                        return None;
                    }
                    Some((
                        collider
                            .position
                            .y
                            .wrapping_add(collision_math::plane_height(
                                record.plane_normal,
                                record.plane_offset,
                                x,
                                z,
                            )),
                        (profile.groups.len() - group_index) as u8,
                        record.box_flags,
                    ))
                })
        } else {
            Some((collider.position.y.wrapping_sub(height as i16), 0, 0))
        };
        let Some((surface, group, flags)) = surface else {
            continue;
        };
        // Only the first containing compound record is considered, even if
        // its vertical tests fail. The source then advances to the next
        // object, not to the remaining groups of the same collider.
        let upper_edge = surface
            .wrapping_add(height as i16)
            .wrapping_add(VERTICAL_MARGIN);
        if upper_edge.wrapping_sub(position.y) < 0 || surface.wrapping_sub(nearest.height) >= 0 {
            continue;
        }
        nearest = SurfaceContact {
            height: surface,
            collider_index: Some(index),
            group,
            flags,
        };
    }
    if nearest.height == REDUCED_SEARCH_HEIGHT {
        nearest.height = 0;
    }
    nearest
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf2_data::collision_data::{CollisionGroup, CollisionRecord};

    const RECORD: CollisionRecord = CollisionRecord {
        center_x: 0,
        center_z: 0,
        width: 100,
        depth: 100,
        plane_normal: [0, 64, 0],
        plane_offset: 0,
        polygon: None,
        box_flags: 3,
    };
    const PROFILE: CollisionProfile = CollisionProfile {
        groups: &[
            CollisionGroup {
                variants: &[RECORD],
            },
            CollisionGroup {
                variants: &[CollisionRecord {
                    plane_offset: 64,
                    ..RECORD
                }],
            },
        ],
    };

    fn collider(profile: Option<&CollisionProfile>) -> SurfaceCollider<'_> {
        SurfaceCollider {
            position: Vector3::default(),
            yaw: Angle::ZERO,
            bounds: [100; 3],
            profile,
            animation_frame: None,
        }
    }

    #[test]
    fn every_authored_compound_profile_is_available_by_native_catalog_id() {
        let mut count = 0;
        for index in 0..sf2_data::shape_data::SHAPE_DATA.len() {
            let collider = SurfaceCollider::from_shape(
                ShapeId::from_catalog_index(index as u16),
                Vector3::default(),
                Angle::ZERO,
                None,
            )
            .unwrap();
            if let Some(profile) = collider.profile {
                count += 1;
                assert!(!profile.groups.is_empty());
                for group in profile.groups {
                    assert!(!group.variants.is_empty());
                    for record in group.variants {
                        if let Some(polygon) = record.polygon {
                            assert!(polygon.vertices.len() >= 3);
                            assert_eq!(polygon.vertices.first(), polygon.vertices.last());
                        }
                    }
                }
            }
        }
        assert_eq!(
            count,
            sf2_data::collision_data::COMPOUND_COLLIDER_SHAPE_COUNT
        );
    }

    #[test]
    fn reduced_no_contact_falls_back_to_ground_but_full_search_keeps_limit() {
        assert_eq!(
            query_surface(Vector3::default(), &[], 0, SurfaceSearch::Reduced).height,
            0
        );
        assert_eq!(
            query_surface(Vector3::default(), &[], 0, SurfaceSearch::Full).height,
            16_384
        );
    }

    #[test]
    fn bounds_and_compound_footprints_keep_their_half_open_source_edges() {
        assert!(!broad_axis_contains(0, 100, -100));
        assert!(broad_axis_contains(0, 100, 100));
        assert!(!footprint_axis_contains(0, 100, -50));
        assert!(footprint_axis_contains(0, 100, 50));
    }

    #[test]
    fn first_containing_group_wins_even_if_later_group_has_a_nearer_plane() {
        let result = query_surface(
            Vector3 {
                x: 0,
                y: -200,
                z: 0,
            },
            &[collider(Some(&PROFILE))],
            0,
            SurfaceSearch::Full,
        );
        assert_eq!(
            result,
            SurfaceContact {
                height: 0,
                collider_index: Some(0),
                group: 2,
                flags: 3
            }
        );
    }

    #[test]
    fn equal_height_ties_keep_active_object_order() {
        let result = query_surface(
            Vector3 {
                x: 0,
                y: -200,
                z: 0,
            },
            &[collider(None), collider(None)],
            0,
            SurfaceSearch::Full,
        );
        assert_eq!(result.collider_index, Some(0));
        assert_eq!(result.height, -100);
    }

    #[test]
    fn animation_outside_variant_count_uses_first_record_not_modulo() {
        let profile = CollisionProfile {
            groups: &[CollisionGroup {
                variants: &[
                    RECORD,
                    CollisionRecord {
                        plane_offset: 64,
                        ..RECORD
                    },
                ],
            }],
        };
        let colliders = [collider(Some(&profile))];
        let probe = Vector3 {
            x: 0,
            y: -200,
            z: 0,
        };
        assert_eq!(
            query_surface(probe, &colliders, 1, SurfaceSearch::Full).height,
            -128
        );
        assert_eq!(
            query_surface(probe, &colliders, 3, SurfaceSearch::Full).height,
            0
        );
    }
}
