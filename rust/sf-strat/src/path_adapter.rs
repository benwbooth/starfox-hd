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
//! Both `Alien` structs are field-for-field ports of the C `Alien`
//! (`src/game/obj.h`) with identical widths; they differ ONLY in the five
//! strategy slots (`StratId` vs `StratRef`). [`copy_g2p`]/[`copy_p2g_data`]
//! copy every data field by C name; the strategy slots round-trip through
//! [`id2ref`]/[`ref2id`]: the path-lane routines (PathTick, PathOnCollision,
//! Explode, ParticleExplode*, Trail, Pollen) map to their registered
//! `StratId`s, an address-resolved strategy encodes as `External(addr)`, and
//! any other game `StratId` (an ordinary enemy AI the path tick never
//! dispatches, only reads position/flags of) is carried as
//! `External(0xF0_0000|id)` so it decodes back to the exact same handle.

use sf_game::alien::{
    Alien as GAlien, StratId, ACF_COLLTYPE2, ASF_COLLDISABLE, ASF_HITFLASH, ASF_SHADOW,
    NUMBER_AL,
};
use sf_game::game::Game;
use sf_game::vars::HARD_HP;
use sf_game::world::World;
use sf_path::alien::{Alien as PAlien, StratRef};
use sf_path::interp::{dispatch_strat, PathHost, PathWorld};

use crate::common::{self, sf_random, snes_cos, snes_sin, strat_chase, strat_chase8, sv, StratRam};
use crate::enemy_a;

/// `al_sflags3` textobj bit (C `src/game/obj.h` ASF3_TEXTOBJ) — not yet a
/// named const in `sf_game::alien`.
const ASF3_TEXTOBJ: u8 = 0x40;

/// External-address tag for carrying an opaque game `StratId` through a
/// `StratRef::External` across the pool boundary. High byte 0xF0 is far above
/// any real SNES bank (real addrs are 0x00xxxx..0x06xxxx), so it never
/// collides with an address the path VM would try to resolve.
const GAME_ID_TAG: u32 = 0xF0_0000;

/// C `g_eroll1` / `g_ebyte3` shadow addresses (paths.c special-cases these ext
/// reads; see `PathWorld::read_ext8`). Mirrored to/from `GameVars` WRAM.
const EROLL1_ADDR: u16 = 0x2302;
const EBYTE3_ADDR: u16 = 0x2303;

/// WRAM addresses holding the path-lane strategy registry ids (id+1; 0 =
/// unregistered), stored the same way `sf_strat::common` stashes the shared
/// projectile ids. Block sits just past the `sv` block (ends 0x0570).
mod psv {
    pub const TICK: u16 = 0x0572;
    pub const COLL: u16 = 0x0574;
    pub const EXPLODE: u16 = 0x0576;
    pub const PEI: u16 = 0x0578;
    pub const PES: u16 = 0x057A;
    pub const TRAIL: u16 = 0x057C;
    pub const POLLEN: u16 = 0x057E;
}

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
    let rd = |a: u16| {
        let v = g.vars.read_ext16(a);
        if v == 0 {
            None
        } else {
            Some(StratId(v - 1))
        }
    };
    Some(PathStratIds {
        tick: rd(psv::TICK)?,
        coll: rd(psv::COLL)?,
        explode: rd(psv::EXPLODE)?,
        pei: rd(psv::PEI)?,
        pes: rd(psv::PES)?,
        trail: rd(psv::TRAIL)?,
        pollen: rd(psv::POLLEN)?,
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

    g.vars.write_ext16(psv::TICK, tick.0 + 1);
    g.vars.write_ext16(psv::COLL, coll.0 + 1);
    g.vars.write_ext16(psv::EXPLODE, explode.0 + 1);
    g.vars.write_ext16(psv::PEI, pei.0 + 1);
    g.vars.write_ext16(psv::PES, pes.0 + 1);
    g.vars.write_ext16(psv::TRAIL, trail.0 + 1);
    g.vars.write_ext16(psv::POLLEN, pollen.0 + 1);

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
    g.path = Some(pw);

    PathInitIds {
        path_init,
        patht_init,
        pathdha_init,
    }
}

// ============================================================
// Init strategies (C path_init_common + the three istrat entries).
// These operate purely on the game alien: set the shadow/collide flags and
// point the strategy slots at the path tick/collision/explode handles. Like
// C, they run once (the map VM assigned IS_PATH's init as stratptr) and do
// NOT move the object this frame; movement begins next frame via pw_tick.
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
}

/// C `Strat_PathText_Init` (patht_istrat: colldisable + textobj + HP/AP).
fn pw_pathtext_init(g: &mut Game, idx: u16) {
    let Some(ids) = load_ids(g) else { return };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.sflags3 |= ASF3_TEXTOBJ;
        al.hp = 10;
        al.ap = 8; // hardAP
    }
    path_init_common(g, idx, ids);
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
    pw.eroll1 = g.vars.read_ext8(EROLL1_ADDR);
    pw.ebyte3 = g.vars.read_ext8(EBYTE3_ADDR);
    // g_currentlevel/g_playerscore/g_friends_meter are not yet GameVars fields
    // (unported strat/HUD-lane globals); they are only touched by rare path
    // opcodes not exercised by SF1 path objects. The PathWorld carries its own
    // persistent copies, left untouched here.
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
    g.vars.write_ext8(EROLL1_ADDR, pw.eroll1);
    g.vars.write_ext8(EBYTE3_ADDR, pw.ebyte3);

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
        StratRef::External(GAME_ID_TAG | id.0 as u32)
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
        StratRef::External(addr) => {
            if addr & 0xFF_0000 == GAME_ID_TAG {
                StratId((addr & 0xFFFF) as u16)
            } else {
                return world.find_strategy_address(addr);
            }
        }
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
    copy_data_fields!(pa, ga);
}

// ============================================================
// PathHost — external functions paths.c calls, backed by Game / sf_strat.
// ============================================================

struct Adapter<'a> {
    g: &'a mut Game,
    ids: PathStratIds,
}

const TWO_PI: f32 = 2.0f32 * 3.141_592_65_f32;

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
        // C Strat_GenVecs2D (real trig, matching sf_strat::common).
        let speed = al.vel as i16 as f32;
        al.vx = (speed * snes_sin(al.roty)) as i16;
        al.vy = 0;
        al.vz = (speed * snes_cos(al.roty)) as i16;
    }

    fn genvecs_3d(&mut self, al: &mut PAlien) {
        // C Strat_GenVecs3D (pitch = -rotx, ASM convention).
        let yaw = al.roty;
        let pitch = (al.rotx as i8).wrapping_neg() as u8;
        let speed = al.vel as i16 as f32;
        let cy = snes_cos(pitch);
        let sy = snes_sin(pitch);
        al.vx = (speed * snes_sin(yaw) * cy) as i16;
        al.vy = (speed * sy) as i16;
        al.vz = (speed * snes_cos(yaw) * cy) as i16;
    }

    fn chase8(&mut self, current: u8, target: u8, rate: u8) -> u8 {
        strat_chase8(current, target, rate)
    }

    fn chase16(&mut self, current: i16, target: i16, rate: i16) -> i16 {
        strat_chase(current, target, rate)
    }

    fn angle_xz(&mut self, src: &PAlien, dst: &PAlien) -> u8 {
        // C Strat_AngleXZ.
        let dx = dst.worldx.wrapping_sub(src.worldx) as f32;
        let dz = dst.worldz.wrapping_sub(src.worldz) as f32;
        let mut angle = dx.atan2(dz);
        if angle < 0.0 {
            angle += TWO_PI;
        }
        ((angle * (256.0f32 / TWO_PI)) as i32) as u8
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

    fn run_inline(&mut self, _world: &mut PathWorld, _self_idx: u16, _callback: u16) {
        // No inline-65816 callbacks are registered on the adapter's PathWorld,
        // so P_START65816 only decodes its return value; this is never reached.
        // (C path_literal_* callbacks are a later game-core-lane item.)
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
