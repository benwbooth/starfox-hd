//! MAP_ID_FINAL — FINALMAP.ASM, the Andross final tunnel + boss.
//!
//! C oracle: `src/map/levels.c` `build_final_slice()` +
//! `register_final_inline_callbacks()` (shared
//! `append_finalmap_content()` with prefix "final").

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // (MAP_ID_FINAL)
    // ============================================================


    // Reuse the shared helper with "final" prefix.
    let (mapwaitboss_cantdie_ptr, mapwaitboss_cleanup_ptr) = append_finalmap_content(&mut b, "final");

    b.resolve();

    let (data, labels) = b.finish();
    // C `register_final_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
