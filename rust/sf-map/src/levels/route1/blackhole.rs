//! MAP_ID_BLACKHOLE — the Black Hole (Out of this Dimension gateway).
//!
//! C oracle: `src/map/levels.c` `build_level_bh_slice()`. No register
//! function: the black hole map registers no native or inline callbacks.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL_BH.ASM` — level wrapper: `initlevel hole,0`, jsr blackholemap.
//! - `BHOLE.ASM`    — the blackholemap subroutine: mother1 asteroid
//!   streams, iris/item drops, and the three warp-exit gates.

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

// Local constants from levels.c not yet in consts.rs.
// TODO(consolidation): move to consts.rs
mod lc {
    /// levels.c `#define SH_MOTHER1 278`
    pub const SH_MOTHER1: u16 = 278;
    /// levels.c `#define SH_IRIS 4`
    pub const SH_IRIS: u16 = 4;
    /// levels.c `#define SH_PARA_0 60`
    pub const SH_PARA_0: u16 = 60;
    /// levels.c `#define SH_SHIELDR 203`
    pub const SH_SHIELDR: u16 = 203;

    /// mother1_istrat (was the colliding 0x020000 = synth istrat 0/player).
    pub const STRAT_ADDR_MOTHER1: u32 = crate::consts::STRAT_ADDR_MOTHER1;
    /// mother2_istrat (was the colliding 0x020001 = synth istrat 1).
    pub const STRAT_ADDR_MOTHER2: u32 = crate::consts::STRAT_ADDR_MOTHER2;

    /// levels.c `#define IS_UP1MAN 90`
    pub const IS_UP1MAN: u32 = 90;
    /// levels.c `#define IS_IRIS 48`
    pub const IS_IRIS: u32 = 48;
    /// levels.c `#define IS_SHOU0A 179`
    pub const IS_SHOU0A: u32 = 179;
    /// levels.c `#define IS_BHOLEEXIT1 244`
    pub const IS_BHOLEEXIT1: u32 = 244;
    /// levels.c `#define IS_BHOLEEXIT2 245`
    pub const IS_BHOLEEXIT2: u32 = 245;
    /// levels.c `#define IS_BHOLEEXIT3 246`
    pub const IS_BHOLEEXIT3: u32 = 246;
}

/// C `build_level_bh_slice()` — no callback registrations at all.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();
    let mm = crate::mothers::mother_maps();

    // LEVEL_BH.ASM: initlevel hole,0
    // mapjsr blackholemap
    b.mapjsr("blackhole.blackholemap");
    b.mapend(1);

    // BHOLE.ASM — blackholemap subroutine
    b.label("blackhole.blackholemap");

    // Line 5: mapwait 2000
    b.mapwait(2000);

    // Line 7: mapmother 08000,0000,0,5000,mother1,mother1_istrat,map_bhole
    // (C passes a literal 0 map_ref — no label fixup involved)
    b.mapmother(0x8000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 8: maprem mother1
    b.mapremove(lc::SH_MOTHER1);
    // Line 9: mapobj 1000,000,00,5000,nullshape,up1man_Istrat
    b.mapobj(1000, 0, 0, 5000, sh::NULLSHAPE, lc::IS_UP1MAN);

    // .bhole — loop target
    b.label("blackhole.bhole");

    // Line 11: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 12: cspecial 4000,0000,0000,4500,zaco_0,shou0a_istrat
    b.cspecial(4000, 0, 0, 4500, sh::NULLSHAPE, lc::IS_SHOU0A);
    // Line 13: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Line 14: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    b.mapobj(200, 0, 0, 4000, lc::SH_IRIS, lc::IS_IRIS);
    // Line 15: mapobj 0000,000,000,4000,item_7,item7_ISTRAT
    b.mapobj(0, 0, 0, 4000, sh::ITEM_7, is::ITEM7);
    // Line 16: setalvar sbyte1,1
    b.setalvarb(al::SBYTE1, 1);

    // Line 17: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 18: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Lines 20-21: exitgate2_4
    b.mapobj(0, 0x0100, 0, 5400, sh::GATE_0, lc::IS_BHOLEEXIT2);
    b.pathobj(4500, 3000, 3000, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);

    // Line 22: mapmother 04000,0000,0,4000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 4000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 23: special 1000,0000,0000,4500,para_0,shou0a_istrat
    b.special(1000, 0, 0, 4500, lc::SH_PARA_0, lc::IS_SHOU0A);
    // Line 24: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Line 25: mapmother 0400,0000,0000,4000,mother1,mother2_istrat,map_amoebas
    b.mapmother(0x0400, 0, 0, 4000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER2, mm.map_amoebas);
    // Line 26: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Line 27: mapmother 02000,0000,0,5000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x2000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 28: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Line 29: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    b.mapobj(200, 0, 0, 4000, lc::SH_IRIS, lc::IS_IRIS);
    // Line 30: mapobj 0000,000,000,4000,item_5,item5_ISTRAT
    b.mapobj(0, 0, 0, 4000, sh::ITEM_5, is::ITEM5);
    // Line 31: setalvar sbyte1,1
    b.setalvarb(al::SBYTE1, 1);

    // Line 32: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 33: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Lines 35-36: exitgate3_4
    b.mapobj(0, -0x0200, -100, 5400, sh::GATE_0, lc::IS_BHOLEEXIT3);
    b.pathobj(4500, 3000, 3000, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);

    // Line 37: mapmother 04000,0000,0,4000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 4000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 38: special 3000,0000,0000,4500,shieldr,shou0a_istrat
    b.special(3000, 0, 0, 4500, lc::SH_SHIELDR, lc::IS_SHOU0A);
    // Line 39: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Line 40: mapobj 0200,000,000,4000,iris,iris_ISTRAT
    b.mapobj(200, 0, 0, 4000, lc::SH_IRIS, lc::IS_IRIS);
    // Line 41: mapobj 0000,000,000,4000,item_5,item5_ISTRAT
    b.mapobj(0, 0, 0, 4000, sh::ITEM_5, is::ITEM5);
    // Line 42: setalvar sbyte1,1
    b.setalvarb(al::SBYTE1, 1);

    // Line 43: mapmother 04000,0000,0,5000,mother1,mother1_istrat,map_bhole
    b.mapmother(0x4000, 0, 0, 5000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, mm.map_bhole);
    // Line 44: maprem mother1
    b.mapremove(lc::SH_MOTHER1);

    // Lines 46-47: exitgate1_5
    b.mapobj(0, 0x0200, 0x0100, 5400, sh::GATE_0, lc::IS_BHOLEEXIT1);
    b.pathobj(4500, 3000, 3000, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);

    // Line 48: mapgoto .bhole
    b.mapgoto("blackhole.bhole");

    // Line 49: maprts
    b.maprts();

    b.resolve();

    let (data, labels) = b.finish();

    Route1Level {
        level: BuiltLevel {
            data,
            labels,
            native_callbacks: vec![],
            inline_callbacks: vec![],
        },
        native_regs: vec![],
        inline_regs: vec![],
    }
}
