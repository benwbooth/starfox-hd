//! Complete original bridge movement/view bodies, including retained locks
//! and the post-movement vertical-velocity publication.

use sf_core::pad;
use sf_game::{alien::Alien, vars::*, Game, Hooks};
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::player::{
    player_on_bridge_strat, set_player_on_bridge, strat_player_clear_bridge_init,
    strat_spawn_player,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const WRAM: u32 = 0x7E0000;
const OBJECT: u32 = 0x0400;

#[derive(Clone, Default)]
struct Sounds(Rc<RefCell<Vec<u8>>>);
impl Hooks for Sounds {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn words(game: &Game) -> Vec<(&'static str, i16)> {
    let v = &game.vars;
    let s = &v.strategy;
    vec![
        ("PLROTX", s.player_rotation[0]),
        ("PLROTY", s.player_rotation[1]),
        ("PLROTZ", s.player_rotation[2]),
        ("PLAYER_SPEED", s.player_speed),
        ("PLAYER_TURNROT", s.player_turn_rotation),
        ("PLAYER_ZSHAKE", s.player_depth_shake),
        ("PLAYER_ZSHAKEV", s.player_depth_shake_velocity),
        ("PVIEWPOSX", s.player_view_position[0]),
        ("PVIEWPOSY", s.player_view_position[1]),
        ("PVIEWPOSZ", s.player_view_position[2]),
        ("PVIEWVELZ", v.pviewvelz),
        ("PLAYERVELZ", v.playervel_z),
        ("OUTDIST", s.view_distance),
        ("VIEWDIST", v.viewdist),
        ("OUTVX", s.view_pitch),
        ("OUTVY", s.view_yaw),
        ("OUTVZ", s.view_roll),
        ("VIEWCY", s.view_center_y),
        ("VIEWPOSZ", s.fixed_view_position[2]),
        ("BGSSCROLLZ", s.background_scroll_z),
        ("HUDROT", s.hud_rotation),
        ("MINPMOVEX", s.player_min_x),
        ("MAXPMOVEX", s.player_max_x),
        ("MINPMOVEY", v.minpmove_y),
        ("MAXPMOVEY", s.player_max_y),
        ("MINPWMOVEY", s.water_player_min_y),
        ("MAXPWMOVEY", s.water_player_max_y),
        ("PLAYER_POSZ", v.player_posz),
        ("GAMEFRAME", v.gameframe as i16),
        ("SPECWEPCNT", s.special_weapon_count as i16),
        ("PSVAR_WORD1", v.psvar_word1),
        ("PSVAR_WORD2", v.psvar_word2),
    ]
}

fn bytes(game: &Game) -> Vec<(&'static str, u8)> {
    let v = &game.vars;
    let s = &v.strategy;
    vec![
        ("PSHIPFLAGS", v.pshipflags),
        ("PSHIPFLAGS2", v.pshipflags2),
        ("PSHIPFLAGS3", v.pshipflags3),
        ("PSTRATFLAGS", v.pstratflags),
        ("PLAYERFLYMODE", v.playerflymode),
        ("SPLAYERFLYMODE", v.player_view_mode as u8),
        ("GAMEFLAGS", v.gameflags),
        ("FRAMERATE", s.frame_rate),
        ("PMOVELIMIT", s.player_move_limit),
        ("PMOVELIMITAND", s.player_move_limit_mask),
        ("PLAYER_TOSPEED", s.player_target_speed),
        ("PLAYER_MEDSPEED", s.player_medium_speed),
        ("PLAYER_ZTILT", s.player_depth_tilt as u8),
        ("PLAYER_ZSTRATADD", s.player_depth_strategy_offset),
        ("PLAYER_ROLLZVEL", s.player_roll_velocity as u8),
        ("PLAYER_ROLLZOFF", s.player_roll_offset as u8),
        ("PLAYER_ROLLDELAY", s.player_roll_delay),
        ("PLAYER_NOCTRLCNT", s.player_control_delay),
        ("FIRECNT", s.fire_count),
        ("FIREDELAY", s.fire_delay),
        ("SPECIALDELAY", s.special_delay),
        ("NUMPLASERS", s.player_laser_count),
        ("ARROWS", s.arrow_flags),
        ("PLAYERSNDFLAG", v.player_snd_flag),
        ("STAYBLACK", s.stay_black as u8),
        ("DOINGWIPE", s.wipe_active),
        ("BOOSTZOFF", s.boost_depth_offset as u8),
        ("PLAYER_ZROTFLOAT", s.player_roll_float as u8),
        ("CONT0", (v.pad1 >> 8) as u8),
        ("CONTL0", v.pad1 as u8),
        ("LASTCONT0", v.lastcont0),
        ("LASTCONTL0", v.lastcontl0),
        ("PSVAR_BYTE1", s.player_bytes[0]),
        ("PSVAR_BYTE2", s.player_bytes[1]),
    ]
}

fn actor_words(actor: &Alien) -> [(&'static str, i16); 6] {
    [
        ("AL_WORLDX", actor.worldx),
        ("AL_WORLDY", actor.worldy),
        ("AL_WORLDZ", actor.worldz),
        ("AL_VX", actor.vx),
        ("AL_VY", actor.vy),
        ("AL_VZ", actor.vz),
    ]
}

fn actor_bytes(actor: &Alien) -> [(&'static str, u8); 21] {
    [
        ("AL_ROTX", actor.rotx),
        ("AL_ROTY", actor.roty),
        ("AL_ROTZ", actor.rotz),
        ("AL_VEL", actor.vel),
        ("AL_HP", actor.hp),
        ("AL_SFLAGS", actor.sflags),
        ("AL_SFLAGS2", actor.sflags2),
        ("AL_SFLAGS3", actor.sflags3),
        ("AL_SFLAGS4", actor.sflags4),
        ("AL_SBYTE1", actor.sbyte1),
        ("AL_SBYTE2", actor.sbyte2),
        ("AL_SBYTE3", actor.sbyte3),
        ("AL_SBYTE4", actor.sbyte4),
        ("AL_COUNT", actor.count),
        ("AL_COUNT1", actor.count1),
        ("AL_TYPE", actor.type_),
        ("AL_FLAGS", actor.flags),
        ("AL_AP", actor.ap),
        ("ALX_SND1", actor.snd1),
        ("ALX_SND2", actor.snd2),
        ("ALX_TX", actor.tx),
    ]
}

struct Original {
    bus: SnesBus,
    names: HashMap<String, u32>,
}

impl Original {
    fn new(game: &Game, player: u16) -> Self {
        let mut result = Self {
            bus: SnesBus::new(load_built_rom().expect("bridge oracle needs source-built ROM")),
            names: load_symbols(),
        };
        for (base, fields) in [
            (0, words(game)),
            (
                OBJECT,
                actor_words(&game.objs.aliens[player as usize]).to_vec(),
            ),
        ] {
            for (name, value) in fields {
                result.word(base, name, value);
            }
        }
        for (base, fields) in [
            (0, bytes(game)),
            (
                OBJECT,
                actor_bytes(&game.objs.aliens[player as usize]).to_vec(),
            ),
        ] {
            for (name, value) in fields {
                result.byte(base, name, value);
            }
        }
        for name in ["PLAYPT", "INTERNALPLAYPT", "AL1PT"] {
            result.word(0, name, OBJECT as i16);
        }
        // Actual source shape words, selected from the authored normal-ship
        // row and decoded only for comparison with native catalog identities.
        for (index, name) in [
            "PLAYERSHAPE",
            "PLAYERSHAPEL",
            "PLAYERSHAPER",
            "PLAYERSHAPELR",
        ]
        .into_iter()
        .enumerate()
        {
            let shape = result
                .bus
                .read16(result.names["PLAYER_SHAPES"] + index as u32 * 2);
            assert_eq!(
                sf_core::shape::resolve_shape_word(shape),
                game.vars.strategy.player_shapes[index]
            );
            result.word(0, name, shape as i16);
            if index == 0 {
                result.word(OBJECT, "AL_SHAPE", shape as i16);
            }
        }
        result
    }

    fn word(&mut self, base: u32, name: &str, value: i16) {
        self.bus
            .write16(WRAM | (base + self.names[name]), value as u16);
    }
    fn byte(&mut self, base: u32, name: &str, value: u8) {
        self.bus.write8(WRAM | (base + self.names[name]), value);
    }
    fn run(&mut self, name: &str) {
        self.run_at(OBJECT, self.names[name]);
    }
    fn run_at(&mut self, object: u32, entry: u32) {
        self.byte(0, "ALDEAD", 0);
        self.word(0, "AL1PT", object as i16);
        let result = call(
            &mut self.bus,
            entry,
            &Entry {
                x: object as u16,
                p: 0x20,
                dbr: 0x7E,
                ..Default::default()
            },
        );
        assert!(
            result.returned,
            "{entry:06X} must return without patching its original body"
        );
        assert_eq!(result.x, object as u16);
    }
    fn strategy(&self, object: u32) -> u32 {
        let address = WRAM | (object + self.names["AL_STRATPTR"]);
        u32::from(self.bus.read16(address)) | u32::from(self.bus.read8(address + 2)) << 16
    }
    fn compare_links(&self, game: &Game, mapping: &[(u32, u16)]) {
        let mut original = self.bus.read16(WRAM | self.names["ALLST"]) as u32;
        let mut native = game.objs.active_head;
        let mut previous = 0;
        let mut native_previous = None;
        for _ in 0..mapping.len() {
            if original == 0 {
                break;
            }
            let index = mapping
                .iter()
                .find(|(source, _)| *source == original)
                .unwrap()
                .1;
            assert_eq!(native, Some(index), "active-list order");
            assert_eq!(
                self.bus.read16(WRAM | (original + self.names["_PREV"])),
                previous
            );
            assert_eq!(game.objs.aliens[index as usize].prev, native_previous);
            previous = original as u16;
            native_previous = Some(index);
            native = game.objs.aliens[index as usize].next;
            original = self.bus.read16(WRAM | (original + self.names["_NEXT"])) as u32;
        }
        assert_eq!(original, 0, "source list must terminate");
        assert_eq!(native, None, "native list must terminate");
    }
    fn compare_actor(&self, object: u32, actor: &Alien, context: &str) {
        for (name, actual) in actor_bytes(actor) {
            assert_eq!(
                actual,
                self.bus.read8(WRAM | (object + self.names[name])),
                "{context}: {name}"
            );
        }
        for (name, actual) in actor_words(actor) {
            assert_eq!(
                actual,
                self.bus.read16(WRAM | (object + self.names[name])) as i16,
                "{context}: {name}"
            );
        }
        assert_eq!(
            actor.shape,
            sf_core::shape::resolve_shape_word(
                self.bus.read16(WRAM | (object + self.names["AL_SHAPE"]))
            ),
            "{context}: shape"
        );
    }
    fn compare(&self, game: &Game, player: u16, context: &str) {
        for (base, fields) in [
            (0, bytes(game)),
            (
                OBJECT,
                actor_bytes(&game.objs.aliens[player as usize]).to_vec(),
            ),
        ] {
            for (name, actual) in fields {
                assert_eq!(
                    actual,
                    self.bus.read8(WRAM | (base + self.names[name])),
                    "{context}: {name}"
                );
            }
        }
        for (base, fields) in [
            (0, words(game)),
            (
                OBJECT,
                actor_words(&game.objs.aliens[player as usize]).to_vec(),
            ),
        ] {
            for (name, actual) in fields {
                assert_eq!(
                    actual,
                    self.bus.read16(WRAM | (base + self.names[name])) as i16,
                    "{context}: {name}"
                );
            }
        }
    }
}

fn scene() -> (Game, u16) {
    let mut game = Game::new();
    let player = strat_spawn_player(&mut game).unwrap();
    set_player_on_bridge(&mut game, player);
    game.vars.pshipflags = PSF_NOCTRL | PSF_NOFIRE;
    game.vars.pstratflags = PSTF_INSEQ | PSTF_NOVDISTC;
    game.vars.strategy.frame_rate = 4;
    game.vars.strategy.stay_black = -1;
    game.vars.strategy.player_view_position = [50, -30, 700];
    game.vars.strategy.view_distance = 200;
    game.vars.viewdist = 300;
    game.vars.player_posz = 400;
    game.vars.pad1 = pad::LEFT | pad::UP | pad::Y;
    let actor = &mut game.objs.aliens[player as usize];
    [actor.worldx, actor.worldy, actor.worldz] = [17, -100, 400];
    (game, player)
}

#[test]
fn bridge_body_retains_clear_sequence_locks_and_publishes_original_post_motion_state() {
    for pitch in [-1000, -257, -1, 0, 1, 257, 1000] {
        for rate in [1, 3, 4, 7, 255] {
            let (mut game, player) = scene();
            game.vars.strategy.player_rotation = [pitch, 777, -100];
            game.vars.strategy.frame_rate = rate;
            let mut original = Original::new(&game, player);
            original.run("PLAYERONBRIDGE_STRAT");
            player_on_bridge_strat(&mut game, player);
            original.compare(
                &game,
                player,
                &format!("bridge pitch={pitch} elapsed={rate}"),
            );
        }
    }
}

#[test]
fn bridge_clear_initializer_and_every_centering_visit_match_original_without_unlocking_controls() {
    for initial_distance in [-1, 0, 499, 500, 501, 1000] {
        let (mut game, player) = scene();
        game.vars.pshipflags = 0;
        game.vars.pstratflags = 0;
        game.vars.strategy.view_distance = initial_distance;
        game.vars.strategy.player_rotation = [-1000, 777, -100];
        let mut original = Original::new(&game, player);
        for visit in 0..163 {
            let input = [0, pad::Y, pad::LEFT | pad::UP, pad::A | pad::RIGHT][visit % 4];
            let frame_rate = [1, 3, 4, 7][visit % 4];
            game.vars.pad1 = input;
            game.vars.gameframe = visit as u16;
            game.vars.strategy.frame_rate = frame_rate;
            original.byte(0, "CONT0", (input >> 8) as u8);
            original.byte(0, "CONTL0", input as u8);
            original.word(0, "GAMEFRAME", visit as i16);
            original.byte(0, "FRAMERATE", frame_rate);
            let depth = game.objs.aliens[player as usize].worldz;
            game.vars.player_posz = depth;
            original.word(0, "PLAYER_POSZ", depth);
            if visit == 0 {
                original.run("PLAYERCLEARBRIDGE_ISTRAT");
                strat_player_clear_bridge_init(&mut game, player);
            } else {
                original.run("PLAYERCLEARBRIDGE_STRAT");
                let tick = game.objs.aliens[player as usize].stratptr.unwrap();
                game.call_strat(tick, player);
            }
            original.compare(
                &game,
                player,
                &format!("clear start-distance={initial_distance} visit={visit}"),
            );
            assert_eq!(game.vars.strategy.player_bytes[0], 162 - visit as u8);
            assert_eq!(
                game.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE),
                PSF_NOCTRL | PSF_NOFIRE
            );
            assert_eq!(
                game.vars.pstratflags & (PSTF_INSEQ | PSTF_NOVDISTC),
                PSTF_INSEQ | PSTF_NOVDISTC
            );
        }
    }
}

#[test]
fn complete_bridge_clear_retains_locks_through_duplicate_drift_and_boost_handoff() {
    for offset in [-128, -80, -30, 0, 127] {
        verify_complete_bridge_clear(offset);
    }
}

fn verify_complete_bridge_clear(offset: i8) {
    const DUPLICATE: u32 = 0x0700;
    const FLAME: u32 = 0x0900;
    let (mut game, player) = scene();
    let sounds = Sounds::default();
    game.hooks = Box::new(sounds.clone());
    game.vars.strategy.boost_depth_offset = offset;
    let duplicate = game.objs.free_head.unwrap();
    let flame = game.objs.aliens[duplicate as usize].next.unwrap();
    let mut original = Original::new(&game, player);
    original.word(0, "ALLST", OBJECT as i16);
    original.word(0, "ALFREELST", DUPLICATE as i16);
    original.word(DUPLICATE, "_NEXT", FLAME as i16);
    original.word(FLAME, "_PREV", DUPLICATE as i16);
    for visit in 0..224 {
        // Drain the original sound queue before the next strategy visit.
        original.byte(0, "SDSPT3", 0);
        sounds.0.borrow_mut().clear();
        game.vars.gameframe = visit;
        original.word(0, "GAMEFRAME", visit as i16);
        game.vars.player_posz = game.objs.aliens[player as usize].worldz;
        original.word(0, "PLAYER_POSZ", game.vars.player_posz);
        if visit == 0 {
            original.run("PLAYERCLEARBRIDGE_ISTRAT");
            strat_player_clear_bridge_init(&mut game, player);
        } else {
            original.run_at(OBJECT, original.strategy(OBJECT));
            game.call_strat(game.objs.aliens[player as usize].stratptr.unwrap(), player);
        }
        original.compare(&game, player, &format!("complete clear {visit}: player"));
        original.compare_actor(
            OBJECT,
            &game.objs.aliens[player as usize],
            "complete player shape",
        );
        if visit >= 163 {
            assert!(game.objs.aliens[duplicate as usize].active);
            original.compare_actor(
                DUPLICATE,
                &game.objs.aliens[duplicate as usize],
                "duplicate entry",
            );
            original.run_at(DUPLICATE, original.strategy(DUPLICATE));
            game.call_strat(
                game.objs.aliens[duplicate as usize].stratptr.unwrap(),
                duplicate,
            );
            original.compare_actor(
                DUPLICATE,
                &game.objs.aliens[duplicate as usize],
                &format!("duplicate after {visit}"),
            );
            original.compare(
                &game,
                player,
                &format!("complete clear {visit}: shared child effects"),
            );
        }
        original.compare_links(
            &game,
            &[(OBJECT, player), (DUPLICATE, duplicate), (FLAME, flame)],
        );
        if (212..=221).contains(&visit) {
            original.compare_actor(FLAME, &game.objs.aliens[flame as usize], "flame before");
            original.run_at(FLAME, original.strategy(FLAME));
            game.objs.aldead = 0;
            game.call_strat(game.objs.aliens[flame as usize].stratptr.unwrap(), flame);
            original.compare_actor(
                FLAME,
                &game.objs.aliens[flame as usize],
                &format!("flame after {visit}"),
            );
            let dead = original.bus.read8(WRAM | original.names["ALDEAD"]);
            assert_eq!(game.objs.aldead, dead, "flame retirement {visit}");
            assert_eq!(dead != 0, visit == 221);
            assert_eq!(game.objs.aliens[flame as usize].count, 221 - visit as u8);
            if dead != 0 {
                original.run_at(FLAME, original.names["REMOVEDEADAL_L"]);
                game.objs.free(flame);
                original.compare_links(
                    &game,
                    &[(OBJECT, player), (DUPLICATE, duplicate), (FLAME, flame)],
                );
            }
        }
        let sound_count = original.bus.read8(WRAM | original.names["SDSPT3"]);
        let expected: Vec<_> = (0..sound_count)
            .map(|index| {
                original
                    .bus
                    .read8((WRAM | original.names["SDPORT3"]) + u32::from(index))
            })
            .collect();
        assert_eq!(*sounds.0.borrow(), expected, "sound visit {visit}");
        assert_eq!(expected == [50], visit == 212, "boost handoff sound");
    }
}

#[test]
fn map_callback_only_schedules_clear_initializer_like_original() {
    let (mut game, player) = scene();
    sf_strat::table::register_all(&mut game);
    game.vars.pshipflags = 0;
    game.vars.pstratflags = 0;
    game.vars.strategy.player_bytes[0] = 77;
    let mut original = Original::new(&game, player);
    original.run("SET_PLAYERCLEARBRIDGE_L");

    let [low, high, bank, _] = (sf_map::consts::cb::SET_PLAYER_CLEAR_BRIDGE_L - 1).to_le_bytes();
    game.world.map = vec![sf_game::world::op::CODEJSL, low, high, bank];
    game.world.map_loaded = true;
    game.vars.mapptr = 0;
    game.map_exec();
    original.compare(&game, player, "scheduled, not initialized");
    assert_eq!(
        original.strategy(OBJECT),
        original.names["PLAYERCLEARBRIDGE_ISTRAT"]
    );
    original.run_at(OBJECT, original.strategy(OBJECT));
    game.call_strat(game.objs.aliens[player as usize].stratptr.unwrap(), player);
    original.compare(&game, player, "first scheduled visit");
}
