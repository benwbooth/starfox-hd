//! Tick 103: makeengine / makesplash / makeSsplash / makeSdrag (GSTRATS / GA2STRAT).

use sf_game::alien::{ObjectVisualKind, AFONFIRE, ASF2_COLLDISABLE, ASF3_REALOBJ, ASF_INVISIBLE};
use sf_game::Game;
use sf_strat::common::{
    makeengine_srou, makeengine_srou_with_extents, makesplash_srou, makessplash_srou, splash_strat,
    updateengine_srou,
};
use sf_strat::enemy_a::make_sdrag;

const DEFAULT_ENGINE_SPRITE_SIZE: u8 = 24;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn makesplash_spawns_colldisable_child() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    g.objs.aliens[parent as usize].worldz = 1000;
    let splash = makesplash_srou(&mut g, parent).expect("splash");
    let al = &g.objs.aliens[splash as usize];
    assert_eq!(al.shape, 360, "makesplash must use the retail splash mesh");
    assert_ne!(al.sflags2 & ASF2_COLLDISABLE, 0);
    assert_eq!(al.sflags3 & ASF3_REALOBJ, 0);
    assert_eq!(al.worldz, 995); // parent z - 5
    assert!(al.stratptr.is_some());
    assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(al.depthoffset, 0);
    assert_eq!(al.tx, 0);
}

#[test]
fn makessplash_same_path() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let splash = makessplash_srou(&mut g, parent).expect("small splash");
    assert_eq!(g.objs.aliens[splash as usize].shape, 359);
    assert_eq!(
        g.objs.aliens[splash as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
}

#[test]
fn splash_strat_removes_after_colanim() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    let splash = makesplash_srou(&mut g, parent).expect("splash");
    g.objs.aldead = 0;
    for _ in 0..10 {
        splash_strat(&mut g, splash);
        if g.objs.aldead != 0 {
            break;
        }
    }
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn makeengine_attaches_fireobj_and_onfire() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    g.objs.aliens[parent as usize].worldx = 10;
    g.objs.aliens[parent as usize].worldy = 20;
    g.objs.aliens[parent as usize].worldz = 30;
    g.objs.aliens[parent as usize].roty = 0;
    g.objs.aliens[parent as usize].rotx = 0;
    let engine = makeengine_srou(&mut g, parent).expect("engine");
    {
        let p = &g.objs.aliens[parent as usize];
        assert_eq!(p.fireobjptr, engine.wrapping_add(1));
        assert_ne!(p.flags & AFONFIRE, 0);
    }
    let al = &g.objs.aliens[engine as usize];
    assert_eq!(al.shape, 362, "engine must use the retail boostshape mesh");
    assert_ne!(al.sflags2 & ASF2_COLLDISABLE, 0);
    assert_ne!(al.sflags & ASF_INVISIBLE, 0); // hidden until update
                                              // Default zmax=40 → relposz = -40
    assert_eq!(al.relposz as i8, -40);
    assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(al.depthoffset, 0, "source color argument is zero");
    assert_eq!(
        al.tx, DEFAULT_ENGINE_SPRITE_SIZE,
        "source size is sh_Ymax - 24"
    );
}

#[test]
fn updateengine_places_behind_parent() {
    let mut g = Game::new();
    let parent = spawn(&mut g);
    g.objs.aliens[parent as usize].worldz = 500;
    g.objs.aliens[parent as usize].roty = 0;
    g.objs.aliens[parent as usize].rotx = 0;
    let engine = makeengine_srou_with_extents(&mut g, parent, 48, 40).expect("e");
    assert!(updateengine_srou(&mut g, parent));
    let al = &g.objs.aliens[engine as usize];
    assert_eq!(al.sflags & ASF_INVISIBLE, 0);
    // Behind parent along -Z when facing forward (roty=0); mulslog may truncate ±2.
    let dz = al.worldz.wrapping_sub(500);
    assert!(
        (-42..=-38).contains(&dz),
        "expected ~-40 behind parent, dz={dz}"
    );
    assert_eq!(al.worldx, g.objs.aliens[parent as usize].worldx);
    assert_eq!(al.tx, DEFAULT_ENGINE_SPRITE_SIZE);
}

#[test]
fn make_sdrag_spawns_sdragonfly() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    g.objs.aliens[mother as usize].worldx = 7;
    let child = make_sdrag(&mut g, mother).expect("sdrag");
    assert_eq!(g.objs.aliens[child as usize].shape, 356);
    assert_eq!(g.objs.aliens[child as usize].worldx, 7);
    assert!(g.objs.aliens[child as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[child as usize].vel, 25);
}
