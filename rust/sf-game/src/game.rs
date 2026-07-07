//! Game context: tick entry, strategy dispatch loop, and the map-VM
//! bytecode interpreter.
//!
//! C oracle:
//! - `Nmi_GameTick` subset -> [`Game::tick`] (src/game/nmi.c:49)
//! - `Obj_RunStrategies`/`do_strat_l`/`init_strats_l` -> here
//!   (src/game/obj.c:224/172/134, from TRANS.ASM dostrats +
//!   STRATROU.ASM/GSTRATS.ASM)
//! - `World_UpdateObjects` -> [`Game::world_update_objects`]
//!   (src/game/world.c:894, WORLD.ASM update_objects_l)
//! - `map_exec` (newobjex dispatcher) -> [`Game::map_exec`]
//!   (src/game/world.c:995, WORLD.ASM:76-158 + all opcode handlers)
//! - world.c native/inline callback builtins -> [`Game::dispatch_native`],
//!   [`Game::run_inline`]

use crate::alien::{
    StratId, ACF_FIRSTFRAME, ASF3_REALOBJ, ASF4_CSPECIAL, ASF4_PLAYEROBJ, ASF4_SFLAG8,
    ASF4_SPECIAL, ASF_COLLIDE, ASF_LCOLLIDE, ATNUKED, ATZREMOVE, NUMBER_AL,
};
use crate::alien_compat as compat;
use crate::coldet::Coldet;
use crate::obj::{strat_init_obj_vars, Objects};
use crate::vars::*;
use crate::world::{op, resolve_shape_word, InlineCb, NativeCb, World};
use sf_map::consts::cb;
use sf_map::consts::wm;
use sf_map::levels::{BuiltLevel, NativeCallback};

/// Strategy function type — replaces C `StrategyFunc`
/// (`void (*)(Alien *self)`, src/game/obj.h). The alien is passed by slot
/// index; the full game context is available for globals access.
///
/// TODO(sf-strat): widen with whatever context the real strategies need;
/// phase 1 only hosts the world.c builtin spacebar strats.
pub type StrategyFn = fn(&mut Game, u16);

/// Outward-effect hooks for systems owned by other lanes (sound, windows,
/// strings, renderer shapes, path data). Defaults mirror the C oracle
/// harness stubs: no-ops, fades never active, unresolved paths -> 0,
/// unknown shape extents -> None (=> coldet default 20).
pub trait Hooks {
    /// C `Sound_PlayMusic` (map setbgm opcode).
    fn play_music(&mut self, _track_id: u8) {}
    /// C `Sound_PlaySE` (level inline callbacks).
    fn play_se(&mut self, _sound_id: u8) {}
    /// C `Strat_TrigSE` (blocksnd builtin).
    fn trig_se(&mut self, _sound_id: u8) {}
    /// C `Strings_SendMessage` (sendmsg opcode, CLfriendmsg builtins).
    fn send_message(&mut self, _msg_id: u8) {}
    /// C `Windows_FadeToBlack`.
    fn fade_to_black(&mut self, _speed: i32) {}
    /// C `Windows_FadeFromBlack`.
    fn fade_from_black(&mut self, _speed: i32) {}
    /// C `Windows_IsMapFadeActive` (waitfade opcode).
    fn is_map_fade_active(&self) -> bool {
        false
    }
    /// C `initblack_l` (src/game/windows.h).
    fn init_black(&mut self) {}
    /// C `initfadewhite2norm_l`.
    fn init_fade_white2norm(&mut self) {}
    /// C `Paths_ResolveStart` (src/path/paths.c:2930) — 0 = remove-stub.
    fn resolve_path_start(&mut self, _path_id: u16) -> u16 {
        0
    }
    /// C `load_collision_extents` shape half (src/game/coldet.c:43):
    /// final per-axis collision extents for a shape, or None when the
    /// shape mesh is unavailable (C falls back to 20/20/20).
    fn shape_extents(&self, _shape: u16) -> Option<(i16, i16, i16)> {
        None
    }
    /// C `Strat_PlayerExitBase` (src/strat/strat_player.c) — the hangar
    /// launch chain, owned by the strat lane.
    fn player_exit_base(&mut self) {}
}

/// No-op hook set (mirrors the C trace harness stubs).
pub struct NullHooks;
impl Hooks for NullHooks {}

/// The phase-1 game core: variables + object pool + map VM + collision.
pub struct Game {
    pub vars: GameVars,
    pub objs: Objects,
    pub world: World,
    pub coldet: Coldet,
    pub hooks: Box<dyn Hooks>,
    /// Path interpreter world (C `src/path/paths.c` translation-unit state:
    /// the path VM stacks/triggers + the globals paths.c touches). The C
    /// engine keeps ONE `g_aliens` pool shared by every strategy; the Rust
    /// port split the pool between `Objects` (game core) and `sf-path`'s
    /// `PathWorld`, so path-following strategies (IS_PATH/IS_PATHT/IS_PATHDHA)
    /// run through `sf_strat::path_adapter`, which mirrors `objs` into this
    /// world for the duration of each path strat call. `None` until the strat
    /// lane installs it in `Strat_RegisterAll` (`register_all`); objects stay
    /// inert while it is `None` (matching the pre-adapter behavior).
    pub path: Option<sf_path::interp::PathWorld>,
}

impl Game {
    /// Boot-order construction for the ported subset: `GameVars_Init` +
    /// `Obj_Init` + `World_Init` + `Coldet_Init`.
    pub fn new() -> Self {
        Self::with_hooks(Box::new(NullHooks))
    }

    pub fn with_hooks(hooks: Box<dyn Hooks>) -> Self {
        Game {
            vars: GameVars::init(),
            objs: Objects::init(),
            world: World::init(),
            coldet: Coldet::init(),
            hooks,
            path: None,
        }
    }

    /// C `MapExec_LoadLevel` (src/map/map_exec.c:14) + `World_LoadLevel`.
    pub fn load_level(&mut self, level: &BuiltLevel) {
        let GameVars { mapptr, mapcnt, .. } = &mut self.vars;
        self.world.load_level(level, mapptr, mapcnt);
    }

    /// C `Nmi_GameTick` (src/game/nmi.c:49) — ported subset: strategy
    /// tick (dostrats), controller latch, collision list + collision run.
    /// Camera/sound/HUD/draw/particle phases belong to other lanes.
    pub fn tick(&mut self) {
        // TRANS.ASM: lda freezestrats / bit #1 / bne stratsfrozen
        if (self.vars.freezestrats & 0x01) == 0 {
            self.run_strategies();
        }
        // Store last frame's controller state (TRANS.ASM:158-161).
        self.vars.lastcont0 = (self.vars.pad1 >> 8) as u8;
        self.vars.lastcontl0 = (self.vars.pad1 & 0xFF) as u8;
        // generate_collist_l + chkcoll.
        self.coldet_generate_list();
        self.coldet_run();
    }

    /// Dispatch a registered strategy on an alien slot.
    pub fn call_strat(&mut self, id: StratId, idx: u16) {
        let f = self.world.strat_registry[id.0 as usize];
        f(self, idx);
    }

    // ============================================================
    // dostrats loop (C Obj_RunStrategies, src/game/obj.c:224)
    // ============================================================
    pub fn run_strategies(&mut self) {
        // incw gameframe
        self.vars.gameframe = self.vars.gameframe.wrapping_add(1);
        // jsl init_strats_l
        self.init_strats();
        // jsl update_objects_l (via MapExec_Tick facade)
        self.world_update_objects();
        // walk active list; strategies may kill via aldead
        let mut cur = self.objs.active_head;
        while let Some(i) = cur {
            self.objs.aldead = 0; // stz aldead
            self.do_strat(i);
            // C reads _next AFTER the strategy ran (obj.c:263).
            let next = self.objs.aliens[i as usize].next;
            if self.objs.aldead != 0 {
                self.objs.free(i); // removedeadal_l
            }
            cur = next;
        }
    }

    /// C `init_strats_l` (src/game/obj.c:134, GSTRATS.ASM:423-434).
    fn init_strats(&mut self) {
        let playpt = self.vars.internal_playpt;
        let mut player: Option<usize> = None;
        if playpt >= 0 && (playpt as usize) < NUMBER_AL {
            if self.objs.aliens[playpt as usize].active {
                player = Some(playpt as usize);
            }
        }
        if player.is_none() && self.objs.aliens[0].active {
            player = Some(0); // Obj_GetPlayer fallback
        }
        let Some(p) = player else { return };
        let al = self.objs.aliens[p];
        self.vars.player_posx = al.worldx;
        self.vars.player_posy = al.worldy;
        self.vars.player_posz = al.worldz;
        self.vars.playervel_z = al.vz;
    }

    /// C `do_strat_l` (src/game/obj.c:172, STRATROU.ASM:2189+).
    fn do_strat(&mut self, idx: u16) {
        if !self.objs.aliens[idx as usize].active {
            return;
        }
        // cpx dummyobj / lbeq .estrat
        if self.vars.dummyobj > 0 && idx as i32 == self.vars.dummyobj as i32 {
            return;
        }
        // s_clr_alcollflag x,firstframe
        self.objs.aliens[idx as usize].collflags &= !ACF_FIRSTFRAME;

        let al = self.objs.aliens[idx as usize];
        // Dead object path (unless nuked).
        if al.hp == 0 && (al.type_ & ATNUKED) == 0 {
            if let Some(endcoll) = al.endcollstratptr {
                if al.sflags & ASF_LCOLLIDE != 0 {
                    self.objs.aliens[idx as usize].sflags &= !ASF_LCOLLIDE;
                    self.call_strat(endcoll, idx);
                    return;
                }
            }
            if let Some(exp) = al.expstratptr {
                self.call_strat(exp, idx);
                return;
            }
        }
        // s_clr_altype x,nuked
        self.objs.aliens[idx as usize].type_ &= !ATNUKED;

        let al = self.objs.aliens[idx as usize];
        if al.sflags & ASF_COLLIDE != 0 {
            if let Some(coll) = al.collstratptr {
                self.call_strat(coll, idx);
            } else {
                self.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
            }
            return;
        }
        if al.sflags & ASF_LCOLLIDE != 0 {
            self.objs.aliens[idx as usize].sflags &= !ASF_LCOLLIDE;
            if let Some(endcoll) = al.endcollstratptr {
                self.call_strat(endcoll, idx);
            }
            return;
        }
        if let Some(strat) = al.stratptr {
            self.call_strat(strat, idx);
        }
    }

    // ============================================================
    // update_objects_l (C World_UpdateObjects, src/game/world.c:894)
    // ============================================================
    pub fn world_update_objects(&mut self) {
        if !self.world.map_loaded || self.world.levelfinished != 0 {
            return;
        }
        if let Some(player) = self.objs.player() {
            let player_z = player.worldz;
            // lda al_worldz,x / sec / sbc lastplayz / sta lastzchange
            self.world.lastzchange = player_z.wrapping_sub(self.world.lastplayz);
            self.world.lastplayz = player_z;
        } else {
            // No player: fixed scroll MEDPSPEED=65 per frame.
            self.world.lastzchange = 65;
        }
        // lda mapcnt / sec / sbc lastzchange / bmi newobjs_l
        let cnt = (self.vars.mapcnt as i16 as i32) - (self.world.lastzchange as i32);
        if cnt < 0 {
            self.map_exec();
        } else {
            self.vars.mapcnt = cnt as i16 as u16;
        }
    }

    // ============================================================
    // Map bytecode helpers (C map_read8/map_read16, world.c:114)
    // ============================================================
    fn map_read8(&self, ptr: u16) -> u8 {
        if (ptr as usize) < self.world.map.len() {
            self.world.map[ptr as usize]
        } else {
            0
        }
    }

    fn map_read16(&self, ptr: u16) -> u16 {
        if (ptr as usize + 1) < self.world.map.len() {
            self.world.map[ptr as usize] as u16
                | ((self.world.map[ptr as usize + 1] as u16) << 8)
        } else {
            0
        }
    }

    fn map_read16s(&self, ptr: u16) -> i16 {
        self.map_read16(ptr) as i16
    }

    /// C `spawn_object` (src/game/world.c:933) — alloc + init vars + place
    /// relative to the player's Z.
    fn spawn_object(&mut self, x: i16, y: i16, z_offset: i16) -> Option<u16> {
        let Some(idx) = self.objs.alloc() else {
            self.world.last_obj = None;
            self.world.lastmapobj = 0;
            return None;
        };
        strat_init_obj_vars(&mut self.objs.aliens[idx as usize]);
        // Obj_GetPlayer() AFTER alloc, exactly like C: if the new object
        // took slot 0 it sees itself (worldz just zeroed).
        let player_z = if self.objs.aliens[0].active {
            Some(self.objs.aliens[0].worldz)
        } else {
            None
        };
        let al = &mut self.objs.aliens[idx as usize];
        al.worldx = x;
        al.worldy = y;
        al.worldz = match player_z {
            Some(pz) => pz.wrapping_add(z_offset),
            None => z_offset,
        };
        self.world.last_obj = Some(idx);
        self.world.lastmapobj = idx + 1; // 0 = invalid encoding
        Some(idx)
    }

    /// C `assign_strategy` (world.c:962) — NULL table entry keeps the
    /// object inert (world_warn_missing_strat is a log-only path).
    fn assign_strategy(&mut self, idx: u16, strat_idx: u8) {
        if let Some(id) = self.world.istrats[strat_idx as usize] {
            self.objs.aliens[idx as usize].stratptr = Some(id);
        }
    }

    /// C `assign_strategy_addr` (world.c:970).
    fn assign_strategy_addr(&mut self, idx: u16, strat_addr: u16, strat_bank: u8) {
        let key = ((strat_bank as u32) << 16) | strat_addr as u32;
        if let Some(id) = self.world.find_strategy_address(key) {
            self.objs.aliens[idx as usize].stratptr = Some(id);
        }
    }

    /// C `assign_shape` (world.c:983).
    fn assign_shape(&mut self, idx: u16, shape_idx: u8) {
        let word = self.world.shapes_table[shape_idx as usize];
        self.objs.aliens[idx as usize].shape = resolve_shape_word(word);
    }

    /// C `world_clear_map_objects` (src/game/world.c:413).
    fn clear_map_objects(&mut self, only_realobj: bool) {
        for idx in self.objs.active_indices() {
            let al = &self.objs.aliens[idx as usize];
            if al.sflags4 & ASF4_PLAYEROBJ != 0 {
                continue;
            }
            if !only_realobj || (al.sflags3 & ASF3_REALOBJ) != 0 {
                self.objs.free(idx);
            }
        }
    }

    // ============================================================
    // Native callbacks (C world.c mapif/mapcodejsl builtins + level regs)
    // ============================================================

    /// C `world_find_native_callback` (world.c:354): level registrations
    /// plus the world.c builtin table.
    fn find_native_callback(&self, addr24: u32) -> Option<NativeCb> {
        if let Some(&(_, id)) = self.world.native_cbs.iter().find(|e| e.0 == addr24) {
            return Some(NativeCb::Level(id));
        }
        const BUILTINS: &[u32] = &[
            cb::CHKSTAGEDONE,
            cb::CHKSTRATDONE1,
            cb::CHKSTRATDONE2,
            cb::CHKBOSSDEAD,
            cb::THEENDDEAD,
            cb::INITBLACK_L,
            cb::SETCHARMAPFROMMAP_L,
            cb::INITFADEWHITE2NORM_L,
            cb::KILL_ROBOT_L,
            cb::CLEARMAP_L,
            cb::CLEARREALOBJMAP_L,
            cb::SETRESTART_L,
            cb::MARKBOSS_L,
            cb::BLOCKSND_L,
            cb::FROG_ALIVE,
            cb::BUNNY_ALIVE,
            cb::COCK_ALIVE,
            cb::CLFRIENDMSG_FROG,
            cb::CLFRIENDMSG_BUNNY,
            cb::CLFRIENDMSG_COCK,
            cb::SET_PLAYER_EXITBASE_L,
            cb::SET_PLAYER_ONPLANET_L,
            cb::SET_PLAYER_CLEARDEMO_L,
            cb::SET_PLAYER_WARP_L,
            cb::SET_PLAYER_CLEAR_EARTH_L,
            cb::SET_PLAYER_CLEAR_CHASE_L,
            cb::SET_PLAYER_CLEAR_SHIP2_L,
            cb::SET_PLAYER_CLEAR_UNDER_L,
            cb::SET_PLAYER_DIVE_L,
            cb::SET_PLAYER_CLEAR_BRIDGE_L,
            cb::SET_PLAYER_CLEAR_TURN_L,
            cb::SET_PLAYER_WARPOUT_L,
            cb::IS_PLAYER_DEAD,
            cb::PLAYER_OUTVIEW_L,
            cb::LEVELFINISHED_ZERO,
        ];
        if BUILTINS.contains(&addr24) {
            return Some(NativeCb::Builtin(addr24));
        }
        None
    }

    /// C `world_friend_meter_from_hp` (world.c:543).
    fn friend_meter_from_hp(hp: u8) -> u8 {
        if hp == 0 {
            return 0;
        }
        if hp >= 40 {
            return 40;
        }
        if hp >= 30 {
            return 30;
        }
        if hp >= 20 {
            return 20;
        }
        if hp >= 10 {
            return 10;
        }
        // HD runtime friend HP is compact (1..3): bucket-map it.
        if hp >= 3 {
            return 40;
        }
        if hp == 2 {
            return 30;
        }
        20
    }

    /// Splayerflymode INSIDE -> TONORM transition shared by several
    /// set_player_* callbacks (world.c pattern).
    fn splayer_tonorm(&mut self) {
        if self.vars.splayerflymode == SPFM_INSIDE {
            self.vars.splayerflymode = SPFM_TONORM;
        }
    }

    /// Dispatch a native callback; returns the 65816 carry model
    /// (C `WorldNativeFunc` return, world.c:431-777 + levels.c:1841-1866).
    fn dispatch_native(&mut self, cbk: NativeCb) -> bool {
        match cbk {
            NativeCb::Level(NativeCallback::ClGroundPrintlevelfin) => {
                // C level1_1_cl_ground_printlevelfin (levels.c:1841).
                self.vars.write_ext8(wm::LEVELFINISHED, 3);
                true
            }
            NativeCb::Level(NativeCallback::ClGroundWipeout) => {
                // C level1_1_cl_ground_wipeout (levels.c:1850).
                self.vars.circleanim = 1;
                self.vars.oncewipe = 0;
                self.vars.pshipflags3 &= !PSF3_ENGINESND;
                true
            }
            NativeCb::Level(NativeCallback::ClDiveClearEnginesnd) => {
                // C cl_dive_clear_enginesnd (levels.c:1858).
                self.vars.pshipflags3 &= !PSF3_ENGINESND;
                true
            }
            NativeCb::Builtin(id) => self.dispatch_builtin(id),
        }
    }

    fn dispatch_builtin(&mut self, id: u32) -> bool {
        match id {
            // world_cb_chkstagedone (world.c:431).
            _ if id == cb::CHKSTAGEDONE => {
                if self.vars.gameflags & GF_STAGEDONE == 0 {
                    return false;
                }
                self.vars.gameflags &= !GF_STAGEDONE;
                true
            }
            // world_cb_chkstratdone1 (world.c:438).
            _ if id == cb::CHKSTRATDONE1 => {
                if self.vars.gameflags & GF_STRATDONE1 == 0 {
                    return false;
                }
                self.vars.gameflags &= !GF_STRATDONE1;
                true
            }
            // world_cb_chkstratdone2 (world.c:445).
            _ if id == cb::CHKSTRATDONE2 => self.vars.gameflags & GF_STRATDONE2 != 0,
            // world_cb_chkbossdead (world.c:450).
            _ if id == cb::CHKBOSSDEAD => {
                if self.vars.gameflags & GF_BOSSDEAD == 0 {
                    return false;
                }
                self.vars.gameflags &= !GF_BOSSDEAD;
                true
            }
            // world_cb_theenddead (world.c:457).
            _ if id == cb::THEENDDEAD => self.vars.numendok == 0xFF,
            // world_cb_initblack_l (world.c:462).
            _ if id == cb::INITBLACK_L => {
                self.hooks.init_black();
                true
            }
            // world_cb_setcharmapfrommap_l (world.c:468) — no flat-memory
            // equivalent; returns true.
            _ if id == cb::SETCHARMAPFROMMAP_L => true,
            // world_cb_initfadewhite2norm_l (world.c:475).
            _ if id == cb::INITFADEWHITE2NORM_L => {
                self.hooks.init_fade_white2norm();
                true
            }
            // world_cb_kill_robot_l (world.c:481): first ro_0 gets sflag8.
            _ if id == cb::KILL_ROBOT_L => {
                for idx in self.objs.active_indices() {
                    if self.objs.aliens[idx as usize].shape
                        == crate::world::WORLD_SHAPE_RO_0
                    {
                        self.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
                        break;
                    }
                }
                true
            }
            // world_cb_clearmap_l (world.c:495).
            _ if id == cb::CLEARMAP_L => {
                self.clear_map_objects(false);
                true
            }
            // world_cb_clearrealobjmap_l (world.c:501).
            _ if id == cb::CLEARREALOBJMAP_L => {
                self.clear_map_objects(true);
                true
            }
            // world_cb_setrestart_l (world.c:507).
            _ if id == cb::SETRESTART_L => {
                self.vars.bgflags |= BGF_RESTART;
                true
            }
            // world_cb_markboss_l (world.c:515) — no-op marker.
            _ if id == cb::MARKBOSS_L => true,
            // world_cb_blocksnd_l (world.c:521): trigse $49.
            _ if id == cb::BLOCKSND_L => {
                self.hooks.trig_se(0x49);
                true
            }
            // Friend-alive checks (world.c:528-541).
            _ if id == cb::FROG_ALIVE => self.vars.frog_hp != 0,
            _ if id == cb::BUNNY_ALIVE => self.vars.bunny_hp != 0,
            _ if id == cb::COCK_ALIVE => self.vars.falcon_hp != 0,
            // CLfriendmsg builtins (world.c:556-593).
            _ if id == cb::CLFRIENDMSG_FROG => {
                match Self::friend_meter_from_hp(self.vars.frog_hp) {
                    10 => self.hooks.send_message(87),
                    20 => self.hooks.send_message(84),
                    30 => self.hooks.send_message(81),
                    40 => self.hooks.send_message(77),
                    _ => {}
                }
                true
            }
            _ if id == cb::CLFRIENDMSG_BUNNY => {
                match Self::friend_meter_from_hp(self.vars.bunny_hp) {
                    10 => self.hooks.send_message(86),
                    20 => self.hooks.send_message(83),
                    30 => self.hooks.send_message(80),
                    40 => self.hooks.send_message(76),
                    _ => {}
                }
                true
            }
            _ if id == cb::CLFRIENDMSG_COCK => {
                match Self::friend_meter_from_hp(self.vars.falcon_hp) {
                    10 => self.hooks.send_message(85),
                    20 => self.hooks.send_message(82),
                    30 => self.hooks.send_message(79),
                    40 => self.hooks.send_message(75),
                    _ => {}
                }
                true
            }
            // world_cb_set_player_exitbase_l (world.c:595).
            _ if id == cb::SET_PLAYER_EXITBASE_L => {
                self.vars.game_mode = WATER_MODE;
                // s_set_var W,lastplayZ,#0 — re-zero map progression.
                self.world.lastplayz = 0;
                // jsl clearmap_l.
                self.clear_map_objects(false);
                // Strat_PlayerExitBase(Obj_GetPlayer()) — routed through the
                // strat registry (registered by sf_strat::table::register_all at
                // STRAT_ADDR_PLAYER_EXITBASE) so sf-game needn't depend on
                // sf-strat. Obj_GetPlayer() is slot 0 when active (obj.c:125).
                if self.objs.aliens[0].active {
                    if let Some(sid) = self
                        .world
                        .find_strategy_address(crate::world::STRAT_ADDR_PLAYER_EXITBASE)
                    {
                        self.call_strat(sid, 0);
                    }
                }
                self.hooks.player_exit_base();
                true
            }
            // world_cb_set_player_onplanet_l (world.c:610).
            _ if id == cb::SET_PLAYER_ONPLANET_L => {
                self.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
                self.vars.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
                // Corneria on-planet is normal ground flight (mode 0), NOT
                // water — the WATER_MODE here was a copy/paste bug.
                self.vars.game_mode = 0;
                self.vars.playerflymode |= PFM_SHADOWS;
                true
            }
            // world_cb_set_player_cleardemo_l (world.c:620).
            _ if id == cb::SET_PLAYER_CLEARDEMO_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_INSEQ | PSTF_NOTDIE;
                self.vars.playerflymode &= !PFM_WOBBLE;
                true
            }
            // world_cb_set_player_warp_l (world.c:629).
            _ if id == cb::SET_PLAYER_WARP_L => {
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
                self.vars.psvar_word2 = 0;
                self.vars.gameflags |= GF_NOZREMOVE;
                self.vars.gameflags &= !GF_VIEWROT;
                self.vars.psvar_word1 = 200;
                self.vars.playerflymode |= PFM_WOBBLE;
                self.vars.pshipflags3 |= PSF3_NOCOLLISIONS;
                true
            }
            // world_cb_set_player_clear_earth_l (world.c:644).
            _ if id == cb::SET_PLAYER_CLEAR_EARTH_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_INSEQ | PSTF_NOTDIE;
                self.vars.playerflymode &= !PFM_WOBBLE;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                self.vars.viewdist = OUTVIEWDIST;
                true
            }
            // world_cb_set_player_clear_chase_l (world.c:660).
            _ if id == cb::SET_PLAYER_CLEAR_CHASE_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ | PSTF_NOTDIE;
                self.vars.playerflymode &= !PFM_WOBBLE;
                self.vars.gameflags |= GF_NOZREMOVE;
                self.vars.gameflags &= !GF_VIEWROT;
                self.vars.psvar_word1 = 300;
                self.vars.psvar_word2 = 0;
                self.vars.psvar_word3 = 218;
                self.vars.psvar_word4 = 5;
                self.vars.minpmove_y = -10000;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                self.vars.viewdist = OUTVIEWDIST;
                true
            }
            // world_cb_set_player_clear_ship2_l (world.c:683).
            _ if id == cb::SET_PLAYER_CLEAR_SHIP2_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_INSEQ;
                self.vars.gameflags |= GF_NOZREMOVE;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                true
            }
            // world_cb_set_player_clear_under_l (world.c:696).
            _ if id == cb::SET_PLAYER_CLEAR_UNDER_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
                self.vars.gameflags &= !GF_VIEWROT;
                self.vars.minpmove_y = -10000;
                self.vars.game_mode = WATER_MODE;
                true
            }
            // world_cb_set_player_dive_l (world.c:707).
            _ if id == cb::SET_PLAYER_DIVE_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC;
                self.vars.playerflymode |= PFM_WOBBLE;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                true
            }
            // world_cb_set_player_clear_bridge_l (world.c:720).
            _ if id == cb::SET_PLAYER_CLEAR_BRIDGE_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
                self.vars.minpmove_y = -10000;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                true
            }
            // world_cb_set_player_clear_turn_l (world.c:733).
            _ if id == cb::SET_PLAYER_CLEAR_TURN_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
                self.vars.gameflags &= !GF_VIEWROT;
                self.vars.playerflymode |= PFM_WOBBLE;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                true
            }
            // world_cb_set_player_warpout_l (world.c:747).
            _ if id == cb::SET_PLAYER_WARPOUT_L => {
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                self.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                true
            }
            // world_cb_is_player_dead (world.c:759).
            _ if id == cb::IS_PLAYER_DEAD => {
                self.vars.pshipflags2 & PSF2_PLAYERHP0 != 0
            }
            // world_cb_set_playeroutview_l (world.c:764).
            _ if id == cb::PLAYER_OUTVIEW_L => {
                self.vars.pstratflags |= PSTF_INSEQ;
                self.vars.game_mode = SPACE_MODE;
                self.splayer_tonorm();
                self.vars.viewdist = OUTVIEWDIST;
                true
            }
            // world_cb_is_levelfinished_zero (world.c:775).
            _ if id == cb::LEVELFINISHED_ZERO => self.world.levelfinished == 0,
            _ => true,
        }
    }

    /// Run an inline CODE65816 callback (C `WorldInlineMapFunc` impls in
    /// src/map/levels.c). `mapptr` points AT the CODE65816 opcode; the
    /// callback advances it (usually by 1) or jumps to a skip target.
    fn run_inline(&mut self, cbk: InlineCb, mapptr: &mut u16) {
        match cbk {
            InlineCb::LevelScrambleKeepPlayerStrat => {
                // levels.c:1690.
                self.vars.pshipflags3 |= PSF3_KEEPPSTRAT;
                self.vars.meters = 1;
                *mapptr = mapptr.wrapping_add(1);
            }
            InlineCb::SkillflyGuard { skip_ptr } => {
                // levels.c:1671.
                if self.vars.read_ext8(wm::SKILLFLY) != 0 {
                    *mapptr = skip_ptr;
                } else {
                    *mapptr = mapptr.wrapping_add(1);
                }
            }
            InlineCb::MapwaitbossTrigse => {
                // levels.c:1769.
                self.hooks.play_se(0x0B);
                *mapptr = mapptr.wrapping_add(1);
            }
            InlineCb::MapwaitbossCantdie => {
                // levels.c:1775.
                self.vars.pstratflags |= PSTF_NOTDIE;
                *mapptr = mapptr.wrapping_add(1);
            }
            InlineCb::MapwaitbossCleanup => {
                // levels.c:1781.
                self.vars.pshipflags |= PSF_NOFIRE;
                self.vars.bossmaxhp = 0;
                self.vars.meters = 0;
                *mapptr = mapptr.wrapping_add(1);
            }
            InlineCb::TitleInit => {
                // levels.c:2245.
                self.vars.playerflymode &= !PFM_WOBBLE;
                self.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
                *mapptr = mapptr.wrapping_add(1);
            }
            InlineCb::ContmapInit => {
                // levels.c:2253.
                self.vars.playerflymode &= !PFM_WOBBLE;
                *mapptr = mapptr.wrapping_add(1);
            }
        }
    }

    /// Write to the last spawned map object through the SNES byte-offset
    /// compat layer (C `AlienCompat_Write8` on `s_lastmapobj`).
    fn last_obj_write8(&mut self, offset: u16, is_alx: bool, value: u8) {
        if let Some(idx) = self.world.last_obj {
            compat::write8(&mut self.objs.aliens[idx as usize], offset, is_alx, value);
        }
    }

    fn last_obj_write16(&mut self, offset: u16, is_alx: bool, value: u16) {
        if let Some(idx) = self.world.last_obj {
            compat::write16(&mut self.objs.aliens[idx as usize], offset, is_alx, value);
        }
    }

    // ============================================================
    // map_exec — C `map_exec` / newobjex bytecode dispatcher
    // (src/game/world.c:995, WORLD.ASM:76-158)
    // ============================================================
    pub fn map_exec(&mut self) {
        if !self.world.map_loaded || self.vars.mapptr as usize >= self.world.map.len() {
            return;
        }
        // Safety valve against runaway scripts (C max_ops = 500).
        let mut max_ops = 500;
        while max_ops > 0 {
            max_ops -= 1;
            if self.vars.mapptr as usize >= self.world.map.len() {
                return;
            }
            let p = self.vars.mapptr;
            let opcode = self.map_read8(p);
            match opcode {
                // 0: mapobj — frame(2),x(2),y(2),z(2),shape(1),strat(1).
                op::MAPOBJ => {
                    let frame = self.map_read16(p.wrapping_add(1));
                    let x = self.map_read16s(p.wrapping_add(3));
                    let y = self.map_read16s(p.wrapping_add(5));
                    let z = self.map_read16s(p.wrapping_add(7));
                    let shape = self.map_read8(p.wrapping_add(9));
                    let strat = self.map_read8(p.wrapping_add(10));
                    self.vars.mapcnt = frame;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.assign_shape(idx, shape);
                        self.assign_strategy(idx, strat);
                    }
                    self.vars.mapptr = p.wrapping_add(11);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 2: mapend.
                op::END => {
                    self.world.levelfinished = 1;
                    return;
                }
                // 4: maploop — addr(2),count(2).
                op::LOOP => {
                    let loop_addr = p;
                    let target = self.map_read16(p.wrapping_add(1));
                    let count = self.map_read16(p.wrapping_add(3));
                    let found = (0..self.world.num_loops)
                        .find(|&i| self.world.loop_addrs[i] == loop_addr);
                    match found {
                        None => {
                            if self.world.num_loops < crate::world::MAP_MAX_LOOPS {
                                let i = self.world.num_loops;
                                self.world.num_loops += 1;
                                self.world.loop_addrs[i] = loop_addr;
                                self.world.loop_counts[i] = count;
                            }
                            self.vars.mapptr = target;
                        }
                        Some(i) => {
                            self.world.loop_counts[i] =
                                self.world.loop_counts[i].wrapping_sub(1);
                            if self.world.loop_counts[i] == 0 {
                                // Compact the loop array (C world.c:1084).
                                for j in i..self.world.num_loops - 1 {
                                    self.world.loop_addrs[j] = self.world.loop_addrs[j + 1];
                                    self.world.loop_counts[j] =
                                        self.world.loop_counts[j + 1];
                                }
                                self.world.num_loops -= 1;
                                self.vars.mapptr = p.wrapping_add(5);
                            } else {
                                self.vars.mapptr = target;
                            }
                        }
                    }
                }
                // 6/8: debug / nop.
                op::DEBUG | op::NOP => {
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 10: mapmother — frame(2),x(2),y(2),z(2),shape(2),
                // strat(3),map(2).
                op::MOTHER => {
                    let frame = self.map_read16(p.wrapping_add(1));
                    let x = self.map_read16s(p.wrapping_add(3));
                    let y = self.map_read16s(p.wrapping_add(5));
                    let z = self.map_read16s(p.wrapping_add(7));
                    let shape = self.map_read16(p.wrapping_add(9));
                    let strat_addr = self.map_read16(p.wrapping_add(11));
                    let strat_bank = self.map_read8(p.wrapping_add(13));
                    let map_ref = self.map_read16(p.wrapping_add(14));
                    self.vars.mapcnt = frame;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.objs.aliens[idx as usize].shape = resolve_shape_word(shape);
                        self.assign_strategy_addr(idx, strat_addr, strat_bank);
                        let al = &mut self.objs.aliens[idx as usize];
                        al.ptr = map_ref; // sub-map pointer
                        al.type_ = ATZREMOVE; // remove when behind camera
                    }
                    self.vars.mapptr = p.wrapping_add(16);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 12: mapremove — frame(2),shape(2).
                op::REMOVE => {
                    // ROM mapremove (WORLD.ASM:1973-1993): scans allst skipping
                    // the player and removes exactly ONE match (falls through to
                    // .out after the first hit — oracle audit_mapvm2 head->A->B
                    // removes only A). removedeadal also clears dangling refs to
                    // the freed slot (l_rem, MACROS.INC:3684). Old port freed
                    // EVERY match including the player.
                    let target_shape = resolve_shape_word(self.map_read16(p.wrapping_add(3)));
                    for idx in self.objs.active_indices() {
                        if idx == 0 {
                            continue; // player never checked
                        }
                        if self.objs.aliens[idx as usize].shape == target_shape {
                            self.objs.free(idx);
                            for al in self.objs.aliens.iter_mut() {
                                if al.immuneptr == idx {
                                    al.immuneptr = 0;
                                }
                                if al.collobjptr == idx {
                                    al.collobjptr = 0;
                                }
                            }
                            break; // one removal per opcode execution
                        }
                    }
                    self.vars.mapptr = p.wrapping_add(5);
                }
                // 14: setstage.
                op::SETSTAGE => {
                    self.vars.stagecnt = 50;
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 16: setbg — bg_index(2).
                op::SETBG => {
                    self.vars.currentbg = self.map_read16(p.wrapping_add(1));
                    self.vars.bgflags |= BGF_BG;
                    self.vars.mapptr = p.wrapping_add(3);
                }
                // 18: mapwait — distance(2) (WORLD.ASM:974-982).
                op::WAIT => {
                    let dist = self.map_read16(p.wrapping_add(1));
                    if dist == 0 {
                        self.vars.mapptr = p.wrapping_add(3);
                        continue;
                    }
                    self.vars.mapcnt = dist;
                    self.vars.mapptr = p.wrapping_add(3);
                    return;
                }
                // 20: setbgm — music(1). ROM (WORLD.ASM:194-206) skips the
                // change while the player is at HP 0 (psf2_playerHP0) so death
                // music isn't stomped — oracle audit_mapvm2.
                op::SETBGM => {
                    let music = self.map_read8(p.wrapping_add(1));
                    if self.vars.pshipflags2 & crate::vars::PSF2_PLAYERHP0 == 0 {
                        self.hooks.play_music(music);
                    }
                    self.vars.mapptr = p.wrapping_add(2);
                }
                // 22/24/26: dot modes.
                op::NODOTS => {
                    self.vars.dotsflag = 0;
                    self.vars.mapptr = p.wrapping_add(1);
                }
                op::GNDDOTS => {
                    self.vars.dotsflag = 1;
                    self.vars.mapptr = p.wrapping_add(1);
                }
                op::SPACEDUST => {
                    self.vars.dotsflag = -1;
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 28: setothmus — music(1).
                op::SETOTHMUS => {
                    self.vars.othmusic = self.map_read8(p.wrapping_add(1));
                    self.vars.mapptr = p.wrapping_add(2);
                }
                // 30-36: offset controls — single-byte no-ops in HD.
                op::VOFSON | op::VOFSOFF | op::HOFSON | op::HOFSOFF => {
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 38: mapobjzrot — mapobj + zrot(1).
                op::OBJZROT => {
                    let frame = self.map_read16(p.wrapping_add(1));
                    let x = self.map_read16s(p.wrapping_add(3));
                    let y = self.map_read16s(p.wrapping_add(5));
                    let z = self.map_read16s(p.wrapping_add(7));
                    let shape = self.map_read8(p.wrapping_add(9));
                    let strat = self.map_read8(p.wrapping_add(10));
                    let zrot = self.map_read8(p.wrapping_add(11));
                    self.vars.mapcnt = frame;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.assign_shape(idx, shape);
                        self.assign_strategy(idx, strat);
                        self.objs.aliens[idx as usize].rotz = zrot;
                    }
                    self.vars.mapptr = p.wrapping_add(12);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 40: mapjsr — addr(2),bank(1) (WORLD.ASM:1104-1124).
                op::JSR => {
                    let target = self.map_read16(p.wrapping_add(1));
                    if self.world.jsr_top < crate::world::MAP_JSR_STACK_SIZE {
                        self.world.jsr_stack[self.world.jsr_top] = p;
                        self.world.jsr_top += 1;
                        self.world.num_jsr += 1;
                    }
                    self.vars.mapptr = target;
                }
                // 42: maprts (WORLD.ASM:1128-1145).
                op::RTS => {
                    if self.world.jsr_top > 0 {
                        self.world.jsr_top -= 1;
                        self.world.num_jsr -= 1;
                        let ret = self.world.jsr_stack[self.world.jsr_top];
                        self.vars.mapptr = ret.wrapping_add(4); // skip JSR (1+3)
                    } else {
                        self.vars.mapptr = p.wrapping_add(1);
                    }
                }
                // 44: mapif — func(3),else_addr(2).
                op::IF => {
                    let func_addr = self.map_read16(p.wrapping_add(1));
                    let func_bank = self.map_read8(p.wrapping_add(3));
                    let else_addr = self.map_read16(p.wrapping_add(4));
                    let key = ((func_bank as u32) << 16) | func_addr as u32;
                    let carry = match self.find_native_callback(key) {
                        Some(cbk) => self.dispatch_native(cbk),
                        None => true,
                    };
                    if !carry {
                        self.vars.mapptr = p.wrapping_add(6);
                        self.vars.mapcnt = 1;
                        return;
                    }
                    self.vars.mapptr = else_addr;
                }
                // 46: mapgoto — addr(2),bank(1) (WORLD.ASM:1025-1033).
                op::GOTO => {
                    self.vars.mapptr = self.map_read16(p.wrapping_add(1));
                }
                // 48-52: set rotation on last spawned object — rot(1).
                op::SETXROT => {
                    if let Some(idx) = self.world.last_obj {
                        self.objs.aliens[idx as usize].rotx = self.map_read8(p.wrapping_add(1));
                    }
                    self.vars.mapptr = p.wrapping_add(2);
                }
                op::SETYROT => {
                    if let Some(idx) = self.world.last_obj {
                        self.objs.aliens[idx as usize].roty = self.map_read8(p.wrapping_add(1));
                    }
                    self.vars.mapptr = p.wrapping_add(2);
                }
                op::SETZROT => {
                    if let Some(idx) = self.world.last_obj {
                        self.objs.aliens[idx as usize].rotz = self.map_read8(p.wrapping_add(1));
                    }
                    self.vars.mapptr = p.wrapping_add(2);
                }
                // 54: setalvarb — offset(2),value(1) (WORLD.ASM:848-862).
                op::SETALVARB => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let value = self.map_read8(p.wrapping_add(3));
                    self.last_obj_write8(offset, false, value);
                    self.vars.mapptr = p.wrapping_add(4);
                }
                // 56: setalvarw — offset(2),value(2).
                op::SETALVARW => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let value = self.map_read16(p.wrapping_add(3));
                    self.last_obj_write16(offset, false, value);
                    self.vars.mapptr = p.wrapping_add(5);
                }
                // 58: setalvarl — offset(2),lo(2),hi(1).
                op::SETALVARL => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let lo = self.map_read16(p.wrapping_add(3));
                    let hi = self.map_read8(p.wrapping_add(5));
                    self.last_obj_write16(offset, false, lo);
                    self.last_obj_write8(offset.wrapping_add(2), false, hi);
                    self.vars.mapptr = p.wrapping_add(6);
                }
                // 60-64: setalxvar — alx offsets, same layout.
                op::SETALXVARB => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let value = self.map_read8(p.wrapping_add(3));
                    self.last_obj_write8(offset, true, value);
                    self.vars.mapptr = p.wrapping_add(4);
                }
                op::SETALXVARW => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let value = self.map_read16(p.wrapping_add(3));
                    self.last_obj_write16(offset, true, value);
                    self.vars.mapptr = p.wrapping_add(5);
                }
                op::SETALXVARL => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let lo = self.map_read16(p.wrapping_add(3));
                    let hi = self.map_read8(p.wrapping_add(5));
                    self.last_obj_write16(offset, true, lo);
                    self.last_obj_write8(offset.wrapping_add(2), true, hi);
                    self.vars.mapptr = p.wrapping_add(6);
                }
                // 66/68: fade up / fade down.
                op::FADEUP => {
                    self.hooks.fade_from_black(1);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                op::FADEDOWN => {
                    self.hooks.fade_to_black(1);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 70/72: setalvarptrb/w — offset(2),ptr(2),bank(1).
                op::SETALVARPB | op::SETALVARPW => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let extptr = self.map_read16(p.wrapping_add(3));
                    if opcode == op::SETALVARPB {
                        let v = self.vars.read_ext8(extptr);
                        self.last_obj_write8(offset, false, v);
                    } else {
                        let v = self.vars.read_ext16(extptr);
                        self.last_obj_write16(offset, false, v);
                    }
                    self.vars.mapptr = p.wrapping_add(6);
                }
                // 74: setvarobj — ptr(2),bank(1). ROM (WORLD.ASM:744-745)
                // `ifobjinvalid` bails BEFORE the store: after a failed spawn
                // (lastmapobj==0) the variable keeps its old value — oracle
                // audit_mapvm2 (sentinel survives on the ROM).
                op::SETVAROBJ => {
                    let extptr = self.map_read16(p.wrapping_add(1));
                    let v = self.world.lastmapobj;
                    if v != 0 {
                        self.vars.write_ext16(extptr, v);
                    }
                    self.vars.mapptr = p.wrapping_add(4);
                }
                // 76: mapwaitfade (WORLD.ASM:726-740).
                op::WAITFADE => {
                    if self.hooks.is_map_fade_active() {
                        self.vars.mapcnt = 1;
                        return;
                    }
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 78/80: quick fades.
                op::QFADEUP => {
                    self.hooks.fade_from_black(2);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                op::QFADEDOWN => {
                    self.hooks.fade_to_black(2);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 86/88: camera z-rotation toggle (WORLD.ASM setzroton/off ->
                // dozrot $1776); gates the view roll in getview_l.
                op::ZROTON => {
                    self.vars.write_ext8(0x1776, 1);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                op::ZROTOFF => {
                    self.vars.write_ext8(0x1776, 0);
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 82/84: screen on/off — no-ops.
                op::SCREENOFF | op::SCREENON => {
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 90: mapspecial (WORLD.ASM:654-663).
                op::SPECIAL => {
                    if let Some(idx) = self.world.last_obj {
                        self.objs.aliens[idx as usize].sflags4 |= ASF4_SPECIAL;
                        self.world.specialobjtotal = self.world.specialobjtotal.wrapping_add(1);
                    }
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 92: setvarb — value(1),ptr(2),bank(1).
                op::SETVARB => {
                    let value = self.map_read8(p.wrapping_add(1));
                    let extptr = self.map_read16(p.wrapping_add(2));
                    self.vars.write_ext8(extptr, value);
                    self.vars.mapptr = p.wrapping_add(5);
                }
                // 94: setvarw — value(2),ptr(2),bank(1).
                op::SETVARW => {
                    let value = self.map_read16(p.wrapping_add(1));
                    let extptr = self.map_read16(p.wrapping_add(3));
                    self.vars.write_ext16(extptr, value);
                    self.vars.mapptr = p.wrapping_add(6);
                }
                // 96: setvarl — ptr(2)... literal C operand order
                // (world.c:1608: extptr @+1, lo @+4, hi @+6).
                op::SETVARL => {
                    let extptr = self.map_read16(p.wrapping_add(1));
                    let lo = self.map_read16(p.wrapping_add(4));
                    let hi = self.map_read8(p.wrapping_add(6));
                    self.vars.write_ext16(extptr, lo);
                    self.vars.write_ext8(extptr.wrapping_add(2), hi);
                    self.vars.mapptr = p.wrapping_add(7);
                }
                // 98: setbgslow — speed(1),bg(2).
                op::SETBGSLOW => {
                    self.vars.bgtransspeed = self.map_read8(p.wrapping_add(1)) as u16;
                    self.vars.bg_dmalist = self.map_read16(p.wrapping_add(2));
                    self.vars.currentbg = self.vars.bg_dmalist;
                    self.vars.mapptr = p.wrapping_add(4);
                }
                // 100: waitsetbg.
                op::WAITSETBG => {
                    if self.vars.bg_dmalist != 0 {
                        return;
                    }
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 102: setbginfo.
                op::SETBGINFO => {
                    self.vars.bgflags |= BGF_INFO;
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 104/106: addalvarptrb/w — offset(2),ptr(2),bank(1).
                op::ADDALVARPB | op::ADDALVARPW => {
                    let offset = self.map_read16(p.wrapping_add(1));
                    let extptr = self.map_read16(p.wrapping_add(3));
                    if let Some(idx) = self.world.last_obj {
                        if opcode == op::ADDALVARPB {
                            let d = self.vars.read_ext8(extptr) as i8;
                            compat::add8(&mut self.objs.aliens[idx as usize], offset, false, d);
                        } else {
                            let d = self.vars.read_ext16(extptr) as i16;
                            compat::add16(&mut self.objs.aliens[idx as usize], offset, false, d);
                        }
                    }
                    self.vars.mapptr = p.wrapping_add(6);
                }
                // 108/110: palette fades — no-ops.
                op::FADETOSEA | op::FADETOGROUND => {
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 112: mapqobj — frame(1)<<4, x(1)<<2, y(1)<<2, z(1)<<4,
                // shape(1), strat(1) (WORLD.ASM:1343-1465).
                op::QOBJ => {
                    let frame_raw = self.map_read8(p.wrapping_add(1));
                    let x_raw = self.map_read8(p.wrapping_add(2)) as i8;
                    let y_raw = self.map_read8(p.wrapping_add(3)) as i8;
                    let z_raw = self.map_read8(p.wrapping_add(4));
                    let shape_idx = self.map_read8(p.wrapping_add(5));
                    let strat_idx = self.map_read8(p.wrapping_add(6));
                    self.vars.mapcnt = (frame_raw as u16) << 4;
                    let x = (x_raw as i16) << 2;
                    let y = (y_raw as i16) << 2;
                    let z = ((z_raw as u16) << 4) as i16;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.assign_shape(idx, shape_idx);
                        self.assign_strategy(idx, strat_idx);
                    }
                    self.vars.mapptr = p.wrapping_add(7);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 114: mapobj8 — compact coords + 3-byte strat addr.
                op::OBJ8 => {
                    let frame_raw = self.map_read8(p.wrapping_add(1));
                    let x_raw = self.map_read8(p.wrapping_add(2)) as i8;
                    let y_raw = self.map_read8(p.wrapping_add(3)) as i8;
                    let z_raw = self.map_read8(p.wrapping_add(4)) as i8;
                    let shape = self.map_read16(p.wrapping_add(5));
                    let strat_addr = self.map_read16(p.wrapping_add(7));
                    let strat_bank = self.map_read8(p.wrapping_add(9));
                    self.vars.mapcnt = (frame_raw as u16) << 2;
                    let x = (x_raw as i16) << 2;
                    let y = (y_raw as i16) << 2;
                    let z = (z_raw as i16) << 4; // sign-extended, unlike QOBJ
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.objs.aliens[idx as usize].shape = resolve_shape_word(shape);
                        self.assign_strategy_addr(idx, strat_addr, strat_bank);
                    }
                    self.vars.mapptr = p.wrapping_add(10);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 116: mapdobj — shape derived from strategy table.
                op::DOBJ => {
                    let frame = self.map_read16(p.wrapping_add(1));
                    let x = self.map_read16s(p.wrapping_add(3));
                    let y = self.map_read16s(p.wrapping_add(5));
                    let z = self.map_read16s(p.wrapping_add(7));
                    let strat = self.map_read8(p.wrapping_add(9));
                    self.vars.mapcnt = frame;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.assign_strategy(idx, strat);
                        self.objs.aliens[idx as usize].shape =
                            self.world.istrat_shapes[strat as usize];
                    }
                    self.vars.mapptr = p.wrapping_add(10);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 118: mapqobj2 — qobj minus the shape byte.
                op::QOBJ2 => {
                    let frame_raw = self.map_read8(p.wrapping_add(1));
                    let x_raw = self.map_read8(p.wrapping_add(2)) as i8;
                    let y_raw = self.map_read8(p.wrapping_add(3)) as i8;
                    let z_raw = self.map_read8(p.wrapping_add(4));
                    let strat_idx = self.map_read8(p.wrapping_add(5));
                    self.vars.mapcnt = (frame_raw as u16) << 4;
                    let x = (x_raw as i16) << 2;
                    let y = (y_raw as i16) << 2;
                    let z = ((z_raw as u16) << 4) as i16;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.assign_strategy(idx, strat_idx);
                        self.objs.aliens[idx as usize].shape =
                            self.world.istrat_shapes[strat_idx as usize];
                    }
                    self.vars.mapptr = p.wrapping_add(6);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 120: ctrl65816 — inline native map callback.
                op::CODE65816 => {
                    let script_ptr = p.wrapping_add(1);
                    match self.world.find_inline(script_ptr) {
                        Some(cbk) => {
                            let mut next_ptr = p;
                            self.run_inline(cbk, &mut next_ptr);
                            self.vars.mapptr = next_ptr;
                        }
                        None => return, // C warns and stops for this frame
                    }
                }
                // 122: mapcodejsl — addr(2),bank(1); encoded as func-1.
                op::CODEJSL => {
                    let func_addr = self.map_read16(p.wrapping_add(1));
                    let func_bank = self.map_read8(p.wrapping_add(3));
                    let key = ((func_bank as u32) << 16) | func_addr.wrapping_add(1) as u32;
                    let cbk = self.find_native_callback(key).or_else(|| {
                        // Fallback for raw (unencoded) func addresses.
                        let raw_key = ((func_bank as u32) << 16) | func_addr as u32;
                        self.find_native_callback(raw_key)
                    });
                    if let Some(cbk) = cbk {
                        self.dispatch_native(cbk);
                    }
                    self.vars.mapptr = p.wrapping_add(4);
                }
                // 124-128: conditional jumps on external byte var
                // (WORLD.ASM:287-364).
                op::JMPVARLESS => {
                    let extptr = self.map_read16(p.wrapping_add(1));
                    let cmpval = self.map_read8(p.wrapping_add(4));
                    let target = self.map_read16(p.wrapping_add(5));
                    let diff = self.vars.read_ext8(extptr).wrapping_sub(cmpval) as i8;
                    if diff < 0 {
                        self.vars.mapptr = target;
                    } else {
                        self.vars.mapptr = p.wrapping_add(7);
                    }
                }
                op::JMPVARMORE => {
                    let extptr = self.map_read16(p.wrapping_add(1));
                    let cmpval = self.map_read8(p.wrapping_add(4));
                    let target = self.map_read16(p.wrapping_add(5));
                    let diff = self.vars.read_ext8(extptr).wrapping_sub(cmpval) as i8;
                    if diff > 0 {
                        self.vars.mapptr = target;
                    } else {
                        self.vars.mapptr = p.wrapping_add(7);
                    }
                }
                op::JMPVAREQ => {
                    let extptr = self.map_read16(p.wrapping_add(1));
                    let cmpval = self.map_read8(p.wrapping_add(4));
                    let target = self.map_read16(p.wrapping_add(5));
                    if self.vars.read_ext8(extptr) == cmpval {
                        self.vars.mapptr = target;
                    } else {
                        self.vars.mapptr = p.wrapping_add(7);
                    }
                }
                // 130: sendmessage — msg(1).
                op::SENDMSG => {
                    let msg = self.map_read8(p.wrapping_add(1));
                    self.hooks.send_message(msg);
                    self.vars.mapptr = p.wrapping_add(2);
                }
                // 132: mapCspecial (WORLD.ASM:668-677).
                op::CSPECIAL => {
                    if let Some(idx) = self.world.last_obj {
                        self.objs.aliens[idx as usize].sflags4 |= ASF4_CSPECIAL;
                        self.world.specialobjtotal = self.world.specialobjtotal.wrapping_add(1);
                    }
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 134: mapnormobj — full-width pointers
                // (WORLD.ASM:1835-1887).
                op::NORMOBJ => {
                    let frame = self.map_read16(p.wrapping_add(1));
                    let x = self.map_read16s(p.wrapping_add(3));
                    let y = self.map_read16s(p.wrapping_add(5));
                    let z = self.map_read16s(p.wrapping_add(7));
                    let shape = self.map_read16(p.wrapping_add(9));
                    let strat_addr = self.map_read16(p.wrapping_add(11));
                    let strat_bank = self.map_read8(p.wrapping_add(13));
                    self.vars.mapcnt = frame;
                    if let Some(idx) = self.spawn_object(x, y, z) {
                        self.objs.aliens[idx as usize].shape = resolve_shape_word(shape);
                        self.assign_strategy_addr(idx, strat_addr, strat_bank);
                    }
                    self.vars.mapptr = p.wrapping_add(14);
                    if self.vars.mapcnt != 0 {
                        return;
                    }
                }
                // 136: reserved.
                op::RESERVED => {
                    self.vars.mapptr = p.wrapping_add(1);
                }
                // 138: mapwait2 — distance_raw(1) << 4 (WORLD.ASM:175-187).
                // The ROM stores mapcnt and RTSes UNCONDITIONALLY — no zero
                // check (unlike mapwait). Oracle audit_mapvm2: wait2 0 ends the
                // frame at mapptr=2 on the ROM; the old `continue` ran on.
                op::WAIT2 => {
                    let raw = self.map_read8(p.wrapping_add(1));
                    self.vars.mapcnt = (raw as u16) << 4;
                    self.vars.mapptr = p.wrapping_add(2);
                    return;
                }
                // 140: mapsetpath — path_ptr(2) (WORLD.ASM:162-170).
                op::SETPATH => {
                    if let Some(idx) = self.world.last_obj {
                        let path_ptr = self.map_read16(p.wrapping_add(1));
                        let start = self.hooks.resolve_path_start(path_ptr);
                        self.objs.aliens[idx as usize].sword2 = start as i16;
                    }
                    self.vars.mapptr = p.wrapping_add(3);
                }
                // Unknown opcode: advance one byte and stop (C default).
                _ => {
                    self.vars.mapptr = p.wrapping_add(1);
                    return;
                }
            }
        }
        // Max ops reached — resume next frame (C prints a warning).
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
