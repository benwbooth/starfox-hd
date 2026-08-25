//! Collision detection system.
//!
//! C oracle: `src/game/coldet.c` — decompiled from
//! `generate_collist_l` (STRATROU.ASM:30-89), `chkcoll` (COLDET.ASM:225-861)
//! and `do_coll_l` (STRATROU.ASM:2143-2178).

use crate::alien::{
    Alien, StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4, ACF_COLLTYPE5,
    ACF_FIRSTFRAME, ACF_WEAPON, AFEXP, ASF2_COLLDISABLE, ASF2_LCOLLIDE, ASF3_SAMESHAPECOLLIDE,
    ASF4_PLAYEROBJ, ASF_COLLIDE, ASF_HITFLASH,
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

/// One flat native collision volume from a source shape's typed box list.
/// Values retain the source coordinate shift so rotated authored offsets are
/// transformed before being expanded into gameplay world units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShapeCollisionBox {
    offset: (i16, i16, i16),
    half_extents: (i16, i16, i16),
    hit_flags: u8,
    rotation: CollisionBoxRotation,
    coordinate_shift: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollisionBoxRotation {
    None,
    Roll,
}

const SHAPE_ARCH: u16 = 228;
const SHAPE_BASE_1: u16 = 232;
const SHAPE_BIG_GATE: u16 = 233;
const SHAPE_PILLAR3: u16 = 27;
const BASE_1_ANIMATION_FRAME_MASK: u8 = 7;
const ARCH_COLLISION_BOXES: [ShapeCollisionBox; 3] = [
    ShapeCollisionBox {
        offset: (-100, -60, 0),
        half_extents: (20, 60, 20),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
    ShapeCollisionBox {
        offset: (100, -60, 0),
        half_extents: (20, 60, 20),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
    ShapeCollisionBox {
        offset: (0, -140, 0),
        half_extents: (60, 20, 20),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
];
// COLBOXES.ASM:783-787 `big_gate_col1..3`, each authored with scale 2.
// The two posts and overhead beam leave the center flight path open; using
// the mesh/header bounds as one solid box makes the player hit empty space.
const BIG_GATE_COLLISION_BOXES: [ShapeCollisionBox; 3] = [
    ShapeCollisionBox {
        offset: (-180, -100, 0),
        half_extents: (20, 100, 220),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
    ShapeCollisionBox {
        offset: (180, -100, 0),
        half_extents: (20, 100, 220),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
    ShapeCollisionBox {
        offset: (0, -220, 0),
        half_extents: (200, 20, 220),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    },
];

// COLBOXES.ASM `base_1_col1..3`. The animated center door follows these
// permanent posts and lintel in the source table.
const BASE_1_FIXED_COLLISION_BOXES: [ShapeCollisionBox; 3] = [
    ShapeCollisionBox {
        offset: (-32, -20, 5),
        half_extents: (7, 20, 30),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (32, -20, 5),
        half_extents: (7, 20, 30),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -47, 5),
        half_extents: (40, 7, 30),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
];

// COLBOXES.ASM `base_1_col4`: eight animation-selected center-door boxes.
// The source starts with an empty opening and grows the door downward. Its
// `colframes 8` selector uses the low three bits of a manually controlled
// animation frame (`animframe` has its high bit set for this strategy).
const BASE_1_ANIMATED_COLLISION_BOXES: [ShapeCollisionBox; 8] = [
    ShapeCollisionBox {
        offset: (0, -35, -30),
        half_extents: (25, 0, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -33, -30),
        half_extents: (25, 2, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -30, -30),
        half_extents: (25, 5, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -28, -30),
        half_extents: (25, 7, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -25, -30),
        half_extents: (25, 10, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -23, -30),
        half_extents: (25, 12, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -20, -30),
        half_extents: (25, 15, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
    ShapeCollisionBox {
        offset: (0, -18, -30),
        half_extents: (25, 17, 5),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 3,
    },
];

#[derive(Debug, Clone, Copy)]
struct ShapeCollisionBoxes {
    fixed: &'static [ShapeCollisionBox],
    animated: Option<ShapeCollisionBox>,
}

impl ShapeCollisionBoxes {
    fn iter(self) -> impl Iterator<Item = ShapeCollisionBox> {
        self.fixed.iter().copied().chain(self.animated)
    }
}

// COLBOXES.ASM:150-157 `pillar3_col1..8`. The broad ShapeHdr bounds are
// only an outer rejection volume; the actual solid target is this narrow
// stack of eight boxes. Entries two through eight follow the pillar's roll.
const PILLAR3_COLLISION_BOXES: [ShapeCollisionBox; 8] = [
    ShapeCollisionBox {
        offset: (0, -10, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_LWING,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -20, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -30, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -40, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -50, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -60, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -70, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
    ShapeCollisionBox {
        offset: (0, -80, 0),
        half_extents: (6, 5, 6),
        hit_flags: PCBOX_HF_BODY,
        rotation: CollisionBoxRotation::Roll,
        coordinate_shift: 2,
    },
];

fn shape_collision_boxes(object: Alien) -> Option<ShapeCollisionBoxes> {
    let fixed = match object.shape {
        SHAPE_ARCH => &ARCH_COLLISION_BOXES[..],
        SHAPE_BASE_1 => {
            return Some(ShapeCollisionBoxes {
                fixed: &BASE_1_FIXED_COLLISION_BOXES,
                animated: Some(
                    BASE_1_ANIMATED_COLLISION_BOXES
                        [usize::from(object.animframe & BASE_1_ANIMATION_FRAME_MASK)],
                ),
            });
        }
        SHAPE_BIG_GATE => &BIG_GATE_COLLISION_BOXES[..],
        SHAPE_PILLAR3 => &PILLAR3_COLLISION_BOXES[..],
        _ => return None,
    };
    Some(ShapeCollisionBoxes {
        fixed,
        animated: None,
    })
}

fn resolve_collision_box(object: Alien, collision_box: ShapeCollisionBox) -> ShapeCollisionBox {
    let offset = match collision_box.rotation {
        CollisionBoxRotation::None => collision_box.offset,
        CollisionBoxRotation::Roll => sf_core::snes_trig::strat_roffs_roll(
            object.rotz,
            collision_box.offset.0 as i8,
            collision_box.offset.1 as i8,
            collision_box.offset.2 as i8,
        ),
    };
    let shift = u32::from(collision_box.coordinate_shift);
    ShapeCollisionBox {
        offset: (
            offset.0.wrapping_shl(shift),
            offset.1.wrapping_shl(shift),
            offset.2.wrapping_shl(shift),
        ),
        half_extents: (
            collision_box.half_extents.0.wrapping_shl(shift),
            collision_box.half_extents.1.wrapping_shl(shift),
            collision_box.half_extents.2.wrapping_shl(shift),
        ),
        hit_flags: collision_box.hit_flags,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    }
}

fn collision_box_overlap(
    first: Alien,
    first_box: ShapeCollisionBox,
    second: Alien,
    second_box: ShapeCollisionBox,
) -> bool {
    let first_box = resolve_collision_box(first, first_box);
    let second_box = resolve_collision_box(second, second_box);
    aabb_overlap(
        first.worldx.wrapping_add(first_box.offset.0),
        first.worldy.wrapping_add(first_box.offset.1),
        first.worldz.wrapping_add(first_box.offset.2),
        first_box.half_extents.0,
        first_box.half_extents.1,
        first_box.half_extents.2,
        second.worldx.wrapping_add(second_box.offset.0),
        second.worldy.wrapping_add(second_box.offset.1),
        second.worldz.wrapping_add(second_box.offset.2),
        second_box.half_extents.0,
        second_box.half_extents.1,
        second_box.half_extents.2,
    )
}

fn object_collision_hit(
    first: Alien,
    first_entry: ColEntry,
    second: Alien,
    second_entry: ColEntry,
) -> Option<(u8, u8)> {
    let first_default = ShapeCollisionBox {
        offset: (0, 0, 0),
        half_extents: (first_entry.xmax, first_entry.ymax, first_entry.zmax),
        hit_flags: 0,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    };
    let second_default = ShapeCollisionBox {
        offset: (0, 0, 0),
        half_extents: (second_entry.xmax, second_entry.ymax, second_entry.zmax),
        hit_flags: 0,
        rotation: CollisionBoxRotation::None,
        coordinate_shift: 0,
    };
    let first_boxes = shape_collision_boxes(first)
        .map(|boxes| boxes.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![first_default]);
    let second_boxes = shape_collision_boxes(second)
        .map(|boxes| boxes.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![second_default]);

    // Source ordering is significant. It scans one box from the first object
    // against every box from the second, combines the second object's region
    // flags, and stops on the first box of the first object that hits.
    for first_box in first_boxes {
        let mut second_hit_flags = 0;
        let mut collided = false;
        for second_box in second_boxes.iter().copied() {
            if collision_box_overlap(first, first_box, second, second_box) {
                collided = true;
                second_hit_flags |= second_box.hit_flags;
            }
        }
        if collided {
            return Some((first_box.hit_flags, second_hit_flags));
        }
    }
    None
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerBoxScan {
    FirstMatch,
    AllMatches,
}

/// Player damage-proxy slots. Empty = direct model (no boxes).
/// Mirrors the ROM `pcboxobj_B/LW/RW` word vars (GILESALC.INC:255-257) plus
/// the ship slot (`playpt`) that owns the three-box collision list. The ROM
/// invalidates `playpt` on death but deliberately retains all three proxy
/// pointers so `calcmeters` can continue reading the depleted body HP.
#[derive(Debug, Clone, Copy, Default)]
pub struct PcboxState {
    /// The ship object (ROM `playpt`) — owns the live three-box collider.
    pub player: Option<u16>,
    pub body: Option<u16>,
    pub lwing: Option<u16>,
    pub rwing: Option<u16>,
}

impl PcboxState {
    /// True while the ship still routes collisions through the boxes.
    pub fn attached(&self) -> bool {
        self.player.is_some() && self.body.is_some()
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
    /// collision-list entry. The source scan is intentionally asymmetric:
    /// when the player is the outer object it returns after the first matching
    /// body/left/right box; when the player is the inner object it visits and
    /// combines every matching box before returning.
    fn pcbox_collision_hit(
        &self,
        player: u16,
        other: ColEntry,
        scan: PlayerBoxScan,
    ) -> Option<(u8, u8)> {
        let p = self.objs.aliens[player as usize];
        let o = self.objs.aliens[other.alien as usize];
        let other_default = ShapeCollisionBox {
            offset: (0, 0, 0),
            half_extents: (other.xmax, other.ymax, other.zmax),
            hit_flags: 0,
            rotation: CollisionBoxRotation::None,
            coordinate_shift: 0,
        };
        let other_boxes = shape_collision_boxes(o)
            .map(|boxes| boxes.iter().collect::<Vec<_>>())
            .unwrap_or_else(|| vec![other_default]);

        let overlaps = |x: i16, y: i16, z: i16, ext: (i16, i16, i16), target: ShapeCollisionBox| {
            let target = resolve_collision_box(o, target);
            aabb_overlap(
                x,
                y,
                z,
                ext.0,
                ext.1,
                ext.2,
                o.worldx.wrapping_add(target.offset.0),
                o.worldy.wrapping_add(target.offset.1),
                o.worldz.wrapping_add(target.offset.2),
                target.half_extents.0,
                target.half_extents.1,
                target.half_extents.2,
            )
        };

        // `s_add_Roffs2pos ...,0,0,1`: rotate the signed byte offsets around
        // Z only.  `strat_roffs_roll` is the byte-exact SNES helper used by the
        // strategy lane for the visible proxy positions as well.
        let (left_dx, left_dy, _) = sf_core::snes_trig::strat_roffs_roll(
            p.rotz,
            -(PCBOX_WING_X as i8),
            PCBOX_WING_Y as i8,
            PCBOX_WING_Z as i8,
        );
        let (rdx, rdy, _) = sf_core::snes_trig::strat_roffs_roll(
            p.rotz,
            PCBOX_WING_X as i8,
            PCBOX_WING_Y as i8,
            PCBOX_WING_Z as i8,
        );

        let player_boxes = [
            (p.worldx, p.worldy, p.worldz, PCBOX_BODY_EXT, PCBOX_HF_BODY),
            (
                p.worldx.wrapping_add(left_dx),
                p.worldy.wrapping_add(left_dy),
                p.worldz.wrapping_add(PCBOX_WING_Z),
                PCBOX_WING_EXT,
                PCBOX_HF_LWING,
            ),
            (
                p.worldx.wrapping_add(rdx),
                p.worldy.wrapping_add(rdy),
                p.worldz.wrapping_add(PCBOX_WING_Z),
                PCBOX_WING_EXT,
                PCBOX_HF_RWING,
            ),
        ];

        match scan {
            PlayerBoxScan::FirstMatch => {
                for (x, y, z, ext, player_hit_flags) in player_boxes {
                    let mut other_hit_flags = 0;
                    let mut collided = false;
                    for other_box in other_boxes.iter().copied() {
                        if overlaps(x, y, z, ext, other_box) {
                            collided = true;
                            other_hit_flags |= other_box.hit_flags;
                        }
                    }
                    if collided {
                        return Some((player_hit_flags, other_hit_flags));
                    }
                }
            }
            PlayerBoxScan::AllMatches => {
                for other_box in other_boxes {
                    let mut player_hit_flags = 0;
                    for (x, y, z, ext, hit_flags) in player_boxes {
                        if overlaps(x, y, z, ext, other_box) {
                            player_hit_flags |= hit_flags;
                        }
                    }
                    if player_hit_flags != 0 {
                        return Some((player_hit_flags, other_box.hit_flags));
                    }
                }
            }
        }
        None
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
                || al.sflags2 & ASF2_COLLDISABLE != 0
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
                al.sflags2 |= ASF2_LCOLLIDE;
            } else {
                al.sflags2 &= !ASF2_LCOLLIDE;
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
                let collision_hit = if self.coldet.pcbox.player == Some(ia) {
                    self.pcbox_collision_hit(ia, eb, PlayerBoxScan::FirstMatch)
                } else if self.coldet.pcbox.player == Some(ib) {
                    self.pcbox_collision_hit(ib, ea, PlayerBoxScan::AllMatches)
                        .map(|(player_hit_flags, other_hit_flags)| {
                            (other_hit_flags, player_hit_flags)
                        })
                } else {
                    object_collision_hit(a, ea, b, eb)
                };
                let Some((ia_hit_flags, ib_hit_flags)) = collision_hit else {
                    continue;
                };

                // --- Collision occurred ---
                self.objs.aliens[ia as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ib as usize].sflags |= ASF_COLLIDE;
                self.objs.aliens[ia as usize].collobjptr = ib;
                self.objs.aliens[ib as usize].collobjptr = ia;

                self.objs.aliens[ia as usize].hitflags |= ia_hit_flags;
                self.objs.aliens[ib as usize].hitflags |= ib_hit_flags;

                // Collision detection only records the pair. The source
                // `chkcoll` routine never changes health; each object's
                // collide strategy decides whether and how to apply the other
                // object's attack power on the following strategy pass.

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
            al.sflags = 0;
            al.sflags2 = ASF2_COLLDISABLE;
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

        // MAPP.ASM emits player/body/left/right as four consecutive map
        // objects. Each allocation becomes the new list head, so strategy
        // order is player -> right -> left -> body once the player is restored
        // as the gameplay head. This ordering is observable when a wing hit
        // inserts its spark and flash after the current proxy.
        self.objs.active_move_after(rwing, player);
        self.objs.active_move_after(lwing, rwing);
        self.objs.active_move_after(body, lwing);

        // GSTRATS marks all four objects playerobj. PSTRATS marks the three HP
        // proxies colldisable. Preserve the ship's current source flag byte:
        // its active strategy clears the startup `colldisable` at the exact
        // control-body boundary (Training intentionally reaches that one
        // update after the boxes are attached).
        let p = &mut self.objs.aliens[player as usize];
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
            al.sflags2 |= ASF2_COLLDISABLE;
            al.sflags &= !ASF_COLLIDE;
            al.stratptr = None;
            al.collstratptr = None;
            al.endcollstratptr = None;
            al.expstratptr = None;
        }
        // `playerdead_Istrat` makes `playpt` invalid but does not clear the
        // three `pcboxobj_*` words. Keep those typed slot references so the
        // HUD reads the body's terminal HP exactly as `calcmeters` does.
        self.coldet.pcbox.player = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{object_collision_hit, ColEntry, PCBOX_HF_LWING, SHAPE_PILLAR3};
    use crate::alien::Alien;

    const SHAPE_ENEMY_LASER: u16 = 478;
    const PILLAR_POSITION: (i16, i16, i16) = (400, 0, -28_533);
    const CORNERIA_LASER_POSITION: (i16, i16, i16) = (752, -192, -28_644);
    const LASER_EXTENTS: (i16, i16, i16) = (8, 8, 120);
    const PILLAR_HEADER_EXTENTS: (i16, i16, i16) = (480, 480, 48);

    fn entry(alien: u16, extents: (i16, i16, i16)) -> ColEntry {
        ColEntry {
            alien,
            xmax: extents.0,
            ymax: extents.1,
            zmax: extents.2,
        }
    }

    #[test]
    fn pillar_uses_its_narrow_authored_box_stack() {
        let laser = Alien {
            shape: SHAPE_ENEMY_LASER,
            worldx: CORNERIA_LASER_POSITION.0,
            worldy: CORNERIA_LASER_POSITION.1,
            worldz: CORNERIA_LASER_POSITION.2,
            ..Alien::default()
        };
        let pillar = Alien {
            shape: SHAPE_PILLAR3,
            worldx: PILLAR_POSITION.0,
            worldy: PILLAR_POSITION.1,
            worldz: PILLAR_POSITION.2,
            ..Alien::default()
        };

        assert_eq!(
            object_collision_hit(
                laser,
                entry(0, LASER_EXTENTS),
                pillar,
                entry(1, PILLAR_HEADER_EXTENTS),
            ),
            None
        );

        let centered_laser = Alien {
            worldx: PILLAR_POSITION.0,
            worldy: PILLAR_POSITION.1 - 40,
            worldz: PILLAR_POSITION.2,
            ..laser
        };
        assert_eq!(
            object_collision_hit(
                centered_laser,
                entry(0, LASER_EXTENTS),
                pillar,
                entry(1, PILLAR_HEADER_EXTENTS),
            ),
            Some((0, PCBOX_HF_LWING))
        );
    }
}
