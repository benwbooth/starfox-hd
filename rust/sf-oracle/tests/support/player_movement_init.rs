//! Shared movement-reset state compared with the original initializer.
//! This gate covers gameplay/object state and particle activation, not the red
//! palette lifecycle or the source's separately allocated camera object.
use super::{Source, OBJECT, WRAM};
use sf_core::{player_view::PlayerViewMode, shape::resolve_shape_word};
use sf_game::Game;
use sf_oracle::SnesBus;
use sf_strat::player::{
    initialize_player_for_map, player_move_init, select_ship, strat_spawn_player,
};

fn words(game: &Game) -> Vec<(&'static str, i16)> {
    let v = &game.vars;
    let s = &v.strategy;
    vec![
        ("VIEWDIST", v.viewdist),
        ("OUTDIST", s.view_distance),
        ("PLROTX", s.player_rotation[0]),
        ("PLROTY", s.player_rotation[1]),
        ("PLROTZ", s.player_rotation[2]),
        ("PLAYER_TURNROT", s.player_turn_rotation),
        ("PLAYER_ZSHAKE", s.player_depth_shake),
        ("PVIEWPOSX", s.player_view_position[0]),
        ("PVIEWPOSY", s.player_view_position[1]),
        ("PVIEWPOSZ", s.player_view_position[2]),
        ("PLAYERDIEYROTSPEED", s.player_death_yaw_step),
        ("PVIEWVELZ", v.pviewvelz),
        ("PLAYERVELZ", v.playervel_z),
        // These neighboring values are deliberately not reset by the entry.
        ("PLAYER_SPEED", s.player_speed),
        ("OUTVX", s.view_pitch),
        ("OUTVY", s.view_yaw),
        ("OUTVZ", s.view_roll),
        ("HUDROT", s.hud_rotation),
        ("SPECWEPCNT", s.special_weapon_count as i16),
    ]
}

fn bytes(game: &Game) -> Vec<(&'static str, u8)> {
    let v = &game.vars;
    let s = &v.strategy;
    vec![
        ("VIEWTYPE", s.view_kind),
        ("SLIMECOUNT", v.shared.slime_count),
        ("PSTRATFLAGS", v.pstratflags),
        ("PLAYER_ZTILT", s.player_depth_tilt as u8),
        ("PLAYER_ZSTRATADD", s.player_depth_strategy_offset),
        ("PLAYER_ROLLZVEL", s.player_roll_velocity as u8),
        ("PLAYER_ROLLZOFF", s.player_roll_offset as u8),
        ("PNUMHITS", s.player_hit_count),
        ("NUMPLASERS", s.player_laser_count),
        ("SPECIALDELAY", s.special_delay),
        ("PLAYER_TOSPEED", s.player_target_speed),
        ("PLAYER_MEDSPEED", s.player_medium_speed),
        ("BOOSTZOFF", s.boost_depth_offset as u8),
        ("NOMAXBG2YSCROLL", s.no_maximum_background_y),
        ("PSHIPFLAGS", v.pshipflags),
        ("PSHIPFLAGS2", v.pshipflags2),
        ("PSHIPFLAGS3", v.pshipflags3),
        ("SPLAYERFLYMODE", v.player_view_mode as u8),
        ("PLAYERFLYMODE", v.playerflymode),
        ("GAMEFLAGS", v.gameflags),
        ("FIRECNT", s.fire_count),
        ("FIREDELAY", s.fire_delay),
        ("PLAYER_ROLLDELAY", s.player_roll_delay),
        ("PLAYER_NOCTRLCNT", s.player_control_delay),
    ]
}

#[test]
fn pending_planet_strategy_install_preserves_shared_movement_and_weapon_fields() {
    let source = Source::load();
    let speeds = [i16::MIN, -257, -1, 0, 1, 63, 64, 65, i16::MAX];
    for byte in u8::MIN..=u8::MAX {
        let mut game = Game::new();
        let player = strat_spawn_player(&mut game).unwrap();
        let speed = speeds[usize::from(byte) % speeds.len()];
        game.vars.pviewvelz = speed;
        game.vars.playervel_z = speed.wrapping_add(19);
        game.vars.strategy.player_rotation = [speed, speed.wrapping_mul(13), !speed];
        game.vars.strategy.player_target_speed = byte;
        game.vars.strategy.player_medium_speed = byte.wrapping_add(47);
        game.vars.strategy.special_delay = byte.wrapping_add(113);
        game.vars.strategy.player_laser_count = byte.wrapping_add(7);
        game.vars.strategy.boost_depth_offset = byte as i8;
        let retained_words = ["PVIEWVELZ", "PLAYERVELZ", "PLROTX", "PLROTY", "PLROTZ"];
        let retained_bytes = [
            "PLAYER_TOSPEED",
            "PLAYER_MEDSPEED",
            "SPECIALDELAY",
            "NUMPLASERS",
            "BOOSTZOFF",
        ];
        let mut bus = SnesBus::new(source.rom.clone());
        // BGS.bg_training_1's pstrat macro queues the initializer. WORLD's
        // background request installs it without executing its movement body.
        source.word(&mut bus, 0, "PLAYPT", OBJECT as i16);
        let initializer = source.symbol("PLAYERONPLANET_ISTRAT");
        source.word(&mut bus, 0, "NEWPLAYERSTRAT", initializer as i16);
        source.byte(&mut bus, 2, "NEWPLAYERSTRAT", (initializer >> 16) as u8);
        for (name, value) in words(&game)
            .into_iter()
            .filter(|(name, _)| retained_words.contains(name))
        {
            source.word(&mut bus, 0, name, value);
        }
        for (name, value) in bytes(&game)
            .into_iter()
            .filter(|(name, _)| retained_bytes.contains(name))
        {
            source.byte(&mut bus, 0, name, value);
        }
        source.run(&mut bus, "SETBGINFOREQ_L");
        let installed = WRAM | (OBJECT + source.symbol("AL_STRATPTR"));
        assert_eq!(bus.read8(installed), initializer as u8);
        assert_eq!(bus.read8(installed + 1), (initializer >> 8) as u8);
        assert_eq!(bus.read8(installed + 2), (initializer >> 16) as u8);
        initialize_player_for_map(&mut game, sf_map::catalog::map_id::TRAINING, player);
        for (name, value) in words(&game)
            .into_iter()
            .filter(|(name, _)| retained_words.contains(name))
        {
            assert_eq!(
                value,
                bus.wram_read16(source.symbol(name)) as i16,
                "{name} seed={byte}"
            );
        }
        for (name, value) in bytes(&game)
            .into_iter()
            .filter(|(name, _)| retained_bytes.contains(name))
        {
            assert_eq!(
                value,
                bus.read8(WRAM | source.symbol(name)),
                "{name} seed={byte}"
            );
        }
    }
}

#[test]
fn movement_init_resets_source_owned_fields_and_preserves_neighboring_state() {
    let source = Source::load();
    for mode in [
        PlayerViewMode::Exterior,
        PlayerViewMode::CloseExterior,
        PlayerViewMode::EnteringCockpit,
        PlayerViewMode::Cockpit,
        PlayerViewMode::LeavingCockpit,
    ] {
        for value in 0..=u8::MAX {
            let mut game = Game::new();
            game.objs.alloc().unwrap();
            let player = game.objs.alloc().unwrap();
            let word = i16::from_le_bytes([value ^ 0xA5, value]);
            let ship = value % 7;
            select_ship(&mut game, ship);
            let v = &mut game.vars;
            v.player_view_mode = mode;
            v.viewdist = word;
            v.pviewvelz = word;
            v.playervel_z = word;
            v.pshipflags = value;
            v.pshipflags2 = value;
            v.pshipflags3 = value;
            v.pstratflags = value;
            v.shared.slime_count = value;
            v.playerflymode = value;
            v.gameflags = value;
            let s = &mut v.strategy;
            s.view_distance = word;
            s.player_rotation = [word; 3];
            s.player_turn_rotation = word;
            s.player_depth_shake = word;
            s.player_view_position = [word; 3];
            s.player_death_yaw_step = word;
            s.player_speed = word;
            s.view_pitch = word;
            s.view_yaw = word;
            s.view_roll = word;
            s.hud_rotation = word;
            s.special_weapon_count = word as u16;
            s.view_kind = value;
            s.player_depth_tilt = value as i8;
            s.player_depth_strategy_offset = value;
            s.player_roll_velocity = value as i8;
            s.player_roll_offset = value as i8;
            s.player_hit_count = value;
            s.player_laser_count = value;
            s.special_delay = value;
            s.player_target_speed = value;
            s.player_medium_speed = value;
            s.boost_depth_offset = value as i8;
            s.no_maximum_background_y = value;
            s.fire_count = value;
            s.fire_delay = value;
            s.player_roll_delay = value;
            s.player_control_delay = value;
            let al = &mut game.objs.aliens[player as usize];
            [al.worldx, al.worldy, al.worldz] = [word; 3];
            [al.vx, al.vy, al.vz] = [word; 3];
            [al.rotx, al.roty, al.rotz] = [value; 3];
            al.vel = value;
            al.hp = value;
            al.ap = value;
            al.type_ = value;
            al.sflags = value;
            al.sflags2 = value;
            al.sflags3 = value;
            al.sflags4 = value;
            let mut bus = SnesBus::new(source.rom.clone());
            source.seed(&mut bus, &game.objs.aliens[player as usize], &game);
            game.vars.particles_enabled = value & 1 != 0;
            let particles_enabled = source.symbol("M_PARTICLESON");
            bus.write8(particles_enabled, value);
            bus.write8(particles_enabled + 1, value ^ 0xA5);
            for (name, value) in words(&game) {
                source.word(&mut bus, 0, name, value);
            }
            for (name, value) in bytes(&game) {
                source.byte(&mut bus, 0, name, value);
            }
            for (index, name) in [
                "PLAYERSHAPE",
                "PLAYERSHAPEL",
                "PLAYERSHAPER",
                "PLAYERSHAPELR",
            ]
            .into_iter()
            .enumerate()
            {
                let address =
                    source.symbol("PLAYER_SHAPES") + u32::from(ship) * 8 + index as u32 * 2;
                let shape = i16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                source.word(&mut bus, 0, name, shape);
            }
            source.run(&mut bus, "PLAYERMOVE_INIT_L");
            player_move_init(&mut game, player);
            assert!(game.vars.particles_enabled);
            assert_eq!(
                [bus.read8(particles_enabled), bus.read8(particles_enabled + 1)],
                [1, 1]
            );
            let context = format!("mode={mode:?} inherited={value}");
            for (name, actual) in words(&game) {
                let address = WRAM | source.symbol(name);
                let expected = i16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
                assert_eq!(actual, expected, "{context}: {name}");
            }
            for (name, actual) in bytes(&game) {
                assert_eq!(
                    actual,
                    bus.read8(WRAM | source.symbol(name)),
                    "{context}: {name}"
                );
            }
            for (name, actual) in [
                ("INTERNALPLAYPT", game.vars.internal_playpt),
                ("PLAYPT", game.vars.player_object),
                ("VIEWTOOBJ", game.vars.strategy.view_target_object),
            ] {
                let address = WRAM | source.symbol(name);
                assert_eq!(actual, player as i16, "{context}: {name}");
                assert_eq!(
                    u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]),
                    OBJECT as u16
                );
            }
            let address = WRAM | (OBJECT + source.symbol("AL_SHAPE"));
            let source_shape = u16::from_le_bytes([bus.read8(address), bus.read8(address + 1)]);
            let mut native_object = game.objs.aliens[player as usize];
            assert_eq!(
                native_object.shape,
                resolve_shape_word(source_shape),
                "{context}: shape"
            );
            native_object.shape = source_shape;
            source.compare(&mut bus, &native_object, &context);
        }
    }
}
