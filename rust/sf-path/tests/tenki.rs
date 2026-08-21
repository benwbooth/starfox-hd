//! Titania weather paths, transcribed from PATHDATA.ASM:198-226.

use sf_path::alien::{Alien, ObjectVisualKind, StratRef, ASF_COLLDISABLE, ATZREMOVE};
use sf_path::ids::{PATH_ID_TENKI_DM, PATH_ID_TENKI_ON};
use sf_path::interp::{strat_path_init, strat_path_tick, PathHost, PathWorld};
use sf_path::literals;
use sf_path::opcodes::{P_END, P_SPRITE};

#[derive(Default)]
struct Host {
    sounds: Vec<u8>,
}

impl PathHost for Host {
    fn random(&mut self) -> u16 {
        0
    }
    fn trig_se(&mut self, sound_id: u8) {
        self.sounds.push(sound_id);
    }
    fn send_message(&mut self, _: u8) {}
    fn find_strategy_address(&mut self, _: u32) -> Option<StratRef> {
        None
    }
    fn genvecs_2d(&mut self, _: &mut Alien) {}
    fn genvecs_3d(&mut self, _: &mut Alien) {}
    fn chase8(&mut self, current: u8, _: u8, _: u8) -> u8 {
        current
    }
    fn chase16(&mut self, current: i16, _: i16, _: i16) -> i16 {
        current
    }
    fn angle_xz(&mut self, _: &Alien, _: &Alien) -> u8 {
        0
    }
    fn apply_velocity(&mut self, _: &mut Alien) {}
    fn hit_flash(&mut self, _: &mut PathWorld, _: u16) {}
    fn init_obj_vars(&mut self, _: &mut Alien) {}
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
    fn player(&mut self, world: &PathWorld) -> Option<u16> {
        world.aliens[0].active.then_some(0)
    }
    fn run_inline(&mut self, _: &mut PathWorld, _: u16, _: u16) {}
    fn run_external_strat(&mut self, _: &mut PathWorld, _: u16, _: StratRef) {}
}

fn load() -> PathWorld {
    let cat = literals::get_catalog();
    let mut world = PathWorld::new();
    world.paths_init();
    world.paths_load_data(cat.data.clone(), cat.offsets.clone());
    world
}

fn spawn_path(world: &mut PathWorld, idx: usize, path_id: u16) {
    world.aliens[idx].active = true;
    strat_path_init(&mut world.aliens[idx]);
    world.aliens[idx].sword2 = world.paths_resolve_start(path_id) as i16;
}

#[test]
fn tenki_on_raises_fog_latch_only_when_player_is_inside_radius() {
    let mut world = load();
    let mut host = Host::default();
    world.aliens[0].active = true;
    world.aliens[0].worldx = 500;
    world.aliens[0].worldy = 0;
    world.aliens[0].worldz = 0;
    spawn_path(&mut world, 1, PATH_ID_TENKI_ON);

    strat_path_tick(&mut world, &mut host, 1);
    assert_eq!(world.ebyte3, 0);
    assert!(world.aliens[1].sflags & ASF_COLLDISABLE != 0);
    assert!(world.aliens[1].type_ & ATZREMOVE != 0);
    assert_eq!(world.aliens[1].roty, 128);
    assert!(host.sounds.is_empty());

    world.aliens[0].worldx = 100;
    strat_path_tick(&mut world, &mut host, 1);
    assert_eq!(world.ebyte3, 1);
    assert_eq!(world.aliens[1].pbyte1, 1);
    assert_eq!(host.sounds, vec![0x12]);

    // P_WAIT 3: two held ticks, then the third resumes at P_REMOVE.
    for _ in 0..3 {
        strat_path_tick(&mut world, &mut host, 1);
    }
    assert_eq!(world.aldead, 1);
}

#[test]
fn tenki_dm_is_inert_collisionless_and_turns_away() {
    let mut world = load();
    let mut host = Host::default();
    spawn_path(&mut world, 1, PATH_ID_TENKI_DM);

    strat_path_tick(&mut world, &mut host, 1);

    assert!(world.aliens[1].sflags & ASF_COLLDISABLE != 0);
    assert_eq!(world.aliens[1].roty, 128);
    assert!(world.aliens[1].type_ & ATZREMOVE != 0);
    assert!(host.sounds.is_empty());
    assert_eq!(world.aldead, 0);
}

#[test]
fn sprite_opcode_sets_typed_presentation_fields() {
    const NEGATIVE_TWO_AS_BYTE: u8 = 254;
    const SPRITE_DEPTH_COLOUR: i16 = -2;
    const SPRITE_SIZE: u8 = 12;

    let mut world = PathWorld::new();
    world.paths_load_data(
        vec![P_SPRITE, NEGATIVE_TWO_AS_BYTE, SPRITE_SIZE, P_END],
        vec![0],
    );
    let mut host = Host::default();
    spawn_path(&mut world, 1, 0);

    strat_path_tick(&mut world, &mut host, 1);

    let object = &world.aliens[1];
    assert_eq!(object.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(object.depthoffset, SPRITE_DEPTH_COLOUR);
    assert_eq!(object.tx, SPRITE_SIZE);
}
