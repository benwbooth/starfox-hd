//! World-lane re-export of shared ROM trig (`sf_core::snes_trig`).
//!
//! Kept as `sf_game::trig8` so existing call sites stay stable.

pub use sf_core::snes_trig::{
    achase_angle_8, mulslog, mulslog_mac8, rotate_16xz, rotate_16yz, rotate_8xz, rotate_8yx,
    rotate_8yz, strat_roffs_full, strat_roffs_full_scaled, strat_roffs_roll, COSTAB, SINTAB,
    XSPACEBAR_HALF_B,
};
