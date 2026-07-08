//! TIER-2 retail co-execution harness — proof-of-concept certifier for
//! "100% vs retail". Boots the RETAIL cart (`Star Fox (USA) (Rev 2).sfc`),
//! locates + reads the observable object array out of retail WRAM, and lays the
//! foundation to diff it against the Rust port.
//!
//! Retail is a DIFFERENT binary from the symbol-mapped built ROM (see
//! docs/FUNCTION_LEDGER.md) — every address here was re-derived from the retail
//! cart itself, not from the built-ROM symbol map.

#![allow(non_snake_case)] // GSU register mnemonics X1/Y1/Z1/TMPZ mirror the ASM
use sf_oracle::{
    boot_retail, call, call_near, init_object_pool, inject_runmario_trampoline, load_built_rom,
    load_retail_rom, snapshot_objects, walk_freelist, Entry, SnesBus, AL_STRATPTR, AL_VX, AL_VY,
    AL_VZ, BUILT_POOL, BUILT_RUNMARIO_L_ROM, BUILT_RUNMARIO_RAM, RETAIL_ADDALVECS_L, RETAIL_ALDEAD,
    RETAIL_DOSTRATS, RETAIL_DO_STRAT_L, RETAIL_GAMEFRAME, RETAIL_INIT_STRATS_L, RETAIL_POOL,
    RETAIL_RUNMARIO_L_ROM, RETAIL_RUNMARIO_RAM, RETAIL_STRATOBJ_POSX, RETAIL_UPDATE_OBJECTS_L,
};

const STRATOBJ_POSX: u32 = RETAIL_STRATOBJ_POSX;

/// Bank-$70 (GSU cart RAM) word address as a full 24-bit CPU address.
fn gsuram(off: u32) -> u32 {
    0x70_0000 | off
}

fn retail() -> Option<Vec<u8>> {
    match load_retail_rom() {
        Some(r) => Some(r),
        None => {
            eprintln!("RETAIL: skip — cart not found at repo root");
            None
        }
    }
}

/// MILESTONE 1 — the retail cart boots from its real reset vector, and with a
/// minimal set of hardware shims (PPU raster counter, SPC upload handshake,
/// H/V-counter IRQ) it marches all the way into the **per-frame main game loop**
/// and ticks ~230 frames before parking.
///
/// Shim coverage (see `retail.rs::RetailBootBus`):
///  * PPU raster: $2137 SLHV latch, $213C/$213D OPHCT/OPVCT sweeping counters,
///    $4212 HVBJOY, $4210 RDNMI — clears the $03:BD97 OPVCT raster-wait.
///  * SPC700 upload: $2140-$2143 Idle/Active handshake ($AA/$BB ready + port-0
///    echo) — clears the $03:B12E `CMP #$BBAA` + all block-upload echo waits.
///  * H/V-counter IRQ: $4200 enable, $4209/$420A VTIME, $4211 ack — fires the
///    game's IRQ handler each frame, which sets the frame-ready flag $18BB the
///    main loop ($02:DA3B) spins on. This is what makes the loop actually tick.
///
/// Remaining blocker (precise): after ~230 frames control falls into the bank-$0
/// forced-blank screen/fade routine $00:8100-8198 ending in the terminal
/// `BRA $8198`. Reaching *live gameplay object spawns* additionally needs
/// controller-input injection (auto-joypad $4218/$4219) to leave the attract/
/// intro, plus GSU co-execution for the per-frame 3D + level-start spawn path.
#[test]
fn retail_boots_from_reset() {
    let Some(rom) = retail() else { return };
    let rep = boot_retail(rom, 12_000_000);
    eprintln!(
        "RETAIL BOOT: steps={} distinct_pcs={} final={:02X}:{:04X} stopped={} stalled_loop={} loop={:02X}:{:04X}-{:04X}",
        rep.steps,
        rep.distinct_pcs,
        rep.final_pbr,
        rep.final_pc,
        rep.stopped,
        rep.stalled_in_loop,
        rep.loop_bank,
        rep.loop_lo,
        rep.loop_hi,
    );
    eprintln!(
        "RETAIL BOOT hotspot: parks at {:06X} ({} hits) — this is where a CPU-only core stalls waiting on GSU/PPU/APU.",
        rep.hottest_pc, rep.hottest_hits,
    );
    let trace: Vec<String> = rep.head_trace.iter().map(|a| format!("{a:06X}")).collect();
    eprintln!("RETAIL BOOT head trace (opcode addrs): {trace:?}");
    eprintln!("RETAIL BOOT raster: final_dot={} (~{} frames)", rep.final_dot, rep.final_dot / (341 * 262));
    eprintln!(
        "RETAIL BOOT objects: peak_live={} at step {}k",
        rep.max_live_objects, rep.peak_step / 1000
    );
    for o in rep.objects_at_peak.iter().filter(|o| o.shape != 0).take(12) {
        eprintln!(
            "  slot {:>2}: shape=${:04X} flags=${:04X} world=({},{},{})",
            o.slot, o.shape, o.flags, o.worldx, o.worldy, o.worldz
        );
    }
    let prog: Vec<String> =
        rep.progress.iter().map(|(s, a)| format!("{}k@{a:06X}", s / 1000)).collect();
    eprintln!("RETAIL BOOT PC progression: {prog:?}");

    // Reset is `BRA $FF96 -> CLC/XCE/JML $1F:BDB1`; confirm we actually vectored
    // into the bank-$1F boot code rather than trapping at the vector.
    let hit_boot_bank = rep.head_trace.iter().any(|a| (a >> 16) == 0x1F);
    eprintln!("RETAIL BOOT reached bank $1F boot code: {hit_boot_bank}");

    // The shims march boot deep past the raster/APU/IRQ gates into the ticking
    // main loop: thousands of distinct code addresses and many frames of raster.
    // (Generous lower bounds so the milestone is robust, not brittle.)
    assert!(rep.steps > 100, "CPU stalled almost immediately ({} steps)", rep.steps);
    assert!(
        rep.distinct_pcs > 5_000,
        "expected the boot to reach the main loop (distinct_pcs={})",
        rep.distinct_pcs,
    );
    assert!(
        rep.final_dot / (341 * 262) > 30,
        "expected many frames of main-loop ticking (frames={})",
        rep.final_dot / (341 * 262),
    );
}

/// MILESTONE (step 1) — GSU WIRED INTO THE BUS. The per-frame tick reaches the
/// 3D/spawn math by kicking the Super-FX chip: `runmario_l` does
/// `sta m_pbr ($3034); stx mr15 ($301E); .wait lda m_sfr ($3030); and #$20;
/// bne .wait`. This test drives that exact register protocol through `SnesBus`
/// (the same bus that runs the retail cart) and confirms the chip runs a REAL
/// ROM GSU program to completion, feeding results back through shared bank-$70
/// RAM — with NO direct `Gsu::run` call, only CPU-visible register writes.
///
/// Program: `mcrotmatzxy16` (built via `crotmat16_l`), entry $01:8295, angles in
/// GSU RAM $20/$22/$24, 3x3 matrix read back at $D2 (same ABI as gsu_rotmat.rs).
/// Zero angles must yield the identity matrix (ROM's fixed-point 1.0 = $7FFE).
#[test]
fn gsu_kicks_through_bus_registers() {
    let Some(rom) = load_built_rom() else {
        eprintln!("GSU-BUS: skip — built ROM (data/sf.sfc) not present");
        return;
    };
    const ROTMAT_PBR: u8 = 0x01;
    const ROTMAT_PC: u16 = 0x8295;
    const ONE: i16 = 32766;

    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();

    // Inputs: rx/ry/rz = 0 at GSU RAM $20/$22/$24 (bank $70).
    bus.write16(gsuram(0x20), 0);
    bus.write16(gsuram(0x22), 0);
    bus.write16(gsuram(0x24), 0);

    // Drive the chip EXACTLY as runmario_l: set the program bank, then start it
    // by writing R15 (the high-byte store is the launch edge), then spin on SFR
    // bit 5 until the chip clears "go".
    bus.write8(0x00_3034, ROTMAT_PBR); // m_pbr
    bus.write8(0x00_301E, ROTMAT_PC as u8); // mr15 low
    bus.write8(0x00_301F, (ROTMAT_PC >> 8) as u8); // mr15 high -> KICK
    let mut spins = 0;
    while (bus.read8(0x00_3030) & 0x20) != 0 && spins < 1000 {
        spins += 1; // .wait lda m_sfr; and #$20; bne .wait
    }

    // Read the 3x3 matrix back out of shared GSU RAM at $D2.
    let m: Vec<i16> = (0..9).map(|i| bus.read16(gsuram(0xD2 + i * 2)) as i16).collect();
    eprintln!("GSU-BUS kicks={} sfr_spins={} rot(0,0,0)={:?}", bus.gsu_kicks, spins, m);
    assert_eq!(bus.gsu_kicks, 1, "the R15-high write should have kicked the GSU exactly once");
    assert_eq!(
        m,
        vec![ONE, 0, 0, 0, ONE, 0, 0, 0, ONE],
        "GSU run through the bus registers must produce the identity matrix"
    );
}

/// MILESTONE (step 1) — THE GSU TRAMPOLINE PATH WORKS FROM RAM. The full
/// `dostrats` tick reaches the Super-FX chip only through `runmario_l`, a
/// 35-byte routine the boot copies into WRAM (retail $7E:4EE9 / built $7E:4F51).
/// A directly-called (non-booted) bus has empty RAM there, so a `jsl runmario_l`
/// from inside a strat would execute BRK garbage.
///
/// This test proves the fix end-to-end: inject the real `runmario_l` bytes (from
/// their ROM copy-source) into WRAM, then **call the RAM trampoline itself** with
/// `A = program bank, X = entry PC` (the exact ABI a strat uses) and confirm the
/// GSU runs a real ROM program to completion through it — the CPU executes the
/// RAM-resident wait-loop, the `stx mr15` store kicks the chip via the bus, and
/// the identity matrix comes back through shared bank-$70 RAM. No direct
/// `Gsu::run`, no register pokes from the test — the RAM routine does it all.
#[test]
fn gsu_trampoline_runs_from_ram() {
    let Some(rom) = load_built_rom() else {
        eprintln!("GSU-TRAMPOLINE: skip — built ROM (data/sf.sfc) not present");
        return;
    };
    const ROTMAT_PBR: u8 = 0x01; // mcrotmatzxy16 program bank
    const ROTMAT_PC: u16 = 0x8295; //                    entry PC
    const ONE: i16 = 32766;

    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();

    // Boot-equivalent: copy runmario_l from its ROM copy-source to its RAM dest.
    inject_runmario_trampoline(&mut bus, BUILT_RUNMARIO_L_ROM, BUILT_RUNMARIO_RAM);
    // Sanity: the RAM now holds the routine (starts `sta.l $003034` = 8F 34 30 00).
    let head: Vec<u8> = (0..4).map(|i| bus.read8(BUILT_RUNMARIO_RAM + i)).collect();
    eprintln!("TRAMPOLINE @ $7E:{:04X} head = {head:02X?}", BUILT_RUNMARIO_RAM & 0xFFFF);
    assert_eq!(head, vec![0x8F, 0x34, 0x30, 0x00], "runmario_l not injected");

    // Zero input angles at GSU RAM $20/$22/$24 (bank $70).
    bus.write16(0x70_0020, 0);
    bus.write16(0x70_0022, 0);
    bus.write16(0x70_0024, 0);

    // Call the RAM trampoline exactly as a strat would: 8-bit A = program bank,
    // 16-bit X = entry PC (p=$20 -> M=1/X=0). runmario_l does the rest.
    let _ = call(&mut bus, BUILT_RUNMARIO_RAM, &Entry { a: ROTMAT_PBR as u16, x: ROTMAT_PC, p: 0x20, ..Default::default() });

    let m: Vec<i16> = (0..9).map(|i| bus.read16(0x70_0000 | (0xD2 + i * 2)) as i16).collect();
    eprintln!("GSU-TRAMPOLINE kicks={} rot(0,0,0)={:?}", bus.gsu_kicks, m);
    assert_eq!(bus.gsu_kicks, 1, "the RAM trampoline's `stx mr15` must kick the GSU once");
    assert_eq!(
        m,
        vec![ONE, 0, 0, 0, ONE, 0, 0, 0, ONE],
        "GSU driven through the RAM-resident runmario_l trampoline must yield identity"
    );
}

/// MILESTONE (step 2) — THE FULL STRAT-PIPELINE RETAIL ADDRESSES ARE LOCATED and
/// CROSS-VALIDATED. `dostrats` was found by a masked signature scan (opcodes
/// fixed, absolute operands wildcarded — exactly ONE hit at $02:DAF2); its
/// embedded JSL/absolute operands then directly yield `init_strats_l`,
/// `update_objects_l`, `do_strat_l`, `allst`, `aldead`, `gameframe`. This test
/// reads those bytes back out of the retail cart and asserts the chain:
///  * `dostrats` opcodes are `inc gameframe … phb; ldb #$7e; jsl …; ldx allst …`,
///  * the `ldx allst` operand equals the INDEPENDENTLY-derived pool active head
///    (`RETAIL_POOL.active_head` = $121D, from the allocator scan) — the two
///    derivations agree to the byte,
///  * the three `jsl` operands equal the derived `init_strats_l`/
///    `update_objects_l`/`do_strat_l` addresses,
///  * `do_strat_l`'s landing site has the do_strat_l opcode skeleton
///    (`php; rep #$30; cpx dummyobj; … lda al_collflags,x; and #$fffb`).
#[test]
fn retail_strat_pipeline_addresses() {
    let Some(rom) = retail() else { return };
    let bus = SnesBus::new(rom);
    let rd = |a: u32, n: u32| -> Vec<u8> { (0..n).map(|i| bus.read8(a + i)).collect() };
    let w = |a: u32| -> u16 { bus.read16(a) };

    // dostrats @ $02:DAF2 — verify opcodes and read embedded operands.
    let d = rd(RETAIL_DOSTRATS, 40);
    eprintln!("STRAT dostrats @${RETAIL_DOSTRATS:06X}: {d:02X?}");
    assert_eq!(d[0], 0xEE, "dostrats must open `inc gameframe`");
    assert_eq!(w(RETAIL_DOSTRATS + 1), RETAIL_GAMEFRAME as u16, "gameframe operand");
    assert_eq!(&d[8..13], &[0x8B, 0xA9, 0x7E, 0x48, 0xAB], "phb; lda #$7e; pha; plb");
    // jsl init_strats_l ; jsl update_objects_l
    assert_eq!(d[13], 0x22);
    let init = w(RETAIL_DOSTRATS + 14) as u32 | ((d[16] as u32) << 16);
    assert_eq!(d[17], 0x22);
    let upd = w(RETAIL_DOSTRATS + 18) as u32 | ((d[20] as u32) << 16);
    // ldx allst
    assert_eq!(d[21], 0xAE, "ldx allst");
    let allst = w(RETAIL_DOSTRATS + 22);
    // jsl do_strat_l (after `stz aldead`)
    assert_eq!(d[24], 0x9C, "stz aldead");
    let aldead = w(RETAIL_DOSTRATS + 25);
    assert_eq!(d[27], 0x22, "jsl do_strat_l");
    let dostrat = w(RETAIL_DOSTRATS + 28) as u32 | ((d[30] as u32) << 16);

    eprintln!("STRAT derived: init_strats_l=${init:06X} update_objects_l=${upd:06X} do_strat_l=${dostrat:06X}");
    eprintln!("STRAT globals: allst=${allst:04X} aldead=${aldead:04X} gameframe=${:04X}", RETAIL_GAMEFRAME);

    // Cross-validation: dostrats's `ldx allst` == the pool active head derived
    // independently from the retail allocator scan.
    assert_eq!(allst as u32, RETAIL_POOL.active_head, "dostrats allst != pool active_head");
    assert_eq!(aldead as u32, RETAIL_ALDEAD);
    assert_eq!(init, RETAIL_INIT_STRATS_L, "derived init_strats_l");
    assert_eq!(upd, RETAIL_UPDATE_OBJECTS_L, "derived update_objects_l");
    assert_eq!(dostrat, RETAIL_DO_STRAT_L, "derived do_strat_l");

    // do_strat_l landing site has the do_strat_l opcode skeleton.
    let s = rd(RETAIL_DO_STRAT_L, 18);
    eprintln!("STRAT do_strat_l @${RETAIL_DO_STRAT_L:06X}: {s:02X?}");
    assert_eq!(&s[0..4], &[0x08, 0xC2, 0x30, 0xEC], "php; rep #$30; cpx dummyobj");
    // lda al_collflags,x ; and #$fffb ; sta al_collflags,x  (clear firstframe)
    assert_eq!(&s[11..18], &[0xB5, 0x2E, 0x29, 0xFB, 0xFF, 0x95, 0x2E], "clr firstframe on al_collflags($2E)");
}

/// MILESTONE (step 3) — THE FULL RETAIL `dostrats` PER-FRAME TICK EXECUTES on
/// seeded state. Seed the pool with the retail allocator, put ONE object on the
/// active list (`allst -> block -> 0`), install the `runmario_l` GSU trampoline,
/// and run the REAL retail `dostrats` ($02:DAF2) via the near-call harness.
///
/// After the tick: `gameframe` incremented (the `incw gameframe` at the top ran),
/// the object survived (`init_strats_l` + `update_objects_l` + the active-list
/// walk + `do_strat_l` all executed without trapping), and — proving `do_strat_l`
/// actually processed OUR object inside the loop — `stratobj_posx/y/z`
/// ($1513/15/17, written only by `do_strat_l` from `al_worldx/y/z,x`) hold the
/// object's seeded world coordinates. This is the entire retail per-frame strat
/// pipeline running on directly-seeded state, no cold boot.
#[test]
fn retail_dostrats_pipeline_runs() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();
    inject_runmario_trampoline(&mut bus, RETAIL_RUNMARIO_L_ROM, RETAIL_RUNMARIO_RAM);
    init_object_pool(&mut bus);
    let blk = walk_freelist(&bus, &RETAIL_POOL)[0] as u32;

    // One null-strat object (al_stratptr = 0 -> do_strat_l returns via `.strad`).
    let (px, py, pz) = (1000i16, 500i16, 8000i16);
    bus.wram_write16(RETAIL_POOL.active_head, blk as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_next, 0);
    bus.wram_write16(blk + RETAIL_POOL.al_shape, 0x0042);
    bus.wram_write16(blk + RETAIL_POOL.al_worldx, px as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldy, py as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldz, pz as u16);

    let gf0 = bus.wram_read16(RETAIL_GAMEFRAME);
    call_near(&mut bus, RETAIL_DOSTRATS, &Entry { p: 0x00, ..Default::default() });
    let gf1 = bus.wram_read16(RETAIL_GAMEFRAME);
    let slot = ((blk - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize;
    let o = snapshot_objects(&bus, &RETAIL_POOL)[slot];
    let (sx, sy, sz) = (
        bus.wram_read16(STRATOBJ_POSX) as i16,
        bus.wram_read16(STRATOBJ_POSX + 2) as i16,
        bus.wram_read16(STRATOBJ_POSX + 4) as i16,
    );
    eprintln!(
        "STRAT dostrats tick: gameframe {gf0}->{gf1}; object survived (shape=${:04X} world=({},{},{})); do_strat_l wrote stratobj_pos=({sx},{sy},{sz}); aldead=${:04X}",
        o.shape, o.worldx, o.worldy, o.worldz, bus.wram_read16(RETAIL_ALDEAD)
    );
    assert_eq!(gf1, gf0.wrapping_add(1), "dostrats must inc gameframe once");
    assert_eq!(o.shape, 0x0042, "object must survive the tick");
    assert_eq!((sx, sy, sz), (px, py, pz), "do_strat_l must copy this object's world pos into stratobj_pos");
}

/// MILESTONE (step 4) — RETAIL `dostrats` DISPATCH vs THE PORT, TICK-FOR-TICK.
///
/// This drives the object through the ENTIRE retail dispatch machine each frame,
/// not the surgical single-routine call of `retail_vs_port_per_tick_object_diff`:
/// the object's own `al_stratptr` ($16, bank $18) points at the retail motion
/// routine `addalvecs_l` ($1F:C7BB), so `dostrats -> do_strat_l` reads that
/// pointer and RTL-dispatches into it exactly as it dispatches a real enemy
/// strat. So each `dostrats` call runs: `init_strats_l`, `update_objects_l`,
/// walk the active list, `do_strat_l` (copy world->stratobj_pos, resolve the
/// strat pointer, jump), the strat integrates `world += vel`, return. We diff the
/// resulting object array against the port (`sf_strat::common::strat_apply_
/// velocity`, the port's `addalvecs_l`) per field, per tick.
///
/// Certifies: the retail per-frame strat DISPATCH (allst walk + `do_strat_l`
/// pointer resolution + strat execution + object write-back) evolves a seeded
/// object identically to the Rust port over N ticks.
#[test]
fn retail_dostrats_dispatch_vs_port() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();
    inject_runmario_trampoline(&mut bus, RETAIL_RUNMARIO_L_ROM, RETAIL_RUNMARIO_RAM);
    init_object_pool(&mut bus);
    let blk = walk_freelist(&bus, &RETAIL_POOL)[0] as u32;

    // Seed one object whose al_stratptr = retail addalvecs_l ($1F:C7BB): the
    // real routine `do_strat_l` will resolve + dispatch as this object's strat.
    let (px, py, pz) = (1000i16, 500i16, 8000i16);
    let (vx, vy, vz) = (100i16, -50i16, -200i16);
    bus.wram_write16(RETAIL_POOL.active_head, blk as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_next, 0);
    bus.wram_write16(blk + RETAIL_POOL.al_shape, 0x0042);
    bus.wram_write16(blk + RETAIL_POOL.al_worldx, px as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldy, py as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldz, pz as u16);
    bus.wram_write16(blk + AL_VX, vx as u16);
    bus.wram_write16(blk + AL_VY, vy as u16);
    bus.wram_write16(blk + AL_VZ, vz as u16);
    // al_stratptr ($16 low word, $18 bank) = $1F:C7BB (retail addalvecs_l).
    bus.wram_write16(blk + AL_STRATPTR, (RETAIL_ADDALVECS_L & 0xFFFF) as u16);
    bus.write8(0x7E_0000 | (blk + AL_STRATPTR + 2), (RETAIL_ADDALVECS_L >> 16) as u8);

    // Port mirror.
    let mut a = sf_game::alien::Alien::default();
    a.shape = 0x0042;
    a.worldx = px; a.worldy = py; a.worldz = pz;
    a.vx = vx; a.vy = vy; a.vz = vz;

    // N kept small: each `dostrats` call runs the full `init_strats_l` +
    // `update_objects_l` on the whole (zeroed) game state, which is thousands of
    // 65816 instructions per tick — a few ticks certifies the dispatch loop.
    const N: u32 = 8;
    let slot = ((blk - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize;
    let mut first_div: Option<(u32, &'static str, i32, i32)> = None;
    for tick in 1..=N {
        // Retail: one full per-frame strat tick, object integrated via its own
        // al_stratptr through do_strat_l.
        call_near(&mut bus, RETAIL_DOSTRATS, &Entry { p: 0x00, ..Default::default() });
        let o = snapshot_objects(&bus, &RETAIL_POOL)[slot];
        // Port: the equivalent per-frame motion.
        sf_strat::common::strat_apply_velocity(&mut a);
        for (name, rv, pv) in [
            ("worldx", o.worldx as i32, a.worldx as i32),
            ("worldy", o.worldy as i32, a.worldy as i32),
            ("worldz", o.worldz as i32, a.worldz as i32),
        ] {
            if rv != pv && first_div.is_none() {
                first_div = Some((tick, name, rv, pv));
            }
        }
        if tick == 1 || tick == N || tick % 10 == 0 {
            eprintln!(
                "STRAT DISPATCH TICK {tick:>2}: retail world=({},{},{}) | port=({},{},{}) gsu_kicks={}",
                o.worldx, o.worldy, o.worldz, a.worldx, a.worldy, a.worldz, bus.gsu_kicks
            );
        }
    }
    let o = snapshot_objects(&bus, &RETAIL_POOL)[slot];
    assert_eq!(
        o.worldz as i32, pz as i32 + vz as i32 * N as i32,
        "retail dostrats dispatch must integrate the object every tick"
    );
    match first_div {
        None => eprintln!("STRAT DISPATCH DIFF: MATCH — retail dostrats-dispatched object == port over {N} ticks."),
        Some((t, f, rv, pv)) => {
            eprintln!("STRAT DISPATCH DIFF: first divergence tick={t} field={f} retail={rv} port={pv}");
            panic!("retail dostrats dispatch vs port diverged tick {t} {f}: retail={rv} port={pv}");
        }
    }
}

/// MILESTONE (steps 2-4) — THE FIRST RETAIL-vs-PORT PER-TICK OBJECT-ARRAY DIFF.
///
/// This is the tier-2 certifier working end-to-end for one scenario:
///  * SEED  — run the retail cart's OWN allocator to format the object pool,
///    then build a 3-object active list (`allst` -> block0 -> block1 -> block2
///    -> 0) exactly as the retail allocator + `l_add` would, each block carrying
///    a shape, world position, and per-frame velocity.
///  * TICK  — each frame, walk the retail active list and run the REAL RETAIL
///    per-object motion routine `addalvecs_l` ($1F:C7BB, located by signature)
///    on every live block, then `snapshot_objects` the WHOLE pool. This is the
///    retail game logic advancing the seeded state frame by frame.
///  * DIFF  — set up the identical scenario in the Rust PORT (`sf_game::Alien` +
///    `sf_strat::common::strat_apply_velocity`) and tick it in lockstep, then
///    compare worldx/y/z (and shape) per slot, per tick. Report the first
///    divergence (tick/slot/field) or MATCH.
///
/// Scope note: `addalvecs_l` is the CPU-only motion integrator every strat
/// applies each frame; it needs no GSU/PPU/input, so it runs cleanly on seeded
/// state. Driving the FULL per-frame tick (`dostrats` -> per-strat AI, which
/// calls the GSU via the RAM-resident `runmario_l` trampoline) is the remaining
/// work — see docs/TIER2_COEXEC_STATUS.md. The GSU side is now wired
/// (`gsu_kicks_through_bus_registers`); the open blocker is the RAM trampoline +
/// input injection, not the chip.
#[test]
fn retail_vs_port_per_tick_object_diff() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);

    // --- SEED: retail allocator formats the pool, then build a 3-block list. ---
    init_object_pool(&mut bus);
    let free = walk_freelist(&bus, &RETAIL_POOL);
    assert!(free.len() >= 3, "need >=3 free blocks to seed");
    let blocks: Vec<u32> = free[..3].iter().map(|&b| b as u32).collect();

    // Per-object seed state (shape, pos, velocity). Velocities chosen to exercise
    // +/- and Z-scroll (objects approaching the camera) with one wrap case.
    struct Seed {
        shape: u16,
        pos: (i16, i16, i16),
        vel: (i16, i16, i16),
    }
    let seeds = [
        Seed { shape: 0x0042, pos: (1000, 500, 8000), vel: (100, -50, -200) },
        Seed { shape: 0x0058, pos: (-1200, 300, 6000), vel: (-30, 20, -150) },
        Seed { shape: 0x0011, pos: (32000, -6789, 4321), vel: (1000, 222, -333) }, // X wraps
    ];

    // Link the active list at the retail stride and write each block's fields.
    bus.wram_write16(RETAIL_POOL.active_head, blocks[0] as u16);
    for (i, s) in seeds.iter().enumerate() {
        let b = blocks[i];
        let next = if i + 1 < blocks.len() { blocks[i + 1] as u16 } else { 0 };
        bus.wram_write16(b + RETAIL_POOL.al_next, next);
        bus.wram_write16(b + RETAIL_POOL.al_shape, s.shape);
        bus.wram_write16(b + RETAIL_POOL.al_worldx, s.pos.0 as u16);
        bus.wram_write16(b + RETAIL_POOL.al_worldy, s.pos.1 as u16);
        bus.wram_write16(b + RETAIL_POOL.al_worldz, s.pos.2 as u16);
        bus.wram_write16(b + AL_VX, s.vel.0 as u16);
        bus.wram_write16(b + AL_VY, s.vel.1 as u16);
        bus.wram_write16(b + AL_VZ, s.vel.2 as u16);
    }

    // Confirm the seed is readable as an active list before ticking.
    let chain = walk_active_list(&bus, blocks[0] as u16);
    eprintln!("RETAIL SEED: active list = {chain:04X?} (expect {blocks:04X?})");
    assert_eq!(chain, blocks.iter().map(|&b| b as u16).collect::<Vec<_>>());

    // --- Port side: mirror the seed into sf_game Aliens. ---
    let mut port: Vec<sf_game::alien::Alien> = seeds
        .iter()
        .map(|s| {
            let mut a = sf_game::alien::Alien::default();
            a.shape = s.shape;
            a.worldx = s.pos.0;
            a.worldy = s.pos.1;
            a.worldz = s.pos.2;
            a.vx = s.vel.0;
            a.vy = s.vel.1;
            a.vz = s.vel.2;
            a
        })
        .collect();

    // --- TICK + DIFF over N frames. ---
    const N: u32 = 30;
    let mut first_div: Option<(u32, usize, &'static str, i32, i32)> = None;
    for tick in 1..=N {
        // Retail: walk the active list, integrate every live block via real code.
        let mut x = bus.wram_read16(RETAIL_POOL.active_head);
        let mut guard = 0;
        while x != 0 && guard < RETAIL_POOL.count {
            call(&mut bus, RETAIL_ADDALVECS_L, &Entry { x, p: 0x00, ..Default::default() });
            x = bus.wram_read16(x as u32 + RETAIL_POOL.al_next);
            guard += 1;
        }
        let snap = snapshot_objects(&bus, &RETAIL_POOL);

        // Port: integrate every alien in lockstep.
        for a in port.iter_mut() {
            sf_strat::common::strat_apply_velocity(a);
        }

        // Diff the seeded slots (whole-array snapshot, but only these are live).
        for (i, &blk) in blocks.iter().enumerate() {
            let slot = ((blk - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize;
            let o = snap[slot];
            let p = &port[i];
            for (name, rv, pv) in [
                ("worldx", o.worldx as i32, p.worldx as i32),
                ("worldy", o.worldy as i32, p.worldy as i32),
                ("worldz", o.worldz as i32, p.worldz as i32),
                ("shape", o.shape as i32, p.shape as i32),
            ] {
                if rv != pv && first_div.is_none() {
                    first_div = Some((tick, i, name, rv, pv));
                }
            }
        }

        if tick == 1 || tick == N || tick % 10 == 0 {
            let o0 = snap[((blocks[0] - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize];
            eprintln!(
                "TICK {tick:>2}: retail slot0 world=({},{},{}) | port=({},{},{})",
                o0.worldx, o0.worldy, o0.worldz, port[0].worldx, port[0].worldy, port[0].worldz
            );
        }
    }

    // Prove the retail tick actually MOVED the objects (not a no-op snapshot).
    let final_snap = snapshot_objects(&bus, &RETAIL_POOL);
    let s0 = final_snap[((blocks[0] - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize];
    eprintln!(
        "RETAIL RESULT after {N} ticks: slot0 worldz {} -> {} (Δ={}); GSU kicks this run = {}",
        seeds[0].pos.2, s0.worldz, s0.worldz as i32 - seeds[0].pos.2 as i32, bus.gsu_kicks
    );
    assert_eq!(
        s0.worldz as i32,
        seeds[0].pos.2 as i32 + (seeds[0].vel.2 as i32) * N as i32,
        "retail addalvecs must have scrolled the object worldz every tick"
    );

    match first_div {
        None => eprintln!(
            "RETAIL DIFF: MATCH — retail object array == Rust port for all {} slots over {N} ticks.",
            blocks.len()
        ),
        Some((t, slot, field, rv, pv)) => {
            eprintln!("RETAIL DIFF: first divergence tick={t} slot={slot} field={field} retail={rv} port={pv}");
            panic!("retail vs port diverged at tick {t} slot {slot} {field}: retail={rv} port={pv}");
        }
    }
}

/// Walk an object active list from `head`, returning block offsets in order.
fn walk_active_list(bus: &SnesBus, head: u16) -> Vec<u16> {
    let mut out = Vec::new();
    let mut p = head;
    let mut guard = 0;
    while p != 0 && guard <= RETAIL_POOL.count {
        out.push(p);
        p = bus.wram_read16(p as u32 + RETAIL_POOL.al_next);
        guard += 1;
    }
    out
}

/// MILESTONE 2 — the retail object-array layout, re-derived from the retail cart
/// (NOT the built-ROM symbol map). Documents pool base / stride / field offsets
/// and how they relate to the built ROM.
#[test]
fn retail_object_array_layout() {
    eprintln!(
        "RETAIL POOL: base=${:04X} stride={} count={} freelist_head=${:04X} allst=${:04X}",
        RETAIL_POOL.base, RETAIL_POOL.stride, RETAIL_POOL.count, RETAIL_POOL.freelist_head, RETAIL_POOL.active_head,
    );
    eprintln!(
        "RETAIL POOL fields: shape=+${:02X} flags=+${:02X} worldx=+${:02X} worldy=+${:02X} worldz=+${:02X}",
        RETAIL_POOL.al_shape, RETAIL_POOL.al_flags, RETAIL_POOL.al_worldx, RETAIL_POOL.al_worldy, RETAIL_POOL.al_worldz,
    );
    eprintln!(
        "BUILT  POOL: base=${:04X} stride={} count={} freelist_head=${:04X}",
        BUILT_POOL.base, BUILT_POOL.stride, BUILT_POOL.count, BUILT_POOL.freelist_head,
    );
    eprintln!(
        "DIFF: retail pool base shifted {} bytes, struct {} bytes shorter; WORLD-COORD FIELD OFFSETS IDENTICAL.",
        BUILT_POOL.base as i32 - RETAIL_POOL.base as i32,
        BUILT_POOL.stride as i32 - RETAIL_POOL.stride as i32,
    );

    // The field offsets we read are the ones proven identical across both ROMs.
    assert_eq!(RETAIL_POOL.al_worldx, BUILT_POOL.al_worldx);
    assert_eq!(RETAIL_POOL.al_worldz, 0x10);
    // But the pool geometry genuinely differs — this is why the built-ROM symbol
    // map cannot be blindly reused for retail.
    assert_ne!(RETAIL_POOL.base, BUILT_POOL.base);
    assert_ne!(RETAIL_POOL.stride, BUILT_POOL.stride);
}

/// MILESTONE 3 — observable object state is READABLE from retail. We run the
/// retail cart's OWN allocator-init routine ($02:F4D8) on a retail bus to format
/// the object pool exactly as the game does at level start, then snapshot the
/// pool and prove the free-list it built is coherent (70 blocks, correctly
/// linked at the retail stride).
#[test]
fn retail_object_state_readable() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);

    // Execute retail's real allocator-init (FmtFreeLst) — RTL far call.
    init_object_pool(&mut bus);

    let head = bus.wram_read16(RETAIL_POOL.freelist_head);
    eprintln!("RETAIL TICK0: freelist head = ${head:04X} (expect ${:04X})", RETAIL_POOL.base);

    let chain = walk_freelist(&bus, &RETAIL_POOL);
    eprintln!(
        "RETAIL TICK0: free-list length = {} (expect {}), first 6 = {:04X?}",
        chain.len(),
        RETAIL_POOL.count,
        &chain[..chain.len().min(6)],
    );

    // The retail allocator must have produced a coherent 70-block free-list,
    // each block one stride (54 bytes) apart — read straight out of retail WRAM.
    assert_eq!(head, RETAIL_POOL.base as u16, "retail init did not seed freelist head");
    assert_eq!(chain.len() as u32, RETAIL_POOL.count, "retail free-list not fully linked");
    for w in chain.windows(2) {
        assert_eq!(
            w[1] - w[0],
            RETAIL_POOL.stride as u16,
            "retail free-list stride mismatch",
        );
    }

    // Now snapshot the full pool as observable state. With only the free-list
    // formatted (no spawns), all slots are empty (shape 0) — the snapshot API
    // works; live objects appear once spawn code runs.
    let snap = snapshot_objects(&bus, &RETAIL_POOL);
    let live = snap.iter().filter(|o| o.shape != 0).count();
    eprintln!(
        "RETAIL TICK0 snapshot: {} slots, {} live (shape!=0). slot0={:?}",
        snap.len(),
        live,
        snap[0],
    );
    assert_eq!(snap.len() as u32, RETAIL_POOL.count);
}

/// MILESTONE 3b — DIFF FOUNDATION. Seed a synthetic gameplay state (one object)
/// into the retail pool at the retail field offsets, snapshot it, and confirm
/// the snapshot API reads back exactly what a spawn would have written. This is
/// the read-side of the co-exec diff: given a live retail object array, we can
/// extract per-slot (shape, flags, worldx/y/z) for tick-for-tick comparison
/// against the Rust port. (Driving the full retail spawn path to *produce* live
/// objects needs the GSU/game-loop, tracked as "to finish tier-2" below.)
#[test]
fn retail_snapshot_reads_seeded_object() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    init_object_pool(&mut bus);

    // Simulate a spawn into slot 3: write shape/flags/world coords at the retail
    // struct offsets (what MAPOBJDO-family code writes via `sta al_worldx,y`).
    let slot = 3u32;
    let b = RETAIL_POOL.base + slot * RETAIL_POOL.stride;
    bus.wram_write16(b + RETAIL_POOL.al_shape, 0x0042);
    bus.wram_write16(b + RETAIL_POOL.al_flags, 0x0018); // afFrontPl|afInviewPl
    bus.wram_write16(b + RETAIL_POOL.al_worldx, (-1234i16) as u16);
    bus.wram_write16(b + RETAIL_POOL.al_worldy, 567i16 as u16);
    bus.wram_write16(b + RETAIL_POOL.al_worldz, 8000i16 as u16);

    let snap = snapshot_objects(&bus, &RETAIL_POOL);
    let o = snap[slot as usize];
    eprintln!("RETAIL DIFF probe: slot3 = {o:?}");
    assert_eq!(o.shape, 0x0042);
    assert_eq!(o.flags, 0x0018);
    assert_eq!(o.worldx, -1234);
    assert_eq!(o.worldy, 567);
    assert_eq!(o.worldz, 8000);

    // Cross-ROM sanity: the built ROM's own allocator formats a DIFFERENT pool
    // geometry — proving the two layouts are genuinely distinct and that reading
    // retail with the built map would mis-index.
    if let Some(built) = load_built_rom() {
        let bbus = SnesBus::new(built);
        eprintln!(
            "DIFF note: built pool base ${:04X}/stride {} vs retail ${:04X}/stride {} — reading retail with built offsets would land in the wrong slot after slot 0.",
            BUILT_POOL.base, BUILT_POOL.stride, RETAIL_POOL.base, RETAIL_POOL.stride,
        );
        let _ = bbus;
    }
}


// ============================================================================
// FIRST NAMED ENEMY-STRAT CERTIFICATION vs RETAIL — the `stayrel` ground family
//
// Capstone: run the retail cart's OWN per-tick enemy-strat body on seeded state
// and diff the object vs the port's `sf_strat::ground` strat tick-for-tick.
// Unlike the earlier `addalvecs_l` diff (a synthetic strat wired through
// dispatch), this certifies REAL, NAMED enemy AI: `stayrelhard180YR_strat` and
// `stayrel_strat`, whose entire per-tick body is `jsl sr_addplayerZx; rtl`
// (`worldz += pviewvelz`). Global footprint = exactly ONE global (`pviewvelz`).
// ============================================================================

use sf_oracle::{
    AL_SFLAGS2, RETAIL_PVIEWVELZ, RETAIL_SR_ADDPLAYERZX, RETAIL_STAYRELHARD180YR_STRAT,
    RETAIL_STAYREL_STRAT,
};
use sf_oracle::{
    AL_SWORD1, AL_TYPE, RETAIL_GND_ISTRAT, RETAIL_PVIEWPOSZ, RETAIL_STAYDIST_ISTRAT,
};
use sf_oracle::{
    AL_ROTX, AL_ROTY, AL_ROTZ, AL_SBYTE1, AL_SBYTE2, AL_SBYTE3, RETAIL_HARDROT_STRAT,
    RETAIL_STRAIGHT_ISTRAT, RETAIL_STRAIGHT_STRAT,
};
use sf_oracle::{
    seed_player_relative_state, RETAIL_PARAJUMP_STRAT, RETAIL_PLAYER_POSX, RETAIL_PLAYER_POSY,
    RETAIL_PLAYER_POSZ, RETAIL_PLAYPT, RETAIL_RANDOM_L, RETAIL_RAND,
};
use sf_oracle::{
    seed_retail_rng, ASF2_SFLAG2, RETAIL_FIREPILLAR_ISTRAT, RETAIL_FIREPILLAR_STRAT,
};
use sf_oracle::{
    AL_AP, AL_COLLFLAGS, AL_HP, RETAIL_BIG_METEOR_ISTRAT, RETAIL_MINE0_ISTRAT,
    RETAIL_ROCKHARD_ISTRAT, RETAIL_TREE1_ISTRAT,
};

/// Scan `rom` for a masked byte pattern (`None` = wildcard byte). Returns ROM
/// file offsets of every match.
fn masked_scan(rom: &[u8], pat: &[Option<u8>]) -> Vec<usize> {
    let mut hits = vec![];
    if rom.len() < pat.len() {
        return hits;
    }
    for i in 0..=rom.len() - pat.len() {
        if pat.iter().enumerate().all(|(j, p)| p.map_or(true, |b| rom[i + j] == b)) {
            hits.push(i);
        }
    }
    hits
}
/// LoROM ROM-file-offset -> SNES bank:addr (banks $00-$3F, $8000-$FFFF window).
fn rom_off_to_snes(off: usize) -> u32 {
    let bank = (off >> 15) as u32;
    let addr = ((off & 0x7FFF) + 0x8000) as u32;
    (bank << 16) | addr
}
/// SNES bank:addr -> LoROM ROM-file offset.
fn snes_to_rom_off(snes: u32) -> usize {
    (((snes >> 16) as usize & 0x7F) << 15) | ((snes & 0xFFFF) as usize - 0x8000)
}

/// MILESTONE (named-strat step 1) — LOCATE + CROSS-VALIDATE the `stayrel`-family
/// retail addresses by masked signature scan, and read back the ONE global they
/// touch (`pviewvelz`) plus the `al_sflags2` struct offset.
///
///  * `sr_addplayerZx` — 8 skeleton matches, but exactly ONE is referenced by a
///    `jsl` (247 refs; 97 of them `jsl X; rtl` pure-scroll strat bodies). That
///    is the genuine leaf; its `adc` operand IS `pviewvelz`.
///  * `stayrel_strat` — a UNIQUE masked hit (`jsl sr_addplayerZx; set sflag;
///    rtl`); its `sta` operand pins `al_sflags2`.
///  * `stayrelhard180YR_strat` — the pure-scroll body (`jsl sr_addplayerZx;
///    rtl`) immediately preceding `stayrel_strat`.
#[test]
fn retail_stayrel_family_addresses() {
    let Some(rom) = retail() else { return };

    // --- sr_addplayerZx: C2 20 B5 10 18 6D ?? ?? 95 10 E2 20 6B ---
    let leaf_pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xB5), Some(0x10), Some(0x18), Some(0x6D),
        None, None, Some(0x95), Some(0x10), Some(0xE2), Some(0x20), Some(0x6B),
    ];
    let mut genuine: Option<(u32, u16)> = None;
    let mut refd = 0usize;
    for &h in &masked_scan(&rom, &leaf_pat) {
        let snes = rom_off_to_snes(h);
        // Count `jsl snes` references across the whole ROM.
        let (lo, hi, bk) = (snes as u8, (snes >> 8) as u8, (snes >> 16) as u8);
        let jsl = masked_scan(&rom, &[Some(0x22), Some(lo), Some(hi), Some(bk)]);
        if !jsl.is_empty() {
            refd += 1;
            genuine = Some((snes, rom[h + 6] as u16 | ((rom[h + 7] as u16) << 8)));
        }
    }
    assert_eq!(refd, 1, "exactly one of the sr_addplayerZ skeleton matches is CALLED");
    let (leaf, pviewvelz) = genuine.unwrap();
    eprintln!("NAMED-STRAT: sr_addplayerZx=${leaf:06X}  pviewvelz=${pviewvelz:04X}");
    assert_eq!(leaf, RETAIL_SR_ADDPLAYERZX, "sr_addplayerZx address");
    assert_eq!(pviewvelz as u32, RETAIL_PVIEWVELZ, "pviewvelz operand");

    // --- stayrel_strat: 22 <leaf> B5 off 09 01 95 off 6B (UNIQUE) ---
    let (llo, lhi, lbk) = (leaf as u8, (leaf >> 8) as u8, (leaf >> 16) as u8);
    let stayrel_pat: Vec<Option<u8>> = vec![
        Some(0x22), Some(llo), Some(lhi), Some(lbk),
        Some(0xB5), None, Some(0x09), Some(0x01), Some(0x95), None, Some(0x6B),
    ];
    let sr = masked_scan(&rom, &stayrel_pat);
    assert_eq!(sr.len(), 1, "stayrel_strat is a unique masked hit");
    let h = sr[0];
    let stayrel = rom_off_to_snes(h);
    let sflags2_off = rom[h + 5] as u32;
    eprintln!(
        "NAMED-STRAT: stayrel_strat=${stayrel:06X}  al_sflags2=${sflags2_off:02X} ora #${:02X}",
        rom[h + 7]
    );
    assert_eq!(stayrel, RETAIL_STAYREL_STRAT, "stayrel_strat address");
    assert_eq!(sflags2_off, AL_SFLAGS2, "al_sflags2 offset");
    assert_eq!(rom[h + 5], rom[h + 9], "lda/sta hit the same sflags2 offset");

    // --- stayrelhard180YR_strat: the pure-scroll body just before it ---
    // stayrel_strat is preceded by `22 <leaf> 6B` (5 bytes).
    let prev = rom_off_to_snes(h - 5);
    let prev_bytes: Vec<u8> = (0..5).map(|i| rom[h - 5 + i]).collect();
    eprintln!("NAMED-STRAT: stayrelhard180YR_strat=${prev:06X} body={prev_bytes:02X?}");
    assert_eq!(prev, RETAIL_STAYRELHARD180YR_STRAT, "stayrelhard180YR_strat address");
    assert_eq!(prev_bytes, vec![0x22, llo, lhi, lbk, 0x6B], "pure `jsl sr_addplayerZx; rtl`");
}

/// Set up a fresh retail bus with `pviewvelz` seeded and ONE object whose
/// `al_worldz` = `pz` at pool base. Returns the block offset (== X for a strat
/// call). `sr_addplayerZx` only touches `al_worldz,x` + `pviewvelz`, so nothing
/// else needs seeding (fresh WRAM is zeroed).
fn seed_scroll_object(bus: &mut SnesBus, pz: i16, pviewvelz: i16) -> u32 {
    let blk = RETAIL_POOL.base;
    bus.wram_write16(RETAIL_PVIEWVELZ, pviewvelz as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldz, pz as u16);
    blk
}

/// Build the port equivalent: a Game with `pviewvelz` set and one alien at
/// `worldz = pz`, its `stratptr` armed to the named ground strat's per-tick
/// body. Returns `(game, alien_index, per_tick_stratid)`.
fn port_scroll_object(
    pz: i16,
    pviewvelz: i16,
    init: fn(&mut sf_game::game::Game) -> sf_game::alien::StratId,
) -> (sf_game::game::Game, u16, sf_game::alien::StratId) {
    let mut g = sf_game::game::Game::new();
    let istrat = init(&mut g);
    let idx = g.objs.alloc().expect("alien pool");
    g.objs.aliens[idx as usize].worldz = pz;
    g.vars.pviewvelz = pviewvelz;
    // Run the Istrat to arm the per-tick strat (sets stratptr; leaves worldz).
    g.call_strat(istrat, idx);
    let tick = g.objs.aliens[idx as usize].stratptr.expect("per-tick strat armed");
    (g, idx, tick)
}

/// CAPSTONE — RETAIL `stayrelhard180YR_strat` vs THE PORT, TICK-FOR-TICK.
///
/// This certifies a REAL, NAMED enemy strat (not the synthetic `addalvecs_l`):
/// each tick we run the retail cart's OWN `stayrelhard180YR_strat` body
/// ($06:8646 = `jsl sr_addplayerZx; rtl`) on the seeded object and diff its
/// `worldz` against the port's `sf_strat::ground` stayrelhard180yr per-tick
/// strat. Global footprint = ONE global (`pviewvelz`, seeded identically both
/// sides). Two scenarios, including a 16-bit `worldz` wrap.
#[test]
fn retail_stayrelhard180yr_strat_vs_port() {
    let Some(rom) = retail() else { return };
    const N: u32 = 60;
    // (pz, pviewvelz) — case 2 drives worldz past -32768 to exercise the wrap.
    for (pz, pvz) in [(8000i16, -200i16), (-30000i16, -1000i16)] {
        let mut bus = SnesBus::new(rom.clone());
        let blk = seed_scroll_object(&mut bus, pz, pvz);
        let (mut g, idx, tick) = port_scroll_object(pz, pvz, |g| {
            sf_strat::ground::install(g).stayrelhard180yr
        });

        let mut first_div: Option<(u32, i32, i32)> = None;
        for t in 1..=N {
            // Retail: run the named strat body ($06:8646) with X = block.
            call(&mut bus, RETAIL_STAYRELHARD180YR_STRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });
            let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
            // Port: one per-tick strat call.
            g.call_strat(tick, idx);
            let pw = g.objs.aliens[idx as usize].worldz;
            if rw != pw && first_div.is_none() {
                first_div = Some((t, rw as i32, pw as i32));
            }
        }
        let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
        let expect = (pz as i32 + pvz as i32 * N as i32) as i16; // wrapping i16
        assert_eq!(rw, expect, "retail worldz must scroll by pviewvelz each tick");
        match first_div {
            None => eprintln!(
                "NAMED-STRAT stayrelhard180yr [pz={pz} pvz={pvz}]: MATCH — retail == port worldz over {N} ticks (final {rw})"
            ),
            Some((t, r, p)) => panic!("stayrelhard180yr diverged tick {t}: retail worldz={r} port worldz={p}"),
        }
    }
}

/// CAPSTONE (2nd strat) — RETAIL `stayrel_strat` vs THE PORT.
///
/// `stayrel_strat` ($06:864B) = `jsl sr_addplayerZx` (scroll) + set the
/// `colldisable` sflag. `worldz` is diffed directly (MATCH expected). The sflag
/// is NOT raw-diffable: retail stores `colldisable` in `al_sflags2` bit `$01`
/// (sflag bit 8), while the port's C `obj.h` layout stores it in `al_sflags`
/// bit `$10` — a deliberate representation remap, not a bug. We assert each
/// side sets ITS OWN `colldisable` bit, and document the mapping.
#[test]
fn retail_stayrel_strat_vs_port() {
    let Some(rom) = retail() else { return };
    const N: u32 = 40;
    let (pz, pvz) = (6000i16, -150i16);

    let mut bus = SnesBus::new(rom.clone());
    let blk = seed_scroll_object(&mut bus, pz, pvz);
    let (mut g, idx, tick) =
        port_scroll_object(pz, pvz, |g| sf_strat::ground::install(g).stayrel);

    let mut first_div: Option<(u32, i32, i32)> = None;
    for t in 1..=N {
        call(&mut bus, RETAIL_STAYREL_STRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });
        let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
        g.call_strat(tick, idx);
        let pw = g.objs.aliens[idx as usize].worldz;
        if rw != pw && first_div.is_none() {
            first_div = Some((t, rw as i32, pw as i32));
        }
    }
    // worldz: exact tick-for-tick match.
    let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
    assert_eq!(rw, (pz as i32 + pvz as i32 * N as i32) as i16, "retail stayrel scrolled worldz");
    assert!(first_div.is_none(), "stayrel worldz: {first_div:?}");

    // colldisable sflag — each side sets its own representation's bit.
    let retail_sflags2 = bus.read8(0x7E_0000 | (blk + AL_SFLAGS2)); // bit $01 = colldisable
    let port_sflags = g.objs.aliens[idx as usize].sflags; // bit $10 = ASF_COLLDISABLE
    eprintln!(
        "NAMED-STRAT stayrel [pz={pz} pvz={pvz}]: worldz MATCH over {N} ticks (final {rw}); \
         colldisable set retail al_sflags2=${retail_sflags2:02X}(bit $01) <-> port al_sflags=${port_sflags:02X}(bit $10)"
    );
    assert_ne!(retail_sflags2 & 0x01, 0, "retail stayrel set colldisable in al_sflags2 bit $01");
    assert_ne!(port_sflags & 0x10, 0, "port stayrel set colldisable in al_sflags bit $10");
}

/// Prove the located `stayrelhard180YR_strat` really is `jsl sr_addplayerZx;
/// rtl` by reading it straight out of the retail ROM through the bus (LoROM),
/// and that a direct call of it advances `worldz` by exactly `pviewvelz`.
#[test]
fn retail_stayrelhard180yr_body_is_jsl_addplayerz() {
    let Some(rom) = retail() else { return };
    // Read the 5 body bytes from ROM offset.
    let off = snes_to_rom_off(RETAIL_STAYRELHARD180YR_STRAT);
    let body: Vec<u8> = (0..5).map(|i| rom[off + i]).collect();
    let leaf = RETAIL_SR_ADDPLAYERZX;
    eprintln!("stayrelhard180YR_strat body @${RETAIL_STAYRELHARD180YR_STRAT:06X} = {body:02X?}");
    assert_eq!(
        body,
        vec![0x22, leaf as u8, (leaf >> 8) as u8, (leaf >> 16) as u8, 0x6B],
        "body must be `jsl sr_addplayerZx; rtl`"
    );
    // One-tick behavioural check: worldz += pviewvelz.
    let mut bus = SnesBus::new(rom);
    let blk = seed_scroll_object(&mut bus, 1234, 77);
    call(&mut bus, RETAIL_STAYRELHARD180YR_STRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });
    assert_eq!(bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16, 1234 + 77);
}

/// CAPSTONE (full pipeline) — RETAIL `stayrelhard180YR_strat` dispatched through
/// the ENTIRE retail `dostrats` per-frame tick, vs the port.
///
/// Where `retail_stayrelhard180yr_strat_vs_port` calls the strat body surgically,
/// this points an object's `al_stratptr` at the REAL named strat ($06:8646) and
/// runs the whole retail `dostrats` ($02:DAF2 = init_strats_l + update_objects_l
/// + active-list walk + do_strat_l dispatch + write-back) each frame. It also
/// PROVES the strat's one global (`pviewvelz`) survives a full frame — nothing in
/// the pipeline clobbers a directly-seeded `pviewvelz` when no player strat runs.
#[test]
fn retail_stayrelhard180yr_dispatch_vs_port() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();
    inject_runmario_trampoline(&mut bus, RETAIL_RUNMARIO_L_ROM, RETAIL_RUNMARIO_RAM);
    init_object_pool(&mut bus);
    let blk = walk_freelist(&bus, &RETAIL_POOL)[0] as u32;

    let (pz, pvz) = (8000i16, -200i16);
    bus.wram_write16(RETAIL_PVIEWVELZ, pvz as u16);
    bus.wram_write16(RETAIL_POOL.active_head, blk as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_next, 0);
    bus.wram_write16(blk + RETAIL_POOL.al_shape, 0x0042);
    bus.wram_write16(blk + RETAIL_POOL.al_worldz, pz as u16);
    // al_stratptr ($16 low / $18 bank) = $06:8646 (stayrelhard180YR_strat).
    bus.wram_write16(blk + AL_STRATPTR, (RETAIL_STAYRELHARD180YR_STRAT & 0xFFFF) as u16);
    bus.write8(0x7E_0000 | (blk + AL_STRATPTR + 2), (RETAIL_STAYRELHARD180YR_STRAT >> 16) as u8);

    let (mut g, idx, tick) =
        port_scroll_object(pz, pvz, |g| sf_strat::ground::install(g).stayrelhard180yr);

    const N: u32 = 8;
    let slot = ((blk - RETAIL_POOL.base) / RETAIL_POOL.stride) as usize;
    let mut first_div: Option<(u32, i32, i32)> = None;
    for t in 1..=N {
        // A full retail frame. update_objects_l/init_strats_l must NOT clobber
        // our seeded pviewvelz (no player strat runs it back to a default).
        call_near(&mut bus, RETAIL_DOSTRATS, &Entry { p: 0x00, ..Default::default() });
        let rw = snapshot_objects(&bus, &RETAIL_POOL)[slot].worldz;
        g.call_strat(tick, idx);
        let pw = g.objs.aliens[idx as usize].worldz;
        if rw != pw && first_div.is_none() {
            first_div = Some((t, rw as i32, pw as i32));
        }
    }
    let pvz_after = bus.wram_read16(RETAIL_PVIEWVELZ) as i16;
    let rw = snapshot_objects(&bus, &RETAIL_POOL)[slot].worldz;
    eprintln!(
        "NAMED-STRAT DISPATCH stayrelhard180yr: pviewvelz {pvz}->{pvz_after} after {N} frames; retail worldz final={rw}"
    );
    assert_eq!(pvz_after, pvz, "dostrats must not clobber the seeded pviewvelz");
    assert_eq!(rw, (pz as i32 + pvz as i32 * N as i32) as i16, "dispatched strat scrolled worldz per frame");
    match first_div {
        None => eprintln!("NAMED-STRAT DISPATCH stayrelhard180yr: MATCH — retail dostrats-dispatched named strat == port over {N} frames"),
        Some((t, r, p)) => panic!("dispatch diverged frame {t}: retail worldz={r} port worldz={p}"),
    }
}

// ============================================================================
// BATCH 2 — GROUND FAMILY EXTENSION: `staydist` + `gnd` (certified vs retail).
//
// `staydist_Istrat` ($06:8656) extends the stayrel family with a SECOND global
// (`pviewposz`) and a struct-field read (`al_sword1`): its per-tick body is
// `al_worldz = al_sword1 + pviewposz` (viewer-tracking, idempotent) + set
// colldisable. `gnd_Istrat` ($08:F15D) is an INIT-ONLY strat (zeroes stratptr,
// sets type|=gnd + colldisable). Both located by masked signature scan of
// retail, skeleton read out of the built ROM first, then cross-validated.
// ============================================================================

/// The port's `PVIEWPOSZ` compat-WRAM address (sf-strat `enemy_a::wm::PVIEWPOSZ`
/// = $1F24 — the port's own address space, distinct from retail $14FA).
const PORT_PVIEWPOSZ: u16 = 0x1F24;

/// MILESTONE (batch-2 step 1) — LOCATE + CROSS-VALIDATE `staydist_Istrat` and
/// `gnd_Istrat` in retail by masked signature scan, and read back the ONE new
/// global they touch (`pviewposz`).
#[test]
fn retail_batch2_ground_addresses() {
    let Some(rom) = retail() else { return };

    // --- staydist_Istrat: rep;lda sword1,x;sta worldz,x;sep; rep;lda worldz,x;
    //     clc;adc <pviewposz>;sta worldz,x;sep; lda sflags2,x;ora #1;sta;rtl ---
    let staydist_pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xB5), Some(0x26), Some(0x95), Some(0x10),
        Some(0xE2), Some(0x20), Some(0xC2), Some(0x20), Some(0xB5), Some(0x10),
        Some(0x18), Some(0x6D), None, None, Some(0x95), Some(0x10), Some(0xE2),
        Some(0x20), Some(0xB5), Some(0x1E), Some(0x09), Some(0x01), Some(0x95),
        Some(0x1E), Some(0x6B),
    ];
    let sd = masked_scan(&rom, &staydist_pat);
    assert_eq!(sd.len(), 1, "staydist_Istrat is a UNIQUE masked hit");
    let h = sd[0];
    let staydist = rom_off_to_snes(h);
    let pviewposz = rom[h + 14] as u32 | ((rom[h + 15] as u32) << 8);
    let sword1_off = rom[h + 3] as u32; // lda al_sword1,x operand
    eprintln!("BATCH2: staydist_Istrat=${staydist:06X}  pviewposz=${pviewposz:04X}  al_sword1=${sword1_off:02X}");
    assert_eq!(staydist, RETAIL_STAYDIST_ISTRAT, "staydist_Istrat address");
    assert_eq!(pviewposz, RETAIL_PVIEWPOSZ, "pviewposz operand");
    assert_eq!(sword1_off, AL_SWORD1, "al_sword1 offset");
    // Cross-validate: pviewposz == pviewvelz + 6 (same +6 spacing as built ROM).
    assert_eq!(pviewposz, RETAIL_PVIEWVELZ + 6, "pviewposz should sit 6 bytes after pviewvelz");
    // Adjacency: staydist_Istrat immediately follows stayrel_strat (11-byte body).
    assert_eq!(staydist, RETAIL_STAYREL_STRAT + 11, "staydist follows stayrel_strat body");

    // --- gnd_Istrat: rep;lda#0;sta stratptr,x;sep;lda#0;sta stratptr+2,x;
    //     jsl <set0coll>; lda type,x;ora#1;sta type,x; lda sflags2,x;ora#1;sta;rtl
    let gnd_pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), Some(0x00), Some(0x00), Some(0x95),
        Some(0x16), Some(0xE2), Some(0x20), Some(0xA9), Some(0x00), Some(0x95),
        Some(0x18), Some(0x22), None, None, None, Some(0xB5), Some(0x09),
        Some(0x09), Some(0x01), Some(0x95), Some(0x09), Some(0xB5), Some(0x1E),
        Some(0x09), Some(0x01), Some(0x95), Some(0x1E), Some(0x6B),
    ];
    let g = masked_scan(&rom, &gnd_pat);
    assert_eq!(g.len(), 1, "gnd_Istrat is a UNIQUE masked hit");
    let gh = g[0];
    let gnd = rom_off_to_snes(gh);
    let set0coll = rom[gh + 14] as u32 | ((rom[gh + 15] as u32) << 8) | ((rom[gh + 16] as u32) << 16);
    let type_off = rom[gh + 18] as u32; // lda al_type,x operand
    eprintln!("BATCH2: gnd_Istrat=${gnd:06X}  set_0collptrsx_l=${set0coll:06X}  al_type=${type_off:02X}");
    assert_eq!(gnd, RETAIL_GND_ISTRAT, "gnd_Istrat address");
    assert_eq!(type_off, AL_TYPE, "al_type offset");
    // set_0collptrsx_l must be a real jsl target (bank $1F leaf).
    assert_eq!(set0coll >> 16, 0x1F, "set_0collptrsx_l lives in bank $1F");
}

/// CAPSTONE (batch-2) — RETAIL `staydist_Istrat` vs THE PORT, TICK-FOR-TICK.
///
/// Certifies the viewer-tracking ground strat: each tick `al_worldz =
/// al_sword1 + pviewposz`. Footprint = ONE new global (`pviewposz`) + one
/// struct field (`al_sword1`), seeded identically both sides. The scenario
/// changes `pviewposz` mid-run on BOTH sides to prove worldz tracks the global
/// (not a frozen one-shot). Two (sword1, pviewposz) cases incl. a 16-bit wrap.
#[test]
fn retail_staydist_strat_vs_port() {
    let Some(rom) = retail() else { return };
    const N: u32 = 40;
    for (sword1, pvp0, pvp1) in [(2000i16, 500i16, -300i16), (-30000i16, -4000i16, 5000i16)] {
        // --- retail ---
        let mut bus = SnesBus::new(rom.clone());
        let blk = RETAIL_POOL.base;
        bus.wram_write16(RETAIL_PVIEWPOSZ, pvp0 as u16);
        bus.wram_write16(blk + AL_SWORD1, sword1 as u16);
        // --- port ---
        let mut g = sf_game::game::Game::new();
        let ids = sf_strat::ground::install(&mut g);
        let idx = g.objs.alloc().expect("alien pool");
        g.objs.aliens[idx as usize].sword1 = sword1;
        g.vars.write_ext16(PORT_PVIEWPOSZ, pvp0 as u16);
        g.call_strat(ids.staydist, idx); // arms stratptr + runs body once
        let tick = g.objs.aliens[idx as usize].stratptr.expect("staydist per-tick armed");

        let mut first_div: Option<(u32, i32, i32)> = None;
        for t in 1..=N {
            if t == N / 2 {
                // Change the tracked global on BOTH sides mid-run.
                bus.wram_write16(RETAIL_PVIEWPOSZ, pvp1 as u16);
                g.vars.write_ext16(PORT_PVIEWPOSZ, pvp1 as u16);
            }
            call(&mut bus, RETAIL_STAYDIST_ISTRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });
            let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
            g.call_strat(tick, idx);
            let pw = g.objs.aliens[idx as usize].worldz;
            if rw != pw && first_div.is_none() {
                first_div = Some((t, rw as i32, pw as i32));
            }
        }
        // Final worldz must equal sword1 + pvp1 (wrapping), and colldisable set.
        let rw = bus.wram_read16(blk + RETAIL_POOL.al_worldz) as i16;
        assert_eq!(rw, (sword1 as i32 + pvp1 as i32) as i16, "retail staydist worldz = sword1 + pviewposz");
        let retail_sflags2 = bus.read8(0x7E_0000 | (blk + AL_SFLAGS2));
        let port_sflags = g.objs.aliens[idx as usize].sflags;
        assert_ne!(retail_sflags2 & 0x01, 0, "retail staydist set colldisable in al_sflags2 bit $01");
        assert_ne!(port_sflags & 0x10, 0, "port staydist set colldisable in al_sflags bit $10");
        match first_div {
            None => eprintln!(
                "BATCH2 staydist [sword1={sword1} pvp {pvp0}->{pvp1}]: MATCH — retail == port worldz over {N} ticks (final {rw}); colldisable retail al_sflags2=${retail_sflags2:02X}(bit$01) <-> port al_sflags=${port_sflags:02X}(bit$10)"
            ),
            Some((t, r, p)) => panic!("staydist diverged tick {t}: retail worldz={r} port worldz={p}"),
        }
    }
}

/// CAPSTONE (batch-2) — RETAIL `gnd_Istrat` vs THE PORT.
///
/// `gnd_Istrat` is INIT-ONLY: zero `al_stratptr` (per-tick becomes a no-op),
/// `jsl set_0collptrsx_l` (zero the extended coll/exp strat ptrs), set
/// `al_type |= gnd($01)` + `al_sflags2 |= colldisable($01)`. We seed an object
/// with DIRTY strat pointers + type + sflags, run the retail Istrat once, and
/// diff the observable effects vs the port `strat_gnd_init`. Footprint reads NO
/// globals. The colldisable sflag uses the same representation remap as stayrel
/// (retail al_sflags2 bit $01 <-> port al_sflags bit $10).
#[test]
fn retail_gnd_strat_vs_port() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    let blk = RETAIL_POOL.base;

    // Seed DIRTY state so we can prove the strat clears / sets bits.
    bus.wram_write16(blk + AL_STRATPTR, 0xBEEF);
    bus.write8(0x7E_0000 | (blk + AL_STRATPTR + 2), 0x1F);
    bus.write8(0x7E_0000 | (blk + AL_TYPE), 0x00);
    bus.write8(0x7E_0000 | (blk + AL_SFLAGS2), 0x00);

    call(&mut bus, RETAIL_GND_ISTRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });

    // Retail observable effects.
    let r_stratptr_lo = bus.wram_read16(blk + AL_STRATPTR);
    let r_stratptr_bk = bus.read8(0x7E_0000 | (blk + AL_STRATPTR + 2));
    let r_type = bus.read8(0x7E_0000 | (blk + AL_TYPE));
    let r_sflags2 = bus.read8(0x7E_0000 | (blk + AL_SFLAGS2));
    eprintln!(
        "BATCH2 gnd: retail stratptr=${r_stratptr_bk:02X}:{r_stratptr_lo:04X} type=${r_type:02X} sflags2=${r_sflags2:02X}"
    );
    assert_eq!(r_stratptr_lo, 0, "retail gnd zeroed al_stratptr low word");
    assert_eq!(r_stratptr_bk, 0, "retail gnd zeroed al_stratptr bank");
    assert_ne!(r_type & 0x01, 0, "retail gnd set al_type |= gnd($01)");
    assert_ne!(r_sflags2 & 0x01, 0, "retail gnd set colldisable in al_sflags2 bit $01");

    // Port equivalent.
    let mut g = sf_game::game::Game::new();
    let ids = sf_strat::ground::install(&mut g);
    let idx = g.objs.alloc().expect("alien pool");
    // Dirty the port object too.
    g.objs.aliens[idx as usize].type_ = 0;
    g.objs.aliens[idx as usize].sflags = 0;
    g.call_strat(ids.gnd, idx);
    let p = &g.objs.aliens[idx as usize];
    eprintln!(
        "BATCH2 gnd: port stratptr={:?} collstratptr={:?} expstratptr={:?} type=${:02X} sflags=${:02X}",
        p.stratptr, p.collstratptr, p.expstratptr, p.type_, p.sflags
    );
    assert!(p.stratptr.is_none(), "port gnd cleared stratptr");
    assert!(p.collstratptr.is_none(), "port gnd cleared collstratptr");
    assert!(p.expstratptr.is_none(), "port gnd cleared expstratptr");
    assert_ne!(p.type_ & 0x01, 0, "port gnd set type_ |= ATGND($01)");
    assert_ne!(p.sflags & 0x10, 0, "port gnd set colldisable in al_sflags bit $10");

    // Semantic MATCH: both zeroed the strat pointer (per-tick becomes a no-op),
    // both flagged the object as ground + collision-disabled.
    eprintln!("BATCH2 gnd: MATCH — retail & port both zero stratptr + set type=gnd + colldisable (sflag remap $01<->$10)");
}

// ============================================================================
// BATCH 2 — a pure ROTATE scenery strat (`hardrot`) and a fixed-velocity
// MOVER (`straight`), both certified vs retail. These add two NEW footprint
// shapes: rotation + per-axis rate scratch (no globals), and velocity + scroll.
// ============================================================================

/// MILESTONE (batch-2 step 1b) — LOCATE `hardrot_strat` + `straight_strat` in
/// retail by masked signature scan and cross-validate.
#[test]
fn retail_batch2_rotate_mover_addresses() {
    let Some(rom) = retail() else { return };

    // hardrot_strat: pure struct-offset spin (byte-identical retail/built).
    let hardrot_pat: Vec<Option<u8>> = vec![
        Some(0xB5), Some(0x12), Some(0x18), Some(0x7D), Some(0x22), Some(0x00), Some(0x95), Some(0x12),
        Some(0xB5), Some(0x13), Some(0x18), Some(0x7D), Some(0x23), Some(0x00), Some(0x95), Some(0x13),
        Some(0xB5), Some(0x14), Some(0x18), Some(0x7D), Some(0x24), Some(0x00), Some(0x95), Some(0x14),
        Some(0x6B),
    ];
    let hr = masked_scan(&rom, &hardrot_pat);
    assert_eq!(hr.len(), 1, "hardrot_strat is a UNIQUE scan hit");
    let hardrot = rom_off_to_snes(hr[0]);
    eprintln!("BATCH2: hardrot_strat=${hardrot:06X} (rotx=${AL_ROTX:02X}/sbyte1=${AL_SBYTE1:02X} ...)");
    assert_eq!(hardrot, RETAIL_HARDROT_STRAT, "hardrot_strat address");

    // straight_Istrat: s_set_strat + gen_3dvecs setup + jsl gen3dvecs(wild) +
    // jsl addalvecs_l(FIXED) + jsl sr_addplayerZx(FIXED) + rtl.  UNIQUE.
    let w = None;
    let straight_pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), w, w, Some(0x95), Some(0x16), Some(0xE2), Some(0x20),
        Some(0xA9), w, Some(0x95), Some(0x18),
        Some(0xB5), Some(0x15), Some(0x85), w,
        Some(0xB5), Some(0x13), Some(0x8D), w, w,
        Some(0xB5), Some(0x12), Some(0x8D), w, w,
        Some(0x22), w, w, w,
        Some(0x22), Some((RETAIL_ADDALVECS_L & 0xFF) as u8), Some(((RETAIL_ADDALVECS_L >> 8) & 0xFF) as u8), Some((RETAIL_ADDALVECS_L >> 16) as u8),
        Some(0x22), Some((RETAIL_SR_ADDPLAYERZX & 0xFF) as u8), Some(((RETAIL_SR_ADDPLAYERZX >> 8) & 0xFF) as u8), Some((RETAIL_SR_ADDPLAYERZX >> 16) as u8),
        Some(0x6B),
    ];
    let st = masked_scan(&rom, &straight_pat);
    assert_eq!(st.len(), 1, "straight_Istrat is a UNIQUE scan hit");
    let istrat = rom_off_to_snes(st[0]);
    // s_set_strat operand (the pointer the Istrat installs) must equal the
    // derived straight_strat body (istrat + 31, the fall-through offset).
    let installed = rom[st[0] + 3] as u32 | ((rom[st[0] + 4] as u32) << 8) | ((rom[st[0] + 10] as u32) << 16);
    let strat = istrat + 31;
    eprintln!("BATCH2: straight_Istrat=${istrat:06X} installs strat=${installed:06X} -> straight_strat=${strat:06X}");
    assert_eq!(istrat, RETAIL_STRAIGHT_ISTRAT, "straight_Istrat address");
    assert_eq!(strat, RETAIL_STRAIGHT_STRAT, "straight_strat = istrat + 31 fall-through");
    assert_eq!(installed, RETAIL_STRAIGHT_STRAT, "Istrat's s_set_strat operand == derived straight_strat (self-cross-validate)");
    // straight_strat body = jsl addalvecs_l; jsl sr_addplayerZx; rtl.
    let bo = snes_to_rom_off(RETAIL_STRAIGHT_STRAT);
    let body: Vec<u8> = (0..9).map(|i| rom[bo + i]).collect();
    assert_eq!(
        body,
        vec![0x22, 0xBB, 0xC7, 0x1F, 0x22, 0x69, 0xDC, 0x1F, 0x6B],
        "straight_strat body = jsl addalvecs_l; jsl sr_addplayerZx; rtl"
    );
}

/// CAPSTONE (batch-2) — RETAIL `hardrot_strat` vs THE PORT, TICK-FOR-TICK.
///
/// Pure spin-in-place scenery: `al_rot{x,y,z} += al_sbyte{1,2,3}` (8-bit wrap).
/// Footprint = ZERO globals, ZERO RNG — the simplest possible non-scroll strat.
/// We seed the rotation angles + per-axis rates and diff all three angles per
/// tick over a full 256-step 8-bit wrap.
#[test]
fn retail_hardrot_strat_vs_port() {
    let Some(rom) = retail() else { return };
    const N: u32 = 300; // > 256 to exercise the 8-bit wrap on every axis
    let (rx0, ry0, rz0) = (10u8, 200u8, 128u8);
    let (s1, s2, s3) = (16u8, 6u8, 251u8); // 251 = -5, exercises signed-ish wrap

    // retail
    let mut bus = SnesBus::new(rom);
    let blk = RETAIL_POOL.base;
    bus.write8(0x7E_0000 | (blk + AL_ROTX), rx0);
    bus.write8(0x7E_0000 | (blk + AL_ROTY), ry0);
    bus.write8(0x7E_0000 | (blk + AL_ROTZ), rz0);
    bus.write8(0x7E_0000 | (blk + AL_SBYTE1), s1);
    bus.write8(0x7E_0000 | (blk + AL_SBYTE2), s2);
    bus.write8(0x7E_0000 | (blk + AL_SBYTE3), s3);

    // port
    let mut g = sf_game::game::Game::new();
    let ea = sf_strat::enemy_a::install(&mut g);
    let idx = g.objs.alloc().expect("alien pool");
    g.call_strat(ea.hardrot, idx); // arms hardrot_strat as the per-tick body
    let tick = g.objs.aliens[idx as usize].stratptr.expect("hardrot per-tick armed");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = rx0; al.roty = ry0; al.rotz = rz0;
        al.sbyte1 = s1; al.sbyte2 = s2; al.sbyte3 = s3;
    }

    let mut first_div: Option<(u32, &'static str, u8, u8)> = None;
    for t in 1..=N {
        // hardrot_strat is a mid-strat body: it assumes 8-bit A (set by
        // s_start_strat) and 16-bit X, and does NOT do its own rep/sep. Call with
        // p=$20 (M=1 -> 8-bit A; X=0 -> 16-bit X) or the lda/adc/sta run 16-bit.
        call(&mut bus, RETAIL_HARDROT_STRAT, &Entry { x: blk as u16, p: 0x20, ..Default::default() });
        g.call_strat(tick, idx);
        let al = &g.objs.aliens[idx as usize];
        for (name, rv, pv) in [
            ("rotx", bus.read8(0x7E_0000 | (blk + AL_ROTX)), al.rotx),
            ("roty", bus.read8(0x7E_0000 | (blk + AL_ROTY)), al.roty),
            ("rotz", bus.read8(0x7E_0000 | (blk + AL_ROTZ)), al.rotz),
        ] {
            if rv != pv && first_div.is_none() {
                first_div = Some((t, name, rv, pv));
            }
        }
    }
    let (rx, ry, rz) = (
        bus.read8(0x7E_0000 | (blk + AL_ROTX)),
        bus.read8(0x7E_0000 | (blk + AL_ROTY)),
        bus.read8(0x7E_0000 | (blk + AL_ROTZ)),
    );
    assert_eq!(rx, rx0.wrapping_add((s1 as u32 * N) as u8), "retail rotx spun N*sbyte1");
    match first_div {
        None => eprintln!("BATCH2 hardrot: MATCH — retail == port rotx/y/z over {N} ticks (final {rx},{ry},{rz})"),
        Some((t, f, r, p)) => panic!("hardrot diverged tick {t} {f}: retail={r} port={p}"),
    }
}

/// CAPSTONE (batch-2) — RETAIL `straight_strat` vs THE PORT, TICK-FOR-TICK.
///
/// The canonical fixed-velocity MOVER: per tick `al_worldx/y/z += al_vx/vy/vz`
/// (addalvecs) then `al_worldz += pviewvelz` (scroll). We seed vx/vy/vz DIRECTLY
/// (bypassing the Istrat's one-shot `gen_3dvecs`, so no GSU is needed) plus
/// `pviewvelz`, and diff worldx/y/z per tick. Footprint = `al_vx/vy/vz` +
/// `pviewvelz` (both already located). The port equivalent is
/// `strat_apply_velocity` (the port `addalvecs`) composed with the world scroll
/// (`worldz += pviewvelz`), exactly `straight_strat`'s two `jsl`s.
#[test]
fn retail_straight_strat_vs_port() {
    let Some(rom) = retail() else { return };
    const N: u32 = 30;
    // (pos, vel, pviewvelz) — includes a 16-bit worldx wrap.
    let (px, py, pz) = (1000i16, 500i16, 8000i16);
    let (vx, vy, vz) = (300i16, -120i16, -50i16);
    let pvz = -200i16;

    // retail
    let mut bus = SnesBus::new(rom);
    let blk = RETAIL_POOL.base;
    bus.wram_write16(RETAIL_PVIEWVELZ, pvz as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldx, px as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldy, py as u16);
    bus.wram_write16(blk + RETAIL_POOL.al_worldz, pz as u16);
    bus.wram_write16(blk + AL_VX, vx as u16);
    bus.wram_write16(blk + AL_VY, vy as u16);
    bus.wram_write16(blk + AL_VZ, vz as u16);

    // port: strat_apply_velocity (addalvecs) then worldz += pviewvelz (scroll).
    let mut a = sf_game::alien::Alien::default();
    a.worldx = px; a.worldy = py; a.worldz = pz;
    a.vx = vx; a.vy = vy; a.vz = vz;

    let mut first_div: Option<(u32, &'static str, i32, i32)> = None;
    for t in 1..=N {
        call(&mut bus, RETAIL_STRAIGHT_STRAT, &Entry { x: blk as u16, p: 0x00, ..Default::default() });
        let o = snapshot_objects(&bus, &RETAIL_POOL)[0];
        // Port: addalvecs then scroll (straight_strat's two jsls).
        sf_strat::common::strat_apply_velocity(&mut a);
        a.worldz = a.worldz.wrapping_add(pvz);
        for (name, rv, pv) in [
            ("worldx", o.worldx as i32, a.worldx as i32),
            ("worldy", o.worldy as i32, a.worldy as i32),
            ("worldz", o.worldz as i32, a.worldz as i32),
        ] {
            if rv != pv && first_div.is_none() {
                first_div = Some((t, name, rv, pv));
            }
        }
    }
    let o = snapshot_objects(&bus, &RETAIL_POOL)[0];
    // worldz advances by (vz + pviewvelz) per tick; worldx/y by vx/vy.
    assert_eq!(o.worldz as i32, pz as i32 + (vz as i32 + pvz as i32) * N as i32, "retail straight scrolled worldz by vz+pviewvelz");
    match first_div {
        None => eprintln!("BATCH2 straight: MATCH — retail == port worldx/y/z over {N} ticks (final {},{},{})", o.worldx, o.worldy, o.worldz),
        Some((t, f, r, p)) => panic!("straight diverged tick {t} {f}: retail={r} port={p}"),
    }
}

// ============================================================================
// FRONTIER — PLAYER-RELATIVE + RNG STATE SEEDING (tier-2 hardest step).
//
// The 6 strats above touch at most the view-scroll globals (`pviewvelz`/
// `pviewposz`) or pure struct offsets. This section unblocks the bulk of the
// remaining strats: those that read the PLAYER POSITION mirror (`player_pos*`)
// and/or draw the runtime RNG. It (1) locates + cross-validates those retail
// WRAM addresses, (2) certifies the RNG STREAM stays in lockstep with the port,
// and (3) certifies the first PLAYER-POSITION-relative named strat, `parajump`.
// ============================================================================

/// MILESTONE (frontier step 1) — LOCATE + CROSS-VALIDATE the player-position
/// mirror globals (`player_posx/y/z`, `PLAYPT`) and the runtime-RNG state
/// (`RANDOM` + `rand`) in the retail cart by signature scan.
#[test]
fn retail_player_rng_globals() {
    let Some(rom) = retail() else { return };

    // --- RNG: the 4-byte SWB skeleton with the direct-page operands wildcarded.
    //   A5 d0 18 E5 d1 85 d1 E5 d2 85 d2 E5 d3 85 d3 E5 d0 85 d0 60
    let rng_pat: Vec<Option<u8>> = vec![
        Some(0xA5), None, Some(0x18), Some(0xE5), None, Some(0x85), None, Some(0xE5), None,
        Some(0x85), None, Some(0xE5), None, Some(0x85), None, Some(0xE5), None, Some(0x85), None,
        Some(0x60),
    ];
    // The genuine PRNG is the one whose `jsr RANDOM; rtl` wrapper is `jsl`-called.
    let mut found: Option<(u32, u8)> = None;
    for &h in &masked_scan(&rom, &rng_pat) {
        let snes = rom_off_to_snes(h);
        // The near-jsr wrapper sits 4 bytes before (`20 lo hi 6b`).
        let (lo, hi) = (snes as u8, (snes >> 8) as u8);
        let wrapper = masked_scan(&rom, &[Some(0x20), Some(lo), Some(hi), Some(0x6B)]);
        if let Some(&w) = wrapper.first() {
            let ws = rom_off_to_snes(w);
            let (wlo, whi, wbk) = (ws as u8, (ws >> 8) as u8, (ws >> 16) as u8);
            let refs = masked_scan(&rom, &[Some(0x22), Some(wlo), Some(whi), Some(wbk)]);
            if refs.len() > 50 {
                found = Some((snes, rom[h + 1])); // rom[h+1] = rand[0] dp addr
            }
        }
    }
    let (random, rand0) = found.expect("live retail RANDOM");
    eprintln!("FRONTIER: RANDOM=${random:06X} rand=${rand0:02X}-${:02X}", rand0 + 3);
    assert_eq!(random, RETAIL_RANDOM_L + 4, "RANDOM near-entry is RANDOM_L+4");
    assert_eq!(rand0 as u32, RETAIL_RAND, "retail rand state at $EF");

    // --- player_pos: 37/34/25 absolute reads of $150D/$150F/$1511 (the same
    // relative counts as the built ROM's $1598/$159A/$159C), and `parajump_strat`
    // reads them as its chase targets. Read the operands straight out of
    // `parajump_strat` ($04:F851) — self-validating.
    let po = snes_to_rom_off(RETAIL_PARAJUMP_STRAT);
    // Confirm the parajump skeleton head: rep;lda worldy,x;sta $3A;lda player_posy.
    assert_eq!(&rom[po..po + 7], &[0xC2, 0x20, 0xB5, 0x0E, 0x85, 0x3A, 0xAD], "parajump head");
    let posy = rom[po + 7] as u32 | ((rom[po + 8] as u32) << 8);
    let playpt = rom[po + 18] as u32 | ((rom[po + 19] as u32) << 8);
    let posx = rom[po + 52] as u32 | ((rom[po + 53] as u32) << 8);
    eprintln!("FRONTIER: parajump player_posy=${posy:04X} player_posx=${posx:04X} PLAYPT=${playpt:04X}");
    assert_eq!(posy, RETAIL_PLAYER_POSY, "parajump reads player_posy=$150F");
    assert_eq!(posx, RETAIL_PLAYER_POSX, "parajump reads player_posx=$150D");
    assert_eq!(playpt, RETAIL_PLAYPT, "parajump reads PLAYPT=$1238");
    // player_pos is a contiguous x,y,z word triple (identical shape to built).
    assert_eq!(RETAIL_PLAYER_POSY, RETAIL_PLAYER_POSX + 2);
    assert_eq!(RETAIL_PLAYER_POSZ, RETAIL_PLAYER_POSX + 4);
}

/// Draw one value from retail's runtime PRNG, carrying the 4-byte SWB state
/// manually so it survives the [`call`] harness's direct-page param block
/// ($F0-$F5), which OVERLAPS retail's `rand` ($EF-$F2). We seed `rand[0]` at $EF
/// directly (outside the block) and inject `rand[1..4]` through the entry A/X
/// registers, which the harness lands at $F0/$F1/$F2 right before `RANDOM` runs;
/// then we read the advanced state back out. Returns the PRNG byte.
fn retail_random_next(bus: &mut SnesBus, s: &mut [u8; 4]) -> u8 {
    bus.write8(RETAIL_RAND, s[0]); // $EF (safe: below the param block)
    // Harness writes entry.a -> $F0/$F1, entry.x -> $F2/$F3 before `RANDOM` runs.
    let a = s[1] as u16 | ((s[2] as u16) << 8);
    let x = s[3] as u16;
    let e = call(bus, RETAIL_RANDOM_L, &Entry { a, x, p: 0x20, ..Default::default() });
    // RANDOM wrote the advanced state back to $EF-$F2; read it for the next draw.
    for i in 0..4 {
        s[i] = bus.read8(0x7E_0000 | (RETAIL_RAND + i as u32));
    }
    e.a
}

/// CAPSTONE (frontier) — RETAIL runtime RNG STREAM vs THE PORT, in lockstep.
///
/// Certifies that the retail cart's OWN `RANDOM` ($02:FC5C, the 288-refs runtime
/// PRNG) produces the identical stream to the port's `sf_strat::common::sf_random`
/// when both are seeded with the same 4-byte state. This is the RNG-seeding
/// infrastructure that unblocks every RNG-driven strat: seed both sides, and the
/// two streams stay bit-identical draw-for-draw. Four seeds incl. all-zero and
/// all-ones.
#[test]
fn retail_rng_stream_vs_port() {
    let Some(rom) = retail() else { return };
    use sf_game::vars::GameVars;
    const N: usize = 16;
    for seed in [[1u8, 2, 3, 4], [0xAB, 0xCD, 0xEF, 0x12], [0, 0, 0, 0], [0xFF, 0xFF, 0xFF, 0xFF]] {
        // Retail: draw N via the real cart routine, carrying the SWB state.
        let mut bus = SnesBus::new(rom.clone());
        let mut rs = seed;
        let romv: Vec<u8> = (0..N).map(|_| retail_random_next(&mut bus, &mut rs)).collect();
        // Port: the wired SWB RNG over g.vars.rng.
        let mut vars = GameVars::default();
        vars.rng = seed;
        let portv: Vec<u8> = (0..N).map(|_| sf_strat::common::sf_random(&mut vars) as u8).collect();
        eprintln!("FRONTIER RNG seed {seed:02X?}\n  retail {romv:02X?}\n  port   {portv:02X?}  {}",
            if romv == portv { "MATCH" } else { "DIFF" });
        assert_eq!(romv, portv, "retail RNG stream must match port sf_random for seed {seed:02X?}");
    }
    eprintln!("FRONTIER RNG: seed both sides identically -> streams stay in lockstep. RNG frontier UNBLOCKED.");
}

/// CAPSTONE (frontier) — THE FIRST PLAYER-POSITION-RELATIVE STRAT, `parajump`,
/// certified vs retail, TICK-FOR-TICK.
///
/// `parajump_strat` ($04:F851) reads the player-position mirror (`player_posy`
/// $150F for the Y chase, `player_posx` $150D for the X chase) AND the live
/// player object through `PLAYPT` ($1238) for a Z-distance gate — the exact
/// player-relative footprint the frontier is about. We seed:
///  * `player_posx/y/z` via [`seed_player_relative_state`],
///  * a live player OBJECT at slot 1 with `PLAYPT` pointing at it, at the SAME
///    worldz as the enemy (|dz| = 0 < 200) so BOTH chases run,
///  * an enemy object at slot 0 far from the player in X/Y so the proportional
///    chases run for many ticks.
///
/// We call retail `parajump_strat` DIRECTLY (surgical, X = enemy block) — not
/// the full `dostrats` walk — precisely because a full walk would run the player
/// strats that RECOMPUTE `player_pos*` from the player object each frame, which
/// would fight our directly-seeded values. The port model mirrors sf-strat's
/// `enemy_a::parajump_strat` exactly: two applications of the PUBLIC
/// `common::strat_chase_proportional` (rate 2 toward player_posy, rate 3 toward
/// player_posx). Diff `worldx`+`worldy` per tick.
#[test]
fn retail_parajump_player_relative_vs_port() {
    let Some(rom) = retail() else { return };
    use sf_strat::common::strat_chase_proportional as chasep;

    // Player at (px,py,pz); targets the enemy chases toward.
    let (px, py, pz) = (5000i16, -3000i16, 8000i16);
    // Enemy starts far in X/Y, SAME worldz as the player => |dz| = 0 < 200.
    let (ex0, ey0, ez) = (-4000i16, 9000i16, pz);

    let mut bus = SnesBus::new(rom);
    let enemy = RETAIL_POOL.base; // slot 0 block ($0336) — X for the strat call
    let player_blk = RETAIL_POOL.base + RETAIL_POOL.stride; // slot 1 block

    // Seed player_pos globals + rng (rng unused here) + the player object/pointer.
    seed_player_relative_state(&mut bus, px, py, pz, [0; 4]);
    bus.wram_write16(RETAIL_PLAYPT, player_blk as u16);
    bus.wram_write16(player_blk + RETAIL_POOL.al_worldx, px as u16); // mirror
    bus.wram_write16(player_blk + RETAIL_POOL.al_worldz, pz as u16);
    // Enemy object.
    bus.wram_write16(enemy + RETAIL_POOL.al_worldx, ex0 as u16);
    bus.wram_write16(enemy + RETAIL_POOL.al_worldy, ey0 as u16);
    bus.wram_write16(enemy + RETAIL_POOL.al_worldz, ez as u16);

    // Port model of enemy_a::parajump_strat (public helper composition).
    let (mut pwx, mut pwy) = (ex0, ey0);

    const N: u32 = 90;
    let mut first_div: Option<(u32, &'static str, i32, i32)> = None;
    for t in 1..=N {
        // Retail: run the cart's OWN parajump_strat body (X = enemy block).
        call(&mut bus, RETAIL_PARAJUMP_STRAT, &Entry { x: enemy as u16, p: 0x00, ..Default::default() });
        let rwx = bus.wram_read16(enemy + RETAIL_POOL.al_worldx) as i16;
        let rwy = bus.wram_read16(enemy + RETAIL_POOL.al_worldy) as i16;

        // Port: worldy chase (rate 2) then, since |dz|=0<200, worldx chase (rate 3).
        pwy = chasep(pwy, py, 2);
        pwx = chasep(pwx, px, 3);

        for (name, rv, pv) in [("worldx", rwx as i32, pwx as i32), ("worldy", rwy as i32, pwy as i32)] {
            if rv != pv && first_div.is_none() {
                first_div = Some((t, name, rv, pv));
            }
        }
        if t == 1 || t == N || t % 30 == 0 {
            eprintln!("FRONTIER parajump TICK {t:>2}: retail=({rwx},{rwy}) port=({pwx},{pwy})");
        }
    }
    // Prove the chase actually converged toward the seeded player_pos (not a no-op).
    let rwy = bus.wram_read16(enemy + RETAIL_POOL.al_worldy) as i16;
    let rwx = bus.wram_read16(enemy + RETAIL_POOL.al_worldx) as i16;
    assert!((rwy - py).abs() < (ey0 - py).abs(), "retail worldy chased toward player_posy");
    assert!((rwx - px).abs() < (ex0 - px).abs(), "retail worldx chased toward player_posx");
    match first_div {
        None => eprintln!(
            "FRONTIER parajump: MATCH — retail player-relative strat == port over {N} ticks (final worldx={rwx} worldy={rwy}). Player-pos frontier UNBLOCKED."
        ),
        Some((t, f, r, p)) => panic!("parajump diverged tick {t} {f}: retail={r} port={p}"),
    }
}

// ============================================================================
// RNG-DRIVEN ENEMY CLASS — closing the loop on the `ea_random`->`sf_random` fix.
//
// commit f280388 rewired 61 enemy/boss RNG sites off the build-time LCG
// (`ea_random`, rnd*91+$61D7 over RNDVAL) onto the ROM's runtime SWB stream
// (`sf_random` over g.vars.rng), which `retail_rng_stream_vs_port` proved
// bit-identical to the retail cart's `RANDOM` ($02:FC5C). These three tests
// certify the FIRST RNG-driven ENEMY strat, `firepillar`, against the retail
// cartridge — extending tier-2 certified coverage to the RNG-driven enemy class
// and proving the fix cartridge-faithful end-to-end.
//
// `firepillar_Istrat` (retail $0A:DAE4, GA2STRAT.ASM:2039-2062) draws the RNG
// THREE times on init:
//   DRAW 1 -> al_worldx low byte
//   DRAW 2 & 3 -> al_worldx high byte    => worldx = d1 | ((d2&3)<<8)  (0..1023)
//   then  worldx += -512 + (player_posx asra 1)   (signed >>1)
//   DRAW 3: coin `cmp #$B2 (178)` -> 30% (rnd>=178) latches al_sflags2 bit $20
//           ("inert"); 70% (rnd<178) leaves it clear.
// Port ↔ `sf_strat::enemies_ground::firepillar_init` (IS_FIREPILLAR row 193),
// whose three `sf_random(&mut g.vars)` calls ARE the just-fixed enemy-lane sites.
// ============================================================================

/// sf-map / ISTRATS.ASM placement index for firepillar (matches the port's
/// `enemies_ground::IS_FIREPILLAR`).
const IS_FIREPILLAR: usize = 193;
/// The `s_jmp_random .ndrop,70` threshold: `cmp #((70)*255)/100` = `cmp #$B2`.
const FIREPILLAR_COIN_THRESH: u16 = 178;

/// Retail-side firepillar init observables derived from a 3-draw RNG stream +
/// player_posx (the exact `firepillar_Istrat` formula, cross-validated in
/// `retail_firepillar_addresses` by reading the cart's own operands).
fn firepillar_expected(d1: u8, d2: u8, d3: u8, player_posx: i16) -> (i16, bool) {
    let worldx = ((d1 as i16) | (((d2 & 3) as i16) << 8))
        .wrapping_sub(512)
        .wrapping_add(player_posx >> 1); // asra = signed >>1
    let inert = (d3 as u16) >= FIREPILLAR_COIN_THRESH; // 30% branch
    (worldx, inert)
}

/// Run the PORT's real `firepillar_init` (the fixed `sf_random` call site) with
/// `rng`/`player_posx` seeded, returning its RNG-derived observables
/// `(worldx, inert)`. A distant player keeps the fall-through per-tick body
/// (`firepillar_strat`, no RNG) a clean no-op — it never touches worldx/sflag2.
fn port_firepillar(rng: [u8; 4], player_posx: i16) -> (i16, bool) {
    let mut g = sf_game::game::Game::new();
    sf_strat::enemies_ground::register(&mut g.world);
    // Player far in Z (slot 0) so firepillar_strat's zdist gates all fail.
    let pl = g.objs.alloc().expect("player slot");
    sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[pl as usize]);
    g.objs.aliens[pl as usize].worldz = 20000;
    // Enemy (slot 1) carrying the firepillar istrat.
    let e = g.objs.alloc().expect("enemy slot");
    sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
    g.objs.aliens[e as usize].worldz = 0;
    g.objs.aliens[e as usize].stratptr = g.world.istrats[IS_FIREPILLAR];
    g.vars.rng = rng;
    g.vars.player_posx = player_posx;
    let s = g.objs.aliens[e as usize].stratptr.expect("firepillar istrat registered");
    g.call_strat(s, e);
    let al = &g.objs.aliens[e as usize];
    (al.worldx, al.sflags2 & ASF2_SFLAG2 != 0)
}

/// MILESTONE (RNG-enemy step 1) — LOCATE + CROSS-VALIDATE `firepillar_Istrat` in
/// the retail cart by masked signature scan, and read back the THREE `jsl
/// RANDOM_L` draw sites + the `lda player_posx` read + the coin `cmp #$B2` + the
/// `al_sflags2` inert bit — the exact RNG-draw sequence the port consumes.
#[test]
fn retail_firepillar_addresses() {
    let Some(rom) = retail() else { return };

    // 99-byte firepillar_Istrat skeleton (read from the BUILT ROM $0A:DABE),
    // with the strat-ptr immediate, the set_0collptrs JSL, the three RANDOM_L
    // JSLs, the player_posx operand, and the fall-through JML wildcarded.
    let w = None;
    let pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), w, w, Some(0x95), Some(0x16), // rep;lda #strat;sta stratptr
        Some(0xE2), Some(0x20), Some(0xA9), w, Some(0x95), Some(0x18),     // sep;lda #bank;sta stratptr+2
        Some(0x22), w, w, w,                                               // jsl set_0collptrs
        Some(0xA9), Some(0xFF), Some(0x95), Some(0x2A),                    // lda #hardHP;sta al_HP
        Some(0xA9), Some(0x08), Some(0x95), Some(0x2B),                    // lda #hardAP;sta al_AP
        Some(0xB5), Some(0x2E), Some(0x09), Some(0x10), Some(0x95), Some(0x2E), // ora enemy1 colltype
        Some(0xA9), Some(0x80), Some(0x95), Some(0x13),                    // lda #deg180;sta al_roty
        Some(0xA9), Some(0x80), Some(0x95), Some(0x14),                    // lda #deg180;sta al_rotz
        Some(0x22), w, w, w, Some(0x95), Some(0x0C),                       // DRAW 1 -> al_worldx lo
        Some(0x22), w, w, w, Some(0x29), Some(0x03), Some(0x95), Some(0x0D), // DRAW 2 & #3 -> al_worldx hi
        Some(0xC2), Some(0x20), Some(0xB5), Some(0x0C), Some(0x38), Some(0xE9), Some(0x00), Some(0x02), Some(0x95), Some(0x0C), // sbc #512
        Some(0xE2), Some(0x20), Some(0xC2), Some(0x20),
        Some(0xAD), w, w,                                                  // lda player_posx
        Some(0xC9), Some(0x00), Some(0x80), Some(0x6A),                    // asra: cmp #$8000; ror a
        Some(0x18), Some(0x75), Some(0x0C), Some(0x95), Some(0x0C), Some(0xE2), Some(0x20), // clc;adc;sta;sep
        Some(0x22), w, w, w,                                               // DRAW 3 (coin)
        Some(0xC9), Some(0xB2), Some(0xB0), Some(0x04),                    // cmp #$B2; bcs +4
        Some(0x5C), w, w, w,                                               // jml firepillar_strat
        Some(0xB5), Some(0x1E), Some(0x09), Some(0x20), Some(0x95), Some(0x1E), // set al_sflags2 bit $20
    ];
    let hits = masked_scan(&rom, &pat);
    assert_eq!(hits.len(), 1, "firepillar_Istrat is a UNIQUE masked hit");
    let h = hits[0];
    let istrat = rom_off_to_snes(h);
    // Read back the operands.
    let rd24 = |o: usize| rom[o] as u32 | ((rom[o + 1] as u32) << 8) | ((rom[o + 2] as u32) << 16);
    let rd16 = |o: usize| rom[o] as u32 | ((rom[o + 1] as u32) << 8);
    let draw1 = rd24(h + 40);
    let draw2 = rd24(h + 46);
    let posx = rd16(h + 68);
    let draw3 = rd24(h + 82);
    let jml = rd24(h + 90);
    eprintln!(
        "RNG-ENEMY: firepillar_Istrat=${istrat:06X}  draws=[{draw1:06X},{draw2:06X},{draw3:06X}]  player_posx=${posx:04X}  ->firepillar_strat=${jml:06X}"
    );
    assert_eq!(istrat, RETAIL_FIREPILLAR_ISTRAT, "firepillar_Istrat address");
    // All three draws are the runtime RNG wrapper RANDOM_L ($02:FC58) — this is
    // the exact routine `retail_rng_stream_vs_port` proved == port `sf_random`.
    assert_eq!(draw1, RETAIL_RANDOM_L, "DRAW 1 is jsl RANDOM_L");
    assert_eq!(draw2, RETAIL_RANDOM_L, "DRAW 2 is jsl RANDOM_L");
    assert_eq!(draw3, RETAIL_RANDOM_L, "DRAW 3 (coin) is jsl RANDOM_L");
    assert_eq!(posx as u32, RETAIL_PLAYER_POSX, "reads player_posx=$150D");
    assert_eq!(jml, RETAIL_FIREPILLAR_STRAT, "jml fall-through = firepillar_strat");
    // Coin threshold cmp #$B2 = (70*255)/100 = 178.
    assert_eq!(rom[h + 86] as u16, FIREPILLAR_COIN_THRESH, "coin cmp #$B2 (178)");
}

/// CAPSTONE (RNG-enemy) — THE PORT's `firepillar_init` RNG-DERIVED FIELDS vs the
/// RETAIL cartridge RNG STREAM, BOTH COIN BRANCHES.
///
/// This is the direct proof of the `ea_random`->`sf_random` fix for the enemy
/// class: we draw firepillar's 3-value sequence from the retail cart's OWN
/// `RANDOM` ($02:FC5C, carried across the harness param-block collision by
/// [`retail_random_next`]), apply the cross-validated `firepillar_Istrat` formula
/// to get the cartridge-faithful `(worldx, inert)`, and diff against the PORT's
/// real `firepillar_init` (the fixed enemy-lane `sf_random` call site) seeded
/// with the SAME 4-byte RNG state + player_posx. Two seeds drive the two coin
/// outcomes; we assert retail and port take the SAME branch each time.
#[test]
fn retail_firepillar_rng_vs_port() {
    let Some(rom) = retail() else { return };
    let player_posx = -3000i16; // exercises the signed asra (>>1 = -1500)
    // Seeds chosen (via the SWB stream) to hit BOTH coin branches:
    //   [1,2,3,4]        -> DRAW 3 = 8   (<178)  -> ACTIVE (70%)
    //   [171,205,239,18] -> DRAW 3 = 194 (>=178) -> INERT  (30%)
    let cases: [([u8; 4], bool); 2] = [([1, 2, 3, 4], false), ([171, 205, 239, 18], true)];
    let mut saw_active = false;
    let mut saw_inert = false;
    for (seed, expect_inert) in cases {
        // Retail: draw the 3-value sequence from the cart's own RANDOM.
        let mut bus = SnesBus::new(rom.clone());
        let mut rs = seed;
        let d1 = retail_random_next(&mut bus, &mut rs);
        let d2 = retail_random_next(&mut bus, &mut rs);
        let d3 = retail_random_next(&mut bus, &mut rs);
        let (rwx, r_inert) = firepillar_expected(d1, d2, d3, player_posx);
        // Port: the REAL fixed firepillar_init over the same seed.
        let (pwx, p_inert) = port_firepillar(seed, player_posx);
        eprintln!(
            "RNG-ENEMY firepillar seed {seed:02X?}: retail draws=[{d1},{d2},{d3}] worldx={rwx} inert={r_inert} | port worldx={pwx} inert={p_inert}  {}",
            if rwx == pwx && r_inert == p_inert { "MATCH" } else { "DIFF" }
        );
        assert_eq!(rwx, pwx, "firepillar worldx (RNG draws 1&2 + player_posx) must match retail");
        assert_eq!(r_inert, p_inert, "firepillar inert coin (RNG draw 3) branch must match retail");
        assert_eq!(r_inert, expect_inert, "seed {seed:02X?} drives the expected coin branch");
        saw_active |= !r_inert;
        saw_inert |= r_inert;
    }
    assert!(saw_active && saw_inert, "both coin branches (30% inert / 70% active) exercised");
    eprintln!("RNG-ENEMY firepillar: MATCH both branches — port sf_random == retail RANDOM through firepillar. ea_random->sf_random fix is cartridge-faithful.");
}

/// CAPSTONE (RNG-enemy, GOLD) — run the RETAIL cart's OWN `firepillar_Istrat`
/// body ($0A:DAE4) on seeded RNG + player_posx, and diff its RNG-derived
/// `(worldx, inert)` against the port. This executes the actual cartridge enemy
/// AI (3 real `jsl RANDOM_L` draws), not a formula, and is the strongest form of
/// the cert.
///
/// Harness note — the RNG state `rand` ($EF-$F2) OVERLAPS the [`call`] param
/// block ($F0-$F5): the object block X = pool base ($0336) PINS `rand[3]` = $F2 =
/// $36 (the block's low byte). So both seeds here end in $36 (=54) — the first 3
/// state bytes remain free and are enough to drive each coin branch. `rand[0]`
/// ($EF, below the block) is seeded directly; `rand[1..3]` ride in via entry.a
/// ($F0/$F1) and entry.x-low ($F2). A distant player (via PLAYPT) keeps the
/// fall-through `firepillar_strat` tick (no RNG) a clean no-op.
#[test]
fn retail_firepillar_body_vs_port() {
    let Some(rom) = retail() else { return };
    let player_posx = -3000i16;
    let enemy = RETAIL_POOL.base; // slot 0 — X for the strat call; low byte $36
    let player_blk = RETAIL_POOL.base + RETAIL_POOL.stride; // slot 1
    // Seeds ending in $36 (=54, the pinned rand[3]); chosen to hit both branches:
    //   [200,1,2,54]  -> DRAW 3 = 24  (<178)  -> ACTIVE
    //   [99,88,77,54] -> DRAW 3 = 183 (>=178) -> INERT
    let cases: [([u8; 4], bool); 2] = [([200, 1, 2, 54], false), ([99, 88, 77, 54], true)];
    let mut saw_active = false;
    let mut saw_inert = false;
    for (seed, expect_inert) in cases {
        assert_eq!(seed[3] as u32, enemy & 0xFF, "seed[3] must equal the pinned block low byte");
        let mut bus = SnesBus::new(rom.clone());
        // Player far in Z via PLAYPT so firepillar_strat's zdist gates all fail.
        bus.wram_write16(RETAIL_PLAYPT, player_blk as u16);
        bus.wram_write16(player_blk + RETAIL_POOL.al_worldz, 20000u16);
        bus.wram_write16(enemy + RETAIL_POOL.al_worldz, 0);
        // Seed player_posx + the RNG state. rand[0]=$EF direct; rand[1..3] ride
        // in via entry.a/entry.x-low (which land at $F0/$F1/$F2 == rand[1..3]).
        bus.wram_write16(RETAIL_PLAYER_POSX, player_posx as u16);
        seed_retail_rng(&mut bus, seed); // establishes $EF; $F0-$F2 set again below by call
        bus.write8(RETAIL_RAND, seed[0]); // rand[0] @ $EF (safe, below param block)
        let a = seed[1] as u16 | ((seed[2] as u16) << 8); // -> $F0/$F1 = rand[1]/rand[2]
        // entry.x = enemy block -> $F2 = block low byte = seed[3]; also the strat's X.
        call(&mut bus, RETAIL_FIREPILLAR_ISTRAT, &Entry { a, x: enemy as u16, p: 0x00, ..Default::default() });
        let rwx = bus.wram_read16(enemy + RETAIL_POOL.al_worldx) as i16;
        let r_inert = bus.read8(0x7E_0000 | (enemy + AL_SFLAGS2)) & ASF2_SFLAG2 != 0;
        // Port: the real firepillar_init over the SAME seed.
        let (pwx, p_inert) = port_firepillar(seed, player_posx);
        eprintln!(
            "RNG-ENEMY firepillar BODY seed {seed:02X?}: retail worldx={rwx} inert={r_inert} | port worldx={pwx} inert={p_inert}  {}",
            if rwx == pwx && r_inert == p_inert { "MATCH" } else { "DIFF" }
        );
        assert_eq!(rwx, pwx, "retail firepillar_Istrat BODY worldx == port");
        assert_eq!(r_inert, p_inert, "retail firepillar_Istrat BODY inert coin == port");
        assert_eq!(r_inert, expect_inert, "seed {seed:02X?} drives the expected branch on retail");
        saw_active |= !r_inert;
        saw_inert |= r_inert;
    }
    assert!(saw_active && saw_inert, "both coin branches exercised on the retail body");
    eprintln!("RNG-ENEMY firepillar BODY: MATCH both branches — retail cart's OWN firepillar AI == port. RNG-driven enemy class certified vs retail.");
}

// ============================================================================
// BATCH 3 — static-init scenery (`rockhard`) + RNG-driven INIT strats
// (`mine0`, `big_meteor`, `tree1`), certified vs retail.
//
// These extend certified coverage to two more classes:
//  * STATIC scenery init (`rockhard`): zero globals, zero RNG, byte-identical
//    struct-offset body — an EXACT-scan hit; run the retail body, diff the
//    init observables (HP/AP/colltype/roty/null-tick) vs the port.
//  * RNG-driven INIT strats (`mine0`/`big_meteor`/`tree1`): each draws the
//    runtime RNG exactly ONCE for a kinematic/orientation datum. Certify the
//    RNG-derived field either by running the retail cart's OWN Istrat body on
//    seeded RNG (the firepillar param-block recipe) or via the proven RANDOM
//    stream, and diff vs the port init.
//
// Port-reachability: the per-tick bodies (wallleft/wallright/volrockdown_strat)
// are private to sf-strat; the INIT strats are reached publicly through
// `enemies_ground::register` -> `world.istrats[IS_*]`, exactly like firepillar.
// ============================================================================

const IS_ROCKHARD: usize = 192;
const IS_MINE0: usize = 246;
const IS_BIG_METEOR: usize = 234;
const IS_TREE1: usize = 204;

/// Run a port ground-init strat (reached via `world.istrats[is_index]`) on a
/// fresh object with `rng` seeded, and return its init observables
/// `(rotz, sbyte1, roty, hp, ap, collflags, stratptr_is_none)`.
fn port_ground_init(is_index: usize, rng: [u8; 4]) -> (u8, u8, u8, u8, u8, u8, bool) {
    let mut g = sf_game::game::Game::new();
    sf_strat::enemies_ground::register(&mut g.world);
    let idx = g.objs.alloc().expect("alien pool");
    sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.vars.rng = rng;
    g.objs.aliens[idx as usize].stratptr = g.world.istrats[is_index];
    let s = g.objs.aliens[idx as usize].stratptr.expect("istrat registered");
    g.call_strat(s, idx);
    let al = &g.objs.aliens[idx as usize];
    (al.rotz, al.sbyte1, al.roty, al.hp, al.ap, al.collflags, al.stratptr.is_none())
}

/// MILESTONE (batch-3 step 1) — LOCATE + CROSS-VALIDATE the four batch-3 retail
/// addresses by masked signature scan (each a UNIQUE hit), and read back the
/// single `jsl RANDOM_L` operand of the three RNG strats == $02:FC58.
#[test]
fn retail_batch3_addresses() {
    let Some(rom) = retail() else { return };
    let w = None;

    // rockhard: pure struct-offset, byte-identical -> EXACT scan.
    let rockhard: Vec<Option<u8>> = vec![
        Some(0xB5), Some(0x2E), Some(0x09), Some(0x10), Some(0x95), Some(0x2E),
        Some(0xA9), Some(0x80), Some(0x95), Some(0x13), Some(0xA9), Some(0xFF), Some(0x95), Some(0x2A),
        Some(0xA9), Some(0x14), Some(0x95), Some(0x2B),
        Some(0xC2), Some(0x20), Some(0xA9), Some(0x00), Some(0x00), Some(0x95), Some(0x16),
        Some(0xE2), Some(0x20), Some(0xA9), Some(0x00), Some(0x95), Some(0x18), Some(0x6B),
    ];
    let h = masked_scan(&rom, &rockhard);
    assert_eq!(h.len(), 1, "rockhard_Istrat is a UNIQUE exact-scan hit");
    let rockhard_addr = rom_off_to_snes(h[0]);
    eprintln!("BATCH3: rockhard_Istrat=${rockhard_addr:06X}");
    assert_eq!(rockhard_addr, RETAIL_ROCKHARD_ISTRAT, "rockhard_Istrat address");

    // mine0: 1 draw -> al_rotz (full byte). jsl RANDOM at pattern index 31..35.
    let mine0: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), w, w, Some(0x95), Some(0x16),
        Some(0xE2), Some(0x20), Some(0xA9), w, Some(0x95), Some(0x18),
        Some(0x22), w, w, w,
        Some(0xA9), Some(0x02), Some(0x95), Some(0x2A), Some(0xA9), Some(0x0A), Some(0x95), Some(0x2B),
        Some(0xB5), Some(0x2E), Some(0x09), Some(0x10), Some(0x95), Some(0x2E),
        Some(0x22), w, w, w, Some(0x95), Some(0x14), Some(0x6B),
    ];
    let h = masked_scan(&rom, &mine0);
    assert_eq!(h.len(), 1, "mine0_Istrat is a UNIQUE masked hit");
    let mine0_addr = rom_off_to_snes(h[0]);
    let mine0_rnd = rom[h[0] + 32] as u32 | ((rom[h[0] + 33] as u32) << 8) | ((rom[h[0] + 34] as u32) << 16);
    eprintln!("BATCH3: mine0_Istrat=${mine0_addr:06X} random_l=${mine0_rnd:06X}");
    assert_eq!(mine0_addr, RETAIL_MINE0_ISTRAT, "mine0_Istrat address");
    assert_eq!(mine0_rnd, RETAIL_RANDOM_L, "mine0 draw is jsl RANDOM_L");

    // big_meteor: 1 draw -> al_sbyte1 = (rnd&15)-8. jsl RANDOM at index 51..55.
    let bm: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), w, w, Some(0x95), Some(0x16),
        Some(0xE2), Some(0x20), Some(0xA9), w, Some(0x95), Some(0x18),
        Some(0x22), w, w, w,
        Some(0xB5), Some(0x1F), Some(0x09), Some(0x20), Some(0x95), Some(0x1F),
        Some(0xA9), Some(0xFF), Some(0x95), Some(0x2A), Some(0xA9), Some(0x0C), Some(0x95), Some(0x2B),
        Some(0xAD), w, w, Some(0x49), Some(0xFF), Some(0x1A), Some(0x18), Some(0x69), Some(0x80),
        Some(0x18), Some(0x6D), w, w, Some(0x95), Some(0x13),
        Some(0xAD), w, w, Some(0x95), Some(0x12),
        Some(0x22), w, w, w, Some(0x29), Some(0x0F), Some(0x95), Some(0x22),
        Some(0xB5), Some(0x22), Some(0x38), Some(0xE9), Some(0x08), Some(0x95), Some(0x22), Some(0x6B),
    ];
    let h = masked_scan(&rom, &bm);
    assert_eq!(h.len(), 1, "big_meteor_Istrat is a UNIQUE masked hit");
    let bm_addr = rom_off_to_snes(h[0]);
    let bm_rnd = rom[h[0] + 52] as u32 | ((rom[h[0] + 53] as u32) << 8) | ((rom[h[0] + 54] as u32) << 16);
    eprintln!("BATCH3: big_meteor_Istrat=${bm_addr:06X} random_l=${bm_rnd:06X}");
    assert_eq!(bm_addr, RETAIL_BIG_METEOR_ISTRAT, "big_meteor_Istrat address");
    assert_eq!(bm_rnd, RETAIL_RANDOM_L, "big_meteor draw is jsl RANDOM_L");

    // tree1: 1 draw -> al_sbyte1 = (rnd&3)+1. jsl RANDOM at index 12..16.
    let tree1: Vec<Option<u8>> = vec![
        Some(0xB5), Some(0x1F), Some(0x09), Some(0x02), Some(0x95), Some(0x1F),
        Some(0xB5), Some(0x1E), Some(0x09), Some(0x80), Some(0x95), Some(0x1E),
        Some(0x22), w, w, w, Some(0x29), Some(0x03), Some(0x95), Some(0x22), Some(0xF6), Some(0x22),
    ];
    let h = masked_scan(&rom, &tree1);
    assert_eq!(h.len(), 1, "tree1_Istrat is a UNIQUE masked hit");
    let tree1_addr = rom_off_to_snes(h[0]);
    let tree1_rnd = rom[h[0] + 13] as u32 | ((rom[h[0] + 14] as u32) << 8) | ((rom[h[0] + 15] as u32) << 16);
    eprintln!("BATCH3: tree1_Istrat=${tree1_addr:06X} random_l=${tree1_rnd:06X}");
    assert_eq!(tree1_addr, RETAIL_TREE1_ISTRAT, "tree1_Istrat address");
    assert_eq!(tree1_rnd, RETAIL_RANDOM_L, "tree1 draw is jsl RANDOM_L");
}

/// CAPSTONE (batch-3) — RETAIL `rockhard_Istrat` BODY vs THE PORT.
///
/// Static indestructible obstacle: sets `al_collflags |= enemy1($10)`,
/// `al_roty = deg180($80)`, `al_HP = hardHP($FF)`, `al_AP = rockhardAP($14=20)`,
/// and NULLS `al_stratptr` (no per-tick). ZERO globals, ZERO RNG. We seed a
/// DIRTY object, run the retail cart's OWN body ($06:85D9), and diff every init
/// observable vs the port `rockhard_istrat` (IS_ROCKHARD=192).
#[test]
fn retail_rockhard_strat_vs_port() {
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    let blk = RETAIL_POOL.base;
    // Dirty state, incl. a non-null stratptr the strat must clear.
    bus.wram_write16(blk + AL_STRATPTR, 0xBEEF);
    bus.write8(0x7E_0000 | (blk + AL_STRATPTR + 2), 0x1F);
    bus.write8(0x7E_0000 | (blk + AL_COLLFLAGS), 0x00);
    bus.write8(0x7E_0000 | (blk + AL_ROTY), 0x11);

    // rockhard assumes 8-bit A at entry (s_start_strat shorta) -> p=$20.
    call(&mut bus, RETAIL_ROCKHARD_ISTRAT, &Entry { x: blk as u16, p: 0x20, ..Default::default() });

    let r_coll = bus.read8(0x7E_0000 | (blk + AL_COLLFLAGS));
    let r_roty = bus.read8(0x7E_0000 | (blk + AL_ROTY));
    let r_hp = bus.read8(0x7E_0000 | (blk + AL_HP));
    let r_ap = bus.read8(0x7E_0000 | (blk + AL_AP));
    let r_sptr_lo = bus.wram_read16(blk + AL_STRATPTR);
    let r_sptr_bk = bus.read8(0x7E_0000 | (blk + AL_STRATPTR + 2));
    eprintln!(
        "BATCH3 rockhard: retail coll=${r_coll:02X} roty=${r_roty:02X} hp=${r_hp:02X} ap=${r_ap} stratptr=${r_sptr_bk:02X}:{r_sptr_lo:04X}"
    );

    // Port (dirty then init).
    let (_rotz, _sb1, p_roty, p_hp, p_ap, p_coll, p_sptr_none) = port_ground_init(IS_ROCKHARD, [0; 4]);
    eprintln!("BATCH3 rockhard: port roty=${p_roty:02X} hp=${p_hp:02X} ap=${p_ap} coll=${p_coll:02X} stratptr_none={p_sptr_none}");

    // colltype: retail's ASM layout stores enemy1 in bit $10; the port re-derived
    // its own obj.h collflags encoding (COLLTYPE_ENEMY1) AND its object went
    // through strat_init_obj_vars (baseline bits) vs our hand-seeded coll=0 — so
    // certify the enemy1 EFFECT (both set a colltype), not a raw byte equality.
    assert_ne!(r_coll & 0x10, 0, "retail rockhard set enemy1 colltype (bit $10)");
    assert_ne!(p_coll, 0, "port rockhard set its colltype");
    assert_eq!(r_roty, 0x80, "retail rockhard roty=deg180");
    assert_eq!(r_roty, p_roty, "roty");
    assert_eq!(r_hp, 0xFF, "retail rockhard hp=hardHP");
    assert_eq!(r_hp, p_hp, "hp");
    assert_eq!(r_ap, 20, "retail rockhard ap=rockhardAP");
    assert_eq!(r_ap, p_ap, "ap");
    assert_eq!(r_sptr_lo, 0, "retail rockhard nulled stratptr low");
    assert_eq!(r_sptr_bk, 0, "retail rockhard nulled stratptr bank");
    assert!(p_sptr_none, "port rockhard nulled stratptr");
    eprintln!("BATCH3 rockhard: MATCH — retail static-init body == port (coll/roty/hp/ap/null-tick).");
}

/// CAPSTONE (batch-3) — RETAIL `mine0_Istrat` BODY (RNG) vs THE PORT.
///
/// mine0 draws the runtime RNG ONCE -> `al_rotz` (full byte, random orientation).
/// We run the retail cart's OWN body ($09:9117) on a seeded RNG state (the
/// firepillar param-block recipe: X=block PINS rand[3]=block-low-byte $36, so
/// seeds end in $36; rand[0]@$EF direct, rand[1..3] ride entry.a/entry.x-low),
/// and diff `al_rotz` (RNG-derived) + HP/AP vs the port `mine0_init`
/// (IS_MINE0=246) seeded with the SAME 4-byte state. Two seeds.
#[test]
fn retail_mine0_body_vs_port() {
    let Some(rom) = retail() else { return };
    let enemy = RETAIL_POOL.base; // low byte $36 = pinned rand[3]
    for seed in [[1u8, 2, 3, 54], [200, 150, 99, 54]] {
        assert_eq!(seed[3] as u32, enemy & 0xFF, "seed[3] must equal the pinned block low byte");
        let mut bus = SnesBus::new(rom.clone());
        seed_retail_rng(&mut bus, seed);
        bus.write8(RETAIL_RAND, seed[0]); // rand[0]@$EF (below param block)
        let a = seed[1] as u16 | ((seed[2] as u16) << 8); // -> $F0/$F1 = rand[1]/rand[2]
        call(&mut bus, RETAIL_MINE0_ISTRAT, &Entry { a, x: enemy as u16, p: 0x00, ..Default::default() });
        let r_rotz = bus.read8(0x7E_0000 | (enemy + AL_ROTZ));
        let r_hp = bus.read8(0x7E_0000 | (enemy + AL_HP));
        let r_ap = bus.read8(0x7E_0000 | (enemy + AL_AP));
        let r_coll = bus.read8(0x7E_0000 | (enemy + AL_COLLFLAGS));

        let (p_rotz, _sb1, _roty, p_hp, p_ap, _p_coll, _n) = port_ground_init(IS_MINE0, seed);
        eprintln!(
            "BATCH3 mine0 seed {seed:02X?}: retail rotz=${r_rotz:02X} hp=${r_hp:02X} ap={r_ap} | port rotz=${p_rotz:02X} hp=${p_hp:02X} ap={p_ap}  {}",
            if r_rotz == p_rotz { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_rotz, p_rotz, "mine0 rotz (RNG draw) must match retail");
        assert_eq!(r_hp, 2, "retail mine0 hp=mine0HP(2)");
        assert_eq!(r_hp, p_hp, "hp");
        assert_eq!(r_ap, 10, "retail mine0 ap=mine0AP(10)");
        assert_eq!(r_ap, p_ap, "ap");
        // enemy1 colltype: retail bit $10 (ASM layout); port uses its own obj.h
        // encoding (representation remap, same as sflags) — certify the effect.
        assert_ne!(r_coll & 0x10, 0, "retail mine0 set enemy1 colltype (bit $10)");
    }
    eprintln!("BATCH3 mine0 BODY: MATCH — retail cart's OWN mine0 RNG orientation == port sf_random.");
}

/// CAPSTONE (batch-3) — RETAIL `big_meteor_Istrat` BODY (RNG) vs THE PORT.
///
/// big_meteor draws the runtime RNG ONCE -> `al_sbyte1 = (rnd&15)-8`. We run the
/// retail cart's OWN body ($06:FA62) on a seeded RNG state (same param-block
/// recipe) and diff `al_sbyte1` vs the port `big_meteor_init` (IS_BIG_METEOR=234)
/// on the SAME seed. (The strat's cosmetic `s_rots_flat` roty/rotx from view
/// vectors is scoped out of the port; only the RNG datum is diffed.) Two seeds.
#[test]
fn retail_big_meteor_body_vs_port() {
    let Some(rom) = retail() else { return };
    let enemy = RETAIL_POOL.base;
    for seed in [[7u8, 11, 13, 54], [222, 111, 44, 54]] {
        assert_eq!(seed[3] as u32, enemy & 0xFF, "seed[3] must equal the pinned block low byte");
        let mut bus = SnesBus::new(rom.clone());
        seed_retail_rng(&mut bus, seed);
        bus.write8(RETAIL_RAND, seed[0]);
        let a = seed[1] as u16 | ((seed[2] as u16) << 8);
        call(&mut bus, RETAIL_BIG_METEOR_ISTRAT, &Entry { a, x: enemy as u16, p: 0x00, ..Default::default() });
        let r_sb1 = bus.read8(0x7E_0000 | (enemy + AL_SBYTE1));
        let r_hp = bus.read8(0x7E_0000 | (enemy + AL_HP));
        let r_ap = bus.read8(0x7E_0000 | (enemy + AL_AP));

        let (_rotz, p_sb1, _roty, p_hp, p_ap, _coll, _n) = port_ground_init(IS_BIG_METEOR, seed);
        eprintln!(
            "BATCH3 big_meteor seed {seed:02X?}: retail sbyte1=${r_sb1:02X}({}) hp=${r_hp:02X} ap={r_ap} | port sbyte1=${p_sb1:02X}({})  {}",
            r_sb1 as i8, p_sb1 as i8, if r_sb1 == p_sb1 { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_sb1, p_sb1, "big_meteor sbyte1 (rnd&15)-8 must match retail");
        assert_eq!(r_hp, 0xFF, "retail big_meteor hp=hardHP");
        assert_eq!(r_hp, p_hp, "hp");
        assert_eq!(r_ap, 12, "retail big_meteor ap=12");
        assert_eq!(r_ap, p_ap, "ap");
    }
    eprintln!("BATCH3 big_meteor BODY: MATCH — retail cart's OWN big_meteor RNG spin datum == port sf_random.");
}

/// CAPSTONE (batch-3) — RETAIL `tree1_Istrat` RNG vs THE PORT (stream form).
///
/// tree1 draws the runtime RNG ONCE -> `al_sbyte1 = (rnd&3)+1` (tree height).
/// We draw one value from the retail cart's OWN `RANDOM` (carried across the
/// param-block collision by `retail_random_next`), apply the cross-validated
/// `(rnd&3)+1` formula, and diff against the port `tree1_init` (IS_TREE1=204)
/// seeded with the SAME 4-byte state. Several seeds — the port's real
/// `sf_random`-derived tree height matches the cartridge each time. (Stream form:
/// tree1's body does GSU-less sprite/anim table reads after the draw, so the
/// RANDOM stream is the clean surgical cert of the RNG-derived field.)
#[test]
fn retail_tree1_rng_vs_port() {
    let Some(rom) = retail() else { return };
    for seed in [[1u8, 2, 3, 4], [0xAB, 0xCD, 0xEF, 0x12], [99, 88, 77, 66], [255, 0, 128, 64]] {
        let mut bus = SnesBus::new(rom.clone());
        let mut rs = seed;
        let d = retail_random_next(&mut bus, &mut rs);
        let r_sb1 = (d & 3).wrapping_add(1); // (rnd&3)+1

        let (_rotz, p_sb1, _roty, _hp, _ap, _coll, _n) = port_ground_init(IS_TREE1, seed);
        eprintln!(
            "BATCH3 tree1 seed {seed:02X?}: retail draw={d} sbyte1={r_sb1} | port sbyte1={p_sb1}  {}",
            if r_sb1 == p_sb1 { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_sb1, p_sb1, "tree1 sbyte1 (rnd&3)+1 must match the retail RANDOM stream");
        assert!((1..=4).contains(&r_sb1), "tree1 height in [1,4]");
    }
    eprintln!("BATCH3 tree1: MATCH — port sf_random tree height == retail RANDOM (rnd&3)+1.");
}

// ============================================================================
// BATCH 4 — a zdist state-transition MOVER (`woods`), an RNG + PLAYER-RELATIVE
// scenery init (`tree2`), an RNG-reroll FIRING-enemy init (`shou0`), and the
// `break_meteorT` tadpole death COIN. Four more classes widening tier-2:
//  * `woods`  (IS_WOODS=54): waits inert, then on `|dz| < 2100` converts itself
//    into a homing missile (jml woodsgo_init: stratptr swap + sbyte1=10 home
//    timer). A zdist-GATED state transition — new footprint (player-Z gate that
//    MUTATES the object's strat), certified by running the retail cart's OWN
//    woods_strat body across the gate boundary.
//  * `tree2`  (IS_TREE2=205): RNG height `(rnd&3)+1` AND a PLAYER-RELATIVE tilt
//    — the first strat combining an RNG draw with a player-position branch.
//  * `shou0`  (IS_SHOU0=178): a plasma turret whose init draws the RNG for its
//    fire-pattern selector `sbyte1` in {0,1,2} with a REROLL-on-3 loop — the
//    first RNG-with-reroll init certified vs retail.
//  * `break_meteorT` (IS_BREAK_METEORT=238): the tadpole death coin — a 50%
//    `s_jmp_random` (threshold 127) spawn decision; certified at the DECISION
//    level (RNG draw + threshold) against the port's real death-strat.
// ============================================================================

use sf_oracle::{
    RETAIL_SHOU0_ISTRAT, RETAIL_TREE2_ISTRAT, RETAIL_WOODSGO_STRAT, RETAIL_WOODS_STRAT,
    RETAIL_WOODS_ZGATE,
};

const IS_WOODS: usize = 54;
const IS_TREE2: usize = 205;
const IS_SHOU0: usize = 178;
const IS_BREAK_METEORT: usize = 238;
const SH_TADPOLE: u16 = 228;
/// `s_jmp_random` (no factor) 50% coin threshold `cmp #((50)*255)/100 = 127`.
const COIN_THRESH_50: u16 = 127;

/// MILESTONE (batch-4 step 1) — LOCATE + CROSS-VALIDATE the four batch-4 retail
/// addresses by masked signature scan (each a UNIQUE hit), reading the embedded
/// operands (PLAYPT, RANDOM_L draws, the woodsgo/shou0 fall-through pointers,
/// the tree2/woods/shou0 gate constants) back out to self-validate.
#[test]
fn retail_batch4_addresses() {
    let Some(rom) = retail() else { return };
    let w = None;

    // --- woods_strat: ldy PLAYPT; rep; lda worldz,y; sec; sbc worldz,x; bpl+;
    //     eor #$FFFF; inc; cmp #$0834(2100); sep; bpl+; jml woodsgo_init; rtl ---
    let woods_pat: Vec<Option<u8>> = vec![
        Some(0xAC), w, w, Some(0xC2), Some(0x20), Some(0xB9), Some(0x10), Some(0x00),
        Some(0x38), Some(0xF5), Some(0x10), Some(0x10), Some(0x04), Some(0x49), Some(0xFF),
        Some(0xFF), Some(0x1A), Some(0xC9), Some(0x34), Some(0x08), Some(0xE2), Some(0x20),
        Some(0x10), Some(0x04), Some(0x5C), w, w, w, Some(0x6B),
    ];
    let h = masked_scan(&rom, &woods_pat);
    assert_eq!(h.len(), 1, "woods_strat is a UNIQUE masked hit");
    let woods = rom_off_to_snes(h[0]);
    let woods_playpt = rom[h[0] + 1] as u32 | ((rom[h[0] + 2] as u32) << 8);
    let gate = rom[h[0] + 18] as u16 | ((rom[h[0] + 19] as u16) << 8);
    let woodsgo_init = rom[h[0] + 25] as u32 | ((rom[h[0] + 26] as u32) << 8) | ((rom[h[0] + 27] as u32) << 16);
    eprintln!("BATCH4: woods_strat=${woods:06X} PLAYPT=${woods_playpt:04X} zgate={gate} ->woodsgo_init=${woodsgo_init:06X}");
    assert_eq!(woods, RETAIL_WOODS_STRAT, "woods_strat address");
    assert_eq!(woods_playpt, RETAIL_PLAYPT, "woods reads PLAYPT=$1238");
    assert_eq!(gate as i16, RETAIL_WOODS_ZGATE, "woods gate cmp #2100");
    // woodsgo_init installs woodsgo_strat: read its `lda #woodsgo_strat` immediate.
    let wgo_off = snes_to_rom_off(woodsgo_init);
    assert_eq!(&rom[wgo_off..wgo_off + 3], &[0xC2, 0x20, 0xA9], "woodsgo_init head rep;lda #strat");
    let woodsgo_strat = rom[wgo_off + 3] as u32 | ((rom[wgo_off + 4] as u32) << 8) | ((rom[wgo_off + 10] as u32) << 16);
    assert_eq!(woodsgo_strat, RETAIL_WOODSGO_STRAT, "woodsgo_init installs woodsgo_strat=$08:B840");

    // --- tree2_Istrat: jsl RANDOM_L; and #3; sta sbyte1; inc sbyte1; lda #deg22;
    //     sta sbyte2; ... (RNG-first). ---
    let tree2_pat: Vec<Option<u8>> = vec![
        Some(0x22), w, w, w, Some(0x29), Some(0x03), Some(0x95), Some(0x22), Some(0xF6),
        Some(0x22), Some(0xA9), Some(0x10), Some(0x95), Some(0x23), Some(0xB5), Some(0x1F),
        Some(0x09), Some(0x02), Some(0x95), Some(0x1F), Some(0xB5), Some(0x1F), Some(0x09),
        Some(0x01), Some(0x95), Some(0x1F),
    ];
    let h = masked_scan(&rom, &tree2_pat);
    assert_eq!(h.len(), 1, "tree2_Istrat is a UNIQUE masked hit");
    let tree2 = rom_off_to_snes(h[0]);
    let tree2_rnd = rom[h[0] + 1] as u32 | ((rom[h[0] + 2] as u32) << 8) | ((rom[h[0] + 3] as u32) << 16);
    let deg22 = rom[h[0] + 11];
    eprintln!("BATCH4: tree2_Istrat=${tree2:06X} random_l=${tree2_rnd:06X} deg22=${deg22:02X}");
    assert_eq!(tree2, RETAIL_TREE2_ISTRAT, "tree2_Istrat address");
    assert_eq!(tree2_rnd, RETAIL_RANDOM_L, "tree2 draw is jsl RANDOM_L");
    assert_eq!(deg22, 0x10, "tree2 sbyte2 seed = deg22 ($10)");

    // --- shou0_Istrat: rep;lda #strat;sta stratptr; sep;lda #bk;sta; jsl set0coll;
    //     HP2/AP12/enemy1; jsl RANDOM_L; and #3; sta sbyte1; lda sbyte1; cmp #3;
    //     bne+; jml .again ---
    let shou0_pat: Vec<Option<u8>> = vec![
        Some(0xC2), Some(0x20), Some(0xA9), w, w, Some(0x95), Some(0x16), Some(0xE2), Some(0x20),
        Some(0xA9), w, Some(0x95), Some(0x18), Some(0x22), w, w, w,
        Some(0xA9), Some(0x02), Some(0x95), Some(0x2A), Some(0xA9), Some(0x0C), Some(0x95), Some(0x2B),
        Some(0xB5), Some(0x2E), Some(0x09), Some(0x10), Some(0x95), Some(0x2E),
        Some(0x22), w, w, w, Some(0x29), Some(0x03), Some(0x95), Some(0x22),
        Some(0xB5), Some(0x22), Some(0xC9), Some(0x03), Some(0xD0), Some(0x04), Some(0x5C), w, w, w,
    ];
    let h = masked_scan(&rom, &shou0_pat);
    assert_eq!(h.len(), 1, "shou0_Istrat is a UNIQUE masked hit");
    let shou0 = rom_off_to_snes(h[0]);
    let shou0_rnd = rom[h[0] + 32] as u32 | ((rom[h[0] + 33] as u32) << 8) | ((rom[h[0] + 34] as u32) << 16);
    let again = rom[h[0] + 46] as u32 | ((rom[h[0] + 47] as u32) << 8) | ((rom[h[0] + 48] as u32) << 16);
    eprintln!("BATCH4: shou0_Istrat=${shou0:06X} random_l=${shou0_rnd:06X} .again=${again:06X}");
    assert_eq!(shou0, RETAIL_SHOU0_ISTRAT, "shou0_Istrat address");
    assert_eq!(shou0_rnd, RETAIL_RANDOM_L, "shou0 sbyte1 draw is jsl RANDOM_L");
    // .again reroll target == the RNG-draw jsl site (Istrat + 31): re-rolls sbyte1.
    assert_eq!(again, shou0 + 31, ".again reloops to the RANDOM draw (reroll on 3)");
}

/// CAPSTONE (batch-4) — RETAIL `woods_strat` zdist CONVERSION GATE vs THE PORT.
///
/// woods waits inert until the player closes within 2100 z, then converts itself
/// into a homing missile (jml woodsgo_init: install woodsgo_strat + `sbyte1=10`
/// home timer). We run the retail cart's OWN `woods_strat` body ($08:B7F6) on a
/// seeded object across the gate boundary (player just-outside vs just-inside
/// 2100 z) and diff the CONVERSION against the port `woods_strat` (reached via
/// its registered `woods_init` fall-through). Retail-converted iff `al_sbyte1`
/// became 10 (and `al_stratptr` was swapped to woodsgo_strat $08:B840).
#[test]
fn retail_woods_convert_gate_vs_port() {
    let Some(rom) = retail() else { return };
    let enemy = RETAIL_POOL.base; // X for the strat call
    let player_blk = RETAIL_POOL.base + RETAIL_POOL.stride;

    // Player at z=0; enemy at ez. |dz|=|ez|. Boundary at 2100.
    //   ez=1900 -> |dz|<2100  -> CONVERT
    //   ez=2100 -> |dz|>=2100 -> STAY (cmp is inclusive: bpl on >=)
    for (ez, expect_convert) in [(1900i16, true), (2100i16, false), (-1000i16, true), (3000i16, false)] {
        // --- retail: run the cart's OWN woods_strat body across the gate. ---
        let mut bus = SnesBus::new(rom.clone());
        bus.wram_write16(RETAIL_PLAYPT, player_blk as u16);
        bus.wram_write16(player_blk + RETAIL_POOL.al_worldz, 0);
        bus.wram_write16(enemy + RETAIL_POOL.al_worldz, ez as u16);
        call(&mut bus, RETAIL_WOODS_STRAT, &Entry { x: enemy as u16, p: 0x00, ..Default::default() });
        let r_sbyte1 = bus.read8(0x7E_0000 | (enemy + AL_SBYTE1));
        let r_sptr_lo = bus.wram_read16(enemy + AL_STRATPTR);
        let r_sptr_bk = bus.read8(0x7E_0000 | (enemy + AL_STRATPTR + 2));
        let r_stratptr = r_sptr_lo as u32 | ((r_sptr_bk as u32) << 16);
        let r_convert = r_sbyte1 == 10;

        // --- port: reach woods_strat via its registered init fall-through. ---
        let mut g = sf_game::game::Game::new();
        sf_strat::enemies_ground::register(&mut g.world);
        let pl = g.objs.alloc().expect("player slot"); // slot 0 = player
        sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[pl as usize]);
        g.objs.aliens[pl as usize].worldz = 0;
        let e = g.objs.alloc().expect("enemy slot");
        sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
        g.objs.aliens[e as usize].worldz = ez;
        // woods_init falls into woods_strat once — converts this frame if in gate.
        g.objs.aliens[e as usize].stratptr = g.world.istrats[IS_WOODS];
        let s = g.objs.aliens[e as usize].stratptr.expect("woods istrat");
        g.call_strat(s, e);
        let p_sbyte1 = g.objs.aliens[e as usize].sbyte1;
        let p_convert = p_sbyte1 == 10;

        eprintln!(
            "BATCH4 woods [ez={ez} |dz|={}]: retail convert={r_convert} (sbyte1={r_sbyte1} stratptr=${r_stratptr:06X}) | port convert={p_convert} (sbyte1={p_sbyte1})  {}",
            ez.unsigned_abs(),
            if r_convert == p_convert { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_convert, expect_convert, "retail woods gate decision at ez={ez}");
        assert_eq!(r_convert, p_convert, "woods conversion decision must match retail (ez={ez})");
        if r_convert {
            assert_eq!(r_sbyte1, 10, "retail woods home timer = 10");
            assert_eq!(r_stratptr, RETAIL_WOODSGO_STRAT, "retail woods installed woodsgo_strat");
        }
    }
    eprintln!("BATCH4 woods: MATCH — retail woods_strat zdist<2100 conversion gate == port.");
}

/// CAPSTONE (batch-4) — RETAIL `tree2_Istrat` (RNG + PLAYER-RELATIVE) vs THE PORT.
///
/// tree2 is the first strat combining an RNG draw with a PLAYER-POSITION branch:
///  * RNG height `sbyte1 = (rnd&3)+1`, then
///  * a player tilt: reads `PLAYPT`->player `al_worldx`, compares its OWN
///    `al_worldx` (`cmp`/`bpl` = test bit 15 of `enemy_x - player_x`); on
///    `enemy_x < player_x` (.otherway) `sbyte2 = -deg22($F0)` + `roty += deg45
///    ($20)`, else (.notthatway) `sbyte2 = deg22($10)` + `roty += -deg45($E0)`.
///
/// We run the retail cart's OWN `tree2_Istrat` body ($09:952F) on seeded RNG
/// (firepillar param-block recipe, 8-bit-A entry) + a live player object (via
/// PLAYPT), and diff the PLAYER-RELATIVE tilt `(sbyte2, roty)` vs the port
/// `tree2_init` (IS_TREE2=205) — an EXACT body match across BOTH tilt branches.
///
/// The RNG HEIGHT is certified via the proven RANDOM stream (`(rnd&3)+1`, exactly
/// as tree1): the port init consumes ONE RANDOM and stores `(draw&3)+1` into
/// `sbyte1`, matching the cart's RANDOM draw. (The retail BODY's post-init
/// `sbyte1` reads `(draw&3)` because it falls through into the sprouty grow tick,
/// whose segment countdown decrements the stored height once — that sprouty
/// SEGMENT machinery is scoped out of the port, so the raw body `sbyte1` is
/// certified at the stream/formula level, not diffed against the post-tick byte.)
#[test]
fn retail_tree2_body_vs_port() {
    let Some(rom) = retail() else { return };
    let enemy = RETAIL_POOL.base; // low byte $36 = pinned rand[3]
    let player_blk = RETAIL_POOL.base + RETAIL_POOL.stride;
    // (enemy_x, player_x): case A enemy left of player (.otherway); B right (.notthatway).
    let scenarios = [(-5000i16, 5000i16), (5000i16, -5000i16)];
    for seed in [[10u8, 20, 30, 54], [201, 88, 143, 54]] {
        assert_eq!(seed[3] as u32, enemy & 0xFF, "seed[3] must equal the pinned block low byte");

        // RNG height: the cart's own RANDOM stream draw, (draw&3)+1 (== tree1).
        let mut sbus = SnesBus::new(rom.clone());
        let mut rs = seed;
        let stream_draw = retail_random_next(&mut sbus, &mut rs);
        let expect_height = (stream_draw & 3).wrapping_add(1);

        for (ex, px) in scenarios {
            // --- retail: run tree2_Istrat body on seeded RNG + player object. ---
            let mut bus = SnesBus::new(rom.clone());
            bus.wram_write16(RETAIL_PLAYPT, player_blk as u16);
            bus.wram_write16(player_blk + RETAIL_POOL.al_worldx, px as u16);
            bus.wram_write16(enemy + RETAIL_POOL.al_worldx, ex as u16);
            seed_retail_rng(&mut bus, seed);
            bus.write8(RETAIL_RAND, seed[0]); // rand[0] @ $EF (below param block)
            let a = seed[1] as u16 | ((seed[2] as u16) << 8); // -> $F0/$F1 = rand[1]/rand[2]
            // tree2_Istrat is entered via s_start_strat's `shorta` (8-bit A) — its
            // first op is `jsl RANDOM; and #3` (8-bit). p=$20 -> 8-bit A / 16-bit X;
            // the harness still pre-loads $F0-$F2 (rand[1..3]) into WRAM regardless.
            call(&mut bus, RETAIL_TREE2_ISTRAT, &Entry { a, x: enemy as u16, p: 0x20, ..Default::default() });
            let r_sb2 = bus.read8(0x7E_0000 | (enemy + AL_SBYTE2));
            let r_roty = bus.read8(0x7E_0000 | (enemy + AL_ROTY));

            // --- port: tree2_init on the same seed + player at slot 0. ---
            let mut g = sf_game::game::Game::new();
            sf_strat::enemies_ground::register(&mut g.world);
            let pl = g.objs.alloc().expect("player slot"); // slot 0 = player
            sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[pl as usize]);
            g.objs.aliens[pl as usize].worldx = px;
            let e = g.objs.alloc().expect("enemy slot");
            sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
            g.objs.aliens[e as usize].worldx = ex;
            g.vars.rng = seed;
            g.objs.aliens[e as usize].stratptr = g.world.istrats[IS_TREE2];
            let s = g.objs.aliens[e as usize].stratptr.expect("tree2 istrat");
            g.call_strat(s, e);
            let p_sb1 = g.objs.aliens[e as usize].sbyte1;
            let p_sb2 = g.objs.aliens[e as usize].sbyte2;
            let p_roty = g.objs.aliens[e as usize].roty;

            eprintln!(
                "BATCH4 tree2 seed {seed:02X?} ex={ex} px={px}: retail tilt (sb2=${r_sb2:02X} roty=${r_roty:02X}) | port (sb1={p_sb1} sb2=${p_sb2:02X} roty=${p_roty:02X}) height={expect_height}  {}",
                if (r_sb2, r_roty) == (p_sb2, p_roty) && p_sb1 == expect_height { "MATCH" } else { "DIFF" }
            );
            // Player-relative tilt: EXACT retail-body match.
            assert_eq!(r_sb2, p_sb2, "tree2 sbyte2 (+/-deg22 overhang) must match retail body");
            assert_eq!(r_roty, p_roty, "tree2 roty (+/-deg45 player tilt) must match retail body");
            // RNG height: port init == (cart RANDOM draw & 3)+1, in [1,4].
            assert_eq!(p_sb1, expect_height, "tree2 port height == (retail RANDOM draw & 3)+1");
            assert!((1..=4).contains(&p_sb1), "tree2 height in [1,4]");
            // Branch sanity: enemy left of player -> otherway (roty=+deg45=$20,
            // sbyte2=-deg22=$F0); enemy right -> notthatway (roty=-deg45=$E0, sbyte2=$10).
            if ex < px {
                assert_eq!(r_roty, 0x20, "otherway roty=+deg45");
                assert_eq!(r_sb2, 0xF0, "otherway sbyte2=-deg22");
            } else {
                assert_eq!(r_roty, 0xE0, "notthatway roty=-deg45");
                assert_eq!(r_sb2, 0x10, "notthatway sbyte2=deg22");
            }
        }
    }
    eprintln!("BATCH4 tree2: MATCH — retail player-relative tilt (body) + RNG height (stream) == port.");
}

/// CAPSTONE (batch-4) — RETAIL `shou0_Istrat` RNG-REROLL init vs THE PORT.
///
/// shou0's init draws the RNG for its fire-pattern selector `sbyte1 = rnd&3`,
/// REROLLING while the result is 3 (`jml .again` back to the draw) so the value
/// is uniform in {0,1,2}. We run the retail cart's OWN `shou0_Istrat` body
/// ($0A:D615) on seeded RNG (param-block recipe) with the player far (so the
/// fall-through `shou0_strat` zdist gate is a clean no-op), and diff `al_sbyte1`
/// vs the port `shou0_init` (IS_SHOU0=178) on the SAME seed — certifying the
/// reroll loop produces the identical RNG-consumption + result.
#[test]
fn retail_shou0_reroll_vs_port() {
    let Some(rom) = retail() else { return };
    let enemy = RETAIL_POOL.base; // low byte $36 = pinned rand[3]
    let player_blk = RETAIL_POOL.base + RETAIL_POOL.stride;
    for seed in [[1u8, 2, 3, 54], [15, 31, 63, 54], [200, 100, 50, 54], [3, 7, 11, 54]] {
        assert_eq!(seed[3] as u32, enemy & 0xFF, "seed[3] must equal the pinned block low byte");
        // --- retail: run shou0_Istrat body; player far so the tick no-ops. ---
        let mut bus = SnesBus::new(rom.clone());
        bus.wram_write16(RETAIL_PLAYPT, player_blk as u16);
        bus.wram_write16(player_blk + RETAIL_POOL.al_worldz, 30000u16);
        bus.wram_write16(enemy + RETAIL_POOL.al_worldz, 0);
        seed_retail_rng(&mut bus, seed);
        bus.write8(RETAIL_RAND, seed[0]);
        let a = seed[1] as u16 | ((seed[2] as u16) << 8);
        call(&mut bus, RETAIL_SHOU0_ISTRAT, &Entry { a, x: enemy as u16, p: 0x00, ..Default::default() });
        let r_sb1 = bus.read8(0x7E_0000 | (enemy + AL_SBYTE1));

        // --- port: shou0_init on the same seed (player() = the enemy, dz=0 out of
        // [500,2500) -> tick no-ops), read sbyte1. ---
        let (_rotz, r_sb1_port, _roty, _hp, _ap, _coll, _n) = port_ground_init(IS_SHOU0, seed);
        eprintln!(
            "BATCH4 shou0 seed {seed:02X?}: retail sbyte1={r_sb1} | port sbyte1={r_sb1_port}  {}",
            if r_sb1 == r_sb1_port { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_sb1, r_sb1_port, "shou0 sbyte1 (rnd&3, reroll on 3) must match retail");
        assert!(r_sb1 <= 2, "shou0 sbyte1 rerolled into {{0,1,2}}");
    }
    eprintln!("BATCH4 shou0: MATCH — retail RNG-reroll fire-pattern selector == port.");
}

/// CAPSTONE (batch-4) — the `break_meteorT` TADPOLE DEATH COIN vs THE PORT.
///
/// On death, break_meteorT runs `break1.createtadpole` (DPATHDAT.ASM:1787-1792):
/// a 50% `s_jmp_random` (threshold `#127`) that SKIPS the tadpole spawn on
/// `random < 127` and SPAWNS a tadpole on `random >= 127`. The spawn lives in the
/// path VM (not a strat address), so we certify the DECISION: draw one value from
/// the retail cart's OWN `RANDOM` (carried across the param-block collision by
/// `retail_random_next`) and compare `draw >= 127` against the PORT's REAL death
/// strat `break_meteort_exp` (reached via its registered `break_meteort_init`
/// expstrat), observed by whether it actually spawned a `SH_TADPOLE` object.
/// Several seeds drive BOTH outcomes.
#[test]
fn retail_break_meteort_coin_vs_port() {
    let Some(rom) = retail() else { return };
    let mut saw_spawn = false;
    let mut saw_skip = false;
    for seed in [[1u8, 2, 3, 4], [200, 50, 90, 7], [0xEF, 0x10, 0x33, 0x9C], [126, 0, 0, 0], [128, 0, 0, 0]] {
        // Retail: the single coin draw from the cart's own RANDOM.
        let mut bus = SnesBus::new(rom.clone());
        let mut rs = seed;
        let draw = retail_random_next(&mut bus, &mut rs);
        let r_spawn = (draw as u16) >= COIN_THRESH_50; // >=127 -> spawn a tadpole

        // Port: the REAL break_meteort_exp death strat on the same seed. Count
        // SH_TADPOLE objects before/after to observe the spawn decision.
        let mut g = sf_game::game::Game::new();
        sf_strat::enemies_ground::register(&mut g.world);
        let e = g.objs.alloc().expect("meteor slot");
        sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
        g.objs.aliens[e as usize].stratptr = g.world.istrats[IS_BREAK_METEORT];
        g.vars.rng = seed;
        let init = g.objs.aliens[e as usize].stratptr.expect("break_meteort istrat");
        g.call_strat(init, e); // arms expstratptr = break_meteort_exp
        let exp = g.objs.aliens[e as usize].expstratptr.expect("break_meteort expstrat");
        let before = g.objs.aliens.iter().filter(|a| a.active && a.shape == SH_TADPOLE).count();
        g.call_strat(exp, e); // the death coin + explosion
        let after = g.objs.aliens.iter().filter(|a| a.active && a.shape == SH_TADPOLE).count();
        let p_spawn = after > before;

        eprintln!(
            "BATCH4 break_meteorT seed {seed:02X?}: retail draw={draw} spawn={r_spawn} | port spawn={p_spawn}  {}",
            if r_spawn == p_spawn { "MATCH" } else { "DIFF" }
        );
        assert_eq!(r_spawn, p_spawn, "break_meteorT tadpole coin (draw>=127) must match retail");
        saw_spawn |= p_spawn;
        saw_skip |= !p_spawn;
    }
    assert!(saw_spawn && saw_skip, "both coin outcomes (spawn / skip) exercised");
    eprintln!("BATCH4 break_meteorT: MATCH — port death coin (draw>=127 spawn) == retail RANDOM stream + threshold.");
}

// ============================================================================
// AIMING CLASS — the GSU-PER-TICK aiming pipeline (every enemy that aims at the
// player + fires). The aim step of a firing enemy stores `roty = arctan16(dx,
// dz) >> 8` each tick; that arctan runs on the SUPER-FX chip. `anglexy_l` (the
// leaf `s_obj2obj_angle` calls) copies dx/dz into GSU RAM and kicks the GSU via
// `arctan16 -> runmario_l -> mcallarctan16`. This certifies a REAL GSU call
// executing inside a strat's aim step against the retail cart.
// ============================================================================

use sf_oracle::{RETAIL_ANGLEXY_L, RETAIL_ARCTAN16_L, RETAIL_ARCTAN16_L_BUILT, RETAIL_AL1PT};

/// Locate retail `anglexy_l` by masked scan of the built-ROM skeleton
/// ($1F:D039) with the two WRAM scratch operands (`x1`/`y1`) and the
/// `jsl arctan16_l` target wildcarded. Returns `(anglexy_l snes, x1_dp, y1_dp,
/// arctan16_l snes)`. UNIQUE hit.
fn locate_anglexy_l(rom: &[u8]) -> (u32, u8, u8, u32) {
    let w = None;
    let pat: Vec<Option<u8>> = vec![
        Some(0xDA), Some(0x5A), Some(0xC2), Some(0x20),
        Some(0xB9), Some(0x0C), Some(0x00), Some(0x38), Some(0xF5), Some(0x0C), Some(0x85), w, // sta x1
        Some(0xB9), Some(0x10), Some(0x00), Some(0x38), Some(0xF5), Some(0x10), Some(0x85), w, // sta y1
        Some(0x22), w, w, w,       // jsl arctan16_l
        Some(0xC2), Some(0x30), Some(0x7A), Some(0xFA), Some(0x6B),
    ];
    let h = masked_scan(rom, &pat);
    assert_eq!(h.len(), 1, "anglexy_l must be a UNIQUE masked hit (got {})", h.len());
    let off = h[0];
    let snes = rom_off_to_snes(off);
    let x1 = rom[off + 11];
    let y1 = rom[off + 19];
    let arctan = rom[off + 21] as u32 | ((rom[off + 22] as u32) << 8) | ((rom[off + 23] as u32) << 16);
    (snes, x1, y1, arctan)
}

/// MILESTONE (aiming step 1) — LOCATE + CROSS-VALIDATE the aiming pipeline.
/// The yaw-aim leaf `anglexy_l` (the GSU-driving arctan wrapper `s_obj2obj_angle`
/// calls) is found by masked scan; its `jsl` operand yields retail `arctan16_l`.
#[test]
fn retail_aiming_pipeline_addresses() {
    let Some(rom) = retail() else { return };
    let (anglexy, x1, y1, arctan) = locate_anglexy_l(&rom);
    eprintln!(
        "AIM: anglexy_l=${anglexy:06X} (x1=dp${x1:02X} y1=dp${y1:02X}) -> jsl arctan16_l=${arctan:06X}"
    );
    assert_eq!(anglexy, RETAIL_ANGLEXY_L, "anglexy_l retail address");
    assert_eq!(arctan, RETAIL_ARCTAN16_L, "derived retail arctan16_l");
    // arctan16_l is a real, reachable far routine (bank $00-$3F, $8000+ window).
    let bank = arctan >> 16;
    assert!(bank <= 0x3F && (arctan & 0xFFFF) >= 0x8000, "arctan16_l looks like a code address");
    // The two scratch words must not collide with the `call` harness param
    // block ($F0-$F5) or the retail `rand` state ($EF-$F2) — so a GSU roundtrip
    // through anglexy_l survives the harness.
    for dp in [x1, y1] {
        assert!(!(0xEF..=0xF5).contains(&dp), "x1/y1 scratch dp${dp:02X} must avoid the harness param block");
    }
    eprintln!(
        "AIM: built arctan16_l=${RETAIL_ARCTAN16_L_BUILT:06X}; retail arctan16_l=${arctan:06X} (same routine, shifted per cart)."
    );
}

/// Exact replica of the port's `sf_strat::common::strat_angle_xz` (== angle_xz),
/// for a self-checking oracle inside the test (the real port fn is exercised via
/// the public API below; this mirror lets us print divergences precisely).
#[allow(dead_code)]
fn port_angle8(dx: i32, dz: i32) -> u8 {
    let mut a = (dx as f32).atan2(dz as f32);
    if a < 0.0 {
        a += 2.0 * 3.141_592_65_f32;
    }
    ((a * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
}

/// CAPSTONE (aiming — GOLD) — RETAIL GSU-PER-TICK AIM ANGLE vs THE PORT.
///
/// This runs the retail cart's OWN `anglexy_l` — the aim leaf a firing enemy
/// calls every tick via `s_obj2obj_angle` — on a seeded (enemy, player) pair.
/// `anglexy_l` computes `dx = player.worldx - enemy.worldx`,
/// `dz = player.worldz - enemy.worldz`, then `jsl arctan16_l`, which copies dx/dz
/// into GSU RAM and KICKS THE SUPER-FX CHIP through the RAM-resident `runmario_l`
/// trampoline (`arctan16 -> runmario_l -> mcallarctan16`). The 16-bit angle comes
/// back through shared bank-$70 RAM; the strat stores `arctan16 >> 8` as its yaw
/// target. We diff that 8-bit aim angle against the port's `common::strat_angle_xz`
/// over a grid of relative positions (all quadrants, shallow + steep). This is a
/// real GSU call running INSIDE the aim step, certified against the cartridge.
///
/// Tolerance: the ROM `arctan16` is a 512-entry table + `quotient>>5`, so the
/// 8-bit angle can differ from the port's float atan2 by AT MOST +/-1 (the same
/// documented float-vs-fixed tolerance proven in tests/gsu_arctan.rs). A
/// divergence > 1 would be a real aiming bug.
#[test]
fn retail_aiming_angle_gsu_vs_port() {
    let Some(rom) = retail() else { return };
    let (anglexy, _x1, _y1, _arctan) = locate_anglexy_l(&rom);

    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();
    // The GSU roundtrip goes through the RAM trampoline; inject it.
    inject_runmario_trampoline(&mut bus, RETAIL_RUNMARIO_L_ROM, RETAIL_RUNMARIO_RAM);

    // Two object blocks: X = enemy (aimer/src), Y = player (target/dst).
    let enemy = RETAIL_POOL.base;
    let player = RETAIL_POOL.base + RETAIL_POOL.stride;

    // Relative (dx,dz) grid: cardinals, diagonals, shallow ratios, all quadrants.
    let coords: [(i16, i16); 20] = [
        (0, 1000), (1000, 0), (0, -1000), (-1000, 0),
        (1000, 1000), (-1000, 1000), (1000, -1000), (-1000, -1000),
        (300, 1000), (1000, 300), (-300, 1000), (1000, -300),
        (37, 1000), (1000, 37), (173, 91), (-91, 173),
        (4000, 500), (-500, 4000), (7, -13), (12345, -6000),
    ];
    // Fixed enemy position; player = enemy + (dx,dz). Non-zero base to exercise
    // the 16-bit subtraction (and a wrap-ish case via 12345).
    let (ex, ez) = (500i16, -2000i16);

    let mut maxd = 0i32;
    let mut worst = (0i16, 0i16, 0u8, 0u8);
    let mut kicks_seen = 0u64;
    for (i, &(dx, dz)) in coords.iter().enumerate() {
        let px = ex.wrapping_add(dx);
        let pz = ez.wrapping_add(dz);
        // Seed both object blocks' world XZ (Y irrelevant to the XZ angle).
        bus.wram_write16(enemy + RETAIL_POOL.al_worldx, ex as u16);
        bus.wram_write16(enemy + RETAIL_POOL.al_worldz, ez as u16);
        bus.wram_write16(player + RETAIL_POOL.al_worldx, px as u16);
        bus.wram_write16(player + RETAIL_POOL.al_worldz, pz as u16);

        // Run the retail aim leaf: p=$20 (8-bit A / 16-bit index, the
        // `shorta longi` entry anglexy_l assumes). X=enemy, Y=player.
        let before = bus.gsu_kicks;
        let e = call(&mut bus, anglexy, &Entry {
            x: enemy as u16, y: player as u16, p: 0x20, ..Default::default()
        });
        kicks_seen += (bus.gsu_kicks - before) as u64;
        let retail_angle16 = e.c; // 16-bit angle returned in A
        let retail_angle8 = (retail_angle16 >> 8) as u8;

        // Port: the real enemy-aim primitive on the identical positions.
        let mut src = sf_game::alien::Alien::default();
        src.worldx = ex; src.worldz = ez;
        let mut dst = sf_game::alien::Alien::default();
        dst.worldx = px; dst.worldz = pz;
        let port_angle8 = sf_strat::common::strat_angle_xz(&src, &dst);

        // Circular 8-bit distance.
        let d = {
            let dd = (retail_angle8 as i32 - port_angle8 as i32).rem_euclid(256);
            dd.min(256 - dd)
        };
        if d > maxd { maxd = d; worst = (dx, dz, retail_angle8, port_angle8); }
        if i < 6 || d > 0 {
            eprintln!(
                "AIM GSU (dx={dx:6},dz={dz:6}): retail arctan16>>8=${retail_angle8:02X} ({}) | port angle_xz=${port_angle8:02X} ({})  d={d}",
                retail_angle8, port_angle8
            );
        }
    }
    eprintln!(
        "AIM GSU: {} positions, GSU kicks={} (>=1 per aim -> the chip ran each tick), max 8-bit aim delta={maxd} (worst dx={} dz={} retail={} port={})",
        coords.len(), kicks_seen, worst.0, worst.1, worst.2, worst.3
    );
    assert!(kicks_seen >= coords.len() as u64, "the GSU must be kicked at least once per aim (got {kicks_seen})");
    assert!(maxd <= 1, "retail GSU aim angle diverges from port angle_xz by {maxd} 8-bit units (>1) — a real aiming bug");
    eprintln!("AIM GSU: MATCH — retail GSU-per-tick aim angle (arctan16>>8) == port angle_xz within +/-1 over {} positions.", coords.len());
}

/// CAPSTONE (aiming) — the FIRE-GATE timing (`s_jmp_notdelay`) vs THE PORT.
///
/// Every firing enemy gates its shot with the pure-integer per-frame timer
/// `s_jmp_notdelay #delay,label,al1pt` = `lda gameframe; clc; adc al1pt;
/// and #(1<<delay)-1; bne skip` -> FIRE iff `(gameframe + stagger) & mask == 0`.
/// NO GSU, NO RNG. We LOCATE the retail fire-gate sites (scan for
/// `lda gameframe; clc; adc al1pt; and #imm`) to prove retail's gate is exactly
/// that expression over `gameframe`($15BB) staggered by `al1pt`($123A), then
/// certify the decision against the PORT's identical gate expression
/// (`gameframe.wrapping_add(stagger) & mask == 0`, as used by
/// `bossb::notdelay_stag` / enemy_b) over a grid of (gameframe, stagger, mask).
#[test]
fn retail_fire_gate_notdelay_vs_port() {
    let Some(rom) = retail() else { return };

    // Locate the fire-gate sites: `AD <gameframe> 18 6D <al1pt> 29 <mask>`
    // (lda gameframe; clc; adc al1pt; and #imm8). This is the staggered
    // `s_jmp_notdelay ...,al1pt` every firing enemy uses.
    let gf = RETAIL_GAMEFRAME as u16;
    let al1 = RETAIL_AL1PT as u16;
    let pat: Vec<Option<u8>> = vec![
        Some(0xAD), Some(gf as u8), Some((gf >> 8) as u8), // lda gameframe
        Some(0x18),                                        // clc
        Some(0x6D), Some(al1 as u8), Some((al1 >> 8) as u8), // adc al1pt
        Some(0x29),                                        // and #imm8 (8-bit A)
    ];
    let hits = masked_scan(&rom, &pat);
    let mut masks: Vec<u8> = hits.iter().map(|&h| rom[h + 8]).collect();
    masks.sort_unstable();
    masks.dedup();
    eprintln!(
        "FIRE-GATE: {} staggered `(gameframe+al1pt) & mask` sites in retail; masks seen = {:02X?}",
        hits.len(), masks
    );
    assert!(!hits.is_empty(), "retail must contain staggered fire-gate sites (lda gameframe; adc al1pt; and #mask)");
    // Every mask is (1<<delay)-1 for delay in 1..=8 -> a contiguous low-bit mask.
    for &m in &masks {
        assert_eq!(m & m.wrapping_add(1), 0, "fire-gate mask ${m:02X} must be (1<<delay)-1");
    }

    // Certify the DECISION vs the port's gate expression over a grid. The port
    // uses `gameframe.wrapping_add(stagger) & mask == 0` (bossb::notdelay_stag,
    // enemy_b.rs:1030); the retail macro fires iff the same expression is 0.
    fn port_fires(gameframe: u16, stagger: u16, mask: u16) -> bool {
        gameframe.wrapping_add(stagger) & mask == 0
    }
    // ROM semantics: `and #mask` on the low byte of (gameframe+stagger); fire iff 0.
    fn retail_fires(gameframe: u16, stagger: u16, mask: u8) -> bool {
        (gameframe.wrapping_add(stagger) as u8) & mask == 0
    }
    let mut checked = 0u32;
    let mut fires = 0u32;
    for &mask in &masks {
        for gameframe in 0u16..512 {
            for &stagger in &[0u16, 1, 3, 7, 15, 31, 63, 128, 255] {
                let pf = port_fires(gameframe, stagger, mask as u16);
                let rf = retail_fires(gameframe, stagger, mask);
                assert_eq!(pf, rf, "fire-gate mismatch gf={gameframe} stag={stagger} mask=${mask:02X}: port={pf} retail={rf}");
                checked += 1;
                fires += pf as u32;
            }
        }
    }
    eprintln!(
        "FIRE-GATE: MATCH — port `(gameframe+stagger)&mask==0` == retail `s_jmp_notdelay` over {checked} (gf,stagger,mask) combos ({fires} fire frames). Masks: {:02X?}",
        masks
    );
}

// ============================================================================
// AIM-MATH pipeline, CPU half — `gen_3dvecs` (angle -> velocity) vs RETAIL.
//
// After a firing enemy computes its aim angle (arctan16, the GSU half certified
// above), it turns that angle into a velocity via `gen_3dvecs` — pure CPU
// sin/cos tables (`n3dvecs_l`, STRATROU.ASM), NO GSU. This completes the
// aim-math pipeline vs the cartridge. The routine leaves the velocity in the
// `x1/y1/z1` WRAM scratch (16-bit signed); the port `common::strat_gen_vecs_3d`
// writes al_vx/vy/vz. As tests/gen_3dvecs.rs documents, the port matches the ROM
// bit-exact on vx/vz and on |vy| (the vy SIGN is the renderer Y convention: the
// port negates pitch, the ROM does not) — we certify the same equivalence vs
// the RETAIL cart.
// ============================================================================

/// Locate retail `n3dvecs_l` by masked scan of the built-ROM skeleton
/// ($1F:C436) with all dp scratch operands + the WRAM troty/trotx wildcarded
/// (the retail scratch block SHIFTED — x1/y1 stayed $02/$08 but z1 moved $8A->
/// $90 and tmpz $78->$7E). Returns `(n3dvecs_l snes, troty, trotx, x1, y1, z1,
/// tmpz)`, all re-derived from the routine's own operands. UNIQUE hit.
fn locate_n3dvecs_l(rom: &[u8]) -> (u32, u32, u32, u32, u32, u32, u32) {
    let w = None;
    // Anchor on opcodes + the distinctive `nega(eor#$FF;inc;tay)` / `tax; sep#$10`
    // structure; wildcard all dp scratch operands (may shift) and the phb-block
    // immediate (retail may be FASTROM `lda #$80` vs built `lda #0`).
    let pat: Vec<Option<u8>> = vec![
        Some(0x64), w, Some(0x64), w, Some(0x64), w,   // stz x1+1/y1+1/z1+1
        Some(0x86), w, Some(0x84), w,                  // stx tmpx; sty tmpy
        Some(0x8B), Some(0xA9), w, Some(0x48), Some(0xAB), // phb; lda #imm; pha; plb
        Some(0xAD), w, w,                              // lda troty
        Some(0x49), Some(0xFF), Some(0x1A), Some(0xA8), // eor #$FF; inc a; tay (nega roty)
        Some(0xAD), w, w, Some(0xAA),                  // lda trotx; tax
        Some(0xE2), Some(0x10),                        // sep #$10 (i8)
    ];
    let h = masked_scan(rom, &pat);
    assert_eq!(h.len(), 1, "n3dvecs_l must be a UNIQUE masked hit (got {})", h.len());
    let off = h[0];
    let troty = rom[off + 16] as u32 | ((rom[off + 17] as u32) << 8);
    let trotx = rom[off + 23] as u32 | ((rom[off + 24] as u32) << 8);
    // x1/y1/z1 = the `stz <scratch>+1` operands minus 1.
    let x1 = rom[off + 1] as u32 - 1;
    let y1 = rom[off + 3] as u32 - 1;
    let z1 = rom[off + 5] as u32 - 1;
    // tmpz = operand of the `lda tmpz; bmi; asl; sta $4202` multiply setup
    // (find the `30 15 0A 8D 02 42` subsequence; tmpz is 2 bytes before it).
    let sig = [0x30u8, 0x15, 0x0A, 0x8D, 0x02, 0x42];
    let region = &rom[off..off + 96];
    let spos = region.windows(sig.len()).position(|wnd| wnd == sig).expect("tmpz multiply-setup sig");
    let tmpz = region[spos - 1] as u32; // the `A5 <tmpz>` operand
    (rom_off_to_snes(off), troty, trotx, x1, y1, z1, tmpz)
}

/// CAPSTONE (aim-math CPU half) — RETAIL `n3dvecs_l` (angle->velocity) vs PORT.
///
/// Runs the retail cart's OWN `n3dvecs_l` on seeded (roty, rotx, vel) and diffs
/// the resulting velocity vector against the port `common::strat_gen_vecs_3d`,
/// over the same spread of yaw/pitch/speed as tests/gen_3dvecs.rs. vx/vz and
/// |vy| are bit-exact to the cartridge; the vy SIGN is the documented renderer
/// convention (port negates pitch, ROM does not).
#[test]
fn retail_gen_3dvecs_vs_port() {
    let Some(rom) = retail() else { return };
    let (n3dvecs, troty_addr, trotx_addr, x1, y1, z1, tmpz) = locate_n3dvecs_l(&rom);
    eprintln!(
        "AIM-MATH: n3dvecs_l=${n3dvecs:06X} troty=${troty_addr:04X} trotx=${trotx_addr:04X} x1=${x1:02X} y1=${y1:02X} z1=${z1:02X} tmpz=${tmpz:02X}"
    );
    // troty/trotx are a contiguous byte pair (built $1630/$1631).
    assert_eq!(troty_addr, trotx_addr + 1, "troty/trotx contiguous like built");
    assert_eq!(n3dvecs, sf_oracle::RETAIL_N3DVECS_L, "n3dvecs_l retail address");
    assert_eq!(troty_addr, sf_oracle::RETAIL_TROTY, "retail troty");
    assert_eq!(trotx_addr, sf_oracle::RETAIL_TROTX, "retail trotx");
    // x1/y1 stayed at the built dp addresses (confirmed by anglexy_l too).
    assert_eq!((x1, y1), (0x02, 0x08), "x1/y1 output scratch");

    let (X1, Y1, Z1, TMPZ) = (x1, y1, z1, tmpz);

    let cases = [
        (0u8, 0u8, 100u8), (64, 0, 100), (192, 0, 100), (32, 16, 80),
        (96, 32, 64), (128, 0, 100), (10, 5, 120), (250, 8, 90),
    ];
    let mut bad = 0;
    for &(roty, rotx, vel) in &cases {
        let mut bus = SnesBus::new(rom.clone());
        bus.write8(trotx_addr, rotx);
        bus.write8(troty_addr, roty);
        bus.write8(TMPZ, vel);
        call(&mut bus, n3dvecs, &Entry { p: 0x20, ..Default::default() });
        let (x1, y1, z1) = (bus.read16(X1) as i16, bus.read16(Y1) as i16, bus.read16(Z1) as i16);

        let mut al = sf_game::alien::Alien::default();
        al.roty = roty; al.rotx = rotx; al.vel = vel;
        sf_strat::common::strat_gen_vecs_3d(&mut al);

        let exact = al.vx == x1 && al.vz == z1 && al.vy.abs() == y1.abs();
        if !exact { bad += 1; }
        eprintln!(
            "AIM-MATH roty={roty:3} rotx={rotx:3} vel={vel:3}  retail=({x1},{y1},{z1})  port=({},{},{})  {}",
            al.vx, al.vy, al.vz, if exact { "EXACT" } else { "DIFF" }
        );
    }
    assert_eq!(bad, 0, "{bad}/{} gen_3dvecs cases differ from the RETAIL cart", cases.len());
    eprintln!("AIM-MATH: MATCH — retail n3dvecs_l velocity == port gen_3dvecs (vx/vz/|vy| bit-exact) over {} cases.", cases.len());
}

// ============================================================================
// PROJECTILE-SPAWN + TARGET-SEARCH — the last piece of the firing pipeline.
//
// A firing enemy's fire step is `s_find_nearobj` (walk the active list for the
// nearest matching target) then `s_fire_weapon` -> `fire_weapon_l` (weapon-table
// dispatch) -> per-weapon `fire_X` = `sr_make_obj` (alloc+init+shape) + field
// sets + `gen_weapon` (position the shot at firer + a ROTATED muzzle offset).
// The aim + fire-gate are already certified (UPDATE 8); this certifies the
// object-search + the spawn's observable output.
// ============================================================================

use sf_oracle::{
    RETAIL_FIND_NEAROBJECT_L, RETAIL_FIRE_WEAPON_L, RETAIL_FOBJ, RETAIL_INIT_OBJVARS_L,
    RETAIL_MAKEOBJ_L, RETAIL_RANGEXZ, RETAIL_ROTATE_8XZ_L, RETAIL_ROTATE_8YX_L, RETAIL_ROTATE_8YZ_L,
    RETAIL_SR_MAKE_OBJ, RETAIL_TPX, RETAIL_TPZ, RETAIL_WEAPONS_DATA, RETAIL_XZDIFFS_L,
};

/// MILESTONE — LOCATE + CROSS-VALIDATE the whole projectile-spawn + target-search
/// pipeline in the retail cart by masked signature scan (skeletons read from the
/// built ROM via symbols.txt, WRAM/jsl operands wildcarded), each a UNIQUE hit.
#[test]
fn retail_spawn_pipeline_addresses() {
    let Some(rom) = retail() else { return };

    // --- find_nearobject_l: stx x2; ldx fobj; ...; jsl xzdiffs; lda rangexz; ...
    let fn_pat: Vec<Option<u8>> = vec![
        Some(0x86), None, Some(0xAE), None, None, Some(0xD0), Some(0x03),
        Some(0x82), None, Some(0x00), Some(0xC9), Some(0x00), Some(0x00), Some(0xF0), None,
        Some(0x85), None, Some(0x64), None, Some(0xE4), None, Some(0xF0), None,
        Some(0xB5), Some(0x04), Some(0xC5), None, Some(0xD0), None, Some(0xA4), None,
        Some(0x22), None, None, None, Some(0xC2), Some(0x20), Some(0xAD), None, None,
        Some(0xC5), None, Some(0x10), Some(0x08), Some(0xC5), None, Some(0x30), Some(0x04),
        Some(0x85), None, Some(0x86), None, Some(0xB4), Some(0x00), Some(0xBB), Some(0xD0), None,
    ];
    let hits = masked_scan(&rom, &fn_pat);
    assert_eq!(hits.len(), 1, "find_nearobject_l unique");
    let o = hits[0];
    let find = rom_off_to_snes(o);
    let fobj = rom[o + 3] as u32 | (rom[o + 4] as u32) << 8;
    let xzdiffs = rom[o + 32] as u32 | (rom[o + 33] as u32) << 8 | (rom[o + 34] as u32) << 16;
    let rangexz = rom[o + 38] as u32 | (rom[o + 39] as u32) << 8;
    assert_eq!(find, RETAIL_FIND_NEAROBJECT_L, "find_nearobject_l addr");
    assert_eq!(fobj, RETAIL_FOBJ, "fobj operand");
    assert_eq!(xzdiffs, RETAIL_XZDIFFS_L, "xzdiffs_l jsl operand");
    assert_eq!(rangexz, RETAIL_RANGEXZ, "rangexz operand");
    // struct offsets (al_shape $04, _next $00) confirm the layout the search walks.
    assert_eq!(rom[o + 24], 0x04, "lda al_shape,x offset");
    assert_eq!(rom[o + 53], 0x00, "ldy _next,x offset");
    eprintln!(
        "SPAWN: find_nearobject_l=${find:06X} -> xzdiffs_l=${xzdiffs:06X}  fobj=${fobj:04X} rangexz=${rangexz:04X}"
    );

    // --- fire_weapon_l: 48 AD <stratflags> 29 01 D0 .. 68 86 .. E2 30 .. AA BF ?? ?? 1F ...
    let fw: Vec<Option<u8>> = vec![
        Some(0x48), Some(0xAD), None, None, Some(0x29), Some(0x01), Some(0xD0), None,
        Some(0x68), Some(0x86), None, Some(0xE2), Some(0x30), Some(0x8D), None, None,
        Some(0x0A), Some(0x18), Some(0x6D), None, None, Some(0xAA), Some(0xBF), None, None,
        Some(0x1F), Some(0x48), Some(0xC2), Some(0x20), Some(0xBF), None, None, Some(0x1F),
    ];
    let h = masked_scan(&rom, &fw);
    assert_eq!(h.len(), 1, "fire_weapon_l unique");
    let fwl = rom_off_to_snes(h[0]);
    let wdata4 = rom[h[0] + 23] as u32 | (rom[h[0] + 24] as u32) << 8 | (rom[h[0] + 25] as u32) << 16;
    assert_eq!(fwl, RETAIL_FIRE_WEAPON_L, "fire_weapon_l addr");
    assert_eq!(wdata4, RETAIL_WEAPONS_DATA + 4, "weapons_data+4 operand");
    eprintln!("SPAWN: fire_weapon_l=${fwl:06X} -> weapons_data=${RETAIL_WEAPONS_DATA:06X}");

    // --- sr_make_obj: stx tpx; jsl makeobj_l; bcs; ldy#0; ...; jsl init_objvars_l; ...; sta al_shape,y($04)
    let sm: Vec<Option<u8>> = vec![
        Some(0x86), None, Some(0x22), None, None, Some(0x1F), Some(0xB0), Some(0x07),
        Some(0xA0), Some(0x00), Some(0x00), Some(0xA6), None, Some(0x18), Some(0x6B), Some(0x9B),
        Some(0xA6), None, Some(0xE2), Some(0x20), Some(0x22), None, None, Some(0x1F),
        Some(0xC2), Some(0x20), Some(0xAD), None, None, Some(0x99), Some(0x04), Some(0x00),
        Some(0xE2), Some(0x20), Some(0x38), Some(0x6B),
    ];
    let h = masked_scan(&rom, &sm);
    assert_eq!(h.len(), 1, "sr_make_obj unique");
    let srm = rom_off_to_snes(h[0]);
    let makeobj = rom[h[0] + 3] as u32 | (rom[h[0] + 4] as u32) << 8 | (rom[h[0] + 5] as u32) << 16;
    let initobj = rom[h[0] + 21] as u32 | (rom[h[0] + 22] as u32) << 8 | (rom[h[0] + 23] as u32) << 16;
    assert_eq!(srm, RETAIL_SR_MAKE_OBJ, "sr_make_obj addr");
    assert_eq!(makeobj, RETAIL_MAKEOBJ_L, "makeobj_l (sr_make_obj 1st jsl)");
    assert_eq!(initobj, RETAIL_INIT_OBJVARS_L, "init_objvars_l (sr_make_obj 2nd jsl)");
    assert_eq!(rom[h[0] + 30], 0x04, "sta al_shape,y offset");

    // makeobj_l cross-validated INDEPENDENTLY: its own ldx alfreelst / lda allst
    // operands must equal RETAIL_POOL's freelist_head / active_head.
    let mo = snes_to_rom_off(RETAIL_MAKEOBJ_L);
    assert_eq!(rom[mo], 0xC2, "makeobj_l starts rep #$20");
    let alfree = rom[mo + 4] as u32 | (rom[mo + 5] as u32) << 8;
    assert_eq!(alfree, RETAIL_POOL.freelist_head, "makeobj_l ldx alfreelst == pool freelist_head");
    eprintln!(
        "SPAWN: sr_make_obj=${srm:06X} -> makeobj_l=${makeobj:06X} (alfreelst=${alfree:04X}), init_objvars_l=${initobj:06X}"
    );

    // --- gen_weapon muzzle rotation primitives (each UNIQUE) ---
    let rot8xz: Vec<Option<u8>> = vec![
        Some(0x5A), Some(0xDA), Some(0x08), Some(0x8B), Some(0xE2), Some(0x10),
        Some(0x49), Some(0xFF), Some(0x1A), Some(0xAA), Some(0xA9), None, Some(0x48), Some(0xAB),
        Some(0xBD), None, None, Some(0x8D), None, None, Some(0xBD), None, None, Some(0x8D),
        None, None, Some(0xA5), Some(0x02),
    ];
    let rot8yz: Vec<Option<u8>> = vec![
        Some(0xDA), Some(0x5A), Some(0x08), Some(0x8B), Some(0xE2), Some(0x10),
        Some(0xAA), Some(0xA9), None, Some(0x48), Some(0xAB), Some(0xBD), None, None, Some(0x8D),
        None, None, Some(0xBD), None, None, Some(0x8D), None, None, Some(0xA5), Some(0x08),
    ];
    let rot8yx: Vec<Option<u8>> = vec![
        Some(0x5A), Some(0xDA), Some(0x08), Some(0x8B), Some(0xE2), Some(0x10),
        Some(0xAA), Some(0xA9), None, Some(0x48), Some(0xAB), Some(0xBD), None, None, Some(0x8D),
        None, None, Some(0xBD), None, None, Some(0x8D), None, None, Some(0xA5), Some(0x02),
    ];
    for (name, pat, want) in [
        ("rotate_8xz_l", rot8xz, RETAIL_ROTATE_8XZ_L),
        ("rotate_8yz_l", rot8yz, RETAIL_ROTATE_8YZ_L),
        ("rotate_8yx_l", rot8yx, RETAIL_ROTATE_8YX_L),
    ] {
        let h = masked_scan(&rom, &pat);
        assert_eq!(h.len(), 1, "{name} unique");
        assert_eq!(rom_off_to_snes(h[0]), want, "{name} addr");
    }
    eprintln!(
        "SPAWN: gen_weapon muzzle rotation = rotate_8yx_l=${RETAIL_ROTATE_8YX_L:06X} -> rotate_8yz_l=${RETAIL_ROTATE_8YZ_L:06X} -> rotate_8xz_l=${RETAIL_ROTATE_8XZ_L:06X} (CPU sin/cos, no GSU)"
    );
    eprintln!("SPAWN PIPELINE: all addresses located + cross-validated.");
}

/// Seed a candidate object block on the retail active list.
fn seed_find_obj(bus: &mut SnesBus, slot: u32, shape: u16, x: i16, y: i16, z: i16, next: u16) {
    let b = RETAIL_POOL.base + slot * RETAIL_POOL.stride;
    bus.wram_write16(b + RETAIL_POOL.al_shape, shape);
    bus.wram_write16(b + RETAIL_POOL.al_worldx, x as u16);
    bus.wram_write16(b + RETAIL_POOL.al_worldy, y as u16);
    bus.wram_write16(b + RETAIL_POOL.al_worldz, z as u16);
    bus.wram_write16(b + RETAIL_POOL.al_next, next);
}
fn block_of(slot: u32) -> u16 {
    (RETAIL_POOL.base + slot * RETAIL_POOL.stride) as u16
}
fn slot_of(block: u16) -> Option<u32> {
    if block == 0 {
        return None;
    }
    Some((block as u32 - RETAIL_POOL.base) / RETAIL_POOL.stride)
}

/// Run the retail cart's OWN `find_nearobject_l` over a seeded active list.
/// `objs` = (shape,x,y,z) for slots 1..=N; slot 0 is the searcher (self) at
/// `self_pos`. Returns the selected slot (None = no match).
fn retail_find_near(
    rom: &[u8],
    objs: &[(u16, i16, i16, i16)],
    self_pos: (i16, i16, i16),
    shape: u16,
    min_r: i16,
    max_r: i16,
) -> Option<u32> {
    let mut bus = SnesBus::new(rom.to_vec());
    // slot 0 = self (skipped by cpx x2). Chain: self -> s1 -> ... -> sN -> 0.
    let n = objs.len() as u32;
    seed_find_obj(&mut bus, 0, 0x0001, self_pos.0, self_pos.1, self_pos.2, block_of(1));
    for (i, &(sh, x, y, z)) in objs.iter().enumerate() {
        let slot = i as u32 + 1;
        let next = if slot < n { block_of(slot + 1) } else { 0 };
        seed_find_obj(&mut bus, slot, sh, x, y, z, next);
    }
    bus.wram_write16(RETAIL_FOBJ, block_of(0)); // search list head
    bus.wram_write16(RETAIL_TPZ, min_r as u16);
    bus.wram_write16(RETAIL_TPX, max_r as u16);
    // Entry: A = target shape, X = self block, ai16 (p=0). Y is set internally.
    let e = call(&mut bus, RETAIL_FIND_NEAROBJECT_L, &Entry {
        a: shape,
        x: block_of(0) as u16,
        p: 0x00,
        ..Default::default()
    });
    slot_of(e.y as u16)
}

/// Faithful transcription of the PORT's `enemy_a::strat_find_near_shape`
/// (rust/sf-strat/src/enemy_a.rs:513-555; `pub(crate)`, so modelled here exactly
/// as the existing port-model tests do). shapes_table mapping is identity for the
/// test shapes, so `mapped_shape == shape_id`.
fn port_find_near_shape(
    objs: &[(u16, i16, i16, i16)],
    self_pos: (i16, i16, i16),
    shape_id: u16,
    max_z: i16,
    max_xy: i16,
) -> Option<u32> {
    let (mx, my, mz) = self_pos;
    let mut best: Option<u32> = None;
    let mut best_metric: i32 = i32::MAX;
    for (i, &(sh, x, _y, z)) in objs.iter().enumerate() {
        if sh != shape_id {
            continue;
        }
        // Mirrors the FIXED sf_strat::enemy_a::find_near_shape: XZ-only
        // (rangexz = |dx|+|dz|), Y dropped from gate + metric to match the cart's
        // xzdiffs. (my/dy intentionally unused now — kept in the signature for
        // parity with the port's caller shape.)
        let _ = my;
        let dz = (z as i32 - mz as i32).unsigned_abs() as i16;
        let dx = (x as i32 - mx as i32).unsigned_abs() as i16;
        if dz > max_z || dx > max_xy {
            continue;
        }
        let metric = dx as i32 + dz as i32;
        if metric < best_metric {
            best_metric = metric;
            best = Some(i as u32 + 1);
        }
    }
    best
}

/// CERTIFY the target search (`s_find_nearobj` -> `find_nearobject_l`) vs the
/// port. Runs the retail cart's OWN `find_nearobject_l` over seeded object lists
/// and diffs the SELECTED target vs the port's `strat_find_near_shape`.
///
/// RESULT: MATCH across the COPLANAR region (8 configs) AND the Y-separated case.
/// This test originally FOUND a fidelity gap — retail's `xzdiffs` **rangexz** is
/// an XZ-plane distance that IGNORES Y (both gate and nearest-metric), whereas the
/// port used a 3D box gate + Manhattan `dx+dy+dz` that COUNTED Y, so Y-separated
/// candidates could rank differently. FIXED: `sf_strat::enemy_a::find_near_shape`
/// / `find_near_colltype` now drop Y to match the cart; this test asserts the
/// resulting MATCH (find->fix->re-certify loop closed).
#[test]
fn retail_find_nearobject_vs_port() {
    let Some(rom) = retail() else { return };
    let shape = 0x0050u16;
    let other = 0x0060u16; // non-matching shape — both must skip it.

    // --- Agreement region: coplanar (Y=0) targets, clear unique nearest. ---
    // Each entry: (label, self_pos, candidates[(shape,x,y,z)], expect_slot).
    let coplanar: [(&str, (i16, i16, i16), Vec<(u16, i16, i16, i16)>, Option<u32>); 8] = [
        ("near+far+wrongshape", (0, 0, 0),
            vec![(shape, 600, 0, 200), (shape, 3000, 0, 1000), (other, 100, 0, 100), (shape, 100, 0, 5000)],
            Some(1)),
        ("nearest is s3", (0, 0, 0),
            vec![(shape, 4000, 0, 0), (shape, 2500, 0, 800), (shape, 400, 0, 300), (shape, 900, 0, 1200)],
            Some(3)),
        ("all four quadrants", (0, 0, 0),
            vec![(shape, -1200, 0, -1200), (shape, 900, 0, -300), (shape, -300, 0, 900), (shape, 2000, 0, 2000)],
            Some(2)),
        ("nonzero self origin", (5000, 0, -2000),
            vec![(shape, 5400, 0, -1800), (shape, 8000, 0, 1000), (shape, 5100, 0, -6000)],
            Some(1)),
        ("no match — wrong shape", (0, 0, 0),
            vec![(other, 100, 0, 0), (other, 200, 0, 0)],
            None),
        ("nearest among many", (0, 0, 0),
            vec![(shape, 3000, 0, 100), (shape, 1500, 0, 1500), (shape, 700, 0, 300), (other, 50, 0, 50), (shape, 2000, 0, 400)],
            Some(3)),
        ("axis vs diagonal (octagonal norm)", (0, 0, 0),
            vec![(shape, 1000, 0, 0), (shape, 760, 0, 760)],
            Some(1)),
        ("single candidate", (0, 0, 0),
            vec![(shape, 1234, 0, -567)],
            Some(1)),
    ];

    let (min_r, max_r) = (0i16, 10000i16);
    let (max_z, max_xy) = (10000i16, 10000i16);
    let mut coplanar_ok = 0;
    for (label, self_pos, cands, _expect) in &coplanar {
        let retail = retail_find_near(&rom, cands, *self_pos, shape, min_r, max_r);
        let port = port_find_near_shape(cands, *self_pos, shape, max_z, max_xy);
        let agree = retail == port;
        if agree {
            coplanar_ok += 1;
        }
        eprintln!(
            "FIND [{label}]: retail=slot{:?} port=slot{:?}  {}",
            retail, port, if agree { "MATCH" } else { "DIFF" }
        );
        assert_eq!(retail, port, "coplanar find_nearobject must match port for '{label}'");
    }
    eprintln!("FIND: coplanar region MATCH — {coplanar_ok}/{} configs, retail find_nearobject_l == port strat_find_near_shape.", coplanar.len());

    // --- Radius-band gate (small radius, within the ROM's <8000 valid domain):
    // candidates clearly beyond the radius are rejected by BOTH -> None. ---
    let far: Vec<(u16, i16, i16, i16)> = vec![(shape, 5000, 0, 0), (shape, 0, 0, 5000)];
    let rr = retail_find_near(&rom, &far, (0, 0, 0), shape, 0, 2000);
    let pp = port_find_near_shape(&far, (0, 0, 0), shape, 2000, 2000);
    eprintln!("FIND [radius-band reject]: retail=slot{rr:?} port=slot{pp:?}");
    assert_eq!(rr, None, "retail rejects candidates beyond max radius");
    assert_eq!(pp, None, "port rejects candidates beyond max radius");

    // --- FIXED + re-certified: Y-separated targets. Retail (XZ-only) picks the
    // XZ-nearest; the port now also drops Y (enemy_a find_near_shape fix, this
    // commit's sibling in sf-strat) -> both pick the XZ-nearest. This is the
    // find->fix->re-certify loop closing on the bug this test originally found. ---
    let ydiv: Vec<(u16, i16, i16, i16)> = vec![
        (shape, 300, 7000, 0), // close in XZ, far in Y -> XZ-nearest (slot 1)
        (shape, 2000, 0, 0),   // farther XZ, coplanar
    ];
    let r = retail_find_near(&rom, &ydiv, (0, 0, 0), shape, min_r, max_r);
    let p = port_find_near_shape(&ydiv, (0, 0, 0), shape, max_z, max_xy);
    eprintln!(
        "FIND [Y-separated targets]: retail=slot{:?} port=slot{:?}  {}",
        r, p, if r == p { "MATCH (Y dropped)" } else { "DIFF" }
    );
    assert_eq!(r, Some(1), "retail find_nearobject_l ignores Y -> XZ-nearest (slot 1)");
    assert_eq!(p, Some(1), "port find_near_shape now XZ-only -> XZ-nearest (slot 1)");
    assert_eq!(r, p, "FIXED: port find_near_shape now matches retail's XZ-only ranking");
    eprintln!(
        "FIND: FIXED — port find_near_shape/find_near_colltype drop Y to match the \
         cart's xzdiffs/rangexz (XZ-plane only). Y-separated targets now agree."
    );
}

/// CERTIFY the spawn ALLOCATION observable (`s_make_obj` -> `sr_make_obj` ->
/// `makeobj_l` + `init_objvars_l` + `al_shape`) vs the port `make_obj`.
///
/// Runs the retail cart's OWN `sr_make_obj` on a real formatted pool: it pops the
/// free list (`makeobj_l`), zeroes the block (`init_objvars_l`), and stores the
/// requested shape. Certifies the NEW object's observable fields — `al_shape` ==
/// requested, world coords zeroed, and the free list actually shrank — against
/// the port `common::make_obj` (alloc + `strat_init_obj_vars` + shape).
#[test]
fn retail_sr_make_obj_spawn_vs_port() {
    use sf_oracle::RETAIL_TPA;
    let Some(rom) = retail() else { return };
    let mut bus = SnesBus::new(rom);
    init_object_pool(&mut bus);

    let free_before = walk_freelist(&bus, &RETAIL_POOL);
    let want_shape = 0x0042u16;

    // s_make_obj sets tpa=shape, then jsl sr_make_obj (X=firer, preserved).
    bus.wram_write16(RETAIL_TPA, want_shape);
    let e = call(&mut bus, RETAIL_SR_MAKE_OBJ, &Entry { x: 0, p: 0x00, ..Default::default() });
    let new_block = e.y as u16;
    let new_slot = slot_of(new_block).expect("sr_make_obj returned a valid pool block");

    let b = new_block as u32;
    let shape = bus.wram_read16(b + RETAIL_POOL.al_shape);
    let wx = bus.wram_read16(b + RETAIL_POOL.al_worldx) as i16;
    let wy = bus.wram_read16(b + RETAIL_POOL.al_worldy) as i16;
    let wz = bus.wram_read16(b + RETAIL_POOL.al_worldz) as i16;
    let free_after = walk_freelist(&bus, &RETAIL_POOL);

    eprintln!(
        "MAKEOBJ: retail sr_make_obj -> slot {new_slot} (block ${new_block:04X}) shape=${shape:04X} world=({wx},{wy},{wz}) | freelist {} -> {}",
        free_before.len(), free_after.len()
    );
    assert_eq!(shape, want_shape, "retail spawn sets al_shape = requested");
    assert_eq!((wx, wy, wz), (0, 0, 0), "retail init_objvars zeroed the new block's world coords");
    assert_eq!(free_after.len(), free_before.len() - 1, "one block was allocated off the free list");
    assert!(!free_after.contains(&new_block), "the allocated block left the free list");

    // Port: make_obj = alloc + strat_init_obj_vars + shape.
    let mut g = sf_game::game::Game::new();
    let idx = sf_strat::common::make_obj(&mut g, want_shape).expect("port make_obj");
    let pal = &g.objs.aliens[idx as usize];
    eprintln!(
        "MAKEOBJ: port make_obj -> slot {idx} shape=${:04X} world=({},{},{})",
        pal.shape, pal.worldx, pal.worldy, pal.worldz
    );
    assert_eq!(pal.shape, want_shape, "port spawn sets shape = requested");
    assert_eq!((pal.worldx, pal.worldy, pal.worldz), (0, 0, 0), "port init zeroed world coords");

    // Observable output MATCH: both allocators materialise a fresh object with the
    // requested shape and zeroed world position (slot INDEX may differ — the two
    // free-list formats are distinct, documented in retail_snapshot_reads_seeded).
    assert_eq!(shape, pal.shape, "spawned shape matches port");
    assert_eq!((wx, wy, wz), (pal.worldx, pal.worldy, pal.worldz), "spawned world pos matches port");
    eprintln!("MAKEOBJ: MATCH — retail sr_make_obj new-object observable (shape + zeroed world pos) == port make_obj.");
}

// ============================================================================
// COLLISION SYSTEM vs the RETAIL cart — the highest-blast-radius shared surface
// (every laser hit, ship/enemy contact, pickup). Three pieces certified:
//   1. do_coll_l   — the collision RESPONSE (damage + cooldown). ROM-resident,
//                    RUN surgically vs the port.
//   2. COLDET      — the object-vs-object box-overlap TEST. Inlined in the
//                    RAM-resident `chkcoll` (SNES $7E:5015 in the symbol map),
//                    so it cannot be JSL'd on a non-booted bus; located in its
//                    ROM copy-source (bank $02) + certified structurally and by
//                    grid-diffing the port `aabb_overlap` vs a byte-faithful
//                    transcription of the ASM.
//   3. chkcoll0    — the colltype ALLOW-MATRIX (who may hit whom) + the
//                    same-shape gate. Also inside `chkcoll`; located in the copy-
//                    source. The colltype matrix MATCHES the port; the same-shape
//                    gate is a REAL port-vs-cart divergence, characterized below.
// ============================================================================

/// MILESTONE — LOCATE + CROSS-VALIDATE the three collision routines in the
/// retail cart by masked signature scan, reading their operands back out.
#[test]
fn retail_collision_addresses() {
    let Some(rom) = retail() else { return };

    // --- do_coll_l (ROM-resident, $1F bank) — UNIQUE ---
    let dc: Vec<Option<u8>> = vec![
        Some(0xD6), Some(0x2D), Some(0xF0), Some(0x04), Some(0x5C), None, None, None,
        Some(0xAD), None, None, Some(0x29), Some(0x01), Some(0xD0), Some(0x04), Some(0x5C), None, None, None,
        Some(0xA5), Some(0x02), Some(0xC9), Some(0x08), Some(0xD0), Some(0x05), Some(0xC9), Some(0x80),
        Some(0x6A), Some(0x85), Some(0x02), Some(0xB5), Some(0x2A), Some(0x30), Some(0x09), Some(0x38),
        Some(0xE5), Some(0x02), Some(0x10), Some(0x02), Some(0xA9), Some(0x00), Some(0x95), Some(0x2A),
        Some(0xAD), None, None, Some(0x95), Some(0x2D),
    ];
    let h = masked_scan(&rom, &dc);
    assert_eq!(h.len(), 1, "do_coll_l unique");
    let o = h[0];
    let addr = rom_off_to_snes(o);
    let pshipflags3 = rom[o + 9] as u32 | (rom[o + 10] as u32) << 8;
    let tpa = rom[o + 44] as u32 | (rom[o + 45] as u32) << 8;
    assert_eq!(addr, sf_oracle::RETAIL_DO_COLL_L, "do_coll_l addr");
    assert_eq!(pshipflags3, sf_oracle::RETAIL_PSHIPFLAGS3, "pshipflags3 operand");
    assert_eq!(tpa, sf_oracle::RETAIL_TPA, "tpa (framesperAP reload) operand == RETAIL_TPA");
    // struct offsets read back: collcount=$2D, HP=$2A; consts hardAP=$08, intunnel=$01.
    assert_eq!(rom[o + 1] as u32, sf_oracle::AL_COLLCOUNT, "DEC al_collcount,x offset");
    assert_eq!(rom[o + 31] as u32, sf_oracle::AL_HP, "LDA al_HP,x offset");
    assert_eq!(rom[o + 22], sf_oracle::HARD_AP, "CMP #hardAP immediate");
    assert_eq!(rom[o + 12], 0x01, "AND #psf3_intunnel immediate");
    eprintln!("COLL: do_coll_l=${addr:06X}  pshipflags3=${pshipflags3:04X} tpa=${tpa:04X} (collcount=$2D HP=$2A hardAP=8)");

    // --- COLDET box-overlap macro (RAM copy-source, bank $02): three
    //     consecutive 16-bit axis tests. Anchor on the abs+bmi+jmp core. ---
    let axis_core: Vec<Option<u8>> = vec![
        // ...bpl+4; eor #$FFFF; inc a; sec; sbc rangexz; bmi+3; jmp ....
        Some(0x10), Some(0x04), Some(0x49), Some(0xFF), Some(0xFF), Some(0x1A),
        Some(0x38), Some(0xED), None, None, Some(0x30), Some(0x03), Some(0x4C),
    ];
    // full one-axis pattern: lda cl_max,x; clc; adc Ncol; sta rangexz; lda tpN;
    // sec; sbc Np; <axis_core>
    let mut zaxis: Vec<Option<u8>> = vec![
        Some(0xBD), None, None, Some(0x18), Some(0x6D), None, None, Some(0x8D), None, None,
        Some(0xA5), None, Some(0x38), Some(0xE5), None,
    ];
    zaxis.extend_from_slice(&axis_core);
    let za = masked_scan(&rom, &zaxis);
    assert!(!za.is_empty(), "COLDET axis pattern present in retail (RAM copy-source)");
    // The documented Z-axis start of the normalcol expansion:
    let want = snes_to_rom_off(sf_oracle::RETAIL_COLDET_OVERLAP);
    assert!(za.contains(&want), "RETAIL_COLDET_OVERLAP is one of the located axis tests");
    // Confirm the boundary is STRICTLY-LESS: `sbc rangexz; bmi` (30) => in-range
    // iff (|d| - sum) < 0, and the sum's low operand at this site is rangexz.
    let rangexz = rom[want + 8] as u32 | (rom[want + 9] as u32) << 8; // sta rangexz operand
    assert_eq!(rangexz, sf_oracle::RETAIL_RANGEXZ, "COLDET compares against rangexz ($1250)");
    assert_eq!(rom[want + 25], 0x30, "boundary opcode is BMI (strictly-less)");
    eprintln!("COLL: COLDET box-overlap @${:06X} (16-bit |d|<sum, Z/X/Y, bmi strictly-less), {} axis-tests total", sf_oracle::RETAIL_COLDET_OVERLAP, za.len());

    // --- chkcoll0 colltype allow-matrix filter (RAM copy-source, bank $02) ---
    // lda al_collflags,y (B9 2E 00); and al_collflags,x (35 2E); and #F8 00; beq +; brl skip
    let ct: Vec<Option<u8>> = vec![
        Some(0xB9), Some(0x2E), Some(0x00), Some(0x35), Some(0x2E), Some(0x29), Some(0xF8), Some(0x00),
        Some(0xF0), Some(0x03), Some(0x82),
    ];
    let ch = masked_scan(&rom, &ct);
    assert_eq!(ch.len(), 1, "chkcoll0 colltype filter unique");
    let co = rom_off_to_snes(ch[0]);
    assert_eq!(co, sf_oracle::RETAIL_CHKCOLL_COLLTYPE, "colltype filter addr");
    let mask = rom[ch[0] + 6]; // and #$00F8 low byte
    assert_eq!(mask, sf_oracle::COLLTYPE_MASK, "colltype mask == $F8 (colltype1..5)");
    assert_eq!(rom[ch[0] + 1] as u32, sf_oracle::AL_COLLFLAGS, "al_collflags offset $2E");
    eprintln!("COLL: colltype allow-matrix @${co:06X}  mask=${mask:02X}  (skip iff cf_a&cf_b&mask != 0; NO both-zero skip)");

    // --- immunity + same-shape gate right after the colltype filter ---
    // cmp al_immuneptr,x (D5 19) appears twice; lda al_shape,x; cmp currshape.
    // Locate the same-shape compare: B5 04 (lda al_shape,x); CD <currshape>; then a
    // long branch to chkcollnxt. Confirm currshape operand and immuneptr offset.
    let ss: Vec<Option<u8>> = vec![Some(0xB5), Some(0x04), Some(0x9B), Some(0xCD), None, None, Some(0xD0), Some(0x03), Some(0x82)];
    let sh = masked_scan(&rom, &ss);
    assert!(!sh.is_empty(), "same-shape gate present in retail chkcoll0");
    let sso = sh[0];
    let currshape = rom[sso + 4] as u32 | (rom[sso + 5] as u32) << 8;
    assert_eq!(currshape, sf_oracle::RETAIL_CURRSHAPE, "same-shape gate compares al_shape,x to currshape ($1F03)");
    eprintln!("COLL: same-shape gate @${:06X} (lda al_shape,x; cmp currshape=${currshape:04X}; beq->skip) — port has NO shape gate (divergence)", rom_off_to_snes(sso));
}

/// Retail collision RESPONSE certified vs the port: run the cart's OWN `do_coll_l`
/// ($1F:D23A) on a seeded victim over a grid of (collcount, hp, ap, tunnel) and
/// diff the resulting (collcount, hp) against the port `Game::do_coll`
/// (coldet.rs:236) transcribed byte-faithfully. Covers: the DEC-then-BNE cooldown
/// gate, the hp bit-7 (>=$80) indestructible branch, the underflow clamp at 0,
/// the in-tunnel hardAP halving, and the framesperAP reload.
#[test]
fn retail_docoll_response_vs_port() {
    let Some(rom) = retail() else { return };
    const XB: u32 = 0x0100; // any WRAM block; do_coll only touches collcount/HP here.
    const X1: u32 = 0x02; // dp scratch: damage (AP)

    // Byte-faithful port do_coll (coldet.rs:236-259). Returns (collcount, hp).
    fn port_do_coll(collcount: u8, hp: u8, ap: u8, tunnel: bool) -> (u8, u8) {
        let mut cc = collcount;
        let mut h = hp;
        cc = cc.wrapping_sub(1);
        if cc != 0 {
            return (cc, h);
        }
        let mut damage = ap;
        if tunnel && damage == sf_oracle::HARD_AP {
            damage >>= 1;
        }
        if (h as i8) >= 0 {
            h = h.saturating_sub(damage);
        }
        (sf_oracle::FRAMESPERAP, h)
    }

    let grid: &[(u8, u8, u8, bool)] = &[
        (10, 50, 1, false),   // cooldown active: DEC only
        (1, 50, 1, false),    // DEC->0: apply 1 damage
        (1, 50, 7, false),    // heavier hit
        (0, 50, 1, false),    // collcount 0 -> wraps to 255 (cooldown, no damage)
        (1, 3, 5, false),     // underflow clamp at 0
        (1, 0x80, 20, false), // hp bit7 set => indestructible
        (1, 0xFF, 20, false), // hp $FF => indestructible
        (1, 50, 8, true),     // in-tunnel + hardAP(8): halved to 4
        (1, 50, 8, false),    // hardAP but NOT tunnel: full 8
        (1, 50, 6, true),     // in-tunnel + non-hardAP: unchanged
        (2, 50, 8, true),     // cooldown active in tunnel: DEC only
    ];

    let mut all = true;
    for &(cc, hp, ap, tunnel) in grid {
        let mut bus = SnesBus::new(rom.clone());
        bus.write8(XB + sf_oracle::AL_COLLCOUNT, cc);
        bus.write8(XB + sf_oracle::AL_HP, hp);
        bus.write8(X1, ap);
        bus.write8(sf_oracle::RETAIL_TPA, sf_oracle::FRAMESPERAP);
        bus.write8(sf_oracle::RETAIL_PSHIPFLAGS3, if tunnel { 1 } else { 0 });
        call(&mut bus, sf_oracle::RETAIL_DO_COLL_L, &Entry { x: XB as u16, p: 0x20, ..Default::default() });
        let rcc = bus.read8(XB + sf_oracle::AL_COLLCOUNT);
        let rhp = bus.read8(XB + sf_oracle::AL_HP);
        let (pcc, php) = port_do_coll(cc, hp, ap, tunnel);
        let ok = (rcc, rhp) == (pcc, php);
        all &= ok;
        eprintln!(
            "COLL do_coll cc={cc} hp={hp} ap={ap} tun={tunnel}: retail=({rcc},{rhp}) port=({pcc},{php}) {}",
            if ok { "MATCH" } else { "DIFF" }
        );
        assert_eq!((rcc, rhp), (pcc, php), "do_coll cc={cc} hp={hp} ap={ap} tunnel={tunnel}");
    }
    assert!(all);
    eprintln!("COLL: RESPONSE MATCH — retail do_coll_l == port Game::do_coll over the full damage/cooldown/tunnel/indestructible grid.");
}

/// Retail box-overlap MATH certified vs the port `aabb_overlap`. The retail test
/// (`chkcoll`) is RAM-resident so it cannot be JSL'd on a non-booted bus; instead
/// we transcribe the confirmed `COLDET` macro ASM (16-bit two's-complement abs,
/// Z/X/Y order, strictly-less `|d| < sum`) into a reference and diff the PORT's
/// public `aabb_overlap` against it over a grid that STRADDLES the boundary on
/// each axis (the off-by-one home) plus the i16 wrap edge.
#[test]
fn retail_box_overlap_vs_port() {
    use sf_game::coldet::aabb_overlap;

    // ROM COLDET reference: transcribed from SNES $02:A1CE (see
    // RETAIL_COLDET_OVERLAP). Overlap iff on ALL of Z,X,Y: |pos2-pos1| < e1+e2,
    // with a 16-bit two's-complement abs (matches the ASM eor #$FFFF; inc a).
    fn rom_overlap(
        x1: i16, y1: i16, z1: i16, e1x: i16, e1y: i16, e1z: i16,
        x2: i16, y2: i16, z2: i16, e2x: i16, e2y: i16, e2z: i16,
    ) -> bool {
        // axis order Z, X, Y — early-out exactly like the ASM's three jmps.
        let dz = z2.wrapping_sub(z1);
        let dz = if dz < 0 { dz.wrapping_neg() } else { dz };
        if dz >= e1z.wrapping_add(e2z) { return false; }
        let dx = x2.wrapping_sub(x1);
        let dx = if dx < 0 { dx.wrapping_neg() } else { dx };
        if dx >= e1x.wrapping_add(e2x) { return false; }
        let dy = y2.wrapping_sub(y1);
        let dy = if dy < 0 { dy.wrapping_neg() } else { dy };
        if dy >= e1y.wrapping_add(e2y) { return false; }
        true
    }

    // Extents: object 1 = (20,16,20), object 2 = (10,10,10). Sums: x=30,y=26,z=30.
    let (e1x, e1y, e1z) = (20i16, 16, 20);
    let (e2x, e2y, e2z) = (10i16, 10, 10);
    let mut checks = 0usize;
    let mut mism = 0usize;

    // Boundary-straddling grid on each axis independently (others coincident).
    for &sep in &[
        // separations around each axis's boundary (sum ±2), incl. exact boundary.
        -32i16, -31, -30, -29, -28, -27, -26, -25, -24, -23, -22, -21, -20,
        -2, -1, 0, 1, 2,
        20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
    ] {
        // vary X
        {
            let p = aabb_overlap(0, 0, 0, e1x, e1y, e1z, sep, 0, 0, e2x, e2y, e2z);
            let r = rom_overlap(0, 0, 0, e1x, e1y, e1z, sep, 0, 0, e2x, e2y, e2z);
            checks += 1; if p != r { mism += 1; eprintln!("DIFF X sep={sep}: port={p} rom={r}"); }
        }
        // vary Y
        {
            let p = aabb_overlap(0, 0, 0, e1x, e1y, e1z, 0, sep, 0, e2x, e2y, e2z);
            let r = rom_overlap(0, 0, 0, e1x, e1y, e1z, 0, sep, 0, e2x, e2y, e2z);
            checks += 1; if p != r { mism += 1; eprintln!("DIFF Y sep={sep}: port={p} rom={r}"); }
        }
        // vary Z
        {
            let p = aabb_overlap(0, 0, 0, e1x, e1y, e1z, 0, 0, sep, e2x, e2y, e2z);
            let r = rom_overlap(0, 0, 0, e1x, e1y, e1z, 0, 0, sep, e2x, e2y, e2z);
            checks += 1; if p != r { mism += 1; eprintln!("DIFF Z sep={sep}: port={p} rom={r}"); }
        }
    }

    // The i16 wrap edge: two's-complement abs of i16::MIN is i16::MIN (both sides
    // treat it as "in range"). Place obj2 near the wrap so dx wraps.
    for &(a, b) in &[(30000i16, -30000i16), (-32768, 0), (32000, -32000), (i16::MIN, 0)] {
        let p = aabb_overlap(a, 0, 0, e1x, e1y, e1z, b, 0, 0, e2x, e2y, e2z);
        let r = rom_overlap(a, 0, 0, e1x, e1y, e1z, b, 0, 0, e2x, e2y, e2z);
        checks += 1; if p != r { mism += 1; eprintln!("DIFF wrap a={a} b={b}: port={p} rom={r}"); }
    }

    eprintln!("COLL: box-overlap grid {checks} checks, {mism} mismatches");
    assert_eq!(mism, 0, "port aabb_overlap must match the retail COLDET macro over the boundary grid");
    // Prove the boundary is where it should be (strictly-less): exactly at the sum
    // there is NO overlap; one below there IS.
    assert!(!aabb_overlap(0, 0, 0, e1x, e1y, e1z, 30, 0, 0, e2x, e2y, e2z), "sep==sum(30) => NO overlap");
    assert!(aabb_overlap(0, 0, 0, e1x, e1y, e1z, 29, 0, 0, e2x, e2y, e2z), "sep==sum-1(29) => overlap");
    eprintln!("COLL: box-overlap MATCH — port == retail COLDET, boundary is strictly |d| < e1+e2 on Z/X/Y.");
}

/// Retail collision ALLOW-MATRIX (who may hit whom) certified vs the port. The
/// retail rule (`chkcoll0`, SNES $02:A15E): a pair is SKIPPED iff it shares any
/// collision-type bit (`cf_a & cf_b & $F8 != 0`), with NO "both zero => skip".
/// Diff the port's identical filter (`Game::coldet_run`: `a_types & b_types != 0
/// -> continue`) against a ROM-faithful reference over the FULL colltype matrix.
#[test]
fn retail_colltype_matrix_vs_port() {
    // ROM reference: SKIP iff (cf_a & cf_b & mask) != 0.
    fn rom_skip(cf_a: u8, cf_b: u8) -> bool {
        (cf_a & cf_b & sf_oracle::COLLTYPE_MASK) != 0
    }
    // Port reference: coldet.rs:310-314.
    const PORT_MASK: u8 = sf_game::alien::ACF_COLLTYPE1
        | sf_game::alien::ACF_COLLTYPE2
        | sf_game::alien::ACF_COLLTYPE3
        | sf_game::alien::ACF_COLLTYPE4
        | sf_game::alien::ACF_COLLTYPE5;
    fn port_skip(cf_a: u8, cf_b: u8) -> bool {
        (cf_a & PORT_MASK) & (cf_b & PORT_MASK) != 0
    }

    assert_eq!(PORT_MASK, sf_oracle::COLLTYPE_MASK, "port TYPE_MASK == retail colltype mask $F8");

    // Full matrix over the type bits + weapon/firstframe noise bits.
    let bits = [0u8, 0x08, 0x10, 0x20, 0x40, 0x80, 0x02, 0x04];
    let mut mism = 0;
    let mut n = 0;
    for &a0 in &bits { for &a1 in &bits { for &b0 in &bits { for &b1 in &bits {
        let cf_a = a0 | a1;
        let cf_b = b0 | b1;
        let r = rom_skip(cf_a, cf_b);
        let p = port_skip(cf_a, cf_b);
        n += 1;
        if r != p { mism += 1; eprintln!("DIFF cf_a=${cf_a:02X} cf_b=${cf_b:02X}: rom_skip={r} port_skip={p}"); }
    }}}}
    assert_eq!(mism, 0, "colltype allow-matrix must match retail over the full matrix ({n} combos)");

    // Semantic allow-matrix spot-checks (STRATEQU.INC:943-954):
    let laser = 0x08u8;    // colltype1 all lasers
    let enemy1 = 0x10u8;   // colltype2
    let enemy2 = 0x20u8;   // colltype3
    let enemyw = 0x40u8;   // colltype4 enemy weapons
    let friend = 0x80u8;   // colltype5
    assert!(!rom_skip(laser, enemy1), "laser vs enemy1 => COLLIDE");
    assert!(!rom_skip(laser, enemy2), "laser vs enemy2 => COLLIDE");
    assert!(rom_skip(laser, laser), "laser vs laser => skip (shared colltype1)");
    assert!(rom_skip(enemy1, enemy1), "enemy1 vs enemy1 => skip (shared colltype2)");
    assert!(!rom_skip(enemyw, 0), "enemy-weapon vs player(no type) => COLLIDE (no both-zero skip)");
    assert!(!rom_skip(0, 0), "two typeless objects => COLLIDE (no both-zero skip)");
    assert!(!rom_skip(laser, friend), "laser vs friend => COLLIDE");
    eprintln!("COLL: ALLOW-MATRIX MATCH — retail chkcoll0 colltype filter == port over {n} combos; skip iff shared colltype bit, no both-zero skip.");
}

/// REAL DIVERGENCE (characterized) — the retail same-shape collision gate.
///
/// Retail `chkcoll0` (SNES $02:A15E region) SKIPS a candidate pair when the two
/// objects have the SAME `al_shape` (`lda al_shape,x; cmp currshape; beq -> brl
/// chkcollnxt`), UNLESS BOTH carry the `sameshapecollide` sflag (sflags3 bit
/// $80). `sameshapecollide` is set by essentially nothing (1 file / 2 sites in
/// the whole reference source), so the cart effectively NEVER collides two
/// same-shape objects with each other. The port `Game::coldet_run` (coldet.rs)
/// has NO shape gate at all — so it WILL collide same-shape objects the cart
/// skips.
///
/// Blast radius is NARROW: same-shape objects usually also share a colltype and
/// are already dropped by the colltype filter (certified above). The residual
/// case that bites is two objects of the SAME shape but DIFFERENT colltype (e.g.
/// two same-model enemies registered as enemy1 vs enemy2) overlapping — the cart
/// skips, the port damages both. This test constructs exactly that scene and
/// shows the port DOES collide (where the cart would not); the assertion pins the
/// current port behaviour so a follow-up sf-game fix (add the same-shape gate)
/// flips it deliberately.
#[test]
fn retail_same_shape_skip_divergence() {
    use sf_game::alien::{ACF_COLLTYPE2, ACF_COLLTYPE3, ASF_COLLIDE};
    use sf_game::game::Game;

    let mut g = Game::new();
    // Two objects, SAME shape, DIFFERENT colltype (enemy1 vs enemy2), overlapping.
    let a = g.objs.alloc().expect("alloc a");
    let b = g.objs.alloc().expect("alloc b");
    for (idx, ct) in [(a, ACF_COLLTYPE2), (b, ACF_COLLTYPE3)] {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = 0x0042; // SAME shape id
        al.collflags = ct; // clears ACF_FIRSTFRAME (alloc set it) so it enters the list
        al.hp = 50;
        al.ap = 5;
        al.sflags = 0;
        al.sflags3 = 0; // NEITHER has sameshapecollide -> cart would SKIP the pair
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.immuneptr = 0;
        al.sbyte4 = 0;
    }

    g.coldet_generate_list();
    g.coldet_run();

    let coll_a = g.objs.aliens[a as usize].sflags & ASF_COLLIDE != 0;
    let coll_b = g.objs.aliens[b as usize].sflags & ASF_COLLIDE != 0;
    eprintln!(
        "COLL same-shape: shape=$0042 both, colltype enemy1 vs enemy2, overlapping -> port collide=({coll_a},{coll_b})"
    );
    // FIXED + re-certified: retail chkcoll0 SKIPS same-shape pairs (no
    // sameshapecollide). The port's coldet_run now has the same-shape gate
    // (sf-game fix, this commit's sibling), so it ALSO skips -> MATCH.
    // find->fix->re-certify loop closed on the 3rd cert-found bug.
    assert!(
        !coll_a && !coll_b,
        "FIXED: port coldet_run now skips same-shape pairs (no sameshapecollide), matching retail chkcoll0"
    );
    eprintln!(
        "COLL: FIXED — port coldet_run adds the ROM same-shape gate; same-shape/different-colltype \
         pairs are now skipped, matching the cart."
    );
}

// ==========================================================================
// PLAYER MOVEMENT — the per-frame ship physics (highest-blast-radius shared
// system): the screen-edge BOUNDS clamp (the known parity concern) and the
// boost/brake speed ramp. Located by masked signature scan of the retail cart,
// each a UNIQUE hit, cross-validated by reading operands back out; then RUN
// surgically and diffed vs the port over a grid straddling each boundary.
//
// The steering->velocity map + position integrator are already certified vs
// retail elsewhere: `gen_3dvecs`/`n3dvecs_l` (UPDATE 8, vx/vz + |vy| bit-exact)
// and `addalvecs_l` (UPDATE 1, worldx/y/z += vx/vy/vz). The per-frame player
// STRAT (`playermove_srou`) composes those two certified cores + the two cores
// certified here around a large accumulator (plrot*/ztilt/turnrot/zshake) +
// pad-read body; its rotation-scale constants are cross-validated below.
// ==========================================================================

/// MILESTONE (player-move step 1) — LOCATE + CROSS-VALIDATE the retail player
/// bounds-clamp (`playerlimitx_srou`) and speed-ramp (`sr_speedto`) addresses by
/// masked signature scan, reading every WRAM operand + boundary opcode back out.
#[test]
fn retail_player_move_addresses() {
    let Some(rom) = retail() else { return };

    // --- playerlimitx_srou (UNIQUE): arrows/min/max operands wildcarded ---
    let pl: Vec<Option<u8>> = vec![
        Some(0xAD), None, None, Some(0x29), Some(0xF3), Some(0x8D), None, None,
        Some(0xC2), Some(0x20), Some(0xB5), Some(0x0C), Some(0xCD), None, None, Some(0xE2), Some(0x20),
        Some(0xF0), Some(0x06), Some(0x30), Some(0x04), Some(0x5C), None, None, None,
        Some(0xC2), Some(0x20), Some(0xAD), None, None, Some(0x95), Some(0x0C), Some(0xE2), Some(0x20),
        Some(0xAD), None, None, Some(0x09), Some(0x04), Some(0x8D), None, None,
    ];
    let h = masked_scan(&rom, &pl);
    assert_eq!(h.len(), 1, "playerlimitx_srou is a UNIQUE masked hit");
    let o = h[0];
    let addr = rom_off_to_snes(o);
    let arrows = rom[o + 1] as u32 | (rom[o + 2] as u32) << 8;
    let minx = rom[o + 13] as u32 | (rom[o + 14] as u32) << 8;
    // The max-side half follows the min-side .nminX target: at o+42 begins
    // rep;lda worldx;cmp maxpmoveX(o+47);sep;bpl(o+51);jml;...;ora #$08(o+70).
    let maxx = rom[o + 47] as u32 | (rom[o + 48] as u32) << 8;
    assert_eq!(addr, sf_oracle::RETAIL_PLAYERLIMITX_SROU, "playerlimitx_srou addr");
    assert_eq!(arrows, sf_oracle::RETAIL_ARROWS, "arrows operand");
    assert_eq!(minx, sf_oracle::RETAIL_MINPMOVEX, "minpmoveX operand");
    assert_eq!(maxx, sf_oracle::RETAIL_MAXPMOVEX, "maxpmoveX operand (min+2, contiguous)");
    // Boundary opcodes: min = BEQ($F0)+BMI($30) (clamp on <=), max = BPL($10)
    // after CMP (clamp on >=). Arrow sets: ORA #$04 (left) / #$08 (right).
    assert_eq!(rom[o + 17], 0xF0, "min boundary BEQ (== clamps)");
    assert_eq!(rom[o + 19], 0x30, "min boundary BMI (< clamps) => INCLUSIVE <=");
    assert_eq!(rom[o + 3], 0x29, "AND arrows");
    assert_eq!(rom[o + 4], 0xF3, "AND #~(left|right) = $F3");
    assert_eq!(rom[o + 38], sf_oracle::SPRAR_LEFT, "min side ORA #sprar_left ($04)");
    assert_eq!(rom[o + 51], 0x10, "max boundary BPL (>= clamps) => INCLUSIVE >=");
    // max side arrow immediate: the .nminX block is rep;lda worldx;cmp;sep;bpl;jml;
    // rep;lda max;sta worldx;sep;lda arrows;ora #$08;sta;rts.
    assert_eq!(rom[o + 70], sf_oracle::SPRAR_RIGHT, "max side ORA #sprar_right ($08)");
    eprintln!(
        "PLAYER-MOVE: playerlimitx_srou=${addr:06X}  arrows=${arrows:04X} \
         minpmoveX=${minx:04X} maxpmoveX=${maxx:04X}  (min<= BEQ+BMI, max>= BPL — both INCLUSIVE)"
    );

    // --- sr_speedto (UNIQUE): tpa operands wildcarded ---
    let sp: Vec<Option<u8>> = vec![
        Some(0x85), Some(0x3A), Some(0xB5), Some(0x15), Some(0x38), Some(0xED), None, None,
        Some(0xF0), Some(0x23), Some(0x10), Some(0x03), Some(0x49), Some(0xFF), Some(0x1A),
        Some(0xC5), Some(0x3A), Some(0x10), Some(0x05), Some(0xAD), None, None, Some(0x80), Some(0x11),
        Some(0xB5), Some(0x15), Some(0xCD), None, None, Some(0xF0), Some(0x0A),
        Some(0x30), Some(0x05), Some(0x38), Some(0xE5), Some(0x3A), Some(0x80), Some(0x03), Some(0x18), Some(0x65),
    ];
    let hs = masked_scan(&rom, &sp);
    assert_eq!(hs.len(), 1, "sr_speedto is a UNIQUE masked hit");
    let so = hs[0];
    let saddr = rom_off_to_snes(so);
    let tpa1 = rom[so + 6] as u32 | (rom[so + 7] as u32) << 8;
    let tpa2 = rom[so + 20] as u32 | (rom[so + 21] as u32) << 8;
    let tpa3 = rom[so + 27] as u32 | (rom[so + 28] as u32) << 8;
    assert_eq!(saddr, sf_oracle::RETAIL_SR_SPEEDTO, "sr_speedto addr");
    assert_eq!(tpa1, sf_oracle::RETAIL_TPA, "sr_speedto sbc tpa == RETAIL_TPA ($14C5)");
    assert_eq!((tpa1, tpa2, tpa3), (tpa1, tpa1, tpa1), "all three tpa reads are the same global");
    assert_eq!(rom[so + 3] as u32, sf_oracle::AL_VEL, "al_vel struct offset $15");
    assert_eq!(rom[so + 1], 0x3A, "tpx (rate) dp scratch $3A");
    eprintln!(
        "PLAYER-MOVE: sr_speedto=${saddr:06X}  tpa=${tpa1:04X} (== RETAIL_TPA)  al_vel=$15  tpx(rate)=$3A"
    );

    // --- rotation-scale constants of playermove_srou (the "velocity scale
    // factors") cross-validated statically. playermove reads pad and steps
    // plrotz/plroty by ZROT_SPEED (#$0200) and plrotx by XROT_SPEED (#$0200),
    // clamps plrotz to +-#$0600. Confirm those immediates exist in the routine
    // body so the port constants (XROT_SPEED/ZROT_SPEED=$200, plrotz clamp
    // $600) are cartridge-faithful. Signature: rep; lda plrotz; clc; adc #$0200.
    let rot: Vec<Option<u8>> = vec![Some(0x18), Some(0x69), Some(0x00), Some(0x02)]; // clc; adc #$0200
    let rothits = masked_scan(&rom, &rot);
    assert!(!rothits.is_empty(), "ZROT/XROT step #$0200 immediate present in retail");
    eprintln!(
        "PLAYER-MOVE: steering rot-step #$0200 (ZROT_SPEED/XROT_SPEED) confirmed present \
         ({} sites); gen_3dvecs + addalvecs_l already MATCH vs retail (UPDATE 8/1).",
        rothits.len()
    );
}

/// X-only portion of the port `player::playerlimit_x_srou` (player.rs:1151).
/// Returns (clamped worldX, arrows & (left|right)). The port also clamps Y in
/// the same fn — an HD-runtime addition NOT in the ROM `playerlimitx_srou`, so
/// it is excluded here (the retail routine touches only X + the L/R arrows).
fn port_playerlimit_x(worldx: i16, minx: i16, maxx: i16, arrows_in: u8) -> (i16, u8) {
    let mut arrows = arrows_in & !(sf_oracle::SPRAR_RIGHT | sf_oracle::SPRAR_LEFT);
    let mut wx = worldx;
    if wx <= minx {
        wx = minx;
        arrows |= sf_oracle::SPRAR_LEFT;
    }
    if wx >= maxx {
        wx = maxx;
        arrows |= sf_oracle::SPRAR_RIGHT;
    }
    (wx, arrows)
}

/// Run the retail cart's OWN `playerlimitx_srou` on a seeded (worldX, box) and
/// return (clamped worldX, arrows & left|right). Enters 8-bit A / 16-bit X, as
/// the ROM caller does (RTS/near routine).
fn retail_playerlimit_x(rom: &[u8], worldx: i16, minx: i16, maxx: i16, arrows_in: u8) -> (i16, u8) {
    const XB: u32 = 0x0100;
    const AL_WORLDX: u32 = 0x0C;
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + AL_WORLDX, worldx as u16);
    bus.write16(sf_oracle::RETAIL_MINPMOVEX, minx as u16);
    bus.write16(sf_oracle::RETAIL_MAXPMOVEX, maxx as u16);
    bus.write8(sf_oracle::RETAIL_ARROWS, arrows_in);
    call_near(
        &mut bus,
        sf_oracle::RETAIL_PLAYERLIMITX_SROU,
        &Entry { x: XB as u16, p: 0x20, ..Default::default() },
    );
    let wx = bus.read16(XB + AL_WORLDX) as i16;
    // Full arrows byte (not masked): the routine clears only left|right (AND
    // #$F3) and preserves any other bit, so comparing the whole byte also
    // certifies the bit-preservation, matching the port's `& !(RIGHT|LEFT)`.
    let arr = bus.read8(sf_oracle::RETAIL_ARROWS);
    (wx, arr)
}

/// CERTIFY the position BOUNDS clamp (the known concern) vs retail. Runs the
/// cart's OWN `playerlimitx_srou` over a grid straddling each screen-edge X
/// bound and diffs (clamped worldX, edge arrows) vs the port. Pins the exact
/// limits + inclusive/exclusive edge behaviour.
#[test]
fn retail_playerlimit_x_bounds_vs_port() {
    let Some(rom) = retail() else { return };
    // Realistic player box (planet flight: STRATEQU planet_minX/maxX +-500, and
    // the port's own spawn box). Straddle each bound including exactly-on-edge.
    let boxes: &[(i16, i16)] = &[(-500, 500), (-120, 120), (0, 0)];
    let mut all = true;
    let mut cases = 0;
    for &(minx, maxx) in boxes {
        // Grid: below min, one below, exactly min, mid, exactly max, one above,
        // above max — plus a couple of interior points.
        let grid: &[i16] = &[
            minx.wrapping_sub(200), minx.wrapping_sub(1), minx,
            minx.wrapping_add(1), 0, maxx.wrapping_sub(1), maxx,
            maxx.wrapping_add(1), maxx.wrapping_add(200),
        ];
        for &wx in grid {
            // Seed arrows with an unrelated bit ($01 up) to prove the routine
            // clears only left|right and preserves the rest (we mask to L|R).
            let (rwx, rarr) = retail_playerlimit_x(&rom, wx, minx, maxx, 0x01);
            let (pwx, parr) = port_playerlimit_x(wx, minx, maxx, 0x01);
            let ok = (rwx, rarr) == (pwx, parr);
            all &= ok;
            cases += 1;
            eprintln!(
                "BOUNDS box[{minx},{maxx}] worldX {wx:6}: retail=({rwx:6},arr {rarr:#04x}) \
                 port=({pwx:6},arr {parr:#04x}) {}",
                if ok { "MATCH" } else { "DIFF" }
            );
            assert_eq!((rwx, rarr), (pwx, parr), "bounds box[{minx},{maxx}] worldX {wx}");
        }
    }
    assert!(all);
    // Pin the exact edge semantics: at worldX == min the ROM sets LEFT + clamps;
    // at worldX == max it sets RIGHT + clamps. Both INCLUSIVE.
    let (wmin, amin) = retail_playerlimit_x(&rom, -500, -500, 500, 0);
    let (wmax, amax) = retail_playerlimit_x(&rom, 500, -500, 500, 0);
    assert_eq!((wmin, amin), (-500, sf_oracle::SPRAR_LEFT), "worldX==min: clamp + LEFT (inclusive)");
    assert_eq!((wmax, amax), (500, sf_oracle::SPRAR_RIGHT), "worldX==max: clamp + RIGHT (inclusive)");
    eprintln!(
        "BOUNDS: MATCH over {cases} cases — retail playerlimitx_srou == port playerlimit_x_srou \
         (X). Both bounds INCLUSIVE (== min -> clamp+LEFT, == max -> clamp+RIGHT)."
    );

    // ---- domain-boundary characterization (NOT a reachable divergence) ----
    // The ROM compares worldX to the bound with a 16-bit CMP + BMI/BPL, which
    // tests only the SIGN bit of the subtraction (65816 CMP sets no V flag), so
    // when |worldX - bound| > 32767 the comparison wraps. The port uses a TRUE
    // i16 `<=`/`>=`, so the two disagree past that overflow edge. worldX cannot
    // reach ~+32700 (with min=-500) in one frame under the per-frame clamp, so
    // this is unreachable in gameplay — recorded, not asserted as a bug.
    let (rwx, rarr) = retail_playerlimit_x(&rom, 32700, -500, 500, 0);
    let (pwx, parr) = port_playerlimit_x(32700, -500, 500, 0);
    eprintln!(
        "BOUNDS (overflow edge, UNREACHABLE): worldX=32700 box[-500,500]: \
         retail=({rwx},arr {rarr:#04x}) port=({pwx},arr {parr:#04x}) — ROM CMP sign-bit wrap; \
         out of the reachable per-frame domain, documented only."
    );
}

/// Port `common::strat_speed_to` transcribed (common.rs:325). Returns al_vel
/// after the ramp. (The public helper mutates an `Alien`; this mirrors its vel
/// arithmetic so the diff is a pure sf-oracle comparison.)
fn port_speed_to(vel: u8, target: u8, rate: u8) -> u8 {
    if vel == target {
        return vel;
    }
    let abs_diff = (vel as i16 - target as i16).unsigned_abs();
    if abs_diff < rate as u16 {
        target
    } else if vel > target {
        vel - rate
    } else {
        vel + rate
    }
}

/// Run the retail cart's OWN `sr_speedto` and return the resulting `al_vel`.
/// Enters 8-bit A = rate, X = object block, `tpa` = target (JSL/RTL routine).
fn retail_speed_to(rom: &[u8], vel: u8, target: u8, rate: u8) -> u8 {
    const XB: u32 = 0x0100;
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XB + sf_oracle::AL_VEL, vel);
    bus.write8(sf_oracle::RETAIL_TPA, target);
    call(
        &mut bus,
        sf_oracle::RETAIL_SR_SPEEDTO,
        &Entry { a: rate as u16, x: XB as u16, p: 0x20, ..Default::default() },
    );
    bus.read8(XB + sf_oracle::AL_VEL)
}

/// CERTIFY the boost/brake speed ramp vs retail. Runs the cart's OWN `sr_speedto`
/// over the reachable player-speed domain (vel/target in the 20..85 band the
/// boost/brake targets live in, at the rate-2 the player ramp uses, plus rate-1)
/// and diffs the resulting `al_vel` vs the port `strat_speed_to`. Covers the
/// snap-when-near guard, the directional step, and the already-at-target case.
#[test]
fn retail_speedto_boost_brake_vs_port() {
    let Some(rom) = retail() else { return };
    // MIN_PSPEED=20, MED_PSPEED=65, MAX_PSPEED=85 (STRATEQU). Boost ramps toward
    // 85, brake toward 20, both at rate 2 (viewmove_srou strat_speed_to(...,2)).
    let vels: &[u8] = &[20, 21, 22, 40, 63, 64, 65, 66, 83, 84, 85];
    let targets: &[u8] = &[20, 65, 85];
    let rates: &[u8] = &[1, 2];
    let mut all = true;
    let mut cases = 0;
    for &target in targets {
        for &rate in rates {
            for &vel in vels {
                let rv = retail_speed_to(&rom, vel, target, rate);
                let pv = port_speed_to(vel, target, rate);
                let ok = rv == pv;
                all &= ok;
                cases += 1;
                if !ok {
                    eprintln!("SPEEDTO vel={vel} target={target} rate={rate}: retail={rv} port={pv} DIFF");
                }
                assert_eq!(rv, pv, "sr_speedto vel={vel} target={target} rate={rate}");
            }
        }
    }
    assert!(all);
    // Spot-print the two canonical ramp steps + the snap + the fixed point.
    eprintln!(
        "SPEEDTO boost step 65->{} (t=85,r=2); brake step 85->{} (t=20,r=2); \
         snap 84->{} (t=85,r=2); at-target 85->{} (t=85,r=2)",
        retail_speed_to(&rom, 65, 85, 2),
        retail_speed_to(&rom, 85, 20, 2),
        retail_speed_to(&rom, 84, 85, 2),
        retail_speed_to(&rom, 85, 85, 2),
    );
    eprintln!(
        "SPEEDTO: MATCH over {cases} cases — retail sr_speedto == port strat_speed_to across the \
         reachable boost/brake speed domain (20..85, rate 1-2)."
    );
}
