//! ROM SR_SPEEDTO ($1FD625, RTL): moves al_vel ($15,X) toward target ($1550)
//! by rate (entry A), snapping when |vel-target| < rate. Disasm proves the
//! move never overshoots (snap handles the near case), so vel±rate stays on
//! target's side. The Rust does `vel = vel.wrapping_add(rate)` then clamps,
//! which can WRAP past 255 for large vel+rate and land the wrong side. ABI:
//! A=rate, $1550=target, al_vel=$15,X, 8-bit A (p=$20).

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const AL_VEL: u32 = 0x15;
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
fn speedto_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("SR_SPEEDTO"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let cases: [(u8, u8, u8); 8] = [
        (5, 8, 2),
        (5, 8, 10),   // snap up
        (10, 5, 2),
        (10, 5, 10),  // snap down
        (8, 8, 2),    // already at target
        (200, 255, 100), // vel+rate overflow: ROM snaps 255, Rust may wrap
        (250, 255, 40),  // vel+rate = 290 -> wrap
        (100, 200, 60),  // move up, no overshoot
    ];
    let mut diffs = 0;
    for (vel, target, rate) in cases {
        let o = rom_speedto(&rom, addr, vel, target, rate);
        let r = rust_speedto(vel, target, rate);
        if o != r { diffs += 1; }
        eprintln!("vel={vel:3} target={target:3} rate={rate:3}: ROM {o:3}  RUST {r:3}  {}",
            if o == r { "ok" } else { "DIFF" });
    }
    eprintln!("=> {diffs} diff(s)");
    assert_eq!(diffs, 0, "strat_speed_to diverges from ROM SR_SPEEDTO");
}
