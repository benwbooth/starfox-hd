//! P_FRIEND `friend_anyone` weighted-selection parity (ROM PATHS.ASM:1262-1321,
//! randomselectmode=0 per GAME.INC:67).
//!
//! The ROM picks a *living* wingman with fixed weights, not uniformly, and the
//! number of `random_l` draws depends on who is alive (the RNG is a shared
//! stream). This drives a one-opcode path (`P_FRIEND friend_anyone; P_END`)
//! with a scripted RNG and asserts the chosen `al_sbyte4` and the draw count.

use sf_path::alien::{Alien, StratRef, NUMBER_AL};
use sf_path::interp::{
    strat_path_init, strat_path_tick, PathHost, PathWorld, FRIEND_ANYONE, FRIEND_FALCON,
    FRIEND_FROG, FRIEND_RABBIT,
};
use sf_path::opcodes::{P_END, P_FRIEND};

/// Minimal host: only `random()` matters for P_FRIEND; the rest is unreachable
/// for a `P_FRIEND; P_END` script.
struct Host {
    seq: Vec<u16>,
    idx: usize,
    draws: usize,
}
impl Host {
    fn new(seq: &[u16]) -> Self {
        Host {
            seq: seq.to_vec(),
            idx: 0,
            draws: 0,
        }
    }
}
impl PathHost for Host {
    fn random(&mut self) -> u16 {
        let v = self.seq.get(self.idx).copied().unwrap_or(0);
        self.idx += 1;
        self.draws += 1;
        v
    }
    fn trig_se(&mut self, _: u8) {}
    fn send_message(&mut self, _: u8) {}
    fn find_strategy_address(&mut self, _: u32) -> Option<StratRef> {
        None
    }
    fn genvecs_2d(&mut self, _: &mut Alien) {}
    fn genvecs_3d(&mut self, _: &mut Alien) {}
    fn chase8(&mut self, c: u8, _: u8, _: u8) -> u8 {
        c
    }
    fn chase16(&mut self, c: i16, _: i16, _: i16) -> i16 {
        c
    }
    fn angle_xz(&mut self, _: &Alien, _: &Alien) -> u8 {
        0
    }
    fn apply_velocity(&mut self, _: &mut Alien) {}
    fn hit_flash(&mut self, _: &mut PathWorld, _: u16) {}
    fn init_obj_vars(&mut self, _: &mut Alien) {}
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        _: &mut PathWorld,
        _: u16,
        _: i16,
        _: i16,
        _: i16,
        _: u8,
        _: u8,
        _: u8,
        _: u8,
        _: u8,
        _: u8,
    ) -> Option<u16> {
        None
    }
    fn explode(&mut self, _: &mut PathWorld, _: u16) {}
    fn obj_alloc(&mut self, _: &mut PathWorld) -> Option<u16> {
        None
    }
    fn obj_free(&mut self, _: &mut PathWorld, _: u16) {}
    fn player(&mut self, _: &PathWorld) -> Option<u16> {
        None
    }
    fn run_inline(&mut self, _: &mut PathWorld, _: u16, _: u16) {}
    fn run_external_strat(&mut self, _: &mut PathWorld, _: u16, _: StratRef) {}
}

/// Run `P_FRIEND friend_anyone; P_END` on a single object with the given
/// wingman HP and scripted RNG; return `(selected_sbyte4, rng_draws)`.
fn run_friend(falcon: u8, rabbit: u8, frog: u8, rng: &[u16]) -> (u8, usize) {
    let mut world = PathWorld::new();
    world.paths_init();
    world.paths_load_data(vec![P_FRIEND, FRIEND_ANYONE, P_END], vec![0]);
    world.falcon_hp = falcon;
    world.bunny_hp = rabbit;
    world.frog_hp = frog;

    // Object 0 runs the path.
    let i = 0usize;
    assert!(i < NUMBER_AL);
    world.aliens[i] = Alien::default();
    world.aliens[i].active = true;
    strat_path_init(&mut world.aliens[i]);
    world.aliens[i].sbyte4 = 0xEE; // sentinel: "unchanged"
    world.aliens[i].sword2 = 0;

    let mut host = Host::new(rng);
    strat_path_tick(&mut world, &mut host, i as u16);
    (world.aliens[i].sbyte4, host.draws)
}

#[test]
fn all_alive_is_weighted_20_40_40() {
    // ROM .404020frogbunnycock: r<50 falcon, r<150 rabbit, else frog. One draw.
    assert_eq!(run_friend(40, 40, 40, &[0]), (FRIEND_FALCON, 1)); // r=0 -> <50
    assert_eq!(run_friend(40, 40, 40, &[49]), (FRIEND_FALCON, 1)); // boundary <50
    assert_eq!(run_friend(40, 40, 40, &[50]), (FRIEND_RABBIT, 1)); // 50 -> rabbit
    assert_eq!(run_friend(40, 40, 40, &[149]), (FRIEND_RABBIT, 1)); // <150
    assert_eq!(run_friend(40, 40, 40, &[150]), (FRIEND_FROG, 1)); // >=150 -> frog
    assert_eq!(run_friend(40, 40, 40, &[255]), (FRIEND_FROG, 1));
    // Low byte only: 0x0100 has low byte 0 -> falcon.
    assert_eq!(run_friend(40, 40, 40, &[0x0100]), (FRIEND_FALCON, 1));
}

#[test]
fn falcon_and_frog_only_threshold_102() {
    // ROM .4060cockfrog: s_jmp_random .cock,40 -> threshold 40*255/100 = 102.
    assert_eq!(run_friend(40, 0, 40, &[101]), (FRIEND_FALCON, 1));
    assert_eq!(run_friend(40, 0, 40, &[102]), (FRIEND_FROG, 1));
}

#[test]
fn falcon_and_rabbit_only_threshold_102() {
    // ROM .4060cockbunny: s_jmp_random .cock,40 -> threshold 102.
    assert_eq!(run_friend(40, 40, 0, &[101]), (FRIEND_FALCON, 1));
    assert_eq!(run_friend(40, 40, 0, &[102]), (FRIEND_RABBIT, 1));
}

#[test]
fn frog_and_rabbit_only_threshold_127() {
    // ROM .5050bunnyfrog: s_jmp_random .frog (default 50) -> 50*255/100 = 127.
    assert_eq!(run_friend(0, 40, 40, &[126]), (FRIEND_FROG, 1));
    assert_eq!(run_friend(0, 40, 40, &[127]), (FRIEND_RABBIT, 1));
}

#[test]
fn single_survivor_no_draw() {
    // Only one wingman alive -> deterministic pick, ZERO random draws
    // (matches the ROM control flow, which never reaches s_jmp_random).
    assert_eq!(run_friend(40, 0, 0, &[]), (FRIEND_FALCON, 0));
    assert_eq!(run_friend(0, 40, 0, &[]), (FRIEND_RABBIT, 0));
    assert_eq!(run_friend(0, 0, 40, &[]), (FRIEND_FROG, 0));
}

#[test]
fn nobody_alive_leaves_sbyte4_unchanged_no_draw() {
    // ROM .nonealive: sbyte4 is left as-is, no draw.
    assert_eq!(run_friend(0, 0, 0, &[]), (0xEE, 0));
}
