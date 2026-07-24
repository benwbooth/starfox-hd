use sf_game::alien::{
    ObjectVisualKind, AFEXP, AFONFIRE, ASF4_PLAYEROBJ, ASF_COLLDISABLE, ASF_HITFLASH, ASF_SHADOW,
};
use sf_game::draw::AF_INVIEW_PL;
use sf_game::{Game, Hooks};
use sf_strat::enemy_a::{strat_explode, ASF2_NOEXPSND};
use std::cell::RefCell;
use std::rc::Rc;

const PLAYER_SHIP_SHAPE: u16 = 2;
const MEDIUM_EXPLOSION_SPRITE_SHAPE: u16 = 462;
const MEDIUM_EXPLOSION_POLYGON_SHAPE: u16 = 466;
const MEDIUM_EXPLOSION_SPRITE_TICKS: u8 = 6;
const POLYGON_EXPLOSION_TICKS: u8 = 12;
const PLAYER_SPRITE_SCALE_ADJUSTMENT: u8 = 253;
const NEAR_DESTRUCTION_SOUND: u8 = 33;
const DESTROYED_POSITION: [i16; 3] = [100, -50, 500];
const DESTROYED_VELOCITY: [i16; 3] = [7, -3, 11];

#[derive(Clone, Default)]
struct SoundLog(Rc<RefCell<Vec<u8>>>);

impl Hooks for SoundLog {
    fn play_se(&mut self, id: u8) {
        self.0.borrow_mut().push(id);
    }
}

#[test]
fn player_sized_explosion_keeps_polygon_debris_and_a_scaled_sprite_alive() {
    let sounds = Rc::new(RefCell::new(Vec::new()));
    let mut game = Game::with_hooks(Box::new(SoundLog(sounds.clone())));
    let player = game.objs.alloc().expect("player");
    assert_eq!(player, 0);
    game.objs.aliens[player as usize].active = true;
    game.objs.aliens[player as usize].sflags4 |= ASF4_PLAYEROBJ;

    let destroyed = game.objs.alloc().expect("destroyed object");
    let fire = game.objs.alloc().expect("attached fire");
    {
        let object = &mut game.objs.aliens[destroyed as usize];
        object.shape = PLAYER_SHIP_SHAPE;
        object.flags = AF_INVIEW_PL | AFONFIRE;
        object.sflags = ASF_HITFLASH | ASF_SHADOW;
        object.sflags2 = 0;
        [object.worldx, object.worldy, object.worldz] = DESTROYED_POSITION;
        [object.vx, object.vy, object.vz] = DESTROYED_VELOCITY;
        object.fireobjptr = fire + 1;
    }

    strat_explode(&mut game, destroyed);

    assert_eq!(*sounds.borrow(), vec![NEAR_DESTRUCTION_SOUND]);
    assert!(!game.objs.aliens[fire as usize].active);
    let polygon = game.objs.aliens[destroyed as usize];
    assert_ne!(polygon.flags & AFEXP, 0);
    assert_eq!(polygon.shape, MEDIUM_EXPLOSION_POLYGON_SHAPE);
    assert_eq!(polygon.count, 0);
    assert_eq!(polygon.count1, POLYGON_EXPLOSION_TICKS);
    assert_eq!(polygon.hp, 0);
    assert_ne!(polygon.sflags & ASF_COLLDISABLE, 0);
    assert_eq!(polygon.sflags & ASF_SHADOW, ASF_SHADOW);
    assert!(polygon.expstratptr.is_some());

    let sprite = game
        .objs
        .active_indices()
        .into_iter()
        .find(|slot| *slot != player && *slot != destroyed)
        .expect("explosion sprite");
    let sprite_object = game.objs.aliens[sprite as usize];
    assert_eq!(sprite_object.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(sprite_object.shape, MEDIUM_EXPLOSION_SPRITE_SHAPE);
    assert_eq!(sprite_object.tx, PLAYER_SPRITE_SCALE_ADJUSTMENT);
    assert_eq!(sprite_object.count, 0);
    assert_eq!(sprite_object.count1, MEDIUM_EXPLOSION_SPRITE_TICKS);
    assert_eq!(sprite_object.sflags & (ASF_HITFLASH | ASF_SHADOW), 0);
    assert_eq!(
        [
            sprite_object.worldx,
            sprite_object.worldy,
            sprite_object.worldz,
        ],
        DESTROYED_POSITION
    );
    assert_eq!(
        [sprite_object.vx, sprite_object.vy, sprite_object.vz],
        DESTROYED_VELOCITY
    );

    for _ in 0..MEDIUM_EXPLOSION_SPRITE_TICKS {
        game.run_strategies();
    }
    assert!(!game.objs.aliens[sprite as usize].active);
    assert!(game.objs.aliens[destroyed as usize].active);
    assert_eq!(
        game.objs.aliens[destroyed as usize].count,
        MEDIUM_EXPLOSION_SPRITE_TICKS
    );

    for _ in MEDIUM_EXPLOSION_SPRITE_TICKS..POLYGON_EXPLOSION_TICKS {
        game.run_strategies();
    }
    assert!(!game.objs.aliens[destroyed as usize].active);
}

#[test]
fn no_polygon_explosion_removes_only_the_destroyed_mesh() {
    let mut game = Game::new();
    let destroyed = game.objs.alloc().expect("destroyed object");
    {
        let object = &mut game.objs.aliens[destroyed as usize];
        object.shape = PLAYER_SHIP_SHAPE;
        object.flags = AF_INVIEW_PL;
        object.sflags2 = ASF2_NOEXPSND;
        object.sflags4 = sf_strat::enemy_a::ASF4_NOPOLYEXP;
    }

    strat_explode(&mut game, destroyed);

    assert_eq!(game.objs.aldead, 1);
    assert!(game.objs.active_indices().into_iter().any(|slot| {
        slot != destroyed
            && game.objs.aliens[slot as usize].visual_kind == ObjectVisualKind::ScaledSprite
    }));
}
