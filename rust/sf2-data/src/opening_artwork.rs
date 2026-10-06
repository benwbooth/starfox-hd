//! Decoded background and palette assets for the standard opening loader.
//!
//! Offsets describe the asset format, not live machine state. The background,
//! foreground, and sprite palettes are separate source loader operations.

use crate::compression::{decode_artwork, DecodeError};

pub const BACKGROUND_COLORS: usize = 64;
pub const FOREGROUND_COLORS: usize = 48;
pub const SPRITE_COLORS: usize = 128;
const DECODED_ARTWORK_BYTES: usize = 0x24C0;
pub const BACKGROUND_TILE_COUNT: usize = 256;
pub const BACKGROUND_CELL_COUNT: usize = 2048;
pub const TILE_EDGE: usize = 8;
const TILE_PLANAR_BYTES: usize = 32;
const TILE_COLOR_BITS: usize = 4;
const PLANES_PER_GROUP: usize = 2;
const CELL_PALETTE_SHIFT: u32 = 10;
const CHARACTER_TRANSFER_BYTES: usize = BACKGROUND_TILE_COUNT * TILE_PLANAR_BYTES;
const CHARACTER_DECODED_BYTES: usize = CHARACTER_TRANSFER_BYTES + TILE_PLANAR_BYTES;
const MAP_TRANSFER_BYTES: usize = BACKGROUND_CELL_COUNT * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundPaletteId {
    Standard,
    CatalogOne,
    CatalogTwo,
}

impl ForegroundPaletteId {
    /// The standard scene loader optionally selects a foreground catalog row.
    /// Zero selects the standard row, one the first alternate, and every
    /// other byte the second alternate. This is not a difficulty conversion.
    pub const fn select(use_catalog: bool, entry: u8) -> Self {
        if !use_catalog || entry == 0 {
            Self::Standard
        } else if entry == 1 {
            Self::CatalogOne
        } else {
            Self::CatalogTwo
        }
    }
}

/// One decoded four-bit tile. Pixels are row-major palette indices; larger
/// background characters are composed from these elementary eight-pixel tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundTile {
    pixels: [u8; TILE_EDGE * TILE_EDGE],
}

impl BackgroundTile {
    pub fn pixels(&self) -> &[u8; TILE_EDGE * TILE_EDGE] {
        &self.pixels
    }

    fn from_planar(bytes: &[u8]) -> Self {
        Self {
            pixels: std::array::from_fn(|pixel| {
                let row = pixel / TILE_EDGE;
                let bit = TILE_EDGE - 1 - pixel % TILE_EDGE;
                (0..TILE_COLOR_BITS).fold(0, |value, plane| {
                    let offset = plane / PLANES_PER_GROUP * TILE_EDGE * PLANES_PER_GROUP
                        + row * PLANES_PER_GROUP
                        + plane % PLANES_PER_GROUP;
                    value | ((bytes[offset] >> bit) & 1) << plane
                })
            }),
        }
    }
}

/// Decoded background artwork attributes, not a live video-memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundCell {
    pub tile: u16,
    pub palette: u8,
    pub priority: bool,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl BackgroundCell {
    pub const fn from_packed(packed: u16) -> Self {
        Self {
            tile: packed & 0x03FF,
            palette: ((packed >> CELL_PALETTE_SHIFT) & 0x07) as u8,
            priority: packed & 0x2000 != 0,
            flip_horizontal: packed & 0x4000 != 0,
            flip_vertical: packed & 0x8000 != 0,
        }
    }
}

/// Assets used by the standard opening scene loader. Character and map uploads
/// remain separate even though decoding the immutable resources is eager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningArtwork {
    pub characters: std::sync::Arc<[BackgroundTile; BACKGROUND_TILE_COUNT]>,
    pub map: std::sync::Arc<[BackgroundCell; BACKGROUND_CELL_COUNT]>,
    pub palettes: OpeningArtworkPalettes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningArtworkResource {
    Characters,
    Map,
    Palettes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningArtworkLoadError {
    pub resource: OpeningArtworkResource,
    pub cause: OpeningArtworkError,
}

impl OpeningArtwork {
    /// Each supplied slice ends at that resource's backward-stream trailer.
    /// Extraction supplies these slices; gameplay never resolves ROM addresses.
    pub fn decode(
        characters: &[u8],
        map: &[u8],
        palettes: &[u8],
    ) -> Result<Self, OpeningArtworkLoadError> {
        let decode = |bytes, resource| {
            decode_artwork(bytes).map_err(|cause| OpeningArtworkLoadError {
                resource,
                cause: OpeningArtworkError::Compression(cause),
            })
        };
        Self::from_decoded(
            &decode(characters, OpeningArtworkResource::Characters)?,
            &decode(map, OpeningArtworkResource::Map)?,
            &decode(palettes, OpeningArtworkResource::Palettes)?,
        )
    }

    pub fn from_decoded(
        characters: &[u8],
        map: &[u8],
        palettes: &[u8],
    ) -> Result<Self, OpeningArtworkLoadError> {
        for (resource, actual, expected) in [
            (
                OpeningArtworkResource::Characters,
                characters.len(),
                CHARACTER_DECODED_BYTES,
            ),
            (OpeningArtworkResource::Map, map.len(), MAP_TRANSFER_BYTES),
        ] {
            if actual != expected {
                return Err(OpeningArtworkLoadError {
                    resource,
                    cause: OpeningArtworkError::UnexpectedLength { actual },
                });
            }
        }
        // The final decoded character tile is not part of this upload.
        // Map entries retain all ten tile-index bits, including references to
        // characters outside this particular upload; composition owns layout.
        Ok(Self {
            characters: std::sync::Arc::new(std::array::from_fn(|tile| {
                let start = tile * TILE_PLANAR_BYTES;
                BackgroundTile::from_planar(&characters[start..start + TILE_PLANAR_BYTES])
            })),
            map: std::sync::Arc::new(std::array::from_fn(|cell| {
                BackgroundCell::from_packed(u16::from_le_bytes(
                    map[cell * 2..cell * 2 + 2].try_into().unwrap(),
                ))
            })),
            palettes: OpeningArtworkPalettes::from_decoded(palettes).map_err(|cause| {
                OpeningArtworkLoadError {
                    resource: OpeningArtworkResource::Palettes,
                    cause,
                }
            })?,
        })
    }
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
    fn foreground_selection_covers_every_catalog_byte_and_enable_state() {
        for entry in 0..=255 {
            assert_eq!(
                ForegroundPaletteId::select(false, entry),
                ForegroundPaletteId::Standard
            );
            assert_eq!(
                ForegroundPaletteId::select(true, entry),
                match entry {
                    0 => ForegroundPaletteId::Standard,
                    1 => ForegroundPaletteId::CatalogOne,
                    _ => ForegroundPaletteId::CatalogTwo,
                }
            );
        }
    }

    #[test]
    fn planar_tiles_preserve_every_plane_bit_and_pixel_position() {
        for offset in 0..TILE_PLANAR_BYTES {
            for bit in 0..TILE_EDGE {
                let mut encoded = [0; TILE_PLANAR_BYTES];
                encoded[offset] = 1 << bit;
                let decoded = BackgroundTile::from_planar(&encoded);
                let plane = offset / (TILE_EDGE * 2) * 2 + offset % 2;
                let row = offset % (TILE_EDGE * 2) / 2;
                let column = TILE_EDGE - 1 - bit;
                for pixel in 0..TILE_EDGE * TILE_EDGE {
                    assert_eq!(
                        decoded.pixels()[pixel],
                        if pixel == row * TILE_EDGE + column {
                            1 << plane
                        } else {
                            0
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn map_attributes_retain_every_encoded_bit() {
        for word in 0..=u16::MAX {
            let cell = BackgroundCell::from_packed(word);
            let reconstructed = cell.tile
                | u16::from(cell.palette) << 10
                | u16::from(cell.priority) << 13
                | u16::from(cell.flip_horizontal) << 14
                | u16::from(cell.flip_vertical) << 15;
            assert_eq!(word, reconstructed);
        }
    }

    #[test]
    fn artwork_transfers_exclude_character_padding_and_reject_wrong_resource_sizes() {
        let mut characters = vec![0; CHARACTER_DECODED_BYTES];
        characters[CHARACTER_TRANSFER_BYTES..].fill(255);
        let map = vec![255; MAP_TRANSFER_BYTES];
        let palettes = vec![0; DECODED_ARTWORK_BYTES];
        let artwork = OpeningArtwork::from_decoded(&characters, &map, &palettes).unwrap();
        assert!(artwork
            .characters
            .iter()
            .all(|tile| tile.pixels() == &[0; 64]));
        assert!(artwork
            .map
            .iter()
            .all(|cell| *cell == BackgroundCell::from_packed(65535)));
        for (bad_characters, bad_map, bad_palettes, resource) in [
            (
                &characters[..CHARACTER_TRANSFER_BYTES],
                &map[..],
                &palettes[..],
                OpeningArtworkResource::Characters,
            ),
            (
                &characters[..],
                &map[..MAP_TRANSFER_BYTES - 1],
                &palettes[..],
                OpeningArtworkResource::Map,
            ),
            (
                &characters[..],
                &map[..],
                &palettes[..DECODED_ARTWORK_BYTES - 1],
                OpeningArtworkResource::Palettes,
            ),
        ] {
            assert_eq!(
                OpeningArtwork::from_decoded(bad_characters, bad_map, bad_palettes)
                    .unwrap_err()
                    .resource,
                resource
            );
        }
        assert_eq!(
            OpeningArtwork::decode(&[], &[], &[]).unwrap_err(),
            OpeningArtworkLoadError {
                resource: OpeningArtworkResource::Characters,
                cause: OpeningArtworkError::Compression(DecodeError::Truncated),
            }
        );
    }

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
