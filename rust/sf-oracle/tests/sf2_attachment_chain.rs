//! Complete original attachment-chain publication, including both children
//! created by the real numbered-child relationship service.

#[path = "support/sf2_attachment_pose.rs"]
mod poses;

use sf2_game::{
    attachments, path_relationships, Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore,
    ShapeId, Vector3,
};
use sf_oracle::{call, Entry, SnesBus};

const OWNER: u16 = 0x03BD;

#[test]
fn original_chain_refreshes_every_numbered_sibling_in_order() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = SnesBus::new(rom.clone());
    for (index, &byte) in rom[0x10000..0x17E00].iter().enumerate() {
        source.write8(0x7F0000 + index as u32, byte);
    }
    source.enable_gsu();
    let mut objects = ObjectStore::new();
    let mut allocate = || {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap()
    };
    let owner = allocate();
    let first = allocate();
    let second = allocate();
    path_relationships::attach_fresh_child(&mut objects, owner, first, 18).unwrap();
    path_relationships::attach_fresh_child(&mut objects, owner, second, 24).unwrap();
    let parent = objects.get_mut(owner).unwrap();
    parent.base.position = Vector3 {
        x: -31851,
        y: 22581,
        z: 14321,
    };
    parent.base.pitch = Angle::from_units(123);
    parent.base.yaw = Angle::from_units(231);
    parent.base.roll = Angle::from_units(97);
    for (child, offset) in [
        (
            first,
            Vector3 {
                x: 100,
                y: -200,
                z: 300,
            },
        ),
        (
            second,
            Vector3 {
                x: -31777,
                y: 22301,
                z: 9971,
            },
        ),
    ] {
        objects.get_mut(child).unwrap().extension.relative_position = offset;
    }
    poses::seed(&mut source, &objects);
    assert!(
        call(
            &mut source,
            0x7F2319,
            &Entry {
                x: OWNER,
                dbr: 0x7E,
                p: 0x20,
                ..Default::default()
            }
        )
        .returned
    );
    attachments::refresh_child_chain(&mut objects, owner).unwrap();
    poses::compare(&source, &objects);
}

#[test]
fn original_chain_handles_full_pool_linear_parents_and_self_reference_without_skipping_next() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = SnesBus::new(rom.clone());
    for (index, &byte) in rom[0x10000..0x17E00].iter().enumerate() {
        source.write8(0x7F0000 + index as u32, byte);
    }
    source.enable_gsu();
    for value in 0..=255u16 {
        let mut objects = ObjectStore::new();
        let ids: Vec<ObjectId> = (0..60)
            .map(|_| {
                objects
                    .allocate(Object::new(
                        ObjectKind::Effect,
                        ShapeId::EMPTY,
                        Behavior::Unassigned,
                    ))
                    .unwrap()
            })
            .collect();
        for (index, &id) in ids.iter().enumerate() {
            let next = ids.get(index + 1).copied();
            let parent = ids[index.saturating_sub(1)];
            let actor = objects.get_mut(id).unwrap();
            actor.base.position = Vector3 {
                x: (value * 127) as i16,
                y: (value * 231) as i16,
                z: -(index as i16 * 379),
            };
            actor.base.pitch = Angle::from_units(value as u8);
            actor.base.yaw = Angle::from_units((value as u8).wrapping_add(index as u8));
            actor.base.roll = Angle::from_units((value as u8).wrapping_neg());
            actor.base.attachment_next = next;
            actor.base.attachment = Some(parent);
            actor.extension.parent = match (index + value as usize) % 4 {
                0 => Some(id),
                1 => Some(ids[0]),
                _ => None,
            };
            actor.extension.relative_position = Vector3 {
                x: (value * 251) as i16,
                y: (value * 193) as i16,
                z: (index as i16).wrapping_mul(557),
            };
            actor.extension.relative_rotation = sf2_game::Rotation {
                pitch: Angle::from_units(value as u8),
                yaw: Angle::from_units(255),
                roll: Angle::from_units(index as u8),
            };
        }
        poses::seed(&mut source, &objects);
        assert!(
            call(
                &mut source,
                0x7F2319,
                &Entry {
                    x: OWNER,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                }
            )
            .returned
        );
        attachments::refresh_child_chain(&mut objects, ids[0]).unwrap();
        poses::compare(&source, &objects);
    }
}
