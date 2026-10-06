//! Ordered target scan (`$7F:1FEB..20FD`). Eligibility belongs to the source
//! search/contact flags, not the actor's high-level kind or visible lifetime.

use super::collision_pass::ExclusionGroups;
use super::{ObjectId, ObjectStore};
use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

const SHARED_TARGET_EXCLUSION: ExclusionGroups = ExclusionGroups::from_authored_class(0x20);
const FINE_TO_COARSE_SHIFT: u32 = u8::BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimWindow {
    pub minimum_distance: i16,
    /// Exclusive initial bound; each accepted candidate replaces it.
    pub maximum_distance: i16,
    pub yaw_half_width: u8,
    pub pitch_half_width: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSearchError {
    MissingActor(ObjectId),
}

fn angle_in_window(heading: u8, angle: u8, half_width: u8) -> bool {
    let centered = half_width.wrapping_add(heading).wrapping_add(angle);
    // The source halves with sign extension, but compares the resulting
    // BYTE as unsigned. Negative odd values round down, not toward zero.
    ((centered as i8 >> 1) as u8) < half_width
}

pub fn nearest(
    objects: &ObjectStore,
    owner: ObjectId,
    window: AimWindow,
) -> Result<Option<ObjectId>, TargetSearchError> {
    let source = &objects
        .get(owner)
        .ok_or(TargetSearchError::MissingActor(owner))?
        .base;
    let mut best_distance = window.maximum_distance;
    let mut selected = None;
    for &id in objects.active_ids() {
        if id == owner {
            continue;
        }
        let candidate = &objects
            .get(id)
            .ok_or(TargetSearchError::MissingActor(id))?
            .base;
        if !candidate.flags.general_search_eligible {
            continue;
        }
        let dx = candidate.position.x.wrapping_sub(source.position.x);
        let dz = candidate.position.z.wrapping_sub(source.position.z);
        let distance = sf2_xz_angle_distance(dx, dz);
        if distance.wrapping_sub(best_distance) >= 0
            || distance.wrapping_sub(window.minimum_distance) < 0
        {
            continue;
        }
        let yaw = (sf2_atan16(dx, dz) >> FINE_TO_COARSE_SHIFT) as u8;
        if !angle_in_window(source.yaw.units(), yaw, window.yaw_half_width) {
            continue;
        }
        let dy = source.position.y.wrapping_sub(candidate.position.y);
        let pitch = (sf2_atan16(dy, distance) >> FINE_TO_COARSE_SHIFT) as u8;
        if !angle_in_window(source.pitch.units(), pitch, window.pitch_half_width) {
            continue;
        }
        let contacts = candidate.contacts;
        if (source
            .contacts
            .exclusion_groups
            .excludes(SHARED_TARGET_EXCLUSION)
            && contacts.exclusion_groups.excludes(SHARED_TARGET_EXCLUSION))
            || contacts.weapon_formatted
            || !contacts
                .exclusion_groups
                .excludes(ExclusionGroups::PATH_SPAWN)
            || contacts.mutually_non_damaging
            || contacts.credits_hit_side
            || candidate.flags.collision_disabled
            || contacts.suppress_contacts_next_epoch
        {
            continue;
        }
        best_distance = distance;
        selected = Some(id);
    }
    Ok(selected)
}

#[cfg(test)]
#[path = "target_search_tests.rs"]
mod tests;
