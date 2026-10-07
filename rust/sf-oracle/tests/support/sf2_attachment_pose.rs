//! Shared original object-pose bridge; original code and expected poses are
//! never replaced. Addresses are confined to this verification-only helper.

use sf2_game::{ObjectId, ObjectStore};
use sf_oracle::SnesBus;

const WRAM: u32 = 0x7E0000;
pub fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| 0x03BD + id.index() as u16 * 0x3F)
}

pub fn seed(bus: &mut SnesBus, objects: &ObjectStore) {
    for (id, actor) in objects.active_objects() {
        let base = WRAM + u32::from(address(Some(id)));
        for (offset, value) in [
            (6, address(actor.base.attachment)),
            (
                0x29,
                address(actor.base.first_child.or(actor.base.next_sibling)),
            ),
            (0x1CD8, address(actor.extension.parent)),
        ] {
            bus.write16(base + offset, value);
        }
        for (offset, value) in [
            (12, actor.base.position.x),
            (14, actor.base.position.y),
            (16, actor.base.position.z),
            (0x1CCF, actor.extension.relative_position.x),
            (0x1CD1, actor.extension.relative_position.y),
            (0x1CD3, actor.extension.relative_position.z),
        ] {
            bus.write16(base + offset, value as u16);
        }
        for (offset, value) in [
            (0x12, actor.base.pitch),
            (0x14, actor.base.yaw),
            (0x16, actor.base.roll),
            (0x1CD5, actor.extension.relative_rotation.pitch),
            (0x1CD6, actor.extension.relative_rotation.yaw),
            (0x1CD7, actor.extension.relative_rotation.roll),
        ] {
            bus.write8(base + offset, value.units());
        }
    }
}

pub fn compare(bus: &SnesBus, objects: &ObjectStore) {
    for (id, actor) in objects.active_objects() {
        let base = WRAM + u32::from(address(Some(id)));
        for (offset, value) in [
            (6, address(actor.base.attachment)),
            (
                0x29,
                address(actor.base.first_child.or(actor.base.next_sibling)),
            ),
            (0x1CD8, address(actor.extension.parent)),
        ] {
            assert_eq!(
                bus.read16(base + offset),
                value,
                "actor {} link {offset:X}",
                id.index()
            );
        }
        for (offset, value) in [
            (12, actor.base.position.x),
            (14, actor.base.position.y),
            (16, actor.base.position.z),
            (0x1CCF, actor.extension.relative_position.x),
            (0x1CD1, actor.extension.relative_position.y),
            (0x1CD3, actor.extension.relative_position.z),
        ] {
            assert_eq!(
                bus.read16(base + offset) as i16,
                value,
                "actor {} field {offset:X}",
                id.index()
            );
        }
        for (offset, value) in [
            (0x12, actor.base.pitch),
            (0x14, actor.base.yaw),
            (0x16, actor.base.roll),
            (0x1CD5, actor.extension.relative_rotation.pitch),
            (0x1CD6, actor.extension.relative_rotation.yaw),
            (0x1CD7, actor.extension.relative_rotation.roll),
        ] {
            assert_eq!(
                bus.read8(base + offset),
                value.units(),
                "actor {} byte {offset:X}",
                id.index()
            );
        }
    }
}
