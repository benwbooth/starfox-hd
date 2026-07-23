//! Differential audit: the mothermap interpreter — ROM `bemother_l`
//! (MOTHER.ASM:36-77, BEMOTHER_L $3ef61) vs the Rust port
//! (`sf-strat::mother::bemother_on`).
//!
//! The ROM routine is entered with X = the mother alien block; it reads the
//! mothermap entry at `$05:8000 + al_ptr` (`lda.l moth_count,x` — the
//! mothermap struct base `mapbase` is $05:8000, symbols MOTH_CTRL/MOTH_COUNT),
//! stores the entry's count into `al_sword1`, dispatches on `moth_ctrl`, and
//! on exit subtracts `lastzchange` ($1786) from a positive `al_sword1`.
//! motherobj (MOTHER.ASM:81) allocates off `alfreelst`, `init_objvars_l`s the
//! block, and places it at mother pos + (mo_x,mo_y,mo_z); motherrnd (:306)
//! draws 6 RANDOM bytes (x lo/hi, y lo/hi, z lo/hi from the $DE-$E1 SWB
//! chain) and offsets each masked axis by `(rand & (mask-1)) - mask/2`.
//!
//! We host a hand-built mothermap in the ROM image at $05:8000+MAPOFF (the
//! bus never writes ROM, so the bytes are patched into the ROM vec before
//! boot), seed a mother + one free block in WRAM, JSL BEMOTHER_L via a WRAM
//! trampoline, and diff the mother's al_sword1/al_ptr and the child's world
//! position tick-for-tick against `bemother_on` on the identical blob.

use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};

// Direct-page / low-WRAM executor vars (symbols.txt).
const ALLST: u32 = 0x12ad;
const ALFREELST: u32 = 0x12af;
const LASTZCHANGE: u32 = 0x1786;
const RAND: u32 = 0x00DE; // RANDOM state $DE..$E1

// Alien struct field offsets (symbols.txt).
const AL_SHAPE: u32 = 0x04;
const AL_PTR: u32 = 0x06;
const AL_TYPE: u32 = 0x09;
const AL_WORLDX: u32 = 0x0c;
const AL_WORLDY: u32 = 0x0e;
const AL_WORLDZ: u32 = 0x10;
const AL_SWORD1: u32 = 0x26;

// WRAM layout (clear of the $0200 stub and $0300 STP trap).
const MOTHER: u32 = 0x0140;
const CHILD: u32 = 0x0180;
const TRAMPOLINE: u32 = 0x0400;

/// Mothermap offset within bank $05 (al_ptr value). $05:8000+MAPOFF is ROM
/// file offset (5<<15)+MAPOFF under the bus's LoROM mapping.
const MAPOFF: u16 = 0x0100;

fn setup() -> Option<(std::collections::HashMap<String, u32>, Vec<u8>)> {
    let syms = load_symbols();
    if syms.is_empty() {
        return None;
    }
    let rom = load_built_rom()?;
    Some((syms, rom))
}

/// Boot the bus with `map_bytes` hosted at $05:8000+MAPOFF and a mother at
/// MOTHER (al_ptr = MAPOFF, world pos 5/-5/4000), one free child block, and
/// lastzchange = 65. Returns the prepared bus.
fn make_bus(rom: &[u8], map_bytes: &[u8], rng_seed: Option<[u8; 4]>) -> SnesBus {
    let mut rom = rom.to_vec();
    let base = (5usize << 15) + MAPOFF as usize;
    rom[base..base + map_bytes.len()].copy_from_slice(map_bytes);
    let mut bus = SnesBus::new(rom);

    // Mother block.
    bus.write16(MOTHER + AL_PTR, MAPOFF);
    bus.write16(MOTHER + AL_SWORD1, 0);
    bus.write16(MOTHER + AL_WORLDX, 5i16 as u16);
    bus.write16(MOTHER + AL_WORLDY, -5i16 as u16);
    bus.write16(MOTHER + AL_WORLDZ, 4000u16);
    // Free list: one block; active list head = the mother.
    bus.write16(ALLST, MOTHER as u16);
    bus.write16(ALFREELST, CHILD as u16);
    bus.write16(CHILD, 0); // _next
    bus.write16(LASTZCHANGE, 65);
    if let Some(seed) = rng_seed {
        for (i, b) in seed.iter().enumerate() {
            bus.write8(RAND + i as u32, *b);
        }
    }
    bus
}

/// JSL BEMOTHER_L (RTL routine) via a WRAM trampoline ending in STP.
fn call_bemother(bus: &mut SnesBus, bemother: u32) {
    let tramp = [
        0x22,
        bemother as u8,
        (bemother >> 8) as u8,
        (bemother >> 16) as u8, // JSL bemother_l
        0xDB,                   // STP
    ];
    for (i, b) in tramp.iter().enumerate() {
        bus.write8(TRAMPOLINE + i as u32, *b);
    }
    call_near(
        bus,
        TRAMPOLINE,
        &Entry {
            x: MOTHER as u16,
            p: 0x00,
            ..Default::default()
        },
    );
}

/// Encode [motherobj count=100 off=(10,20,30) shape=42 strat=0][goto 0,MAPOFF]
/// exactly like MAPMACS.INC motherobj/mothergoto (and sf-map's encoder).
fn obj_goto_map() -> Vec<u8> {
    let mut v = Vec::new();
    v.push(0u8); // ctrlmotherobj
    v.extend_from_slice(&100u16.to_le_bytes());
    v.extend_from_slice(&10i16.to_le_bytes());
    v.extend_from_slice(&20i16.to_le_bytes());
    v.extend_from_slice(&30i16.to_le_bytes());
    v.extend_from_slice(&42u16.to_le_bytes());
    v.extend_from_slice(&[0, 0, 0]); // mo_strat = 0
    v.push(8u8); // ctrlmothergoto
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&MAPOFF.to_le_bytes());
    v
}

/// Same blob laid out for the Rust interpreter: MAPOFF pad bytes so blob
/// offsets equal the ROM's $8000-relative al_ptr values.
fn rust_blob(map: &[u8]) -> Vec<u8> {
    let mut blob = vec![0xFFu8; MAPOFF as usize];
    blob.extend_from_slice(map);
    blob
}

fn rust_game_with_mother() -> (sf_game::Game, u16) {
    let mut g = sf_game::Game::new();
    let m = g.objs.alloc().expect("slot");
    {
        let al = &mut g.objs.aliens[m as usize];
        al.worldx = 5;
        al.worldy = -5;
        al.worldz = 4000;
        al.ptr = MAPOFF;
    }
    g.world.lastzchange = 65;
    (g, m)
}

/// motherobj + wait cadence + goto: ROM and Rust agree tick-for-tick on
/// al_sword1, al_ptr and the spawned child's world position.
#[test]
fn bemother_obj_wait_goto_matches_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: sf-oracle/data/sf.sfc or symbols.txt missing");
        return;
    };
    let Some(&bemother) = syms.get("BEMOTHER_L") else {
        eprintln!("skip: BEMOTHER_L not in symbols");
        return;
    };

    let map = obj_goto_map();
    let mut bus = make_bus(&rom, &map, None);

    // ---- ROM tick 1: spawn + wait load ----
    call_bemother(&mut bus, bemother);
    let rom_child = (
        bus.read16(CHILD + AL_WORLDX) as i16,
        bus.read16(CHILD + AL_WORLDY) as i16,
        bus.read16(CHILD + AL_WORLDZ) as i16,
        bus.read16(CHILD + AL_SHAPE),
        bus.read8(CHILD + AL_TYPE),
    );
    let rom_t1 = (
        bus.read16(MOTHER + AL_SWORD1) as i16,
        bus.read16(MOTHER + AL_PTR),
    );
    // ---- ROM tick 2: pure wait ----
    call_bemother(&mut bus, bemother);
    let rom_t2 = (
        bus.read16(MOTHER + AL_SWORD1) as i16,
        bus.read16(MOTHER + AL_PTR),
    );
    // ---- ROM tick 3: goto wraps, obj re-fires (free list empty -> skip) ----
    call_bemother(&mut bus, bemother);
    let rom_t3 = (
        bus.read16(MOTHER + AL_SWORD1) as i16,
        bus.read16(MOTHER + AL_PTR),
    );

    // ---- Rust, identical setup ----
    let blob = rust_blob(&map);
    let (mut g, m) = rust_game_with_mother();
    sf_strat::mother::bemother_on(&mut g, m, &blob);
    let ci = g
        .objs
        .active_indices()
        .into_iter()
        .find(|&i| i != m && g.objs.aliens[i as usize].shape == 42)
        .expect("rust child");
    let c = &g.objs.aliens[ci as usize];
    assert_eq!(
        (c.worldx, c.worldy, c.worldz, c.shape, c.type_),
        rom_child,
        "child spawn (mother pos + mo_x/y/z, shape, atzremove)"
    );
    let rust_t1 = (
        g.objs.aliens[m as usize].sword1,
        g.objs.aliens[m as usize].ptr,
    );
    sf_strat::mother::bemother_on(&mut g, m, &blob);
    let rust_t2 = (
        g.objs.aliens[m as usize].sword1,
        g.objs.aliens[m as usize].ptr,
    );
    sf_strat::mother::bemother_on(&mut g, m, &blob);
    let rust_t3 = (
        g.objs.aliens[m as usize].sword1,
        g.objs.aliens[m as usize].ptr,
    );

    assert_eq!(rust_t1, rom_t1, "tick1 (al_sword1, al_ptr)");
    assert_eq!(rust_t2, rom_t2, "tick2 (wait only)");
    assert_eq!(rust_t3, rom_t3, "tick3 (goto + refire)");
    // Sanity on the ROM values themselves (count 100 - lastzchange 65).
    assert_eq!(rom_t1, (35, MAPOFF + 14));
    assert_eq!(rom_t2, (-30, MAPOFF + 14));
    assert_eq!(rom_t3, (35, MAPOFF + 14));
}

/// motherrnd: with the same $DE-$E1 SWB seed, ROM and Rust produce the
/// identical random spawn offset (6 RNG bytes, mask-and-center math).
#[test]
fn bemother_rnd_offsets_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: sf-oracle/data/sf.sfc or symbols.txt missing");
        return;
    };
    let Some(&bemother) = syms.get("BEMOTHER_L") else {
        eprintln!("skip: BEMOTHER_L not in symbols");
        return;
    };

    // [motherrnd count=200 masks=(1024,2048,0) shape=7 strat=0][goto 0,MAPOFF]
    let mut map = Vec::new();
    map.push(6u8); // ctrlmotherrnd
    map.extend_from_slice(&200u16.to_le_bytes());
    map.extend_from_slice(&1024u16.to_le_bytes());
    map.extend_from_slice(&2048u16.to_le_bytes());
    map.extend_from_slice(&0u16.to_le_bytes());
    map.extend_from_slice(&7u16.to_le_bytes());
    map.extend_from_slice(&[0, 0, 0]);
    map.push(8u8);
    map.extend_from_slice(&0u16.to_le_bytes());
    map.extend_from_slice(&MAPOFF.to_le_bytes());

    for seed in [[1u8, 2, 3, 4], [0xAB, 0xCD, 0x12, 0x99], [0, 0, 0, 1]] {
        let mut bus = make_bus(&rom, &map, Some(seed));
        call_bemother(&mut bus, bemother);
        let rom_child = (
            bus.read16(CHILD + AL_WORLDX) as i16,
            bus.read16(CHILD + AL_WORLDY) as i16,
            bus.read16(CHILD + AL_WORLDZ) as i16,
        );

        let blob = rust_blob(&map);
        let (mut g, m) = rust_game_with_mother();
        g.vars.rng = seed;
        sf_strat::mother::bemother_on(&mut g, m, &blob);
        let ci = g
            .objs
            .active_indices()
            .into_iter()
            .find(|&i| i != m && g.objs.aliens[i as usize].shape == 7)
            .expect("rust child");
        let c = &g.objs.aliens[ci as usize];
        assert_eq!(
            (c.worldx, c.worldy, c.worldz),
            rom_child,
            "motherrnd offsets diverge for seed {seed:?}"
        );
        // Zero z-mask: exact mother z in both.
        assert_eq!(rom_child.2, 4000);
    }
}
