//! Flat SF1 shape identities.
//!
//! Gameplay and rendering store native flat ids. Source ShapeHdr words are
//! accepted only at data-import boundaries and normalized through the
//! generated symbol-derived table.

use crate::sf1_shape_words;

/// Start of the shared renderer's native SF2 shape namespace. SF1 owns the
/// lower 512 flat ids; SF2 catalog indices are offset into this disjoint range
/// rather than carrying source ShapeHdr addresses through the shipping port.
pub const SF2_SHAPE_NAMESPACE_START: u16 = 1024;

pub const fn sf2_shape_id(catalog_index: u16) -> u16 {
    SF2_SHAPE_NAMESPACE_START + catalog_index
}

pub const fn sf2_catalog_index(shape_id: u16) -> Option<u16> {
    shape_id.checked_sub(SF2_SHAPE_NAMESPACE_START)
}

/// Convert an SF1 source shape word or compatibility alias into the native
/// flat shape id used by the Rust port. Already-flat ids pass through.
pub fn resolve_shape_word(shape_word: u16) -> u16 {
    match shape_word {
        // Compatibility words used by the native map builders.
        551 => 508,
        552 => 509,
        553 => 510,
        554 => 2,
        557 => 282,
        614 => 298,
        _ => sf1_shape_words::flat_id(shape_word).unwrap_or(shape_word),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembled_path_shape_words_become_flat_ids() {
        assert_eq!(resolve_shape_word(0xB6E7), 442); // flower
        assert_eq!(resolve_shape_word(0xA4BF), 443); // big_bird
        assert_eq!(resolve_shape_word(0x98B8), 386); // egg
        assert_eq!(resolve_shape_word(0xA2A6), 387); // boss_d_8
        assert_eq!(resolve_shape_word(0xA2C2), 388); // boss_d_9
    }

    #[test]
    fn sf2_catalog_uses_a_disjoint_flat_namespace() {
        assert_eq!(sf2_shape_id(0), SF2_SHAPE_NAMESPACE_START);
        assert_eq!(sf2_catalog_index(sf2_shape_id(576)), Some(576));
        assert_eq!(sf2_catalog_index(511), None);
    }
}
