//! PATH-interpreter accuracy audit — ROM (PATHS.ASM) vs the Rust port
//! (`sf-path/src/interp.rs`), using the 65816 oracle.
//!
//! The ROM side runs the *real* path strategy `path_istrat` ($04:801E,
//! PATHS.ASM:63) on crafted bytecode patched into the `paths` blob
//! ($04:A420), one JSL per game frame (the istrat prologue is idempotent:
//! it re-stores the strat pointers and zeroes `pathptr`, then falls through
//! into `.strat`, exactly one interpreter tick).
//!
//! The Rust side runs `sf_path::interp::strat_path_tick` over the identical
//! bytes with a minimal host. Path chases are implemented inside the interp
//! itself (`path_achase8`/`path_achase16`, the ROM proportional chase); the
//! host's chase8/chase16 callbacks (bound to the strat-lane
//! `sf_strat::common::strat_chase8` / `strat_chase` like the shipping
//! adapter) remain only to satisfy the `PathHost` trait.
//!
//! Also audits the chase primitive directly (SR8/SR16_ACHASE_ALVAR*,
//! STRATROU.ASM:2740, the routines behind `s_achase_var` /
//! `s_obj2obj_3dangle`) and diffs the shipped path DATA (catalog bytes vs the
//! assembled ROM blob).

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_path::alien::NUMBER_AL;
use sf_path::interp::{path_achase16, path_achase8, strat_path_tick, PathHost, PathWorld};
use sf_path::opcodes::*;
use sf_strat::common::{strat_chase, strat_chase8};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Common addresses / offsets (data/symbols.txt)
// ---------------------------------------------------------------------------

// Alien struct field offsets.
const AL_COUNT1: u32 = 0x0B;
const AL_WORLDY: u32 = 0x0E;
const AL_ROTX: u32 = 0x12;
const AL_ROTY: u32 = 0x13;
const AL_ROTZ: u32 = 0x14;
const AL_VEL: u32 = 0x15;
const AL_SBYTE2: u32 = 0x23;
const AL_SBYTE3: u32 = 0x24;
const AL_SWORD2: u32 = 0x28;

// Harness WRAM layout ($0200 stub / $0300 RTS trap are reserved).
const TRAMP: u32 = 0x0400; // DBR=$7E setter + JML path_istrat
const OBJ: u32 = 0x0500; // the path object
const NEWOBJ: u32 = 0x0600; // free block for P_SPAWN

// Executor vars.
const ALLST: u32 = 0x12AD;
const ALFREELST: u32 = 0x12AF;
const TPX: u32 = 0x003A; // DP temp used by sr8/sr16_achase_alvarN

/// SNES 24-bit (LoROM, $xx:8000+) -> ROM file offset.
fn lorom_off(addr: u32) -> usize {
    ((((addr >> 16) & 0x7F) as usize) << 15) | ((addr as usize & 0xFFFF) - 0x8000)
}

struct Syms(HashMap<String, u32>);
impl Syms {
    fn get(&self, name: &str) -> u32 {
        *self.0.get(name).unwrap_or_else(|| panic!("symbol {name} missing"))
    }
}

fn setup() -> Option<(Vec<u8>, Syms)> {
    let rom = load_built_rom()?;
    let syms = load_symbols();
    if syms.is_empty() {
        return None;
    }
    Some((rom, Syms(syms)))
}

// ---------------------------------------------------------------------------
// ROM-side per-frame path ticker
// ---------------------------------------------------------------------------

struct RomPath {
    bus: SnesBus,
}

impl RomPath {
    /// Patch `program` into the paths blob at offset 0 and set up one path
    /// object at OBJ with al_sword2 = 0.
    fn new(rom: &[u8], syms: &Syms, program: &[u8]) -> RomPath {
        let mut rom = rom.to_vec();
        let base = lorom_off(syms.get("PATHS"));
        for (i, b) in program.iter().enumerate() {
            rom[base + i] = *b;
        }
        let mut bus = SnesBus::new(rom);

        // Trampoline: DBR=$7E (strategies run with the data bank in WRAM so
        // `al_*,x` addressing hits our object block), then JML path_istrat.
        // Entered in 8-bit A / 16-bit X (Entry.p = 0x20), like dostrats.
        let istrat = syms.get("PATH_ISTRAT");
        let t = [
            0xA9, 0x7E, // LDA #$7E
            0x48, 0xAB, // PHA : PLB
            0x5C, istrat as u8, (istrat >> 8) as u8, (istrat >> 16) as u8, // JML
        ];
        for (i, b) in t.iter().enumerate() {
            bus.write8(TRAMP + i as u32, *b);
        }
        bus.write16(OBJ + AL_SWORD2, 0);
        RomPath { bus }
    }

    /// One game frame: run the path strategy once (istrat prologue + .strat).
    fn tick(&mut self) {
        call(
            &mut self.bus,
            TRAMP,
            &Entry { x: OBJ as u16, p: 0x20, ..Default::default() },
        );
    }

    fn r8(&self, off: u32) -> u8 {
        self.bus.read8(OBJ + off)
    }
    fn r16(&self, off: u32) -> u16 {
        self.bus.read16(OBJ + off)
    }
    fn w8(&mut self, off: u32, v: u8) {
        self.bus.write8(OBJ + off, v);
    }
}

// ---------------------------------------------------------------------------
// Rust-side runner with the shipping adapter's chase callbacks
// ---------------------------------------------------------------------------

struct MiniHost;

impl PathHost for MiniHost {
    fn random(&mut self) -> u16 {
        0
    }
    fn trig_se(&mut self, _sound_id: u8) {}
    fn send_message(&mut self, _msg_id: u8) {}
    fn find_strategy_address(&mut self, _addr24: u32) -> Option<sf_path::alien::StratRef> {
        None
    }
    fn genvecs_2d(&mut self, _al: &mut sf_path::alien::Alien) {}
    fn genvecs_3d(&mut self, _al: &mut sf_path::alien::Alien) {}
    // Same functions sf-game/src/path_adapter.rs binds for the real game.
    fn chase8(&mut self, current: u8, target: u8, rate: u8) -> u8 {
        strat_chase8(current, target, rate)
    }
    fn chase16(&mut self, current: i16, target: i16, rate: i16) -> i16 {
        strat_chase(current, target, rate)
    }
    fn angle_xz(&mut self, _src: &sf_path::alien::Alien, _dst: &sf_path::alien::Alien) -> u8 {
        0
    }
    fn apply_velocity(&mut self, _al: &mut sf_path::alien::Alien) {}
    fn hit_flash(&mut self, _al: &mut sf_path::alien::Alien) {}
    fn init_obj_vars(&mut self, _al: &mut sf_path::alien::Alien) {}
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        _world: &mut PathWorld,
        _owner: u16,
        _off_x: i16,
        _off_y: i16,
        _off_z: i16,
        _rot_x: u8,
        _rot_y: u8,
        _speed: u8,
        _lifetime: u8,
        _ap: u8,
        _coll_type_bit: u8,
    ) -> Option<u16> {
        None
    }
    fn explode(&mut self, _world: &mut PathWorld, _idx: u16) {}
    fn obj_alloc(&mut self, world: &mut PathWorld) -> Option<u16> {
        for i in 2..NUMBER_AL as u16 {
            if !world.aliens[i as usize].active {
                world.aliens[i as usize].active = true;
                world.aliens[i as usize].next = None;
                world.active_list = Some(i);
                return Some(i);
            }
        }
        None
    }
    fn obj_free(&mut self, world: &mut PathWorld, idx: u16) {
        world.aliens[idx as usize].active = false;
    }
    fn player(&mut self, _world: &PathWorld) -> Option<u16> {
        None
    }
    fn run_inline(&mut self, _world: &mut PathWorld, _self_idx: u16, _callback: u16) {}
    fn run_external_strat(
        &mut self,
        _world: &mut PathWorld,
        _idx: u16,
        _strat: sf_path::alien::StratRef,
    ) {
    }
}

/// Rust world with `program` as the whole path blob and object #1 active
/// at ip 0.
fn rust_world(program: &[u8]) -> PathWorld {
    let mut w = PathWorld::new();
    w.paths_load_data(program.to_vec(), Vec::new());
    w.aliens[1].active = true;
    w
}

/// Build a 0x40-byte program pre-filled with P_END and splice `code` at 0.
fn prog(code: &[u8]) -> Vec<u8> {
    let mut p = vec![P_END; 0x40];
    p[..code.len()].copy_from_slice(code);
    p
}

// ===========================================================================
// 1. Chase primitive: ROM is a PROPORTIONAL chase (delta >> rate, min step 1,
//    8-bit wrap = short way around the circle). The path interpreter now
//    implements it locally (`path_achase8`/`path_achase16`, rates straight
//    from PATHS.ASM); sf-strat's `strat_chase8` stays the fixed-step
//    primitive for the strat lane and must keep diverging here.
// ===========================================================================

/// ROM sr8_achase_alvarN: tpx = current (8-bit), A = target; returns new
/// current in A.
fn rom_achase8(rom: &[u8], addr: u32, cur: u8, target: u8) -> u8 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(TPX, cur);
    let exit = call(&mut bus, addr, &Entry { a: target as u16, p: 0x20, ..Default::default() });
    exit.a
}

fn rom_achase16(rom: &[u8], addr: u32, cur: i16, target: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(TPX, cur as u16);
    let exit = call(&mut bus, addr, &Entry { a: target as u16, p: 0x00, ..Default::default() });
    exit.c as i16
}

/// The ROM formula (Achase_var2A, STRATMAC.INC:525): delta = target-cur as
/// signed 8-bit; |delta| raised to at least 2^rate (nolessrange); then `rate`
/// applications of adiv2 (halve, rounding toward zero); cur += delta.
fn proportional8(cur: u8, target: u8, rate: u32) -> u8 {
    if cur == target {
        return cur;
    }
    let mut d = target.wrapping_sub(cur) as i8 as i32;
    let lim = 1i32 << rate;
    if d >= 0 && d < lim {
        d = lim;
    }
    if d < 0 && d > -lim {
        d = -lim;
    }
    for _ in 0..rate {
        d = if d >= 0 { d >> 1 } else { -((-d) >> 1) }; // adiv2: toward zero
    }
    cur.wrapping_add(d as u8)
}

#[test]
fn achase_rom_is_proportional_and_port_matches() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    let a2 = syms.get("SR8_ACHASE_ALVAR2"); // p_faceplayer rate (PATHS.ASM:573)
    let a3 = syms.get("SR8_ACHASE_ALVAR3"); // p_achaseB / p_faceshape / waits
    let w3 = syms.get("SR16_ACHASE_ALVAR3"); // p_achaseW rate

    // The port now implements the ROM proportional chase in the path interp
    // (path_achase8/16) at the PATHS.ASM rates: P_ACHASEB/W + waits +
    // P_FACESHAPE/P_WAITFACEPLAYER rate 3, P_FACEPLAYER/P_GOTOPOS rate 2.
    let cases: [(u8, u8); 6] = [(0, 96), (250, 10), (100, 90), (10, 5), (5, 5), (0, 128)];
    for (cur, t) in cases {
        let rom3 = rom_achase8(&rom, a3, cur, t);
        let rom2 = rom_achase8(&rom, a2, cur, t);
        let port3 = path_achase8(cur, t, 3);
        let port2 = path_achase8(cur, t, 2);
        println!(
            "cur={cur:3} target={t:3} | ROM rate3 -> {rom3:3} (model {m3:3}) rate2 -> {rom2:3} (model {m2:3}) | port rate3 -> {port3:3}  rate2 -> {port2:3}",
            m3 = proportional8(cur, t, 3),
            m2 = proportional8(cur, t, 2),
        );
        assert_eq!(rom3, proportional8(cur, t, 3), "proportional model mismatch (rate3)");
        assert_eq!(rom2, proportional8(cur, t, 2), "proportional model mismatch (rate2)");
        assert_eq!(rom3, port3, "port path_achase8 rate 3 vs ROM");
        assert_eq!(rom2, port2, "port path_achase8 rate 2 vs ROM");
    }
    // 16-bit: p_achaseW — ROM rate-3 proportional; port path_achase16 matches.
    let rw = rom_achase16(&rom, w3, 0, 1000);
    let pw = path_achase16(0, 1000, 3);
    println!("16-bit cur=0 target=1000 | ROM rate3 -> {rw} | port achaseW -> {pw}");
    assert_eq!(rw, 125, "ROM 16-bit proportional step (1000>>3)");
    assert_eq!(rw, pw, "port path_achase16 matches ROM");

    // The wrap case (250 -> 10): both go UP through 255 (short way, +2).
    let wrap_rom = rom_achase8(&rom, a3, 250, 10);
    assert_eq!(wrap_rom, 252, "ROM chases angles the short way around");
    assert_eq!(path_achase8(250, 10, 3), 252, "port takes the short way too");
    // sf-strat's fixed-step primitive is a DIFFERENT routine (strat lane) and
    // must not be confused with the path chase: it still walks the long way.
    assert_eq!(strat_chase8(250, 10, 1), 249, "strat-lane fixed step unchanged");
    assert_eq!(strat_chase(0, 1000, 1), 1, "strat-lane 16-bit fixed step unchanged");
}

// ===========================================================================
// 2. Harness sanity + timing parity: wait / addrotx / wait1 / goto.
//    These semantics ARE correct in the port — a pass validates the whole
//    ROM-tick harness (opcode numbering, per-frame IP advance, wait counter).
// ===========================================================================

#[test]
fn wait_goto_timing_parity() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    let program = prog(&[P_WAIT, 2, P_ADDROTX, 5, P_WAIT1, P_GOTO, 0, 0]);

    let mut rp = RomPath::new(&rom, &syms, &program);
    let mut w = rust_world(&program);
    let mut h = MiniHost;

    for frame in 1..=8 {
        rp.tick();
        strat_path_tick(&mut w, &mut h, 1);
        let rom_state = (rp.r16(AL_SWORD2), rp.r8(AL_SBYTE3), rp.r8(AL_ROTX));
        let rust_state = (
            w.aliens[1].sword2 as u16,
            w.aliens[1].sbyte3,
            w.aliens[1].rotx,
        );
        println!("frame {frame}: ROM (ip,wait,rotx)={rom_state:?}  RUST={rust_state:?}");
        assert_eq!(rom_state, rust_state, "frame {frame} diverged");
    }
}

// ===========================================================================
// 3. P_ACCEL move-phase: when vel+rate lands EXACTLY on the target the ROM
//    clamps and zeroes al_count1 (PATHS.ASM:2828-2834 .incit/.finish2); the
//    port mirrors the ROM branch structure (decel keeps its one-frame-late
//    zeroing).
// ===========================================================================

#[test]
fn accel_exact_hit_zeroes_count1_in_rom_and_port() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    // 0 -> 40 at +8/frame: hits 40 exactly on the 5th move phase.
    let program = prog(&[P_SETACCEL, 40, 8, P_WAIT, 200]);

    let mut rp = RomPath::new(&rom, &syms, &program);
    let mut w = rust_world(&program);
    let mut h = MiniHost;

    for frame in 1..=7 {
        rp.tick();
        strat_path_tick(&mut w, &mut h, 1);
        println!(
            "frame {frame}: ROM vel={:3} count1={} | RUST vel={:3} count1={}",
            rp.r8(AL_VEL),
            rp.r8(AL_COUNT1),
            w.aliens[1].vel,
            w.aliens[1].count1
        );
        assert_eq!(rp.r8(AL_VEL), w.aliens[1].vel, "vel ramp diverged at frame {frame}");
    }
    assert_eq!(rp.r8(AL_VEL), 40);
    assert_eq!(rp.r8(AL_COUNT1), 0, "ROM zeroes count1 once the target is hit exactly");
    assert_eq!(w.aliens[1].count1, 0, "port zeroes count1 on the exact hit like the ROM");
}

// ===========================================================================
// 4. Space-flight coupling (sflag4): ROM does roty += adiv2(adiv2(rotz)) —
//    each halving rounds TOWARD ZERO, so rotz in -3..=3 contributes nothing
//    and rotz=-5 contributes -1. The port now divides toward zero too.
// ===========================================================================

#[test]
fn space_flight_negative_rotz_coupling() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    for (rotz, rom_expect, rust_expect) in [
        (0xFFu8, 100u8, 100u8), // rotz=-1: 0/frame (adiv2 toward zero)
        (0xFB, 96, 96),         // rotz=-5: -1/frame
        (0x05, 104, 104),       // rotz=+5: +1/frame
    ] {
        let program = prog(&[P_SPACESHIPON, P_WAIT, 200]);
        let mut rp = RomPath::new(&rom, &syms, &program);
        rp.w8(AL_ROTZ, rotz);
        rp.w8(AL_ROTY, 100);

        let mut w = rust_world(&program);
        w.aliens[1].rotz = rotz;
        w.aliens[1].roty = 100;
        let mut h = MiniHost;

        for _ in 0..4 {
            rp.tick();
            strat_path_tick(&mut w, &mut h, 1);
        }
        println!(
            "rotz={rotz:#04x}: after 4 frames ROM roty={} RUST roty={}",
            rp.r8(AL_ROTY),
            w.aliens[1].roty
        );
        assert_eq!(rp.r8(AL_ROTY), rom_expect, "ROM adiv2 (round-toward-zero) coupling");
        assert_eq!(w.aliens[1].roty, rust_expect, "port adiv2 (round-toward-zero) coupling");
    }
}

// ===========================================================================
// 5. P_IFBETWEENB bounds: ROM branches only when val1 < var <= val2
//    (PATHS.ASM:1606-1613 — `bpl .add6` on val1-var makes the LOWER bound
//    exclusive). The port now matches.
// ===========================================================================

#[test]
fn ifbetween_lower_bound_exclusive_in_rom() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    // if sbyte2 (alvar offset 0x23) between 5 and 10 -> jump to 0x20.
    let program = prog(&[P_IFBETWEENB, 0x23, 5, 10, 0x20, 0x00]);

    for (val, rom_expect, rust_expect) in [
        (5u8, 6u16, 6u16), // == lower bound: exclusive, both fall through
        (6, 0x20, 0x20),   // inside: jump
        (10, 0x20, 0x20),  // == upper bound: inclusive, both jump
        (11, 6, 6),        // above: fall through
    ] {
        let mut rp = RomPath::new(&rom, &syms, &program);
        rp.w8(AL_SBYTE2, val);
        rp.tick();

        let mut w = rust_world(&program);
        w.aliens[1].sbyte2 = val;
        let mut h = MiniHost;
        strat_path_tick(&mut w, &mut h, 1);

        println!(
            "sbyte2={val:2}: ROM ip={:#04x} RUST ip={:#04x}",
            rp.r16(AL_SWORD2),
            w.aliens[1].sword2 as u16
        );
        assert_eq!(rp.r16(AL_SWORD2), rom_expect, "ROM ip for val={val}");
        assert_eq!(w.aliens[1].sword2 as u16, rust_expect, "port ip for val={val}");
    }
}

// ===========================================================================
// 6. P_CHILDDEAD on an object with NO mother: ROM falls through
//    (PATHS.ASM:1813-1826 — objtobemother yields al_ptr==0 -> .cannaedojim2 ->
//    .add4); the port now falls through too.
// ===========================================================================

#[test]
fn childdead_without_mother_falls_through_in_rom() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    let program = prog(&[P_CHILDDEAD, 1, 0x20, 0x00]);

    let mut rp = RomPath::new(&rom, &syms, &program);
    rp.tick();

    let mut w = rust_world(&program);
    let mut h = MiniHost;
    strat_path_tick(&mut w, &mut h, 1);

    println!(
        "no-mother childdead: ROM ip={:#04x} RUST ip={:#04x}",
        rp.r16(AL_SWORD2),
        w.aliens[1].sword2 as u16
    );
    assert_eq!(rp.r16(AL_SWORD2), 4, "ROM: no mother -> fall through (no jump)");
    assert_eq!(w.aliens[1].sword2 as u16, 4, "port: no mother -> fall through");
}

// ===========================================================================
// 7. P_SPAWN offsets: the ROM multiplies the x/y/z payload bytes by 4 after
//    rotation (PATHS.ASM:1790 `s_add_Roffs2pos ...,2,2,2` — two ASLs;
//    matching PATHMACS.ASM P_SPAWN which stores ({x})/4). The port now stores
//    coord/4 in the catalog and scales x4 after a Z,X,Y-order rotation; only
//    the ROM's fixed-point trig wobble (a few units) remains.
// ===========================================================================

#[test]
fn spawn_offsets_scaled_by_4_in_rom() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    let nullshape = (syms.get("NULLSHAPE") & 0xFFFF) as u16;
    // P_SPAWN shape=nullshape path=0 rots=0 hp/ap=10, offset (0,-50,0).
    let program = prog(&[
        P_SPAWN,
        nullshape as u8,
        (nullshape >> 8) as u8,
        0,
        0, // path offset 0 (never run by the child here)
        0,
        0,
        0, // rot offsets
        10,
        10, // hp/ap
        0,
        (-50i8) as u8,
        0, // x,y,z payload
        P_WAIT,
        200,
    ]);

    let mut rp = RomPath::new(&rom, &syms, &program);
    // Free list with one block for the spawn (mapvm-style seeding).
    rp.bus.write16(ALLST, 0);
    rp.bus.write16(ALFREELST, NEWOBJ as u16);
    rp.bus.write16(NEWOBJ, 0); // _next
    rp.tick();
    let rom_y = rp.bus.read16(NEWOBJ + AL_WORLDY) as i16;

    let mut w = rust_world(&program);
    let mut h = MiniHost;
    strat_path_tick(&mut w, &mut h, 1);
    let spawned = (2..NUMBER_AL).find(|&i| w.aliens[i].active);
    let rust_y = spawned.map(|i| w.aliens[i].worldy);

    println!("spawn offset y=-50: ROM child worldy={rom_y}  RUST child worldy={rust_y:?}");
    // rotate_8yx/yz/xz at angle 0 lose a little magnitude in fixed point
    // (observed -48 pre-scale), then the two ASLs (s_add_Roffs2pos ...,2,2,2)
    // multiply by 4: observed -192. The port's float trig is exact at angle
    // 0, so it lands on the full -200; the fixed-point wobble (<= ~2 units
    // pre-scale, 8 post-scale) is the accepted tolerance.
    assert!(
        (-208..=-184).contains(&rom_y),
        "ROM spawns at ~payload*4 = -200 (fixed-point trig wobble), got {rom_y}"
    );
    assert_eq!(rust_y, Some(-200), "port spawns at payload*4 (exact float trig)");
    let rust_y = rust_y.unwrap() as i32;
    assert!(
        (rom_y as i32 - rust_y).abs() <= 8,
        "port within the fixed-point tolerance of the ROM ({rom_y} vs {rust_y})"
    );
}

// ===========================================================================
// 8. Path DATA: catalog bytes vs the assembled ROM blob, opcode-by-opcode
//    with jump operands normalized (each side's blob has a different base).
// ===========================================================================

/// (length, offsets-of-16bit-path-address-operands) per opcode.
fn op_shape(op: u8) -> Option<(usize, &'static [usize])> {
    Some(match op {
        P_RELTOPLAYERON | P_RELTOPLAYEROFF | P_ALWAYSGENVECSON | P_ALWAYSGENVECSOFF
        | P_FACEPLAYER | P_WAITFACEPLAYER | P_END | P_FACESHAPE | P_REMOVE | P_EXPLODE
        | P_IMMUNE | P_SPACESHIPON | P_SPACESHIPOFF | P_HELION | P_HELIOFF | P_LINK
        | P_DAMAGE | P_SMOKEON | P_SMOKEOFF | P_INVINCIBLEON | P_INVINCIBLEOFF | P_ZREMOVEON
        | P_ZREMOVEOFF | P_DEBUG | P_FIRE | P_FIRECANHIT | P_FIREATPLAYER
        | P_FIREATPLAYERCANHIT | P_FIREATSHAPE | P_FIREATSHAPECANHIT | P_FLAGSHAPE
        | P_FLAGMOTHER | P_RETURN | P_NEXT | P_INEXT | P_BREAKC | P_INVISIBLEON
        | P_INVISIBLEOFF | P_IFNOT | P_PARTICLE | P_SHADOWON | P_SHADOWOFF | P_UNLINKSELF
        | P_BECOMESHAPE | P_BECOMEMOTHER | P_UNBECOME | P_BECOME | P_WAIT1 => (1, &[]),
        P_WAIT | P_SETVEL | P_INITANIM | P_FRIEND | P_MSG | P_MSG2 | P_WEAPON | P_LINKCHILD
        | P_FLAGCHILD | P_TRAIL | P_SOUND | P_SOUND2 | P_UNLINKCHILD | P_MSGWITHMETER | P_DOQ
        | P_DOALVARB | P_DOALVARW | P_GOSUBALVAR | P_BECOMECHILD | P_REMOVECHILD | P_SET0B
        | P_SET0W | P_INCB | P_INCW | P_DECB | P_DECW | P_ADDROTX | P_ADDROTY | P_ADDROTZ
        | P_ADDWORLDX | P_ADDWORLDY | P_ADDWORLDZ | P_PUSHB | P_PUSHW | P_PULLB | P_PULLW => {
            (2, &[])
        }
        P_ADDB | P_ACHASEB | P_WAITACHASEB | P_SETB | P_FINDSHAPE | P_FINDNEXTSHAPE
        | P_SETACCEL | P_ADDANIM | P_DEBRIS | P_SPRITE | P_SETVBB | P_SETVBW | P_SETVWW
        | P_SETVWB | P_ADDVBB | P_ADDVBW | P_ADDVWW | P_ADDVWB | P_NEGB | P_NEGW | P_DIV2B
        | P_DIV2W | P_ADDWS | P_SCORE | P_DO => (3, &[]),
        P_GOTO | P_IGOTO => (3, &[1]),
        P_HITWALL | P_SHAPEDEAD | P_RANDOMGOTO | P_PLAYERDEAD | P_IFFLAG | P_ALMOSTDEAD
        | P_GOSUB | P_BREAK | P_ALWAYSOFF | P_FORCE => (3, &[1]),
        P_LEFTOFPLAYER | P_RIGHTOFPLAYER | P_ABOVEPLAYER | P_BELOWPLAYER | P_BEHINDPLAYER => {
            (3, &[1])
        }
        P_ADDW | P_ACHASEW | P_WAITACHASEW | P_SETW | P_SETRANDOMB | P_SETSTRAT | P_IMPORTB
        | P_IMPORTW | P_EXPORTB | P_EXPORTW => (4, &[]),
        P_LOOP => (4, &[2]),
        P_IFLEVEL | P_NOTFRIEND | P_CHILDDEAD => (4, &[2]),
        P_IFHITFLAG => (4, &[1]),
        P_ALWAYS => (4, &[1]),
        P_IFZEROB | P_IFZEROW | P_IFNOTZEROB | P_IFNOTZEROW => (4, &[2]),
        P_DISTLESS | P_SHAPEDISTLESS | P_HITGROUND | P_WITHINRANGE | P_SHAPEINRANGE => (5, &[3]),
        P_IFSAMEB => (5, &[3]),
        P_TEXT | P_SETRANDOMW => (5, &[]),
        P_IFSAMEW | P_IFBETWEENB => (6, &[4]),
        P_QSPAWN => (7, &[3]),
        P_GOTOPOS | P_GOTOSHAPEPOS | P_INDEXB | P_INDEXW | P_IFBETWEENW => match op {
            P_IFBETWEENW => (8, &[6]),
            _ => (8, &[]),
        },
        P_SPAWN | P_SPAWNLINK => (13, &[3]),
        P_SPAWNCHILD => (14, &[3]),
        _ => return None, // P_START65816 (inline machine code) or unknown
    })
}

fn is_terminal(op: u8) -> bool {
    matches!(op, P_END | P_REMOVE | P_GOTO | P_IGOTO | P_EXPLODE | P_SETSTRAT)
}

#[test]
fn path_data_matches_rom_blob() {
    let Some((rom, syms)) = setup() else {
        eprintln!("skip: no built ROM/symbols");
        return;
    };
    let catalog = sf_path::literals::get_catalog();
    let paths_base = lorom_off(syms.get("PATHS"));
    let rom_blob = &rom[paths_base..paths_base + 0x5000];

    use sf_path::ids::*;
    // (name, ROM symbol, rust path id)
    let table: &[(&str, &str, u16)] = &[
        ("e_gate", "PATH_E_GATE", PATH_ID_E_GATE),
        ("e_rabbit", "PATH_E_RABBIT", PATH_ID_E_RABBIT),
        ("e_frog", "PATH_E_FROG", PATH_ID_E_FROG),
        ("e_falcon", "PATH_E_FALCON", PATH_ID_E_FALCON),
        ("tow_0", "PATH_TOW_0", PATH_ID_TOW_0),
        ("tow_1", "PATH_TOW_1", PATH_ID_TOW_1),
        ("korori", "PATH_KORORI", PATH_ID_KORORI),
        ("ponpon", "PATH_PONPON", PATH_ID_PONPON),
    ];

    let mut total_mismatch = 0;
    for &(name, sym, id) in table {
        let rom_start = syms.get(sym) as usize;
        let rust_start = catalog.offsets[id as usize];
        if rust_start == 0xFFFF {
            println!("{name}: not in Rust catalog, skipped");
            continue;
        }
        let rust_start = rust_start as usize;
        println!("--- {name}: ROM +{rom_start:#06x}  RUST +{rust_start:#06x} ---");

        let (mut rp, mut up) = (rom_start, rust_start);
        for _ in 0..200 {
            let rop = rom_blob[rp];
            let uop = catalog.data[up];
            if rop != uop {
                println!(
                    "  OPCODE MISMATCH at ROM+{rp:#06x}/RUST+{up:#06x}: ROM {rop} vs RUST {uop}"
                );
                total_mismatch += 1;
                break;
            }
            let Some((len, addrs)) = op_shape(rop) else {
                println!("  [{rop}] start65816/unknown at ROM+{rp:#06x} — stopping walk");
                break;
            };
            // Compare operand bytes, skipping normalized address operands.
            for i in 1..len {
                if addrs.iter().any(|&a| i == a || i == a + 1) {
                    continue;
                }
                // Shape-id operands legitimately differ (ROM stores shape
                // header addresses; the port stores internal ids): spawn
                // ops bytes 1-2, qspawn bytes 1-2, setstrat all.
                let skip = match rop {
                    P_SPAWN | P_SPAWNLINK | P_SPAWNCHILD | P_QSPAWN => i == 1 || i == 2,
                    P_SETSTRAT | P_MSG | P_MSGWITHMETER | P_SOUND | P_SOUND2 => true,
                    P_FINDSHAPE | P_FINDNEXTSHAPE | P_DEBRIS => i == 1 || i == 2,
                    _ => false,
                };
                if skip {
                    continue;
                }
                if rom_blob[rp + i] != catalog.data[up + i] {
                    println!(
                        "  [{rop}] operand +{i} differs at ROM+{:#06x}: ROM {:#04x} vs RUST {:#04x}",
                        rp + i,
                        rom_blob[rp + i],
                        catalog.data[up + i]
                    );
                    total_mismatch += 1;
                }
            }
            // Address operands: compare relative to the path start; if the
            // target is another known path's start, compare identities.
            for &a in addrs {
                let rt = u16::from_le_bytes([rom_blob[rp + a], rom_blob[rp + a + 1]]) as usize;
                let ut = u16::from_le_bytes([catalog.data[up + a], catalog.data[up + a + 1]])
                    as usize;
                let internal_ok = rt >= rom_start
                    && ut >= rust_start
                    && (rt - rom_start) == (ut - rust_start);
                let cross_ok = table.iter().any(|&(_, s2, id2)| {
                    syms.get(s2) as usize == rt
                        && catalog.offsets[id2 as usize] as usize == ut
                });
                if !internal_ok && !cross_ok {
                    println!(
                        "  [{rop}] addr operand: ROM->+{rt:#06x} (rel {:+}) RUST->+{ut:#06x} (rel {:+}) [unverified or mismatched]",
                        rt as isize - rom_start as isize,
                        ut as isize - rust_start as isize
                    );
                }
            }
            if is_terminal(rop) {
                println!("  walk ok: terminated at [{rop}] after {} bytes", rp + len - rom_start);
                break;
            }
            rp += len;
            up += len;
        }
    }

    // Locked-in finding: tow_0's P_SPAWN(link) y-offset. ROM stores -200/4 =
    // -50 (0xCE) and multiplies by 4 at runtime; the Rust catalog now stores
    // the same coord/4 byte and the interpreter scales x4 after rotation.
    // (tow_0 contains a P_START65816 blob — variable-length machine code on
    // the ROM side — so locate the spawn op with a raw byte scan keyed on the
    // hp/ap payload rather than an opcode walk.)
    let tow0 = syms.get("PATH_TOW_0") as usize;
    let rust_tow0 = catalog.offsets[PATH_ID_TOW_0 as usize] as usize;
    assert_ne!(rust_tow0, 0xFFFF, "tow_0 must be in the Rust catalog");
    let find_spawn = |blob: &[u8], start: usize| -> Option<usize> {
        (start..start + 0x40).find(|&p| {
            (blob[p] == P_SPAWNLINK || blob[p] == P_SPAWN)
                && blob[p + 8] == 10
                && blob[p + 9] == 10
        })
    };
    let rs = find_spawn(rom_blob, tow0).expect("tow_0 spawnlink in ROM blob");
    let us = find_spawn(&catalog.data, rust_tow0).expect("tow_0 spawnlink in Rust catalog");
    let rom_y = rom_blob[rs + 11] as i8;
    let rust_y = catalog.data[us + 11] as i8;
    println!(
        "tow_0 spawnlink y payload: ROM {rom_y} (runtime {}), RUST {rust_y} (runtime {})",
        rom_y as i32 * 4,
        rust_y as i32 * 4
    );
    assert_eq!(rom_y as i32 * 4, -200, "ROM: byte*4 = -200 (DPATHDAT.ASM:248)");
    assert_eq!(rust_y, rom_y, "port stores the same coord/4 payload byte as the ROM");

    println!("total non-address operand mismatches: {total_mismatch}");
    assert_eq!(total_mismatch, 0, "catalog bytes match the ROM blob on all walked paths");
}
