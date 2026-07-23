use sf_game::world::{InlineCb, World};

#[test]
fn every_route_callback_record_resolves_to_a_typed_callback() {
    for map_id in sf_map::catalog::map_id::NONE..=sf_map::catalog::map_id::TRAINING {
        let Some((natives, inlines)) = sf_map::catalog::get_map_callback_regs(map_id) else {
            continue;
        };
        let level = sf_map::catalog::get_map_data(map_id).expect("catalog map");
        let mut world = World::init();
        world.register_named_callbacks(natives, inlines, &level.labels);

        let expected_native = natives
            .iter()
            .map(|(address, _)| address)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let expected_inline = inlines
            .iter()
            .map(|(offset, _)| offset)
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(
            world.native_cbs.len(),
            expected_native,
            "map {map_id} dropped a native callback"
        );
        assert_eq!(
            world.inline_cbs.len(),
            expected_inline,
            "map {map_id} dropped an inline callback"
        );
    }
}

#[test]
fn blackhole_skillfly_bonus_guard_resolves_its_skip_label() {
    let mut world = World::init();
    let labels = vec![
        ("level1_2.map1_2.skillfly_bonus_0_skip".to_string(), 1800),
        ("level1_2.map1_2.blackhole_bonus_skip".to_string(), 2103),
    ];

    world.register_named_callbacks(&[], &[(2088, "level1_2_blackhole_bonus_guard")], &labels);

    assert_eq!(
        world.find_inline(2088),
        Some(InlineCb::SkillflyGuard { skip_ptr: 2103 })
    );
}

#[test]
fn armada_stratdone_guard_resolves_its_continuation() {
    let mut world = World::init();
    let labels = vec![
        ("level1_3.map1_3c.loop".to_string(), 1884),
        ("level1_3.map1_3c.cont".to_string(), 1892),
        ("level1_3.map1_3d".to_string(), 1901),
    ];

    world.register_named_callbacks(&[], &[(1887, "map1_3c_chkstratdone1_check")], &labels);

    assert_eq!(
        world.find_inline(1887),
        Some(InlineCb::Stratdone1Guard { skip_ptr: 1892 })
    );
}

#[test]
fn fortuna_player_dead_guards_resolve_their_own_loop_labels() {
    let mut world = World::init();
    let labels = vec![
        ("level3_3.pdead2".to_string(), 713),
        ("level3_3.pdead".to_string(), 1392),
    ];

    world.register_named_callbacks(
        &[],
        &[
            (717, "level3_3_pdead2_check"),
            (1396, "level3_3_pdead_check"),
        ],
        &labels,
    );

    assert_eq!(
        world.find_inline(717),
        Some(InlineCb::PlayerDeadLoop { loop_ptr: 713 })
    );
    assert_eq!(
        world.find_inline(1396),
        Some(InlineCb::PlayerDeadLoop { loop_ptr: 1392 })
    );
}

#[test]
fn venom_orbital_boss_gates_resolve_their_loop_and_spawn_labels() {
    let mut world = World::init();
    let labels = vec![
        ("level3_6.boss".to_string(), 1203),
        ("level3_6.owait".to_string(), 1207),
        ("level3_6.cont2".to_string(), 1211),
    ];

    world.register_named_callbacks(
        &[],
        &[(1206, "map3_6_noctrl_wait"), (1211, "map3_6_hpcheck_wait")],
        &labels,
    );

    assert_eq!(
        world.find_inline(1206),
        Some(InlineCb::NoctrlLoop { loop_ptr: 1203 })
    );
    assert_eq!(
        world.find_inline(1211),
        Some(InlineCb::HpFlymodeGate {
            hp_loop_ptr: 1207,
            cont_ptr: 1211,
        })
    );
}

#[test]
fn venom_orbital_real_map_registers_both_boss_gates() {
    let level =
        sf_map::catalog::get_map_data(sf_map::catalog::map_id::M3_6).expect("route-3 orbital map");
    let (_, inlines) = sf_map::catalog::get_map_callback_regs(sf_map::catalog::map_id::M3_6)
        .expect("route-3 callback records");
    let mut world = World::init();
    world.register_named_callbacks(&[], inlines, &level.labels);

    let noctrl_ptr = inlines
        .iter()
        .find(|(_, name)| *name == "map3_6_noctrl_wait")
        .map(|(ptr, _)| *ptr)
        .expect("no-control gate");
    let hp_ptr = inlines
        .iter()
        .find(|(_, name)| *name == "map3_6_hpcheck_wait")
        .map(|(ptr, _)| *ptr)
        .expect("HP gate");
    let label = |wanted: &str| {
        level
            .labels
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, ptr)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };

    assert_eq!(
        world.find_inline(noctrl_ptr),
        Some(InlineCb::NoctrlLoop {
            loop_ptr: label("level3_6.boss"),
        })
    );
    assert_eq!(
        world.find_inline(hp_ptr),
        Some(InlineCb::HpFlymodeGate {
            hp_loop_ptr: label("level3_6.owait"),
            cont_ptr: label("level3_6.cont2"),
        })
    );
}

#[test]
fn special_real_map_registers_every_progression_inline() {
    let level =
        sf_map::catalog::get_map_data(sf_map::catalog::map_id::SPECIAL).expect("secret-level map");
    let (_, inlines) = sf_map::catalog::get_map_callback_regs(sf_map::catalog::map_id::SPECIAL)
        .expect("secret-level callback records");
    let mut world = World::init();
    world.register_named_callbacks(&[], inlines, &level.labels);

    let ptr = |wanted: &str| {
        inlines
            .iter()
            .find(|(_, name)| *name == wanted)
            .map(|(ptr, _)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };
    let label = |wanted: &str| {
        level
            .labels
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, ptr)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };

    assert_eq!(
        world.find_inline(ptr("special_mapwaitboss_trigse")),
        Some(InlineCb::MapwaitbossTrigse)
    );
    assert_eq!(
        world.find_inline(ptr("special_mapwaitboss_cantdie")),
        Some(InlineCb::MapwaitbossCantdie)
    );
    assert_eq!(
        world.find_inline(ptr("special_mapwaitboss_cleanup")),
        Some(InlineCb::MapwaitbossCleanup)
    );
    assert_eq!(
        world.find_inline(ptr("special_boss_cleanup")),
        Some(InlineCb::SpecialBossCleanup)
    );
    assert_eq!(
        world.find_inline(ptr("special_theenddead_check")),
        Some(InlineCb::SpecialTheEndGate {
            loop_ptr: label("special.theenddead_check"),
            cont_ptr: label("special.theenddead_cont"),
        })
    );
}

#[test]
fn titania_real_map_registers_all_four_progression_callbacks() {
    let level = sf_map::catalog::get_map_data(sf_map::catalog::map_id::M2_3).expect("Titania map");
    let (_, inlines) = sf_map::catalog::get_map_callback_regs(sf_map::catalog::map_id::M2_3)
        .expect("Titania callback records");
    let mut world = World::init();
    world.register_named_callbacks(&[], inlines, &level.labels);

    let ptr = |wanted: &str| {
        inlines
            .iter()
            .find(|(_, name)| *name == wanted)
            .map(|(ptr, _)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };
    let label = |wanted: &str| {
        level
            .labels
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, ptr)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };

    assert_eq!(
        world.find_inline(ptr("level2_3_fog_guard")),
        Some(InlineCb::FogGuard {
            continue_ptr: label("level2_3.fog_guard_continue"),
        })
    );
    assert_eq!(
        world.find_inline(ptr("level2_3_setvar_inline")),
        Some(InlineCb::PostFog)
    );
    assert_eq!(
        world.find_inline(ptr("level2_3b_trigger_check")),
        Some(InlineCb::MapTriggerGate {
            carryon_ptr: label("level2_3b.carryon"),
            waitabit_ptr: label("level2_3b.waitabit"),
        })
    );
    assert_eq!(
        world.find_inline(ptr("level2_3b_seatest_check")),
        Some(InlineCb::SeaTestLoop {
            loop_ptr: label("level2_3b.seatest"),
        })
    );
}

#[test]
fn macbeth_trucker_real_map_registers_biker_and_trigger_gates() {
    let level = sf_map::catalog::get_map_data(sf_map::catalog::map_id::M2_6).expect("Macbeth map");
    let (_, inlines) = sf_map::catalog::get_map_callback_regs(sf_map::catalog::map_id::M2_6)
        .expect("Macbeth callback records");
    let mut world = World::init();
    world.register_named_callbacks(&[], inlines, &level.labels);

    let ptr = |wanted: &str| {
        inlines
            .iter()
            .find(|(_, name)| *name == wanted)
            .map(|(ptr, _)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };
    let label = |wanted: &str| {
        level
            .labels
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, ptr)| *ptr)
            .unwrap_or_else(|| panic!("missing {wanted}"))
    };

    assert_eq!(
        world.find_inline(ptr("trucker_biker_check")),
        Some(InlineCb::TruckerBikerGate {
            carryon_ptr: label("level2_6.trucker.carryon"),
        })
    );
    assert_eq!(
        world.find_inline(ptr("trucker_trigger_check")),
        Some(InlineCb::TruckerTriggerGate {
            rightblock_ptr: label("level2_6.trucker.rightblockbit"),
            continue_ptr: label("level2_6.trucker.continue"),
        })
    );
}

#[test]
fn training_real_map_registers_ring_course_gate() {
    let level =
        sf_map::catalog::get_map_data(sf_map::catalog::map_id::TRAINING).expect("training map");
    let (_, inlines) = sf_map::catalog::get_map_callback_regs(sf_map::catalog::map_id::TRAINING)
        .expect("training callback records");
    let mut world = World::init();
    world.register_named_callbacks(&[], inlines, &level.labels);

    let gate_ptr = inlines
        .iter()
        .find(|(_, name)| *name == "training_eguchifly_check")
        .map(|(ptr, _)| *ptr)
        .expect("training ring gate");
    let continue_ptr = level
        .labels
        .iter()
        .find(|(name, _)| name == "training.eguchifly_continue")
        .map(|(_, ptr)| *ptr)
        .expect("training continuation");

    assert_eq!(
        world.find_inline(gate_ptr),
        Some(InlineCb::EguchiFlyGate { continue_ptr })
    );
}
