//! Retail Andross split-link, collision, smoke, and animation regressions.

use sf_game::alien::{AFONFIRE, ASF4_CHILDOBJ, ASF4_MOTHEROBJ, ASF_COLLIDE, ASF_HITFLASH};
use sf_game::{Game, Hooks};
use sf_strat::bossb::{
    bossbentsplit_istrat, bossbentsplit_strat, bossbentsplitcol_istrat, bossbrobcol_istrat,
    bossbrobment_srou, bossbrobvecs_cont4,
};
use std::cell::RefCell;
use std::rc::Rc;

const SPLIT_PARENT_SHUTDOWN_FLAG: u8 = 64;
const SPLIT_HIT_SOUND: u8 = 39;
const WALKING_FORM_SMOKE_THRESHOLD: u8 = 60;
const TOP_HIT_SOUND: u8 = 128;

#[derive(Clone)]
struct SoundLog(Rc<RefCell<Vec<u8>>>);

impl Hooks for SoundLog {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn spawn_player(g: &mut Game) {
    let player = g.objs.alloc().expect("player");
    assert_eq!(player, 0);
    g.objs.aliens[player as usize].active = true;
    g.objs.aliens[player as usize].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_robot(g: &mut Game) -> u16 {
    let robot = g.objs.alloc().expect("Andross robot");
    let object = &mut g.objs.aliens[robot as usize];
    object.active = true;
    object.shape = 76;
    object.worldy = -320;
    object.worldz = 2500;
    robot
}

#[test]
fn split_parts_use_a_plain_object_link_without_corrupting_coordinates() {
    let mut game = Game::new();
    spawn_player(&mut game);
    let mother = spawn_robot(&mut game);
    game.objs.aliens[mother as usize].sword1 = 450;

    let part = bossbrobment_srou(&mut game, mother, 1).expect("split part");
    assert_eq!(game.objs.aliens[part as usize].shape, 76);
    assert_eq!(game.objs.aliens[part as usize].ptr, mother + 1);
    assert_eq!(game.objs.aliens[mother as usize].sword1, 450);
    assert_eq!(
        game.objs.aliens[mother as usize].sflags4 & ASF4_MOTHEROBJ,
        0
    );
    assert_eq!(game.objs.aliens[part as usize].sflags4 & ASF4_CHILDOBJ, 0);
}

#[test]
fn split_part_obeys_parent_shutdown_and_collision_resume() {
    let sounds = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(SoundLog(sounds.clone())));
    spawn_player(&mut game);
    let mother = spawn_robot(&mut game);
    let part = bossbrobment_srou(&mut game, mother, 1).expect("split part");
    bossbentsplit_istrat(&mut game, part);

    game.vars.gameframe = 1;
    game.objs.aliens[part as usize].sbyte1 = 100;
    game.objs.aliens[part as usize].count = 8;
    game.objs.aliens[mother as usize].sflags2 |= SPLIT_PARENT_SHUTDOWN_FLAG;
    bossbentsplit_strat(&mut game, part);
    assert_eq!(game.objs.aliens[part as usize].count, 7);

    game.objs.aliens[mother as usize].sflags2 &= !SPLIT_PARENT_SHUTDOWN_FLAG;
    game.objs.aliens[part as usize].sbyte1 = 50;
    game.objs.aliens[part as usize].sflags |= ASF_COLLIDE;
    bossbentsplitcol_istrat(&mut game, part);
    assert_eq!(*sounds.borrow(), vec![SPLIT_HIT_SOUND]);
    assert_eq!(game.objs.aliens[part as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(game.objs.aliens[part as usize].sbyte1, 49);
}

#[test]
fn damaged_walking_form_emits_smoke_at_the_retail_period() {
    let mut game = Game::new();
    spawn_player(&mut game);
    let robot = spawn_robot(&mut game);
    game.objs.aliens[robot as usize].hp = WALKING_FORM_SMOKE_THRESHOLD;
    game.vars.gameframe = 0;
    let before = game.objs.active_indices().len();

    bossbrobvecs_cont4(&mut game, robot);

    assert_ne!(game.objs.aliens[robot as usize].flags & AFONFIRE, 0);
    assert_eq!(game.objs.active_indices().len(), before + 1);

    game.vars.gameframe = 1;
    bossbrobvecs_cont4(&mut game, robot);
    assert_eq!(game.objs.active_indices().len(), before + 1);
}

#[test]
fn robot_top_collision_spawns_contact_flash_and_resumes() {
    let sounds = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(SoundLog(sounds.clone())));
    spawn_player(&mut game);
    let robot = spawn_robot(&mut game);
    game.objs.aliens[robot as usize].collobjptr = 0;
    game.objs.aliens[robot as usize].hitflags = 1;
    game.objs.aliens[robot as usize].sflags |= ASF_COLLIDE;
    let before = game.objs.active_indices().len();

    bossbrobcol_istrat(&mut game, robot);

    assert_eq!(*sounds.borrow(), vec![TOP_HIT_SOUND]);
    assert_eq!(game.objs.aliens[robot as usize].sflags & ASF_COLLIDE, 0);
    assert_ne!(game.objs.aliens[robot as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(game.objs.aliens[robot as usize].sbyte4, 64);
    assert_eq!(game.objs.active_indices().len(), before + 1);
}
