//! Exact Star Fox 2 retail draw-list record ABI.
//!
//! The 65816 routine at `$02:9201..$02:947D` builds these records at
//! `$7E:B273`.  The live word count is `$7E:18C6` (mirrored to `$7E:CF1D`)
//! and each record is copied byte-for-byte to GSU RAM `$70:0AD0` before the
//! renderer runs.  Bytes after the live count are stale and are deliberately
//! excluded by [`parse_draw_list`].

pub const DRAW_LIST_WRAM_ADDRESS: u16 = 0xB273;
pub const DRAW_COUNT_WRAM_ADDRESS: u16 = 0x18C6;
pub const DRAW_COUNT_MIRROR_WRAM_ADDRESS: u16 = 0xCF1D;
pub const DRAW_RECORD_SIZE: usize = 0x26;
pub const DRAW_RECORD_CAPACITY: usize = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DrawRecord {
    /// GSU-produced depth-sort linkage.
    pub next: u16,
    /// Initial special-object value is `$3A98`; otherwise zero before GSU
    /// projection/sorting.
    pub sort_z: i16,
    /// Low bytes of object rotations `$12/$14/$16`.
    pub rotation_x: u8,
    pub rotation_y: u8,
    pub rotation_z: u8,
    /// Object field `$20`, consumed as renderer flags.
    pub shape_flags: u8,
    /// Bank-$00 ShapeHdr token copied from object field `$04`.
    pub shape: u16,
    /// Legacy `dl_shady` slot. SF2 stores the source object address here when
    /// object flag `$20 & $10` is set, else zero.
    pub shadow_y: i16,
    /// Legacy `dl_shadx`/`dl_shadz` slots. The SF2 extended-rotation path
    /// stores the high rotation bytes in `$0C/$0D/$0E`.
    pub shadow_x: i16,
    pub shadow_z: i16,
    /// Legacy projected `dl_y/dl_x/dl_z` workspace, not initialized by the
    /// CPU list builder before its byte-for-byte copy to GSU RAM.
    pub projected_y: i16,
    pub projected_x: i16,
    pub projected_z: i16,
    /// Bank-$01 material-table pointer, copied from object field `$1CCD`.
    pub color_table: u16,
    /// Object explosion counter `$0A` when object field `$08 & 1` is set,
    /// else zero.
    pub explosion_count: u8,
    /// Object extended fields `$1CCB/$1CCA`; nonnegative values select the
    /// global frame byte `$C4`, and the stored value is masked with `$7F`.
    pub animation_frame: u8,
    pub color_frame: u8,
    /// Object extended fields `alx_depthoffset/alx_tx/alx_ty` at
    /// `$1CC8/$1CDA/$1CDB` respectively.
    pub depth_offset: u8,
    pub texture_scroll_x: u8,
    pub texture_scroll_y: u8,
    /// Clipping-plane selector from object extension `$1CEF`. Zero disables
    /// clipping; nonzero selects a scene plane (including shape command `$68`).
    /// The GSU copies it to `$24DE` at `$01:D1BD` and applies the plane in
    /// `$01:F2FA/$01:F379/$01:F3A6` before polygon rasterization.
    pub field_1e: u8,
    /// Byte `$1F`; not written by the list builder.
    pub reserved_1f: u8,
    /// Signed world position copied from object fields `$0C/$0E/$10`.
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl DrawRecord {
    pub fn from_bytes(bytes: &[u8; DRAW_RECORD_SIZE]) -> Self {
        Self {
            next: word(bytes, 0x00),
            sort_z: word(bytes, 0x02) as i16,
            rotation_x: bytes[0x04],
            rotation_y: bytes[0x05],
            rotation_z: bytes[0x06],
            shape_flags: bytes[0x07],
            shape: word(bytes, 0x08),
            shadow_y: word(bytes, 0x0A) as i16,
            shadow_x: word(bytes, 0x0C) as i16,
            shadow_z: word(bytes, 0x0E) as i16,
            projected_y: word(bytes, 0x10) as i16,
            projected_x: word(bytes, 0x12) as i16,
            projected_z: word(bytes, 0x14) as i16,
            color_table: word(bytes, 0x16),
            explosion_count: bytes[0x18],
            animation_frame: bytes[0x19],
            color_frame: bytes[0x1A],
            depth_offset: bytes[0x1B],
            texture_scroll_x: bytes[0x1C],
            texture_scroll_y: bytes[0x1D],
            field_1e: bytes[0x1E],
            reserved_1f: bytes[0x1F],
            x: word(bytes, 0x20) as i16,
            y: word(bytes, 0x22) as i16,
            z: word(bytes, 0x24) as i16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawListError {
    CountExceedsCapacity { count: usize },
    Truncated { count: usize, available: usize },
}

/// Parse exactly `count` live records, ignoring any stale capacity bytes that
/// follow them in WRAM.
pub fn parse_draw_list(bytes: &[u8], count: usize) -> Result<Vec<DrawRecord>, DrawListError> {
    if count > DRAW_RECORD_CAPACITY {
        return Err(DrawListError::CountExceedsCapacity { count });
    }
    let required = count * DRAW_RECORD_SIZE;
    if bytes.len() < required {
        return Err(DrawListError::Truncated {
            count,
            available: bytes.len(),
        });
    }
    Ok(bytes[..required]
        .chunks_exact(DRAW_RECORD_SIZE)
        .map(|record| DrawRecord::from_bytes(record.try_into().expect("exact record-sized chunk")))
        .collect())
}

fn word(bytes: &[u8; DRAW_RECORD_SIZE], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from retail SF2 after the dogfight transition.  This fixture is
    // the first live `$7E:B273` record with `$18C6 == 5`.
    const RETAIL_RECORD: [u8; DRAW_RECORD_SIZE] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x3E, 0xF8, 0x08, 0x00, 0xEA, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0xFE, 0x04,
        0x00, 0x00, 0xB8, 0xED, 0xB7, 0xFE, 0xC4, 0xEB,
    ];

    #[test]
    fn decodes_retail_record_at_exact_offsets() {
        let record = DrawRecord::from_bytes(&RETAIL_RECORD);
        assert_eq!(record.next, 0);
        assert_eq!(record.sort_z, 0);
        assert_eq!(
            (record.rotation_x, record.rotation_y, record.rotation_z),
            (0, 0x3E, 0xF8)
        );
        assert_eq!(record.shape_flags, 0x08);
        assert_eq!(record.shape, 0xEA00);
        assert_eq!(
            (record.shadow_y, record.shadow_x, record.shadow_z),
            (0, 0, 0)
        );
        assert_eq!(
            (record.projected_y, record.projected_x, record.projected_z),
            (0, 0, 0)
        );
        assert_eq!(record.color_table, 0);
        assert_eq!(
            (
                record.explosion_count,
                record.animation_frame,
                record.color_frame
            ),
            (0, 0x18, 0x18)
        );
        assert_eq!(
            (
                record.depth_offset,
                record.texture_scroll_x,
                record.texture_scroll_y,
                record.field_1e
            ),
            (0, 0xFE, 4, 0)
        );
        assert_eq!(record.reserved_1f, 0);
        assert_eq!((record.x, record.y, record.z), (-4680, -329, -5180));
    }

    #[test]
    fn count_is_authoritative_and_stale_tail_is_ignored() {
        let mut bytes = RETAIL_RECORD.to_vec();
        bytes.extend([0xA5; DRAW_RECORD_SIZE]);
        let records = parse_draw_list(&bytes, 1).unwrap();
        assert_eq!(records, vec![DrawRecord::from_bytes(&RETAIL_RECORD)]);
    }

    #[test]
    fn invalid_count_or_truncated_bytes_are_rejected() {
        assert_eq!(
            parse_draw_list(&[], DRAW_RECORD_CAPACITY + 1),
            Err(DrawListError::CountExceedsCapacity {
                count: DRAW_RECORD_CAPACITY + 1
            })
        );
        assert_eq!(
            parse_draw_list(&RETAIL_RECORD[..DRAW_RECORD_SIZE - 1], 1),
            Err(DrawListError::Truncated {
                count: 1,
                available: DRAW_RECORD_SIZE - 1
            })
        );
    }
}
