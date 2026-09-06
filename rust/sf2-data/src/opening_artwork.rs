//! Palette assets in the decoded opening-artwork bundle.
//!
//! Offsets describe the asset format, not live machine state. The background,
//! foreground, and sprite palettes are separate source loader operations.

use crate::compression::{decode_artwork, DecodeError};

pub const BACKGROUND_COLORS: usize = 64;
pub const FOREGROUND_COLORS: usize = 48;
pub const SPRITE_COLORS: usize = 128;
const DECODED_ARTWORK_BYTES: usize = 0x24C0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundPaletteId {
    Standard,
    CatalogOne,
    CatalogTwo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningArtworkPalettes {
    pub background: [u16; BACKGROUND_COLORS],
    foreground: [[u16; FOREGROUND_COLORS]; 3],
    pub sprites: [u16; SPRITE_COLORS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningArtworkError {
    Compression(DecodeError),
    UnexpectedLength { actual: usize },
}

impl OpeningArtworkPalettes {
    /// Decode the original compressed bundle. The caller supplies the stream
    /// ending at its trailer, as specified by `compression::decode_artwork`.
    pub fn decode(compressed: &[u8]) -> Result<Self, OpeningArtworkError> {
        let decoded = decode_artwork(compressed).map_err(OpeningArtworkError::Compression)?;
        Self::from_decoded(&decoded)
    }

    pub fn from_decoded(decoded: &[u8]) -> Result<Self, OpeningArtworkError> {
        if decoded.len() != DECODED_ARTWORK_BYTES {
            return Err(OpeningArtworkError::UnexpectedLength {
                actual: decoded.len(),
            });
        }
        fn colors<const N: usize>(bytes: &[u8], start: usize) -> [u16; N] {
            std::array::from_fn(|index| {
                u16::from_le_bytes(
                    bytes[start + index * 2..start + index * 2 + 2]
                        .try_into()
                        .unwrap(),
                )
            })
        }
        Ok(Self {
            background: colors(decoded, 0x80),
            foreground: [
                colors(decoded, 0x400),
                colors(decoded, 0x7C0),
                colors(decoded, 0x760),
            ],
            sprites: colors(decoded, 0x2340),
        })
    }

    pub fn foreground(&self, id: ForegroundPaletteId) -> &[u16; FOREGROUND_COLORS] {
        &self.foreground[match id {
            ForegroundPaletteId::Standard => 0,
            ForegroundPaletteId::CatalogOne => 1,
            ForegroundPaletteId::CatalogTwo => 2,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_incomplete_or_different_sized_bundles() {
        for size in [
            0,
            0x243F,
            DECODED_ARTWORK_BYTES - 1,
            DECODED_ARTWORK_BYTES + 1,
        ] {
            assert_eq!(
                OpeningArtworkPalettes::from_decoded(&vec![0; size]),
                Err(OpeningArtworkError::UnexpectedLength { actual: size })
            );
        }
        assert_eq!(
            OpeningArtworkPalettes::decode(&[]),
            Err(OpeningArtworkError::Compression(DecodeError::Truncated))
        );
    }

    #[test]
    fn preserves_distinct_palette_blocks_and_full_color_words() {
        let bytes: Vec<_> = (0..DECODED_ARTWORK_BYTES / 2)
            .flat_map(|word| (word as u16 | 0x8000).to_le_bytes())
            .collect();
        let palettes = OpeningArtworkPalettes::from_decoded(&bytes).unwrap();
        assert_eq!(
            palettes.background,
            std::array::from_fn(|i| (0x40 + i) as u16 | 0x8000)
        );
        assert_eq!(
            *palettes.foreground(ForegroundPaletteId::Standard),
            std::array::from_fn(|i| (0x200 + i) as u16 | 0x8000)
        );
        assert_eq!(
            *palettes.foreground(ForegroundPaletteId::CatalogOne),
            std::array::from_fn(|i| (0x3E0 + i) as u16 | 0x8000)
        );
        assert_eq!(
            *palettes.foreground(ForegroundPaletteId::CatalogTwo),
            std::array::from_fn(|i| (0x3B0 + i) as u16 | 0x8000)
        );
        assert_eq!(
            palettes.sprites,
            std::array::from_fn(|i| (0x11A0 + i) as u16 | 0x8000)
        );
    }
}
