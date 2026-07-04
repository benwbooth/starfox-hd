//! Accuracy audit of the sf-strat core trig/math primitives against the real
//! ROM (STRATROU.ASM + MACROS.INC mulslogmac + SGDATA.ASM sin/cos tables),
//! driven through the sf-oracle 65816 core.
//!
//! Scope: SINTAB/COSTAB byte tables, mulslog (via the vec routines),
//! Strat_GenVecs2D (alvelvecs_l), GenVecs3D (n3dvecs_l), GenSideVecs
//! (sidevecs_l), GenFrontVecs (frontvecs_l), perc56/62/75/87/93, SpeedTo.
//!
//! Field/temp addresses from data/symbols.txt:
//!   al_vel=$15 al_rotx=$12 al_roty=$13 ; x1=$02 y1=$08 z1=$8A ; tpa(target)=$1550

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const AL_ROTY: u32 = 0x13;
const AL_VEL: u32 = 0x15;
const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const Z1: u32 = 0x008A;

const SINTAB_SNES: u32 = 0x0000_8b62;
const COSTAB_SNES: u32 = 0x0000_8ba2;

// ---------------------------------------------------------------------------
// 1. SIN / COS byte tables
// ---------------------------------------------------------------------------
#[test]
fn sincos_tables_match_rom() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };
    let bus = SnesBus::new(rom);
    let mut bad = 0;
    for i in 0..256u32 {
        let rom_sin = bus.read8(SINTAB_SNES + i) as i8;
        let rom_cos = bus.read8(COSTAB_SNES + i) as i8;
        let rs = sf_strat::snes_trig::SINTAB[i as usize];
        let rc = sf_strat::snes_trig::COSTAB[i as usize];
        if rom_sin != rs {
            bad += 1;
            eprintln!("SIN[{i}] ROM={rom_sin} RUST={rs} DIFF");
        }
        if rom_cos != rc {
            bad += 1;
            eprintln!("COS[{i}] ROM={rom_cos} RUST={rc} DIFF");
        }
    }
    eprintln!("sin/cos table byte diffs: {bad}");
    assert_eq!(bad, 0, "SINTAB/COSTAB bytes diverge from ROM");
}

// ---------------------------------------------------------------------------
// 2. GenVecs2D (alvelvecs_l) — exercises mulslog(vel, sin/cos) directly.
//    Sweeps roty 0..255 and vel 0..255 to catch the vel>=128 sign question.
// ---------------------------------------------------------------------------
fn rom_2d(rom: &[u8], addr: u32, roty: u8, vel: u8) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XB + AL_ROTY, roty);
    bus.write8(XB + AL_VEL, vel);
    call(&mut bus, addr, &Entry { x: XB as u16, p: 0x20, ..Default::default() });
    (bus.read16(X1) as i16, bus.read16(Y1) as i16, bus.read16(Z1) as i16)
}
fn rust_2d(roty: u8, vel: u8) -> (i16, i16, i16) {
    let mut al = Alien::default();
    al.roty = roty;
    al.vel = vel;
    sf_strat::common::strat_gen_vecs_2d(&mut al);
    (al.vx, al.vy, al.vz)
}
#[test]
fn gen_vecs_2d_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ALVELVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let mut bad = 0;
    let mut bad_lo = 0; // divergences with vel < 128 (the realistic range)
    let mut first: Vec<String> = Vec::new();
    for roty in (0..=255u8).step_by(7) {
        for vel in (0..=255u8).step_by(5) {
            let r = rom_2d(&rom, addr, roty, vel);
            let g = rust_2d(roty, vel);
            if r != g {
                bad += 1;
                if vel < 128 {
                    bad_lo += 1;
                }
                if first.len() < 12 {
                    first.push(format!("roty={roty:3} vel={vel:3}: ROM{r:?} RUST{g:?}"));
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("gen_vecs_2d diffs: {bad} total, {bad_lo} with vel<128");
    // Reachable range (all speed constants are <=120) is bit-exact.
    assert_eq!(bad_lo, 0, "gen_vecs_2d diverges from ROM for vel<128");
    // LATENT: for vel>=128 the ROM's mulslogmac treats the magnitude byte as
    // signed (|vel|=256-vel, negated result); Rust's mulslog uses the unsigned
    // value. Not reachable with current speed constants. Documented, not asserted.
    if bad != 0 {
        eprintln!(
            "NOTE: {} vel>=128 cases diverge (ROM signed-byte mulslog quirk)",
            bad - bad_lo
        );
    }
}

// ---------------------------------------------------------------------------
// 3. GenVecs3D (n3dvecs_l) — vel sweep at rotx=0 (pitch negation is a no-op at
//    0, so vx/vz/vy are all directly comparable).
// ---------------------------------------------------------------------------
const TROTX: u32 = 0x1630;
const TROTY: u32 = 0x1631;
const TMPZ: u32 = 0x0078;
fn rom_3d(rom: &[u8], addr: u32, roty: u8, rotx: u8, vel: u8) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(TROTX, rotx);
    bus.write8(TROTY, roty);
    bus.write8(TMPZ, vel);
    call(&mut bus, addr, &Entry { p: 0x20, ..Default::default() });
    (bus.read16(X1) as i16, bus.read16(Y1) as i16, bus.read16(Z1) as i16)
}
fn rust_3d(roty: u8, rotx: u8, vel: u8) -> (i16, i16, i16) {
    let mut al = Alien::default();
    al.roty = roty;
    al.rotx = rotx;
    al.vel = vel;
    sf_strat::common::strat_gen_vecs_3d(&mut al);
    (al.vx, al.vy, al.vz)
}
#[test]
fn gen_vecs_3d_vel_sweep() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("N3DVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let mut bad = 0;
    let mut bad_lo = 0;
    let mut first: Vec<String> = Vec::new();
    for &roty in &[0u8, 32, 64, 96, 128, 200] {
        for vel in (0..=255u8).step_by(3) {
            let (x1, y1, z1) = rom_3d(&rom, addr, roty, 0, vel);
            let (vx, vy, vz) = rust_3d(roty, 0, vel);
            // rotx=0 -> pitch negation is identity, y1 == 0, direct compare.
            if (vx, vy, vz) != (x1, y1, z1) {
                bad += 1;
                if vel < 128 {
                    bad_lo += 1;
                }
                if first.len() < 12 {
                    first.push(format!(
                        "roty={roty:3} vel={vel:3}: ROM{:?} RUST{:?}",
                        (x1, y1, z1),
                        (vx, vy, vz)
                    ));
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("gen_vecs_3d(rotx=0) diffs: {bad} total, {bad_lo} with vel<128");
    assert_eq!(bad_lo, 0, "gen_vecs_3d diverges from ROM for vel<128");
    // LATENT vel>=128 signed-byte mulslog quirk (same as gen_vecs_2d).
    if bad != 0 {
        eprintln!("NOTE: {} vel>=128 cases diverge (ROM signed-byte mulslog quirk)", bad - bad_lo);
    }
}

// ---------------------------------------------------------------------------
// 4. GenSideVecs (sidevecs_l) and GenFrontVecs (frontvecs_l).
//    Entry: A=vel, X=base (al_roty,x).  Out: x1,z1.
// ---------------------------------------------------------------------------
fn rom_vec_a(rom: &[u8], addr: u32, roty: u8, vel: u8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XB + AL_ROTY, roty);
    call(
        &mut bus,
        addr,
        &Entry { a: vel as u16, x: XB as u16, p: 0x20, ..Default::default() },
    );
    (bus.read16(X1) as i16, bus.read16(Z1) as i16)
}
#[test]
fn side_and_front_vecs_match_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let (Some(&side), Some(&front)) = (syms.get("SIDEVECS_L"), syms.get("FRONTVECS_L")) else {
        eprintln!("skip: no side/front symbol");
        return;
    };
    let mut bad_side = 0;
    let mut bad_front = 0;
    for roty in (0..=255u8).step_by(3) {
        for &vel in &[0u8, 1, 30, 60, 100, 127] {
            let mut al = Alien::default();
            al.roty = roty;
            al.vel = vel;
            let rs = sf_strat::common::strat_gen_side_vecs(&al);
            let os = rom_vec_a(&rom, side, roty, vel);
            if rs != os {
                if bad_side < 8 {
                    eprintln!("SIDE roty={roty:3} vel={vel:3}: ROM{os:?} RUST{rs:?}");
                }
                bad_side += 1;
            }
            let rf = sf_strat::common::strat_gen_front_vecs(&al);
            let of = rom_vec_a(&rom, front, roty, vel);
            if rf != of {
                if bad_front < 8 {
                    eprintln!("FRONT roty={roty:3} vel={vel:3}: ROM{of:?} RUST{rf:?}");
                }
                bad_front += 1;
            }
        }
    }
    eprintln!("sidevecs diffs: {bad_side}  frontvecs diffs: {bad_front}");
    assert_eq!(bad_side, 0, "gen_side_vecs diverges from ROM");
    assert_eq!(bad_front, 0, "gen_front_vecs diverges from ROM");
}

// ---------------------------------------------------------------------------
// 5. perc56/62/75/87/93 (perc*A_l) — 16-bit signed input in A, result in A.
// ---------------------------------------------------------------------------
fn rom_perc(rom: &[u8], addr: u32, val: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    let e = call(&mut bus, addr, &Entry { a: val as u16, p: 0x00, ..Default::default() });
    e.c as i16
}
#[test]
fn perc_routines_match_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let table: [(&str, fn(i16) -> i16); 5] = [
        ("PERC56A_L", sf_strat::common::strat_perc56),
        ("PERC62A_L", sf_strat::common::strat_perc62),
        ("PERC75A_L", sf_strat::common::strat_perc75),
        ("PERC87A_L", sf_strat::common::strat_perc87),
        ("PERC93A_L", sf_strat::common::strat_perc93),
    ];
    let vals = [
        0i16, 1, 2, 3, 4, 7, 8, 15, 16, 100, 127, 255, 256, 1000, 12345, 32767, -1, -2, -3, -4, -7,
        -8, -15, -100, -255, -1000, -12345, -32768,
    ];
    let mut bad = 0;
    for (name, rustfn) in table {
        let Some(&addr) = syms.get(name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for &v in &vals {
            let o = rom_perc(&rom, addr, v);
            let r = rustfn(v);
            if o != r {
                bad += 1;
                eprintln!("{name}({v}) ROM={o} RUST={r} DIFF");
            }
        }
    }
    eprintln!("perc diffs: {bad}");
    assert_eq!(bad, 0, "perc routines diverge from ROM");
}

// ---------------------------------------------------------------------------
// 5b. AngleXZ (anglexy_l) — ROM uses table-based arctan16; Rust uses f32 atan2
//     (matching the C build). Quantify the divergence.  ABI: X=src base,
//     Y=dst base; reads al_worldx($0C)/al_worldz($10); returns A = angle>>8.
// ---------------------------------------------------------------------------
const AL_WORLDX: u32 = 0x0C;
const AL_WORLDZ: u32 = 0x10;
const XSRC: u32 = 0x0100;
const YDST: u32 = 0x0200;
fn rom_angle(rom: &[u8], addr: u32, dx: i16, dz: i16) -> u8 {
    let mut bus = SnesBus::new(rom.to_vec());
    // src at origin, dst at (dx, dz).
    bus.write16(XSRC + AL_WORLDX, 0);
    bus.write16(XSRC + AL_WORLDZ, 0);
    bus.write16(YDST + AL_WORLDX, dx as u16);
    bus.write16(YDST + AL_WORLDZ, dz as u16);
    let e = call(
        &mut bus,
        addr,
        &Entry { x: XSRC as u16, y: YDST as u16, p: 0x20, ..Default::default() },
    );
    e.a
}
fn rust_angle(dx: i16, dz: i16) -> u8 {
    let src = Alien::default();
    let mut dst = Alien::default();
    dst.worldx = dx;
    dst.worldz = dz;
    sf_strat::common::strat_angle_xz(&src, &dst)
}
#[test]
fn angle_xz_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ANGLEXY_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    // arctan16_l ($03:8550 in GAME.ASM) is an `rtl` stub; the real arctan runs
    // on the SuperFX via `runmario_l`, which the 65816 `call` harness does NOT
    // execute. So the ROM side returns a constant 0 and cannot validate
    // angle_xz. Detect that and skip rather than emit misleading data. The
    // proper oracle path for arctan is the GSU (see tests/gsu_arctan.rs, whose
    // divide-refinement is still WIP).
    let probe = rom_angle(&rom, addr, 1000, 0); // due east -> expect ~64
    if probe == 0 {
        eprintln!(
            "SKIP angle_xz: arctan16_l is a SuperFX stub in the 65816 core \
             (probe dx=1000,dz=0 -> {probe}, expected ~64). Validate via the GSU \
             harness (gsu_arctan.rs), not `call`."
        );
        return;
    }
    let mut worst = 0i32;
    for &dx in &[-2000i16, -1000, -300, -50, 0, 50, 300, 1000, 2000] {
        for &dz in &[-2000i16, -1000, -300, -50, 0, 50, 300, 1000, 2000] {
            if dx == 0 && dz == 0 {
                continue;
            }
            let o = rom_angle(&rom, addr, dx, dz) as i32;
            let r = rust_angle(dx, dz) as i32;
            let d0 = (o - r).rem_euclid(256);
            let d = d0.min(256 - d0);
            worst = worst.max(d);
            if d > 2 {
                eprintln!("dx={dx:5} dz={dz:5}: ROM={o:3} RUST={r:3} circdist={d}");
            }
        }
    }
    eprintln!("angle_xz: worst circular dist vs ROM = {worst}");
}

// ---------------------------------------------------------------------------
// 6. SpeedTo (sr_speedto) — broad sweep incl. |vel-target| >= 128.
// ---------------------------------------------------------------------------
const TARGET: u32 = 0x1550;
fn rom_speedto(rom: &[u8], addr: u32, vel: u8, target: u8, rate: u8) -> u8 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XB + AL_VEL, vel);
    bus.write8(TARGET, target);
    call(&mut bus, addr, &Entry { a: rate as u16, x: XB as u16, p: 0x20, ..Default::default() });
    bus.read8(XB + AL_VEL)
}
fn rust_speedto(vel: u8, target: u8, rate: u8) -> u8 {
    let mut al = Alien::default();
    al.vel = vel;
    sf_strat::common::strat_speed_to(&mut al, target, rate);
    al.vel
}
#[test]
fn speedto_sweep() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("SR_SPEEDTO"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let mut bad = 0;
    let mut bad_reasonable = 0; // |diff|<128 && rate<128
    let mut first: Vec<String> = Vec::new();
    for vel in (0..=255u8).step_by(9) {
        for target in (0..=255u8).step_by(13) {
            for &rate in &[1u8, 2, 5, 40] {
                let o = rom_speedto(&rom, addr, vel, target, rate);
                let r = rust_speedto(vel, target, rate);
                if o != r {
                    bad += 1;
                    let diff = (vel as i16 - target as i16).unsigned_abs();
                    if diff < 128 && rate < 128 {
                        bad_reasonable += 1;
                    }
                    if first.len() < 16 {
                        first.push(format!(
                            "vel={vel:3} target={target:3} rate={rate:3}: ROM={o:3} RUST={r:3} (|diff|={diff})"
                        ));
                    }
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("speedto diffs: {bad} total, {bad_reasonable} with |diff|<128");
    assert_eq!(bad_reasonable, 0, "speedto diverges from ROM for |diff|<128 && rate<128");
    // LATENT: for |vel-target|>=128 the ROM's 8-bit signed compare flips the
    // step direction and wraps the long way round the 256 circle; Rust steps the
    // short way. Not reachable (all speeds/targets are <128). Documented only.
    if bad != 0 {
        eprintln!("NOTE: {} |diff|>=128 cases diverge (ROM 8-bit signed-cmp wrap quirk)", bad - bad_reasonable);
    }
}
