//! Game-variable state — the C globals the phase-1 game core touches.
//!
//! C oracle: `src/game/game_vars.c/h` (GILESALC.INC allocations) plus the
//! `src/sf_rtl.h` WRAM mirror (`g_ram`) and pad state. Only the globals that
//! `world.c` / `obj.c` / `coldet.c` / `map_exec.c` / the `Nmi_GameTick`
//! subset actually read or write are ported; every field cites its C name.
//! game_vars.c declares 246 globals; 43 are ported here (plus the map-VM
//! state world.c/obj.c export: lastplayz/lastzchange/lastmapobj/
//! specialobjtotal/levelfinished in [`crate::world::World`], the alien
//! pool/lists/aldead in [`crate::obj::Objects`]). The rest stay in C until
//! their lanes (strat/render/audio/windows) come over.

/// SNES WRAM size (C `src/types.h` WRAM_SIZE) — backing for the `g_ram`
/// external-variable mirror used by the map VM's setvar/jmpvar opcodes.
pub const WRAM_SIZE: usize = 0x20000;

// ============================================================
// Flag constants (C `src/variables.h`)
// ============================================================
// gameflags (GILESALC.INC)
pub const GF_NOZREMOVE: u8 = 1;
pub const GF_PLAYERDYING: u8 = 2;
pub const GF_BOSSDEAD: u8 = 4;
pub const GF_STRATDONE1: u8 = 8;
pub const GF_STRATDONE2: u8 = 16;
pub const GF_VIEWROT: u8 = 32;
pub const GF_PLAYERDEAD: u8 = 64;
pub const GF_STAGEDONE: u8 = 128;

// pshipflags
pub const PSF_NOCTRL: u8 = 32;
pub const PSF_NOFIRE: u8 = 64;

// pshipflags2
pub const PSF2_PLAYERHP0: u8 = 128;

// pshipflags3
pub const PSF3_INTUNNEL: u8 = 1;
pub const PSF3_ENGINESND: u8 = 2;
pub const PSF3_NOCOLLISIONS: u8 = 8;
pub const PSF3_KEEPPSTRAT: u8 = 64;

// pstratflags
pub const PSTF_NOVDISTC: u8 = 1;
pub const PSTF_INSEQ: u8 = 8;
pub const PSTF_NOTDIE: u8 = 32;

// playerflymode
pub const PFM_SHADOWS: u8 = 8;
pub const PFM_WOBBLE: u8 = 16;

// splayerflymode values
pub const SPFM_NORM: u8 = 0;
pub const SPFM_INSIDE: u8 = 3;
pub const SPFM_TONORM: u8 = 4;

// Game modes (C `src/variables.h`)
pub const SPACE_MODE: u8 = 1;
pub const WATER_MODE: u8 = 2;

// bgflags (C `src/game/bgs.h` BGF_*)
pub const BGF_RESTART: u8 = 0x01;
pub const BGF_BG: u8 = 0x04;
pub const BGF_INFO: u8 = 0x08;

// Gameplay constants (C `src/variables.h` / STRATEQU.INC)
pub const OUTVIEWDIST: i16 = 120;
pub const FRAMESPERAP: u8 = 10;

// Enemy strategy constants (C `src/strat/strat_enemy.h`)
pub const HARD_HP: u8 = 0xFF;
pub const HARD_AP: u8 = 8;
pub const COLLTYPE_ENEMY1: u8 = 0x01;

/// The ported game-variable set. One field per C global (cited); default
/// zero-state matches C BSS, [`GameVars::init`] matches `GameVars_Init()`.
pub struct GameVars {
    // --- Game flags (game_vars.c) ---
    /// C `g_gameflags` (GF_*).
    pub gameflags: u8,

    // --- Player ship flags ---
    /// C `g_pshipflags` (PSF_*).
    pub pshipflags: u8,
    /// C `g_pshipflags2` (PSF2_*).
    pub pshipflags2: u8,
    /// C `g_pshipflags3` (PSF3_*).
    pub pshipflags3: u8,
    /// C `g_pstratflags` (PSTF_*).
    pub pstratflags: u8,
    /// C `g_playerflymode` (PFM_*).
    pub playerflymode: u8,
    /// C `g_splayerflymode` (SPFM_* mode value).
    pub splayerflymode: u8,

    // --- Player state mirrors (written by init_strats_l, GSTRATS.ASM) ---
    /// C `g_player_posx/y/z`.
    pub player_posx: i16,
    pub player_posy: i16,
    pub player_posz: i16,
    /// C `g_playervelZ`.
    pub playervel_z: i16,
    /// C `g_pviewvelz` — view Z velocity, read by the spacebar strats.
    pub pviewvelz: i16,

    // --- Counters ---
    /// C `g_gameframe`.
    pub gameframe: u16,
    /// Runtime RNG state — the ROM's `rand` ($DE-$E1), a 4-byte
    /// subtract-with-borrow chain (`RANDOM` $2F7BF). See `sf_random`. Boot
    /// value 0 (matches the ROM's cleared `rand`).
    pub rng: [u8; 4],
    /// C `g_freezestrats` (bit 0 freezes the strategy update).
    pub freezestrats: u8,
    /// C `g_internalPLAYPT` — authoritative player alien index.
    pub internal_playpt: i16,
    /// C `g_dummyobj` — do_strat_l skip index (STRATROU.ASM dummyobj).
    pub dummyobj: i16,

    // --- Player strategy variables (world.c set_player_* callbacks) ---
    /// C `g_psvar_word1..4`.
    pub psvar_word1: i16,
    pub psvar_word2: i16,
    pub psvar_word3: i16,
    pub psvar_word4: i16,
    /// C `g_minpmoveY`.
    pub minpmove_y: i16,
    /// C `g_viewdist`.
    pub viewdist: i16,

    // --- Map VM state (game_vars.c) ---
    /// C `g_mapcnt` — distance countdown to next map-script execution.
    pub mapcnt: u16,
    /// C `g_mapptr` — map bytecode instruction pointer.
    pub mapptr: u16,
    /// C `g_stagecnt` (setstage opcode).
    pub stagecnt: i16,
    /// C `g_dotsflag` (-1 space dust, 0 none, 1 ground dots).
    pub dotsflag: i16,
    /// C `g_othmusic`.
    pub othmusic: u8,

    // --- Background state ---
    /// C `g_currentbg`.
    pub currentbg: u16,
    /// C `g_bgflags` (BGF_*).
    pub bgflags: u8,
    /// C `g_bg_dmalist`.
    pub bg_dmalist: u16,
    /// C `g_bgtransspeed`.
    pub bgtransspeed: u16,

    // --- Boss/HUD mirrors written by level inline callbacks ---
    /// C `g_bossmaxhp`.
    pub bossmaxhp: u16,
    /// C `g_meters`.
    pub meters: u16,
    /// C `g_circleanim`.
    pub circleanim: i16,
    /// C `g_oncewipe`.
    pub oncewipe: u8,

    // --- Game mode ---
    /// C `g_game_mode` (SPACE_MODE / WATER_MODE).
    pub game_mode: u8,

    // --- Friend HP (world.c friend-alive / CLfriendmsg callbacks) ---
    /// C `g_frog_hp`.
    pub frog_hp: u8,
    /// C `g_bunny_hp`.
    pub bunny_hp: u8,
    /// C `g_falcon_hp` ("cock" in ASM).
    pub falcon_hp: u8,
    /// C `g_numendok` (KSTRATS.ASM theenddead state).
    pub numendok: u8,

    // --- Pad latch (TRANS.ASM lastcont; C sf_rtl.h g_pad1) ---
    /// C `g_pad1`.
    pub pad1: u16,
    /// C `g_lastcont0`.
    pub lastcont0: u8,
    /// C `g_lastcontl0`.
    pub lastcontl0: u8,

    /// C `g_ram` (`src/sf_rtl.h`) — flat WRAM mirror addressed by the map
    /// VM's external-variable opcodes (setvarb/w/l, jmpvar*, setalvarp*).
    pub ram: Vec<u8>,
}

impl Default for GameVars {
    fn default() -> Self {
        GameVars {
            gameflags: 0,
            rng: [0, 0, 0, 0],
            pshipflags: 0,
            pshipflags2: 0,
            pshipflags3: 0,
            pstratflags: 0,
            playerflymode: 0,
            splayerflymode: 0,
            player_posx: 0,
            player_posy: 0,
            player_posz: 0,
            playervel_z: 0,
            pviewvelz: 0,
            gameframe: 0,
            freezestrats: 0,
            internal_playpt: 0,
            dummyobj: 0,
            psvar_word1: 0,
            psvar_word2: 0,
            psvar_word3: 0,
            psvar_word4: 0,
            minpmove_y: 0,
            viewdist: 0,
            mapcnt: 0,
            mapptr: 0,
            stagecnt: 0,
            dotsflag: 0,
            othmusic: 0,
            currentbg: 0,
            bgflags: 0,
            bg_dmalist: 0,
            bgtransspeed: 0,
            bossmaxhp: 0,
            meters: 0,
            circleanim: 0,
            oncewipe: 0,
            game_mode: 0,
            frog_hp: 0,
            bunny_hp: 0,
            falcon_hp: 0,
            numendok: 0,
            pad1: 0,
            lastcont0: 0,
            lastcontl0: 0,
            ram: vec![0u8; WRAM_SIZE],
        }
    }
}

impl GameVars {
    /// C `GameVars_Init()` (src/game/game_vars.c:348) — the subset covering
    /// the ported fields, with the same default values.
    pub fn init() -> Self {
        GameVars {
            playerflymode: PFM_SHADOWS, // shadows on by default
            splayerflymode: SPFM_NORM,
            minpmove_y: -60,
            game_mode: SPACE_MODE,
            frog_hp: 3,
            bunny_hp: 3,
            falcon_hp: 3,
            oncewipe: 1,
            ..GameVars::default()
        }
    }

    /// C `world_read_ext8` (src/game/world.c:134) — WRAM read with
    /// address wrap at WRAM_SIZE.
    pub fn read_ext8(&self, addr: u16) -> u8 {
        self.ram[addr as usize % WRAM_SIZE]
    }

    /// C `world_read_ext16` (src/game/world.c:138).
    pub fn read_ext16(&self, addr: u16) -> u16 {
        let lo = self.ram[addr as usize % WRAM_SIZE] as u16;
        let hi = self.ram[(addr as u32 + 1) as usize % WRAM_SIZE] as u16;
        lo | (hi << 8)
    }

    /// C `world_write_ext8` (src/game/world.c:144).
    pub fn write_ext8(&mut self, addr: u16, value: u8) {
        self.ram[addr as usize % WRAM_SIZE] = value;
    }

    /// C `world_write_ext16` (src/game/world.c:148).
    pub fn write_ext16(&mut self, addr: u16, value: u16) {
        self.ram[addr as usize % WRAM_SIZE] = (value & 0xFF) as u8;
        self.ram[(addr as u32 + 1) as usize % WRAM_SIZE] = (value >> 8) as u8;
    }
}
