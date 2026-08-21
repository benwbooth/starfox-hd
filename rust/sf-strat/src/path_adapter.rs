//! Game <-> PathWorld adapter — the last SF1 gameplay-parity wire.
//!
//! C oracle: `src/path/paths.c` runs against the SAME single `g_aliens` pool
//! as every other strategy (`Strat_Path_Init`/`Strat_Path_Tick`, path_istrat).
//! The Rust port split that one pool in two: `sf_game::obj::Objects` (game
//! core, `StratId` registry handles) and `sf_path::interp::PathWorld` (the
//! path VM state + its own `sf_path::alien::Alien` pool with a `StratRef`
//! strategy enum). This module reconciles the two so path-following objects
//! (IS_PATH/IS_PATHT/IS_PATHDHA — friend ships, chasing enemies, many map
//! objects) actually run.
//!
//! ## Design — Option A (bridge pool)
//!
//! `sf_game::Objects` stays canonical for object data and list membership.
//! Each path strategy call:
//!   1. takes the persistent `PathWorld` out of `Game` (`Option::take`, cheap
//!      — no realloc; the VM stacks/triggers persist across ticks, exactly
//!      like the C file-statics),
//!   2. mirrors the whole `Objects` pool + the globals paths.c reads into it
//!      ([`sync_in`], slot-for-slot so every object "pointer" index and the
//!      active-list order match C's single pool),
//!   3. runs the trace-verified `sf_path` interpreter with an [`Adapter`]
//!      [`PathHost`] backed by `Game`'s real systems (RNG/sound/strategy-
//!      address map + `sf_strat::common` math + `strat_spawn_projectile` /
//!      `strat_explode`),
//!   4. writes object data + the friend-HP/latch globals back
//!      ([`sync_out`]), leaving `Objects`' lists untouched (the host mutated
//!      them directly for alloc/free), then
//!   5. returns the `PathWorld` to `Game`.
//!
//! Option B (embed a live PathWorld and mirror spawns continuously) was
//! rejected: the per-call copy is only ~70 slots and keeps `Objects` the
//! single source of truth for the draw list / collision / other lanes, with
//! no dual-ownership of list structure.
//!
//! ## Alien field map
//!
//! Both `Alien` structs model the C `Alien` (`src/game/obj.h`) field-for-field.
//! Their strategy identities and visual-kind enums use crate-local nominal
//! types, so [`copy_g2p`]/[`copy_p2g_data`] map those typed fields explicitly
//! while copying every other data field by C name. Strategy slots round-trip
//! through [`id2ref`]/[`ref2id`]: the path-lane routines (PathTick,
//! PathOnCollision, Explode, ParticleExplode*, Trail, Pollen) map to their
//! registered `StratId`s, an address-resolved strategy encodes as
//! `External(addr)`, and any other game `StratId` (an ordinary enemy AI the
//! path tick never dispatches, only reads position/flags of) is carried as a
//! typed `StratRef::Native` handle so it decodes back to the exact same
//! identity.

use sf_game::alien::{
    Alien as GAlien, ObjectVisualKind as GObjectVisualKind, StratId, ACF_COLLTYPE2, ASF4_TEXTOBJ,
    ASF_COLLDISABLE, ASF_HITFLASH, ASF_SHADOW, NUMBER_AL,
};
use sf_game::game::Game;
use sf_game::vars::HARD_HP;
use sf_game::world::World;
use sf_path::alien::{
    Alien as PAlien, ObjectVisualKind as PObjectVisualKind, StratRef, AFEXP, ASF4_NOPOLYEXP,
    ASF_PARTOBJ,
};
use sf_path::interp::{dispatch_strat, PathHost, PathWorld};
use sf_path::literals::InlineIps;
use sf_path::rom_catalog_data::{ROM_DINTRO1_EXIT_IP, ROM_DINTRO1_LOOP_IP};

use crate::common::{self, sf_random, strat_chase, strat_chase8, sv, StratRam};
use crate::enemy_a;

/// Path inline-65816 callback ids (host `run_inline` dispatch).
const CB_TOW0_SET_EXPSTRAT: u16 = 1;
const CB_ROBEXPLODE_NOPOLYEXP: u16 = 2;
const CB_DSMOKE_INIT_COLANIM: u16 = 3;
const CB_DSMOKE_ADD_COLANIM: u16 = 4;
const CB_PBOOSTON_MAKEENGINE: u16 = 5;
const CB_PBOOSTCODE_UPDATEENGINE: u16 = 6;
const CB_MAKEPOLLEN: u16 = 7;
const CB_E_BIG_BIRD_TOUCH: u16 = 8;
const CB_CHECKIFEND_BASE: u16 = 9;
const CB_DINTRO1_ZOOM_TO_CENTRE: u16 = 16;
const CB_DINTRO1_KEEP_DISTANCE: u16 = 17;
const PATH_MISSING: u16 = 0xFFFF;
const DINTRO1_VIEW_DISTANCE: i16 = 4000;

/// DPATHDAT dintro1's signed half/clamp X chase. Returns (new X, centered).
fn dintro1_chase_x(x: i16) -> (i16, bool) {
    let half = x / 2; // ROM adiv2 rounds signed values toward zero.
    let step = half.clamp(-400, 400).wrapping_neg();
    if step == 0 {
        // The assembly branches on the zero step before adding it, then
        // explicitly clears worldx; this terminates cleanly from +/-1.
        return (0, true);
    }
    let next = x.wrapping_add(step);
    if next == 0 {
        (0, true)
    } else {
        (next, false)
    }
}

/// Register catalog-captured P_START65816 IPs (C `register_inline_callbacks`).
/// ROM PATHDATA.ASM bird_touch inline (P_START65816): set LE_ENTERSPEC,
/// nosetport3, kill engine SFX, disable collisions, startbgm $2.
pub fn path_bird_touch(g: &mut Game) {
    const LE_ENTERSPEC: u8 = 16;
    g.world.levelfinished = LE_ENTERSPEC;
    g.hooks.set_nosetport3(true);
    g.vars.pshipflags3 &= !sf_game::vars::PSF3_ENGINESND;
    g.vars.pshipflags3 |= sf_game::vars::PSF3_NOCOLLISIONS;
    g.hooks.play_music(0x02);
}

fn register_path_inline_callbacks(
    pw: &mut PathWorld,
    ips: &InlineIps,
    continuations: &[(u16, u16)],
) {
    let pairs = [
        (ips.tow_0_set_expstrat, CB_TOW0_SET_EXPSTRAT),
        (ips.robexplode_nopolyexp, CB_ROBEXPLODE_NOPOLYEXP),
        (ips.dsmoke_init_colanim, CB_DSMOKE_INIT_COLANIM),
        (ips.dsmoke_add_colanim, CB_DSMOKE_ADD_COLANIM),
        (ips.makepollen, CB_MAKEPOLLEN),
        (ips.e_big_bird_touch, CB_E_BIG_BIRD_TOUCH),
        (ips.dintro1_zoom_to_centre, CB_DINTRO1_ZOOM_TO_CENTRE),
        (ips.dintro1_keep_distance, CB_DINTRO1_KEEP_DISTANCE),
        (ips.pbooston_makeengine, CB_PBOOSTON_MAKEENGINE),
        (ips.pboostcode_updateengine, CB_PBOOSTCODE_UPDATEENGINE),
        (ips.checkifend1, CB_CHECKIFEND_BASE),
        (ips.checkifend2, CB_CHECKIFEND_BASE + 1),
        (ips.checkifend3, CB_CHECKIFEND_BASE + 2),
        (ips.checkifend4, CB_CHECKIFEND_BASE + 3),
        (ips.checkifend5, CB_CHECKIFEND_BASE + 4),
        (ips.checkifend6, CB_CHECKIFEND_BASE + 5),
        (ips.checkifend7, CB_CHECKIFEND_BASE + 6),
    ];
    for &(ip, cb) in &pairs {
        if ip != PATH_MISSING {
            let continuation = continuations
                .iter()
                .find_map(|&(action, continuation)| (action == ip).then_some(continuation))
                .expect("every native path action has generated continuation metadata");
            pw.register_inline_code(ip, cb, continuation);
        }
    }
}

/// PathWorld's compatibility location for the ROM `al_sflags` textobj bit.
/// The game pool additionally carries [`ASF4_TEXTOBJ`] as a renderer-only
/// discriminator because its retained flag layout already assigned this bit
/// position to lock-on behavior.
const ASF3_TEXTOBJ: u8 = 0x40;

/// Encoded source operands for the two path latches. These exist only at the
/// retained path-program import boundary.
const EROLL1_ADDR: u16 = 0x2302;
const EBYTE3_ADDR: u16 = 0x2303;
const ROM_EROLL1_ADDR: u16 = 0xF168;
const ROM_EBYTE3_ADDR: u16 = 0xF169;
const CTYPE_ADDR: u16 = 0x1A13;

/// Registered path-lane strategy handles, loaded once per strat call.
#[derive(Clone, Copy)]
struct PathStratIds {
    tick: StratId,
    coll: StratId,
    explode: StratId,
    pei: StratId,
    pes: StratId,
    trail: StratId,
    pollen: StratId,
}

/// Init-strategy handles handed back to `table::register_all` for the three
/// IS_PATH istrat rows.
pub struct PathInitIds {
    pub path_init: StratId,
    pub patht_init: StratId,
    pub pathdha_init: StratId,
}

fn load_ids(g: &Game) -> Option<PathStratIds> {
    let rd = |index: usize| {
        let v = g.vars.strategy_bindings.path[index];
        if v == 0 {
            None
        } else {
            Some(StratId(v - 1))
        }
    };
    Some(PathStratIds {
        tick: rd(0)?,
        coll: rd(1)?,
        explode: rd(2)?,
        pei: rd(3)?,
        pes: rd(4)?,
        trail: rd(5)?,
        pollen: rd(6)?,
    })
}

/// C `Strat_RegisterAll` path wiring: register the path-lane strategies, stash
/// their ids in WRAM, (re)install the `PathWorld` on `Game`, and return the
/// three IS_PATH init handles. Called from `table::register_all`, which runs
/// on every `World::init` reset — matching C re-running Strat_RegisterAll +
/// Paths_Init/Paths_LoadData on each level load.
pub fn register(g: &mut Game) -> PathInitIds {
    let tick = g.world.register_strategy(pw_tick);
    let coll = g.world.register_strategy(pw_coll);
    // StratExplode is the ordinary game explosion strat (C explode_Istrat ==
    // Strat_Explode); dispatch it directly, no path round-trip.
    let explode = g.world.register_strategy(enemy_a::strat_explode);
    let pei = g.world.register_strategy(pw_pei);
    let pes = g.world.register_strategy(pw_pes);
    let trail = g.world.register_strategy(pw_trail);
    let pollen = g.world.register_strategy(pw_pollen);

    g.vars.strategy_bindings.path = [
        tick.0 + 1,
        coll.0 + 1,
        explode.0 + 1,
        pei.0 + 1,
        pes.0 + 1,
        trail.0 + 1,
        pollen.0 + 1,
    ];

    let path_init = g.world.register_strategy(pw_path_init);
    let patht_init = g.world.register_strategy(pw_pathtext_init);
    let pathdha_init = g.world.register_strategy(pw_pathdha_init);

    // Fresh PathWorld with the literal path catalog loaded (C Paths_Init +
    // Paths_LoadData). The map VM's SETPATH opcode resolves a start offset
    // into this same blob (via Hooks::resolve_path_start over the same
    // catalog) and stores it in al_sword2, so the IP indexes correctly.
    let cat = sf_path::literals::get_catalog();
    let mut pw = PathWorld::new();
    pw.paths_load_data(cat.data.clone(), cat.offsets.clone());
    // C Paths_RegisterInlineCode — map captured P_START65816 IPs to host
    // callback ids (path_adapter::run_inline).
    register_path_inline_callbacks(&mut pw, &cat.ips, &cat.inline_continuations);
    g.path = Some(pw);

    PathInitIds {
        path_init,
        patht_init,
        pathdha_init,
    }
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    use sf_game::obj::strat_init_obj_vars;
    use sf_path::ids::PATH_ID_MATEMSG;

    #[test]
    fn every_certified_inline_path_action_is_registered() {
        let catalog = sf_path::literals::get_catalog();
        let mut world = PathWorld::new();
        register_path_inline_callbacks(&mut world, &catalog.ips, &catalog.inline_continuations);

        let actions = [
            catalog.ips.tow_0_set_expstrat,
            catalog.ips.robexplode_nopolyexp,
            catalog.ips.dsmoke_init_colanim,
            catalog.ips.dsmoke_add_colanim,
            catalog.ips.makepollen,
            catalog.ips.e_big_bird_touch,
            catalog.ips.dintro1_zoom_to_centre,
            catalog.ips.dintro1_keep_distance,
            catalog.ips.pbooston_makeengine,
            catalog.ips.pboostcode_updateengine,
            catalog.ips.checkifend1,
            catalog.ips.checkifend2,
            catalog.ips.checkifend3,
            catalog.ips.checkifend4,
            catalog.ips.checkifend5,
            catalog.ips.checkifend6,
            catalog.ips.checkifend7,
        ];

        for action in actions {
            assert!(
                world.find_inline_code(action).is_some(),
                "path action {action} was not registered"
            );
        }
    }

    #[test]
    fn path_initializer_falls_through_to_the_spawn_pass_movement() {
        const INITIAL_DEPTH: i16 = 2_800;
        const FORWARD_VELOCITY: i16 = 65;
        const FIRST_DEPTH: i16 = INITIAL_DEPTH + FORWARD_VELOCITY;

        let mut game = Game::new();
        let init = register(&mut game).pathdha_init;
        let object = game.objs.alloc().expect("path object slot");
        strat_init_obj_vars(&mut game.objs.aliens[object as usize]);
        game.objs.aliens[object as usize].worldz = INITIAL_DEPTH;
        game.vars.pviewvelz = FORWARD_VELOCITY;
        set_object_path(&mut game, object, PATH_ID_MATEMSG);

        game.call_strat(init, object);

        assert_eq!(game.objs.aliens[object as usize].worldz, FIRST_DEPTH);
        assert_ne!(game.objs.aliens[object as usize].stratptr, Some(init));
    }
}

// ============================================================
// Init strategies (C path_init_common + the three istrat entries). PATHS.ASM
// places `.strat` immediately after `path_istrat`, so every initializer falls
// through into the first path dispatch and movement phase on its spawn pass.
// ============================================================

/// C `path_init_common` (src/path/paths.c:932).
fn path_init_common(g: &mut Game, idx: u16, ids: PathStratIds) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(ids.tick);
    al.collstratptr = Some(ids.coll);
    al.expstratptr = Some(ids.explode);
    al.collflags |= ACF_COLLTYPE2; // ENEMY1
    al.sflags |= ASF_SHADOW;
    al.sbyte4 = 0; // friend id
}

/// C `Strat_Path_Init` (path_istrat).
fn pw_path_init(g: &mut Game, idx: u16) {
    let Some(ids) = load_ids(g) else { return };
    path_init_common(g, idx, ids);
    run_path(g, idx, StratRef::PathTick);
}

/// Assign one native path program to an object. Direct initialization
/// strategies use this for the same typed path cursor that map bytecode fills
/// through its `SETPATH` operation.
pub(crate) fn set_object_path(g: &mut Game, idx: u16, path_id: u16) {
    let start = g
        .path
        .as_mut()
        .map(|paths| paths.paths_resolve_start(path_id))
        .unwrap_or_default();
    g.objs.aliens[idx as usize].sword2 = start as i16;
}

/// Finish direct initialization as an ordinary path-driven object.
pub(crate) fn initialize_path_object(g: &mut Game, idx: u16) {
    pw_path_init(g, idx);
}

/// C `Strat_PathDha_Init` (pathdha_istrat: s_set_aldata #10,#10 ; bra path).
fn pw_pathdha_init(g: &mut Game, idx: u16) {
    let Some(ids) = load_ids(g) else { return };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = 10;
        al.ap = 10;
    }
    path_init_common(g, idx, ids);
    run_path(g, idx, StratRef::PathTick);
}

/// C `Strat_PathText_Init` (patht_istrat: colldisable + textobj + HP/AP).
fn pw_pathtext_init(g: &mut Game, idx: u16) {
    let Some(ids) = load_ids(g) else { return };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.sflags3 |= ASF3_TEXTOBJ;
        al.sflags4 |= ASF4_TEXTOBJ;
        al.hp = 10;
        al.ap = 8; // hardAP
    }
    path_init_common(g, idx, ids);
    run_path(g, idx, StratRef::PathTick);
}

// ============================================================
// Per-frame dispatch wrappers — each drives one path-lane routine through the
// take/sync/run/sync/return cycle. dispatch_strat routes the StratRef to the
// matching interpreter function.
// ============================================================

fn pw_tick(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::PathTick);
}
fn pw_coll(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::PathOnCollision);
}
fn pw_pei(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::ParticleExplodeIstrat);
}
fn pw_pes(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::ParticleExplodeStrat);
}
fn pw_trail(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::TrailTick);
}
fn pw_pollen(g: &mut Game, idx: u16) {
    run_path(g, idx, StratRef::ParticlePollenStrat);
}

fn run_path(g: &mut Game, idx: u16, strat: StratRef) {
    let Some(ids) = load_ids(g) else { return };
    let Some(mut pw) = g.path.take() else { return };
    sync_in(g, &mut pw, ids);
    {
        let mut host = Adapter { g, ids };
        dispatch_strat(&mut pw, &mut host, idx, strat);
    }
    sync_out(g, &pw, ids);
    g.path = Some(pw);
}

// ============================================================
// Pool + globals mirroring.
// ============================================================

fn sync_in(g: &mut Game, pw: &mut PathWorld, ids: PathStratIds) {
    pw.aldead = 0;
    // Globals paths.c reads (variables.h / game_vars.h).
    pw.gameframe = g.vars.gameframe;
    pw.pviewvelz = g.vars.pviewvelz;
    pw.pshipflags2 = g.vars.pshipflags2;
    pw.gameflags = g.vars.gameflags;
    pw.bunny_hp = g.vars.bunny_hp;
    pw.falcon_hp = g.vars.falcon_hp;
    pw.frog_hp = g.vars.frog_hp;
    pw.minpmove_y = g.vars.minpmove_y;
    pw.minpmove_x = g.vars.sv_i16(sv::MINPMOVEX);
    pw.maxpmove_x = g.vars.sv_i16(sv::MAXPMOVEX);
    pw.maxpmove_y = g.vars.sv_i16(sv::MAXPMOVEY);
    pw.eroll1 = g.vars.shared.enemy_path.roll1;
    pw.ebyte3 = g.vars.shared.enemy_path.byte3;
    pw.currentlevel = g.vars.shared.current_level;
    pw.playerscore = g.vars.shared.player_score;
    pw.friends_meter = g.vars.shared.friends_meter;
    for i in 0..pw.shapes_table.len().min(g.world.shapes_table.len()) {
        pw.shapes_table[i] = g.world.shapes_table[i];
    }
    for i in 0..NUMBER_AL {
        pw.aliens[i] = copy_g2p(&g.objs.aliens[i], ids);
    }
    pw.active_list = g.objs.active_head;
}

fn sync_out(g: &mut Game, pw: &PathWorld, ids: PathStratIds) {
    g.objs.aldead = pw.aldead;
    // Globals paths.c writes back.
    g.vars.bunny_hp = pw.bunny_hp;
    g.vars.falcon_hp = pw.falcon_hp;
    g.vars.frog_hp = pw.frog_hp;
    g.vars.shared.enemy_path.roll1 = pw.eroll1;
    g.vars.shared.enemy_path.byte3 = pw.ebyte3;
    g.vars.shared.player_score = pw.playerscore;
    g.vars.shared.friends_meter = pw.friends_meter;

    for i in 0..NUMBER_AL {
        let pa = pw.aliens[i];
        // Skip slots that are inert in both pools (nothing to write).
        if !g.objs.aliens[i].active && !pa.active {
            continue;
        }
        // Resolve strategy handles first (immutable World borrow), then take
        // the mutable Objects borrow — do NOT touch next/prev/active, which
        // Objects owns (the host maintained them for alloc/free).
        let s0 = ref2id(pa.stratptr, ids, &g.world);
        let s1 = ref2id(pa.expstratptr, ids, &g.world);
        let s2 = ref2id(pa.collstratptr, ids, &g.world);
        let s3 = ref2id(pa.endcollstratptr, ids, &g.world);
        let s4 = ref2id(pa.tempstratptr, ids, &g.world);
        let ga = &mut g.objs.aliens[i];
        copy_p2g_data(&pa, ga);
        if pa.sflags3 & ASF3_TEXTOBJ != 0 {
            ga.sflags4 |= ASF4_TEXTOBJ;
        } else {
            ga.sflags4 &= !ASF4_TEXTOBJ;
        }
        ga.stratptr = s0;
        ga.expstratptr = s1;
        ga.collstratptr = s2;
        ga.endcollstratptr = s3;
        ga.tempstratptr = s4;
    }
}

/// Re-mirror the active-list structure (Objects is authoritative) into the
/// PathWorld after any host alloc/free/spawn.
fn mirror_list(g: &Game, world: &mut PathWorld) {
    world.active_list = g.objs.active_head;
    for i in 0..NUMBER_AL {
        world.aliens[i].next = g.objs.aliens[i].next;
        world.aliens[i].prev = g.objs.aliens[i].prev;
        world.aliens[i].active = g.objs.aliens[i].active;
    }
}

// ============================================================
// Strategy-slot mapping.
// ============================================================

fn id2ref(id: Option<StratId>, ids: PathStratIds) -> Option<StratRef> {
    let id = id?;
    Some(if id == ids.tick {
        StratRef::PathTick
    } else if id == ids.coll {
        StratRef::PathOnCollision
    } else if id == ids.explode {
        StratRef::StratExplode
    } else if id == ids.pei {
        StratRef::ParticleExplodeIstrat
    } else if id == ids.pes {
        StratRef::ParticleExplodeStrat
    } else if id == ids.trail {
        StratRef::TrailTick
    } else if id == ids.pollen {
        StratRef::ParticlePollenStrat
    } else {
        StratRef::Native(id.0)
    })
}

fn ref2id(sr: Option<StratRef>, ids: PathStratIds, world: &World) -> Option<StratId> {
    Some(match sr? {
        StratRef::PathTick => ids.tick,
        StratRef::PathOnCollision => ids.coll,
        StratRef::StratExplode => ids.explode,
        StratRef::ParticleExplodeIstrat => ids.pei,
        StratRef::ParticleExplodeStrat => ids.pes,
        StratRef::TrailTick => ids.trail,
        StratRef::ParticlePollenStrat => ids.pollen,
        StratRef::Native(id) => StratId(id),
        StratRef::External(addr) => return world.find_strategy_address(addr),
    })
}

// ============================================================
// Alien field copy (both are ports of C `Alien`; only the strategy slots
// differ). Data fields are copied by C name; list/active fields and strategy
// slots are handled by the caller.
// ============================================================

macro_rules! copy_data_fields {
    ($src:expr, $dst:expr) => {{
        let s = $src;
        let d = $dst;
        d.shape = s.shape;
        d.ptr = s.ptr;
        d.flags = s.flags;
        d.type_ = s.type_;
        d.count = s.count;
        d.count1 = s.count1;
        d.worldx = s.worldx;
        d.worldy = s.worldy;
        d.worldz = s.worldz;
        d.rotx = s.rotx;
        d.roty = s.roty;
        d.rotz = s.rotz;
        d.vel = s.vel;
        d.immuneptr = s.immuneptr;
        d.collobjptr = s.collobjptr;
        d.sflags = s.sflags;
        d.sflags2 = s.sflags2;
        d.sflags3 = s.sflags3;
        d.sflags4 = s.sflags4;
        d.skidy = s.skidy;
        d.sbyte1 = s.sbyte1;
        d.sbyte2 = s.sbyte2;
        d.sbyte3 = s.sbyte3;
        d.sbyte4 = s.sbyte4;
        d.sword1 = s.sword1;
        d.sword2 = s.sword2;
        d.hp = s.hp;
        d.ap = s.ap;
        d.weapontype = s.weapontype;
        d.collcount = s.collcount;
        d.collflags = s.collflags;
        d.vx = s.vx;
        d.vy = s.vy;
        d.vz = s.vz;
        d.hitflags = s.hitflags;
        d.sbyte5 = s.sbyte5;
        d.sbyte6 = s.sbyte6;
        d.swpx1 = s.swpx1;
        d.swpy1 = s.swpy1;
        d.swpz1 = s.swpz1;
        d.stratstate = s.stratstate;
        d.fireobjptr = s.fireobjptr;
        d.depthoffset = s.depthoffset;
        d.relposx = s.relposx;
        d.relposy = s.relposy;
        d.relposz = s.relposz;
        d.debrisshape = s.debrisshape;
        d.colframe = s.colframe;
        d.animframe = s.animframe;
        d.snd1 = s.snd1;
        d.snd2 = s.snd2;
        d.coltab = s.coltab;
        d.childx = s.childx;
        d.childy = s.childy;
        d.childz = s.childz;
        d.childrotx = s.childrotx;
        d.childroty = s.childroty;
        d.childrotz = s.childrotz;
        d.childrotobj = s.childrotobj;
        d.tx = s.tx;
        d.ty = s.ty;
        d.memptr = s.memptr;
        d.stackptr = s.stackptr;
        d.stratmem = s.stratmem;
        d.pbyte1 = s.pbyte1;
        d.pbyte2 = s.pbyte2;
        d.pword1 = s.pword1;
    }};
}

/// game `Alien` -> path `Alien` (full: data + strat slots + list/active).
fn copy_g2p(ga: &GAlien, ids: PathStratIds) -> PAlien {
    let mut pa = PAlien::default();
    copy_data_fields!(ga, &mut pa);
    pa.visual_kind = match ga.visual_kind {
        GObjectVisualKind::Mesh => PObjectVisualKind::Mesh,
        GObjectVisualKind::ScaledSprite => PObjectVisualKind::ScaledSprite,
    };
    pa.stratptr = id2ref(ga.stratptr, ids);
    pa.expstratptr = id2ref(ga.expstratptr, ids);
    pa.collstratptr = id2ref(ga.collstratptr, ids);
    pa.endcollstratptr = id2ref(ga.endcollstratptr, ids);
    pa.tempstratptr = id2ref(ga.tempstratptr, ids);
    pa.next = ga.next;
    pa.prev = ga.prev;
    pa.active = ga.active;
    pa
}

/// path `Alien` -> game `Alien` (data only; caller sets strat slots and never
/// touches list/active).
fn copy_p2g_data(pa: &PAlien, ga: &mut GAlien) {
    copy_data_fields!(pa, &mut *ga);
    ga.visual_kind = match pa.visual_kind {
        PObjectVisualKind::Mesh => GObjectVisualKind::Mesh,
        PObjectVisualKind::ScaledSprite => GObjectVisualKind::ScaledSprite,
    };
}

// ============================================================
// PathHost — external functions paths.c calls, backed by Game / sf_strat.
// ============================================================

struct Adapter<'a> {
    g: &'a mut Game,
    ids: PathStratIds,
}

impl PathHost for Adapter<'_> {
    fn random(&mut self) -> u16 {
        // Shares g_rndval with every other strategy (single PRNG, like C).
        sf_random(&mut self.g.vars)
    }

    fn trig_se(&mut self, sound_id: u8) {
        // C Strat_TrigSE == Sound_PlaySE (strat_common.c:323).
        self.g.hooks.play_se(sound_id);
    }

    fn send_message(&mut self, msg_id: u8) {
        self.g.hooks.send_message(msg_id);
    }

    fn find_strategy_address(&mut self, addr24: u32) -> Option<StratRef> {
        // Resolve only if the game knows the address (C returns NULL else);
        // carry the raw address so ref2id resolves it back to the handle.
        self.g
            .world
            .find_strategy_address(addr24)
            .map(|_| StratRef::External(addr24))
    }

    fn genvecs_2d(&mut self, al: &mut PAlien) {
        // ROM-exact fixed-point (matches sf_strat::common::strat_gen_vecs_2d).
        use crate::snes_trig::{mulslog, COSTAB, SINTAB};
        let angle = al.roty as usize;
        let vel = i32::from(al.vel as i8);
        al.vx = mulslog(vel, SINTAB[angle] as i32) as i16;
        al.vy = 0;
        al.vz = mulslog(vel, COSTAB[angle] as i32) as i16;
    }

    fn genvecs_3d(&mut self, al: &mut PAlien) {
        // ROM-exact, matching sf_strat::common::strat_gen_vecs_3d: YAW negated
        // (this lane previously did not, i.e. the same inversion bug the oracle
        // caught for the player) + mulslog fixed-point.
        use crate::snes_trig::{mulslog, COSTAB, SINTAB};
        let yaw = (al.roty as i8).wrapping_neg() as u8 as usize;
        let pitch = al.rotx as usize;
        let vel = i32::from(al.vel as i8);
        let cosx = COSTAB[pitch] as i32;
        al.vx = mulslog(mulslog(vel, SINTAB[yaw] as i32), cosx) as i16;
        al.vy = mulslog(vel, SINTAB[pitch] as i32) as i16;
        al.vz = mulslog(mulslog(vel, COSTAB[yaw] as i32), cosx) as i16;
    }

    fn chase8(&mut self, current: u8, target: u8, rate: u8) -> u8 {
        strat_chase8(current, target, rate)
    }

    fn chase16(&mut self, current: i16, target: i16, rate: i16) -> i16 {
        strat_chase(current, target, rate)
    }

    fn angle_xz(&mut self, src: &PAlien, dst: &PAlien) -> u8 {
        // ROM Yanglexy_l (raw; face/goto callers apply nega).
        sf_core::aim_angle::yanglexy(
            dst.worldx.wrapping_sub(src.worldx),
            dst.worldz.wrapping_sub(src.worldz),
        )
    }

    fn apply_velocity(&mut self, al: &mut PAlien) {
        al.worldx = al.worldx.wrapping_add(al.vx);
        al.worldy = al.worldy.wrapping_add(al.vy);
        al.worldz = al.worldz.wrapping_add(al.vz);
    }

    fn hit_flash(&mut self, al: &mut PAlien) {
        // C Strat_HitFlash (strat_enemy.c:5895), reduced: the trait only sees
        // the single alien, so on death we leave hp==0 for the object's
        // expstratptr to explode it via the game dostrats loop next frame
        // (1-frame-late vs C's same-frame expstrat call; not hit by SF1 path
        // objects in the parity window).
        if al.hp != HARD_HP {
            if al.hp > 0 {
                al.hp -= 1;
            }
            if al.hp == 0 {
                return;
            }
        }
        al.sflags |= ASF_HITFLASH;
    }

    fn init_obj_vars(&mut self, al: &mut PAlien) {
        // C Strat_InitObjVars (strat_common.c:218) — spawn-time defaults.
        al.sflags = 0;
        al.sflags2 = 0;
        al.hp = 0;
        al.ap = 0;
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.count = 0;
        al.count1 = 0;
        al.animframe = 0xFF;
        al.colframe = 0xFF;
        al.collflags = ACF_FIRSTFRAME_P;
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        world: &mut PathWorld,
        owner: u16,
        off_x: i16,
        off_y: i16,
        off_z: i16,
        rot_x: u8,
        rot_y: u8,
        speed: u8,
        lifetime: u8,
        ap: u8,
        coll_type_bit: u8,
    ) -> Option<u16> {
        // Allocate + set up in the canonical Objects pool via the shared
        // strat routine (correct StratIds/RNG), then mirror into the path
        // pool so the interpreter can tweak the returned slot.
        let k = common::strat_spawn_projectile(
            self.g,
            Some(owner),
            off_x,
            off_y,
            off_z,
            rot_x,
            rot_y,
            speed,
            lifetime,
            ap,
            coll_type_bit,
        )?;
        world.aliens[k as usize] = copy_g2p(&self.g.objs.aliens[k as usize], self.ids);
        mirror_list(self.g, world);
        Some(k)
    }

    fn explode(&mut self, world: &mut PathWorld, idx: u16) {
        // C Strat_Explode on the canonical pool (sets aldead, SE, specials).
        enemy_a::strat_explode(self.g, idx);
        world.aldead = self.g.objs.aldead;
        world.aliens[idx as usize] = copy_g2p(&self.g.objs.aliens[idx as usize], self.ids);
        mirror_list(self.g, world);
    }

    fn obj_alloc(&mut self, world: &mut PathWorld) -> Option<u16> {
        // C Obj_Alloc on the canonical pool (identical LIFO free-slot choice
        // to C's single pool), then mirror.
        let k = self.g.objs.alloc()?;
        world.aliens[k as usize] = copy_g2p(&self.g.objs.aliens[k as usize], self.ids);
        mirror_list(self.g, world);
        Some(k)
    }

    fn obj_free(&mut self, world: &mut PathWorld, idx: u16) {
        self.g.objs.free(idx);
        mirror_list(self.g, world);
    }

    fn player(&mut self, _world: &PathWorld) -> Option<u16> {
        // C Obj_GetPlayer — slot 0 when active.
        if self.g.objs.aliens[0].active {
            Some(0)
        } else {
            None
        }
    }

    fn run_inline(&mut self, world: &mut PathWorld, self_idx: u16, callback: u16) {
        let si = self_idx as usize;
        match callback {
            // DPATHDAT tow_0 installs its bespoke falling-top explosion.
            CB_TOW0_SET_EXPSTRAT => {
                world.aliens[si].expstratptr = self.find_strategy_address(0x030001);
            }
            // robexplode disables polygon debris before its particle burst.
            CB_ROBEXPLODE_NOPOLYEXP => {
                world.aliens[si].sflags4 |= ASF4_NOPOLYEXP;
            }
            CB_DSMOKE_INIT_COLANIM => {
                world.aliens[si].colframe = 0;
            }
            CB_DSMOKE_ADD_COLANIM => {
                if world.aliens[si].colframe < 15 {
                    world.aliens[si].colframe += 1;
                }
            }
            CB_MAKEPOLLEN => {
                let (worldx, worldy, worldz) = {
                    let source = &world.aliens[si];
                    (source.worldx, source.worldy, source.worldz)
                };
                let Some(pollen) = self.obj_alloc(world).map(|idx| idx as usize) else {
                    return;
                };
                self.init_obj_vars(&mut world.aliens[pollen]);
                let particle = &mut world.aliens[pollen];
                particle.shape = 0;
                particle.expstratptr = Some(StratRef::ParticlePollenStrat);
                particle.worldx = worldx;
                particle.worldy = worldy.wrapping_sub(120);
                particle.worldz = worldz;
                particle.sflags |= ASF_COLLDISABLE | ASF_PARTOBJ;
                particle.flags |= AFEXP;
                particle.sbyte1 = 6;
                particle.sbyte2 = 60;
                particle.sbyte3 = 250;
            }
            CB_PBOOSTON_MAKEENGINE => {
                // The engine helper operates on the canonical game pool. Sync
                // its parent in, run it, then mirror parent/child back before
                // the path VM resumes in the same tick.
                copy_p2g_data(&world.aliens[si], &mut self.g.objs.aliens[si]);
                if let Some(engine) = common::makeengine_srou(self.g, self_idx) {
                    world.aliens[si] = copy_g2p(&self.g.objs.aliens[si], self.ids);
                    world.aliens[engine as usize] =
                        copy_g2p(&self.g.objs.aliens[engine as usize], self.ids);
                    mirror_list(self.g, world);
                }
            }
            CB_PBOOSTCODE_UPDATEENGINE => {
                copy_p2g_data(&world.aliens[si], &mut self.g.objs.aliens[si]);
                let raw_engine = world.aliens[si].fireobjptr;
                if raw_engine != 0 {
                    let engine = raw_engine.wrapping_sub(1) as usize;
                    if engine < NUMBER_AL {
                        copy_p2g_data(&world.aliens[engine], &mut self.g.objs.aliens[engine]);
                    }
                }
                common::updateengine_srou(self.g, self_idx);
                world.aliens[si] = copy_g2p(&self.g.objs.aliens[si], self.ids);
                if raw_engine != 0 {
                    let engine = raw_engine.wrapping_sub(1) as usize;
                    if engine < NUMBER_AL {
                        world.aliens[engine] = copy_g2p(&self.g.objs.aliens[engine], self.ids);
                    }
                }
            }
            // ROM PATHDATA.ASM:370-390 bird_touch: export levelfinished /
            // nosetport3, kill engine SFX, disable collisions, startbgm $2,
            // routechange 1. Catalog already set eflag1; LE_ENTERSPEC +
            // nosetport3 live in Game/shell, not path WRAM.
            CB_E_BIG_BIRD_TOUCH => {
                path_bird_touch(self.g);
                // routechange 1 is applied by Shell::warp_advance on ENTERSPEC.
            }
            // DPATHDAT dintro1 special achase: signed divide X by two,
            // clamp the step to +/-400, negate it, and either loop the inline
            // block or switch out to TRAIL OFF once centered.
            CB_DINTRO1_ZOOM_TO_CENTRE => {
                let (next_x, centered) = dintro1_chase_x(world.aliens[si].worldx);
                world.aliens[si].worldx = next_x;
                if centered {
                    world.override_inline_return(ROM_DINTRO1_EXIT_IP);
                } else {
                    world.override_inline_return(ROM_DINTRO1_LOOP_IP);
                }
            }
            // DPATHDAT dintro1 `.keep4000`: keep each text path at the
            // authored depth in front of the moving passive-player view.
            CB_DINTRO1_KEEP_DISTANCE => {
                world.aliens[si].worldz = self
                    .g
                    .vars
                    .sv_i16(sv::VIEWPOSZ)
                    .wrapping_add(DINTRO1_VIEW_DISTANCE);
            }
            // KPATHDAT `checkifend N`: if stage==N, c_type=201. Both are real
            // low-WRAM cells shared with ending/map code.
            n if (CB_CHECKIFEND_BASE..CB_CHECKIFEND_BASE + 7).contains(&n) => {
                let expected = (n - CB_CHECKIFEND_BASE + 1) as u8;
                if self.g.vars.shared.stage == expected {
                    self.path_write_ext8(world, CTYPE_ADDR, 201);
                }
            }
            _ => {}
        }
    }

    fn path_read_ext8(&mut self, _world: &PathWorld, addr: u16) -> u8 {
        let canonical = match addr {
            ROM_EROLL1_ADDR => EROLL1_ADDR,
            ROM_EBYTE3_ADDR => EBYTE3_ADDR,
            _ => addr,
        };
        self.g.vars.read_ext8(canonical)
    }

    fn path_read_ext16(&mut self, _world: &PathWorld, addr: u16) -> u16 {
        self.g.vars.read_ext16(addr)
    }

    fn path_write_ext8(&mut self, world: &mut PathWorld, addr: u16, value: u8) {
        match addr {
            EROLL1_ADDR | ROM_EROLL1_ADDR => {
                world.eroll1 = value;
                self.g.vars.shared.enemy_path.roll1 = value;
            }
            EBYTE3_ADDR | ROM_EBYTE3_ADDR => {
                world.ebyte3 = value;
                self.g.vars.shared.enemy_path.byte3 = value;
            }
            _ => self.g.vars.write_ext8(addr, value),
        }
    }

    fn path_write_ext16(&mut self, _world: &mut PathWorld, addr: u16, value: u16) {
        self.g.vars.write_ext16(addr, value);
    }

    fn path_set_friends_meter(&mut self, world: &mut PathWorld, value: u8) {
        world.friends_meter = value;
        self.g.vars.shared.friends_meter = value;
        self.g.hooks.set_friends_meter(value);
    }

    fn run_external_strat(&mut self, _world: &mut PathWorld, _idx: u16, _strat: StratRef) {
        // dispatch_strat only routes here for ParticlePollenStrat / External,
        // neither of which is reached in this integration: pollen objects are
        // created only by the (unregistered) makepollen inline callback, and
        // address-resolved strategies are dispatched by the game dostrats loop
        // (via their round-tripped StratId), not from inside a path tick.
    }
}

/// `sf_path::alien::ACF_FIRSTFRAME` value (0x04) — kept as a local const so
/// `init_obj_vars` need not import the path-lane collision-flag set.
const ACF_FIRSTFRAME_P: u8 = 0x04;

#[cfg(test)]
mod tests {
    use super::{
        copy_g2p, copy_p2g_data, dintro1_chase_x, Adapter, GAlien, GObjectVisualKind, Game, PAlien,
        PObjectVisualKind, PathHost, PathStratIds, StratId,
    };
    use crate::common::{strat_gen_vecs_2d, strat_gen_vecs_3d};

    fn test_ids() -> PathStratIds {
        PathStratIds {
            tick: StratId(0),
            coll: StratId(1),
            explode: StratId(2),
            pei: StratId(3),
            pes: StratId(4),
            trail: StratId(5),
            pollen: StratId(6),
        }
    }

    #[test]
    fn dintro1_special_achase_matches_signed_65816_edges() {
        assert_eq!(dintro1_chase_x(-3000), (-2600, false));
        assert_eq!(dintro1_chase_x(3000), (2600, false));
        assert_eq!(dintro1_chase_x(-75), (-38, false));
        assert_eq!(dintro1_chase_x(75), (38, false));
        assert_eq!(dintro1_chase_x(-1), (0, true));
        assert_eq!(dintro1_chase_x(1), (0, true));
        assert_eq!(dintro1_chase_x(0), (0, true));
    }

    #[test]
    fn typed_sprite_presentation_round_trips_through_path_bridge() {
        const SPRITE_DEPTH_COLOUR: i16 = -2;
        const SPRITE_SIZE: u8 = 12;

        let ids = test_ids();
        let mut game_object = GAlien::default();
        game_object.visual_kind = GObjectVisualKind::ScaledSprite;
        game_object.depthoffset = SPRITE_DEPTH_COLOUR;
        game_object.tx = SPRITE_SIZE;

        let path_object = copy_g2p(&game_object, ids);
        assert_eq!(path_object.visual_kind, PObjectVisualKind::ScaledSprite);
        assert_eq!(path_object.depthoffset, SPRITE_DEPTH_COLOUR);
        assert_eq!(path_object.tx, SPRITE_SIZE);

        let mut round_trip = GAlien::default();
        copy_p2g_data(&path_object, &mut round_trip);
        assert_eq!(round_trip.visual_kind, GObjectVisualKind::ScaledSprite);
        assert_eq!(round_trip.depthoffset, SPRITE_DEPTH_COLOUR);
        assert_eq!(round_trip.tx, SPRITE_SIZE);
    }

    #[test]
    fn path_vectors_preserve_signed_source_speed() {
        const NEGATIVE_SPEED: i8 = -20;
        const YAW: u8 = 0;
        const PITCH: u8 = 0;

        let mut game = Game::new();
        let mut adapter = Adapter {
            g: &mut game,
            ids: test_ids(),
        };
        let mut path_object = PAlien {
            vel: NEGATIVE_SPEED as u8,
            rotx: PITCH,
            roty: YAW,
            ..PAlien::default()
        };
        let mut game_object = GAlien {
            vel: NEGATIVE_SPEED as u8,
            rotx: PITCH,
            roty: YAW,
            ..GAlien::default()
        };

        adapter.genvecs_2d(&mut path_object);
        strat_gen_vecs_2d(&mut game_object);
        assert_eq!(
            (path_object.vx, path_object.vy, path_object.vz),
            (game_object.vx, game_object.vy, game_object.vz)
        );
        assert!(path_object.vz < 0);

        adapter.genvecs_3d(&mut path_object);
        strat_gen_vecs_3d(&mut game_object);
        assert_eq!(
            (path_object.vx, path_object.vy, path_object.vz),
            (game_object.vx, game_object.vy, game_object.vz)
        );
        assert!(path_object.vz < 0);
    }
}
