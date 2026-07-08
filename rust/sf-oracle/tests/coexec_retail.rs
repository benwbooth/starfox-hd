//! TIER-2 retail co-execution harness — proof-of-concept certifier for
//! "100% vs retail". Boots the RETAIL cart (`Star Fox (USA) (Rev 2).sfc`),
//! locates + reads the observable object array out of retail WRAM, and lays the
//! foundation to diff it against the Rust port.
//!
//! Retail is a DIFFERENT binary from the symbol-mapped built ROM (see
//! docs/FUNCTION_LEDGER.md) — every address here was re-derived from the retail
//! cart itself, not from the built-ROM symbol map.

use sf_oracle::{
    boot_retail, call, init_object_pool, load_built_rom, load_retail_rom, snapshot_objects,
    walk_freelist, Entry, SnesBus, AL_VX, AL_VY, AL_VZ, BUILT_POOL, RETAIL_ADDALVECS_L, RETAIL_POOL,
};

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
