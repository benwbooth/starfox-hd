//! Unmodified complete player publication branch and continuous effects tail.

use super::surface_particle_tests::{Native, OWNER, SLOT};
use super::{attachment_poses as poses, frame_effects_tests as effects, rom, Source, WRAM};
use sf2_game::{
    path_relationships, player_attachments, player_frame_effects, Angle, Rotation, Vector3,
};

pub(super) fn prepare(native: &mut Native, mode: u8, ignored: bool, value: u16) {
    let ids: Vec<_> = native
        .objects
        .active_ids()
        .iter()
        .copied()
        .filter(|&id| id != native.owner)
        .collect();
    for (index, &id) in ids.iter().enumerate() {
        path_relationships::attach_fresh_child(
            &mut native.objects,
            native.owner,
            id,
            71 + index as u8,
        )
        .unwrap();
        let child = native.objects.get_mut(id).unwrap();
        child.extension.relative_position = Vector3 {
            x: value as i16,
            y: value.rotate_left(5) as i16,
            z: value.wrapping_neg() as i16,
        };
        child.extension.relative_rotation = Rotation {
            pitch: Angle::from_units(value as u8),
            yaw: Angle::from_units((value >> 8) as u8),
            roll: Angle::from_units(197),
        };
        child.extension.parent = match (index + value as usize) % 4 {
            0 => Some(id),
            1 => Some(ids[0]),
            _ => None,
        };
    }
    let record = native
        .world
        .player_mut(&native.objects, native.owner)
        .unwrap();
    record.contact.as_mut().unwrap().ignores_contacts = ignored;
    record.auxiliary.as_mut().unwrap().mode = mode;
    record.camera_ground = Some(sf2_game::player_camera_ground::PlayerCameraGround {
        carried_target_height: value.rotate_left(3) as i16,
        ..Default::default()
    });
    record.motion.as_mut().unwrap().walker_attachment_yaw = value as u8;
}

pub(super) fn seed(source: &mut Source, native: &Native) {
    poses::seed(&mut source.bus, &native.objects);
    for (id, actor) in native.objects.active_objects() {
        let flags = u32::from(poses::address(Some(id))) + 0x23;
        source.bus.write8(
            flags,
            (source.bus.read8(flags) & !0x10)
                | if actor.extension.path_state.motion.refresh_child_chain {
                    0x10
                } else {
                    0
                },
        );
    }
    let record = native.world.player(&native.objects, native.owner).unwrap();
    source.bus.write8(
        WRAM + SLOT + 0x6A72,
        0xEF | if record.contact.unwrap().ignores_contacts {
            0x10
        } else {
            0
        },
    );
    source
        .bus
        .write8(WRAM + SLOT + 0x6AA0, record.auxiliary.unwrap().mode);
    source.bus.write16(
        WRAM + SLOT + 0x6BF3,
        record.camera_ground.unwrap().carried_target_height as u16,
    );
    source.bus.write8(
        WRAM + SLOT + 0x6AEE,
        record.motion.unwrap().walker_attachment_yaw,
    );
}

#[test]
fn original_attachment_modes_contact_gate_all_yaw_offsets_and_height_extremes() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    for mode in 0..=255u8 {
        for variant in 0..8u16 {
            let value = u16::from(mode)
                .wrapping_mul(257)
                .wrapping_add(variant * 7193);
            let mut native = effects::fixture(&mut source, 4, value);
            prepare(&mut native, mode, variant & 1 != 0, value);
            seed(&mut source, &native);
            let before = native
                .world
                .player(&native.objects, native.owner)
                .copied()
                .unwrap();
            let owner_before = native.objects.get(native.owner).unwrap().clone();
            source.run_with_y(0x069F54, Some(0x069FAD), 0, OWNER, true, Some(SLOT as u16));
            player_attachments::publish(&mut native.objects, &native.world, native.owner).unwrap();
            poses::compare(&source.bus, &native.objects);
            assert_eq!(native.objects.get(native.owner).unwrap(), &owner_before);
            assert_eq!(
                *native.world.player(&native.objects, native.owner).unwrap(),
                before
            );
        }
    }
    for yaw in 0..=255u16 {
        for height in [i16::MIN, -1, 0, 1, i16::MAX] {
            let mut native = effects::fixture(&mut source, 4, yaw * 257);
            prepare(&mut native, 0x2F, false, yaw);
            native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap()
                .camera_ground
                .as_mut()
                .unwrap()
                .carried_target_height = height;
            seed(&mut source, &native);
            source.run_with_y(0x069F54, Some(0x069FAD), 0, OWNER, true, Some(SLOT as u16));
            player_attachments::publish(&mut native.objects, &native.world, native.owner).unwrap();
            poses::compare(&source.bus, &native.objects);
        }
    }
}

#[test]
fn original_full_effects_recovery_and_new_children_publish_in_one_continuous_visit() {
    let mut source = Source::new(&rom(), 0);
    source.bus.enable_gsu();
    for mode in 0..=255u8 {
        for variant in 0..8u16 {
            let value = u16::from(mode)
                .wrapping_mul(257)
                .wrapping_add(variant * 7193);
            let mut native = effects::fixture(&mut source, 4, value);
            prepare(&mut native, mode, variant & 1 != 0, value);
            let record = native
                .world
                .player_mut(&native.objects, native.owner)
                .unwrap();
            record.particles.as_mut().unwrap().flags = [0, 0x20, 0x60, 0x80][variant as usize % 4];
            record.particles.as_mut().unwrap().age = if variant & 2 == 0 { 24 } else { 128 };
            record.contact.as_mut().unwrap().hit.reserve_shield = mode % 80;
            native.world.shield_recovery.as_mut().unwrap().amount =
                if variant & 4 == 0 { 0 } else { 5 };
            native.world.action_gate.as_mut().unwrap().code = (variant & 2) as u8;
            effects::seed(&mut source, &mut native, 79);
            seed(&mut source, &native);
            let before = *native.world.player(&native.objects, native.owner).unwrap();
            source.run_with_y(0x069EE8, Some(0x069FAD), 0, OWNER, true, Some(SLOT as u16));
            player_frame_effects::advance_with_attachments(
                &mut native.objects,
                &mut native.world,
                native.owner,
                Some(79),
            )
            .unwrap();
            poses::compare(&source.bus, &native.objects);
            effects::compare(&source, &mut native, before);
        }
    }
}
