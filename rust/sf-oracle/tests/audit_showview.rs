//! Differential audit: the showview per-object flag/cull pass.
//!
//! ROM truth (built ROM + symbols): the retail pipeline is
//!   showview_l  (MAIN.ASM:2001)  = marioshowview (MAIN.ASM:2097, copies
//!                world - viewpos into the GSU drawlist) + mallrotzsort
//!                (MDRAWLIS.MC:1399, rotates dl_x/y/z by the view matrix
//!                and computes shadow coords on the Super FX)
//!   alienflags_l (MAIN.ASM:2009) = per-object flag/cull pass over the
//!                *rotated* drawlist, run after the GSU frame
//!                (RAMSTUFF.ASM:70). This is the routine under test.
//!
//! alienflags_l per visible (non-invisible) alien, walking allst and the
//! drawlist in lockstep:
//!   * behind test: sh_zmax(shape) + dl_z(rotated view z) < 0 (MAIN.ASM:2030-36)
//!   * behind    -> clear affrontpl; then free the alien iff
//!                  !gf_nozremove && !acf_firstframe && (al_type & atzremove)
//!                  (MAIN.ASM:2038-2062)
//!   * survivor  -> ora afinviewpl, plus afleftpl iff dl_x (rotated view x) < 0
//!                  (MAIN.ASM:2065-2077)
//!   * invisible aliens (al_sflags4 & 8) are skipped entirely: no flag
//!     update, never z-removed, and their drawlist slot is not consumed
//!     (MAIN.ASM:2020-22 with marioshowview:2103-05 skipping emission).
//!
//! Rust port under audit: sf-game/src/draw.rs `build_list`.
//! The Rust side is fed an identity camera (viewpos = 0, no rotation) so
//! its world coords line up with the ROM's rotated drawlist values.
//!
//! NOTE on the +1: the harness stub's CLC/XCE leaves carry=1 at JSL, and
//! alienflags_l's `adc dl_z,x` has no CLC (MAIN.ASM:2029-30 quirk: carry is
//! whatever the caller left), so in these tests the behind test is
//! sh_zmax + dl_z + 1 < 0. All test values keep a >=2 margin from zero.

use sf_game::draw::{self, AF_FRONT_PL, AF_INVIEW_PL, AF_LEFT_PL};
use sf_game::obj::Objects;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

// Direct-page / WRAM symbols (data/symbols.txt).
const ALLST: u32 = 0x12ad;
const ALFREELST: u32 = 0x12af;
const GAMEFLAGS: u32 = 0x155b;

// Alien struct offsets.
const AL_NEXT: u32 = 0x00;
const AL_SHAPE: u32 = 0x04;
const AL_FLAGS: u32 = 0x08;
const AL_TYPE: u32 = 0x09;
const AL_SFLAGS4: u32 = 0x20;
const AL_COLLFLAGS: u32 = 0x2e;

// ROM flag values (symbols.txt; match the Rust AF_* in draw.rs).
const AFFRONTPL: u8 = 0x08;
const AFINVIEWPL: u8 = 0x10;
const AFLEFTPL: u8 = 0x04;
const ATZREMOVE: u8 = 0x08;
const ACF_FIRSTFRAME: u8 = 0x04;
const ASF4_INVISIBLE: u8 = 0x08; // ROM asf_invisible lives in al_sflags4
const GF_NOZREMOVE: u8 = 0x01;

// Drawlist (GSU cart RAM bank $70): dl_x at $700012+x, dl_z at $700014+x,
// with x starting at m_drawlist&WM = $1960 and advancing dl_sizeof = $1e.
const DL_BASE: u32 = 0x1960;
const DL_X: u32 = 0x70_0012;
const DL_Z: u32 = 0x70_0014;
const DL_SIZEOF: u32 = 0x1e;

// WRAM hosting for the test fixtures.
const OBJ: [u32; 2] = [0x0500, 0x0540]; // al_size = $38, spaced $40
const SHAPE_HDR: u32 = 0x0480; // long read $00:048E hits WRAM (sh_zmax = $0e)
const SH_ZMAX: u32 = 0x0e;

struct Obj {
    flags: u8,
    type_: u8,
    collflags: u8,
    sflags4: u8,
    /// Rotated view-space x/z this object's drawlist slot would hold.
    /// `None` = invisible object (no drawlist slot emitted).
    dl: Option<(i16, i16)>,
}

struct RomOut {
    flags: Vec<u8>,
    /// Objects still on allst after the pass, by fixture index.
    alive: Vec<bool>,
}

fn rom_pass(rom: &[u8], addr: u32, gameflags: u8, zmax: i16, objs: &[Obj]) -> RomOut {
    assert!(objs.len() <= OBJ.len());
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(GAMEFLAGS, gameflags);
    bus.write16(SHAPE_HDR + SH_ZMAX, zmax as u16);
    bus.write16(ALLST, OBJ[0] as u16);
    bus.write16(ALFREELST, 0);
    let mut slot = 0u32;
    for (i, o) in objs.iter().enumerate() {
        let a = OBJ[i];
        let next = if i + 1 < objs.len() {
            OBJ[i + 1] as u16
        } else {
            0
        };
        bus.write16(a + AL_NEXT, next);
        bus.write16(a + AL_SHAPE, SHAPE_HDR as u16);
        bus.write8(a + AL_FLAGS, o.flags);
        bus.write8(a + AL_TYPE, o.type_);
        bus.write8(a + AL_SFLAGS4, o.sflags4);
        bus.write8(a + AL_COLLFLAGS, o.collflags);
        if let Some((x, z)) = o.dl {
            assert_eq!(o.sflags4 & ASF4_INVISIBLE, 0, "invisible objs emit no slot");
            let xreg = DL_BASE + slot * DL_SIZEOF;
            bus.write16(DL_X + xreg, x as u16);
            bus.write16(DL_Z + xreg, z as u16);
            slot += 1;
        } else {
            assert_ne!(o.sflags4 & ASF4_INVISIBLE, 0, "visible objs need a slot");
        }
    }
    call(&mut bus, addr, &Entry::default());

    // Walk allst to see who survived.
    let mut alive = vec![false; objs.len()];
    let mut p = bus.wram_read16(ALLST);
    let mut guard = 0;
    while p != 0 && guard < 16 {
        if let Some(i) = OBJ.iter().position(|&a| a == p as u32) {
            alive[i] = true;
        }
        p = bus.read16(p as u32 + AL_NEXT);
        guard += 1;
    }
    RomOut {
        flags: (0..objs.len())
            .map(|i| bus.read8(OBJ[i] as u32 + AL_FLAGS))
            .collect(),
        alive,
    }
}

/// Run the Rust port on the equivalent identity-camera scene: worldx/z equal
/// to the ROM's rotated dl_x/dl_z, viewpos = 0. Returns (flags, alive).
fn rust_pass(gameflags: u8, objs: &[Obj]) -> (Vec<u8>, Vec<bool>) {
    let mut o = Objects::init();
    // Objects::alloc pushes to the head of the active list, so allocate in
    // reverse to make fixture order match ROM list order.
    let mut ids = vec![0u16; objs.len()];
    for i in (0..objs.len()).rev() {
        ids[i] = o.alloc().unwrap();
    }
    for (i, spec) in objs.iter().enumerate() {
        let al = &mut o.aliens[ids[i] as usize];
        al.shape = 1;
        al.flags = spec.flags;
        al.type_ = spec.type_;
        al.collflags = spec.collflags;
        al.sflags4 = if spec.sflags4 & ASF4_INVISIBLE != 0 {
            sf_game::alien::ASF4_INVISIBLE
        } else {
            0
        };
        let (x, z) = spec.dl.unwrap_or((0, 0));
        al.worldx = x;
        al.worldz = z;
    }
    let mut out = Vec::new();
    // Extents provider mirrors the ROM fixture's shape header: sh_zmax = 100
    // (the same per-shape source coldet uses supplies the cull margin).
    draw::build_list(
        &mut o,
        0,
        0,
        draw::CullView::default(),
        gameflags,
        &|_| Some((100, 100, 100)),
        &mut out,
    );
    (
        ids.iter().map(|&i| o.aliens[i as usize].flags).collect(),
        ids.iter().map(|&i| o.aliens[i as usize].active).collect(),
    )
}

fn setup() -> Option<(u32, Vec<u8>)> {
    let syms = load_symbols();
    let rom = load_built_rom()?;
    let addr = *syms.get("ALIENFLAGS_L")?;
    Some((addr, rom))
}

fn obj(flags: u8, type_: u8, collflags: u8, sflags4: u8, dl: Option<(i16, i16)>) -> Obj {
    Obj {
        flags,
        type_,
        collflags,
        sflags4,
        dl,
    }
}

/// In-front objects: afinviewpl always set, afleftpl iff rotated view x < 0.
/// The Rust port agrees under an identity camera (dl_x == worldx - viewposx).
#[test]
fn leftpl_is_sign_of_rotated_view_x() {
    let Some((addr, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    for (x, want_left) in [(-500i16, true), (-2, true), (2, false), (500, false)] {
        let objs = [obj(AFFRONTPL, ATZREMOVE, 0, 0, Some((x, 1000)))];
        let r = rom_pass(&rom, addr, 0, 100, &objs);
        let want = AFFRONTPL | AFINVIEWPL | if want_left { AFLEFTPL } else { 0 };
        assert!(r.alive[0], "x={x}: in-front object must survive");
        assert_eq!(r.flags[0], want, "ROM flags for dl_x={x}");

        // Rust port, identity camera: must agree bit-for-bit.
        let (rf, ra) = rust_pass(0, &objs);
        assert!(ra[0]);
        assert_eq!(
            rf[0],
            AF_FRONT_PL | AF_INVIEW_PL | if want_left { AF_LEFT_PL } else { 0 },
            "Rust flags for worldx={x}"
        );
    }
}

/// ROM keeps an object whose rotated z is negative as long as
/// z + sh_zmax >= 0 (the shape's own extent is the cull margin), and such an
/// object keeps affrontpl and still gets afinviewpl/afleftpl.
///
/// (Was a KNOWN DIVERGENCE: draw.rs used to cull at worldz - pviewposz < 0
/// with no shape margin. Fixed: the cull now anchors on the camera viewpos
/// and adds the shape's z half-extent as the margin.)
#[test]
fn behind_test_uses_shape_zmax_margin() {
    let Some((addr, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    // Rotated z = -50, shape zmax = 100 -> -50 + 100 (+1 carry) > 0: in front.
    let objs = [obj(AFFRONTPL, ATZREMOVE, 0, 0, Some((-500, -50)))];
    let r = rom_pass(&rom, addr, 0, 100, &objs);
    assert!(
        r.alive[0],
        "ROM keeps z=-50 with zmax=100 (margin not crossed)"
    );
    assert_eq!(r.flags[0], AFFRONTPL | AFINVIEWPL | AFLEFTPL);

    // Rust port, identity camera: same zmax margin -> kept, same flags.
    let (rf, ra) = rust_pass(0, &objs);
    assert!(ra[0], "Rust keeps z=-50 with zmax=100 (margin not crossed)");
    assert_eq!(rf[0], AF_FRONT_PL | AF_INVIEW_PL | AF_LEFT_PL);
}

/// Fully behind (z + zmax < 0): freed iff atzremove && !firstframe &&
/// !gf_nozremove. Survivors get affrontpl CLEARED and afinviewpl set.
#[test]
fn behind_kill_conditions_and_frontpl_clear() {
    let Some((addr, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let behind = Some((300i16, -2000i16)); // -2000 + 100 << 0

    // (a) atzremove, no firstframe, zremove enabled -> freed.
    let r = rom_pass(
        &rom,
        addr,
        0,
        100,
        &[obj(AFFRONTPL, ATZREMOVE, 0, 0, behind)],
    );
    assert!(!r.alive[0], "ROM frees behind atzremove object");

    // (b) gf_nozremove -> survives, but affrontpl cleared, inview set.
    let r = rom_pass(
        &rom,
        addr,
        GF_NOZREMOVE,
        100,
        &[obj(AFFRONTPL, ATZREMOVE, 0, 0, behind)],
    );
    assert!(r.alive[0]);
    assert_eq!(
        r.flags[0], AFINVIEWPL,
        "ROM clears affrontpl on behind survivors (MAIN.ASM:2038-41)"
    );

    // (c) acf_firstframe -> survives with affrontpl cleared.
    let r = rom_pass(
        &rom,
        addr,
        0,
        100,
        &[obj(AFFRONTPL, ATZREMOVE, ACF_FIRSTFRAME, 0, behind)],
    );
    assert!(r.alive[0]);
    assert_eq!(r.flags[0], AFINVIEWPL);

    // (d) no atzremove -> survives with affrontpl cleared.
    let r = rom_pass(&rom, addr, 0, 100, &[obj(AFFRONTPL, 0, 0, 0, behind)]);
    assert!(r.alive[0]);
    assert_eq!(r.flags[0], AFINVIEWPL);

    // Rust port (was a KNOWN DIVERGENCE: AF_FRONT_PL used to stay set on
    // behind survivors). Now matches the ROM: behind clears AF_FRONT_PL on
    // ANY behind object, survivors still get AF_INVIEW_PL.
    let (rf, ra) = rust_pass(GF_NOZREMOVE, &[obj(AFFRONTPL, ATZREMOVE, 0, 0, behind)]);
    assert!(ra[0]);
    assert_eq!(
        rf[0], AF_INVIEW_PL,
        "Rust clears AF_FRONT_PL on behind survivors (ROM parity)"
    );

    // And the free path itself: behind + atzremove + no firstframe + zremove
    // enabled -> freed, like ROM case (a).
    let (_, ra) = rust_pass(0, &[obj(AFFRONTPL, ATZREMOVE, 0, 0, behind)]);
    assert!(!ra[0], "Rust frees behind atzremove objects");
}

/// Invisible objects are skipped before the behind test: never z-removed,
/// flags untouched, and they do not consume a drawlist slot (the next
/// visible alien reads the slot the invisible one would have used).
///
/// (Was a KNOWN DIVERGENCE: the Rust port used to cull before the invisible
/// check, freeing behind invisible atzremove objects. Fixed: invisible skip
/// now runs before the behind test, like alienflags_l.)
#[test]
fn invisible_objects_skip_cull_and_keep_flags() {
    let Some((addr, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    // A: invisible, atzremove, would be far behind if it had a slot.
    // B: visible, in front, rotated x negative -> leftpl; reads slot 0.
    let objs = [
        obj(AFFRONTPL, ATZREMOVE, 0, ASF4_INVISIBLE, None),
        obj(AFFRONTPL, ATZREMOVE, 0, 0, Some((-500, 1000))),
    ];
    let r = rom_pass(&rom, addr, 0, 100, &objs);
    assert!(r.alive[0], "ROM never z-removes invisible objects");
    assert_eq!(
        r.flags[0], AFFRONTPL,
        "ROM leaves invisible flags untouched"
    );
    assert!(r.alive[1]);
    assert_eq!(
        r.flags[1],
        AFFRONTPL | AFINVIEWPL | AFLEFTPL,
        "visible alien after an invisible one reads drawlist slot 0"
    );

    // Rust: an invisible object with worldz far behind the camera must NOT
    // be freed (invisible skip runs before the behind test, ROM parity).
    let mut o = Objects::init();
    let id = o.alloc().unwrap();
    {
        let al = &mut o.aliens[id as usize];
        al.shape = 1;
        al.flags = AFFRONTPL;
        al.type_ = ATZREMOVE;
        al.collflags = 0; // clear the alloc-time firstframe protection
        al.sflags4 = sf_game::alien::ASF4_INVISIBLE;
        al.worldz = -2000;
    }
    let mut out = Vec::new();
    draw::build_list(
        &mut o,
        0,
        0,
        draw::CullView::default(),
        0,
        &|_| Some((100, 100, 100)),
        &mut out,
    );
    assert!(
        o.aliens[id as usize].active,
        "invisible objects skip the behind cull (ROM parity)"
    );
    // marioshowview's placement-flag rewrite runs before the invisible skip,
    // so the flags read affrontpl — the same value the ROM leaves in place.
    assert_eq!(o.aliens[id as usize].flags, AF_FRONT_PL);
    assert!(out.is_empty(), "invisible objects emit no draw entry");
}
