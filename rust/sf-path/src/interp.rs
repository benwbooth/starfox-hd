//! Path interpreter (C oracle: `src/path/paths.c`).
//!
//! PATHS.ASM → C → Rust conversion. PATH bytecode interpreter — scripted
//! movement sequences (decompiled from PATHS.ASM: path_istrat, .strat,
//! mode_table, .move and PATHMACS.ASM opcode definitions).
//!
//! The path interpreter is a bytecode VM embedded within the strategy system.
//! Each path-following alien has `al_sword2` as its instruction pointer into
//! the global "paths" bytecode array. Each frame, the interpreter reads one or
//! more opcodes, applies their effects, then runs the physics "move" phase.
//!
//! Key architectural details (same as C):
//! - `al_sword2`: instruction pointer (16-bit offset into the path blob)
//! - `al_sbyte2`: loop counter
//! - `al_sbyte3`: wait counter
//! - `al_sbyte4`: friend ID (0 = not a friend)
//! - `al_count` / `al_count1`: acceleration parameters
//! - `sflags2` bits: control per-frame behavior (Z-relative, auto-genvecs, …)
//!
//! Structure of the port:
//! - [`PathWorld`] owns what the C translation unit reads/writes directly:
//!   `g_aliens` + the file-static VM state (`g_path_vm`, link/become latches,
//!   inline-callback table) + the `variables.h`/`game_vars.h` globals paths.c
//!   touches (`g_ram`, `g_eroll1`, friend HPs, `g_pviewvelz`, …). The
//!   game-core lane embeds/aliases this when it lands.
//! - [`PathHost`] mirrors, one method per symbol, the external functions
//!   paths.c calls: `SfRtl_Random`, `Strat_TrigSE`, `Strings_SendMessage`,
//!   `World_FindStrategyAddress`, `Strat_GenVecs2D/3D`, `Strat_Chase8`,
//!   `Strat_Chase`, `Strat_AngleXZ`, `Strat_ApplyVelocity`, `Strat_HitFlash`,
//!   `Strat_InitObjVars`, `Strat_SpawnProjectile`, `Strat_Explode`,
//!   `Obj_Alloc`, `Obj_Free`, `Obj_GetPlayer`, plus registered inline-65816
//!   callbacks (C `PathInlineFunc`).
//!
//! Spawn Roffs and aim (GOTOPOS / face*) use ROM helpers in `sf_core`.

use crate::alien::{
    Alien, ObjectVisualKind, StratRef, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE4, ACF_COLLTYPE5,
    AFEXP, ASF2_COLLDISABLE, ASF3_CHILDOBJ, ASF3_MOTHEROBJ, ASF3_NOHITAFFECT, ASF3_SFLAG5,
    ASF3_SFLAG7, ASF4_INVISIBLE, ASF4_SFLAG8, ASF_PARTOBJ, ASF_SHADOW, ASF_TEXTOBJ, ATZREMOVE,
    NUMBER_AL,
};
use crate::alien_compat;
use crate::opcodes::*;
use crate::rom_catalog_data;
use sf_core::shape::resolve_shape_word;

/// Encoded operands for the path globals mirrored directly by `PathWorld`.
/// The `SOURCE_*` values are the original assembled addresses; `LEGACY_*`
/// values occur only in the ROM-less source catalog retained for tests.
const SOURCE_EROLL1_OPERAND: u16 = 0xF168;
const SOURCE_EBYTE3_OPERAND: u16 = 0xF169;
const LEGACY_EROLL1_OPERAND: u16 = 0x2302;
const LEGACY_EBYTE3_OPERAND: u16 = 0x2303;

const PATH_VM_STACK_DEPTH: usize = 64;
const PATH_VM_TRIGGER_CAPACITY: usize = 16;
const PATH_INLINE_CALLBACK_CAPACITY: usize = rom_catalog_data::inline_continuations().len();
const ALIEN_ABS_ALX_START: u16 = 0x100;
/// Object "pointer" 0 means none (SNES convention).
pub const PATH_NULL_OBJ: u16 = 0;
const PATH_MED_PSPEED: i16 = 65;
const RANDOM_GOTO_THRESHOLD: u8 = 127;

// Friend ids (src/variables.h).
pub const FRIEND_RABBIT: u8 = 1;
pub const FRIEND_FALCON: u8 = 2;
pub const FRIEND_FROG: u8 = 3;
pub const FRIEND_ANYONE: u8 = 6;

/// src/variables.h `GF_PLAYERDYING`.
pub const GF_PLAYERDYING: u8 = 2;
/// src/variables.h `PSF2_PLAYERHP0`.
pub const PSF2_PLAYERHP0: u8 = 128;

// Trigger kinds (paths.c anonymous enum).
pub const PATH_TRIGGER_ALWAYS: u8 = 0;
pub const PATH_TRIGGER_2: u8 = 1;
pub const PATH_TRIGGER_4: u8 = 2;
pub const PATH_TRIGGER_8: u8 = 3;
pub const PATH_TRIGGER_16: u8 = 4;
pub const PATH_TRIGGER_32: u8 = 5;
pub const PATH_TRIGGER_64: u8 = 6;
pub const PATH_TRIGGER_128: u8 = 7;
pub const PATH_TRIGGER_WHENHIT: u8 = 8;
pub const PATH_TRIGGER_WHENHITBYPLAYER: u8 = 9;
pub const PATH_TRIGGER_WHENFLAGGED: u8 = 10;
pub const PATH_TRIGGER_WHENSHAPEDEAD: u8 = 11;
pub const PATH_TRIGGER_WHENDEAD: u8 = 12;

/// C `PathTriggerEntry`.
#[derive(Debug, Clone, Copy, Default)]
struct PathTriggerEntry {
    addr: u16,
    trigger: u8,
}

/// Per-live-object path execution state.
///
/// The source stores trigger lists in heap memory referenced by the object's
/// `stratmem` field. Normal object initialization clears that field, so a
/// recycled pool slot starts with no path stack or triggers. This sidecar must
/// therefore be reset on every path-object allocation generation.
#[derive(Clone)]
struct PathVmState {
    /// Position at the start of this tick — used to translate linked
    /// children rigidly by the mother's per-frame delta (retail behavior:
    /// robot/log children drift with the carrier, vx=-28/frame observed).
    prev_x: i16,
    prev_y: i16,
    prev_z: i16,
    delta_valid: bool,
    data: [u16; PATH_VM_STACK_DEPTH],
    sp: u8,
    triggers: [PathTriggerEntry; PATH_VM_TRIGGER_CAPACITY],
    trigger_count: u8,
    in_trigger: bool,
    trigger_force_pending: bool,
    trigger_forced_ip: u16,
    ifnot_pending: bool,
}

impl Default for PathVmState {
    fn default() -> Self {
        PathVmState {
            prev_x: 0,
            prev_y: 0,
            prev_z: 0,
            delta_valid: false,
            data: [0; PATH_VM_STACK_DEPTH],
            sp: 0,
            triggers: [PathTriggerEntry::default(); PATH_VM_TRIGGER_CAPACITY],
            trigger_count: 0,
            in_trigger: false,
            trigger_force_pending: false,
            trigger_forced_ip: 0,
            ifnot_pending: false,
        }
    }
}

/// C `PathInlineCallback` — the C `PathInlineFunc` becomes an opaque callback
/// id dispatched by the host (`PathHost::run_inline`).
#[derive(Debug, Clone, Copy, Default)]
struct PathInlineCallback {
    script_ptr: u16,
    callback: Option<u16>,
    continuation: u16,
}

/// External calls made by `src/path/paths.c`, one method per C symbol.
/// The sf-game lane implements this against the real obj/strat/world/sound
/// modules; tests use a recording implementation.
pub trait PathHost {
    /// `SfRtl_Random` (src/sf_rtl.h) — 16-bit PRNG.
    fn random(&mut self) -> u16;
    /// `Strat_TrigSE` (src/strat/strat_common.h).
    fn trig_se(&mut self, sound_id: u8);
    /// `Strings_SendMessage` (src/game/strings.h).
    fn send_message(&mut self, msg_id: u8);
    /// `World_FindStrategyAddress` (src/game/world.h): 24-bit SNES address →
    /// strategy routine (None = NULL).
    fn find_strategy_address(&mut self, addr24: u32) -> Option<StratRef>;
    /// `Strat_GenVecs2D` (src/strat/strat_common.h).
    fn genvecs_2d(&mut self, al: &mut Alien);
    /// `Strat_GenVecs3D` (src/strat/strat_common.h).
    fn genvecs_3d(&mut self, al: &mut Alien);
    /// `Strat_Chase8` (src/strat/strat_common.h).
    fn chase8(&mut self, current: u8, target: u8, rate: u8) -> u8;
    /// `Strat_Chase` (src/strat/strat_common.h).
    fn chase16(&mut self, current: i16, target: i16, rate: i16) -> i16;
    /// `Strat_AngleXZ` (src/strat/strat_common.h).
    fn angle_xz(&mut self, src: &Alien, dst: &Alien) -> u8;
    /// `Strat_ApplyVelocity` (src/strat/strat_common.h).
    fn apply_velocity(&mut self, al: &mut Alien);
    /// `Strat_HitFlash` (src/strat/strat_enemy.h). Receives the pool and the
    /// object index because the ROM routine resolves the attacker's attack
    /// power through `al_collobjptr` (`hitflash_Istrat` → `s_docoll`).
    fn hit_flash(&mut self, world: &mut PathWorld, idx: u16);
    /// `Strat_InitObjVars` (src/strat/strat_common.h).
    fn init_obj_vars(&mut self, al: &mut Alien);
    /// `Strat_SpawnProjectile` (src/strat/strat_common.h). Returns the index
    /// of the spawned projectile, if any.
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        world: &mut PathWorld,
        owner: u16,
        off_x: i16,
        off_y: i16,
        off_z: i16,
        rot_x: u8,
        rot_y: u8,
        speed: u8,
        lifetime: u8,
        ap: u8,
        coll_type_bit: u8,
    ) -> Option<u16>;
    /// `Strat_Explode` (src/strat/strat_enemy.h).
    fn explode(&mut self, world: &mut PathWorld, idx: u16);
    /// `Obj_Alloc` (src/game/obj.h). Must mark the slot active and link it
    /// into `world.active_list`.
    fn obj_alloc(&mut self, world: &mut PathWorld) -> Option<u16>;
    /// ROM `l_add` insertion used by every path-owned object constructor.
    /// The newly allocated object is seated immediately after the strategy's
    /// current object, so it can execute later in the same strategy pass.
    fn obj_move_after(&mut self, world: &mut PathWorld, idx: u16, after: u16) {
        if idx == after
            || idx as usize >= world.aliens.len()
            || after as usize >= world.aliens.len()
            || !world.aliens[idx as usize].active
            || !world.aliens[after as usize].active
        {
            return;
        }

        let previous = world.aliens[idx as usize].prev;
        let next = world.aliens[idx as usize].next;
        if let Some(previous) = previous {
            world.aliens[previous as usize].next = next;
        } else if world.active_list == Some(idx) {
            world.active_list = next;
        }
        if let Some(next) = next {
            world.aliens[next as usize].prev = previous;
        }

        let after_next = world.aliens[after as usize].next;
        world.aliens[idx as usize].prev = Some(after);
        world.aliens[idx as usize].next = after_next;
        world.aliens[after as usize].next = Some(idx);
        if let Some(after_next) = after_next {
            world.aliens[after_next as usize].prev = Some(idx);
        }
    }
    /// `Obj_Free` (src/game/obj.h). Must unlink from `world.active_list`.
    fn obj_free(&mut self, world: &mut PathWorld, idx: u16);
    /// `Obj_GetPlayer` (src/game/obj.h): index of the player object, if
    /// spawned (C returns NULL otherwise).
    fn player(&mut self, world: &PathWorld) -> Option<u16>;
    /// Registered native inline callback. `callback` is the id passed to
    /// [`PathWorld::register_inline_code`].
    fn run_inline(&mut self, world: &mut PathWorld, self_idx: u16, callback: u16);
    /// Dispatch for strategy refs not owned by this crate
    /// (`StratRef::External`, `StratRef::ParticlePollenStrat`, …) when the
    /// driver runs strategies through [`dispatch_strat`].
    fn run_external_strat(&mut self, world: &mut PathWorld, idx: u16, strat: StratRef);

    /// Read/write a variable referenced by imported retail path data.
    ///
    /// The operand is decoded by the host into a named game-state field. The
    /// path runtime deliberately owns no byte-addressed machine memory.
    fn path_read_ext8(&mut self, world: &PathWorld, encoded_variable: u16) -> u8 {
        match encoded_variable {
            SOURCE_EROLL1_OPERAND | LEGACY_EROLL1_OPERAND => world.eroll1,
            SOURCE_EBYTE3_OPERAND | LEGACY_EBYTE3_OPERAND => world.ebyte3,
            _ => 0,
        }
    }
    fn path_read_ext16(&mut self, _world: &PathWorld, _encoded_variable: u16) -> u16 {
        0
    }
    fn path_write_ext8(&mut self, world: &mut PathWorld, encoded_variable: u16, value: u8) {
        match encoded_variable {
            SOURCE_EROLL1_OPERAND | LEGACY_EROLL1_OPERAND => world.eroll1 = value,
            SOURCE_EBYTE3_OPERAND | LEGACY_EBYTE3_OPERAND => world.ebyte3 = value,
            _ => {}
        }
    }
    fn path_write_ext16(&mut self, _world: &mut PathWorld, _encoded_variable: u16, _value: u16) {}
    /// Direct `g_friends_meter` write performed by P_MSGWITHMETER immediately
    /// before Strings_SendMessage. Shipping hosts must publish it before the
    /// message call because the string routine consumes the 0xFF handshake.
    fn path_set_friends_meter(&mut self, world: &mut PathWorld, value: u8) {
        world.friends_meter = value;
    }
}

/// State owned by the path translation unit + the globals it touches.
pub struct PathWorld {
    /// `g_aliens[NUMBER_AL]` (src/game/obj.c).
    pub aliens: Vec<Alien>,
    /// `g_active_list` (src/game/obj.c) — head index of the `next` chain.
    pub active_list: Option<u16>,
    /// `g_path_data` / `g_path_length`.
    pub path_data: Vec<u8>,
    /// `g_path_offsets` / `g_path_count`.
    pub path_offsets: Vec<u16>,
    /// The assembled ROM blob stores P_SPAWN/P_QSPAWN operands as offsets
    /// from `paths`; the legacy source builder stores stable Rust path ids.
    path_operands_are_offsets: bool,
    /// `s_missing_path_warned[512]`.
    missing_path_warned: Vec<bool>,
    vm: Vec<PathVmState>,
    /// `g_path_link_obj`.
    link_obj: u16,
    /// `g_path_become_obj`.
    become_obj: u16,
    /// `g_path_become_last_obj`.
    become_last_obj: u16,
    /// `g_path_become_path`.
    become_path: u16,
    /// `s_path_inline_callbacks`.
    inline_callbacks: [PathInlineCallback; PATH_INLINE_CALLBACK_CAPACITY],
    /// A registered callback can select a conditional continuation target.
    /// Most actions use their generated static continuation; DINTRO1's
    /// special X chase has two branches.
    inline_return_override: Option<u16>,

    // --- globals from variables.h / game_vars.h touched by paths.c ---
    /// `g_aldead` (src/game/obj.h) — set to request removal of the alien
    /// whose strategy is currently running.
    pub aldead: u8,
    /// `g_gameframe` (game_vars.h).
    pub gameframe: u16,
    /// `g_pviewvelz` (game_vars.h).
    pub pviewvelz: i16,
    /// `g_playerscore` (game_vars.h).
    pub playerscore: u16,
    /// `g_eroll1` (game_vars.h) — gate checkpoint latch (ext addr 0x2302).
    pub eroll1: u8,
    /// `g_ebyte3` (game_vars.h) — fog loop break latch (ext addr 0x2303).
    pub ebyte3: u8,
    /// `g_bunny_hp` (game_vars.h).
    pub bunny_hp: u8,
    /// `g_falcon_hp` (game_vars.h; "cock" in ASM).
    pub falcon_hp: u8,
    /// `g_frog_hp` (game_vars.h).
    pub frog_hp: u8,
    /// `g_minpmoveX` (game_vars.h).
    pub minpmove_x: i16,
    /// `g_maxpmoveX` (game_vars.h).
    pub maxpmove_x: i16,
    /// `g_minpmoveY` (game_vars.h).
    pub minpmove_y: i16,
    /// `g_maxpmoveY` (game_vars.h).
    pub maxpmove_y: i16,
    /// `g_currentlevel` (game_vars.h).
    pub currentlevel: u8,
    /// `g_pshipflags2` (game_vars.h).
    pub pshipflags2: u8,
    /// `g_gameflags` (game_vars.h).
    pub gameflags: u8,
    /// `g_friends_meter` (game_vars.h).
    pub friends_meter: u8,
    /// `g_shapes_table` (src/game/world.h) — shape_id → internal (256 slots).
    pub shapes_table: Vec<u16>,
}

impl Default for PathWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PathWorld {
    pub fn new() -> Self {
        PathWorld {
            aliens: vec![Alien::default(); NUMBER_AL],
            active_list: None,
            path_data: Vec::new(),
            path_offsets: Vec::new(),
            path_operands_are_offsets: false,
            missing_path_warned: vec![false; 512],
            vm: vec![PathVmState::default(); NUMBER_AL],
            link_obj: PATH_NULL_OBJ,
            become_obj: PATH_NULL_OBJ,
            become_last_obj: PATH_NULL_OBJ,
            become_path: 0,
            inline_callbacks: [PathInlineCallback::default(); PATH_INLINE_CALLBACK_CAPACITY],
            inline_return_override: None,
            aldead: 0,
            gameframe: 0,
            pviewvelz: 0,
            playerscore: 0,
            eroll1: 0,
            ebyte3: 0,
            bunny_hp: 0,
            falcon_hp: 0,
            frog_hp: 0,
            minpmove_x: 0,
            maxpmove_x: 0,
            minpmove_y: 0,
            maxpmove_y: 0,
            currentlevel: 0,
            pshipflags2: 0,
            gameflags: 0,
            friends_meter: 0,
            shapes_table: vec![0; 256],
        }
    }

    /// C `Paths_Init`.
    pub fn paths_init(&mut self) {
        self.path_data = Vec::new();
        self.path_offsets = Vec::new();
        self.path_operands_are_offsets = false;
        self.link_obj = PATH_NULL_OBJ;
        self.become_obj = PATH_NULL_OBJ;
        self.become_last_obj = PATH_NULL_OBJ;
        self.become_path = 0;
        self.vm = vec![PathVmState::default(); NUMBER_AL];
        self.inline_callbacks = [PathInlineCallback::default(); PATH_INLINE_CALLBACK_CAPACITY];
        self.inline_return_override = None;
        self.missing_path_warned = vec![false; 512];
    }

    /// C `Paths_LoadData`.
    pub fn paths_load_data(&mut self, data: Vec<u8>, offsets: Vec<u16>) {
        self.path_operands_are_offsets = data.len() == rom_catalog_data::ROM_PATH_CATALOG_SIZE;
        self.path_data = data;
        self.path_offsets = offsets;
    }

    /// Begin a new path-object lifetime in an existing object-pool slot.
    pub fn reset_object_state(&mut self, object: u16) {
        let index = usize::from(object);
        if index < self.vm.len() {
            self.vm[index] = PathVmState::default();
        }
    }

    /// Resolve a path reference embedded inside bytecode. The original ROM
    /// writes offsets relative to the `paths` label, whereas the retained
    /// hand-built fallback catalog writes stable Rust path ids.
    fn paths_resolve_embedded_start(&mut self, value: u16) -> u16 {
        if self.path_operands_are_offsets {
            if (value as usize) < self.path_data.len() {
                return value;
            }
            return 0;
        }
        self.paths_resolve_start(value)
    }

    /// C `Paths_ResolveStart`.
    pub fn paths_resolve_start(&mut self, path_id: u16) -> u16 {
        if !self.path_offsets.is_empty() && (path_id as usize) < self.path_offsets.len() {
            let offset = self.path_offsets[path_id as usize];
            if offset != 0xFFFF {
                return offset;
            }
            if (path_id as usize) < self.missing_path_warned.len()
                && !self.missing_path_warned[path_id as usize]
            {
                self.missing_path_warned[path_id as usize] = true;
                println!("Paths: path id {path_id} is not ported yet; using remove-stub");
            }
            return 0;
        }
        if (path_id as usize) < self.missing_path_warned.len()
            && !self.missing_path_warned[path_id as usize]
        {
            self.missing_path_warned[path_id as usize] = true;
            println!("Paths: path id {path_id} is out of range; using remove-stub");
        }
        0
    }

    /// Select the bytecode offset returned by the current registered action.
    pub fn override_inline_return(&mut self, target: u16) {
        self.inline_return_override = Some(target);
    }

    /// C `Paths_RegisterInlineCode` (fn pointer → host callback id).
    pub fn register_inline_code(&mut self, script_ptr: u16, callback: u16, continuation: u16) {
        for slot in self.inline_callbacks.iter_mut() {
            if slot.callback.is_some() && slot.script_ptr == script_ptr {
                slot.callback = Some(callback);
                slot.continuation = continuation;
                return;
            }
        }
        for slot in self.inline_callbacks.iter_mut() {
            if slot.callback.is_none() {
                slot.script_ptr = script_ptr;
                slot.callback = Some(callback);
                slot.continuation = continuation;
                return;
            }
        }
        panic!("path inline callback catalog exceeds its certified capacity");
    }

    /// C `path_find_inline_code`.
    pub fn find_inline_code(&self, script_ptr: u16) -> Option<(u16, u16)> {
        for slot in self.inline_callbacks.iter() {
            if slot.callback.is_some() && slot.script_ptr == script_ptr {
                return slot.callback.map(|callback| (callback, slot.continuation));
            }
        }
        None
    }

    // --- Bytecode read helpers (C path_read8/8s/16/16s) ---
    fn pread8(&self, ip: u16, offset: i32) -> u8 {
        let addr = (ip as u32).wrapping_add(offset as u32);
        if (addr as usize) < self.path_data.len() {
            self.path_data[addr as usize]
        } else {
            0
        }
    }

    fn pread8s(&self, ip: u16, offset: i32) -> i8 {
        self.pread8(ip, offset) as i8
    }

    fn pread16(&self, ip: u16, offset: i32) -> u16 {
        let addr = (ip as u32).wrapping_add(offset as u32) as usize;
        if addr + 1 < self.path_data.len() {
            (self.path_data[addr] as u16) | ((self.path_data[addr + 1] as u16) << 8)
        } else {
            0
        }
    }

    fn pread16s(&self, ip: u16, offset: i32) -> i16 {
        self.pread16(ip, offset) as i16
    }

    // --- VM stack (C path_vm_push / path_vm_pop) ---
    fn vm_push(&mut self, vi: usize, value: u16) -> bool {
        let vm = &mut self.vm[vi];
        if (vm.sp as usize) >= PATH_VM_STACK_DEPTH {
            return false;
        }
        vm.data[vm.sp as usize] = value;
        vm.sp += 1;
        true
    }

    fn vm_pop(&mut self, vi: usize) -> Option<u16> {
        let vm = &mut self.vm[vi];
        if vm.sp == 0 {
            return None;
        }
        vm.sp -= 1;
        Some(vm.data[vm.sp as usize])
    }
}

// ============================================================
// Small helpers (C statics)
// ============================================================

/// Imported retail blobs address the alien scratch block with SNES-style
/// `$7E:1Cxx` absolutes (`ALX_START=$7E:1CC8`). Normalize them to the port's
/// `0x100 + typed-offset` form before the alien_compat layer sees them.
fn normalize_alx_abs(addr: u16) -> u16 {
    const ALX_START_SNES: u16 = 0x1CC8;
    const ALX_END_SNES: u16 = ALX_START_SNES + 0x40;
    if (ALX_START_SNES..ALX_END_SNES).contains(&addr) {
        0x100 + (addr - ALX_START_SNES)
    } else {
        addr
    }
}

/// C `path_abs_read8` (absolute alien offset: alx block starts at 0x100).
fn path_abs_read8(al: &Alien, abs_offset: u16) -> Option<u8> {
    let abs_offset = normalize_alx_abs(abs_offset);
    if abs_offset < ALIEN_ABS_ALX_START {
        alien_compat::read8(al, abs_offset, false)
    } else {
        alien_compat::read8(al, abs_offset - ALIEN_ABS_ALX_START, true)
    }
}

/// C `path_abs_read16`.
fn path_abs_read16(al: &Alien, abs_offset: u16) -> Option<u16> {
    let abs_offset = normalize_alx_abs(abs_offset);
    if abs_offset < ALIEN_ABS_ALX_START {
        alien_compat::read16(al, abs_offset, false)
    } else {
        alien_compat::read16(al, abs_offset - ALIEN_ABS_ALX_START, true)
    }
}

/// C `path_abs_write8`.
fn path_abs_write8(al: &mut Alien, abs_offset: u16, value: u8) -> bool {
    let abs_offset = normalize_alx_abs(abs_offset);
    if abs_offset < ALIEN_ABS_ALX_START {
        alien_compat::write8(al, abs_offset, false, value)
    } else {
        alien_compat::write8(al, abs_offset - ALIEN_ABS_ALX_START, true, value)
    }
}

/// C `path_abs_write16`.
fn path_abs_write16(al: &mut Alien, abs_offset: u16, value: u16) -> bool {
    let abs_offset = normalize_alx_abs(abs_offset);
    if abs_offset < ALIEN_ABS_ALX_START {
        alien_compat::write16(al, abs_offset, false, value)
    } else {
        alien_compat::write16(al, abs_offset - ALIEN_ABS_ALX_START, true, value)
    }
}

/// C `path_genvecs`.
fn path_genvecs<H: PathHost>(host: &mut H, al: &mut Alien) {
    if al.sflags2 & PSFLAG3_HELI != 0 {
        // Helicopter mode repurposes rotx as velocity, so the ROM's .norotx
        // branch deliberately ignores pitch (PATHS.ASM:416-421).
        host.genvecs_2d(al);
    } else {
        // Ordinary path flight retains pitch and generates full 3D vectors.
        host.genvecs_3d(al);
    }
}

/// C `path_get_obj_by_ptr`.
fn path_get_obj_by_ptr(world: &PathWorld, ptr_index: u16) -> Option<usize> {
    if ptr_index == PATH_NULL_OBJ {
        return None;
    }
    if ptr_index as usize >= NUMBER_AL {
        return None;
    }
    if !world.aliens[ptr_index as usize].active {
        return None;
    }
    Some(ptr_index as usize)
}

/// ROM `Xanglexy_l` — elevation via `xzdiffs_l` adjacent (not float hypot).
fn path_pitch_toward(src: &Alien, dst: &Alien) -> u8 {
    sf_core::aim_angle::xanglexy(
        dst.worldy.wrapping_sub(src.worldy),
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

/// C `path_find_near_shape`.
fn path_find_near_shape(
    world: &PathWorld,
    self_idx: usize,
    shape_id: u16,
    exclude: Option<usize>,
    max_z: i16,
    max_xy: i16,
) -> Option<usize> {
    let mapped_shape = if shape_id < 256 {
        world.shapes_table[(shape_id as u8) as usize]
    } else {
        resolve_shape_word(shape_id)
    };

    let sself = &world.aliens[self_idx];
    let mut best: Option<usize> = None;
    let mut best_metric: i32 = 0x7FFF_FFFF;
    let mut cursor = world.active_list;
    while let Some(idx16) = cursor {
        let i = idx16 as usize;
        let it = &world.aliens[i];
        cursor = it.next;
        if !it.active || i == self_idx || Some(i) == exclude {
            continue;
        }
        if !(it.shape == shape_id || it.shape == mapped_shape) {
            continue;
        }

        let dz = (it.worldz as i32 - sself.worldz as i32).abs() as i16;
        if dz > max_z {
            continue;
        }
        let dx = (it.worldx as i32 - sself.worldx as i32).abs() as i16;
        let dy = (it.worldy as i32 - sself.worldy as i32).abs() as i16;
        if dx > max_xy || dy > max_xy {
            continue;
        }

        let metric = dx as i32 + dy as i32 + dz as i32;
        if metric < best_metric {
            best_metric = metric;
            best = Some(i);
        }
    }
    best
}

/// C `path_fire_weapon`.
fn path_fire_weapon<H: PathHost>(
    world: &mut PathWorld,
    host: &mut H,
    self_idx: usize,
    can_hit_owner: bool,
) -> Option<u16> {
    let (mut speed, weapontype, rotx, roty) = {
        let al = &world.aliens[self_idx];
        (
            if al.vel != 0 { al.vel } else { 48 },
            al.weapontype,
            al.rotx,
            al.roty,
        )
    };
    let mut ap: u8 = 2;
    let lifetime: u8 = 50;

    match weapontype {
        0 => {}
        1 => speed = speed.wrapping_add(8),
        2 => ap = 4,
        _ => speed = speed.wrapping_add(4),
    }

    let shot = host.spawn_projectile(
        world,
        self_idx as u16,
        0,
        0,
        0,
        rotx,
        roty,
        speed,
        lifetime,
        ap,
        ACF_COLLTYPE4,
    );
    if let Some(s) = shot {
        if can_hit_owner {
            world.aliens[s as usize].immuneptr = 0;
        }
    }
    shot
}

/// C `path_get_player` (Obj_GetPlayer + active check).
fn path_get_player<H: PathHost>(world: &PathWorld, host: &mut H) -> Option<usize> {
    let p = host.player(world)? as usize;
    if !world.aliens[p].active {
        return None;
    }
    Some(p)
}

/// Source-axis distance in its wrapping 16-bit coordinate domain.
fn path_axis_distance(first: i16, second: i16) -> i16 {
    let difference = first.wrapping_sub(second);
    if difference < 0 {
        difference.wrapping_neg()
    } else {
        difference
    }
}

/// Source distance predicates branch on the sign of the wrapped subtraction,
/// including at the signed-coordinate boundary.
fn path_axis_distance_is_less(first: i16, second: i16, limit: i16) -> bool {
    path_axis_distance(first, second).wrapping_sub(limit) < 0
}

/// Source `xydiffs_l` Manhattan distance in the wrapping coordinate domain.
fn path_xy_distance(first: &Alien, second: &Alien) -> i16 {
    path_axis_distance(first.worldx, second.worldx)
        .wrapping_add(path_axis_distance(first.worldy, second.worldy))
}

/// Source `s_jmp_outXYdistrng ...,#0,#radius`: a negative wrapped distance is
/// outside the lower bound and equality with the upper bound is outside.
fn path_xy_distance_is_within(first: &Alien, second: &Alien, radius: i16) -> bool {
    let distance = path_xy_distance(first, second);
    distance >= 0 && distance.wrapping_sub(radius) < 0
}

/// C `path_resolve_strategy`.
fn path_resolve_strategy<H: PathHost>(host: &mut H, addr16: u16, bank: u8) -> Option<StratRef> {
    let addr24 = ((bank as u32) << 16) | (addr16 as u32);
    host.find_strategy_address(addr24)
}

/// C `path_friend_hp_ptr` — read half.
fn path_friend_hp(world: &PathWorld, friend_id: u8) -> Option<u8> {
    match friend_id {
        FRIEND_RABBIT => Some(world.bunny_hp),
        FRIEND_FALCON => Some(world.falcon_hp),
        FRIEND_FROG => Some(world.frog_hp),
        _ => None,
    }
}

/// C `path_friend_hp_ptr` — write half.
fn path_friend_hp_set(world: &mut PathWorld, friend_id: u8, value: u8) -> bool {
    match friend_id {
        FRIEND_RABBIT => world.bunny_hp = value,
        FRIEND_FALCON => world.falcon_hp = value,
        FRIEND_FROG => world.frog_hp = value,
        _ => return false,
    }
    true
}

/// C `path_fire_at_target`.
fn path_fire_at_target<H: PathHost>(
    world: &mut PathWorld,
    host: &mut H,
    self_idx: usize,
    can_hit_owner: bool,
    target: Option<usize>,
) {
    let shot = path_fire_weapon(world, host, self_idx, can_hit_owner);
    let (shot, target) = match (shot, target) {
        (Some(s), Some(t)) => (s, t),
        _ => return,
    };
    world.aliens[shot as usize].ptr = target as u16;
}

/// ROM `adiv2` (STRATMAC.INC:712) applied `rate` times with the
/// `nolessrange -(1<<rate),(1<<rate)` prologue — the proportional ease-out
/// chase core of `Achase_var2A` (STRATMAC.INC:525) / `sr8_achase_alvarN`
/// (STRATROU.ASM:2740).
///
/// delta = target - cur as a SIGNED difference (8-bit for the byte form, so
/// angle chases wrap the short way around the circle); |delta| is raised to
/// at least 2^rate, then halved toward zero `rate` times (minimum step ±1);
/// cur += delta. Equal-at-entry leaves cur untouched (the WAIT variants key
/// their "finished" test off that entry equality, not off reaching the
/// target after the step).
fn achase_delta(mut d: i32, rate: u32) -> i32 {
    let lim = 1i32 << rate;
    if d >= 0 && d < lim {
        d = lim;
    }
    if d < 0 && d > -lim {
        d = -lim;
    }
    for _ in 0..rate {
        d = if d >= 0 { d >> 1 } else { -((-d) >> 1) }; // adiv2: toward zero
    }
    d
}

/// ROM `sr8_achase_alvar<rate>` — 8-bit proportional chase (see
/// [`achase_delta`]). Public so the oracle audit can diff it against the ROM
/// routines directly.
pub fn path_achase8(cur: u8, target: u8, rate: u32) -> u8 {
    if cur == target {
        return cur;
    }
    let d = achase_delta(target.wrapping_sub(cur) as i8 as i32, rate);
    cur.wrapping_add(d as u8)
}

/// ROM `sr16_achase_alvar<rate>` — 16-bit proportional chase (see
/// [`achase_delta`]).
pub fn path_achase16(cur: i16, target: i16, rate: u32) -> i16 {
    if cur == target {
        return cur;
    }
    let d = achase_delta(target.wrapping_sub(cur) as i32, rate);
    cur.wrapping_add(d as i16)
}

/// ROM `s_jmp_random` with its default 50-percent parameter.
///
/// The macro compares the generated byte with `(50 * 255) / 100`, so draws
/// 0 through 126 branch and draws 127 through 255 fall through. This is not
/// equivalent to selecting on the low bit of the draw.
pub fn path_random_goto(draw: u16) -> bool {
    (draw as u8) < RANDOM_GOTO_THRESHOLD
}

/// C `path_obj_index_or_null` — every index is in range in this port.
fn path_obj_index_or_null(idx: usize) -> u16 {
    idx as u16
}

/// C `path_obj_from_index_raw`.
fn path_obj_from_index_raw(idx: u16) -> Option<usize> {
    if idx as usize >= NUMBER_AL {
        return None;
    }
    Some(idx as usize)
}

/// C `path_obj_from_index`.
fn path_obj_from_index(world: &PathWorld, idx: u16) -> Option<usize> {
    let i = path_obj_from_index_raw(idx)?;
    if !world.aliens[i].active {
        return None;
    }
    Some(i)
}

/// C `path_clear_child_link`.
fn path_clear_child_link(world: &mut PathWorld, child: usize) {
    let c = &mut world.aliens[child];
    c.sflags3 &= !ASF3_CHILDOBJ;
    c.ptr = PATH_NULL_OBJ;
    c.sword1 = 0;
}

/// C `path_get_mother_obj`.
fn path_get_mother_obj(world: &mut PathWorld, self_idx: usize) -> Option<usize> {
    if world.aliens[self_idx].sflags3 & ASF3_MOTHEROBJ != 0 {
        return Some(self_idx);
    }
    if world.aliens[self_idx].sflags3 & ASF3_CHILDOBJ == 0 {
        return None;
    }
    let ptr = world.aliens[self_idx].ptr;
    match path_obj_from_index(world, ptr) {
        Some(m) => Some(m),
        None => {
            path_clear_child_link(world, self_idx);
            None
        }
    }
}

/// C `path_find_child_obj`.
fn path_find_child_obj(world: &PathWorld, mother: usize, child_num: u8) -> Option<usize> {
    let mut idx = world.aliens[mother].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != PATH_NULL_OBJ && guard > 0 {
        guard -= 1;
        let child = path_obj_from_index(world, idx)?;
        let c = &world.aliens[child];
        if c.sflags3 & ASF3_CHILDOBJ != 0
            && c.ptr == path_obj_index_or_null(mother)
            && c.sbyte1 == child_num
        {
            return Some(child);
        }
        idx = c.sword1 as u16;
    }
    None
}

/// C `path_prune_family_links`.
fn path_prune_family_links(world: &mut PathWorld, self_idx: usize) {
    if world.aliens[self_idx].sflags3 & ASF3_CHILDOBJ != 0 {
        let ptr = world.aliens[self_idx].ptr;
        let mother = path_obj_from_index(world, ptr);
        let mother_ok = match mother {
            Some(m) => world.aliens[m].sflags3 & ASF3_MOTHEROBJ != 0,
            None => false,
        };
        if !mother_ok {
            path_clear_child_link(world, self_idx);
        }
    }

    if world.aliens[self_idx].sflags3 & ASF3_MOTHEROBJ == 0 {
        return;
    }

    let mother_idx = path_obj_index_or_null(self_idx);
    let mut prev_idx = PATH_NULL_OBJ;
    let mut idx = world.aliens[self_idx].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != PATH_NULL_OBJ && guard > 0 {
        guard -= 1;
        let raw = match path_obj_from_index_raw(idx) {
            Some(r) => r,
            None => break,
        };
        let next_idx = world.aliens[raw].sword1 as u16;
        let valid = world.aliens[raw].active
            && world.aliens[raw].sflags3 & ASF3_CHILDOBJ != 0
            && world.aliens[raw].ptr == mother_idx;

        if !valid {
            if prev_idx == PATH_NULL_OBJ {
                world.aliens[self_idx].sword1 = next_idx as i16;
            } else if let Some(prev) = path_obj_from_index_raw(prev_idx) {
                world.aliens[prev].sword1 = next_idx as i16;
            }
            if world.aliens[raw].ptr == mother_idx {
                path_clear_child_link(world, raw);
            }
            idx = next_idx;
            continue;
        }

        prev_idx = idx;
        idx = next_idx;
    }

    if world.aliens[self_idx].sword1 as u16 == PATH_NULL_OBJ {
        world.aliens[self_idx].sflags3 &= !ASF3_MOTHEROBJ;
    }
}

/// C `path_detach_child_from_mother`.
fn path_detach_child_from_mother(world: &mut PathWorld, child: usize) {
    if world.aliens[child].sflags3 & ASF3_CHILDOBJ == 0 {
        return;
    }
    let ptr = world.aliens[child].ptr;
    let mother = match path_obj_from_index(world, ptr) {
        Some(m) => m,
        None => {
            path_clear_child_link(world, child);
            return;
        }
    };

    let child_idx = path_obj_index_or_null(child);
    let mut prev_idx = PATH_NULL_OBJ;
    let mut idx = world.aliens[mother].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != PATH_NULL_OBJ && guard > 0 {
        guard -= 1;
        let it = match path_obj_from_index_raw(idx) {
            Some(i) => i,
            None => break,
        };
        let next_idx = world.aliens[it].sword1 as u16;
        if idx == child_idx {
            if prev_idx == PATH_NULL_OBJ {
                world.aliens[mother].sword1 = next_idx as i16;
            } else if let Some(prev) = path_obj_from_index_raw(prev_idx) {
                world.aliens[prev].sword1 = next_idx as i16;
            }
            break;
        }
        prev_idx = idx;
        idx = next_idx;
    }

    path_clear_child_link(world, child);
    if world.aliens[mother].sword1 as u16 == PATH_NULL_OBJ {
        world.aliens[mother].sflags3 &= !ASF3_MOTHEROBJ;
    }
}

/// C `path_attach_child_to_mother`.
fn path_attach_child_to_mother(
    world: &mut PathWorld,
    mother: usize,
    child: usize,
    child_num: u8,
) -> bool {
    if !world.aliens[mother].active || !world.aliens[child].active || mother == child {
        return false;
    }

    path_detach_child_from_mother(world, child);
    world.aliens[mother].sflags3 |= ASF3_MOTHEROBJ;
    world.aliens[child].sflags3 |= ASF3_CHILDOBJ;
    world.aliens[child].sbyte1 = child_num;
    world.aliens[child].ptr = path_obj_index_or_null(mother);
    world.aliens[child].sword1 = 0;

    let child_idx = path_obj_index_or_null(child);
    if child_idx == PATH_NULL_OBJ {
        path_clear_child_link(world, child);
        return false;
    }

    let first = world.aliens[mother].sword1 as u16;
    if first == PATH_NULL_OBJ {
        world.aliens[mother].sword1 = child_idx as i16;
        return true;
    }

    let mut idx = first;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != PATH_NULL_OBJ && guard > 0 {
        guard -= 1;
        let it = match path_obj_from_index_raw(idx) {
            Some(i) => i,
            None => break,
        };
        let next_idx = world.aliens[it].sword1 as u16;
        if next_idx == PATH_NULL_OBJ {
            world.aliens[it].sword1 = child_idx as i16;
            return true;
        }
        idx = next_idx;
    }

    world.aliens[mother].sword1 = child_idx as i16;
    true
}

/// C `path_remove_all_children`.
fn path_remove_all_children<H: PathHost>(world: &mut PathWorld, host: &mut H, mother: usize) {
    let mut idx = world.aliens[mother].sword1 as u16;
    world.aliens[mother].sword1 = 0;
    world.aliens[mother].sflags3 &= !ASF3_MOTHEROBJ;

    let mut guard = NUMBER_AL as i32 + 1;
    while idx != PATH_NULL_OBJ && guard > 0 {
        guard -= 1;
        let child = match path_obj_from_index_raw(idx) {
            Some(c) => c,
            None => break,
        };
        let next_idx = world.aliens[child].sword1 as u16;
        path_clear_child_link(world, child);
        if world.aliens[child].active {
            host.obj_free(world, child as u16);
        }
        idx = next_idx;
    }
}

/// C `path_switch_context_to` — the VM handle follows the index in this port.
fn path_switch_context_to(world: &PathWorld, self_idx: &mut usize, target: usize) -> bool {
    if !world.aliens[target].active {
        return false;
    }
    *self_idx = target;
    true
}

/// C `path_switch_context_by_ptr`.
fn path_switch_context_by_ptr(world: &PathWorld, self_idx: &mut usize, ptr: u16) -> bool {
    let target = match path_obj_from_index(world, ptr) {
        Some(t) => t,
        None => return false,
    };
    path_switch_context_to(world, self_idx, target)
}

/// C `path_add_rotated_offset` — ROM `s_add_Roffs2pos` flags 1,1,1
/// (STRATMAC.INC:4098 / PATHS.ASM:1790): `rotate_8yx` → `rotate_8yz` →
/// `rotate_8xz`, then `ASL` each axis `scale_shift` times (P_SPAWN uses 2).
fn path_add_rotated_offset(
    world: &mut PathWorld,
    dst: usize,
    src: usize,
    ox: i8,
    oy: i8,
    oz: i8,
    scale_shift: u32,
) {
    let (srotx, sroty, srotz) = {
        let s = &world.aliens[src];
        (s.rotx, s.roty, s.rotz)
    };
    let (dx, dy, dz) =
        sf_core::snes_trig::strat_roffs_full_scaled(srotz, srotx, sroty, ox, oy, oz, scale_shift);
    let d = &mut world.aliens[dst];
    d.worldx = d.worldx.wrapping_add(dx);
    d.worldy = d.worldy.wrapping_add(dy);
    d.worldz = d.worldz.wrapping_add(dz);
}

/// C `path_trigger_condition`.
fn path_trigger_condition(world: &mut PathWorld, self_idx: usize, trigger: u8) -> bool {
    match trigger {
        PATH_TRIGGER_ALWAYS => true,
        PATH_TRIGGER_2..=PATH_TRIGGER_128 => {
            let shift = trigger;
            let mask = (1u16 << shift).wrapping_sub(1);
            world.gameframe & mask == 0
        }
        PATH_TRIGGER_WHENHIT => world.aliens[self_idx].sflags3 & ASF3_SFLAG7 != 0,
        PATH_TRIGGER_WHENHITBYPLAYER => world.aliens[self_idx].sflags3 & ASF3_SFLAG5 != 0,
        PATH_TRIGGER_WHENFLAGGED => {
            if world.aliens[self_idx].sflags4 & ASF4_SFLAG8 != 0 {
                world.aliens[self_idx].sflags4 &= !ASF4_SFLAG8;
                true
            } else {
                false
            }
        }
        PATH_TRIGGER_WHENSHAPEDEAD => {
            let ptr = world.aliens[self_idx].ptr;
            path_get_obj_by_ptr(world, ptr).is_none()
        }
        PATH_TRIGGER_WHENDEAD => world.aliens[self_idx].hp == 0,
        _ => false,
    }
}

/// C `path_register_trigger`.
fn path_register_trigger(world: &mut PathWorld, vi: usize, addr: u16, trigger: u8) {
    let vm = &mut world.vm[vi];
    if vm.trigger_count as usize >= PATH_VM_TRIGGER_CAPACITY {
        return;
    }
    vm.triggers[vm.trigger_count as usize] = PathTriggerEntry { addr, trigger };
    vm.trigger_count += 1;
}

/// C `path_unregister_trigger`.
fn path_unregister_trigger(world: &mut PathWorld, vi: usize, addr: u16) {
    let vm = &mut world.vm[vi];
    if vm.trigger_count == 0 {
        return;
    }
    for i in 0..vm.trigger_count as usize {
        if vm.triggers[i].addr == addr {
            for j in (i + 1)..vm.trigger_count as usize {
                vm.triggers[j - 1] = vm.triggers[j];
            }
            vm.trigger_count -= 1;
            return;
        }
    }
}

/// C `path_run_triggers`.
fn path_run_triggers<H: PathHost>(world: &mut PathWorld, host: &mut H, self_idx: usize) {
    if world.vm[self_idx].in_trigger || world.vm[self_idx].trigger_count == 0 {
        return;
    }

    let mut resume_ip = world.aliens[self_idx].sword2 as u16;
    let count = world.vm[self_idx].trigger_count;
    for i in 0..count {
        if i >= world.vm[self_idx].trigger_count {
            break;
        }
        let entry = world.vm[self_idx].triggers[i as usize];
        if !path_trigger_condition(world, self_idx, entry.trigger) {
            continue;
        }

        world.vm_push(self_idx, 0xFFFF);
        world.vm[self_idx].in_trigger = true;
        world.vm[self_idx].trigger_force_pending = false;
        world.aliens[self_idx].sword2 = entry.addr as i16;
        strat_path_tick(world, host, self_idx as u16);
        world.vm[self_idx].in_trigger = false;

        if world.aldead != 0 {
            return;
        }

        if world.vm[self_idx].trigger_force_pending {
            resume_ip = world.vm[self_idx].trigger_forced_ip;
        }
        {
            let vm = &mut world.vm[self_idx];
            if vm.sp > 0 && vm.data[vm.sp as usize - 1] == 0xFFFF {
                vm.sp -= 1;
            }
        }
        world.aliens[self_idx].sword2 = resume_ip as i16;
    }
}

/// C `path_on_collision` — the path lane's collision strategy
/// ([`StratRef::PathOnCollision`]).
pub fn path_on_collision<H: PathHost>(world: &mut PathWorld, host: &mut H, self_idx: u16) {
    let si = self_idx as usize;

    world.aliens[si].sflags3 |= ASF3_SFLAG7;
    let collobj = world.aliens[si].collobjptr;
    if let Some(other) = path_obj_from_index(world, collobj) {
        if world.aliens[other].collflags & (ACF_COLLTYPE1 | ACF_COLLTYPE5)
            == (ACF_COLLTYPE1 | ACF_COLLTYPE5)
        {
            world.aliens[si].sflags3 |= ASF3_SFLAG5;
        }
    }

    host.hit_flash(world, si.try_into().unwrap());
    // PATHS.ASM jumps into hitflash_Istrat, whose final operation dispatches
    // the object's ordinary strategy. Collision therefore replaces, but does
    // not consume, the path update for this object-strategy pass.
    strat_path_tick(world, host, self_idx);
}

/// C `path_particleexplode_strat` ([`StratRef::ParticleExplodeStrat`]).
pub fn particleexplode_strat(world: &mut PathWorld, self_idx: u16) {
    let al = &mut world.aliens[self_idx as usize];

    // particleexplode_strat:
    // s_particle_data x,0,0,0
    al.sbyte1 = 0;
    al.sbyte2 = 0;
    al.sbyte3 = 0;

    // s_inc_alvar B,x,al_count
    al.count = al.count.wrapping_add(1);

    // s_jmp_alvarEQ B,x,al_count,#40,remove_Istrat
    if al.count == 40 {
        world.aldead = 1;
        return;
    }

    // s_add_alvar W,x,al_worldz,#medpspeed-30
    al.worldz = (al.worldz as i32 + (PATH_MED_PSPEED - 30) as i32) as i16;
}

/// C `path_particleexplode_istrat` ([`StratRef::ParticleExplodeIstrat`]).
pub fn particleexplode_istrat(world: &mut PathWorld, self_idx: u16) {
    let al = &mut world.aliens[self_idx as usize];

    // particleexplode_Istrat
    // s_set_expstrat x,particleexplode_strat
    al.expstratptr = Some(StratRef::ParticleExplodeStrat);
    // s_set_alsflag x,colldisable
    al.sflags2 |= ASF2_COLLDISABLE;
    // s_set_alflag x,exp
    al.flags |= AFEXP;
    // Particle payload for renderer: s_particle_data x,6,60,30
    al.sflags |= ASF_PARTOBJ;
    al.sbyte1 = 6;
    al.sbyte2 = 60;
    al.sbyte3 = 30;
}

/// C `path_trail_tick` ([`StratRef::TrailTick`]).
pub fn trail_tick(world: &mut PathWorld, self_idx: u16) {
    let pviewvelz = world.pviewvelz;
    let al = &mut world.aliens[self_idx as usize];

    // trail_istrat: fade depth colour every other frame except stop colours.
    if al.sbyte1 & 1 == 0 && al.depthoffset != 1 && al.depthoffset != 5 && al.depthoffset != 9 {
        al.depthoffset = al.depthoffset.wrapping_sub(1);
    }

    // Keep in player-relative Z space.
    al.worldz = al.worldz.wrapping_add(pviewvelz);

    // Lifetime countdown (starts at 5 on spawn).
    if al.sbyte1 > 0 {
        al.sbyte1 -= 1;
    }
    if al.sbyte1 == 0 {
        world.aldead = 1;
    }
}

/// C `path_apply_setv`.
fn path_apply_setv(al: &mut Alien, op: u8, dst_off: u8, src_off: u8) {
    match op {
        P_SETVBB => {
            if let Some(src_b) = alien_compat::path_read8(al, src_off) {
                alien_compat::path_write8(al, dst_off, src_b);
            }
        }
        P_SETVWW => {
            if let Some(src_w) = alien_compat::path_read16(al, src_off) {
                alien_compat::path_write16(al, dst_off, src_w);
            }
        }
        P_SETVBW => {
            if let Some(src_w) = alien_compat::path_read16(al, src_off) {
                alien_compat::path_write8(al, dst_off, (src_w & 0xFF) as u8);
            }
        }
        P_SETVWB => {
            if let Some(src_b) = alien_compat::path_read8(al, src_off) {
                alien_compat::path_write16(al, dst_off, (src_b as i8) as i16 as u16);
            }
        }
        _ => {}
    }
}

/// C `path_apply_addv`.
fn path_apply_addv(al: &mut Alien, op: u8, dst_off: u8, src_off: u8) {
    match op {
        P_ADDVBB => {
            if let Some(src_b) = alien_compat::path_read8(al, src_off) {
                alien_compat::path_add8(al, dst_off, src_b as i8);
            }
        }
        P_ADDVWW => {
            if let Some(src_w) = alien_compat::path_read16(al, src_off) {
                alien_compat::path_add16(al, dst_off, src_w as i16);
            }
        }
        P_ADDVBW => {
            if let Some(src_w) = alien_compat::path_read16(al, src_off) {
                alien_compat::path_add8(al, dst_off, (src_w & 0xFF) as u8 as i8);
            }
        }
        P_ADDVWB => {
            if let Some(src_b) = alien_compat::path_read8(al, src_off) {
                alien_compat::path_add16(al, dst_off, (src_b as i8) as i16);
            }
        }
        _ => {}
    }
}

/// C `path_spawn_text_trail`.
fn path_spawn_text_trail<H: PathHost>(world: &mut PathWorld, host: &mut H, self_idx: usize) {
    if world.aliens[self_idx].sflags & ASF_TEXTOBJ == 0 {
        return;
    }
    if world.aliens[self_idx].ty == 0 {
        return;
    }

    let trail = match host.obj_alloc(world) {
        Some(t) => t as usize,
        None => return,
    };
    host.obj_move_after(world, trail as u16, self_idx as u16);

    host.init_obj_vars(&mut world.aliens[trail]);
    let (rotx, roty, rotz, worldx, worldy, worldz, coltab, tx, ty) = {
        let s = &world.aliens[self_idx];
        (
            s.rotx, s.roty, s.rotz, s.worldx, s.worldy, s.worldz, s.coltab, s.tx, s.ty,
        )
    };
    let t = &mut world.aliens[trail];
    t.hp = 10;
    t.ap = 8; // hardAP
    t.stratptr = Some(StratRef::TrailTick);
    t.collstratptr = None;
    t.expstratptr = None;
    t.sflags |= ASF_TEXTOBJ;
    t.sflags2 |= ASF2_COLLDISABLE;
    t.sbyte1 = 5;

    t.rotx = rotx;
    t.roty = roty;
    t.rotz = rotz;
    t.worldx = worldx;
    t.worldy = worldy;
    t.worldz = worldz;
    t.coltab = coltab;
    t.tx = tx;
    t.depthoffset = ty as i16;
}

/// C `path_init_common`.
fn path_init_common(al: &mut Alien) {
    // s_set_alptrs x,.strat,pathhit_istrat,explode_istrat
    al.stratptr = Some(StratRef::PathTick);
    al.collstratptr = Some(StratRef::PathOnCollision);
    al.expstratptr = Some(StratRef::StratExplode);

    // s_set_colltype x,ENEMY1
    al.collflags |= ACF_COLLTYPE2;

    // s_set_alsflag x,shadow
    al.sflags |= ASF_SHADOW;

    // s_set_alvar B,x,al_sbyte4,#0 — friend ID = 0
    al.sbyte4 = 0;
}

// ============================================================
// Path init strategies
// Decompiled from PATHS.ASM:54-70
// ============================================================

/// C `Strat_Path_Init` (path_istrat).
pub fn strat_path_init(al: &mut Alien) {
    path_init_common(al);
}

/// C `Strat_PathDha_Init` (pathdha_istrat: s_set_aldata x,#10,#10 ; bra path_istrat).
pub fn strat_pathdha_init(al: &mut Alien) {
    al.hp = 10;
    al.ap = 10;
    path_init_common(al);
}

/// C `Strat_PathText_Init` (patht_istrat: colldisable + textobj + HP/AP).
pub fn strat_pathtext_init(al: &mut Alien) {
    al.sflags2 |= ASF2_COLLDISABLE;
    al.sflags |= ASF_TEXTOBJ;
    al.hp = 10;
    al.ap = 8; // hardAP
    path_init_common(al);
}

/// Run a [`StratRef`] the way the C engine calls `al->stratptr(al)`.
/// Crate-owned routines dispatch directly; strat-lane / literal-lane routines
/// go through the host.
pub fn dispatch_strat<H: PathHost>(world: &mut PathWorld, host: &mut H, idx: u16, strat: StratRef) {
    match strat {
        StratRef::PathTick => strat_path_tick(world, host, idx),
        StratRef::PathOnCollision => path_on_collision(world, host, idx),
        StratRef::ParticleExplodeIstrat => particleexplode_istrat(world, idx),
        StratRef::ParticleExplodeStrat => particleexplode_strat(world, idx),
        StratRef::TrailTick => trail_tick(world, idx),
        StratRef::StratExplode => host.explode(world, idx),
        other => host.run_external_strat(world, idx, other),
    }
}

// ============================================================
// Path strategy per-frame (PATHS.ASM .strat + .move)
// ============================================================

/// C `Strat_Path_Tick`.
pub fn strat_path_tick<H: PathHost>(world: &mut PathWorld, host: &mut H, self_idx: u16) {
    let mut si = self_idx as usize;
    if si >= NUMBER_AL || !world.aliens[si].active {
        return;
    }
    if world.path_data.is_empty() {
        return;
    }
    let (tick_x0, tick_y0, tick_z0) = {
        let a = &world.aliens[si];
        (a.worldx, a.worldy, a.worldz)
    };

    path_prune_family_links(world, si);

    let mut ip = world.aliens[si].sword2 as u16;
    let mut max_ops: i32 = 50; // Safety: max opcodes per frame
    let mut invert_next_cond = world.vm[si].ifnot_pending;

    // --- Opcode dispatch loop ---
    // C: while (max_ops-- > 0) { switch (opcode) { ... } ip += advance; }
    // `break 'dispatch` == C `goto move_phase`; `continue 'dispatch` == C
    // `continue`; `return` == C `return` (skips the move phase entirely).
    'dispatch: loop {
        let more = max_ops > 0;
        max_ops -= 1;
        if !more {
            break 'dispatch;
        }

        if ip as usize >= world.path_data.len() {
            // Past end of data — remove
            world.aldead = 1;
            return;
        }

        let opcode = world.path_data[ip as usize];
        let advance: i32; // C default is 1; every arm sets it or jumps.

        match opcode {
            // ============================================================
            // Flag opcodes (0-4)
            // ============================================================
            P_RELTOPLAYERON => {
                world.aliens[si].sflags2 |= PSFLAG1_RELZ;
                advance = 1;
            }

            P_RELTOPLAYEROFF => {
                world.aliens[si].sflags2 &= !PSFLAG1_RELZ;
                advance = 1;
            }

            P_ALWAYSGENVECSON => {
                world.aliens[si].sflags2 |= PSFLAG2_GENVECS;
                advance = 1;
            }

            P_ALWAYSGENVECSOFF => {
                world.aliens[si].sflags2 &= !PSFLAG2_GENVECS;
                advance = 1;
            }

            // ============================================================
            // Wait (opcode 2): pause N frames
            // ============================================================
            P_WAIT => {
                let target = world.pread8(ip, 1);
                if world.aliens[si].sbyte3 != target {
                    world.aliens[si].sbyte3 = world.aliens[si].sbyte3.wrapping_add(1);
                    world.aliens[si].sword2 = ip as i16;
                    break 'dispatch; // Don't advance IP, go to move
                }
                world.aliens[si].sbyte3 = 0;
                advance = 2;
            }

            P_WAIT1 => {
                world.aliens[si].sword2 = ip.wrapping_add(1) as i16;
                break 'dispatch;
            }

            // ============================================================
            // Set velocity (opcode 5)
            // ============================================================
            P_SETVEL => {
                let vel = world.pread8(ip, 1);
                world.aliens[si].vel = vel;
                if world.aliens[si].sflags2 & PSFLAG2_GENVECS == 0 {
                    path_genvecs(host, &mut world.aliens[si]);
                }
                advance = 2;
            }

            // ============================================================
            // Loop (opcode 6): repeat N times
            // ============================================================
            P_LOOP => {
                let count = world.pread8(ip, 1);
                if world.aliens[si].sbyte2 == count {
                    world.aliens[si].sbyte2 = 0;
                    advance = 4;
                } else {
                    world.aliens[si].sbyte2 = world.aliens[si].sbyte2.wrapping_add(1);
                    ip = world.pread16(ip, 2);
                    world.aliens[si].sword2 = ip as i16;
                    break 'dispatch;
                }
            }

            // ============================================================
            // Stack loops
            // ============================================================
            P_DO => {
                let loop_count = world.pread16(ip, 1);
                world.vm_push(si, ip.wrapping_add(3));
                world.vm_push(si, loop_count);
                advance = 3;
            }

            P_DOQ => {
                let loop_count = world.pread8(ip, 1) as u16;
                world.vm_push(si, ip.wrapping_add(2));
                world.vm_push(si, loop_count);
                advance = 2;
            }

            P_DOALVARB => {
                let offset = world.pread8(ip, 1);
                let loop_count = alien_compat::path_read8(&world.aliens[si], offset).unwrap_or(0);
                world.vm_push(si, ip.wrapping_add(2));
                world.vm_push(si, loop_count as u16);
                advance = 2;
            }

            P_DOALVARW => {
                let offset = world.pread8(ip, 1);
                let loop_count = alien_compat::path_read16(&world.aliens[si], offset).unwrap_or(0);
                world.vm_push(si, ip.wrapping_add(2));
                world.vm_push(si, loop_count);
                advance = 2;
            }

            P_NEXT | P_INEXT => {
                let loop_count = match world.vm_pop(si) {
                    Some(v) => v.wrapping_sub(1),
                    None => {
                        advance = 1;
                        ip = ip.wrapping_add(advance as u16);
                        world.aliens[si].sword2 = ip as i16;
                        continue 'dispatch;
                    }
                };

                if loop_count != 0 {
                    let loop_ip = match world.vm_pop(si) {
                        Some(v) => v,
                        None => {
                            advance = 1;
                            ip = ip.wrapping_add(advance as u16);
                            world.aliens[si].sword2 = ip as i16;
                            continue 'dispatch;
                        }
                    };

                    world.vm_push(si, loop_ip);
                    world.vm_push(si, loop_count);

                    ip = loop_ip;
                    world.aliens[si].sword2 = ip as i16;
                    if opcode == P_NEXT {
                        break 'dispatch;
                    }
                    continue 'dispatch;
                }

                world.vm_pop(si); // discard stored loop target
                advance = 1;
            }

            P_BREAK => {
                let target = world.pread16(ip, 1);
                world.vm_pop(si);
                world.vm_pop(si);
                ip = target;
                world.aliens[si].sword2 = ip as i16;
                continue 'dispatch;
            }

            P_BREAKC => {
                world.vm_pop(si);
                world.vm_pop(si);
                advance = 1;
            }

            // ============================================================
            // Add byte to alien var (opcode 7)
            // ============================================================
            P_ADDB => {
                let offset = world.pread8(ip, 1);
                let value = world.pread8s(ip, 2);
                alien_compat::path_add8(&mut world.aliens[si], offset, value);
                advance = 3;
            }

            // ============================================================
            // Add word to alien var (opcode 8)
            // ============================================================
            P_ADDW => {
                let offset = world.pread8(ip, 1);
                let value = world.pread16s(ip, 2);
                alien_compat::path_add16(&mut world.aliens[si], offset, value);
                advance = 4;
            }

            // ============================================================
            // Face player (opcode 9)
            // ============================================================
            P_FACEPLAYER => {
                if let Some(pi) = host.player(world).map(|p| p as usize) {
                    // ROM s_obj2obj_3dangle: nega(Yanglexy) + Xanglexy, rate 2.
                    let target_yaw = host
                        .angle_xz(&world.aliens[si], &world.aliens[pi])
                        .wrapping_neg();
                    let target_pitch = path_pitch_toward(&world.aliens[si], &world.aliens[pi]);
                    let (roty, rotx) = (world.aliens[si].roty, world.aliens[si].rotx);
                    world.aliens[si].roty = path_achase8(roty, target_yaw, 2);
                    world.aliens[si].rotx = path_achase8(rotx, target_pitch, 2);
                }
                advance = 1;
            }

            // ============================================================
            // End / remove (opcodes 15, 21)
            // ============================================================
            P_END => {
                world.aliens[si].type_ |= ATZREMOVE;
                world.aliens[si].sword2 = ip as i16;
                break 'dispatch;
            }

            P_REMOVE => {
                if world.aliens[si].sflags3 & ASF3_CHILDOBJ != 0 {
                    path_detach_child_from_mother(world, si);
                }
                if world.aliens[si].sflags3 & ASF3_MOTHEROBJ != 0 {
                    path_remove_all_children(world, host, si);
                }
                world.aldead = 1;
                return;
            }

            P_REMOVECHILD => {
                let child_num = world.pread8(ip, 1);
                if let Some(mother) = path_get_mother_obj(world, si) {
                    if let Some(child) = path_find_child_obj(world, mother, child_num) {
                        if world.aliens[child].active {
                            path_clear_child_link(world, child);
                            host.obj_free(world, child as u16);
                        }
                    }
                }
                advance = 2;
            }

            P_EXPLODE => {
                let friend_id = world.aliens[si].sbyte4;
                path_friend_hp_set(world, friend_id, 0);
                if world.aliens[si].sflags3 & ASF3_CHILDOBJ != 0 {
                    path_detach_child_from_mother(world, si);
                }
                if world.aliens[si].sflags3 & ASF3_MOTHEROBJ != 0 {
                    path_remove_all_children(world, host, si);
                }
                host.explode(world, si as u16);
                return;
            }

            // ============================================================
            // Set byte/word (opcodes 16, 17)
            // ============================================================
            P_SETB => {
                let value = world.pread8(ip, 1);
                let offset = world.pread8(ip, 2);
                alien_compat::path_write8(&mut world.aliens[si], offset, value);
                advance = 3;
            }

            P_SETW => {
                let value = world.pread16s(ip, 1);
                let offset = world.pread8(ip, 3);
                alien_compat::path_write16(&mut world.aliens[si], offset, value as u16);
                advance = 4;
            }

            P_FINDSHAPE => {
                let shape = world.pread16(ip, 1);
                let found = path_find_near_shape(world, si, shape, None, 7000, 7000);
                world.aliens[si].ptr = match found {
                    Some(f) => f as u16,
                    None => PATH_NULL_OBJ,
                };
                advance = 3;
            }

            P_FINDNEXTSHAPE => {
                let shape = world.pread16(ip, 1);
                let cur = path_get_obj_by_ptr(world, world.aliens[si].ptr);
                let found = path_find_near_shape(world, si, shape, cur, 3000, 3000);
                world.aliens[si].ptr = match found {
                    Some(f) => f as u16,
                    None => PATH_NULL_OBJ,
                };
                advance = 3;
            }

            P_FACESHAPE => {
                let obj = match path_get_obj_by_ptr(world, world.aliens[si].ptr) {
                    Some(o) => o,
                    None => {
                        advance = 1;
                        ip = ip.wrapping_add(advance as u16);
                        world.aliens[si].sword2 = ip as i16;
                        continue 'dispatch;
                    }
                };
                // ROM s_obj2obj_3dangle rate 3: nega(Yanglexy) + Xanglexy.
                let target_yaw = host
                    .angle_xz(&world.aliens[si], &world.aliens[obj])
                    .wrapping_neg();
                let target_pitch = path_pitch_toward(&world.aliens[si], &world.aliens[obj]);
                let (roty, rotx) = (world.aliens[si].roty, world.aliens[si].rotx);
                // Finishes only when both angles were already equal AT ENTRY.
                let done = roty == target_yaw && rotx == target_pitch;
                world.aliens[si].roty = path_achase8(roty, target_yaw, 3);
                world.aliens[si].rotx = path_achase8(rotx, target_pitch, 3);
                if !done {
                    world.aliens[si].sword2 = ip as i16;
                    break 'dispatch;
                }
                advance = 1;
            }

            P_GOTOPOS | P_GOTOSHAPEPOS => {
                let off_x = world.pread16s(ip, 1);
                let off_y = world.pread16s(ip, 3);
                let off_z = world.pread16s(ip, 5);
                let speed = world.pread8(ip, 7);

                let mut target_x = off_x;
                let mut target_y = off_y;
                let mut target_z = off_z;

                if opcode == P_GOTOSHAPEPOS {
                    let obj = match path_get_obj_by_ptr(world, world.aliens[si].ptr) {
                        Some(o) => o,
                        None => {
                            advance = 8;
                            ip = ip.wrapping_add(advance as u16);
                            world.aliens[si].sword2 = ip as i16;
                            continue 'dispatch;
                        }
                    };
                    let o = &world.aliens[obj];
                    target_x = (o.worldx as i32 + off_x as i32) as i16;
                    target_y = (o.worldy as i32 + off_y as i32) as i16;
                    target_z = (o.worldz as i32 + off_z as i32) as i16;
                } else if let Some(pi) = host.player(world).map(|p| p as usize) {
                    let p = &world.aliens[pi];
                    target_x = (p.worldx as i32 + off_x as i32) as i16;
                    target_y = (p.worldy as i32 + off_y as i32) as i16;
                    target_z = (p.worldz as i32 + off_z as i32) as i16;
                }

                if world.aliens[si].sflags2 & PSFLAG1_RELZ != 0 {
                    world.aliens[si].worldz = world.aliens[si].worldz.wrapping_add(world.pviewvelz);
                }

                let dx = target_x.wrapping_sub(world.aliens[si].worldx);
                let dy = target_y.wrapping_sub(world.aliens[si].worldy);
                let dz = target_z.wrapping_sub(world.aliens[si].worldz);
                if (dx as i32).abs() <= 4 && (dy as i32).abs() <= 4 && (dz as i32).abs() <= 4 {
                    advance = 8;
                    ip = ip.wrapping_add(advance as u16);
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }

                // ROM s_goto_WP → s_obj2WP_angle: nega(anglexy_abs) + Xanglexabs.
                let target_yaw = sf_core::aim_angle::yanglexy_nega(dx, dz);
                let target_pitch = sf_core::aim_angle::xanglexabs(dy, dx, dz);

                let (roty, rotx) = (world.aliens[si].roty, world.aliens[si].rotx);
                // ROM angle chase rate 2 (PATHS.ASM:1091).
                world.aliens[si].roty = path_achase8(roty, target_yaw, 2);
                world.aliens[si].rotx = path_achase8(rotx, target_pitch, 2);
                world.aliens[si].vel = speed;
                path_genvecs(host, &mut world.aliens[si]);
                host.apply_velocity(&mut world.aliens[si]);
                world.aliens[si].sword2 = ip as i16;
                return;
            }

            P_IMMUNE => {
                if let Some(obj) = path_get_obj_by_ptr(world, world.aliens[si].ptr) {
                    world.aliens[obj].immuneptr = si as u16;
                }
                advance = 1;
            }

            P_SHAPEDEAD => {
                let target = world.pread16(ip, 1);
                let mut take = path_get_obj_by_ptr(world, world.aliens[si].ptr).is_none();
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_LINK => {
                let self_idx16 = path_obj_index_or_null(si);
                if world.link_obj == PATH_NULL_OBJ {
                    world.link_obj = self_idx16;
                } else {
                    if let Some(other) = path_obj_from_index(world, world.link_obj) {
                        if other != si {
                            world.aliens[si].ptr = world.link_obj;
                            world.aliens[other].ptr = self_idx16;
                        }
                    }
                    world.link_obj = PATH_NULL_OBJ;
                }
                advance = 1;
            }

            P_INITANIM => {
                world.aliens[si].animframe = world.pread8(ip, 1);
                advance = 2;
            }

            P_ADDANIM => {
                world.aliens[si].animframe =
                    world.aliens[si].animframe.wrapping_add(world.pread8(ip, 2));
                advance = 3;
            }

            P_DEBRIS => {
                world.aliens[si].debrisshape = resolve_shape_word(world.pread16(ip, 1));
                advance = 3;
            }

            P_FRIEND => {
                let friend_id = world.pread8(ip, 1);
                if friend_id == FRIEND_ANYONE {
                    // ROM .friend friend_anyone, randomselectmode=0
                    // (GAME.INC:67 selects the IFEQ block, PATHS.ASM:1262-1321).
                    // This is a WEIGHTED pick over the living wingmen, not a
                    // uniform choice — and the exact number of random_l draws
                    // is load-bearing because the RNG is a shared stream.
                    // cock=falcon, bunny=rabbit, frog=frog (VARS.INC:180-182).
                    let cock = world.falcon_hp > 0;
                    let bunny = world.bunny_hp > 0;
                    let frog = world.frog_hp > 0;
                    let pick: Option<u8> = if cock {
                        if frog {
                            if bunny {
                                // .404020frogbunnycock: all three alive. One
                                // random_l: <50 falcon (~20%), <150 rabbit
                                // (~40%), else frog (~40%) (PATHS.ASM:1271-1277).
                                let r = host.random() as u8;
                                Some(if r < 50 {
                                    FRIEND_FALCON
                                } else if r < 150 {
                                    FRIEND_RABBIT
                                } else {
                                    FRIEND_FROG
                                })
                            } else {
                                // .4060cockfrog: falcon+frog, rabbit dead.
                                // s_jmp_random .cock,40 -> 40*255/100 = 102:
                                // r<102 falcon else frog (PATHS.ASM:1283-1285).
                                let r = host.random() as u8;
                                Some(if r < 102 { FRIEND_FALCON } else { FRIEND_FROG })
                            }
                        } else if bunny {
                            // .4060cockbunny: falcon+rabbit, frog dead.
                            // s_jmp_random .cock,40 -> 102: r<102 falcon else
                            // rabbit (PATHS.ASM:1278-1282).
                            let r = host.random() as u8;
                            Some(if r < 102 {
                                FRIEND_FALCON
                            } else {
                                FRIEND_RABBIT
                            })
                        } else {
                            // Only falcon alive (source branch .4060cockbunny;
                            // beq .cock, PATHS.ASM:1279-1280) — no draw.
                            Some(FRIEND_FALCON)
                        }
                    } else if frog {
                        if bunny {
                            // .5050bunnyfrog: falcon dead, frog+rabbit alive.
                            // s_jmp_random .frog default 50 -> 50*255/100 = 127:
                            // r<127 frog else rabbit (PATHS.ASM:1286-1292).
                            let r = host.random() as u8;
                            Some(if r < 127 { FRIEND_FROG } else { FRIEND_RABBIT })
                        } else {
                            // Only frog alive (source branch .5050bunnyfrog;
                            // beq .frog, PATHS.ASM:1289-1290) — no draw.
                            Some(FRIEND_FROG)
                        }
                    } else if bunny {
                        // .nooneleft: only rabbit alive (PATHS.ASM:1309-1312).
                        Some(FRIEND_RABBIT)
                    } else {
                        // .nonealive: nobody left, sbyte4 unchanged
                        // (PATHS.ASM:1320-1321).
                        None
                    };
                    if let Some(f) = pick {
                        world.aliens[si].sbyte4 = f;
                    }
                } else if let Some(hp) = path_friend_hp(world, friend_id) {
                    if hp > 0 {
                        world.aliens[si].sbyte4 = friend_id;
                    }
                }
                advance = 2;
            }

            P_SPAWN | P_SPAWNLINK => {
                let shape = world.pread16(ip, 1);
                let path_start = world.pread16(ip, 3);
                let rotx_off = world.pread8s(ip, 5);
                let roty_off = world.pread8s(ip, 6);
                let rotz_off = world.pread8s(ip, 7);
                let hp = world.pread8(ip, 8);
                let ap = world.pread8(ip, 9);
                let off_x = world.pread8s(ip, 10);
                let off_y = world.pread8s(ip, 11);
                let off_z = world.pread8s(ip, 12);

                if let Some(na) = host.obj_alloc(world).map(|a| a as usize) {
                    host.obj_move_after(world, na as u16, si as u16);
                    host.init_obj_vars(&mut world.aliens[na]);
                    world.reset_object_state(na as u16);
                    world.aliens[na].shape = if shape < 256 {
                        world.shapes_table[(shape as u8) as usize]
                    } else {
                        resolve_shape_word(shape)
                    };
                    strat_path_init(&mut world.aliens[na]);
                    let start = world.paths_resolve_embedded_start(path_start);
                    world.aliens[na].sword2 = start as i16;
                    let (wx, wy, wz, srotx, sroty, srotz) = {
                        let s = &world.aliens[si];
                        (s.worldx, s.worldy, s.worldz, s.rotx, s.roty, s.rotz)
                    };
                    world.aliens[na].worldx = wx;
                    world.aliens[na].worldy = wy;
                    world.aliens[na].worldz = wz;
                    // ROM stores coord/4 in the P_SPAWN payload (PATHMACS.ASM:1167)
                    // and scales x4 after rotation (PATHS.ASM:1790).
                    path_add_rotated_offset(world, na, si, off_x, off_y, off_z, 2);
                    world.aliens[na].rotx = srotx.wrapping_add(rotx_off as u8);
                    world.aliens[na].roty = sroty.wrapping_add(roty_off as u8);
                    world.aliens[na].rotz = srotz.wrapping_add(rotz_off as u8);
                    world.aliens[na].hp = hp;
                    world.aliens[na].ap = ap;
                    world.become_last_obj = path_obj_index_or_null(na);

                    if opcode == P_SPAWNLINK {
                        let new_idx = path_obj_index_or_null(na);
                        let self_idx16 = path_obj_index_or_null(si);
                        world.aliens[si].ptr = new_idx;
                        if new_idx != PATH_NULL_OBJ {
                            world.aliens[na].ptr = self_idx16;
                        }
                    }
                }
                advance = 13;
            }

            P_SPAWNCHILD => {
                let shape = world.pread16(ip, 1);
                let path_start = world.pread16(ip, 3);
                let rotx = world.pread8(ip, 5);
                let roty = world.pread8(ip, 6);
                let rotz = world.pread8(ip, 7);
                let hp = world.pread8(ip, 8);
                let ap = world.pread8(ip, 9);
                let off_x = world.pread8s(ip, 10);
                let off_y = world.pread8s(ip, 11);
                let off_z = world.pread8s(ip, 12);
                let child_num = world.pread8(ip, 13);

                if let Some(na) = host.obj_alloc(world).map(|a| a as usize) {
                    host.obj_move_after(world, na as u16, si as u16);
                    host.init_obj_vars(&mut world.aliens[na]);
                    world.reset_object_state(na as u16);
                    world.aliens[na].shape = if shape < 256 {
                        world.shapes_table[(shape as u8) as usize]
                    } else {
                        resolve_shape_word(shape)
                    };
                    strat_path_init(&mut world.aliens[na]);
                    let start = world.paths_resolve_embedded_start(path_start);
                    world.aliens[na].sword2 = start as i16;
                    let mother = path_get_mother_obj(world, si).unwrap_or(si);
                    path_attach_child_to_mother(world, mother, na, child_num);
                    world.aliens[na].childx = off_x as u8;
                    world.aliens[na].childy = off_y as u8;
                    world.aliens[na].childz = off_z as u8;
                    world.aliens[na].childrotx = rotx;
                    world.aliens[na].childroty = roty;
                    world.aliens[na].childrotz = rotz;

                    // Positioning (and rots) are handled by the mother's
                    // per-frame child-follow below — it runs post-movement
                    // this same tick, matching retail `.makeit` ordering.
                    world.aliens[na].hp = hp;
                    world.aliens[na].ap = ap;
                    world.become_last_obj = path_obj_index_or_null(na);
                }
                advance = 14;
            }

            P_CHILDDEAD => {
                let child_num = world.pread8(ip, 1);
                let target = world.pread16(ip, 2);
                // ROM .childdead (PATHS.ASM:1813-1828): with NO mother
                // (objtobemother yields al_ptr==0 -> .cannaedojim2) it falls
                // through — it does NOT treat "no mother" as "child dead".
                let mother = path_get_mother_obj(world, si);
                let mut take = false;
                if let Some(m) = mother {
                    take = path_find_child_obj(world, m, child_num).is_none();
                }
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_LINKCHILD => {
                let child_num = world.pread8(ip, 1);
                let mother = path_get_mother_obj(world, si);
                let child = mother.and_then(|m| path_find_child_obj(world, m, child_num));
                world.aliens[si].ptr = match child {
                    Some(c) => path_obj_index_or_null(c),
                    None => PATH_NULL_OBJ,
                };
                advance = 2;
            }

            P_FLAGCHILD => {
                let child_num = world.pread8(ip, 1);
                let mother = path_get_mother_obj(world, si);
                let child = mother.and_then(|m| path_find_child_obj(world, m, child_num));
                if let Some(c) = child {
                    world.aliens[c].sflags4 |= ASF4_SFLAG8;
                }
                advance = 2;
            }

            P_UNLINKCHILD => {
                let child_num = world.pread8(ip, 1);
                let mother = path_get_mother_obj(world, si);
                let child = mother.and_then(|m| path_find_child_obj(world, m, child_num));
                if let Some(c) = child {
                    path_detach_child_from_mother(world, c);
                }
                advance = 2;
            }

            P_FLAGMOTHER => {
                if let Some(mother) = path_get_mother_obj(world, si) {
                    world.aliens[mother].sflags4 |= ASF4_SFLAG8;
                }
                advance = 1;
            }

            P_FLAGSHAPE => {
                if let Some(obj) = path_get_obj_by_ptr(world, world.aliens[si].ptr) {
                    world.aliens[obj].sflags4 |= ASF4_SFLAG8;
                }
                advance = 1;
            }

            P_UNLINKSELF => {
                if world.aliens[si].sflags3 & ASF3_CHILDOBJ != 0 {
                    path_detach_child_from_mother(world, si);
                }
                advance = 1;
            }

            P_BECOMECHILD => {
                let caller_ip = ip;
                let child_num = world.pread8(ip, 1);
                world.become_obj = path_obj_index_or_null(si);
                world.become_path = world.aliens[si].sword2 as u16;

                let mother = path_get_mother_obj(world, si);
                let child = mother.and_then(|m| path_find_child_obj(world, m, child_num));
                if !world.vm[si].in_trigger {
                    if let Some(c) = child {
                        if path_switch_context_to(world, &mut si, c) {
                            ip = caller_ip;
                            world.aliens[si].sword2 = ip as i16;
                        }
                    }
                }
                advance = 2;
            }

            P_BECOMESHAPE => {
                let caller_ip = ip;
                world.become_obj = path_obj_index_or_null(si);
                world.become_path = world.aliens[si].sword2 as u16;
                let ptr = world.aliens[si].ptr;
                if !world.vm[si].in_trigger && path_switch_context_by_ptr(world, &mut si, ptr) {
                    ip = caller_ip;
                    world.aliens[si].sword2 = ip as i16;
                }
                advance = 1;
            }

            P_BECOMEMOTHER => {
                let caller_ip = ip;
                world.become_obj = path_obj_index_or_null(si);
                world.become_path = world.aliens[si].sword2 as u16;
                let mother = path_get_mother_obj(world, si);
                if !world.vm[si].in_trigger {
                    if let Some(m) = mother {
                        if path_switch_context_to(world, &mut si, m) {
                            ip = caller_ip;
                            world.aliens[si].sword2 = ip as i16;
                        }
                    }
                }
                advance = 1;
            }

            P_UNBECOME => {
                let caller_ip = ip;
                world.become_path = world.aliens[si].sword2 as u16;
                let become_obj = world.become_obj;
                if !world.vm[si].in_trigger
                    && path_switch_context_by_ptr(world, &mut si, become_obj)
                {
                    ip = caller_ip;
                    world.aliens[si].sword2 = ip as i16;
                }
                advance = 1;
            }

            P_BECOME => {
                let caller_ip = ip;
                world.become_obj = path_obj_index_or_null(si);
                world.become_path = world.aliens[si].sword2 as u16;
                let last = world.become_last_obj;
                if !world.vm[si].in_trigger && path_switch_context_by_ptr(world, &mut si, last) {
                    ip = caller_ip;
                    world.aliens[si].sword2 = ip as i16;
                }
                advance = 1;
            }

            P_TEXT => {
                world.aliens[si].sflags |= ASF_TEXTOBJ;
                world.aliens[si].sflags2 |= ASF2_COLLDISABLE;
                world.aliens[si].coltab = world.pread16(ip, 1);
                world.aliens[si].depthoffset = world.pread8s(ip, 3) as i16;
                world.aliens[si].tx = world.pread8(ip, 4);
                advance = 5;
            }

            P_TRAIL => {
                world.aliens[si].ty = world.pread8(ip, 1);
                advance = 2;
            }

            P_SCORE => {
                world.playerscore = world.playerscore.wrapping_add(world.pread16(ip, 1));
                advance = 3;
            }

            P_SET0B => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_write8(&mut world.aliens[si], offset, 0);
                advance = 2;
            }

            P_SET0W => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_write16(&mut world.aliens[si], offset, 0);
                advance = 2;
            }

            P_INCB => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_add8(&mut world.aliens[si], offset, 1);
                advance = 2;
            }

            P_INCW => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_add16(&mut world.aliens[si], offset, 1);
                advance = 2;
            }

            P_DECB => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_add8(&mut world.aliens[si], offset, -1);
                advance = 2;
            }

            P_DECW => {
                let offset = world.pread8(ip, 1);
                alien_compat::path_add16(&mut world.aliens[si], offset, -1);
                advance = 2;
            }

            P_SETVBB | P_SETVBW | P_SETVWW | P_SETVWB => {
                let dst_off = world.pread8(ip, 1);
                let src_off = world.pread8(ip, 2);
                path_apply_setv(&mut world.aliens[si], opcode, dst_off, src_off);
                advance = 3;
            }

            P_ADDVBB | P_ADDVBW | P_ADDVWW | P_ADDVWB => {
                let dst_off = world.pread8(ip, 1);
                let src_off = world.pread8(ip, 2);
                path_apply_addv(&mut world.aliens[si], opcode, dst_off, src_off);
                advance = 3;
            }

            P_NEGB | P_NEGW => {
                let abs_off = world.pread16(ip, 1);
                if opcode == P_NEGW {
                    if let Some(cur_u) = path_abs_read16(&world.aliens[si], abs_off) {
                        let cur = (cur_u as i16).wrapping_neg();
                        path_abs_write16(&mut world.aliens[si], abs_off, cur as u16);
                    }
                } else if let Some(cur_u) = path_abs_read8(&world.aliens[si], abs_off) {
                    let cur = (cur_u as i8).wrapping_neg();
                    path_abs_write8(&mut world.aliens[si], abs_off, cur as u8);
                }
                advance = 3;
            }

            P_SETRANDOMB | P_SETRANDOMW => {
                let abs_off = world.pread16(ip, 1);
                if opcode == P_SETRANDOMW {
                    let mask = world.pread16(ip, 3);
                    // C: (uint16)((SfRtl_Random() << 8) | SfRtl_Random()).
                    // The evaluation order of `|` operands is pinned by the
                    // trace fixtures (gcc evaluates left-to-right here).
                    let hi = host.random();
                    let lo = host.random();
                    let rnd = (((hi as u32) << 8) | lo as u32) as u16;
                    path_abs_write16(&mut world.aliens[si], abs_off, rnd & mask);
                    advance = 5;
                } else {
                    let mask = world.pread8(ip, 3);
                    let rnd = host.random() as u8;
                    path_abs_write8(&mut world.aliens[si], abs_off, rnd & mask);
                    advance = 4;
                }
            }

            P_IFHITFLAG => {
                let target = world.pread16(ip, 1);
                let mask = world.pread8(ip, 3);
                let mut take = world.aliens[si].hitflags & mask != 0;
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    world.aliens[si].hitflags &= !mask;
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_ADDROTX => {
                world.aliens[si].rotx = world.aliens[si]
                    .rotx
                    .wrapping_add(world.pread8s(ip, 1) as u8);
                advance = 2;
            }

            P_ADDROTY => {
                world.aliens[si].roty = world.aliens[si]
                    .roty
                    .wrapping_add(world.pread8s(ip, 1) as u8);
                advance = 2;
            }

            P_ADDROTZ => {
                world.aliens[si].rotz = world.aliens[si]
                    .rotz
                    .wrapping_add(world.pread8s(ip, 1) as u8);
                advance = 2;
            }

            P_ADDWORLDX => {
                world.aliens[si].worldx =
                    (world.aliens[si].worldx as i32 + world.pread8s(ip, 1) as i32) as i16;
                advance = 2;
            }

            P_ADDWORLDY => {
                world.aliens[si].worldy =
                    (world.aliens[si].worldy as i32 + world.pread8s(ip, 1) as i32) as i16;
                advance = 2;
            }

            P_ADDWORLDZ => {
                world.aliens[si].worldz =
                    (world.aliens[si].worldz as i32 + world.pread8s(ip, 1) as i32) as i16;
                advance = 2;
            }

            P_ADDWS => {
                let offset = world.pread8(ip, 1);
                let value = world.pread8s(ip, 2);
                alien_compat::path_add16(&mut world.aliens[si], offset, value as i16);
                advance = 3;
            }

            // ============================================================
            // Chase byte toward target (opcode 11)
            // ============================================================
            P_ACHASEB => {
                let target = world.pread8(ip, 1);
                let offset = world.pread8(ip, 2);
                if let Some(cur) = alien_compat::path_read8(&world.aliens[si], offset) {
                    // ROM .achaseB: s_achase_var rate 3 (PATHS.ASM:680).
                    let cur = path_achase8(cur, target, 3);
                    alien_compat::path_write8(&mut world.aliens[si], offset, cur);
                }
                advance = 3;
            }

            // ============================================================
            // Distance check jump (opcode 30)
            // ============================================================
            P_DISTLESS => {
                let dist = world.pread16s(ip, 1);
                let target = world.pread16(ip, 3);
                let mut take = false;
                if let Some(pi) = path_get_player(world, host) {
                    take = path_axis_distance_is_less(
                        world.aliens[pi].worldz,
                        world.aliens[si].worldz,
                        dist,
                    );
                }
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch; // Dispatch next opcode
                }
                advance = 5;
            }

            P_SHAPEDISTLESS => {
                let dist = world.pread16s(ip, 1);
                let target = world.pread16(ip, 3);
                let mut take = false;
                if let Some(obj) = path_get_obj_by_ptr(world, world.aliens[si].ptr) {
                    take = path_axis_distance_is_less(
                        world.aliens[obj].worldz,
                        world.aliens[si].worldz,
                        dist,
                    );
                }
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 5;
            }

            // ============================================================
            // Goto (opcode 32): unconditional jump
            // ============================================================
            P_GOTO => {
                let target = world.pread16(ip, 1);
                ip = target;
                world.aliens[si].sword2 = ip as i16;
                break 'dispatch;
            }

            P_IGOTO => {
                let target = world.pread16(ip, 1);
                ip = target;
                world.aliens[si].sword2 = ip as i16;
                continue 'dispatch;
            }

            // ============================================================
            // Set acceleration (opcode 34)
            // ============================================================
            P_SETACCEL => {
                let target_vel = world.pread8(ip, 1);
                let accel_rate = world.pread8(ip, 2);
                world.aliens[si].count = target_vel; // Target velocity
                world.aliens[si].count1 = accel_rate; // Acceleration rate
                advance = 3;
            }

            P_HITGROUND => {
                let ground_h = world.pread16s(ip, 1);
                let target = world.pread16(ip, 3);
                let sum = (world.aliens[si].worldy as i32 + ground_h as i32) as i16;
                let mut take = sum >= 0;
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 5;
            }

            P_HITWALL => {
                let target = world.pread16(ip, 1);
                let out_x = world.aliens[si].worldx < world.minpmove_x
                    || world.aliens[si].worldx >= world.maxpmove_x;
                let out_y = world.aliens[si].worldy < world.minpmove_y
                    || world.aliens[si].worldy >= world.maxpmove_y;
                let mut take = out_x || out_y;
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_IFLEVEL => {
                let level_1based = world.pread8(ip, 1);
                let target = world.pread16(ip, 2);
                let level_0based = level_1based.wrapping_sub(1);
                let mut take = world.currentlevel == level_0based;
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_LEFTOFPLAYER | P_RIGHTOFPLAYER | P_ABOVEPLAYER | P_BELOWPLAYER | P_BEHINDPLAYER => {
                let target = world.pread16(ip, 1);
                let mut take = false;
                if let Some(pi) = path_get_player(world, host) {
                    let p = &world.aliens[pi];
                    let s = &world.aliens[si];
                    // These source predicates inspect the sign of the wrapped
                    // 16-bit difference. A direct signed comparison diverges
                    // when the course crosses the world-coordinate boundary.
                    take = match opcode {
                        P_LEFTOFPLAYER => p.worldx.wrapping_sub(s.worldx) >= 0,
                        P_RIGHTOFPLAYER => p.worldx.wrapping_sub(s.worldx) < 0,
                        P_ABOVEPLAYER => p.worldy.wrapping_sub(s.worldy) >= 0,
                        P_BELOWPLAYER => p.worldy.wrapping_sub(s.worldy) < 0,
                        P_BEHINDPLAYER => p.worldz.wrapping_sub(s.worldz) >= 0,
                        _ => false,
                    };
                }
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            // ============================================================
            // waitfaceplayer — pause until roughly facing player
            // ============================================================
            P_WAITFACEPLAYER => {
                if let Some(pi) = host.player(world).map(|p| p as usize) {
                    // ROM s_obj2obj_3dangle rate 3: nega(Yanglexy) + Xanglexy.
                    let target_yaw = host
                        .angle_xz(&world.aliens[si], &world.aliens[pi])
                        .wrapping_neg();
                    let target_pitch = path_pitch_toward(&world.aliens[si], &world.aliens[pi]);
                    let (roty, rotx) = (world.aliens[si].roty, world.aliens[si].rotx);
                    let done = roty == target_yaw && rotx == target_pitch;
                    world.aliens[si].roty = path_achase8(roty, target_yaw, 3);
                    world.aliens[si].rotx = path_achase8(rotx, target_pitch, 3);
                    if !done {
                        world.aliens[si].sword2 = ip as i16;
                        break 'dispatch;
                    }
                }
                advance = 1;
            }

            // ============================================================
            // 16-bit chase helpers
            // ============================================================
            P_ACHASEW => {
                let target = world.pread16s(ip, 1);
                let offset = world.pread8(ip, 3);
                if let Some(cur_u) = alien_compat::path_read16(&world.aliens[si], offset) {
                    // ROM .achaseW: s_achase_var rate 3 (PATHS.ASM:700).
                    let next = path_achase16(cur_u as i16, target, 3);
                    alien_compat::path_write16(&mut world.aliens[si], offset, next as u16);
                }
                advance = 4;
            }

            P_WAITACHASEB => {
                let target = world.pread8(ip, 1);
                let offset = world.pread8(ip, 2);
                if let Some(cur) = alien_compat::path_read8(&world.aliens[si], offset) {
                    // ROM .waitachaseB: rate 3 (PATHS.ASM:717); the finish
                    // branch fires only when cur == target AT ENTRY (one
                    // frame later than reached-after-step).
                    if cur != target {
                        let next = path_achase8(cur, target, 3);
                        alien_compat::path_write8(&mut world.aliens[si], offset, next);
                        world.aliens[si].sword2 = ip as i16;
                        break 'dispatch;
                    }
                }
                advance = 3;
            }

            P_WAITACHASEW => {
                let target = world.pread16s(ip, 1);
                let offset = world.pread8(ip, 3);
                if let Some(cur_u) = alien_compat::path_read16(&world.aliens[si], offset) {
                    // ROM .waitachaseW: rate 3 (PATHS.ASM:741); finishes only
                    // when already equal at entry.
                    if cur_u as i16 != target {
                        let next = path_achase16(cur_u as i16, target, 3);
                        alien_compat::path_write16(&mut world.aliens[si], offset, next as u16);
                        world.aliens[si].sword2 = ip as i16;
                        break 'dispatch;
                    }
                }
                advance = 4;
            }

            P_SPACESHIPON => {
                world.aliens[si].sflags2 |= PSFLAG4_SPACE;
                advance = 1;
            }

            P_SPACESHIPOFF => {
                world.aliens[si].sflags2 &= !PSFLAG4_SPACE;
                advance = 1;
            }

            P_HELION => {
                world.aliens[si].sflags2 |= PSFLAG3_HELI;
                advance = 1;
            }

            P_HELIOFF => {
                world.aliens[si].sflags2 &= !PSFLAG3_HELI;
                advance = 1;
            }

            P_INVISIBLEON => {
                world.aliens[si].sflags4 |= ASF4_INVISIBLE;
                world.aliens[si].sflags2 |= ASF2_COLLDISABLE;
                advance = 1;
            }

            P_INVISIBLEOFF => {
                world.aliens[si].sflags4 &= !ASF4_INVISIBLE;
                world.aliens[si].sflags2 &= !ASF2_COLLDISABLE;
                advance = 1;
            }

            P_SHADOWON => {
                world.aliens[si].sflags |= ASF_SHADOW;
                advance = 1;
            }

            P_SHADOWOFF => {
                world.aliens[si].sflags &= !ASF_SHADOW;
                advance = 1;
            }

            P_ALWAYS => {
                let addr = world.pread16(ip, 1);
                let trigger = world.pread8(ip, 3);
                path_register_trigger(world, si, addr, trigger);
                advance = 4;
            }

            P_ALWAYSOFF => {
                let addr = world.pread16(ip, 1);
                path_unregister_trigger(world, si, addr);
                advance = 3;
            }

            P_FORCE => {
                if world.vm[si].in_trigger {
                    world.vm[si].trigger_force_pending = true;
                    world.vm[si].trigger_forced_ip = world.pread16(ip, 1);
                }
                advance = 3;
            }

            P_SPRITE => {
                world.aliens[si].visual_kind = ObjectVisualKind::ScaledSprite;
                world.aliens[si].depthoffset = world.pread8s(ip, 1) as i16;
                world.aliens[si].tx = world.pread8(ip, 2);
                advance = 3;
            }

            P_SETSTRAT => {
                let strat_addr = world.pread16(ip, 1);
                let strat_bank = world.pread8(ip, 3);
                if let Some(f) = path_resolve_strategy(host, strat_addr, strat_bank) {
                    world.aliens[si].stratptr = Some(f);
                    world.aliens[si].sword2 = 0;
                    break 'dispatch;
                }
                advance = 4;
            }

            P_DEBUG => {
                advance = 1;
            }

            P_START65816 => {
                world.inline_return_override = None;
                let default_continuation =
                    if let Some((cb, continuation)) = world.find_inline_code(ip) {
                        host.run_inline(world, si as u16, cb);
                        if world.aldead != 0 {
                            return;
                        }
                        Some(continuation)
                    } else {
                        None
                    };
                if let Some(target) = world.inline_return_override.take().or(default_continuation) {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                println!("Path: unsupported start65816 blob at ip={ip}");
                world.aldead = 1;
                return;
            }

            P_PARTICLE => {
                if let Some(p) = host.obj_alloc(world).map(|p| p as usize) {
                    host.obj_move_after(world, p as u16, si as u16);
                    host.init_obj_vars(&mut world.aliens[p]);
                    let (wx, wy, wz) = {
                        let s = &world.aliens[si];
                        (s.worldx, s.worldy, s.worldz)
                    };
                    let pa = &mut world.aliens[p];
                    pa.shape = 0;
                    pa.stratptr = Some(StratRef::ParticleExplodeIstrat);
                    pa.collstratptr = None;
                    pa.expstratptr = None;
                    pa.worldx = wx;
                    pa.worldy = wy;
                    pa.worldz = wz;
                }
                advance = 1;
            }

            P_COLLISIONSON => {
                world.aliens[si].sflags2 &= !ASF2_COLLDISABLE;
                advance = 1;
            }

            P_COLLISIONSOFF => {
                world.aliens[si].sflags2 |= ASF2_COLLDISABLE;
                advance = 1;
            }

            P_SOUND => {
                let id = world.pread8(ip, 1);
                host.trig_se(id);
                advance = 2;
            }

            P_SOUND2 => {
                world.aliens[si].snd2 = world.pread8(ip, 1);
                advance = 2;
            }

            P_INVINCIBLEON => {
                world.aliens[si].sflags3 |= ASF3_NOHITAFFECT;
                advance = 1;
            }

            P_INVINCIBLEOFF => {
                world.aliens[si].sflags3 &= !ASF3_NOHITAFFECT;
                advance = 1;
            }

            P_PLAYERDEAD => {
                let target = world.pread16(ip, 1);
                let mut take = (world.pshipflags2 & PSF2_PLAYERHP0 != 0)
                    || (world.gameflags & GF_PLAYERDYING != 0);
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_ZREMOVEON => {
                world.aliens[si].type_ |= ATZREMOVE;
                advance = 1;
            }

            P_ZREMOVEOFF => {
                world.aliens[si].type_ &= !ATZREMOVE;
                advance = 1;
            }

            P_SMOKEON => {
                world.aliens[si].sflags3 |= PSFLAG6_SMOKE;
                advance = 1;
            }

            P_SMOKEOFF => {
                world.aliens[si].sflags3 &= !PSFLAG6_SMOKE;
                advance = 1;
            }

            P_WEAPON => {
                let weapon = world.pread8(ip, 1);
                world.aliens[si].weapontype = weapon;
                advance = 2;
            }

            P_NOTFRIEND => {
                let friend_id = world.pread8(ip, 1);
                let target = world.pread16(ip, 2);
                let mut take = if friend_id == FRIEND_ANYONE {
                    world.aliens[si].sbyte4 == 0
                } else {
                    world.aliens[si].sbyte4 != friend_id
                };
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_IFFLAG => {
                let target = world.pread16(ip, 1);
                let mut take = world.aliens[si].sflags4 & ASF4_SFLAG8 != 0;
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    world.aliens[si].sflags4 &= !ASF4_SFLAG8;
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_IFNOT => {
                invert_next_cond = true;
                advance = 1;
            }

            P_IFSAMEB => {
                let offset = world.pread8(ip, 1);
                let value = world.pread8(ip, 2);
                let target = world.pread16(ip, 3);
                let mut take = alien_compat::path_read8(&world.aliens[si], offset)
                    .map(|cur| cur == value)
                    .unwrap_or(false);
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 5;
            }

            P_IFSAMEW => {
                let offset = world.pread8(ip, 1);
                let value = alien_compat::path_comparison_word(offset, world.pread16(ip, 2));
                let target = world.pread16(ip, 4);
                let mut take = alien_compat::path_read16(&world.aliens[si], offset)
                    .map(|cur| cur == value)
                    .unwrap_or(false);
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 6;
            }

            P_IFBETWEENB => {
                let offset = world.pread8(ip, 1);
                let lo = world.pread8s(ip, 2);
                let hi = world.pread8s(ip, 3);
                let target = world.pread16(ip, 4);
                // ROM .ifbetweenb (PATHS.ASM:1606-1613): `bpl .add6` on
                // (val1 - var) makes the LOWER bound EXCLUSIVE; the upper
                // stays inclusive. Both compares are signed 8-bit wrapped
                // differences.
                let mut take = match alien_compat::path_read8(&world.aliens[si], offset) {
                    Some(cur_u) => {
                        ((lo as u8).wrapping_sub(cur_u) as i8) < 0
                            && ((hi as u8).wrapping_sub(cur_u) as i8) >= 0
                    }
                    None => false,
                };
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 6;
            }

            P_IFBETWEENW => {
                let offset = world.pread8(ip, 1);
                let lo = world.pread16s(ip, 2);
                let hi = world.pread16s(ip, 4);
                let target = world.pread16(ip, 6);
                // ROM .ifbetweenw: same as .ifbetweenb with 16-bit compares —
                // lower bound exclusive, upper inclusive.
                let mut take = match alien_compat::path_read16(&world.aliens[si], offset) {
                    Some(cur_u) => {
                        ((lo as u16).wrapping_sub(cur_u) as i16) < 0
                            && ((hi as u16).wrapping_sub(cur_u) as i16) >= 0
                    }
                    None => false,
                };
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 8;
            }

            P_IFZEROB | P_IFNOTZEROB => {
                let offset = world.pread8(ip, 1);
                let target = world.pread16(ip, 2);
                let mut take = alien_compat::path_read8(&world.aliens[si], offset)
                    .map(|cur| {
                        if opcode == P_IFZEROB {
                            cur == 0
                        } else {
                            cur != 0
                        }
                    })
                    .unwrap_or(false);
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_IFZEROW | P_IFNOTZEROW => {
                let offset = world.pread8(ip, 1);
                let target = world.pread16(ip, 2);
                let mut take = alien_compat::path_read16(&world.aliens[si], offset)
                    .map(|cur| {
                        if opcode == P_IFZEROW {
                            cur == 0
                        } else {
                            cur != 0
                        }
                    })
                    .unwrap_or(false);
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 4;
            }

            P_MSG => {
                let msg_id = world.pread8(ip, 1);
                host.send_message(msg_id);
                advance = 2;
            }

            P_MSG2 => {
                let msg_id = world.aliens[si].sbyte1;
                host.send_message(msg_id);
                advance = 2;
            }

            P_MSGWITHMETER => {
                let msg_id = world.pread8(ip, 1);
                host.path_set_friends_meter(world, 0xFF);
                host.send_message(msg_id);
                advance = 2;
            }

            P_ALMOSTDEAD => {
                let target = world.pread16(ip, 1);
                let mut take = false;
                if let Some(hp) = path_friend_hp(world, world.aliens[si].sbyte4) {
                    if hp <= 10 {
                        take = true;
                    }
                }
                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_DAMAGE => {
                let friend_id = world.aliens[si].sbyte4;
                if let Some(hp) = path_friend_hp(world, friend_id) {
                    let hp = if hp > 10 { hp - 10 } else { 0 };
                    path_friend_hp_set(world, friend_id, hp);
                }
                advance = 1;
            }

            P_RANDOMGOTO => {
                let target = world.pread16(ip, 1);
                if path_random_goto(host.random()) {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 3;
            }

            P_QSPAWN => {
                let shape = world.pread16(ip, 1);
                let path_start = world.pread16(ip, 3);
                let hp = world.pread8(ip, 5);
                let ap = world.pread8(ip, 6);

                if let Some(na) = host.obj_alloc(world).map(|a| a as usize) {
                    host.obj_move_after(world, na as u16, si as u16);
                    host.init_obj_vars(&mut world.aliens[na]);
                    world.reset_object_state(na as u16);
                    world.aliens[na].shape = if shape < 256 {
                        world.shapes_table[(shape as u8) as usize]
                    } else {
                        resolve_shape_word(shape)
                    };
                    strat_path_init(&mut world.aliens[na]);
                    let start = world.paths_resolve_embedded_start(path_start);
                    world.aliens[na].sword2 = start as i16;
                    let (wx, wy, wz, rx, ry, rz) = {
                        let s = &world.aliens[si];
                        (s.worldx, s.worldy, s.worldz, s.rotx, s.roty, s.rotz)
                    };
                    world.aliens[na].worldx = wx;
                    world.aliens[na].worldy = wy;
                    world.aliens[na].worldz = wz;
                    world.aliens[na].rotx = rx;
                    world.aliens[na].roty = ry;
                    world.aliens[na].rotz = rz;
                    world.aliens[na].hp = hp;
                    world.aliens[na].ap = ap;
                    world.become_last_obj = path_obj_index_or_null(na);
                }
                advance = 7;
            }

            P_IMPORTB | P_IMPORTW => {
                let offset = world.pread8(ip, 1);
                let addr = world.pread16(ip, 2);
                if opcode == P_IMPORTW {
                    let value = host.path_read_ext16(world, addr);
                    alien_compat::path_write16(&mut world.aliens[si], offset, value);
                } else {
                    let value = host.path_read_ext8(world, addr);
                    alien_compat::path_write8(&mut world.aliens[si], offset, value);
                }
                advance = 4;
            }

            P_EXPORTB | P_EXPORTW => {
                let offset = world.pread8(ip, 1);
                let addr = world.pread16(ip, 2);
                if opcode == P_EXPORTW {
                    if let Some(value) = alien_compat::path_read16(&world.aliens[si], offset) {
                        host.path_write_ext16(world, addr, value);
                    }
                } else if let Some(value) = alien_compat::path_read8(&world.aliens[si], offset) {
                    host.path_write_ext8(world, addr, value);
                }
                advance = 4;
            }

            P_PUSHB | P_PUSHW => {
                let offset = world.pread8(ip, 1);
                if opcode == P_PUSHW {
                    if let Some(value) = alien_compat::path_read16(&world.aliens[si], offset) {
                        world.vm_push(si, value);
                    }
                } else if let Some(value) = alien_compat::path_read8(&world.aliens[si], offset) {
                    world.vm_push(si, value as u16);
                }
                advance = 2;
            }

            P_PULLB | P_PULLW => {
                let offset = world.pread8(ip, 1);
                let value = world.vm_pop(si).unwrap_or(0);
                if opcode == P_PULLW {
                    alien_compat::path_write16(&mut world.aliens[si], offset, value);
                } else {
                    alien_compat::path_write8(&mut world.aliens[si], offset, (value & 0xFF) as u8);
                }
                advance = 2;
            }

            P_DIV2B | P_DIV2W => {
                let abs_offset = world.pread16(ip, 1);
                if opcode == P_DIV2W {
                    if let Some(value_u) = path_abs_read16(&world.aliens[si], abs_offset) {
                        let value = (value_u as i16) / 2;
                        path_abs_write16(&mut world.aliens[si], abs_offset, value as u16);
                    }
                } else if let Some(value_u) = path_abs_read8(&world.aliens[si], abs_offset) {
                    let value = (value_u as i8) / 2;
                    path_abs_write8(&mut world.aliens[si], abs_offset, value as u8);
                }
                advance = 3;
            }

            P_INDEXB | P_INDEXW => {
                let table_addr = world.pread16(ip, 1);
                let _bank = world.pread8(ip, 3); // bank byte ignored in flat-memory runtime
                let index_abs = world.pread16(ip, 4);
                let dest_abs = world.pread16(ip, 6);


                if world.gameframe >= 851 && world.gameframe <= 853 && si == 31 {
                    eprintln!(
                        "[idxb] gf={} si={} tbl={:04x} idx_abs={:04x} dst={:04x}",
                        world.gameframe, si, table_addr, index_abs, dest_abs
                    );
                }
                if let Some(index_src) = path_abs_read16(&world.aliens[si], index_abs) {
                    let idx = index_src & 0x00FF;
                    if world.gameframe >= 851 && world.gameframe <= 853 && si == 31 {
                        let v = host.path_read_ext8(world, table_addr.wrapping_add(idx));
                        eprintln!("[idxb]   raw={:04x} idx={} sintab={}", index_src, idx, v);
                    }
                    if opcode == P_INDEXW {
                        let value = host.path_read_ext16(world, table_addr.wrapping_add(idx << 1));
                        path_abs_write16(&mut world.aliens[si], dest_abs, value);
                    } else {
                        let value = host.path_read_ext8(world, table_addr.wrapping_add(idx));
                        path_abs_write8(&mut world.aliens[si], dest_abs, value);
                    }
                }
                advance = 8;
            }

            P_WITHINRANGE | P_SHAPEINRANGE => {
                let radius = world.pread16s(ip, 1);
                let target = world.pread16(ip, 3);
                let obj = if opcode == P_SHAPEINRANGE {
                    path_get_obj_by_ptr(world, world.aliens[si].ptr)
                } else {
                    host.player(world).map(|p| p as usize)
                };

                let mut take = false;
                if let Some(o) = obj {
                    let s = &world.aliens[si];
                    let ob = &world.aliens[o];
                    const WITHIN_RANGE_DEPTH: i16 = 200;
                    if path_axis_distance_is_less(s.worldz, ob.worldz, WITHIN_RANGE_DEPTH) {
                        take = path_xy_distance_is_within(s, ob, radius);
                    }
                }

                if invert_next_cond {
                    take = !take;
                    invert_next_cond = false;
                }
                if take {
                    ip = target;
                    world.aliens[si].sword2 = ip as i16;
                    continue 'dispatch;
                }
                advance = 5;
            }

            // ============================================================
            // Fire weapon (opcodes 69, 70)
            // ============================================================
            P_FIRE => {
                path_fire_weapon(world, host, si, false);
                advance = 1;
            }

            P_FIRECANHIT => {
                path_fire_weapon(world, host, si, true);
                advance = 1;
            }

            P_FIREATPLAYER | P_FIREATPLAYERCANHIT => {
                let can_hit_owner = opcode == P_FIREATPLAYERCANHIT;
                let target = path_get_player(world, host);
                path_fire_at_target(world, host, si, can_hit_owner, target);
                advance = 1;
            }

            P_FIREATSHAPE | P_FIREATSHAPECANHIT => {
                let can_hit_owner = opcode == P_FIREATSHAPECANHIT;
                let target = path_get_obj_by_ptr(world, world.aliens[si].ptr);
                path_fire_at_target(world, host, si, can_hit_owner, target);
                advance = 1;
            }

            // ============================================================
            // Gosub / Return
            // ============================================================
            P_GOSUBALVAR => {
                let offset = world.pread8(ip, 1);
                let target = alien_compat::path_read16(&world.aliens[si], offset).unwrap_or(0);

                // gosubalvar pushes (ip - 1) because return handler adds 3.
                world.vm_push(si, ip.wrapping_sub(1));
                ip = target;
                world.aliens[si].sword2 = ip as i16;
                continue 'dispatch;
            }

            P_GOSUB => {
                world.vm_push(si, ip);
                let target = world.pread16(ip, 1);
                ip = target;
                world.aliens[si].sword2 = ip as i16;
                continue 'dispatch;
            }

            P_RETURN => {
                let return_ip = world.vm_pop(si).unwrap_or(0xFFFF);
                if return_ip == 0xFFFF {
                    if world.vm[si].in_trigger {
                        world.vm[si].ifnot_pending = invert_next_cond;
                        return;
                    }
                    world.aldead = 1;
                    return;
                }
                ip = return_ip;
                world.aliens[si].sword2 = ip as i16;
                advance = 3;
            }

            _ => {
                if opcode >= P_NUM_OPCODES {
                    println!("Path: invalid opcode {opcode} at ip={ip}");
                    world.aldead = 1;
                    return;
                }
                println!("Path: unhandled opcode {opcode} at ip={ip}");
                world.aldead = 1;
                return;
            }
        }

        // Advance instruction pointer
        ip = ip.wrapping_add(advance as u16);
        world.aliens[si].sword2 = ip as i16;
    }

    // move_phase:
    world.vm[si].ifnot_pending = invert_next_cond;
    if world.vm[si].in_trigger {
        return;
    }

    // ============================================================
    // Move phase (PATHS.ASM .move)
    // Applied every frame after opcode dispatch.
    // ============================================================

    // 1. Acceleration: chase vel toward count (target) at count1 (rate).
    // ROM .move (PATHS.ASM:2816-2837): all compares are signed 8-bit
    // differences (bmi/bpl after cmp). On increase, landing AT OR ABOVE the
    // target clamps and zeroes count1 (exact hit included, .incit/.finish2);
    // on decrease an exact hit stores vel==count but keeps count1 — it is
    // zeroed one frame later when the equal-vel pass overshoots.
    if world.aliens[si].count1 != 0 {
        {
            let al = &mut world.aliens[si];
            if (al.vel.wrapping_sub(al.count) as i8) < 0 {
                // .incit
                let next = al.vel.wrapping_add(al.count1);
                if (next.wrapping_sub(al.count) as i8) < 0 {
                    al.vel = next; // .finish
                } else {
                    al.vel = al.count; // .finish2
                    al.count1 = 0;
                }
            } else {
                let next = al.vel.wrapping_sub(al.count1);
                if (next.wrapping_sub(al.count) as i8) >= 0 {
                    al.vel = next; // .finish
                } else {
                    al.vel = al.count; // .finish2
                    al.count1 = 0;
                }
            }
        }
        if world.aliens[si].sflags2 & PSFLAG2_GENVECS == 0 {
            path_genvecs(host, &mut world.aliens[si]);
        }
    }

    // 2. Space flight coupling (sflag4): roty += adiv2(adiv2(rotz)) — each
    // halving rounds TOWARD ZERO (PATHS.ASM:2845, STRATMAC.INC:712), so rotz
    // in -3..=3 contributes nothing. Rust `/` truncates toward zero, and two
    // toward-zero halvings equal one toward-zero /4.
    if world.aliens[si].sflags2 & PSFLAG4_SPACE != 0 {
        let al = &mut world.aliens[si];
        let rotz = al.rotz as i8;
        al.roty = al.roty.wrapping_add((rotz / 4) as u8);
    }

    // 3. Helicopter mode (sflag3): vel = rotx
    if world.aliens[si].sflags2 & PSFLAG3_HELI != 0 {
        world.aliens[si].vel = world.aliens[si].rotx;
        if world.aliens[si].sflags2 & PSFLAG2_GENVECS == 0 {
            path_genvecs(host, &mut world.aliens[si]);
        }
    }

    // 4. Z-relative to player (sflag1): add player view velocity
    if world.aliens[si].sflags2 & PSFLAG1_RELZ != 0 {
        world.aliens[si].worldz = world.aliens[si].worldz.wrapping_add(world.pviewvelz);
    }

    // 5. Always regenerate vectors (sflag2)
    if world.aliens[si].sflags2 & PSFLAG2_GENVECS != 0 {
        path_genvecs(host, &mut world.aliens[si]);
    }

    // 6. Apply velocity to position
    host.apply_velocity(&mut world.aliens[si]);

    // Per-frame child follow: retail re-anchors every linked child to its
    // mother's POST-movement position and reapplies the rotated offset at
    // ASL x3 each frame (carriedlog mutates CHILDY per frame via a sintab
    // weave; X drift emerges from the moving mother itself).
    if world.aliens[si].sflags3 & ASF3_MOTHEROBJ != 0 {
        path_prune_family_links(world, si);
        let mut idx = world.aliens[si].sword1 as u16;
        let mut guard = NUMBER_AL as i32 + 1;
        while idx != PATH_NULL_OBJ && guard > 0 {
            guard -= 1;
            let child = match path_obj_from_index_raw(idx) {
                Some(c) => c,
                None => break,
            };
            let next_idx = world.aliens[child].sword1 as u16;
            if world.aliens[child].active
                && world.aliens[child].sflags3 & ASF3_CHILDOBJ != 0
                && world.aliens[child].ptr == path_obj_index_or_null(si)
            {
                {
                    let (mwx, mwy, mwz, mrx, mry, mrz) = {
                        let m = &world.aliens[si];
                        (m.worldx, m.worldy, m.worldz, m.rotx, m.roty, m.rotz)
                    };
                    let c = &mut world.aliens[child];
                    c.worldx = mwx;
                    c.worldy = mwy;
                    c.worldz = mwz;
                    c.rotx = mrx.wrapping_add(c.childrotx);
                    c.roty = mry.wrapping_add(c.childroty);
                    c.rotz = mrz.wrapping_add(c.childrotz);
                }
                let (cx, cy, cz) = {
                    let c = &world.aliens[child];
                    (c.childx as i8, c.childy as i8, c.childz as i8)
                };
                path_add_rotated_offset(world, child, si, cx, cy, cz, 3);
            }
            idx = next_idx;
        }
    }

    // Text objects can emit short-lived trail sprites.
    path_spawn_text_trail(world, host, si);

    // Trigger routines run after movement and may execute immediate path code.
    path_run_triggers(world, host, si);
    if world.aldead != 0 {
        return;
    }

    // Rigid child translation DISABLED: retail children move under their own
    // inherited velocity (pillar weaves Y independently), not a mother-delta.
    if false && world.aliens[si].sflags3 & ASF3_MOTHEROBJ != 0 {
        let vm = &world.vm[si];
        if vm.delta_valid {
            let dx = world.aliens[si].worldx.wrapping_sub(vm.prev_x);
            let dy = world.aliens[si].worldy.wrapping_sub(vm.prev_y);
            let dz = world.aliens[si].worldz.wrapping_sub(vm.prev_z);
            if dx != 0 || dy != 0 || dz != 0 {
                path_prune_family_links(world, si);
                let mut idx = world.aliens[si].sword1 as u16;
                let mut guard = NUMBER_AL as i32 + 1;
                while idx != PATH_NULL_OBJ && guard > 0 {
                    guard -= 1;
                    let child = match path_obj_from_index_raw(idx) {
                        Some(c) => c,
                        None => break,
                    };
                    let next_idx = world.aliens[child].sword1 as u16;
                    if world.aliens[child].active
                        && world.aliens[child].sflags3 & ASF3_CHILDOBJ != 0
                    {
                        let c = &mut world.aliens[child];
                        c.worldx = c.worldx.wrapping_add(dx);
                        c.worldy = c.worldy.wrapping_add(dy);
                        c.worldz = c.worldz.wrapping_add(dz);
                    }
                    idx = next_idx;
                }
            }
        }
    }
    let vm = &mut world.vm[si];
    if !vm.delta_valid {
        vm.prev_x = tick_x0;
        vm.prev_y = tick_y0;
        vm.prev_z = tick_z0;
        vm.delta_valid = true;
    } else {
        vm.prev_x = world.aliens[si].worldx;
        vm.prev_y = world.aliens[si].worldy;
        vm.prev_z = world.aliens[si].worldz;
    }

    // Path collision trigger flags are one-frame latches.
    world.aliens[si].sflags3 &= !(ASF3_SFLAG5 | ASF3_SFLAG7);
}

#[cfg(test)]
mod vm_lifecycle_tests {
    use super::*;

    #[test]
    fn recycled_path_object_discards_stack_triggers_and_motion_history() {
        const OBJECT: u16 = 8;
        let mut world = PathWorld::new();
        let vm = &mut world.vm[usize::from(OBJECT)];
        vm.prev_x = 123;
        vm.delta_valid = true;
        vm.data[0] = 456;
        vm.sp = 1;
        vm.triggers[0] = PathTriggerEntry {
            addr: 789,
            trigger: PATH_TRIGGER_ALWAYS,
        };
        vm.trigger_count = 1;
        vm.ifnot_pending = true;

        world.reset_object_state(OBJECT);

        let vm = &world.vm[usize::from(OBJECT)];
        assert_eq!(vm.prev_x, 0);
        assert!(!vm.delta_valid);
        assert_eq!(vm.sp, 0);
        assert_eq!(vm.trigger_count, 0);
        assert!(!vm.ifnot_pending);
    }
}

#[cfg(test)]
mod distance_tests {
    use super::*;

    fn alien_at(x: i16, y: i16) -> Alien {
        Alien {
            worldx: x,
            worldy: y,
            ..Alien::default()
        }
    }

    #[test]
    fn within_radius_uses_strict_manhattan_distance() {
        let origin = alien_at(0, 0);
        assert!(!path_xy_distance_is_within(&origin, &alien_at(80, 60), 130));
        assert!(!path_xy_distance_is_within(&origin, &alien_at(80, 50), 130));
        assert!(path_xy_distance_is_within(&origin, &alien_at(79, 50), 130));
    }

    #[test]
    fn within_radius_preserves_wrapped_source_coordinates() {
        let first = alien_at(i16::MAX - 7, 0);
        let second = alien_at(i16::MIN + 8, 0);
        assert_eq!(path_xy_distance(&first, &second), 16);
        assert!(path_xy_distance_is_within(&first, &second, 17));
        assert!(!path_xy_distance_is_within(&first, &second, 16));
    }
}
