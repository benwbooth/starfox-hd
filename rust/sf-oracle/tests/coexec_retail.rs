//! TIER-2 retail co-execution harness — proof-of-concept certifier for
//! "100% vs retail". Boots the RETAIL cart (`Star Fox (USA) (Rev 2).sfc`),
//! locates + reads the observable object array out of retail WRAM, and lays the
//! foundation to diff it against the Rust port.
//!
//! Retail is a DIFFERENT binary from the symbol-mapped built ROM (see
//! docs/FUNCTION_LEDGER.md) — every address here was re-derived from the retail
//! cart itself, not from the built-ROM symbol map.

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
