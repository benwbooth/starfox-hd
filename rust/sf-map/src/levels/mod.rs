//! Literal level builders.
//!
//! C oracle: `src/map/levels.c` `build_*_slice()` + `register_*_inline_callbacks()`.
//! Each ported level gets one module here; each build function documents the
//! original ASM sources it transcribes (LEVEL1_1.ASM, MAP1_1A.ASM, ...).

pub mod level1_1;
pub mod planet;
pub mod title;

use crate::builder::Label;
use crate::consts::op;

/// Identity of a native map callback registered under a `MAP_CB_*` addr24
/// when a level is selected (C: `World_RegisterNativeCallback`).
/// The variant names mirror the C callback functions in levels.c.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCallback {
    /// C `level1_1_cl_ground_printlevelfin` @ MAP_CB_CL_GROUND_PRINTLEVELFIN
    ClGroundPrintlevelfin,
    /// C `level1_1_cl_ground_wipeout` @ MAP_CB_CL_GROUND_WIPEOUT
    ClGroundWipeout,
    /// C `cl_dive_clear_enginesnd` @ MAP_CB_CL_DIVE_CLEAR_ENGINESND
    ClDiveClearEnginesnd,
}

/// Identity of an inline map-code callback (C: `World_RegisterInlineMapCode`,
/// keyed by the CODE65816 script ptr captured during the build).
/// The variant names mirror the C inline functions in levels.c.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCallback {
    /// C `level_scramble_keep_player_strat`
    LevelScrambleKeepPlayerStrat,
    /// C `level1_1_skillfly_bonus_guard`
    Level1_1SkillflyBonusGuard,
    /// C `level1_1_mapwaitboss_trigse`
    Level1_1MapwaitbossTrigse,
    /// C `level1_1_mapwaitboss_cantdie`
    Level1_1MapwaitbossCantdie,
    /// C `level1_1_mapwaitboss_cleanup`
    Level1_1MapwaitbossCleanup,
    /// C `title_init_inline`
    TitleInit,
    /// C `contmap_init_inline`
    ContmapInit,
}

/// A fully built level: the bytecode blob plus everything the map VM needs
/// to hook it up at load time.
///
/// The C build keeps labels and callback script ptrs in file-static state;
/// here they travel with the level so the executor (and the fixture tests)
/// can compare registration OFFSETS rather than function addresses.
pub struct BuiltLevel {
    /// The emitted map bytecode (byte-identical to the C MapBuilder output).
    pub data: Vec<u8>,
    /// Label table (name -> byte offset), in emission order.
    pub labels: Vec<Label>,
    /// Native callbacks registered when the level is selected, in the C
    /// registration-call order: (MAP_CB_* addr24, identity).
    pub native_callbacks: Vec<(u32, NativeCallback)>,
    /// Inline CODE65816 callbacks, in the C registration-call order:
    /// (script ptr into `data`, identity).
    pub inline_callbacks: Vec<(u16, InlineCallback)>,
}

impl BuiltLevel {
    /// Look up a label offset by name (first match, like the C lookup).
    pub fn label_offset(&self, name: &str) -> Option<u16> {
        self.labels
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, offset)| offset)
    }
}

/// C `s_empty_level`: a single MAP_OP_END byte. Returned for MAP_ID_NONE.
pub fn build_empty() -> BuiltLevel {
    BuiltLevel {
        data: vec![op::END],
        labels: Vec::new(),
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}
