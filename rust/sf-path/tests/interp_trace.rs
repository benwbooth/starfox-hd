//! Trace parity of the path interpreter against the C oracle.
//!
//! Fixtures (`tests/fixtures/pi_<scenario>.txt`) were dumped by a standalone
//! harness (`pi_harness.c`, kept in the session scratchpad) that compiles
//! `src/path/paths.c` + `src/path/path_literals.c` + `src/game/alien_compat.c`
//! + `src/game/game_vars.c` with recording stubs for every extern paths.c
//! calls, then drives N ticks of a scripted scenario per path id.
//!
//! This test replays the identical scenarios through the Rust interpreter
//! with a recording [`PathHost`] whose stub behavior matches the C harness
//! stubs 1:1, and asserts the produced trace text is byte-identical.

use sf_path::alien::{Alien, StratRef, ACF_COLLTYPE1, ACF_COLLTYPE5, AFEXP, ASF4_NOPOLYEXP,
    ASF_COLLDISABLE, ASF_HITFLASH, ASF_PARTOBJ, NUMBER_AL};
use sf_path::ids::*;
use sf_path::interp::{
    dispatch_strat, strat_path_init, strat_pathdha_init, strat_pathtext_init, PathHost,
    PathWorld, WRAM_SIZE,
};
use sf_path::literals;

// libm FFI for seed_runtime_tables parity (same libm as the C build).
extern "C" {
    fn sinf(x: f32) -> f32;
}

// ============================================================
// Inline-65816 callback ids (registered from literals::InlineIps; the host
// replicates the C path_literal_* callback bodies — sf-game will own these).
// ============================================================
const CB_TOW0_SET_EXPSTRAT: u16 = 1;
const CB_ROBEXPLODE_NOPOLYEXP: u16 = 2;
const CB_DSMOKE_INIT_COLANIM: u16 = 3;
const CB_DSMOKE_ADD_COLANIM: u16 = 4;
const CB_PBOOSTON_MAKEENGINE: u16 = 5;
const CB_PBOOSTCODE_UPDATEENGINE: u16 = 6;
const CB_MAKEPOLLEN: u16 = 7;
const CB_E_BIG_BIRD_TOUCH: u16 = 8;
const CB_CHECKIFEND_BASE: u16 = 9; // 9..=15 => checkifend1..7

const PATH_EXT_CTYPE: u16 = 0x2305;
const STRAT_ADDR_TOW0EXPLODE: u32 = 0x030001;

// ============================================================
// Recording host — mirrors the pi_harness.c stubs exactly.
// ============================================================
struct RecHost {
    rng: u16,
    out: String,
    stage: u16, // C g_stage (checkifend callbacks)
}

impl RecHost {
    fn new() -> Self {
        RecHost {
            rng: 0x1234,
            out: String::new(),
            stage: 0,
        }
    }

    fn obj_alloc_impl(world: &mut PathWorld) -> Option<u16> {
        for i in 0..NUMBER_AL {
            if world.aliens[i].active {
                continue;
            }
            world.aliens[i] = Alien::default();
            world.aliens[i].active = true;
            world.aliens[i].next = world.active_list;
            world.active_list = Some(i as u16);
            return Some(i as u16);
        }
        None
    }

    fn init_obj_vars_impl(al: &mut Alien) {
        let next = al.next;
        let prev = al.prev;
        let active = al.active;
        *al = Alien::default();
        al.next = next;
        al.prev = prev;
        al.active = active;
    }
}

impl PathHost for RecHost {
    fn random(&mut self) -> u16 {
        self.rng = self.rng.wrapping_mul(25173).wrapping_add(13849);
        self.rng
    }

    fn trig_se(&mut self, sound_id: u8) {
        self.out.push_str(&format!("C se {sound_id}\n"));
    }

    fn send_message(&mut self, msg_id: u8) {
        self.out.push_str(&format!("C msg {msg_id}\n"));
    }

    fn find_strategy_address(&mut self, addr24: u32) -> Option<StratRef> {
        self.out.push_str(&format!("C findstrat {addr24:06x}\n"));
        None
    }

    fn genvecs_2d(&mut self, al: &mut Alien) {
        al.vx = (((al.roty as i8) as i32 * al.vel as i32) / 32) as i16;
        al.vy = 0;
        al.vz = al.vel as i16;
    }

    fn genvecs_3d(&mut self, al: &mut Alien) {
        self.genvecs_2d(al);
        al.vy = (((al.rotx as i8) as i32 * al.vel as i32) / 32) as i16;
    }

    fn chase8(&mut self, current: u8, target: u8, rate: u8) -> u8 {
        let d = (target.wrapping_sub(current) as i8) as i32;
        if d == 0 {
            return current;
        }
        if d > 0 {
            if d <= rate as i32 {
                return target;
            }
            return current.wrapping_add(rate);
        }
        if -d <= rate as i32 {
            return target;
        }
        current.wrapping_sub(rate)
    }

    fn chase16(&mut self, current: i16, target: i16, rate: i16) -> i16 {
        if current < target {
            let n = (current as i32 + rate as i32).min(target as i32);
            return n as i16;
        }
        if current > target {
            let n = (current as i32 - rate as i32).max(target as i32);
            return n as i16;
        }
        current
    }

    fn angle_xz(&mut self, src: &Alien, dst: &Alien) -> u8 {
        let dx = dst.worldx as i32 - src.worldx as i32;
        let dz = dst.worldz as i32 - src.worldz as i32;
        ((dx + dz) & 0xFF) as u8
    }

    fn apply_velocity(&mut self, al: &mut Alien) {
        al.worldx = al.worldx.wrapping_add(al.vx);
        al.worldy = al.worldy.wrapping_add(al.vy);
        al.worldz = al.worldz.wrapping_add(al.vz);
    }

    fn hit_flash(&mut self, al: &mut Alien) {
        al.sflags |= ASF_HITFLASH;
    }

    fn init_obj_vars(&mut self, al: &mut Alien) {
        Self::init_obj_vars_impl(al);
    }

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
        self.out.push_str(&format!(
            "C sproj o={owner} off={off_x},{off_y},{off_z} rot={rot_x},{rot_y} sp={speed} \
             lt={lifetime} ap={ap} cf={coll_type_bit}"
        ));
        let (owx, owy, owz) = {
            let o = &world.aliens[owner as usize];
            (o.worldx, o.worldy, o.worldz)
        };
        let p = match Self::obj_alloc_impl(world) {
            Some(p) => p,
            None => {
                self.out.push_str(" -> none\n");
                return None;
            }
        };
        Self::init_obj_vars_impl(&mut world.aliens[p as usize]);
        let a = &mut world.aliens[p as usize];
        a.shape = 1;
        a.worldx = owx.wrapping_add(off_x);
        a.worldy = owy.wrapping_add(off_y);
        a.worldz = owz.wrapping_add(off_z);
        a.rotx = rot_x;
        a.roty = rot_y;
        a.vel = speed;
        a.count = lifetime;
        a.ap = ap;
        a.collflags |= coll_type_bit;
        self.out.push_str(&format!(" -> {p}\n"));
        Some(p)
    }

    fn explode(&mut self, world: &mut PathWorld, idx: u16) {
        self.out.push_str(&format!("C explode {idx}\n"));
        world.aliens[idx as usize].flags |= AFEXP;
        world.aldead = 1;
    }

    fn obj_alloc(&mut self, world: &mut PathWorld) -> Option<u16> {
        Self::obj_alloc_impl(world)
    }

    fn obj_free(&mut self, world: &mut PathWorld, idx: u16) {
        self.out.push_str(&format!("C free {idx}\n"));
        // Unlink from active list.
        let target = Some(idx);
        if world.active_list == target {
            world.active_list = world.aliens[idx as usize].next;
        } else {
            let mut cur = world.active_list;
            while let Some(c) = cur {
                let next = world.aliens[c as usize].next;
                if next == target {
                    world.aliens[c as usize].next = world.aliens[idx as usize].next;
                    break;
                }
                cur = next;
            }
        }
        let a = &mut world.aliens[idx as usize];
        a.next = None;
        a.active = false;
        a.stratptr = None;
    }

    fn player(&mut self, world: &PathWorld) -> Option<u16> {
        if world.aliens[0].active {
            Some(0)
        } else {
            None
        }
    }

    fn run_inline(&mut self, world: &mut PathWorld, self_idx: u16, callback: u16) {
        let si = self_idx as usize;
        match callback {
            // C path_literal_tow_0_set_expstrat.
            CB_TOW0_SET_EXPSTRAT => {
                let f = self.find_strategy_address(STRAT_ADDR_TOW0EXPLODE);
                world.aliens[si].expstratptr = f;
                // (C prints "missing tow0explode" to stdout, not the trace.)
            }
            // C path_literal_robexplode_set_nopolyexp.
            CB_ROBEXPLODE_NOPOLYEXP => {
                world.aliens[si].sflags4 |= ASF4_NOPOLYEXP;
            }
            // C path_literal_dsmoke_init_colanim.
            CB_DSMOKE_INIT_COLANIM => {
                world.aliens[si].colframe = 0;
            }
            // C path_literal_dsmoke_add_colanim.
            CB_DSMOKE_ADD_COLANIM => {
                if world.aliens[si].colframe < 15 {
                    world.aliens[si].colframe += 1;
                }
            }
            // C path_literal_pbooston_makeengine / pboostcode_updateengine.
            CB_PBOOSTON_MAKEENGINE | CB_PBOOSTCODE_UPDATEENGINE => {}
            // C path_literal_makepollen.
            CB_MAKEPOLLEN => {
                let (wx, wy, wz) = {
                    let s = &world.aliens[si];
                    (s.worldx, s.worldy, s.worldz)
                };
                let p = match Self::obj_alloc_impl(world) {
                    Some(p) => p as usize,
                    None => return,
                };
                Self::init_obj_vars_impl(&mut world.aliens[p]);
                let a = &mut world.aliens[p];
                a.shape = 0; // SH_NULLSHAPE
                a.expstratptr = Some(StratRef::ParticlePollenStrat);
                a.worldx = wx;
                a.worldy = wy.wrapping_sub(120);
                a.worldz = wz;
                a.sflags |= ASF_COLLDISABLE | ASF_PARTOBJ;
                a.flags |= AFEXP;
                a.sbyte1 = 6;
                a.sbyte2 = 60;
                a.sbyte3 = 250;
            }
            // C path_literal_e_big_bird_touch (rumble/bitmap hooks are no-ops).
            CB_E_BIG_BIRD_TOUCH => {
                self.out.push_str("C music 2\n");
                self.out.push_str("C route1\n");
            }
            // C path_literal_checkifend1..7.
            n if (CB_CHECKIFEND_BASE..CB_CHECKIFEND_BASE + 7).contains(&n) => {
                let expected = (n - CB_CHECKIFEND_BASE + 1) as u16;
                if self.stage == expected {
                    world.ram[PATH_EXT_CTYPE as usize % WRAM_SIZE] = 201;
                }
            }
            _ => {}
        }
    }

    fn run_external_strat(&mut self, _world: &mut PathWorld, _idx: u16, _strat: StratRef) {
        // No external strategies are reachable in these scenarios (the
        // find_strategy_address stub always returns None).
    }
}

// ============================================================
// Trace dump — must match pi_harness.c dump_tick byte for byte.
// ============================================================
fn strat_code(f: Option<StratRef>) -> char {
    match f {
        None => '0',
        Some(StratRef::PathTick) => 'T',
        Some(_) => 'X',
    }
}
fn exp_code(f: Option<StratRef>) -> char {
    match f {
        None => '0',
        Some(StratRef::StratExplode) => 'E',
        Some(_) => 'X',
    }
}
fn coll_code(f: Option<StratRef>) -> char {
    if f.is_some() {
        'X'
    } else {
        '0'
    }
}

fn ram_hash(world: &PathWorld) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in world.ram.iter() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}

fn dump_tick(world: &PathWorld, out: &mut String, tick: i32) {
    out.push_str(&format!("T {tick}\n"));
    for i in 0..NUMBER_AL {
        let a = &world.aliens[i];
        if !a.active {
            continue;
        }
        out.push_str(&format!(
            "A {} sh={} pt={} fl={} ty={} c={} c1={} wx={} wy={} wz={} rx={} ry={} rz={} v={} \
             st={} im={} co={} s1={} s2={} s3={} s4={} sk={} b1={} b2={} b3={} b4={} w1={} w2={} \
             hp={} ap={} wt={} cc={} cf={} vx={} vy={} vz={} hf={} b5={} b6={} ex={} cs={} do={} \
             dbs={} cfr={} af={} sn1={} sn2={} ct={} cx={} cy={} cz={} crx={} cry={} crz={} \
             tx={} ty={} pb1={} pb2={}\n",
            i,
            a.shape,
            a.ptr,
            a.flags,
            a.type_,
            a.count,
            a.count1,
            a.worldx,
            a.worldy,
            a.worldz,
            a.rotx,
            a.roty,
            a.rotz,
            a.vel,
            strat_code(a.stratptr),
            a.immuneptr,
            a.collobjptr,
            a.sflags,
            a.sflags2,
            a.sflags3,
            a.sflags4,
            a.skidy,
            a.sbyte1,
            a.sbyte2,
            a.sbyte3,
            a.sbyte4,
            a.sword1,
            a.sword2,
            a.hp,
            a.ap,
            a.weapontype,
            a.collcount,
            a.collflags,
            a.vx,
            a.vy,
            a.vz,
            a.hitflags,
            a.sbyte5,
            a.sbyte6,
            exp_code(a.expstratptr),
            coll_code(a.collstratptr),
            a.depthoffset,
            a.debrisshape,
            a.colframe,
            a.animframe,
            a.snd1,
            a.snd2,
            a.coltab,
            a.childx,
            a.childy,
            a.childz,
            a.childrotx,
            a.childroty,
            a.childrotz,
            a.tx,
            a.ty,
            a.pbyte1,
            a.pbyte2,
        ));
    }
    out.push_str(&format!(
        "G gf={} sc={} er={} eb={} bh={} fh={} gh={} fm={} ramh={:08x}\n",
        world.gameframe,
        world.playerscore,
        world.eroll1,
        world.ebyte3,
        world.bunny_hp,
        world.falcon_hp,
        world.frog_hp,
        world.friends_meter,
        ram_hash(world)
    ));
}

// ============================================================
// Scenario driver — mirrors pi_harness.c main().
// ============================================================
struct Scenario {
    name: &'static str,
    path_id: u16,
    nticks: i32,
    init_kind: u8, // 0 = path, 1 = pathdha, 2 = pathtext
    shape: u16,
    hp: u8,
    ap: u8,
    pos: (i16, i16, i16),
    rot: (u8, u8, u8),
    hit_tick: i32, // -1 = never
}

const SCENARIOS: &[Scenario] = &[
    Scenario { name: "e_gate", path_id: PATH_ID_E_GATE, nticks: 240, init_kind: 0, shape: 90, hp: 8, ap: 2, pos: (100, 0, 2500), rot: (0, 0, 0), hit_tick: 30 },
    Scenario { name: "e_flower", path_id: PATH_ID_E_FLOWER, nticks: 300, init_kind: 0, shape: 0, hp: 8, ap: 2, pos: (-60, 0, 1800), rot: (0, 0, 0), hit_tick: -1 },
    Scenario { name: "ponpon", path_id: PATH_ID_PONPON, nticks: 240, init_kind: 0, shape: 40, hp: 6, ap: 2, pos: (120, 0, 2200), rot: (0, 0, 0), hit_tick: 40 },
    Scenario { name: "chase1_1", path_id: PATH_ID_CHASE1_1, nticks: 200, init_kind: 0, shape: 30, hp: 8, ap: 2, pos: (-40, 20, 2600), rot: (0, 0, 0), hit_tick: -1 },
    Scenario { name: "tow_0", path_id: PATH_ID_TOW_0, nticks: 200, init_kind: 0, shape: 50, hp: 8, ap: 2, pos: (80, -20, 2000), rot: (0, 0, 0), hit_tick: 60 },
    Scenario { name: "robot", path_id: PATH_ID_ROBOT, nticks: 200, init_kind: 0, shape: 60, hp: 10, ap: 4, pos: (-100, 0, 1500), rot: (0, 0, 0), hit_tick: -1 },
    Scenario { name: "dsmoke", path_id: PATH_ID_DSMOKE, nticks: 120, init_kind: 0, shape: 20, hp: 8, ap: 2, pos: (0, -40, 1200), rot: (0, 0, 0), hit_tick: -1 },
    Scenario { name: "falco_lv1", path_id: PATH_ID_FALCO_LV1, nticks: 300, init_kind: 0, shape: 70, hp: 20, ap: 4, pos: (40, 10, 900), rot: (0, 0, 0), hit_tick: 50 },
    Scenario { name: "mes_message", path_id: PATH_ID_MES_MESSAGE, nticks: 120, init_kind: 2, shape: 20, hp: 8, ap: 2, pos: (0, 0, 400), rot: (0, 0, 0), hit_tick: -1 },
];

/// C `seed_runtime_tables` (path_literals.c) — the C harness gets this from
/// `PathLiterals_GetCatalog`; the Rust literal catalog leaves ram seeding to
/// the game lane, so the driver replicates it here (same libm sinf).
fn seed_runtime_tables(world: &mut PathWorld) {
    const PATH_EXT_SINTAB: usize = 0x2200;
    const PATH_EXT_GWORD1: usize = 0x2300;
    const PATH_TWO_PI: f32 = 6.283_185_307_179_586_476_92_f32;
    for i in 0..256usize {
        let angle = (i as f32 * PATH_TWO_PI) / 256.0;
        let s = unsafe { sinf(angle) } * 127.0;
        let value: i32 = if s >= 0.0 {
            (s + 0.5) as i32
        } else {
            (s - 0.5) as i32
        };
        world.ram[(PATH_EXT_SINTAB + i) % WRAM_SIZE] = (value as i8) as u8;
    }
    world.ram[PATH_EXT_GWORD1 % WRAM_SIZE] = 0;
    world.ram[(PATH_EXT_GWORD1 + 1) % WRAM_SIZE] = 0;
}

fn register_inline_callbacks(world: &mut PathWorld, ips: &literals::InlineIps) {
    const MISSING: u16 = 0xFFFF;
    let regs = [
        (ips.tow_0_set_expstrat, CB_TOW0_SET_EXPSTRAT),
        (ips.robexplode_nopolyexp, CB_ROBEXPLODE_NOPOLYEXP),
        (ips.dsmoke_init_colanim, CB_DSMOKE_INIT_COLANIM),
        (ips.dsmoke_add_colanim, CB_DSMOKE_ADD_COLANIM),
        (ips.pbooston_makeengine, CB_PBOOSTON_MAKEENGINE),
        (ips.pboostcode_updateengine, CB_PBOOSTCODE_UPDATEENGINE),
        (ips.makepollen, CB_MAKEPOLLEN),
        (ips.e_big_bird_touch, CB_E_BIG_BIRD_TOUCH),
        (ips.checkifend1, CB_CHECKIFEND_BASE),
        (ips.checkifend2, CB_CHECKIFEND_BASE + 1),
        (ips.checkifend3, CB_CHECKIFEND_BASE + 2),
        (ips.checkifend4, CB_CHECKIFEND_BASE + 3),
        (ips.checkifend5, CB_CHECKIFEND_BASE + 4),
        (ips.checkifend6, CB_CHECKIFEND_BASE + 5),
        (ips.checkifend7, CB_CHECKIFEND_BASE + 6),
    ];
    for (ip, cb) in regs {
        if ip != MISSING {
            world.register_inline_code(ip, cb);
        }
    }
}

fn run_scenario(sc: &Scenario) -> String {
    let catalog = literals::get_catalog();

    let mut world = PathWorld::new();
    let mut host = RecHost::new();

    // --- World reset (mirrors pi_harness.c main) ---
    world.pviewvelz = 40;
    world.bunny_hp = 100;
    world.falcon_hp = 100;
    world.frog_hp = 100;
    world.minpmove_x = -3000;
    world.maxpmove_x = 3000;
    world.minpmove_y = -3000;
    world.maxpmove_y = 3000;
    for i in 0..256 {
        world.shapes_table[i] = i as u16;
    }

    world.paths_init();
    seed_runtime_tables(&mut world);
    world.paths_load_data(catalog.data.clone(), catalog.offsets.clone());
    register_inline_callbacks(&mut world, &catalog.ips);

    // Player at slot 0.
    let pl = RecHost::obj_alloc_impl(&mut world).unwrap();
    assert_eq!(pl, 0);
    world.aliens[0].shape = 2;
    world.aliens[0].hp = 100;
    world.aliens[0].worldx = 0;
    world.aliens[0].worldy = 60;
    world.aliens[0].worldz = 0;

    // Dummy "player laser" collider at slot 1 (used by the hit event).
    let dummy = RecHost::obj_alloc_impl(&mut world).unwrap();
    assert_eq!(dummy, 1);
    world.aliens[1].shape = 999;
    world.aliens[1].collflags = ACF_COLLTYPE1 | ACF_COLLTYPE5;
    world.aliens[1].worldz = 100;

    // Path alien at slot 2.
    let ai = RecHost::obj_alloc_impl(&mut world).unwrap();
    assert_eq!(ai, 2);
    world.aliens[2].shape = sc.shape;
    match sc.init_kind {
        1 => strat_pathdha_init(&mut world.aliens[2]),
        2 => strat_pathtext_init(&mut world.aliens[2]),
        _ => strat_path_init(&mut world.aliens[2]),
    }
    world.aliens[2].hp = sc.hp;
    world.aliens[2].ap = sc.ap;
    world.aliens[2].worldx = sc.pos.0;
    world.aliens[2].worldy = sc.pos.1;
    world.aliens[2].worldz = sc.pos.2;
    world.aliens[2].rotx = sc.rot.0;
    world.aliens[2].roty = sc.rot.1;
    world.aliens[2].rotz = sc.rot.2;
    let start = world.paths_resolve_start(sc.path_id);
    world.aliens[2].sword2 = start as i16;

    for tick in 0..sc.nticks {
        world.gameframe = tick as u16;
        let pv = world.pviewvelz;
        world.aliens[0].worldz = world.aliens[0].worldz.wrapping_add(pv);

        if tick == sc.hit_tick && world.aliens[2].active {
            if let Some(cs) = world.aliens[2].collstratptr {
                world.aliens[2].collobjptr = 1;
                dispatch_strat(&mut world, &mut host, 2, cs);
            }
        }

        for i in 0..NUMBER_AL {
            if !world.aliens[i].active {
                continue;
            }
            let strat = match world.aliens[i].stratptr {
                Some(s) => s,
                None => continue,
            };
            world.aldead = 0;
            dispatch_strat(&mut world, &mut host, i as u16, strat);
            if world.aldead != 0 {
                host.obj_free(&mut world, i as u16);
                world.aldead = 0;
            }
        }

        dump_tick(&world, &mut host.out, tick);
    }

    host.out
}

#[test]
fn interp_trace_parity() {
    let mut failures = Vec::new();
    for sc in SCENARIOS {
        let fixture_path = format!(
            "{}/tests/fixtures/pi_{}.txt",
            env!("CARGO_MANIFEST_DIR"),
            sc.name
        );
        let expected = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|e| panic!("read {fixture_path}: {e}"));
        let actual = run_scenario(sc);

        if actual != expected {
            // Report the first diverging line for debugging.
            let mut line_no = 0usize;
            for (a, e) in actual.lines().zip(expected.lines()) {
                line_no += 1;
                if a != e {
                    failures.push(format!(
                        "{}: divergence at line {line_no}\n  C:    {e}\n  rust: {a}",
                        sc.name
                    ));
                    break;
                }
            }
            if actual.lines().count() != expected.lines().count() {
                failures.push(format!(
                    "{}: line count rust {} vs C {}",
                    sc.name,
                    actual.lines().count(),
                    expected.lines().count()
                ));
            }
            if failures.is_empty() {
                failures.push(format!("{}: content differs", sc.name));
            }
        }
    }
    assert!(failures.is_empty(), "trace mismatches:\n{}", failures.join("\n"));
}
