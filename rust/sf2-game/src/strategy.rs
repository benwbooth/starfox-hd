use crate::object::*;
use crate::oracle_compat::{Error, Game};

const PLAYER_INIT_ONE: u32 = 0x0682F9;
const PLAYER_INIT_TWO: u32 = 0x0682ED;
const PLAYER_ENTRY: u32 = 0x06845C;
const PLAYER_MAIN: u32 = 0x069C27;

fn strategy(memory: &crate::memory::Memory, object: u16) -> u32 {
    u32::from(memory.read_word(object + FIELD_STRATEGY))
        | (u32::from(memory.read_byte(object + FIELD_STRATEGY + 2)) << 16)
}

fn set_strategy(memory: &mut crate::memory::Memory, object: u16, target: u32) {
    memory.write_word(object + FIELD_STRATEGY, target as u16);
    memory.write_byte(object + FIELD_STRATEGY + 2, (target >> 16) as u8);
}

fn install_auxiliary_callback(
    memory: &mut crate::memory::Memory,
    object: u16,
    kind: u8,
    target: u32,
) -> Result<(), Error> {
    let entry =
        get_or_create_auxiliary_type(memory, object, kind).ok_or(Error::AuxiliaryHeapExhausted)?;
    write_auxiliary_word(memory, entry + 1, target as u16);
    write_auxiliary_byte(memory, entry + 3, (target >> 16) as u8);
    Ok(())
}

impl Game {
    pub(crate) fn tick_strategies(&mut self) -> Result<(), Error> {
        // Retail walks a stable list for a frame. Initializers may allocate
        // auxiliary storage, so preserve the object addresses up front.
        for object in active_objects(&self.memory) {
            let target = strategy(&self.memory, object);
            match target {
                PLAYER_INIT_ONE | PLAYER_INIT_TWO => self.initialize_player_strategy(object)?,
                PLAYER_ENTRY => self.enter_player_main_strategy(object)?,
                PLAYER_MAIN => self.tick_player_main_strategy(object)?,
                0 => {}
                // These are the only three path-bytecode entries owned by
                // `tick_paths`. Other `$7F` addresses (notably `$7F:9DDE`)
                // are ordinary generated per-frame strategies and must run.
                0x7F7E00 | 0x7F7E1E | 0x7F7E53 => {}
                _ => self.run_unported_strategy(target, object)?,
            }
        }
        Ok(())
    }

    /// Execute one exact retail `$06:9C27` player frame through the shared
    /// WRAM compatibility boundary. This remains public for differential
    /// certification while its leaf routines are progressively lifted.
    pub fn tick_player_main_strategy(&mut self, object: u16) -> Result<(), Error> {
        if object_index(object).is_none() {
            return Err(Error::InvalidObject(object));
        }
        if strategy(&self.memory, object) != PLAYER_MAIN {
            return Ok(());
        }
        self.run_unported_strategy(PLAYER_MAIN, object)
    }

    /// Clean-room port of retail `$06:82ED/$06:82F9` and their shared
    /// `$06:832C/$06:8260`, `$06:958C`, and `$07:B0CE` initialization leaves.
    pub fn initialize_player_strategy(&mut self, object: u16) -> Result<(), Error> {
        if object_index(object).is_none() {
            return Err(Error::InvalidObject(object));
        }
        let initializer = strategy(&self.memory, object);
        if !matches!(initializer, PLAYER_INIT_ONE | PLAYER_INIT_TWO) {
            return Ok(());
        }

        let mut flags23 = self.memory.read_byte(object + 0x23);
        if initializer == PLAYER_INIT_TWO {
            flags23 |= 0x40;
        } else {
            flags23 &= !0x40;
        }
        self.memory.write_byte(object + 0x23, flags23);
        self.memory
            .write_byte(object + 0x26, self.memory.read_byte(object + 0x26) | 0x08);
        self.memory.write_byte(0x1DE2, 0);
        self.memory.write_byte(object + 0x1CDA, 1);

        self.memory
            .write_byte(object + 0x21, self.memory.read_byte(object + 0x21) | 1);
        self.memory.write_byte(object + 0x2D, 0xFF);
        self.memory.write_byte(object + 0x2E, 1);
        set_strategy(&mut self.memory, object, PLAYER_ENTRY);
        // `$832C` installs this entry immediately before `$8260` deliberately
        // clears the allocation chain. Preserve the allocator traffic because
        // it is observable in failure behavior and free-list coalescing.
        install_auxiliary_callback(&mut self.memory, object, 0x0C, 0x06F3A4)?;

        free_all_auxiliary(&mut self.memory, object);
        let slot = allocate_auxiliary(&mut self.memory, object, 0x01D8)
            .ok_or(Error::AuxiliaryHeapExhausted)?;
        self.memory.write_word(object + FIELD_PATH, slot);
        for offset in 0..0x01D8u16 {
            self.memory
                .write_byte(AUX_HEAP_BASE.wrapping_add(slot).wrapping_add(offset), 0);
        }
        self.memory.write_byte(object + 0x1CF0, 0xFF);
        self.memory
            .write_byte(0x6A72 + slot, self.memory.read_byte(0x6A72 + slot) & !0x10);
        self.memory.write_word(PLAYER_ONE, object);
        self.memory.write_word(0x1E24, slot);
        self.memory
            .write_byte(0x6BFF + slot, self.memory.read_byte(0x1E14));
        self.memory.write_byte(object + 0x2D, 1);
        let initial_health = self.memory.read_byte(0x1DD1);
        self.memory.write_byte(0x6C00 + slot, initial_health);
        self.memory.write_byte(0x6C38 + slot, initial_health);
        self.memory
            .write_word(0x6C33 + slot, self.memory.read_word(0xD816));
        self.memory
            .write_word(0x6C34 + slot, self.memory.read_word(0xD817));
        for address in [0x150Du16, 0x12C1, 0x1509] {
            self.memory.write_word(address, object);
        }
        self.memory.write_byte(object + 0x1CCB, 0x80);
        self.memory.write_byte(object + 0x2E, 0);
        self.memory
            .write_byte(object + 0x09, self.memory.read_byte(object + 0x09) & !0x08);
        for (field, bit) in [(0x22u16, 0x20u8), (0x20, 0x08), (0x24, 0x04), (0x26, 0x10)] {
            self.memory
                .write_byte(object + field, self.memory.read_byte(object + field) | bit);
        }

        self.reset_player_globals();
        let slot = self.memory.read_word(object + FIELD_PATH);
        for address in [0x6BB8u16, 0x6BCC, 0x6BCE, 0x6BD0] {
            self.memory.write_word(address + slot, 0);
        }
        self.memory.write_word(0x6BBA + slot, 0xFFFF);
        let mut target_flags = self.memory.read_byte(0x6BC2 + slot) | 0x40;
        if self.memory.read_byte(0x1AA6) & 2 == 0 {
            target_flags |= 0x80;
        }
        self.memory.write_byte(0x6BC2 + slot, target_flags);
        set_strategy(&mut self.memory, object, PLAYER_ENTRY);
        Ok(())
    }

    fn reset_player_globals(&mut self) {
        self.memory.write_word(0x1936, 0);
        self.memory.write_word(0x1938, 0);
        for address in [
            0x1DE2u16, 0x1DCE, 0x1DDE, 0x1DDF, 0x1DF2, 0x1DE1, 0x1E08, 0x1B4D, 0x1E2F, 0xD7D5,
            0x1D99, 0x1D72, 0x1D74, 0x1D7A, 0x1E16, 0x1E17, 0x1E0A, 0x1D9D, 0x1E1B, 0x1E0D, 0x1E0E,
            0x1CE5, 0x1CE6, 0x1D71,
        ] {
            self.memory.write_byte(address, 0);
        }
        let lowest = self.memory.read_byte(0x1DD5);
        let current = self.memory.read_byte(0x1DD1);
        if lowest.wrapping_sub(current) & 0x80 != 0 {
            self.memory.write_byte(0x1DD1, lowest);
        }
        self.memory.write_word(0x1DF9, 0x0320);
        self.memory.write_word(0x1DFB, 0xFFF6);
        self.memory.write_byte(0x1D73, 0xFE);
        self.memory.write_byte(0x1D75, 0xFE);
        self.memory.write_byte(0x1D76, 0xFE);
        self.memory.write_word(0x1D7B, 0);
        self.memory.write_byte(0x1E30, 0x64);
        for address in [
            0x1DFFu16, 0x1E01, 0x1E03, 0x1E05, 0x1D92, 0x1D94, 0x1D96, 0x1E18, 0x1D9B, 0xD7EC,
            0xD7EE, 0xD7F0, 0x1E1C, 0x1E1E, 0x1E20, 0x1E4E, 0x1E52, 0x1E3C, 0x1E40, 0x1E48, 0x1E0B,
            0x1E0F, 0x1D6F,
        ] {
            self.memory.write_word(address, 0);
        }
    }

    /// Deterministic first-entry path through `$06:845C` after the shared
    /// initializer has set `$1D72 = 0`. It installs `$06:9C27` and the exact
    /// player auxiliary callback records before the normal per-frame tick.
    pub fn enter_player_main_strategy(&mut self, object: u16) -> Result<(), Error> {
        self.memory.write_word(0x1D69, 0);
        self.memory.write_word(0x1D6B, 0);
        self.memory.write_word(0x1D6D, 0);
        if self.memory.read_byte(0x1D72) != 0 {
            // The nonzero branch is the later menu/stage re-entry path. It is
            // uncommon during a cold start but must not be parked: execute the
            // retail routine exactly until that branch is independently
            // lifted and differential-tested.
            return self.run_unported_strategy(PLAYER_ENTRY, object);
        }
        self.memory
            .write_byte(0x033F + 0x21, self.memory.read_byte(0x033F + 0x21) | 0x20);
        self.memory
            .write_byte(0x1E2F, self.memory.read_byte(0x1E2F) | 0x80);
        self.memory.write_byte(0x1E30, 0x80);
        self.memory.write_byte(0x1E31, 0x80);
        let slot = self.memory.read_word(object + FIELD_PATH);
        self.memory
            .write_byte(0x6C06 + slot, self.memory.read_byte(0x1DD4));
        self.memory
            .write_byte(0x6C05 + slot, self.memory.read_byte(0x1DD3));
        self.memory
            .write_byte(0x6C04 + slot, self.memory.read_byte(0x1DD2));
        self.memory
            .write_word(0x1B84, self.memory.read_word(0x1B84) & !0x0010);
        self.memory
            .write_word(0x1B96, self.memory.read_word(0x1B96) | 0x0004);
        self.memory
            .write_byte(0x6B77 + slot, self.memory.read_byte(0x6B77 + slot) | 1);
        self.memory
            .write_byte(object + 0x23, self.memory.read_byte(object + 0x23) & !2);
        self.memory
            .write_byte(object + 0x26, self.memory.read_byte(object + 0x26) | 8);
        install_auxiliary_callback(&mut self.memory, object, 0x0C, 0x06F3A4)?;
        self.memory
            .write_byte(object + 0x21, self.memory.read_byte(object + 0x21) & !1);
        self.memory
            .write_byte(object + 0x22, self.memory.read_byte(object + 0x22) & !4);
        set_strategy(&mut self.memory, object, PLAYER_MAIN);
        for (kind, callback) in [(0x06, 0x069707), (0x05, 0x069707), (0x07, 0x069707)] {
            install_auxiliary_callback(&mut self.memory, object, kind, callback)?;
        }
        self.memory.write_word(0x6A9D + slot, 0);
        self.memory.write_byte(0x6A9F + slot, 0);

        // Retail has 16-bit A here: `$1DE2/$1DE3` are deliberately read as a
        // word for the table bound, even though `$1DE3.7` is then tested as a
        // separate variant selector below.
        let character = self.memory.read_word(0x1DE2);
        let table_index = if character < 0x000A {
            u32::from(character) * 2
        } else {
            0
        };
        let record = self.memory.read_long_word(0x069D5E + table_index);
        let variant = if self.memory.read_byte(0x1DE3) & 0x80 != 0 {
            2
        } else {
            0
        };
        let shape = self
            .memory
            .read_long_word(0x060000 + u32::from(record) + variant);
        let field_1e56 = self.memory.read_long_word(0x060000 + u32::from(record) + 4);
        let field_1e54 = self.memory.read_long_word(0x060000 + u32::from(record) + 6);
        self.memory.write_word(0x6B7B + slot, shape);
        self.memory.write_word(0x1E56, field_1e56);
        self.memory.write_word(0x1E54, field_1e54);
        self.memory.write_byte(object + 0x1CC7, shape as u8);
        Ok(())
    }
}
