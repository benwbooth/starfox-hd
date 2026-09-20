//! Downward surface selection (`$0D:AF3A..B209`).
//!
//! The caller supplies eligible object colliders in active-list order, with
//! the probing object excluded. This service owns geometry and surface
//! selection, not collision response or object-field mutation.

use sf2_data::collision_data::CollisionProfile;

use super::{collision_math, Angle, ObjectId, ObjectStore, ShapeId, Vector3};

const FULL_SEARCH_HEIGHT: i16 = 16_384;
const REDUCED_SEARCH_HEIGHT: i16 = 8_192;
const VERTICAL_MARGIN: i16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSearch {
    Full,
    Reduced,
}

/// Shared collision-mode byte maintained by player setup ($1B4D).
/// Paths read the whole byte; the surface search tests only its low three bits.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMode {
    pub flags: u8,
}

impl SurfaceMode {
    pub const fn search(self) -> SurfaceSearch {
        const SEARCH_MODE_BITS: u8 = 0x07;
        if self.flags & SEARCH_MODE_BITS == 0 {
            SurfaceSearch::Full
        } else {
            SurfaceSearch::Reduced
        }
    }
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

/// Persistent actor outputs of the downward probe ($1CE8..1CEB).
/// This relationship is independent of parenting and object-pair contacts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActorSurfaceContact {
    pub supporting_object: Option<ObjectId>,
    pub group: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectSurfaceContact {
    pub height: i16,
    pub contact: ActorSurfaceContact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceQueryError {
    MissingOwner(ObjectId),
    UnknownShape { object: ObjectId, shape: ShapeId },
}

/// Object-list adapter for $0D:AF3A, including the eligibility rules in
/// $7F:1BF0. A query is read-only; callers explicitly publish actor outputs.
/// This adapter exposes height and actor contacts only; response-specific
/// normals and footprint outputs remain outside this path observation.
pub fn query_object_surface(
    objects: &ObjectStore,
    owner: ObjectId,
    strategy_tick: u8,
    search: SurfaceSearch,
) -> Result<ObjectSurfaceContact, SurfaceQueryError> {
    let position = objects
        .get(owner)
        .ok_or(SurfaceQueryError::MissingOwner(owner))?
        .base
        .position;
    let mut identities = Vec::new();
    let mut colliders = Vec::new();
    for &id in objects.active_ids() {
        let actor = objects.get(id).expect("active surface candidate");
        if id == owner
            || actor.base.contacts.first_strategy_visit
            || actor.base.flags.exclude_from_shape_footprint_search
        {
            continue;
        }
        let collider = SurfaceCollider::from_shape(
            actor.base.shape,
            actor.base.position,
            actor.base.yaw,
            actor.extension.path_state.animation.shape.fixed_frame(),
        )
        .ok_or(SurfaceQueryError::UnknownShape {
            object: id,
            shape: actor.base.shape,
        })?;
        identities.push(id);
        colliders.push(collider);
    }
    let result = query_surface(position, &colliders, strategy_tick, search);
    Ok(ObjectSurfaceContact {
        height: result.height,
        contact: ActorSurfaceContact {
            supporting_object: result.collider_index.map(|index| identities[index]),
            group: result.group,
            flags: result.flags,
        },
    })
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
    use super::super::{Behavior, Object, ObjectKind};
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

    fn object(shape: u16, y: i16) -> Object {
        let mut object = Object::new(
            ObjectKind::Enemy,
            ShapeId::from_catalog_index(shape),
            Behavior::FollowPath,
        );
        object.base.position.y = y;
        object.base.contacts.first_strategy_visit = false;
        object
    }

    #[test]
    fn object_query_uses_live_list_order_and_only_source_eligibility_flags() {
        let mut objects = ObjectStore::new();
        // Shape 7 is an ordinary box with half extents [16, 16, 16].
        let owner = objects.allocate(object(7, -100)).unwrap();
        let first = objects.allocate(object(7, 0)).unwrap();
        let second = objects.allocate(object(7, 0)).unwrap();
        let order: Vec<_> = objects
            .active_ids()
            .iter()
            .copied()
            .filter(|id| *id != owner)
            .collect();
        assert!(order.contains(&first) && order.contains(&second));
        let [head, tail] = [order[0], order[1]];
        objects.get_mut(head).unwrap().base.flags.collision_disabled = true;
        let before = objects.clone();
        let result = query_object_surface(&objects, owner, 0, SurfaceSearch::Full).unwrap();
        assert_eq!(result.height, -16);
        assert_eq!(
            result.contact,
            ActorSurfaceContact {
                supporting_object: Some(head),
                group: 0,
                flags: 0
            }
        );
        assert_eq!(objects, before);
        for (fresh, excluded) in [(true, false), (false, true), (true, true)] {
            let actor = objects.get_mut(head).unwrap();
            actor.base.contacts.first_strategy_visit = fresh;
            actor.base.flags.exclude_from_shape_footprint_search = excluded;
            assert_eq!(
                query_object_surface(&objects, owner, 0, SurfaceSearch::Full)
                    .unwrap()
                    .contact
                    .supporting_object,
                Some(tail)
            );
        }
        objects.get_mut(tail).unwrap().base.position.x = 100;
        assert_eq!(
            query_object_surface(&objects, owner, 0, SurfaceSearch::Reduced).unwrap(),
            ObjectSurfaceContact {
                height: 0,
                contact: ActorSurfaceContact::default()
            }
        );
    }

    #[test]
    fn object_query_samples_authored_animation_without_render_clock_masking() {
        use super::super::path_appearance::AnimationControl;
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(object(0, -1000)).unwrap();
        // Source shape 200: three flat planes, with offsets 64, 128, 192.
        let candidate = objects.allocate(object(200, 0)).unwrap();
        for (packed, clock, expected) in [
            (0, 0, -65),
            (0, 1, -129),
            (0, 129, -65),
            (129, 0, -129),
            (129, 255, -129),
            (255, 1, -65),
        ] {
            let actor = objects.get_mut(candidate).unwrap();
            actor.extension.path_state.animation.shape = AnimationControl::from_packed(packed);
            actor.extension.animation_frame = 2; // Renderer output is not the source channel.
            let result = query_object_surface(&objects, owner, clock, SurfaceSearch::Full).unwrap();
            assert_eq!(result.height, expected, "packed {packed}, clock {clock}");
            assert_eq!(result.contact.supporting_object, Some(candidate));
            assert_eq!(result.contact.group, 1);
        }
    }

    #[test]
    fn object_query_returns_compound_flags_and_rejects_unknown_eligible_shapes() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(object(0, -1000)).unwrap();
        let candidate = objects.allocate(object(156, 0)).unwrap();
        assert_eq!(
            query_object_surface(&objects, owner, 0, SurfaceSearch::Full).unwrap(),
            ObjectSurfaceContact {
                height: -403,
                contact: ActorSurfaceContact {
                    supporting_object: Some(candidate),
                    group: 1,
                    flags: 6
                }
            }
        );
        objects.get_mut(candidate).unwrap().base.shape = ShapeId::from_catalog_index(u16::MAX);
        let before = objects.clone();
        assert_eq!(
            query_object_surface(&objects, owner, 0, SurfaceSearch::Full),
            Err(SurfaceQueryError::UnknownShape {
                object: candidate,
                shape: ShapeId::from_catalog_index(u16::MAX)
            })
        );
        assert_eq!(objects, before);
        objects
            .get_mut(candidate)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit = true;
        assert_eq!(
            query_object_surface(&objects, owner, 0, SurfaceSearch::Full)
                .unwrap()
                .contact,
            ActorSurfaceContact::default()
        );
        objects.remove(owner).unwrap();
        assert_eq!(
            query_object_surface(&objects, owner, 0, SurfaceSearch::Full),
            Err(SurfaceQueryError::MissingOwner(owner))
        );
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
