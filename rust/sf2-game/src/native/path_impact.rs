//! Projectile contact classification (`$0D:DE78..DFD8`, `$7F:BDF8`).
//! Pair contacts take precedence over terrain. Surface probing publishes its
//! link and flags but retains the actor's previous surface-group byte.

use super::collision_contacts::ContactStore;
use super::collision_surface::{self, SurfaceMode, SurfaceQueryError};
use super::path_control::PlayerTarget;
use super::{ObjectId, ObjectStore};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;

const GROUND_CONTACT_MARGIN: i16 = 20;

/// Read-only view of authored auxiliary records 11 and 13. The records live
/// in ActorAuxiliary, not in a second persistent copy on the actor. Updating
/// an existing record replaces its cue; neither aliases render materials.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImpactMaterials {
    pub ordinary: Option<u8>,
    pub suppressed: Option<u8>,
}

#[cfg(test)]
pub(super) fn material_fixture(
    actor: &mut super::Object,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    materials: ImpactMaterials,
) {
    use super::actor_auxiliary::AuxiliaryRecord;
    if let Some(value) = materials.ordinary {
        actor.extension.auxiliary.set(resources, owner, AuxiliaryRecord::OrdinaryImpactMaterial(value)).unwrap();
    }
    if let Some(value) = materials.suppressed {
        actor.extension.auxiliary.set(resources, owner, AuxiliaryRecord::SuppressedImpactMaterial(value)).unwrap();
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImpactState {
    /// Last classifier result's cue family; even a miss clears this byte.
    pub material: u8,
    /// Last *pair* resolution's suppression observation (D746). Ground,
    /// surface-latch and miss paths leave this independent byte unchanged.
    pub pair_suppressed: bool,
}

/// The three authored branch operands, not damage amounts. Player contacts
/// retain a source slot-dependent choice between First and Third.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactBranch {
    First,
    Second,
    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactError {
    Auxiliary(super::actor_auxiliary::AuxiliaryError),
    MissingActor(ObjectId),
    MissingContacts,
    MissingPair(ObjectId),
    MissingSurfaceMode,
    Surface(SurfaceQueryError),
}

/// `$0D:DF64..DF78` returns the player identity's residual low byte rather
/// than an explicit class. `$7F:BE08` decrements it before the signed branch.
/// The initialized sixty-slot pool gives this repeating four-slot pattern;
/// no source pointer is reconstructed or exposed in the native runtime.
fn player_branch(player: ObjectId) -> ImpactBranch {
    const SLOT_BRANCH_PERIOD: usize = 4;
    const FIRST_BRANCH_SLOTS: usize = 2;
    if player.index() % SLOT_BRANCH_PERIOD < FIRST_BRANCH_SLOTS {
        ImpactBranch::First
    } else {
        ImpactBranch::Third
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ShapeId};

    fn actor(shape: u16, y: i16) -> Object {
        let mut actor = Object::new(
            ObjectKind::Enemy,
            ShapeId::from_catalog_index(shape),
            Behavior::FollowPath,
        );
        actor.base.position.y = y;
        actor.base.hit_points = 1;
        actor.base.contacts.first_strategy_visit = false;
        actor
    }

    #[test]
    fn pair_classification_reads_head_live_health_and_correct_material_without_consuming_contacts()
    {
        let mut objects = ObjectStore::new();
        let mut resources = ProgramResources::default();
        let owner = objects.allocate(actor(0, 300)).unwrap();
        let peer = objects.allocate(actor(0, 0)).unwrap();
        let ignored = objects.allocate(actor(0, 0)).unwrap();
        let mut contacts = ContactStore::default();
        contacts
            .record_pair(owner, peer, [Some(0xA5), Some(0x5A)])
            .unwrap();
        contacts.record_pair(owner, ignored, [None, None]).unwrap();
        let head = *contacts.get(contacts.first(owner).unwrap()).unwrap();
        for bits in 1..4 {
            for suppressed in [false, true] {
                for health in [0, 1, 128, 255] {
                    for value in 0..=u8::MAX {
                        for present in [false, true] {
                            let source = objects.get_mut(owner).unwrap();
                            source.base.contacts.pending_hit = bits & 1 != 0;
                            source.base.contacts.previous_hit = bits & 2 != 0;
                            let target = objects.get_mut(peer).unwrap();
                            target.base.hit_points = health;
                            target.base.contacts.suppress_contacts_next_epoch = suppressed;
                            resources.release_owner(peer);
                            target.extension.auxiliary.clear_after_owner_release(&resources, peer).unwrap();
                            material_fixture(target, &mut resources, peer, ImpactMaterials {
                                ordinary: present.then_some(value),
                                suppressed: present.then_some(!value),
                            });
                            let before = objects.clone();
                            let mut state = ImpactState {
                                material: 87,
                                pair_suppressed: !suppressed,
                            };
                            let result = classify(
                                &mut objects,
                                &resources,
                                Some(&contacts),
                                owner,
                                [None; 2],
                                None,
                                255,
                                &mut state,
                            )
                            .unwrap();
                            assert_eq!(
                                result,
                                Some(if health == 0 {
                                    ImpactBranch::First
                                } else if suppressed {
                                    ImpactBranch::Third
                                } else {
                                    ImpactBranch::Second
                                })
                            );
                            assert_eq!(
                                state,
                                ImpactState {
                                    pair_suppressed: suppressed,
                                    material: if health == 0 || !present {
                                        0
                                    } else if suppressed {
                                        !value
                                    } else {
                                        value
                                    },
                                }
                            );
                            assert_eq!(objects, before);
                            assert_eq!(contacts.get(contacts.first(owner).unwrap()), Some(&head));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn both_live_player_pointers_override_health_materials_and_have_all_sixty_slot_choices() {
        let mut objects = ObjectStore::new();
        let mut resources = ProgramResources::default();
        let ids: Vec<_> = (0..super::super::OBJECT_CAPACITY)
            .map(|_| objects.allocate(actor(0, 0)).unwrap())
            .collect();
        for (index, &peer) in ids.iter().enumerate() {
            let owner = ids[(index + 1) % ids.len()];
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, peer, [None; 2]).unwrap();
            objects.get_mut(owner).unwrap().base.contacts.pending_hit = true;
            for suppressed in [false, true] {
                for health in [0, 255] {
                    let target = objects.get_mut(peer).unwrap();
                    target.base.hit_points = health;
                    target.base.contacts.suppress_contacts_next_epoch = suppressed;
                    material_fixture(target, &mut resources, peer, ImpactMaterials {
                        ordinary: Some(255),
                        suppressed: Some(128),
                    });
                    for players in [[Some(peer), None], [None, Some(peer)], [Some(peer); 2]] {
                        let before = objects.clone();
                        let mut state = ImpactState::default();
                        let result = classify(
                            &mut objects,
                            &resources,
                            Some(&contacts),
                            owner,
                            players,
                            None,
                            0,
                            &mut state,
                        )
                        .unwrap();
                        assert_eq!(
                            result,
                            Some(match index % 4 {
                                0 | 1 => ImpactBranch::First,
                                _ => ImpactBranch::Third,
                            })
                        );
                        assert_eq!(state.material, 0);
                        assert_eq!(objects, before);
                    }
                }
            }
        }
    }

    #[test]
    fn ground_gate_uses_wrapping_signed_height_and_only_low_three_mode_bits() {
        let mut objects = ObjectStore::new();
        let resources = ProgramResources::default();
        let owner = objects.allocate(actor(0, 0)).unwrap();
        for raw in 0..=u16::MAX {
            for flags in [0, 1, 7, 8, 128, 255] {
                let source = objects.get_mut(owner).unwrap();
                source.base.position.y = raw as i16;
                source.extension.surface_contact = collision_surface::ActorSurfaceContact {
                    supporting_object: Some(owner),
                    group: 0xA5,
                    flags: 0x5A,
                };
                let mut expected = objects.clone();
                let ground = flags & 7 != 0 && (raw as i16).wrapping_add(20) >= 0;
                if !ground {
                    expected.get_mut(owner).unwrap().extension.surface_contact =
                        collision_surface::ActorSurfaceContact {
                            group: 0xA5,
                            ..Default::default()
                        };
                }
                let mut state = ImpactState {
                    material: 255,
                    pair_suppressed: true,
                };
                assert_eq!(
                    classify(
                        &mut objects,
                        &resources,
                        None,
                        owner,
                        [None; 2],
                        Some(SurfaceMode { flags }),
                        0,
                        &mut state
                    )
                    .unwrap(),
                    ground.then_some(ImpactBranch::First)
                );
                assert_eq!(
                    state,
                    ImpactState {
                        material: 0,
                        pair_suppressed: true
                    }
                );
                assert_eq!(objects, expected);
            }
        }
    }

    #[test]
    fn surface_probe_retains_group_publishes_link_and_flags_and_credits_path_selected_side() {
        for bits in 0..32 {
            let mut objects = ObjectStore::new();
            let mut resources = ProgramResources::default();
            let owner = objects.allocate(actor(0, -403)).unwrap();
            let surface = objects.allocate(actor(156, 0)).unwrap();
            let other = objects.allocate(actor(0, 0)).unwrap();
            objects
                .get_mut(other)
                .unwrap()
                .base
                .flags
                .exclude_from_shape_footprint_search = true;
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, other, [None; 2]).unwrap();
            let source = objects.get_mut(owner).unwrap();
            source.extension.surface_contact.group = 0xA5;
            source.base.contacts.mutually_non_damaging = bits & 1 != 0;
            source.base.contacts.credits_hit_side = bits & 2 != 0;
            source.extension.path_state.conditions.selected_player = if bits & 4 == 0 {
                PlayerTarget::Primary
            } else {
                PlayerTarget::Secondary
            };
            source.base.contacts.hit_side = if bits & 4 == 0 {
                crate::hit_response::HitSide::Secondary
            } else {
                crate::hit_response::HitSide::Primary
            };
            let target = objects.get_mut(surface).unwrap();
            target.base.contacts.latch_new_contact = bits & 8 != 0;
            target.base.contacts.suppress_contacts_next_epoch = bits & 16 != 0;
            material_fixture(target, &mut resources, surface, ImpactMaterials {
                ordinary: Some(33),
                suppressed: Some(44),
            });
            let target = objects.get_mut(other).unwrap();
            target.base.contacts.suppress_contacts_next_epoch = false;
            material_fixture(target, &mut resources, other, ImpactMaterials { ordinary: Some(55), suppressed: None });
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().extension.surface_contact =
                collision_surface::ActorSurfaceContact {
                    supporting_object: Some(surface),
                    group: 0xA5,
                    flags: 6,
                };
            let target = expected.get_mut(surface).unwrap();
            if bits & 3 == 3 {
                target.base.contacts.hit_by_primary = bits & 4 == 0;
                target.base.contacts.hit_by_secondary = bits & 4 != 0;
            }
            target.base.contacts.new_contact_latched = bits & 8 != 0;
            let mut state = ImpactState {
                material: 77,
                pair_suppressed: true,
            };
            let latched = bits & 8 != 0;
            let suppressed = latched && bits & 16 != 0;
            assert_eq!(
                classify(
                    &mut objects,
                    &resources,
                    Some(&contacts),
                    owner,
                    [None; 2],
                    Some(SurfaceMode::default()),
                    0,
                    &mut state
                )
                .unwrap(),
                Some(if suppressed {
                    ImpactBranch::Third
                } else {
                    ImpactBranch::Second
                })
            );
            assert_eq!(
                state,
                ImpactState {
                    pair_suppressed: latched,
                    material: if !latched {
                        55
                    } else if suppressed {
                        44
                    } else {
                        33
                    }
                }
            );
            assert_eq!(objects, expected);
        }
    }

    #[test]
    fn missing_inputs_and_stale_pair_links_are_errors_not_synthetic_hits() {
        let mut objects = ObjectStore::new();
        let resources = ProgramResources::default();
        let owner = objects.allocate(actor(0, 0)).unwrap();
        let peer = objects.allocate(actor(0, 0)).unwrap();
        let mut contacts = ContactStore::default();
        let mut state = ImpactState {
            material: 77,
            pair_suppressed: true,
        };
        let before = objects.clone();
        assert_eq!(
            classify(&mut objects, &resources, None, owner, [None; 2], None, 0, &mut state),
            Err(ImpactError::MissingSurfaceMode)
        );
        assert_eq!(objects, before);
        objects.get_mut(owner).unwrap().base.contacts.pending_hit = true;
        assert_eq!(
            classify(&mut objects, &resources, None, owner, [None; 2], None, 0, &mut state),
            Err(ImpactError::MissingContacts)
        );
        assert_eq!(
            classify(
                &mut objects,
                &resources,
                Some(&contacts),
                owner,
                [None; 2],
                None,
                0,
                &mut state
            ),
            Err(ImpactError::MissingPair(owner))
        );
        contacts.record_pair(owner, peer, [None; 2]).unwrap();
        objects.remove(peer).unwrap();
        assert_eq!(
            classify(
                &mut objects,
                &resources,
                Some(&contacts),
                owner,
                [None; 2],
                None,
                0,
                &mut state
            ),
            Err(ImpactError::MissingActor(peer))
        );
        assert_eq!(
            state,
            ImpactState {
                material: 77,
                pair_suppressed: true
            }
        );
    }
}

/// Read the actual head pair, never a selected/nearest actor. Contact records
/// and pending/previous-hit flags are observations, not consumed here.
fn pair_peer(
    objects: &ObjectStore,
    contacts: Option<&ContactStore>,
    owner: ObjectId,
    state: &mut ImpactState,
) -> Result<ObjectId, ImpactError> {
    let contacts = contacts.ok_or(ImpactError::MissingContacts)?;
    let peer = contacts
        .first(owner)
        .and_then(|head| contacts.get(head))
        .ok_or(ImpactError::MissingPair(owner))?
        .other;
    let actor = objects.get(peer).ok_or(ImpactError::MissingActor(peer))?;
    state.pair_suppressed = actor.base.contacts.suppress_contacts_next_epoch;
    Ok(peer)
}

pub fn classify(
    objects: &mut ObjectStore,
    resources: &ProgramResources<ProgramData>,
    contacts: Option<&ContactStore>,
    owner: ObjectId,
    players: [Option<ObjectId>; 2],
    surface_mode: Option<SurfaceMode>,
    animation_clock: u8,
    state: &mut ImpactState,
) -> Result<Option<ImpactBranch>, ImpactError> {
    let actor = objects.get(owner).ok_or(ImpactError::MissingActor(owner))?;
    let peer = if actor.base.contacts.pending_hit || actor.base.contacts.previous_hit {
        Some(pair_peer(objects, contacts, owner, state)?)
    } else {
        let mode = surface_mode.ok_or(ImpactError::MissingSurfaceMode)?;
        if mode.flags & 0x07 != 0 && actor.base.position.y.wrapping_add(GROUND_CONTACT_MARGIN) >= 0
        {
            None
        } else {
            let result = collision_surface::query_object_surface(
                objects,
                owner,
                animation_clock,
                mode.search(),
            )
            .map_err(ImpactError::Surface)?;
            let actor = objects.get_mut(owner).expect("validated projectile");
            let group = actor.extension.surface_contact.group;
            actor.extension.surface_contact = result.contact;
            actor.extension.surface_contact.group = group;
            if result.contact.supporting_object.is_none()
                || actor.base.position.y.wrapping_sub(result.height) < 0
            {
                state.material = 0;
                return Ok(None);
            }
            let attributes_hit =
                actor.base.contacts.mutually_non_damaging && actor.base.contacts.credits_hit_side;
            let selected = actor.extension.path_state.conditions.selected_player;
            let supporting = result.contact.supporting_object.expect("checked surface");
            let support = objects.get_mut(supporting).expect("queried surface actor");
            if attributes_hit {
                match selected {
                    PlayerTarget::Primary => support.base.contacts.hit_by_primary = true,
                    PlayerTarget::Secondary => support.base.contacts.hit_by_secondary = true,
                }
            }
            if support.base.contacts.latch_new_contact {
                support.base.contacts.new_contact_latched = true;
                Some(supporting)
            } else {
                // This is an intentional source fallthrough, even when the
                // projectile's pending/previous flags were both clear.
                Some(pair_peer(objects, contacts, owner, state)?)
            }
        }
    };
    state.material = 0;
    let Some(peer) = peer else {
        return Ok(Some(ImpactBranch::First));
    };
    if players.contains(&Some(peer)) {
        return Ok(Some(player_branch(peer)));
    }
    let actor = objects.get(peer).ok_or(ImpactError::MissingActor(peer))?;
    if actor.base.hit_points == 0 {
        return Ok(Some(ImpactBranch::First));
    }
    let materials = actor.extension.auxiliary.impact_materials(resources, peer)
        .map_err(ImpactError::Auxiliary)?;
    if actor.base.contacts.suppress_contacts_next_epoch {
        state.material = materials.suppressed.unwrap_or(0);
        Ok(Some(ImpactBranch::Third))
    } else {
        state.material = materials.ordinary.unwrap_or(0);
        Ok(Some(ImpactBranch::Second))
    }
}
