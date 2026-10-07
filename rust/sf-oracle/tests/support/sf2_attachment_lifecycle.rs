//! The original attachment service owns one linear chain, not a tree.
//! Exercise mixed parents through original append, search, death and cleanup.

use super::surface_particle_tests::{address, Native};
use super::{rom, Source, WRAM};
use sf2_game::{common_destruction, path_death, path_relationships, ObjectId};
use sf_oracle::{call, call_near, Entry};

fn source(rom: &[u8]) -> Source {
    let mut source = Source::new(rom, 0);
    for (index, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + index as u32, byte);
    }
    source
}

fn compare(source: &Source, native: &Native) {
    native.compare_pool(source);
    for (id, actor) in native.objects.active_objects() {
        let base = u32::from(address(Some(id)));
        for (offset, expected) in [
            (0x1C, address(actor.base.linked_object)),
            (0x1CD8, address(actor.extension.parent)),
        ] {
            assert_eq!(
                source.bus.read16(WRAM + base + offset),
                expected,
                "actor {} link {offset:X}",
                id.index()
            );
        }
        for (offset, expected) in [
            (0x13, actor.base.child_number),
            (0x2D, actor.base.hit_points),
            (0x21, 0x80 | u8::from(actor.base.flags.collision_disabled)),
            (
                0x23,
                0x80 | u8::from(actor.extension.path_state.motion.attached_coordinates) * 4
                    | u8::from(actor.extension.path_state.motion.refresh_child_chain) * 16,
            ),
            (
                0x25,
                0x80 | u8::from(actor.base.flags.remove_with_parent)
                    | u8::from(actor.base.flags.remove_after_tick) * 8,
            ),
        ] {
            assert_eq!(
                source.bus.read8(base + offset),
                expected,
                "actor {} byte {offset:X}",
                id.index()
            );
        }
    }
}

fn mixed_chain(
    source: &mut Source,
    first_number: u8,
    lifetime_mask: u8,
) -> (Native, Vec<ObjectId>) {
    let mut native = Native::new(source, 6, 91, 231, 0xA71B);
    let mut ids = native.objects.active_ids().to_vec();
    ids.reverse();
    for (index, &id) in ids.iter().enumerate() {
        let base = u32::from(address(Some(id)));
        let actor = native.objects.get_mut(id).unwrap();
        actor.base.hit_points = 77;
        actor.base.linked_object = Some(ids[(index + 2) % ids.len()]);
        actor.extension.parent = Some(ids[(index + 1) % ids.len()]);
        source.bus.write8(base + 0x2D, 77);
        source.bus.write8(base + 0x21, 0x80);
        source.bus.write8(base + 0x23, 0x80);
        source.bus.write8(base + 0x25, 0x80);
        source
            .bus
            .write16(base + 0x1C, address(actor.base.linked_object));
        source
            .bus
            .write16(WRAM + base + 0x1CD8, address(actor.extension.parent));
    }
    // Appending to a nested owner still walks through its existing siblings.
    // Parent roles: root -> craft -> shield; root -> trail; shield -> nested;
    // root -> later trail. Their one chain stays in allocation order.
    for (child_index, parent_index) in [(1, 0), (2, 1), (3, 0), (4, 2), (5, 0)] {
        let child = ids[child_index];
        let parent = ids[parent_index];
        let number = first_number.wrapping_add(child_index as u8);
        assert!(
            call(
                &mut source.bus,
                0x7F2A3D,
                &Entry {
                    a: u16::from(number),
                    x: address(Some(parent)),
                    y: address(Some(child)),
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                }
            )
            .returned
        );
        path_relationships::attach_fresh_child(&mut native.objects, parent, child, number).unwrap();
        compare(source, &native);
    }
    for (index, &id) in ids.iter().enumerate() {
        let owned = lifetime_mask & (1 << index) != 0;
        native
            .objects
            .get_mut(id)
            .unwrap()
            .base
            .flags
            .remove_with_parent = owned;
        source
            .bus
            .write8(u32::from(address(Some(id))) + 0x25, 0x80 | u8::from(owned));
    }
    compare(source, &native);
    (native, ids)
}

#[test]
fn original_mixed_attachment_append_and_full_byte_lookup_use_one_linear_suffix() {
    let mut source = source(&rom());
    for start in [0, 1, 12, 77, 250, 255] {
        let (native, ids) = mixed_chain(&mut source, start, 0x3F);
        for owner in ids {
            for number in 0..=u8::MAX {
                let output = call(
                    &mut source.bus,
                    0x7F2A7B,
                    &Entry {
                        a: u16::from(number),
                        x: address(Some(owner)),
                        dbr: 0x7E,
                        p: 0x20,
                        ..Default::default()
                    },
                );
                assert!(output.returned);
                assert_eq!(
                    output.y,
                    address(
                        path_relationships::find_direct_child(&native.objects, owner, number)
                            .unwrap()
                    )
                );
            }
        }
        compare(&source, &native);
    }
}

#[test]
fn original_mixed_attachment_retirement_splices_before_detach_and_retains_relative_frames() {
    let mut source = source(&rom());
    for owner_index in 0..6 {
        for mask in 0..64 {
            for disable_owner_gate in [false, true] {
                let (mut native, ids) = mixed_chain(&mut source, 250, mask);
                let owner = ids[owner_index];
                if disable_owner_gate {
                    native
                        .objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion
                        .refresh_child_chain = false;
                    let base = u32::from(address(Some(owner)));
                    source
                        .bus
                        .write8(base + 0x23, source.bus.read8(base + 0x23) & !0x10);
                }
                assert!(
                    call_near(
                        &mut source.bus,
                        0x7F335A,
                        &Entry {
                            x: address(Some(owner)),
                            dbr: 0x7E,
                            p: 0x20,
                            ..Default::default()
                        }
                    )
                    .returned
                );
                native.objects.remove(owner).unwrap();
                compare(&source, &native);
                assert_eq!(source.bus.read16(0x12AA), address(Some(owner)));
            }
        }
    }
}

#[test]
fn original_mixed_attachment_death_services_visit_nested_and_sibling_links_identically() {
    let mut source = source(&rom());
    for owner_index in 0..6 {
        for mask in 0..64 {
            for mark_only in [false, true] {
                let (mut native, ids) = mixed_chain(&mut source, 73, mask);
                let owner = ids[owner_index];
                if mark_only {
                    source.run(0x7F8B94, Some(0x7F9E70), 0, address(Some(owner)), true);
                    path_death::mark_for_death(&mut native.objects, owner, None).unwrap();
                } else {
                    source.run(0x7F2AA4, None, 0, address(Some(owner)), true);
                    common_destruction::detach_dying_children(&mut native.objects, owner).unwrap();
                }
                compare(&source, &native);
            }
        }
    }
}

#[test]
fn original_mixed_attachment_self_unlink_splices_nested_suffix_without_changing_relative_frames() {
    let mut source = source(&rom());
    for owner_index in 0..6 {
        for mask in 0..64 {
            let (mut native, ids) = mixed_chain(&mut source, 255, mask);
            let owner = ids[owner_index];
            source.run(0x7F9435, Some(0x7F7E75), 0, address(Some(owner)), true);
            path_relationships::apply(
                &mut native.objects,
                owner,
                path_relationships::RelationshipCommand::UnlinkSelf,
            )
            .unwrap();
            compare(&source, &native);
        }
    }
}
