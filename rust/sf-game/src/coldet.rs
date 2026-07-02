//! Collision detection system.
//!
//! C oracle: `src/game/coldet.c` — decompiled from
//! `generate_collist_l` (STRATROU.ASM:30-89), `chkcoll` (COLDET.ASM:225-861)
//! and `do_coll_l` (STRATROU.ASM:2143-2178).

use crate::alien::{
    ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4, ACF_COLLTYPE5,
    ACF_FIRSTFRAME, AFEXP, ASF_COLLDISABLE, ASF_COLLIDE, ASF_HITFLASH, ASF_LCOLLIDE,
};
use crate::game::Game;
use crate::vars::{FRAMESPERAP, HARD_AP, PSF3_INTUNNEL};

/// C `MAX_COLLIST` (src/game/coldet.c:36).
pub const MAX_COLLIST: usize = 70;

/// C `DEFAULT_COLL_EXTENT` (src/game/coldet.c:41) — used when shape data
/// isn't loaded.
pub const DEFAULT_COLL_EXTENT: i16 = 20;

/// Collision list entry (C `ColListEntry`, from STRUCTS.INC cl_ structure).
#[derive(Debug, Clone, Copy)]
pub struct ColEntry {
    /// C `cl_alien` — alien slot index.
    pub alien: u16,
    /// C `cl_xmax/ymax/zmax` — collision box half-extents.
    pub xmax: i16,
    pub ymax: i16,
    pub zmax: i16,
}

/// Collision system state (C file-statics `s_collist`/`s_collist_count`).
pub struct Coldet {
    pub list: Vec<ColEntry>,
}

impl Coldet {
    /// C `Coldet_Init()` (src/game/coldet.c:71).
    pub fn init() -> Self {
        Coldet { list: Vec::new() }
    }
}

/// C `aabb_overlap` (src/game/coldet.c:157, COLDET macro COLDET.ASM:10-65).
/// Axis order Z, X, Y as in the ASM; i16 arithmetic throughout.
pub fn aabb_overlap(
    x1: i16, y1: i16, z1: i16, e1x: i16, e1y: i16, e1z: i16,
    x2: i16, y2: i16, z2: i16, e2x: i16, e2y: i16, e2z: i16,
) -> bool {
    let mut dz = z2.wrapping_sub(z1);
    if dz < 0 {
        dz = dz.wrapping_neg();
    }
    if dz >= e1z.wrapping_add(e2z) {
        return false;
    }
    let mut dx = x2.wrapping_sub(x1);
    if dx < 0 {
        dx = dx.wrapping_neg();
    }
    if dx >= e1x.wrapping_add(e2x) {
        return false;
    }
    let mut dy = y2.wrapping_sub(y1);
    if dy < 0 {
        dy = dy.wrapping_neg();
    }
    if dy >= e1y.wrapping_add(e2y) {
        return false;
    }
    true
}

impl Game {
    /// C `Coldet_GenerateList()` (src/game/coldet.c:75) — walk the active
    /// list, skip ineligible aliens, build collision entries. Extents come
    /// from the shape hook (C `load_collision_extents`); missing shape data
    /// falls back to DEFAULT_COLL_EXTENT on every axis.
    pub fn coldet_generate_list(&mut self) {
        self.coldet.list.clear();
        let mut cur = self.objs.active_head;
        while let Some(i) = cur {
            if self.coldet.list.len() >= MAX_COLLIST {
                break;
            }
            let al = &self.objs.aliens[i as usize];
            let next = al.next;
            // Skip: just spawned / collision disabled / no HP / exploding.
            if al.collflags & ACF_FIRSTFRAME != 0
                || al.sflags & ASF_COLLDISABLE != 0
                || al.hp == 0
                || al.flags & AFEXP != 0
            {
                cur = next;
                continue;
            }
            let (xmax, ymax, zmax) = self
                .hooks
                .shape_extents(al.shape)
                .unwrap_or((DEFAULT_COLL_EXTENT, DEFAULT_COLL_EXTENT, DEFAULT_COLL_EXTENT));
            self.coldet.list.push(ColEntry {
                alien: i,
                xmax,
                ymax,
                zmax,
            });
            cur = next;
        }
    }

    /// C `do_coll` (src/game/coldet.c:120, do_coll_l STRATROU.ASM:2143) —
    /// apply damage with the framesperAP cooldown.
    fn do_coll(&mut self, victim: u16, attacker_ap: u8) {
        let in_tunnel = self.vars.pshipflags3 & PSF3_INTUNNEL != 0;
        let al = &mut self.objs.aliens[victim as usize];
        // Cooldown timer: only apply damage every framesperAP frames.
        if al.collcount > 0 {
            al.collcount -= 1;
            return;
        }
        // Indestructible objects (hardHP) don't take damage.
        if al.hp == 0xFF {
            al.collcount = FRAMESPERAP;
            return;
        }
        let mut damage = attacker_ap;
        // In tunnel mode, halve hardAP damage.
        if in_tunnel && damage == HARD_AP {
            damage >>= 1;
        }
        if al.hp <= damage {
            al.hp = 0;
        } else {
            al.hp -= damage;
        }
        al.collcount = FRAMESPERAP;
    }

    /// C `Coldet_Run()` (src/game/coldet.c:179, chkcoll COLDET.ASM:225-861).
    pub fn coldet_run(&mut self) {
        // Step 1: clear per-frame collision state (init_strats_ram_l
        // mirrors collide -> Lcollide before clearing collide).
        for k in 0..self.coldet.list.len() {
            let idx = self.coldet.list[k].alien;
            let al = &mut self.objs.aliens[idx as usize];
            if al.sflags & ASF_COLLIDE != 0 {
                al.sflags |= ASF_LCOLLIDE;
            } else {
                al.sflags &= !ASF_LCOLLIDE;
            }
            al.sflags &= !ASF_COLLIDE;
            al.sflags &= !ASF_HITFLASH;
            al.collobjptr = 0;
        }

        // Step 2: test all pairs.
        const TYPE_MASK: u8 = ACF_COLLTYPE1 | ACF_COLLTYPE2 | ACF_COLLTYPE3
            | ACF_COLLTYPE4 | ACF_COLLTYPE5;
        for i in 0..self.coldet.list.len() {
            let ia = self.coldet.list[i].alien;
            if self.objs.aliens[ia as usize].sflags & ASF_COLLIDE != 0 {
                continue; // already colliding this frame
            }
            for j in (i + 1)..self.coldet.list.len() {
                let ib = self.coldet.list[j].alien;
                if self.objs.aliens[ib as usize].sflags & ASF_COLLIDE != 0 {
                    continue;
                }
                let a = self.objs.aliens[ia as usize];
                let b = self.objs.aliens[ib as usize];
                // Same-category filter: shared type bits never collide.
                let a_types = a.collflags & TYPE_MASK;
                let b_types = b.collflags & TYPE_MASK;
                if a_types & b_types != 0 {
                    continue;
                }
                if a_types == 0 && b_types == 0 {
                    continue;
                }
                // Immunity cross-checks (C compares against slot index).
                if a.immuneptr != 0 && a.immuneptr == ib {
                    continue;
                }
                if b.immuneptr != 0 && b.immuneptr == ia {
                    continue;
                }
                let ea = self.coldet.list[i];
                let eb = self.coldet.list[j];
                if !aabb_overlap(
                    a.worldx, a.worldy, a.worldz, ea.xmax, ea.ymax, ea.zmax,
                    b.worldx, b.worldy, b.worldz, eb.xmax, eb.ymax, eb.zmax,
                ) {
                    continue;
                }

                // --- Collision occurred ---
                self.objs.aliens[ia as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ib as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ia as usize].collobjptr = ib;
                self.objs.aliens[ib as usize].collobjptr = ia;

                // Damage: A takes B's AP, B takes A's AP.
                if b.ap > 0 && a.hp > 0 {
                    self.do_coll(ia, b.ap);
                }
                if a.ap > 0 && b.hp > 0 {
                    self.do_coll(ib, a.ap);
                }

                // Collision strategy callbacks fire immediately.
                if let Some(cs) = self.objs.aliens[ia as usize].collstratptr {
                    self.call_strat(cs, ia);
                }
                if let Some(cs) = self.objs.aliens[ib as usize].collstratptr {
                    self.call_strat(cs, ib);
                }

                break; // each alien collides at most once per frame
            }
        }
    }
}
