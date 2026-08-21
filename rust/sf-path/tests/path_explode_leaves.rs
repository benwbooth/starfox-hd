//! Path explode leaves: pcoinexplode, explodeparticles/pparticles, robexplode alias.

use sf_path::alien::{Alien, StratRef, ASF_COLLDISABLE, NUMBER_AL};
use sf_path::builder::{PAL_HP, PATH_MISSING_OFFSET};
use sf_path::ids::{PATH_ID_EXPLODEPARTICLES, PATH_ID_PCOINEXPLODE, PATH_ID_ROBEXPLODE};
use sf_path::interp::{
    strat_path_init, strat_path_tick, PathHost, PathWorld, PATH_TRIGGER_WHENDEAD,
};
use sf_path::literals;
use sf_path::opcodes::{PSFLAG6_SMOKE, P_ALWAYS, P_COLLISIONSOFF, P_END, P_SETB, P_SMOKEON};

struct Host {
    exploded: Vec<u16>,
    allocs: usize,
}
impl Host {
    fn new() -> Self {
        Host {
            exploded: Vec::new(),
            allocs: 0,
        }
    }
}
impl PathHost for Host {
    fn random(&mut self) -> u16 {
        0
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
    fn explode(&mut self, _: &mut PathWorld, idx: u16) {
        self.exploded.push(idx);
    }
    fn obj_alloc(&mut self, world: &mut PathWorld) -> Option<u16> {
        for i in 1..NUMBER_AL {
            if !world.aliens[i].active {
                world.aliens[i] = Alien::default();
                world.aliens[i].active = true;
                self.allocs += 1;
                return Some(i as u16);
            }
        }
        None
    }
    fn obj_free(&mut self, _: &mut PathWorld, _: u16) {}
    fn player(&mut self, _: &PathWorld) -> Option<u16> {
        None
    }
    fn run_inline(&mut self, _: &mut PathWorld, _: u16, _: u16) {}
    fn run_external_strat(&mut self, _: &mut PathWorld, _: u16, _: StratRef) {}
}

fn load_catalog(world: &mut PathWorld) {
    let cat = literals::get_catalog();
    world.paths_init();
    world.paths_load_data(cat.data.clone(), cat.offsets.clone());
}

fn spawn_at(world: &mut PathWorld, ip: u16) -> usize {
    let i = 0usize;
    world.aliens[i] = Alien::default();
    world.aliens[i].active = true;
    strat_path_init(&mut world.aliens[i]);
    world.aliens[i].sword2 = ip as i16;
    world.aliens[i].worldy = 0;
    world.aliens[i].rotz = 0;
    world.aliens[i].hp = 10;
    i
}

/// Layout of `pcoinexplode` prologue: COLLOFF(1)+SETB(3)+FORCE(3)+RETURN(1)=8,
/// then `.coin` begins at SMOKEON.
fn pcoinexplode_coin_ip(cat: &literals::PathCatalog) -> u16 {
    let start = cat.offsets[PATH_ID_PCOINEXPLODE as usize];
    assert_ne!(start, PATH_MISSING_OFFSET);
    assert_eq!(cat.data[start as usize], P_COLLISIONSOFF);
    let coin = start.wrapping_add(8);
    assert_eq!(
        cat.data[coin as usize], P_SMOKEON,
        "coin body starts with smoke"
    );
    coin
}

/// `.coin` body: smoke, lift 5×−50, explode.
#[test]
fn pcoinexplode_coin_lifts_and_explodes() {
    let cat = literals::get_catalog();
    let coin = pcoinexplode_coin_ip(cat);
    let mut world = PathWorld::new();
    load_catalog(&mut world);
    let i = spawn_at(&mut world, coin);
    let mut host = Host::new();

    for _ in 0..20 {
        if !host.exploded.is_empty() {
            break;
        }
        strat_path_tick(&mut world, &mut host, i as u16);
    }
    assert_eq!(host.exploded, vec![0]);
    assert_ne!(world.aliens[i].sflags3 & PSFLAG6_SMOKE, 0);
    assert_eq!(world.aliens[i].worldy, -250);
}

/// WHENDEAD harness: register `pcoinexplode`, zero hp, then tick until explode.
#[test]
fn pcoinexplode_when_dead_trigger_explodes() {
    let cat = literals::get_catalog();
    let coin_path = cat.offsets[PATH_ID_PCOINEXPLODE as usize];
    assert_ne!(coin_path, PATH_MISSING_OFFSET);

    let mut data = cat.data.clone();
    let mut offsets = cat.offsets.clone();
    let harness = data.len() as u16;
    // P_ALWAYS <addr:16> WHENDEAD ; P_SETB hp,0 ; P_END
    data.push(P_ALWAYS);
    data.push((coin_path & 0xff) as u8);
    data.push((coin_path >> 8) as u8);
    data.push(PATH_TRIGGER_WHENDEAD);
    data.push(P_SETB);
    data.push(0); // hp = 0
    data.push(PAL_HP as u8);
    data.push(P_END);
    offsets.push(harness);

    let mut world = PathWorld::new();
    world.paths_init();
    world.paths_load_data(data, offsets);
    let i = spawn_at(&mut world, harness);
    world.aliens[i].hp = 10; // set before first tick; SETB zeros it
    let mut host = Host::new();

    // Tick 1: register trigger + set hp=0 + end → move phase runs triggers.
    strat_path_tick(&mut world, &mut host, i as u16);
    // Trigger FORCE may have redirected sword2; keep ticking until explode.
    for _ in 0..30 {
        if !host.exploded.is_empty() {
            break;
        }
        // Keep hp at 0 so WHENDEAD stays armed if re-checked.
        world.aliens[i].hp = 0;
        strat_path_tick(&mut world, &mut host, i as u16);
    }
    assert!(
        !host.exploded.is_empty(),
        "WHENDEAD pcoinexplode should explode"
    );
    assert_ne!(world.aliens[i].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn explodeparticles_spawns_particle_then_explodes() {
    let mut world = PathWorld::new();
    load_catalog(&mut world);
    let off = world.paths_resolve_start(PATH_ID_EXPLODEPARTICLES);
    assert_ne!(off, 0);
    let i = spawn_at(&mut world, off);
    let mut host = Host::new();
    strat_path_tick(&mut world, &mut host, i as u16);
    assert_eq!(host.allocs, 1, "P_PARTICLES allocates one child");
    assert_eq!(host.exploded, vec![0]);
    let child = (1..NUMBER_AL)
        .find(|&j| world.aliens[j].active && j != i)
        .expect("particle child");
    assert_eq!(
        world.aliens[child].stratptr,
        Some(StratRef::ParticleExplodeIstrat)
    );
}

#[test]
fn robexplode_path_is_catalogued_as_probexplode_alias() {
    let cat = literals::get_catalog();
    let off = cat.offsets[PATH_ID_ROBEXPLODE as usize];
    assert_ne!(off, PATH_MISSING_OFFSET);
    assert_eq!(cat.data[off as usize], P_COLLISIONSOFF);
}

#[test]
fn pcoinexplode_and_particles_offsets_present() {
    let cat = literals::get_catalog();
    assert_ne!(
        cat.offsets[PATH_ID_PCOINEXPLODE as usize],
        PATH_MISSING_OFFSET
    );
    assert_ne!(
        cat.offsets[PATH_ID_EXPLODEPARTICLES as usize],
        PATH_MISSING_OFFSET
    );
}
