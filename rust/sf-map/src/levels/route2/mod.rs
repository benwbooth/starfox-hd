//! route2 level builders (lane-owned; see the route porting lane).
//!
//! Levels register here so porting lanes never edit shared files:
//! add `pub mod levelX_Y;` plus a match arm in [`get`].

use crate::levels::BuiltLevel;

/// Lane-owned map-id dispatch, chained from `catalog::get_map_data`.
pub fn get(_id: u32) -> Option<&'static BuiltLevel> {
    None
}
