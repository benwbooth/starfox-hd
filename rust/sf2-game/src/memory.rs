use std::collections::BTreeMap;

/// Mutable native game-state storage. The two sizes reflect how much source
/// data must be preserved while fields are lifted, but they are offsets in
/// one flat arena rather than independently mapped address spaces.
pub const MAIN_STATE_SIZE: usize = 131_072;
pub const GRAPHICS_WORKSPACE_SIZE: usize = 65_536;
pub const GRAPHICS_WORKSPACE_START: usize = MAIN_STATE_SIZE;
pub const FLAT_STATE_SIZE: usize = MAIN_STATE_SIZE + GRAPHICS_WORKSPACE_SIZE;

#[derive(Clone)]
struct SourceData {
    rom: Box<[u8]>,
}

/// Flat mutable state used by the native SF2 port.
///
/// `SourceData` is immutable input used while generated tables are being
/// converted to typed Rust constants; it is not part of the runtime memory
/// model. `source_address_overrides` is likewise a compatibility ledger for
/// decoded map commands and will disappear as those commands gain semantic
/// fields.
#[derive(Clone)]
pub struct Memory {
    state: Box<[u8; FLAT_STATE_SIZE]>,
    source_data: SourceData,
    source_address_overrides: BTreeMap<u32, u8>,
}

impl Memory {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut memory = Self {
            state: Box::new([0; FLAT_STATE_SIZE]),
            source_data: SourceData {
                rom: rom.into_boxed_slice(),
            },
            source_address_overrides: BTreeMap::new(),
        };
        // Retail reset copies these two raw code/table windows into bank $7F.
        // Path handlers make direct long reads from tables in those windows,
        // so the host must expose the same bytes even though Rust executes the
        // recovered handlers instead of their copied machine code.
        memory.copy_source_data_into_state(0x010000, 0x10000, 0x007E00);
        memory.copy_source_data_into_state(0x050000, 0x17E00, 0x004E00);
        memory
    }

    fn copy_source_data_into_state(&mut self, rom_start: usize, wram_start: usize, length: usize) {
        let available = self
            .source_data
            .rom
            .len()
            .saturating_sub(rom_start)
            .min(length);
        if available != 0 {
            self.state[wram_start..wram_start + available]
                .copy_from_slice(&self.source_data.rom[rom_start..rom_start + available]);
        }
    }

    pub fn flat_state(&self) -> &[u8; FLAT_STATE_SIZE] {
        &self.state
    }

    pub fn main_state(&self) -> &[u8; MAIN_STATE_SIZE] {
        self.state[..MAIN_STATE_SIZE]
            .try_into()
            .expect("main state has a fixed size")
    }

    #[cfg(feature = "oracle-bridge")]
    pub(crate) fn rom(&self) -> &[u8] {
        &self.source_data.rom
    }

    #[cfg(feature = "oracle-bridge")]
    pub(crate) fn gsu_ram(&self) -> &[u8; GRAPHICS_WORKSPACE_SIZE] {
        self.state[GRAPHICS_WORKSPACE_START..]
            .try_into()
            .expect("graphics workspace has a fixed size")
    }

    #[cfg(feature = "oracle-bridge")]
    pub(crate) fn gsu_ram_mut(&mut self) -> &mut [u8; GRAPHICS_WORKSPACE_SIZE] {
        (&mut self.state[GRAPHICS_WORKSPACE_START..])
            .try_into()
            .expect("graphics workspace has a fixed size")
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        self.state[address as usize]
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        self.state[address as usize] = value;
    }

    pub fn read_word(&self, address: u16) -> u16 {
        u16::from_le_bytes([
            self.read_byte(address),
            self.read_byte(address.wrapping_add(1)),
        ])
    }

    pub fn write_word(&mut self, address: u16, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.write_byte(address, low);
        self.write_byte(address.wrapping_add(1), high);
    }

    pub fn read_long_byte(&self, address: u32) -> u8 {
        let bank = (address >> 16) as u8;
        let offset = address as u16;
        match bank {
            0x7E => self.state[offset as usize],
            0x7F => self.state[65_536 + offset as usize],
            0x70 => self.state[GRAPHICS_WORKSPACE_START + offset as usize],
            0x00 if offset < 0x2000 => self.state[offset as usize],
            _ => self
                .lorom_file_offset(bank, offset)
                .and_then(|index| self.source_data.rom.get(index).copied())
                .or_else(|| self.source_address_overrides.get(&address).copied())
                .unwrap_or(0),
        }
    }

    pub fn write_long_byte(&mut self, address: u32, value: u8) {
        let bank = (address >> 16) as u8;
        let offset = address as u16;
        match bank {
            0x7E => self.state[offset as usize] = value,
            0x7F => self.state[65_536 + offset as usize] = value,
            0x70 => self.state[GRAPHICS_WORKSPACE_START + offset as usize] = value,
            0x00 if offset < 0x2000 => self.state[offset as usize] = value,
            _ => {
                self.source_address_overrides.insert(address, value);
            }
        }
    }

    pub fn read_long_word(&self, address: u32) -> u16 {
        u16::from_le_bytes([
            self.read_long_byte(address),
            self.read_long_byte(address.wrapping_add(1)),
        ])
    }

    pub fn write_long_word(&mut self, address: u32, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.write_long_byte(address, low);
        self.write_long_byte(address.wrapping_add(1), high);
    }

    fn lorom_file_offset(&self, bank: u8, address: u16) -> Option<usize> {
        if address < 0x8000 {
            return None;
        }
        let bank = usize::from(bank & 0x7F);
        Some(bank * 0x8000 + usize::from(address & 0x7FFF))
    }
}
