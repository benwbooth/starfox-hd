//! 3D object (alien) system — allocation, free lists, list walking.
//!
//! C oracle: `src/game/obj.c` (decompiled from OBJ.ASM, MAIN.ASM
//! kill_list_l/newal_l/removedeadal_l and TRANS.ASM dostrats). The strategy
//! dispatch loop itself (`Obj_RunStrategies` / `do_strat_l`) lives in
//! [`crate::game`] because it needs the map VM and strategy registry.
//!
//! List-order semantics are load-bearing for trace parity:
//! - `Obj_Init` pushes free slots 69..0 -> free head is slot 0.
//! - `Obj_KillAll` pushes 0..69 -> free head is slot 69 (C asymmetry kept).
//! - `Obj_Alloc` pops the free head and pushes it on the active head.
//! - `Obj_Free` unlinks and pushes on the free head (LIFO reuse).

use crate::alien::{Alien, ACF_FIRSTFRAME, ATZREMOVE, NUMBER_AL};

/// The alien pool plus the two intrusive lists (C `g_aliens`,
/// `g_active_list` (allst), `g_free_list` (alfreelst), `g_aldead`).
pub struct Objects {
    /// C `g_aliens[NUMBER_AL]`.
    pub aliens: Vec<Alien>,
    /// C `g_active_list` head (slot index).
    pub active_head: Option<u16>,
    /// C `g_free_list` head.
    pub free_head: Option<u16>,
    /// C `g_aldead` — death flag set by strategies, checked by the
    /// dostrats loop after each strategy call.
    pub aldead: u8,
}

impl Objects {
    /// C `Obj_Init()` (src/game/obj.c:53): clear all alien data, build the
    /// free list (69..0 push-front -> head 0), empty active list.
    pub fn init() -> Self {
        let mut o = Objects {
            aliens: vec![Alien::default(); NUMBER_AL],
            active_head: None,
            free_head: None,
            aldead: 0,
        };
        for i in (0..NUMBER_AL as u16).rev() {
            o.aliens[i as usize].active = false;
            o.free_push_front(i);
        }
        o
    }

    /// C `Obj_KillAll()` (src/game/obj.c:73): full wipe, rebuild free list
    /// pushing 0..69 (free head ends at slot 69 — kept exactly like C).
    pub fn kill_all(&mut self) {
        self.active_head = None;
        self.free_head = None;
        for i in 0..NUMBER_AL as u16 {
            self.aliens[i as usize] = Alien::default();
            self.free_push_front(i);
        }
    }

    // C `list_push_front` (src/game/obj.c:44) specialized per list head.
    fn free_push_front(&mut self, idx: u16) {
        let head = self.free_head;
        {
            let al = &mut self.aliens[idx as usize];
            al.next = head;
            al.prev = None;
        }
        if let Some(h) = head {
            self.aliens[h as usize].prev = Some(idx);
        }
        self.free_head = Some(idx);
    }

    fn active_push_front(&mut self, idx: u16) {
        let head = self.active_head;
        {
            let al = &mut self.aliens[idx as usize];
            al.next = head;
            al.prev = None;
        }
        if let Some(h) = head {
            self.aliens[h as usize].prev = Some(idx);
        }
        self.active_head = Some(idx);
    }

    // C `list_unlink` (src/game/obj.c:35). `head` selects which list head
    // to patch when the node is the head.
    fn unlink(&mut self, idx: u16, from_active: bool) {
        let (next, prev) = {
            let al = &self.aliens[idx as usize];
            (al.next, al.prev)
        };
        match prev {
            Some(p) => self.aliens[p as usize].next = next,
            None => {
                if from_active {
                    self.active_head = next;
                } else {
                    self.free_head = next;
                }
            }
        }
        if let Some(n) = next {
            self.aliens[n as usize].prev = prev;
        }
        let al = &mut self.aliens[idx as usize];
        al.next = None;
        al.prev = None;
    }

    /// C `Obj_Alloc()` (src/game/obj.c:88): take from free list, zero the
    /// block, set firstframe collflag, insert at active head.
    pub fn alloc(&mut self) -> Option<u16> {
        let idx = self.free_head?;
        self.unlink(idx, false);
        self.aliens[idx as usize] = Alien::default();
        {
            let al = &mut self.aliens[idx as usize];
            al.active = true;
            al.collflags = ACF_FIRSTFRAME;
        }
        self.active_push_front(idx);
        Some(idx)
    }

    /// C `Obj_Free()` (src/game/obj.c:108): unlink from active list,
    /// return to free list. No-op if not active.
    pub fn free(&mut self, idx: u16) {
        if idx as usize >= NUMBER_AL || !self.aliens[idx as usize].active {
            return;
        }
        self.unlink(idx, true);
        {
            let al = &mut self.aliens[idx as usize];
            al.active = false;
            al.stratptr = None;
        }
        self.free_push_front(idx);
    }

    /// C `Obj_GetByIndex()` (src/game/obj.c:120) — bounds-checked slot.
    pub fn get(&self, idx: i32) -> Option<&Alien> {
        if idx < 0 || idx >= NUMBER_AL as i32 {
            return None;
        }
        Some(&self.aliens[idx as usize])
    }

    /// C `Obj_GetPlayer()` (src/game/obj.c:125): slot 0 when active.
    pub fn player(&self) -> Option<&Alien> {
        if self.aliens[0].active {
            Some(&self.aliens[0])
        } else {
            None
        }
    }

    /// Iterate the active list in list order, tolerating mutation between
    /// steps (mirrors C `for (al = g_active_list; al; al = al->next)` when
    /// the caller pre-saves `next`). Returns slot indices.
    pub fn active_indices(&self) -> Vec<u16> {
        let mut v = Vec::new();
        let mut cur = self.active_head;
        while let Some(i) = cur {
            v.push(i);
            cur = self.aliens[i as usize].next;
        }
        v
    }
}

/// C `Strat_InitObjVars()` (src/strat/strat_common.c:218, decompiled from
/// STRATROU.ASM init_objvars_l) — spawn-time field defaults. Lives here (not
/// sf-strat) because the map VM's spawn path depends on it.
/// TODO(consolidation): move to sf-strat when that lane lands.
pub fn strat_init_obj_vars(al: &mut Alien) {
    al.sflags = 0;
    al.sflags2 = 0;
    al.hp = 0;
    al.ap = 0;
    al.vx = 0;
    al.vy = 0;
    al.vz = 0;
    al.count = 0;
    al.count1 = 0;
    // Same ROM zeroing (init_objvars_l) applies to animframe: default 0 = bit7
    // clear = follow gameframe (animate). 0xFF = bit7 set = FIXED frame 127
    // (= last frame for most meshes), which froze multi-frame shape animations.
    al.animframe = 0;
    // ROM init_objvars_l (STRATROU.ASM:2311) zeroes the struct -> colframe=0 =
    // bit7 CLEAR = "follow gameframe" (MAIN.ASM:2205-2216: bmi->fixed frame,
    // else gameframe). The port had 0xFF = bit7 SET = FIXED frame 127, which
    // froze animated colanims on one frame — e.g. the player laser stuck on
    // bullet_a1[127&3=3]=blue instead of shimmering white/cyan/blue. Objects
    // wanting a fixed frame set it explicitly (bit7 set), as in the ROM.
    al.colframe = 0;
    al.collflags = ACF_FIRSTFRAME;
    // init_objvars_l sets `s_setremove_behind` (atzremove) on every object by
    // default (STRATROU.ASM:2311); showview frees it once it scrolls behind
    // the camera. Objects that must persist off-screen clear this bit later.
    al.type_ = ATZREMOVE;
}
