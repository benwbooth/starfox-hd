//! Boss-lane / shared-init accuracy audit against the built ROM.
//!
//! The boss parity harness (`sf-strat/tests/bo_parity.rs`) diverges on the very
//! first dumped row — the *player* object (slot 0), which every spawn routes
//! through the shared `init_objvars_l` ($1FD386). The C fixtures show
//! `flags=0 type=0 sflags3=0 colframe=255 animframe=255`; the Rust port's
//! `strat_init_obj_vars` was updated this session to `flags=0x10 type=8
//! sflags3=8 colframe=0 animframe=0`. This test asks the *real ROM* which is
//! correct by executing `init_objvars_l` over a poisoned alien block and
//! reading the struct back.
//!
//! Alien struct offsets (STRUCTS.INC + GILESAL.INC; X/Y = block base):
//!   flags 0x08, type 0x09, sflags3 0x1F, collflags 0x2E.
//! Flag bits (VARS.INC / STRATEQU.INC): afInviewPl=0x10, atzremove=0x08,
//!   acf_firstframe=0x04, realobj (sflags3) = 0x08.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

// Object block base. 0x0200/0x0300 are reserved by the oracle harness (stub /
// RTS trap), so keep the block clear of them.
const OBJ: u32 = 0x0100;
const AL_FLAGS: u32 = 0x08;
const AL_TYPE: u32 = 0x09;
const AL_SFLAGS3: u32 = 0x1F;
const AL_COLLFLAGS: u32 = 0x2E;

struct InitResult {
    flags: u8,
    type_: u8,
    sflags3: u8,
    collflags: u8,
}

fn rom_init_objvars(rom: &[u8], addr: u32) -> InitResult {
    let mut bus = SnesBus::new(rom.to_vec());
    // Poison the whole block so we can see exactly what init clears / sets.
    for off in 0..0x60u32 {
        bus.write8(0x7E_0000 | (OBJ + off), 0xFF);
    }
    // init_objvars_l is a8i16 with Y = ptr to object.
    call(
        &mut bus,
        addr,
        &Entry {
            y: OBJ as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    InitResult {
        flags: bus.read8(0x7E_0000 | (OBJ + AL_FLAGS)),
        type_: bus.read8(0x7E_0000 | (OBJ + AL_TYPE)),
        sflags3: bus.read8(0x7E_0000 | (OBJ + AL_SFLAGS3)),
        collflags: bus.read8(0x7E_0000 | (OBJ + AL_COLLFLAGS)),
    }
}

#[test]
fn init_objvars_sets_inviewpl_atzremove_firstframe_realobj() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("INIT_OBJVARS_L"), load_built_rom()) else {
        eprintln!("skip: no INIT_OBJVARS_L symbol / built ROM");
        return;
    };

    let r = rom_init_objvars(&rom, addr);
    eprintln!(
        "ROM init_objvars: flags={:#04x} type={:#04x} sflags3={:#04x} collflags={:#04x}",
        r.flags, r.type_, r.sflags3, r.collflags
    );

    // Ground truth per STRATROU.ASM init_objvars_l:
    //   s_set_alflag inviewpl      -> flags |= 0x10
    //   s_setremove_behind         -> type  |= atzremove (0x08)
    //   s_set_alcollflag firstframe-> collflags |= 0x04
    //   s_set_alsflag realobj      -> sflags3 |= 0x08
    // Everything else in the block is zeroed.
    assert_eq!(r.flags, 0x10, "afInviewPl must be set (and only that)");
    assert_eq!(r.type_, 0x08, "atzremove must be set (type_ is 8, NOT 0)");
    assert_eq!(r.sflags3, 0x08, "realobj must be set");
    assert_eq!(r.collflags, 0x04, "acf_firstframe must be set");
}
