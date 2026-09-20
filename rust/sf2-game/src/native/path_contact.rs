//! Typed path contact controls. Class masks update every semantic use of
//! each source bit, including exclusion/attribution aliases (31:08/31:80).

use super::collision_pass::ExclusionGroups;
use super::Object;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContactClassMask {
    pub groups: ExclusionGroups,
    pub first_strategy_visit: bool,
    pub suppress_attack_damage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactCommand {
    RetainClass(ContactClassMask),
    IncludeClass(ContactClassMask),
    SuppressContactsNextEpoch(bool),
    SuppressHitMarker(bool),
    ShapeFootprintSearch(bool),
    MarkHit,
}

impl ContactCommand {
    pub fn apply(self, actor: &mut Object) {
        let contacts = &mut actor.base.contacts;
        match self {
            Self::ShapeFootprintSearch(enabled) => {
                actor.base.flags.exclude_from_shape_footprint_search = !enabled;
            }
            Self::MarkHit => contacts.hit_marked = true,
            Self::SuppressContactsNextEpoch(enabled) => {
                contacts.suppress_contacts_next_epoch = enabled;
            }
            Self::SuppressHitMarker(enabled) => contacts.suppress_hit_marker = enabled,
            Self::RetainClass(mask) => {
                contacts.exclusion_groups = contacts.exclusion_groups.intersection(mask.groups);
                contacts.first_strategy_visit &= mask.first_strategy_visit;
                contacts.suppress_attack_damage &= mask.suppress_attack_damage;
                contacts.credits_hit_side &= mask.groups.excludes(ExclusionGroups::HIT_SIDE_CLASS);
                contacts.mutually_non_damaging &= mask
                    .groups
                    .excludes(ExclusionGroups::MUTUALLY_NON_DAMAGING_CLASS);
            }
            Self::IncludeClass(mask) => {
                contacts.exclusion_groups = contacts.exclusion_groups.union(mask.groups);
                contacts.first_strategy_visit |= mask.first_strategy_visit;
                contacts.suppress_attack_damage |= mask.suppress_attack_damage;
                contacts.credits_hit_side |= mask.groups.excludes(ExclusionGroups::HIT_SIDE_CLASS);
                contacts.mutually_non_damaging |= mask
                    .groups
                    .excludes(ExclusionGroups::MUTUALLY_NON_DAMAGING_CLASS);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, ObjectKind, ShapeId};

    fn mask(value: u8) -> ContactClassMask {
        ContactClassMask {
            groups: ExclusionGroups::from_authored_class(value),
            first_strategy_visit: value & 4 != 0,
            suppress_attack_damage: value & 1 != 0,
        }
    }

    fn class(actor: &mut Object, value: u8) {
        actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(value);
        actor.base.contacts.first_strategy_visit = value & 4 != 0;
        actor.base.contacts.suppress_attack_damage = value & 1 != 0;
        actor.base.contacts.credits_hit_side = value & 8 != 0;
        actor.base.contacts.mutually_non_damaging = value & 0x80 != 0;
    }

    #[test]
    fn class_updates_preserve_all_aliases_and_other_actor_state_for_every_mask() {
        let mut original = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        original.base.contacts.skip_contacts = true;
        original.base.contacts.hit_marked = true;
        original.base.hit_flags = 0xA5;
        original.base.wait_timer = 57;
        for old in 0..=u8::MAX {
            class(&mut original, old);
            for bits in 0..=u8::MAX {
                for (command, result) in [
                    (ContactCommand::RetainClass(mask(bits)), old & bits),
                    (ContactCommand::IncludeClass(mask(bits)), old | bits),
                ] {
                    let mut actual = original.clone();
                    let mut expected = original.clone();
                    class(&mut expected, result);
                    command.apply(&mut actual);
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn footprint_search_only_changes_candidate_admission_not_damage_collision_or_visibility() {
        for enabled in [false, true] {
            for bits in 0..128 {
                let mut actual = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
                actual.base.flags.exclude_from_shape_footprint_search = bits & 1 != 0;
                actual.base.flags.visible = bits & 2 != 0;
                actual.base.flags.collision_disabled = bits & 4 != 0;
                actual.base.flags.collided = bits & 8 != 0;
                actual.base.contacts.first_strategy_visit = bits & 16 != 0;
                actual.base.contacts.suppress_hit_marker = bits & 32 != 0;
                actual.base.contacts.skip_contacts = bits & 64 != 0;
                actual.base.wait_timer = 253;
                let mut expected = actual.clone();
                expected.base.flags.exclude_from_shape_footprint_search = !enabled;
                ContactCommand::ShapeFootprintSearch(enabled).apply(&mut actual);
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn footprint_command_controls_the_real_surface_query_without_changing_damage_collision() {
        use super::super::{collision_surface::{query_object_surface, SurfaceSearch}, ObjectStore};
        const FULL_SEARCH_MISS_HEIGHT: i16 = 16_384;
        let mut objects = ObjectStore::new();
        let mut probe = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
        probe.base.position.y = -100;
        let owner = objects.allocate(probe).unwrap();
        let mut scenery = Object::new(ObjectKind::Enemy, ShapeId::from_catalog_index(7), Behavior::FollowPath);
        scenery.base.contacts.first_strategy_visit = false;
        scenery.base.flags.visible = false;
        scenery.base.flags.collision_disabled = true;
        let candidate = objects.allocate(scenery).unwrap();
        for enabled in [false, true, false, true] {
            ContactCommand::ShapeFootprintSearch(enabled).apply(objects.get_mut(candidate).unwrap());
            let result = query_object_surface(&objects, owner, 0, SurfaceSearch::Full).unwrap();
            assert_eq!(result.contact.supporting_object, enabled.then_some(candidate));
            assert_eq!(result.height, if enabled { -16 } else { FULL_SEARCH_MISS_HEIGHT });
            assert!(!objects.get(candidate).unwrap().base.flags.visible);
            assert!(objects.get(candidate).unwrap().base.flags.collision_disabled);
        }
    }

    #[test]
    fn next_epoch_suppression_and_hit_marker_control_do_not_change_current_contact_state() {
        for current in [false, true] {
            for enabled in [false, true] {
                let mut actual =
                    Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
                actual.base.contacts.skip_contacts = current;
                actual.base.contacts.hit_marked = current;
                let mut expected = actual.clone();
                expected.base.contacts.suppress_contacts_next_epoch = enabled;
                ContactCommand::SuppressContactsNextEpoch(enabled).apply(&mut actual);
                assert_eq!(actual, expected);
                expected.base.contacts.suppress_hit_marker = enabled;
                ContactCommand::SuppressHitMarker(enabled).apply(&mut actual);
                assert_eq!(actual, expected);
                expected.base.contacts.hit_marked = true;
                ContactCommand::MarkHit.apply(&mut actual);
                assert_eq!(actual, expected);
            }
        }
    }
}
