//! Collision detection system.
//!
//! C oracle: `src/game/coldet.c` — decompiled from
//! `generate_collist_l` (STRATROU.ASM:30-89), `chkcoll` (COLDET.ASM:225-861)
//! and `do_coll_l` (STRATROU.ASM:2143-2178).

use crate::alien::{
    StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4, ACF_COLLTYPE5,
    ACF_FIRSTFRAME, ACF_WEAPON, AFEXP, ASF3_SAMESHAPECOLLIDE, ASF4_PLAYEROBJ, ASF_COLLDISABLE,
    ASF_COLLIDE, ASF_HITFLASH, ASF_INVISIBLE, ASF_LCOLLIDE,
};
use crate::game::Game;
use crate::vars::{FRAMESPERAP, HARD_AP, PSF3_INTUNNEL};

/// C `MAX_COLLIST` (src/game/coldet.c:36).
pub const MAX_COLLIST: usize = 70;

/// C `DEFAULT_COLL_EXTENT` (src/game/coldet.c:41) — used when shape data
/// isn't loaded.
pub const DEFAULT_COLL_EXTENT: i16 = 20;

// ============================================================
// pcbox (player collision-proxy boxes)
// ============================================================
//
// ROM model (STRAT/PSTRATS.ASM + STRAT/GSTRATS.ASM + INC/GILESALC.INC):
//
// The ROM allocates THREE damage-state objects — `pcboxobj_B`
// (body), `pcboxobj_LW` (left wing), `pcboxobj_RW` (right wing)
// (GILESALC.INC:255-257) — set up by `pBody_Istrat` / `pLWing_Istrat` /
// `pRWing_Istrat` (PSTRATS.ASM:145/262/408) and flagged `playerobj`
// alongside the ship in the per-level player setup (GSTRATS.ASM:100-125).
//
//   * BODY carries HP=playerB_HP(40)/AP=playerB_AP(3) (STRATEQU.INC:325-327);
//     each WING carries HP=playerW_HP(5)/AP=playerW_AP(3) (324-326).
//   * `pBody_strat`/`pLWing_strat`/`pRWing_strat` re-park each box on the
//     ship every frame: body at the ship centre (offset 0,0,0), the wings
//     at `s_add_Roffs2pos ...#±playerW_x,#playerW_y,#playerW_z,0,0,1`
//     (STRATEQU.INC:332-334 → ±33,13,0) rotated by the ship's Z roll ONLY
//     (the `0,0,1` flag = rotz on, rotx/roty off).
//
// In the SNES, the ship object itself carries the collision (a multi-box
// `cl_colbox` list — COLDET.ASM:585-816 — whose sub-boxes set hitflags
// HF1/HF2/HF3), and `playercoll_Istrat` (PSTRATS.ASM:3279) ROUTES those
// hitflags onto the corresponding pcbox (set collide/hitflash/collobjptr on
// pcboxobj_B/LW/RW). Each box's OWN collide-strat then applies the hit:
// `pcolB_strat` (PSTRATS.ASM:213) `s_docoll`s the BODY box HP, sets the
// player hitflash timer + body screenflash, and on body HP==0 the box's exp
// strat drives the death sequence; the wing strats break wings.
//
// The proxy objects are themselves `colldisable`; they only carry HP/AP and
// collision strategies.  The ship remains in the collision list.  Its exact
// three-entry `playerB_col` list is evaluated below and sets HF1/HF2/HF3 on
// the ship, after which `playercoll_Istrat` routes the hit to the appropriate
// proxy object.  This mirrors COLBOXES.ASM:20-22 and PSTRATS.ASM:3332-3350.
//
// The shell's per-level gameplay setup calls [`Game::pcbox_attach_player`].
// A direct-model fallback remains only for isolated sf-game/headless callers
// that intentionally spawn a player without running the level setup.

/// Exact `playerB_col` body half-extents (COLBOXES.ASM:20).
pub const PCBOX_BODY_EXT: (i16, i16, i16) = (10, 10, 20);
/// Exact `playerLW_col` / `playerRW_col` half-extents (COLBOXES.ASM:21-22).
pub const PCBOX_WING_EXT: (i16, i16, i16) = (5, 5, 10);

/// Hit-zone bits written by the player's three collision boxes.
pub const PCBOX_HF_BODY: u8 = 0x01;
pub const PCBOX_HF_LWING: u8 = 0x02;
pub const PCBOX_HF_RWING: u8 = 0x04;

/// Wing offset relative to the ship centre (STRATEQU.INC:332-334
/// playerW_x/y/z). The right wing uses +x, the left wing -x.
pub const PCBOX_WING_X: i16 = 33;
pub const PCBOX_WING_Y: i16 = 13;
pub const PCBOX_WING_Z: i16 = 0;

/// Body/wing box HP and AP (STRATEQU.INC:324-327).
pub const PCBOX_BODY_HP: u8 = 40;
pub const PCBOX_WING_HP: u8 = 5;
pub const PCBOX_BODY_AP: u8 = 3;
pub const PCBOX_WING_AP: u8 = 3;

/// Which of the three player collision boxes a slot is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcboxKind {
    Body,
    LWing,
    RWing,
}

/// Live player damage-proxy slots. Empty = direct model (no boxes).
/// Mirrors the ROM `pcboxobj_B/LW/RW` word vars (GILESALC.INC:255-257) plus
/// the ship slot (`playpt`) that owns the three-box collision list.
#[derive(Debug, Clone, Copy, Default)]
pub struct PcboxState {
    /// The ship object (ROM `playpt`) — owns the live three-box collider.
    pub player: Option<u16>,
    pub body: Option<u16>,
    pub lwing: Option<u16>,
    pub rwing: Option<u16>,
}

impl PcboxState {
    /// True once [`Game::pcbox_attach`] has built the boxes.
    pub fn attached(&self) -> bool {
        self.body.is_some()
    }

    /// Classify a slot, if it is one of the boxes.
    pub fn kind_of(&self, idx: u16) -> Option<PcboxKind> {
        if self.body == Some(idx) {
            Some(PcboxKind::Body)
        } else if self.lwing == Some(idx) {
            Some(PcboxKind::LWing)
        } else if self.rwing == Some(idx) {
            Some(PcboxKind::RWing)
        } else {
            None
        }
    }
}

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
    /// Player collision-proxy boxes (ROM `pcboxobj_B/LW/RW`). Empty until
    /// [`Game::pcbox_attach`] runs.
    pub pcbox: PcboxState,
    /// (body, wing, coll) proxy-box strategy handles, published by the strat
    /// lane during `Strat_RegisterAll` (sf_strat::table::register_all) so the
    /// game-core per-level setup ([`Game::pcbox_attach_player`]) can build the
    /// boxes without an sf-game -> sf-strat dependency. `None` until the strat
    /// lane registers (e.g. headless sf-game-only tests never set it).
    pub pcbox_strats: Option<(StratId, StratId, StratId)>,
}

impl Coldet {
    /// C `Coldet_Init()` (src/game/coldet.c:71).
    pub fn init() -> Self {
        Coldet {
            list: Vec::new(),
            pcbox: PcboxState::default(),
            pcbox_strats: None,
        }
    }
}

/// C `aabb_overlap` (src/game/coldet.c:157, COLDET macro COLDET.ASM:10-65).
/// Axis order Z, X, Y as in the ASM; i16 arithmetic throughout.
pub fn aabb_overlap(
    x1: i16,
    y1: i16,
    z1: i16,
    e1x: i16,
    e1y: i16,
    e1z: i16,
    x2: i16,
    y2: i16,
    z2: i16,
    e2x: i16,
    e2y: i16,
    e2z: i16,
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
    /// Evaluate the exact `playerB_col` three-box list against one normal
    /// collision-list entry.  The ROM ORs every overlapping box's flag, so a
    /// large object can legitimately hit body and wing in the same frame.
    fn pcbox_hitflags(&self, player: u16, other: ColEntry) -> u8 {
        let p = self.objs.aliens[player as usize];
        let o = self.objs.aliens[other.alien as usize];
        let mut flags = 0;

        let overlaps = |x: i16, y: i16, z: i16, ext: (i16, i16, i16)| {
            aabb_overlap(
                x, y, z, ext.0, ext.1, ext.2, o.worldx, o.worldy, o.worldz, other.xmax, other.ymax,
                other.zmax,
            )
        };

        if overlaps(p.worldx, p.worldy, p.worldz, PCBOX_BODY_EXT) {
            flags |= PCBOX_HF_BODY;
        }

        // `s_add_Roffs2pos ...,0,0,1`: rotate the signed byte offsets around
        // Z only.  `strat_roffs_roll` is the byte-exact SNES helper used by the
        // strategy lane for the visible proxy positions as well.
        let (left_dx, left_dy, _) = sf_core::snes_trig::strat_roffs_roll(
            p.rotz,
            -(PCBOX_WING_X as i8),
            PCBOX_WING_Y as i8,
            PCBOX_WING_Z as i8,
        );
        if overlaps(
            p.worldx.wrapping_add(left_dx),
            p.worldy.wrapping_add(left_dy),
            p.worldz.wrapping_add(PCBOX_WING_Z),
            PCBOX_WING_EXT,
        ) {
            flags |= PCBOX_HF_LWING;
        }

        let (rdx, rdy, _) = sf_core::snes_trig::strat_roffs_roll(
            p.rotz,
            PCBOX_WING_X as i8,
            PCBOX_WING_Y as i8,
            PCBOX_WING_Z as i8,
        );
        if overlaps(
            p.worldx.wrapping_add(rdx),
            p.worldy.wrapping_add(rdy),
            p.worldz.wrapping_add(PCBOX_WING_Z),
            PCBOX_WING_EXT,
        ) {
            flags |= PCBOX_HF_RWING;
        }
        flags
    }

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
            let (xmax, ymax, zmax) = self.hooks.shape_extents(al.shape).unwrap_or((
                DEFAULT_COLL_EXTENT,
                DEFAULT_COLL_EXTENT,
                DEFAULT_COLL_EXTENT,
            ));
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
        // ROM do_coll_l ($1FD252): `DEC collcount; BNE exit` — decrement THEN
        // check zero. The port did check-then-decrement, which shifted damage
        // by one frame on every hit and mishandled collcount==0. Oracle-proven
        // (sf-oracle tests/do_coll.rs: 0 diffs after this fix).
        al.collcount = al.collcount.wrapping_sub(1);
        if al.collcount != 0 {
            return;
        }
        let mut damage = attacker_ap;
        // In tunnel mode, halve hardAP damage.
        if in_tunnel && damage == HARD_AP {
            damage >>= 1;
        }
        // Any health byte with its sign bit set is
        // indestructible (the port only treated $FF as such). Reset the
        // cooldown regardless, matching the ROM's `.o2c` fall-through.
        if (al.hp as i8) >= 0 {
            al.hp = al.hp.saturating_sub(damage);
        }
        al.collcount = FRAMESPERAP; // tpa = framesperAP
    }

    /// Strategy-facing `s_docoll`/`s_docollAP` bridge. Collision detection in
    /// the ROM only records the pair; individual collide strategies decide
    /// how much damage to apply. Most of the compatibility port folds that
    /// call into [`Game::coldet_run`], but the player's routed body/wing proxy
    /// strategies need the original operation and its AP scale-down argument.
    pub fn coldet_apply_damage(&mut self, victim: u16, attacker_ap: u8, scale_down: u8) {
        self.do_coll(victim, attacker_ap >> scale_down.min(7));
    }

    /// C `Coldet_Run()` (src/game/coldet.c:179, chkcoll COLDET.ASM:225-861),
    /// prefixed by the per-frame reset from `init_strats_ram_l`
    /// (COLDET.ASM:165-218).
    pub fn coldet_run(&mut self) {
        // Step 1: `init_strats_ram_l` walks ALL active objects, including
        // colldisable proxy objects. It mirrors collide -> Lcollide before
        // clearing collide/collobjptr and seeds collcount on a fresh pair.
        let mut active = Vec::new();
        let mut cur = self.objs.active_head;
        while let Some(idx) = cur {
            active.push(idx);
            cur = self.objs.aliens[idx as usize].next;
        }
        for idx in active {
            let al = &mut self.objs.aliens[idx as usize];
            if al.sflags & ASF_COLLIDE != 0 {
                al.sflags |= ASF_LCOLLIDE;
            } else {
                al.sflags &= !ASF_LCOLLIDE;
                // ROM init_strats_ram_l (COLDET.ASM:172-182): an object that is
                // NOT already colliding gets `al_collcount = 1` each frame
                // (`s_set_alvar B,x,al_collcount,#1`). This is what makes the
                // first do_coll on a fresh collision damage: do_coll DECs
                // collcount (1 -> 0) and the `BNE exit` falls through. The port
                // previously never seeded collcount, so with the ROM-correct
                // do_coll (DEC-then-BNE) a fresh collision saw collcount 0 -> 255
                // and never applied damage.
                al.collcount = 1;
            }
            al.sflags &= !ASF_COLLIDE;
            al.sflags &= !ASF_HITFLASH;
            al.collobjptr = 0;
        }

        // Step 2: test all pairs.
        const TYPE_MASK: u8 =
            ACF_COLLTYPE1 | ACF_COLLTYPE2 | ACF_COLLTYPE3 | ACF_COLLTYPE4 | ACF_COLLTYPE5;
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
                // Same-category filter (COLDET.ASM:518-521): skip ONLY when the
                // pair shares a collision-type bit
                // (`cf1 & cf2 & typemask != 0`). The ROM does NOT require either
                // object to carry a type bit — the earlier port added a spurious
                // `a_types == 0 && b_types == 0 -> skip` (from src/game/coldet.c),
                // which dropped every object that has collflags but no type bit.
                let a_types = a.collflags & TYPE_MASK;
                let b_types = b.collflags & TYPE_MASK;
                if a_types & b_types != 0 {
                    continue;
                }
                // Same-shape gate (ROM chkcoll0, COLDET.ASM; retail $02:A199):
                // skip a pair with equal al_shape UNLESS both set
                // sameshapecollide (sflags3 bit $80) — which ~nothing does, so
                // the cart effectively never collides two same-shape objects. The
                // port lacked this gate (tier-2 coexec-found:
                // retail_same_shape_skip_divergence), so same-shape /
                // different-colltype pairs collided where the cart skips.
                if a.shape == b.shape
                    && !(a.sflags3 & ASF3_SAMESHAPECOLLIDE != 0
                        && b.sflags3 & ASF3_SAMESHAPECOLLIDE != 0)
                {
                    continue;
                }
                // Immunity cross-checks. ROM chkcoll0 (COLDET.ASM:523-529)
                // compares al_immuneptr against the other object's slot directly,
                // with NO nonzero guard — in the ROM immuneptr is a real (nonzero)
                // alien pointer and 0 means "none", so the compare is unambiguous.
                // The port stores immuneptr as a raw 0-based slot and the player
                // is slot 0, so a player-owned projectile stores immuneptr == 0,
                // which is indistinguishable from the default "no owner" 0. The
                // earlier `!= 0` guard therefore dropped a player projectile's
                // immunity and let it hit its own ship. A spawned projectile
                // always carries ACF_WEAPON (set beside immuneptr by its
                // spawner), so treat a weapon's immuneptr as owned even when it
                // is 0 (== the player), while a non-weapon's default 0 still
                // means "no immunity".
                if a.immuneptr == ib && (a.immuneptr != 0 || a.collflags & ACF_WEAPON != 0) {
                    continue;
                }
                if b.immuneptr == ia && (b.immuneptr != 0 || b.collflags & ACF_WEAPON != 0) {
                    continue;
                }
                let ea = self.coldet.list[i];
                let eb = self.coldet.list[j];
                let player_side = if self.coldet.pcbox.player == Some(ia) {
                    Some((ia, ib, self.pcbox_hitflags(ia, eb)))
                } else if self.coldet.pcbox.player == Some(ib) {
                    Some((ib, ia, self.pcbox_hitflags(ib, ea)))
                } else {
                    None
                };
                let hitflags = if let Some((_, _, flags)) = player_side {
                    if flags == 0 {
                        continue;
                    }
                    flags
                } else {
                    if !aabb_overlap(
                        a.worldx, a.worldy, a.worldz, ea.xmax, ea.ymax, ea.zmax, b.worldx,
                        b.worldy, b.worldz, eb.xmax, eb.ymax, eb.zmax,
                    ) {
                        continue;
                    }
                    0
                };

                // --- Collision occurred ---
                self.objs.aliens[ia as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ib as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ia as usize].collobjptr = ib;
                self.objs.aliens[ib as usize].collobjptr = ia;

                if let Some((player, _, _)) = player_side {
                    self.objs.aliens[player as usize].hitflags |= hitflags;
                }

                // Damage: A takes B's AP, B takes A's AP.
                // A live player collision is routed to the colldisable body /
                // wing HP objects by `playercoll_Istrat`; never damage the ship
                // object's placeholder HP here.
                if b.ap > 0 && a.hp > 0 && self.coldet.pcbox.player != Some(ia) {
                    self.do_coll(ia, b.ap);
                }
                if a.ap > 0 && b.hp > 0 && self.coldet.pcbox.player != Some(ib) {
                    self.do_coll(ib, a.ap);
                }

                // NOTE: the collide-strategy is NOT dispatched here. ROM chkcoll
                // (COLDET.ASM:829-846 / :791-821) only SETS collide/collobjptr/
                // hitflags; each object's own collide-strategy runs later from
                // do_strat (game.rs, when ASF_COLLIDE is seen) — the s_docoll
                // side of that split is folded into do_coll above. The earlier
                // port dispatched collstratptr inline here AND again from
                // do_strat on the next frame (ASF_COLLIDE persists), so every
                // collision fired its collide-strategy twice.

                break; // each alien collides at most once per frame
            }
        }
    }

    /// Build the three player collision-proxy boxes and route the ship's body
    /// collisions through them (ROM `pBody_Istrat`/`pLWing_Istrat`/
    /// `pRWing_Istrat`, PSTRATS.ASM:145/262/408, wired by the per-level player
    /// setup GSTRATS.ASM:100-125).
    ///
    /// `player` is the ship slot (`playpt`). `body_strat`/`wing_strat` are the
    /// per-frame re-park strategies; `coll_strat` is the shared collide-strat
    /// that routes a box hit back onto the ship. These are sf-strat registry
    /// handles, passed in because the strategy bodies live in that lane.
    ///
    /// The boxes are `colldisable` state carriers; the ship remains collision
    /// enabled and owns the exact `playerB_col` multi-box collider. Idempotent:
    /// a second call with boxes already live is a no-op.
    pub fn pcbox_attach(
        &mut self,
        player: u16,
        body_strat: StratId,
        wing_strat: StratId,
        coll_strat: StratId,
    ) -> bool {
        if self.coldet.pcbox.attached() {
            return true;
        }
        let Some(body) = self.objs.alloc() else {
            return false;
        };
        let Some(lwing) = self.objs.alloc() else {
            self.objs.free(body);
            return false;
        };
        let Some(rwing) = self.objs.alloc() else {
            self.objs.free(body);
            self.objs.free(lwing);
            return false;
        };

        let (px, py, pz) = {
            let p = &self.objs.aliens[player as usize];
            (p.worldx, p.worldy, p.worldz)
        };
        let setup = |al: &mut crate::alien::Alien, hp: u8, ap: u8, strat: StratId| {
            al.shape = 0;
            al.sflags = ASF_INVISIBLE | ASF_COLLDISABLE;
            al.sflags4 = ASF4_PLAYEROBJ;
            al.collflags = 0;
            al.hp = hp;
            al.ap = ap;
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
            al.stratptr = Some(strat);
            al.collstratptr = Some(coll_strat);
            // The shared dispatcher distinguishes collide-entry from hp==0
            // and implements pcolBexp / PLWbrk / PRWbrk as well.
            al.expstratptr = Some(coll_strat);
            al.endcollstratptr = None;
        };
        setup(
            &mut self.objs.aliens[body as usize],
            PCBOX_BODY_HP,
            PCBOX_BODY_AP,
            body_strat,
        );
        setup(
            &mut self.objs.aliens[lwing as usize],
            PCBOX_WING_HP,
            PCBOX_WING_AP,
            wing_strat,
        );
        setup(
            &mut self.objs.aliens[rwing as usize],
            PCBOX_WING_HP,
            PCBOX_WING_AP,
            wing_strat,
        );

        // MAPP.ASM allocates these in player/body/left/right order and ROM
        // `l_add` inserts after the current map object. Preserve that strategy
        // order even though the compatibility allocator normally pushes new
        // objects at the active-list head.
        self.objs.active_move_after(body, player);
        self.objs.active_move_after(lwing, body);
        self.objs.active_move_after(rwing, lwing);

        // GSTRATS marks all four objects playerobj. PSTRATS marks only the
        // three HP proxies colldisable; the ship itself stays live in coldet.
        let p = &mut self.objs.aliens[player as usize];
        p.sflags &= !ASF_COLLDISABLE;
        p.sflags4 |= ASF4_PLAYEROBJ;

        self.coldet.pcbox = PcboxState {
            player: Some(player),
            body: Some(body),
            lwing: Some(lwing),
            rwing: Some(rwing),
        };
        true
    }

    /// Per-level player-collision setup entry point (ROM GSTRATS.ASM:100-125,
    /// invoked from the `mapplayermode`/exit-base level init): build the three
    /// player collision-proxy boxes on `player` using the box strategy handles
    /// the strat lane published in [`Coldet::pcbox_strats`].
    ///
    /// `Coldet` persists across level loads (it is only `Coldet::init`ed once at
    /// `Game::init`), so any boxes from a previous level are stale slot indices
    /// after the objects table is reset. This first drops the stale
    /// [`PcboxState`] so a fresh attach always rebuilds the boxes for the newly
    /// spawned player. Returns false if the strat lane hasn't registered (no
    /// handles) or object allocation fails.
    pub fn pcbox_attach_player(&mut self, player: u16) -> bool {
        let Some((body, wing, coll)) = self.coldet.pcbox_strats else {
            return false;
        };
        // Fresh level: discard any stale box slots from a previous load.
        self.coldet.pcbox = PcboxState::default();
        self.pcbox_attach(player, body, wing, coll)
    }

    /// Detach the player collision boxes (ROM `playerdead_Istrat`
    /// PSTRATS.ASM:3031-3044): each box gets its strat pointers cleared and
    /// `colldisable` set, so it drops out of the collision list and stops
    /// routing hits. Called from the death sequence; that sequence separately
    /// changes the ship to the enemy-weapon collision category.
    pub fn pcbox_detach(&mut self) {
        for slot in [
            self.coldet.pcbox.body,
            self.coldet.pcbox.lwing,
            self.coldet.pcbox.rwing,
        ]
        .into_iter()
        .flatten()
        {
            let al = &mut self.objs.aliens[slot as usize];
            al.sflags |= ASF_COLLDISABLE;
            al.sflags &= !ASF_COLLIDE;
            al.stratptr = None;
            al.collstratptr = None;
            al.endcollstratptr = None;
            al.expstratptr = None;
        }
        self.coldet.pcbox = PcboxState::default();
    }
}
