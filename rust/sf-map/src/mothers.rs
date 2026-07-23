//! Mothermap sub-map bytecode data — port of `MAPS/MOTHERS.ASM`.
//!
//! A "mother" object (spawned by the map-VM `mapmother` opcode) carries a
//! pointer (`al_ptr`) into this blob. Every strategy tick, `bemother_l`
//! (ASM/MOTHER.ASM) interprets entries at that pointer, spawning child
//! objects relative to the mother's position. The interpreter itself lives
//! in the strat lane (`sf-strat::mother::bemother`); this module owns only
//! the DATA, encoded byte-identically to the ROM structures
//! (MAPSTRUC.INC:39-93):
//!
//! entry = `[moth_ctrl u8][moth_count u16][payload...]`
//!   - `moth_count` is the wait AFTER the entry executes, in world-Z units
//!     (`al_sword1 -= lastzchange` per tick, MOTHER.ASM:73-75).
//!   - `motherobj`/`motherrnd` payload: x,y,z (i16), shape (u16),
//!     strat (u24) = 11 bytes (mo_sizeof 14 total).
//!   - `mothergoto` payload: target offset (u16).
//!   - `motherloop` payload: target offset (u16), count (u8).
//!   - `mothercnt` payload: shape (u16); `motherjump`: val u16, addr u16,
//!     func u8; `motherwait`: none; `motherend`: ctrl byte only (the ROM
//!     macro emits no count word, MAPMACS.INC:639-641).
//!
//! Goto/loop targets are offsets into [`MotherMaps::blob`]. Offset 0 is a
//! pad byte so `al_ptr == 0` can keep meaning "no mothermap".
//!
//! Compact-table and ROM compatibility entries retain their encoded strategy
//! operands. Native Rust strategies use distinct control bytes and carry a
//! typed [`DirectStrategy`] identity, so the two namespaces cannot collide.

use crate::consts::{
    is, sh, StrategyRef, STRATEGY_CLASTEROID, STRATEGY_DAMYSCR, STRATEGY_METEOR,
    STRATEGY_SEADRAGON, STRATEGY_SEARCHMETEOR, STRATEGY_SLOWMETEOR,
};
use std::sync::OnceLock;

/// mothermap control bytes (MAPMACS.INC:553-561). These are jump-table
/// byte offsets (table of `dw`), hence the stride of 2.
pub mod mop {
    pub const OBJ: u8 = 0;
    pub const LOOP: u8 = 2;
    pub const END: u8 = 4;
    pub const RND: u8 = 6;
    pub const GOTO: u8 = 8;
    pub const WAIT: u8 = 10;
    pub const COUNT: u8 = 12;
    pub const JUMP: u8 = 14;
    pub const SET: u8 = 16; // motherset — no ROM jump-table entry; unused by data
    pub const DIRECT_OBJ: u8 = 18;
    pub const DIRECT_RND: u8 = 20;
}

/// Struct sizes (MAPSTRUC.INC): header + payload.
pub const MOTH_SIZEOF: usize = 3; // ctrl + count
pub const MO_SIZEOF: usize = 14; // motherobj / motherrnd
pub const ML_SIZEOF: usize = 6; // motherloop
pub const MG_SIZEOF: usize = 5; // mothergoto
pub const MC_SIZEOF: usize = 5; // mothercnt
pub const MJ_SIZEOF: usize = 8; // motherjump

/// The assembled MOTHERS.ASM blob plus the entry offsets of each mothermap
/// referenced by ported levels.
pub struct MotherMaps {
    pub blob: Vec<u8>,
    // MOTHERS.ASM labels (order as in the source file).
    pub mother_0: u16,
    pub mother_1: u16,
    pub mother_2: u16,
    pub mother_3: u16,
    pub mother_5: u16,
    pub map_amoebas: u16,
    pub map_uperm: u16,
    pub map_shou0: u16,
    pub map_meteo0: u16,
    pub mother_snakes: u16,
    pub map_mine2: u16,
    pub map_bhole: u16,
    pub mother_clasteroids: u16,
    pub map_pillars: u16,
    pub map_flypillars: u16,
}

struct Mb {
    v: Vec<u8>,
}

impl Mb {
    fn here(&self) -> u16 {
        self.v.len() as u16
    }
    fn e8(&mut self, b: u8) {
        self.v.push(b);
    }
    fn e16(&mut self, w: u16) {
        self.v.push((w & 0xFF) as u8);
        self.v.push((w >> 8) as u8);
    }
    /// `motherobj frame,x,y,z,shape,strat` (MAPMACS.INC:606-613).
    fn obj<S: Into<StrategyRef>>(
        &mut self,
        frame: u16,
        x: i16,
        y: i16,
        z: i16,
        shape: u16,
        strategy: S,
    ) {
        let (control, strategy_word, strategy_tag) = match strategy.into() {
            StrategyRef::Encoded(value) => (mop::OBJ, value as u16, (value >> 16) as u8),
            StrategyRef::Direct(value) => (mop::DIRECT_OBJ, value.id() as u16, 0),
        };
        self.e8(control);
        self.e16(frame);
        self.e16(x as u16);
        self.e16(y as u16);
        self.e16(z as u16);
        self.e16(shape);
        self.e16(strategy_word);
        self.e8(strategy_tag);
    }
    /// `motherrnd frame,xmask,ymask,zmask,shape,strat` (MAPMACS.INC:643-650).
    fn rnd<S: Into<StrategyRef>>(
        &mut self,
        frame: u16,
        xm: u16,
        ym: u16,
        zm: u16,
        shape: u16,
        strategy: S,
    ) {
        let (control, strategy_word, strategy_tag) = match strategy.into() {
            StrategyRef::Encoded(value) => (mop::RND, value as u16, (value >> 16) as u8),
            StrategyRef::Direct(value) => (mop::DIRECT_RND, value.id() as u16, 0),
        };
        self.e8(control);
        self.e16(frame);
        self.e16(xm);
        self.e16(ym);
        self.e16(zm);
        self.e16(shape);
        self.e16(strategy_word);
        self.e8(strategy_tag);
    }
    /// `mothergoto frame,addr` (MAPMACS.INC:628-637).
    fn goto_(&mut self, frame: u16, addr: u16) {
        self.e8(mop::GOTO);
        self.e16(frame);
        self.e16(addr);
    }
}

/// Assemble the MOTHERS.ASM subset used by the ported levels. Values are
/// verbatim from `reference/ultrastarfox/SF/MAPS/MOTHERS.ASM` (with the
/// file's running `asterdist =` reassignments resolved per block).
fn build() -> MotherMaps {
    // Child shape ids (levels.c SH_* values; wireframe-only shapes proxy
    // through the extended catalog like the level bytecode does).
    const ASTEROID1: u16 = 275; // SH_ASTEROID1_PROXY (SHAPE_EXT_ASTEROID1)
    const ASTEROID2: u16 = 195; // SH_ASTEROID2
    const AMOEBA2: u16 = 104; // SH_AMOEBA2
    const UPER_M: u16 = 133; // SH_UPER_M
    const MINE_2: u16 = 210; // SH_MINE_2
    const S_HOU_0: u16 = 163; // SH_S_HOU_0
    const R_HOU_0: u16 = 162; // SH_R_HOU_0
    const METEO_0: u16 = 193; // SH_METEO_0
    const CLASTEROID: u16 = 280; // SH_CLASTEROID_PROXY (SHAPE_EXT_CLASTEROID)
    const PILLAR2: u16 = 41; // def_shape pillar2 (ISTRATS.ASM)
    const RPILLAR3: u16 = sh::PILLAR3; // SH_RPILLAR3_PROXY -> SH_PILLAR3

    // Child strategy addresses. Table strategies use the synthetic
    // `0x02:00xx` istrat form (the exact zero-based ISTRATS.ASM row);
    // label-only strategies use STRAT_ADDR_*.
    const IS_SYNTH: u32 = 0x020000;
    const HARD: u32 = IS_SYNTH | is::HARD;
    const BREAK_METEOR: u32 = IS_SYNTH | is::BREAK_METEOR;
    const AMOEBA: u32 = IS_SYNTH | is::AMOEBA;
    const UPERM: u32 = IS_SYNTH | is::UPERM;
    const SHOU0: u32 = IS_SYNTH | is::SHOU0;
    const SHOU0A: u32 = IS_SYNTH | is::SHOU0A;
    const METEO0: u32 = IS_SYNTH | is::METEO0;
    const MINE2: u32 = IS_SYNTH | is::MINE2;
    const FLYPILLAR: u32 = IS_SYNTH | is::FLYPILLARS;
    const PILLAR2_ISTRAT: u32 = 0x09_97B3; // DSTRATS.ASM pillar2_Istrat

    let mut b = Mb { v: Vec::new() };
    b.e8(0xFF); // pad: offset 0 == "no mothermap"

    // mother_0 (asterdist = 150)
    let mother_0 = b.here();
    b.rnd(150, 2048, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(150, 1024, 1024, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(150, 2048, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.goto_(0, mother_0);

    // asterdist = 500
    let mother_1 = b.here();
    b.rnd(500, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.goto_(0, mother_1);

    let mother_2 = b.here();
    b.rnd(500, 1024, 1024, 0, ASTEROID1, HARD);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, HARD);
    b.rnd(800, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, HARD);
    b.rnd(500, 1024, 1024, 0, ASTEROID1, HARD);
    b.rnd(800, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.goto_(0, mother_2);

    // asterdist = 100
    let mother_3 = b.here();
    b.rnd(100, 2048, 2048, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(100, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.rnd(100, 2048, 2048, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(100, 2048, 2048, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.goto_(0, mother_3);

    // asterdist = 250
    let mother_5 = b.here();
    b.rnd(250, 2048, 2048, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID2, STRATEGY_SEARCHMETEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(250, 2048, 2048, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, STRATEGY_SLOWMETEOR);
    b.goto_(0, mother_5);

    // asterdist = 250
    let map_amoebas = b.here();
    b.rnd(250, 1024, 1024, 0, AMOEBA2, AMOEBA);
    b.goto_(0, map_amoebas);

    let map_uperm = b.here();
    b.rnd(1500, 1024, 0, 0, UPER_M, UPERM);
    b.goto_(0, map_uperm);

    let map_shou0 = b.here();
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 1024, 0, R_HOU_0, SHOU0A);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 1024, 0, R_HOU_0, SHOU0A);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 1024, 0, S_HOU_0, SHOU0);
    b.goto_(0, map_shou0);

    // (asterdist still 250 for the break_meteor line)
    let map_meteo0 = b.here();
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 512, 0, METEO_0, METEO0);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.rnd(250, 1024, 1024, 0, ASTEROID1, BREAK_METEOR);
    b.rnd(250, 1024, 2048, 0, ASTEROID1, STRATEGY_METEOR);
    b.goto_(0, map_meteo0);

    // snakedist = 500
    let mother_snakes = b.here();
    b.rnd(500, 1024, 0, 256, sh::NULLSHAPE, STRATEGY_SEADRAGON);
    b.goto_(0, mother_snakes);

    let map_mine2 = b.here();
    b.rnd(1500, 1024, 0, 0, UPER_M, UPERM);
    b.rnd(1500, 1024, 256, 0, MINE_2, MINE2);
    b.goto_(0, map_mine2);

    let map_bhole = b.here();
    b.rnd(800, 1024, 1024, 4000, sh::NULLSHAPE, STRATEGY_DAMYSCR);
    b.goto_(0, map_bhole);

    let mother_clasteroids = b.here();
    b.rnd(200, 1024, 1024, 0, CLASTEROID, STRATEGY_CLASTEROID);
    b.goto_(0, mother_clasteroids);

    // MOTHERS.ASM map_pillars — the extending pillar wall used by MAP1_6A.
    let map_pillars = b.here();
    for x in [-500, -250, 0, 250, 500, 250, 0, -250] {
        b.obj(100, x, 0, 0, PILLAR2, PILLAR2_ISTRAT);
    }
    b.goto_(0, map_pillars);

    let map_flypillars = b.here();
    b.obj(600, -300, -150 * 2, -4100, RPILLAR3, FLYPILLAR);
    b.goto_(0, map_flypillars);

    MotherMaps {
        blob: b.v,
        mother_0,
        mother_1,
        mother_2,
        mother_3,
        mother_5,
        map_amoebas,
        map_uperm,
        map_shou0,
        map_meteo0,
        mother_snakes,
        map_mine2,
        map_bhole,
        mother_clasteroids,
        map_pillars,
        map_flypillars,
    }
}

/// The assembled mothermap blob (built once).
pub fn mother_maps() -> &'static MotherMaps {
    static MM: OnceLock<MotherMaps> = OnceLock::new();
    MM.get_or_init(build)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rd16(b: &[u8], p: usize) -> u16 {
        b[p] as u16 | ((b[p + 1] as u16) << 8)
    }

    fn rd24(b: &[u8], p: usize) -> u32 {
        rd16(b, p) as u32 | ((b[p + 2] as u32) << 16)
    }

    /// mother_0's first entry must decode exactly per MOTHERS.ASM /
    /// MAPSTRUC.INC: motherrnd, count 150, masks 2048/2048/0, asteroid1,
    /// meteor strat; the trailing goto loops back to mother_0.
    #[test]
    fn mother_0_encoding() {
        let m = mother_maps();
        let b = &m.blob;
        let p = m.mother_0 as usize;
        assert_ne!(p, 0, "offset 0 is the pad byte");
        assert_eq!(b[p], mop::DIRECT_RND);
        assert_eq!(rd16(b, p + 1), 150); // moth_count
        assert_eq!(rd16(b, p + 3), 2048); // mo_x mask
        assert_eq!(rd16(b, p + 5), 2048); // mo_y mask
        assert_eq!(rd16(b, p + 7), 0); // mo_z mask
        assert_eq!(rd16(b, p + 9), 275); // asteroid1 proxy shape
        assert_eq!(rd16(b, p + 11), STRATEGY_METEOR.id() as u16);
        assert_eq!(b[p + 13], 0);
        // 3 motherrnd entries then mothergoto back to mother_0.
        let g = p + 3 * MO_SIZEOF;
        assert_eq!(b[g], mop::GOTO);
        assert_eq!(rd16(b, g + 1), 0); // goto wait 0
        assert_eq!(rd16(b, g + 3), m.mother_0); // target
    }

    /// Every map's terminating goto targets a valid entry inside the blob,
    /// and all entry offsets are nonzero.
    #[test]
    fn all_offsets_valid() {
        let m = mother_maps();
        let offs = [
            m.mother_0,
            m.mother_1,
            m.mother_2,
            m.mother_3,
            m.mother_5,
            m.map_amoebas,
            m.map_uperm,
            m.map_shou0,
            m.map_meteo0,
            m.mother_snakes,
            m.map_mine2,
            m.map_bhole,
            m.mother_clasteroids,
            m.map_pillars,
            m.map_flypillars,
        ];
        for o in offs {
            assert_ne!(o, 0);
            assert!((o as usize) < m.blob.len());
            let ctrl = m.blob[o as usize];
            assert!(
                matches!(
                    ctrl,
                    mop::RND | mop::OBJ | mop::DIRECT_RND | mop::DIRECT_OBJ
                ),
                "ctrl {ctrl}"
            );
        }
    }

    #[test]
    fn table_strategy_addresses_use_exact_istrats_rows() {
        let m = mother_maps();
        let b = &m.blob;
        // Some mother maps interleave asteroid entries before the strategy
        // being checked, so name the exact entry ordinal from MOTHERS.ASM.
        for (offset, entry, row, name) in [
            (m.mother_2, 0, is::HARD, "hard"),
            (m.map_amoebas, 0, is::AMOEBA, "amoeba"),
            (m.map_uperm, 0, is::UPERM, "uperm"),
            (m.map_shou0, 2, is::SHOU0A, "shou0a"),
            (m.map_shou0, 8, is::SHOU0, "shou0"),
            (m.map_meteo0, 1, is::METEO0, "meteo0"),
            (m.map_mine2, 1, is::MINE2, "mine2"),
            (m.map_flypillars, 0, is::FLYPILLARS, "flypillars"),
        ] {
            let p = offset as usize + entry * MO_SIZEOF;
            assert!(b[p] == mop::OBJ || b[p] == mop::RND, "{name} entry kind");
            assert_eq!(rd24(b, p + 11), 0x020000 | row, "{name} synth address");
        }
    }

    #[test]
    fn map_pillars_matches_mothers_asm() {
        let m = mother_maps();
        let b = &m.blob;
        let expected_x = [-500i16, -250, 0, 250, 500, 250, 0, -250];
        for (entry, x) in expected_x.into_iter().enumerate() {
            let p = m.map_pillars as usize + entry * MO_SIZEOF;
            assert_eq!(b[p], mop::OBJ);
            assert_eq!(rd16(b, p + 1), 100);
            assert_eq!(rd16(b, p + 3), x as u16);
            assert_eq!(rd16(b, p + 5), 0);
            assert_eq!(rd16(b, p + 7), 0);
            assert_eq!(rd16(b, p + 9), 41);
            assert_eq!(rd24(b, p + 11), 0x09_97B3);
        }
        let g = m.map_pillars as usize + expected_x.len() * MO_SIZEOF;
        assert_eq!(b[g], mop::GOTO);
        assert_eq!(rd16(b, g + 3), m.map_pillars);
    }
}
