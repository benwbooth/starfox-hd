//! Original chain construction and the source follower transform.

use sf2_game::intro_chain::{
    OpeningChainAllocationContext, OpeningChainControls, OpeningChainFamily, OpeningChainPart,
    OpeningChainPhase,
};
use sf2_game::intro_motion::{follow_intro_predecessor, IntroScenePose};
use sf2_game::object::{
    active_objects, allocate, CURRENT_OBJECT, FIELD_PATH, FIELD_SHAPE, FIELD_STRATEGY, PLAYER_ONE,
    SELECTED_OBJECT,
};
use sf2_game::oracle_compat::Game;
use sf2_game::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, Rotation, ShapeId,
    Vector3,
};

const STRATEGY: u32 = 0x7F7E1E;

fn retail() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("chain verification requires the user-owned retail SF2 ROM")
}

fn native_ids() -> (ObjectId, [ObjectId; 9]) {
    let mut objects = ObjectStore::new();
    let mut next = || {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::from_catalog_index(340),
                Behavior::Effect,
            ))
            .unwrap()
    };
    (next(), std::array::from_fn(|_| next()))
}

fn free_slots(exact: &Game) -> usize {
    let mut next = exact.memory.read_word(sf2_game::object::FREE_LIST);
    let mut count = 0;
    while next != 0 {
        count += 1;
        next = exact.memory.read_word(next);
    }
    count
}

fn controls(scenario: u8, update: u8) -> OpeningChainControls {
    let sequence = update
        .wrapping_mul(13)
        .wrapping_add(scenario.wrapping_mul(7));
    OpeningChainControls {
        suppress_initial_contact: (scenario & 1 != 0) ^ (update >= 3),
        sort_override_on_reveal: (scenario & 2 != 0) ^ (update >= 7),
        depart: scenario != 5 && update >= [32, 43, 17, 1, 26][usize::from(scenario.min(4))],
        raise_depth_offset: sequence & 1 != 0,
        settle_pitch: sequence & 2 != 0,
        bank_by_part: sequence & 4 != 0,
        level_pitch: sequence & 8 != 0,
    }
}

fn write_controls(exact: &mut Game, controls: OpeningChainControls) {
    let mut value = 0;
    for (enabled, mask) in [
        (controls.suppress_initial_contact, 2),
        (controls.sort_override_on_reveal, 128),
        (controls.depart, 64),
        (controls.raise_depth_offset, 1024),
        (controls.settle_pitch, 8),
        (controls.bank_by_part, 4),
        (controls.level_pitch, 256),
    ] {
        if enabled {
            value |= mask;
        }
    }
    exact.memory.write_word(0xD77D, value);
}

fn family_parent_pose(update: u8, varied: bool) -> IntroScenePose {
    let frame = i16::from(update);
    pose(
        Vector3 {
            x: if varied {
                32000_i16.wrapping_add(frame * 53)
            } else {
                -1700
            },
            y: if varied {
                (-32500_i16).wrapping_sub(frame * 29)
            } else {
                530
            },
            z: if varied {
                frame.wrapping_mul(839).wrapping_sub(24000)
            } else {
                3200
            },
        },
        if varied { update.wrapping_mul(19) } else { 31 },
        if varied { update.wrapping_mul(31) } else { 190 },
        if varied { update.wrapping_mul(47) } else { 227 },
    )
}

#[test]
fn native_chain_family_matches_original_controls_departure_and_burst_cleanup() {
    let rom = retail();
    for scenario in 0..6 {
        for varied in [false, true] {
            for seed in [[0; 4], [1, 2, 3, 4], [255, 0, 127, 128]] {
                for (capacity, trailing_actor) in [
                    (None, false),
                    (Some(0), false),
                    (Some(1), false),
                    (Some(1), true),
                ] {
                    if capacity.is_some() && (scenario != 3 || varied || seed != [0; 4]) {
                        continue;
                    }
                    let mut exact = Game::new(rom.clone()).unwrap();
                    let parent = allocate(&mut exact.memory, 0).unwrap();
                    install_strategy(&mut exact, parent, 0xFDC2);
                    exact.memory.write_word(parent + FIELD_SHAPE, 0xE194);
                    exact.memory.write_byte(parent + 0x2E, 16);
                    exact.memory.write_word(PLAYER_ONE, 0x033F);
                    exact.memory.write_word(SELECTED_OBJECT, 0x033F);
                    exact.memory.write_word(0x033F + FIELD_PATH, 0x0140);
                    exact.memory.write_word(CURRENT_OBJECT, parent);
                    exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
                    exact.memory.write_word(parent + FIELD_PATH, 0xFF43);
                    exact.memory.write_byte(parent + 24, 0);
                    for field in [50, 52, 54] {
                        exact.memory.write_word(parent + field, 0);
                    }
                    for (index, byte) in seed.into_iter().enumerate() {
                        exact.memory.write_byte(0xE0 + index as u16, byte);
                    }
                    let (parent_id, ids) = native_ids();
                    let mut native = OpeningChainFamily::new(parent_id, ids);
                    let mut random = RandomState::new(seed);
                    let mut source_ids = Vec::new();
                    let mut retirements = 0;
                    let mut saw_capacity_error = false;
                    let mut pressure_updates = 0;
                    for update in 0..80 {
                        let parent_pose = family_parent_pose(update, varied);
                        write_pose(&mut exact, parent, parent_pose);
                        let signals = controls(scenario, update);
                        write_controls(&mut exact, signals);
                        native.publish_from_parent(parent_id, parent_pose);
                        if update == 2 {
                            if let Some(remaining) = capacity {
                                // Other scene occupants keep their pool slots but
                                // stay outside this isolated family's traversal.
                                let next = exact.memory.read_word(parent);
                                while free_slots(&exact) > remaining {
                                    allocate(&mut exact.memory, parent).unwrap();
                                }
                                exact.memory.write_word(parent, next);
                                exact.memory.write_word(next + 2, parent);
                            }
                        }
                        let available = free_slots(&exact);
                        // A zero-health actor only reaches its path when the
                        // engine grants the one-update strategy bypass. Exercise
                        // persistent callbacks, and cancellation by departure.
                        let inject_zero = update > 0
                            && native.segments()[0].phase() == OpeningChainPhase::Following
                            && if scenario == 2 {
                                update == 17
                            } else {
                                [5, 6, 32, 43].contains(&update)
                            };
                        if inject_zero {
                            let actor = source_ids[0];
                            exact.memory.write_byte(actor + 0x2D, 0);
                            let flags = exact.memory.read_byte(actor + 0x31) | 4;
                            exact.memory.write_byte(actor + 0x31, flags);
                            native.set_health_at_strategy_entry(OpeningChainPart::First, 0);
                        }
                        exact.memory.write_word(CURRENT_OBJECT, parent);
                        exact.run_retail_oracle_routine(0x7F34E7, parent).unwrap();
                        let source_update = exact.run_retail_oracle_routine(0x7F354A, parent);
                        let allocation = OpeningChainAllocationContext {
                            available_slots: available,
                            oldest_burst_at_list_tail: !trailing_actor,
                        };
                        if let Err(error) = source_update {
                            assert_eq!(capacity, Some(0));
                            assert!(format!("{error:?}").contains("$00:80"), "{error:?}");
                            assert_eq!(
                                exact.memory.read_word(0x192C),
                                0x03E0,
                                "object-pool diagnostic"
                            );
                            let saved = native.clone();
                            let saved_random = random;
                            let error = native
                                .tick(parent_pose, signals, &mut random, allocation)
                                .unwrap_err();
                            assert_eq!(error.required_slots, 1);
                            assert_eq!(error.available_slots, 0);
                            assert_eq!(native, saved);
                            assert_eq!(random, saved_random);
                            saw_capacity_error = true;
                            break;
                        }
                        if update == 0 {
                            source_ids = active_objects(&exact.memory)
                                .into_iter()
                                .filter(|id| *id != parent)
                                .collect();
                            assert_eq!(source_ids.len(), 9);
                            if trailing_actor {
                                let sentinel = allocate(&mut exact.memory, source_ids[8]).unwrap();
                                exact.memory.write_byte(sentinel + 0x2D, 1);
                                exact.memory.write_word(sentinel + FIELD_STRATEGY, 0);
                            }
                        }
                        let events = native
                            .tick(parent_pose, signals, &mut random, allocation)
                            .unwrap();
                        let retired = &events.retired_segments;
                        pressure_updates += usize::from(events.allocation_pressure);
                        assert_eq!(events.spawned_bursts, retired.len());
                        for identity in retired {
                            let index = ids.iter().position(|id| id == identity).unwrap();
                            let segment = &native.segments()[index];
                            let actor = source_ids[index];
                            assert_eq!(segment.pose, read_pose(&exact, actor));
                            assert_eq!(exact.memory.read_byte(actor + 0x25) & 8, 8);
                            assert_eq!(
                                segment.local_offset,
                                Vector3 {
                                    x: exact.memory.read_word(actor + 0x1CCF) as i16,
                                    y: exact.memory.read_word(actor + 0x1CD1) as i16,
                                    z: exact.memory.read_word(actor + 0x1CD3) as i16,
                                }
                            );
                        }
                        retirements += retired.len();
                        // End requests retirement; the real cleanup pass reclaims
                        // those actors after the complete update traversal.
                        exact.run_retail_oracle_routine(0x7F402D, parent).unwrap();
                        assert_eq!(native.initialized_count(), 9);
                        assert_eq!(
                            random.bytes(),
                            std::array::from_fn(|i| exact.memory.read_byte(0xE0 + i as u16)),
                            "random scenario={scenario} varied={varied} update={update}"
                        );
                        let active = active_objects(&exact.memory);
                        let mut live_segments = 0;
                        for (index, segment) in native.segments().iter().enumerate() {
                            if segment.phase() == OpeningChainPhase::Finished {
                                continue;
                            }
                            live_segments += 1;
                            let actor = source_ids[index];
                            assert!(active.contains(&actor));
                            assert_eq!(segment.pose, read_pose(&exact, actor), "pose scenario={scenario} varied={varied} update={update} part={index}");
                            assert_eq!(
                                segment.local_offset,
                                Vector3 {
                                    x: exact.memory.read_word(actor + 0x1CCF) as i16,
                                    y: exact.memory.read_word(actor + 0x1CD1) as i16,
                                    z: exact.memory.read_word(actor + 0x1CD3) as i16,
                                },
                                "local update={update} part={index}"
                            );
                            assert_eq!(
                                segment.velocity,
                                Vector3 {
                                    x: exact.memory.read_word(actor + 50) as i16,
                                    y: exact.memory.read_word(actor + 52) as i16,
                                    z: exact.memory.read_word(actor + 54) as i16,
                                }
                            );
                            assert_eq!(segment.health(), exact.memory.read_byte(actor + 0x2D));
                            assert_eq!(segment.contact_disabled(), exact.memory.read_byte(actor + 0x21) & 1 != 0, "contact scenario={scenario} varied={varied} update={update} part={index} controls={signals:?}");
                            assert_eq!(
                                segment.suppresses_peer_contacts(),
                                exact.memory.read_byte(actor + 0x31) & 32 != 0
                            );
                            assert_eq!(
                                segment.sort_override(),
                                exact.memory.read_byte(actor + 9) & 1 != 0
                            );
                            assert_eq!(
                                segment.trail_style().unwrap_or(0),
                                exact.memory.read_byte(actor + 0x1CEE)
                            );
                            assert_eq!(
                                segment.depth_offset,
                                exact.memory.read_word(actor + 0x1CC8)
                            );
                            assert_eq!(
                                segment.is_visible(),
                                exact.memory.read_byte(actor + 0x23) & 2 == 0
                            );
                            let trigger = exact.memory.read_word(actor + 0x1CE0);
                            let armed =
                                trigger != 0 && exact.memory.read_byte(trigger + 0x6A61) != 0;
                            assert_eq!(segment.health_response_armed(), armed);
                            if armed {
                                assert_eq!(exact.memory.read_word(trigger + 0x6A62), 0xF8E5);
                                assert_eq!(exact.memory.read_byte(trigger + 0x6A64), 12);
                                assert_eq!(exact.memory.read_byte(trigger + 0x6A65), 0);
                            }
                            assert_eq!(
                                segment.shape().catalog_index(),
                                usize::from(
                                    (exact.memory.read_word(actor + FIELD_SHAPE) - 0xBC9C) / 28
                                )
                            );
                            assert_eq!(segment.parent(), parent_id);
                            assert_eq!(
                                segment.predecessor(),
                                if index == 0 {
                                    parent_id
                                } else {
                                    ids[index - 1]
                                }
                            );
                            assert_eq!(exact.memory.read_word(actor + 6), parent);
                            assert_eq!(
                                exact.memory.read_word(actor + 0x1C),
                                if index == 0 {
                                    parent
                                } else {
                                    source_ids[index - 1]
                                }
                            );
                            assert_eq!(exact.memory.read_word(actor + 0x1CD8), actor);
                            let expected_path = match segment.phase() {
                                OpeningChainPhase::HiddenUntilNextUpdate => 0xF868,
                                OpeningChainPhase::Following => 0xF881,
                                OpeningChainPhase::Departing { .. } => 0xF8CD,
                                _ => unreachable!(),
                            };
                            assert_eq!(exact.memory.read_word(actor + FIELD_PATH), expected_path);
                            let aux = exact.memory.read_word(actor + 0x1CEC);
                            assert_eq!(exact.memory.read_byte(aux + 0x6A61), 1);
                            assert_eq!(exact.memory.read_byte(aux + 0x6A62), 11);
                            assert_eq!(
                                Some(exact.memory.read_byte(aux + 0x6A63)),
                                segment.ordinary_contact_payload()
                            );
                        }
                        let source_bursts: Vec<_> = active
                            .iter()
                            .copied()
                            .filter(|actor| exact.memory.read_word(*actor + FIELD_SHAPE) == 0xBDD0)
                            .collect();
                        assert_eq!(
                            active.len(),
                            1 + usize::from(trailing_actor) + live_segments + source_bursts.len(),
                            "active count scenario={scenario} update={update}: {:?}",
                            active
                                .iter()
                                .map(|id| (
                                    *id,
                                    exact.memory.read_word(*id + FIELD_PATH),
                                    exact.memory.read_byte(*id + 0x25)
                                ))
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            source_bursts.len(),
                            native.bursts().len(),
                            "burst count update={update}"
                        );
                        // Later departures originate earlier in the chain; their
                        // after-spawner insertion leaves bursts newest-first.
                        for (actor, burst) in
                            source_bursts.into_iter().zip(native.bursts().iter().rev())
                        {
                            assert_eq!(
                                read_pose(&exact, actor),
                                burst.pose,
                                "burst pose update={update}"
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + 0x1CCA) & 127,
                                burst.color_frame
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + 0x1CE2),
                                0,
                                "authored sprite size delta remains zero"
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + 0x20) & 32 != 0,
                                burst.is_sprite()
                            );
                            assert_eq!(
                                exact.memory.read_byte(actor + 0x1CC8),
                                burst.depth_offset()
                            );
                            assert_eq!(exact.memory.read_byte(actor + 0x1CDA), burst.size_bias());
                        }
                    }
                    assert_eq!(saw_capacity_error, capacity == Some(0));
                    assert_eq!(
                        retirements,
                        if scenario == 5 || saw_capacity_error {
                            0
                        } else {
                            9
                        }
                    );
                    assert_eq!(native.is_finished(), scenario != 5 && !saw_capacity_error);
                    assert_eq!(pressure_updates != 0, capacity == Some(1));
                }
            }
        }
    }
}

fn install_strategy(exact: &mut Game, actor: u16, path: u16) {
    exact.memory.write_word(actor + FIELD_PATH, path);
    exact
        .memory
        .write_word(actor + FIELD_STRATEGY, STRATEGY as u16);
    exact
        .memory
        .write_byte(actor + FIELD_STRATEGY + 2, (STRATEGY >> 16) as u8);
    exact.memory.write_byte(actor + 0x2D, 1);
}

#[test]
fn original_chain_constructor_uses_shared_primary_owner_and_distinct_predecessors() {
    let mut exact = Game::new(retail()).unwrap();
    let parent = allocate(&mut exact.memory, 0).unwrap();
    install_strategy(&mut exact, parent, 0xFDC2);
    exact.memory.write_word(parent + FIELD_SHAPE, 0xE194);
    exact.memory.write_byte(parent + 0x2E, 16);
    exact.memory.write_word(PLAYER_ONE, 0x033F);
    exact.memory.write_word(SELECTED_OBJECT, 0x033F);
    exact.memory.write_word(0x033F + FIELD_PATH, 0x0140);
    exact.memory.write_word(CURRENT_OBJECT, parent);
    exact.run_retail_oracle_routine(STRATEGY, parent).unwrap();
    // Retain real parent publication, without advancing its later choreography.
    exact.memory.write_word(parent + FIELD_PATH, 0xFF43);
    assert_eq!(active_objects(&exact.memory).len(), 2);
    for update in 0..16 {
        let prior_poses: Vec<_> = active_objects(&exact.memory)
            .into_iter()
            .filter(|id| *id != parent)
            .map(|id| read_pose(&exact, id))
            .collect();
        exact.memory.write_word(CURRENT_OBJECT, parent);
        exact.run_retail_oracle_routine(0x7F34E7, parent).unwrap();
        exact.run_retail_oracle_routine(0x7F354A, parent).unwrap();
        let actors = active_objects(&exact.memory);
        assert_eq!(actors.len(), 10, "same-update recursive construction");
        assert_eq!(exact.memory.read_byte(parent + 0x1CE2), 9);
        let mut predecessor = parent;
        let mut predecessor_pose = read_pose(&exact, parent);
        for (index, actor) in actors.into_iter().filter(|id| *id != parent).enumerate() {
            assert_eq!(exact.memory.read_byte(actor + 0x1CE2), index as u8 + 1);
            assert_eq!(exact.memory.read_word(actor + 6), parent, "primary owner");
            assert_eq!(
                exact.memory.read_word(actor + 0x1C),
                predecessor,
                "predecessor"
            );
            assert_eq!(
                exact.memory.read_word(actor + 0x1CD8),
                actor,
                "self-published pose"
            );
            assert_eq!(
                (exact.memory.read_word(actor + FIELD_SHAPE) - 0xBC9C) / 28,
                if index == 8 { 342 } else { 340 }
            );
            assert_eq!(exact.memory.read_word(actor + 0x1CCF), 0);
            assert_eq!(exact.memory.read_word(actor + 0x1CD1), 0);
            assert_eq!(
                exact.memory.read_word(actor + 0x1CD3) as i16,
                if index == 0 { -11 } else { -25 }
            );
            assert_eq!(
                exact.memory.read_byte(actor + 0x2D),
                if update == 0 { 2 } else { 15 }
            );
            assert_eq!(
                exact.memory.read_word(actor + FIELD_PATH),
                if update == 0 { 0xF868 } else { 0xF881 }
            );
            if update != 0 {
                predecessor_pose = follow_intro_predecessor(
                    prior_poses[index],
                    predecessor_pose,
                    if index == 0 { -11 } else { -25 },
                );
                assert_eq!(
                    predecessor_pose,
                    read_pose(&exact, actor),
                    "update={update}, ordinal={}",
                    index + 1
                );
            }
            predecessor = actor;
        }
    }
}

#[test]
fn follower_geometry_matches_original_byte_rotations_and_pre_move_facing() {
    let mut exact = Game::new(retail()).unwrap();
    let predecessor = allocate(&mut exact.memory, 0).unwrap();
    let actor = allocate(&mut exact.memory, 0).unwrap();
    exact.memory.write_word(actor + 6, predecessor);
    for depth in [-128, -127, -100, -25, -11, -1, 0, 1, 11, 25, 100, 127] {
        for pitch in 0..=255_u8 {
            for yaw in [0, 1, 31, 64, 127, 128, 192, 255] {
                let prior = pose(
                    Vector3 {
                        x: -31000,
                        y: 32760,
                        z: 17000,
                    },
                    73,
                    91,
                    pitch.wrapping_add(yaw),
                );
                let anchor = pose(
                    Vector3 {
                        x: 32740,
                        y: -32750,
                        z: -29000,
                    },
                    pitch,
                    yaw,
                    197,
                );
                compare_follow(&mut exact, actor, predecessor, prior, anchor, depth);
            }
        }
    }
    // Whole signed depth domain and wrapped/coincident aim deltas.
    for depth in i8::MIN..=i8::MAX {
        for delta in [
            Vector3::default(),
            Vector3 { x: 1, y: -1, z: 0 },
            Vector3 {
                x: -32768,
                y: 32767,
                z: -32768,
            },
            Vector3 {
                x: 6000,
                y: -13000,
                z: 28000,
            },
        ] {
            let anchor = pose(
                Vector3::default(),
                depth as u8,
                (depth as u8).wrapping_mul(17),
                63,
            );
            compare_follow(
                &mut exact,
                actor,
                predecessor,
                pose(delta, 17, 37, 251),
                anchor,
                depth,
            );
        }
    }
}

fn compare_follow(
    exact: &mut Game,
    actor: u16,
    predecessor: u16,
    prior: IntroScenePose,
    anchor: IntroScenePose,
    depth: i8,
) {
    write_pose(exact, actor, prior);
    write_pose(exact, predecessor, anchor);
    install_strategy(exact, actor, 0xF863);
    exact.memory.write_word(actor + 0x1CD3, depth as i16 as u16);
    exact.memory.write_word(CURRENT_OBJECT, actor);
    // Enter the authored command and stop at its real WaitOne; no patched code.
    exact.run_retail_oracle_routine(STRATEGY, actor).unwrap();
    assert_eq!(exact.memory.read_word(actor + FIELD_PATH), 0xF868);
    assert_eq!(
        follow_intro_predecessor(prior, anchor, depth),
        read_pose(exact, actor),
        "depth={depth}, prior={prior:?}, predecessor={anchor:?}"
    );
}

fn pose(position: Vector3, pitch: u8, yaw: u8, roll: u8) -> IntroScenePose {
    IntroScenePose {
        position,
        rotation: Rotation {
            pitch: Angle::from_units(pitch),
            yaw: Angle::from_units(yaw),
            roll: Angle::from_units(roll),
        },
    }
}

fn write_pose(exact: &mut Game, actor: u16, pose: IntroScenePose) {
    for (offset, value) in
        [12, 14, 16]
            .into_iter()
            .zip([pose.position.x, pose.position.y, pose.position.z])
    {
        exact.memory.write_word(actor + offset, value as u16);
    }
    for (offset, value) in
        [18, 20, 22]
            .into_iter()
            .zip([pose.rotation.pitch, pose.rotation.yaw, pose.rotation.roll])
    {
        exact.memory.write_byte(actor + offset, value.units());
    }
}

fn read_pose(exact: &Game, actor: u16) -> IntroScenePose {
    pose(
        Vector3 {
            x: exact.memory.read_word(actor + 12) as i16,
            y: exact.memory.read_word(actor + 14) as i16,
            z: exact.memory.read_word(actor + 16) as i16,
        },
        exact.memory.read_byte(actor + 18),
        exact.memory.read_byte(actor + 20),
        exact.memory.read_byte(actor + 22),
    )
}
