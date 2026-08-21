//! MAP_ID_PLANET — planet selection screen.
//!
//! C oracle: `src/map/levels.c` `build_planet_slice()` (no inline or
//! native callbacks).
//!
//! ASM source transcribed (via the C port): `PLANET.ASM` — background
//! rock rows for the map/planet select screen.

use super::BuiltLevel;
use crate::builder::MapBuilder;
use crate::consts::*;

/// C `build_planet_slice()`.
pub fn build() -> BuiltLevel {
    let mut b = MapBuilder::new();

    // PLANET.ASM — planet selection screen background objects.
    b.preserve_behind_view_objects();

    // Lines 7-9: r_bu_4 row (right side)
    b.mapobj(0, 0x0220, -1000, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, 0x0220, -500, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, 0x0220, -10, -200, sh::R_BU_4, is::HARD);

    // Lines 11-12: bu_6 pair with roty
    b.mapobj(0, -500, 0, 400, sh::BU_6, is::HARD);
    b.setalvarb(al::ROTY, -64);
    b.mapobj(0, 500, 0, 400, sh::BU_6, is::HARD);
    b.setalvarb(al::ROTY, 64);

    // Lines 15-18: bu_0 and bu_2 pairs
    b.mapobj(0, 800, 0, -300, sh::BU_0, is::HARD);
    b.mapobj(0, -800, 0, -300, sh::BU_0, is::HARD);
    b.mapobj(0, -300, 0, -800, sh::BU_2, is::HARD);
    b.mapobj(0, 300, 0, -800, sh::BU_2, is::HARD);

    // Lines 19-21: r_bu_4 row (left side)
    b.mapobj(0, -0x0220, -1000, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0, -0x0220, -500, -200, sh::R_BU_4, is::HARD);
    b.mapobj(0x0200, -0x0220, -10, -200, sh::R_BU_4, is::HARD);

    // Lines 24-31: r_bu_7 grid
    b.mapobj(0, -200, -300, 600, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0200, 200, -300, 600, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 200, -300, 800, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0200, -200, -300, 800, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 180, -250, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0400, -180, -250, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0, 150, -200, 1000, sh::R_BU_7, is::HARD180YR);
    b.mapobj(0x0300, -150, -200, 1000, sh::R_BU_7, is::HARD180YR);

    // Infinite wait — planet screen runs until the player selects a level.
    b.label("planet.wait");
    b.mapwait(4000);
    b.mapgoto("planet.wait");

    b.resolve();

    let (data, labels) = b.finish();
    BuiltLevel {
        data,
        labels,
        native_callbacks: Vec::new(),
        inline_callbacks: Vec::new(),
    }
}
