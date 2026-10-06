//! PATHS.ASM .fire/.fireatplayer/.fireatshape and their CANHIT variants.
//! This checks the opcode boundary, not the individual weapon constructors.

use sf_path::alien::{Alien, StratRef, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE4};
use sf_path::interp::{strat_path_init, strat_path_tick, PathHost, PathWorld};
use sf_path::opcodes::{
    P_END, P_FIRE, P_FIREATPLAYER, P_FIREATPLAYERCANHIT, P_FIREATSHAPE, P_FIREATSHAPECANHIT,
    P_FIRECANHIT, P_WAIT1,
};

const FIRER: u16 = 3;
const TARGET: u16 = 0;
const SHOT: u16 = 5;
const OWNER_IMMUNITY: u16 = FIRER + 1;
const INITIAL_TARGET: u16 = 12;

struct FireHost {
    succeeds: bool,
    initial_flags: u8,
    calls: usize,
}

impl PathHost for FireHost {
    fn random(&mut self) -> u16 {
        panic!("unexpected random draw")
    }
    fn trig_se(&mut self, _: u8) {
        panic!("unexpected sound")
    }
    fn send_message(&mut self, _: u8) {
        panic!("unexpected message")
    }
    fn find_strategy_address(&mut self, _: u32) -> Option<StratRef> {
        panic!("unexpected strategy lookup")
    }
    fn genvecs_2d(&mut self, _: &mut Alien) {
        panic!("unexpected vector update")
    }
    fn genvecs_3d(&mut self, _: &mut Alien) {
        panic!("unexpected vector update")
    }
    fn chase8(&mut self, _: u8, _: u8, _: u8) -> u8 {
        panic!("unexpected chase")
    }
    fn chase16(&mut self, _: i16, _: i16, _: i16) -> i16 {
        panic!("unexpected chase")
    }
    fn angle_xz(&mut self, _: &Alien, _: &Alien) -> u8 {
        panic!("unexpected aiming")
    }
    fn apply_velocity(&mut self, al: &mut Alien) {
        assert_eq!((al.vx, al.vy, al.vz), (0, 0, 0));
    }
    fn hit_flash(&mut self, _: &mut PathWorld, _: u16) {
        panic!("unexpected hit")
    }
    fn init_obj_vars(&mut self, _: &mut Alien) {
        panic!("unexpected initialization")
    }
    fn spawn_projectile(
        &mut self,
        world: &mut PathWorld,
        owner: u16,
        x: i16,
        y: i16,
        z: i16,
        pitch: u8,
        yaw: u8,
        speed: u8,
        lifetime: u8,
        ap: u8,
        collision_class: u8,
    ) -> Option<u16> {
        self.calls += 1;
        assert_eq!(owner, FIRER);
        assert_eq!((x, y, z, pitch, yaw), (0, 0, 0, 7, 249));
        assert_eq!((speed, lifetime, ap), (48, 50, 2));
        // ENEMY1 is applied after the weapon constructor returns.
        assert_eq!(collision_class, ACF_COLLTYPE4);
        if !self.succeeds {
            return None;
        }
        world.aliens[usize::from(SHOT)] = Alien {
            active: true,
            collflags: self.initial_flags,
            immuneptr: OWNER_IMMUNITY,
            ptr: INITIAL_TARGET,
            ..Alien::default()
        };
        Some(SHOT)
    }
    fn explode(&mut self, _: &mut PathWorld, _: u16) {
        panic!("unexpected explosion")
    }
    fn obj_alloc(&mut self, _: &mut PathWorld) -> Option<u16> {
        panic!("unexpected allocation")
    }
    fn obj_free(&mut self, _: &mut PathWorld, _: u16) {
        panic!("unexpected retirement")
    }
    fn player(&mut self, world: &PathWorld) -> Option<u16> {
        world.aliens[usize::from(TARGET)].active.then_some(TARGET)
    }
    fn run_inline(&mut self, _: &mut PathWorld, _: u16, _: u16) {
        panic!("unexpected callback")
    }
    fn run_external_strat(&mut self, _: &mut PathWorld, _: u16, _: StratRef) {
        panic!("unexpected strategy")
    }
}

#[test]
fn every_fire_opcode_preserves_constructor_flags_and_immunity() {
    for (opcode, adds_enemy_class, targeted) in [
        (P_FIRE, true, false),
        (P_FIRECANHIT, false, false),
        (P_FIREATPLAYER, true, true),
        (P_FIREATPLAYERCANHIT, false, true),
        (P_FIREATSHAPE, true, true),
        (P_FIREATSHAPECANHIT, false, true),
    ] {
        for target_active in [false, true] {
            for succeeds in [false, true] {
                // CANHIT omits an OR; it must not clear an existing ENEMY1 bit.
                for inherited_enemy_class in [0, ACF_COLLTYPE2] {
                    let mut world = PathWorld::new();
                    world.paths_load_data(vec![opcode, P_WAIT1, P_END], vec![0]);
                    world.aliens[usize::from(TARGET)].active = target_active;
                    let firer = &mut world.aliens[usize::from(FIRER)];
                    firer.active = true;
                    firer.ptr = TARGET + 1; // Slot zero is a valid target.
                    firer.rotx = 7;
                    firer.roty = 249;
                    strat_path_init(firer);
                    let initial_flags = ACF_COLLTYPE1 | ACF_COLLTYPE4 | inherited_enemy_class;
                    let mut host = FireHost {
                        succeeds,
                        initial_flags,
                        calls: 0,
                    };
                    strat_path_tick(&mut world, &mut host, FIRER);
                    assert_eq!(host.calls, 1, "opcode {opcode}");
                    assert_eq!(world.aliens[usize::from(FIRER)].sword2, 2);
                    assert_eq!(world.aldead, 0);
                    let shot = &world.aliens[usize::from(SHOT)];
                    if succeeds {
                        let extra = if adds_enemy_class { ACF_COLLTYPE2 } else { 0 };
                        assert_eq!(shot.collflags, initial_flags | extra, "opcode {opcode}");
                        assert_eq!(shot.immuneptr, OWNER_IMMUNITY, "opcode {opcode}");
                        let target = if targeted && target_active {
                            TARGET + 1
                        } else {
                            INITIAL_TARGET
                        };
                        assert_eq!(shot.ptr, target, "opcode {opcode}");
                    } else {
                        assert_eq!(*shot, Alien::default(), "failed opcode {opcode}");
                    }
                }
            }
        }
    }
}
