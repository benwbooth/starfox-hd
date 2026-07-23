use sf2_data::map::SpawnRecord;
use sf2_map::Sf2MapHost;

use crate::object::*;
use crate::oracle_compat::{Error, Game, MapMarker};

impl Sf2MapHost for Game {
    type Error = Error;

    fn request_stage_load(&mut self, table_offset: u16) -> Result<(), Self::Error> {
        self.stage_load = Some(table_offset);
        self.load_table_idle = true;
        Ok(())
    }

    fn set_current_object_byte(&mut self, field: u16, value: u8) -> Result<(), Self::Error> {
        let object = self.memory.read_word(CURRENT_OBJECT);
        if object_index(object).is_some() {
            self.memory.write_byte(object.wrapping_add(field), value);
        }
        Ok(())
    }

    fn display_ready(&self) -> bool {
        self.display_ready
    }

    fn set_f3(&mut self, value: i8) -> Result<(), Self::Error> {
        self.memory.write_byte(0x00F3, value as u8);
        Ok(())
    }

    fn write_long_byte(&mut self, address: u32, value: u8) -> Result<(), Self::Error> {
        self.memory.write_long_byte(address, value);
        Ok(())
    }

    fn write_long_word(&mut self, address: u32, value: u16) -> Result<(), Self::Error> {
        self.memory.write_long_word(address, value);
        Ok(())
    }

    fn read_long_byte(&self, address: u32) -> Result<u8, Self::Error> {
        Ok(self.memory.read_long_byte(address))
    }

    fn read_long_word(&self, address: u32) -> Result<u16, Self::Error> {
        Ok(self.memory.read_long_word(address))
    }

    fn load_table_idle(&self) -> bool {
        self.load_table_idle
    }

    fn request_post_load(&mut self) -> Result<(), Self::Error> {
        self.post_load_requested = true;
        Ok(())
    }

    fn call_65816(&mut self, target: u32, accumulator: Option<u8>) -> Result<(), Self::Error> {
        match target {
            // Retail `$03:DD6E` is exactly one `RTS` byte.  Most script roots
            // invoke this deliberately empty hook during their common setup.
            0x03DD6E => {}
            // Copy the current direct-page map-placement parameters into the
            // auxiliary record selected by player one's `+$2B` slot.
            0x069A2F => {
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    for (source, destination) in [(0x08u16, 0x6BF5u16), (0x0A, 0x6BF7)] {
                        let value = self.memory.read_word(source);
                        self.memory
                            .write_word(destination.wrapping_add(slot), value);
                    }
                    for (source, destination) in [(0x02u16, 0x6BF9u16), (0x04, 0x6BFA)] {
                        let value = self.memory.read_byte(source);
                        self.memory
                            .write_byte(destination.wrapping_add(slot), value);
                    }
                }
            }
            // Alternate auxiliary placement parameters used by the strategic
            // map selection scripts (`$06:9A5F`).
            0x069A5F => {
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    let value = self.memory.read_word(0x08);
                    self.memory.write_word(0x6BFB + slot, value);
                    for (source, destination) in [
                        (0x0A, 0x6B49),
                        (0x02, 0x6BFD),
                        (0x04, 0x6BFE),
                        (0xA7, 0x6B59),
                    ] {
                        let value = self.memory.read_byte(source);
                        self.memory.write_byte(destination + slot, value);
                    }
                }
            }
            // Copy the strategic-map placement into player one and its pilot
            // auxiliary record (`$06:9A92`).
            0x069A92 => {
                let player = self.memory.read_word(PLAYER_ONE);
                if object_index(player).is_some() {
                    for (source, field) in [(0x1DE4, FIELD_X), (0x1DE6, FIELD_Y), (0x1DE8, FIELD_Z)]
                    {
                        let value = self.memory.read_word(source);
                        self.memory.write_word(player + field, value);
                    }
                    let yaw = self.memory.read_byte(0x1DEA);
                    let slot = self.memory.read_word(player + FIELD_PATH);
                    self.memory.write_byte(0x6ABC + slot, yaw);
                    self.memory.write_byte(player + FIELD_ROT_Y, yaw);
                    let inverse_yaw = yaw.wrapping_neg();
                    self.memory.write_byte(0x6B34 + slot, inverse_yaw);
                    self.memory.write_byte(0x0354, inverse_yaw);
                }
            }
            // Mark player auxiliary records with retail bit `$40`.
            0x069ACD => {
                let player_one = self.memory.read_word(PLAYER_ONE);
                self.set_pilot_aux_bit(player_one, 0x6B65, 0x40, true);
                if self.memory.read_word(0x1916) != 0x00C0 {
                    let player_two = self.memory.read_word(PLAYER_TWO);
                    self.set_pilot_aux_bit(player_two, 0x6B65, 0x40, true);
                }
            }
            0x069B04 | 0x069B20 => {
                let player = self.memory.read_word(PLAYER_ONE);
                self.set_pilot_aux_bit(player, 0x6BEB, 0x80, target == 0x069B04);
            }
            0x069B3C | 0x069B4C => {
                let distance = if target == 0x069B3C { 0x4650 } else { 0 };
                self.memory.write_word(0x1DC0, distance);
                self.position_map_player(distance);
            }
            // Map/radar marker projection. The input accumulator selects one
            // exact 16-byte table record in `$686A`; `$0D:DAC5` rasterizes its
            // packed rectangle into the 2 KiB bitplane at `$CF36`.
            0x0DDA7A => {
                let selector = accumulator.unwrap_or(0);
                self.rasterize_map_marker(selector);
                self.map_markers.push(MapMarker {
                    kind: selector & 0x80,
                    table_index: u16::from(selector & 0x7F),
                });
            }
            _ => return Err(Error::UnsupportedMapRoutine(target)),
        }
        Ok(())
    }

    fn spawn_object(&mut self, record: SpawnRecord) -> Result<(), Self::Error> {
        let after = self.memory.read_word(CURRENT_OBJECT);
        let object = allocate(&mut self.memory, after).ok_or(Error::ObjectPoolExhausted)?;
        self.memory.write_word(object + FIELD_SHAPE, record.shape);
        self.memory.write_word(object + FIELD_X, record.x as u16);
        self.memory.write_word(object + FIELD_Y, record.y as u16);
        self.memory.write_word(object + FIELD_Z, record.z as u16);
        self.memory
            .write_byte(object + FIELD_STRATEGY, record.strategy as u8);
        self.memory
            .write_byte(object + FIELD_STRATEGY + 1, (record.strategy >> 8) as u8);
        self.memory
            .write_byte(object + FIELD_STRATEGY + 2, (record.strategy >> 16) as u8);
        self.memory.write_byte(object + 0x20, 0x08);
        if let Some(linked) = record.linked_object {
            self.memory.write_word(object + 0x06, linked);
        }
        Ok(())
    }

    fn set_current_object_path(&mut self, stream_offset: u16) -> Result<(), Self::Error> {
        let object = self.memory.read_word(CURRENT_OBJECT);
        if object_index(object).is_some() {
            self.memory.write_word(object + FIELD_PATH, stream_offset);
        }
        Ok(())
    }

    fn spawn_aux_object(
        &mut self,
        field_04: u16,
        field_06: u16,
        field_08: u16,
        field_0b: u8,
        field_0d: u16,
        field_0f: u16,
    ) -> Result<(), Self::Error> {
        // Retail opcode `$90` uses the independent 512-entry, 19-byte record
        // pool at `$342E`; using the gameplay object allocator here exhausts
        // the 60 real object slots during ordinary map progression.
        let record =
            allocate_map_auxiliary(&mut self.memory).ok_or(Error::MapAuxiliaryPoolExhausted)?;
        self.memory.write_byte(0xD73D, 2);
        self.memory.write_word(record + 0x04, field_04);
        self.memory.write_word(record + 0x06, field_06);
        self.memory.write_word(record + 0x08, field_08);
        self.memory.write_byte(record + 0x0B, field_0b);
        self.memory.write_word(record + 0x0D, field_0d);
        self.memory.write_word(record + 0x0F, field_0f);
        self.memory
            .write_byte(record + 0x18, self.memory.read_byte(0x190E));
        self.memory
            .write_byte(record + 0x12, self.memory.read_byte(0xD73D));
        self.memory.write_byte(record + 0x0A, 0);
        self.memory.write_byte(record + 0x0C, 0);
        Ok(())
    }

    fn configure_slot(
        &mut self,
        slot: u8,
        bit_7_set: bool,
        params: [u8; 7],
    ) -> Result<(), Self::Error> {
        // Opcode `$94` writes a sparse, deliberately overlapping 16-byte
        // table record.  Its four single-byte operands are promoted into the
        // high byte of a word via XBA; the final two word stores overlap at
        // record byte `$0D` exactly as they do at `$03:9104..913D`.
        let base = 0x686Au16.wrapping_add(u16::from(slot) * 0x10);
        for (value, offset) in [
            (params[0], 0x00u16),
            (params[1], 0x04),
            (params[2], 0x06),
            (params[3], 0x0A),
        ] {
            self.memory.write_word(base + offset, u16::from(value) << 8);
        }
        self.memory
            .write_word(base + 0x0C, u16::from_le_bytes([params[4], params[5]]));
        self.memory
            .write_word(base + 0x0D, u16::from_le_bytes([params[5], params[6]]));
        self.memory
            .write_byte(base + 0x0F, if bit_7_set { 0 } else { 2 });
        let configured = self.memory.read_byte(0x1910).wrapping_add(1);
        self.memory.write_byte(0x1910, configured);
        Ok(())
    }

    fn set_gsu_word_01bc(&mut self, value: u16) -> Result<(), Self::Error> {
        self.memory.write_long_word(0x7001BC, value);
        Ok(())
    }

    fn mode(&self) -> u8 {
        self.mode
    }
}

impl Game {
    fn set_pilot_aux_bit(&mut self, player: u16, base: u16, bit: u8, set: bool) {
        if object_index(player).is_none() {
            return;
        }
        let slot = self.memory.read_word(player + FIELD_PATH);
        let address = base.wrapping_add(slot);
        let value = self.memory.read_byte(address);
        self.memory
            .write_byte(address, if set { value | bit } else { value & !bit });
    }

    /// Exact high-level transcription of `$06:9B5A..9C1E`, shared by the
    /// `$9B3C` and `$9B4C` map calls.
    fn position_map_player(&mut self, distance: u16) {
        let player = self.memory.read_word(PLAYER_ONE);
        if object_index(player).is_none() {
            return;
        }

        let phase = self.memory.read_byte(0x1D98).wrapping_add(1) & 7;
        self.memory.write_byte(0x1D98, phase);

        let map_x = self.memory.read_word(0x1DE4);
        let map_z = self.memory.read_word(0x1DE8);
        self.memory.write_word(player + FIELD_X, map_x);
        self.memory.write_word(player + FIELD_Y, 0);
        self.memory.write_word(player + FIELD_Z, map_z);

        let target = self.memory.read_word(0x14D6);
        let target_x = self.memory.read_word(0x1DF3);
        let target_z = self.memory.read_word(0x1DF5);
        if target != 0 {
            self.memory.write_word(target + FIELD_X, target_x);
            self.memory.write_word(target + FIELD_Y, 0);
            self.memory.write_word(target + FIELD_Z, target_z);
        }

        let dx = target_x.wrapping_sub(map_x) as i16;
        let dz = target_z.wrapping_sub(map_z) as i16;
        let facing = sf_core::aim_angle::yanglexy(dx, dz).wrapping_neg();
        self.memory.write_byte(player + FIELD_ROT_Y, facing);

        if distance != 0 {
            let (offset_x, offset_z) =
                sf_core::snes_trig::rotate_16xz(facing, 0, -(distance as i16));
            self.memory
                .write_word(player + FIELD_X, map_x.wrapping_add(offset_x as u16));
            self.memory
                .write_word(player + FIELD_Z, map_z.wrapping_add(offset_z as u16));
        }

        let slot = self.memory.read_word(player + FIELD_PATH);
        let yaw = self.memory.read_byte(0x1DEA);
        self.memory.write_byte(0x6AE6 + slot, 0x16);
        self.memory.write_byte(0x6ABC + slot, yaw);
        self.memory.write_byte(player + FIELD_ROT_Y, yaw);
        let inverse_yaw = yaw.wrapping_neg();
        self.memory.write_byte(0x6B34 + slot, inverse_yaw);
        self.memory.write_byte(0x0354, inverse_yaw);
    }

    /// High-level transcription of `$0D:DA7A..DB70`. The routine draws or
    /// erases a packed rectangular marker in the strategic-map bitplane.
    fn rasterize_map_marker(&mut self, selector: u8) {
        let record = 0x686Au16.wrapping_add(u16::from(selector & 0x7F) * 0x10);
        let packed_x = self.memory.read_word(record);
        let packed_y = self.memory.read_word(record + 0x04);
        let packed_width = self.memory.read_word(record + 0x06);
        let packed_height = self.memory.read_word(record + 0x0A);

        let x = (packed_x.swap_bytes() >> 1) & 0x7F;
        let mut row = (((packed_y & 0xFE00).swap_bytes() << 3).wrapping_add(x >> 3)) & 0x07FF;
        let initial_mask = 1u8 << (x & 7);
        let width = ((packed_width.wrapping_add(0x01FF)).swap_bytes() >> 1) & 0x7F;
        let height = ((packed_height.wrapping_add(0x01FF)).swap_bytes() >> 1) & 0x7F;
        let width_iterations = if width == 0 { 256 } else { usize::from(width) };
        let height_iterations = if height == 0 {
            65_536
        } else {
            usize::from(height)
        };
        let set = selector & 0x80 != 0;

        for _ in 0..height_iterations {
            let mut cursor = row;
            let mut mask = initial_mask;
            for _ in 0..width_iterations {
                let address = 0xCF36u16.wrapping_add(cursor);
                let byte = self.memory.read_byte(address);
                self.memory
                    .write_byte(address, if set { byte | mask } else { byte & !mask });
                let crossed_byte = mask & 0x80 != 0;
                mask = mask.rotate_left(1);
                if crossed_byte {
                    cursor = cursor.wrapping_add(1);
                    if cursor & 7 == 0 {
                        cursor = cursor.wrapping_sub(0x10);
                    }
                }
            }
            row = row.wrapping_add(0x10) & 0x07FF;
        }
    }
}
