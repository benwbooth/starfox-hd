//! Build the per-tick draw list from the active alien list.
//!
//! C oracle: `src/game/draw.c` (`Draw_BuildList`, decompiled from
//! marioshowview MAIN.ASM:2097-2271) plus the submit-side cap in
//! `src/game/boot.c:295-299` (`Game_SubmitDrawEntry`, MAX_DRAW_LIST).
//!
//! Ported field-for-field, including the explosion/partobj shadow fields,
//! the anim/col frame bit-7 rule, `obj_id` = alien index + 1, the
//! DL_FLAG_SHADOW gate on `(sflags & ASF_SHADOW) && (playerflymode &
//! PFM_SHADOWS)`, and the AF_* placement-flag clears/sets on the alien.

use crate::alien::{
    ACF_FIRSTFRAME, AFEXP, ASF_HITFLASH, ASF_INVISIBLE, ASF_PARTOBJ, ASF_SHADOW, ATGND, ATZREMOVE,
};
use crate::obj::Objects;
use crate::vars::{GF_NOZREMOVE, PFM_SHADOWS};
use sf_core::{dl_flags, DrawListEntry, MAX_DRAW_LIST};

// C al_flags placement bits (src/variables.h:70-73). Not in alien.rs yet;
// defined here additively for the draw pass.
pub const AF_INRNG_PL: u8 = 2;
pub const AF_LEFT_PL: u8 = 4;
pub const AF_FRONT_PL: u8 = 8;
pub const AF_INVIEW_PL: u8 = 16;

/// C `ASF4_NOPOLYEXP` (src/game/obj.h:117).
pub const ASF4_NOPOLYEXP: u8 = 0x04;

/// C `FP16_FROM_INT` (src/types.h:44).
#[inline]
fn fp16_from_int(i: i32) -> i32 {
    ((i as u32) << 16) as i32
}

/// C `Draw_BuildList()` (src/game/draw.c:26). Appends into `out`
/// (the caller clears it first, mirroring `Game_ClearDrawList`); entries
/// beyond MAX_DRAW_LIST are dropped exactly like `Game_SubmitDrawEntry`
/// (src/game/boot.c:295).
///
/// * `playerflymode` — C `g_playerflymode` (shadow gate, draw.c:145).
/// * `gameframe` — C `g_gameframe` (anim/col frame source, draw.c:119/126).
/// * `viewposx`/`viewposz` — the CAMERA position published by getview
///   (ROM `viewposx/z`, game.c:94-98) — NOT pviewpos (the player). The
///   camera sits behind the player by viewdist, so anchoring the cull on
///   pviewposz popped objects that were still on screen.
/// * `shape_extents` — per-shape AABB half-extents provider (same source
///   coldet uses, C `load_collision_extents`); `.2` supplies the ROM
///   `sh_zmax` cull margin. `None` (mesh unavailable) -> margin 0.
#[allow(clippy::too_many_arguments)]
pub fn build_list(
    objs: &mut Objects,
    playerflymode: u8,
    gameframe: u16,
    viewposx: i16,
    viewposz: i16,
    gameflags: u8,
    shape_extents: &dyn Fn(u16) -> Option<(i16, i16, i16)>,
    out: &mut Vec<DrawListEntry>,
) {
    let mut cur = objs.active_head;
    while let Some(i) = cur {
        let idx = i as usize;
        // Advance first; the walk body never mutates list links.
        cur = objs.aliens[idx].next;

        // --- Clear view placement flags (draw.c:51-55, MAIN.ASM:2118-2121):
        // al_flags &= ~(afinrngpl | affrontpl | afinviewpl | afleftpl);
        // al_flags |= affrontpl. marioshowview does this BEFORE the
        // invisible skip, so invisible aliens get the rewrite too.
        {
            let al = &mut objs.aliens[idx];
            al.flags &= !(AF_INRNG_PL | AF_FRONT_PL | AF_INVIEW_PL | AF_LEFT_PL);
            al.flags |= AF_FRONT_PL;
        }

        // --- Skip invisible aliens (draw.c:57-61) ---
        // ROM checks asf_invisible BEFORE the behind test (alienflags_l,
        // MAIN.ASM:2019-2021): invisible objects are never z-freed, keep the
        // flags written above, and consume no drawlist slot.
        if objs.aliens[idx].sflags & ASF_INVISIBLE != 0 {
            continue;
        }

        // --- Behind-camera cull (alienflags_l, MAIN.ASM:2024-2062) ---
        // ROM: behind iff sh_zmax(shape) + dl_z < 0, where dl_z is the
        // ROTATED camera-space z (world - viewpos through the view matrix).
        // The on-rails camera flies with yaw ~= 0, so unrotated
        // worldz - viewposz + zmax is equivalent; if a free-yaw camera ever
        // lands here this needs the rotated z like the ROM. The shape's own
        // z half-extent is the margin, so an object stays drawable until its
        // far edge passes the camera plane instead of popping at its origin.
        //
        // Behind objects first lose affrontpl (MAIN.ASM:2039-2041), then are
        // freed iff !gf_nozremove && !acf_firstframe && atzremove
        // (init_objvars sets atzremove by default, STRATROU.ASM:2311; the
        // free keeps the 70-slot pool from filling with passed scenery).
        // Behind survivors fall through: ROM's .dontkill still gives them
        // afinviewpl/afleftpl and their marioshowview drawlist slot.
        {
            let al = &objs.aliens[idx];
            let zmax = shape_extents(al.shape).map_or(0, |(_, _, z)| z);
            let rel_z = al.worldz.wrapping_sub(viewposz);
            if zmax.wrapping_add(rel_z) < 0 {
                objs.aliens[idx].flags &= !AF_FRONT_PL;
                let al = &objs.aliens[idx];
                if gameflags & GF_NOZREMOVE == 0
                    && al.collflags & ACF_FIRSTFRAME == 0
                    && al.type_ & ATZREMOVE != 0
                {
                    // ROM freed the alien after its slot was already emitted;
                    // we skip the entry instead (behind the near plane, the
                    // renderer would clip it anyway).
                    objs.free(i);
                    continue;
                }
            }
        }

        // --- View-side flags (alienflags_l .dontkill, MAIN.ASM:2065-2077):
        // every non-invisible survivor (in-front OR behind) gets afinviewpl,
        // plus afleftpl iff rotated view-space x < 0. Same yaw ~= 0 note as
        // the cull: worldx - viewposx stands in for the rotated dl_x.
        {
            let al = &mut objs.aliens[idx];
            al.flags |= AF_INVIEW_PL;
            if al.worldx.wrapping_sub(viewposx) < 0 {
                al.flags |= AF_LEFT_PL;
            }
        }

        // --- Skip if no shape (draw.c:63-67) ---
        if objs.aliens[idx].shape == 0 {
            continue;
        }

        let al = objs.aliens[idx];
        let mut entry = DrawListEntry::default();

        // Sort Z: ground objects sort behind everything (draw.c:73-74).
        entry.sort_z = if al.type_ & ATGND != 0 { 15000 } else { 0 };

        // World position as FP16.16 (draw.c:76-81).
        entry.x = fp16_from_int(al.worldx as i32);
        entry.y = fp16_from_int(al.worldy as i32);
        entry.z = fp16_from_int(al.worldz as i32);

        // Shape (draw.c:83-84).
        entry.shape_id = al.shape;

        // Rotation angles (draw.c:86-89).
        entry.rx = al.rotx as i16;
        entry.ry = al.roty as i16;
        entry.rz = al.rotz as i16;

        // Explosion state (draw.c:91-107).
        if al.flags & AFEXP != 0 {
            entry.explosion_cnt = if al.sflags4 & ASF4_NOPOLYEXP != 0 { 0 } else { al.count };
            // Particle objects store extra data in shadow fields.
            if al.sflags & ASF_PARTOBJ != 0 {
                entry.shad_y = i as i16; // alien index as unique ID
                entry.shad_x = al.sbyte1 as i16; // particle amount
                entry.shad_z = al.sbyte3 as i16; // sbyte2 = life, sbyte3 = type
            }
        } else {
            entry.explosion_cnt = 0;
        }

        // Strategy flags; hitflash is a one-frame effect masked off the
        // alien AFTER the copy (draw.c:109-112).
        entry.sflags = al.sflags;
        objs.aliens[idx].sflags &= !ASF_HITFLASH;

        // Animation frame: bit 7 clear -> follow gameframe; bit 7 set keeps
        // the stored frame (draw.c:114-120).
        entry.anim_frame = if (al.animframe as i8) < 0 {
            al.animframe & 127
        } else {
            (gameframe & 127) as u8
        };

        // Color animation frame (draw.c:122-127).
        entry.col_frame = if (al.colframe as i8) < 0 {
            al.colframe & 127
        } else {
            (gameframe & 127) as u8
        };

        // Depth offset (draw.c:129-130).
        entry.depth_offset = al.depthoffset as u8;

        // Texture scroll coordinates (draw.c:132-134).
        entry.tscroll_x = al.tx;
        entry.tscroll_y = al.ty;

        // Color table (draw.c:136-137).
        entry.color_table = al.coltab;

        // Mark as visible (draw.c:139-140).
        entry.flags = dl_flags::VISIBLE;

        // Drop shadow (draw.c:142-147).
        if al.sflags & ASF_SHADOW != 0 && playerflymode & PFM_SHADOWS != 0 {
            entry.flags |= dl_flags::SHADOW;
        }

        // Stable identity for render interpolation (draw.c:149-152).
        entry.obj_id = i + 1;

        // Game_SubmitDrawEntry cap (boot.c:295-299).
        if out.len() < MAX_DRAW_LIST {
            out.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::ASF_HITFLASH;

    /// A synthetic alien must produce exactly the DrawListEntry the C
    /// Draw_BuildList would emit, including the alien-side flag effects.
    #[test]
    fn synthetic_alien_exact_entry() {
        let mut objs = Objects::init();
        let idx = objs.alloc().unwrap();
        assert_eq!(idx, 0);
        {
            let al = &mut objs.aliens[idx as usize];
            al.shape = 42;
            al.type_ = ATGND;
            al.worldx = -10;
            al.worldy = 25;
            al.worldz = 300;
            al.rotx = 5;
            al.roty = 250; // u8 -> zero-extended to 250 (C (int16)al->rotx)
            al.rotz = 7;
            al.sflags = ASF_SHADOW | ASF_HITFLASH;
            al.animframe = 0x85; // bit 7 set: keep stored frame & 127 = 5
            al.colframe = 0x02; // bit 7 clear: follow gameframe
            al.depthoffset = 0x1234; // (uint8) truncation -> 0x34
            al.tx = 3;
            al.ty = 4;
            al.coltab = 0x0770;
            al.flags = AF_INRNG_PL | AF_LEFT_PL; // stale placement bits
        }

        let mut out = Vec::new();
        // playerflymode has PFM_SHADOWS; gameframe = 130 -> & 127 = 2;
        // camera viewposx = 0 > worldx -10 -> AF_LEFT_PL set.
        build_list(&mut objs, PFM_SHADOWS, 130, 0, 0, GF_NOZREMOVE, &|_| None, &mut out);

        assert_eq!(out.len(), 1);
        let e = &out[0];
        let expect = DrawListEntry {
            x: fp16_from_int(-10),
            y: fp16_from_int(25),
            z: fp16_from_int(300),
            rx: 5,
            ry: 250,
            rz: 7,
            shape_id: 42,
            color_table: 0x0770,
            sort_z: 15000,
            sflags: ASF_SHADOW | ASF_HITFLASH, // copy keeps hitflash
            explosion_cnt: 0,
            anim_frame: 5,
            col_frame: 2,
            depth_offset: 0x34,
            flags: dl_flags::VISIBLE | dl_flags::SHADOW,
            shad_x: 0,
            shad_y: 0,
            shad_z: 0,
            tscroll_x: 3,
            tscroll_y: 4,
            obj_id: 1,
        };
        assert_eq!(e, &expect);

        // Alien-side effects: hitflash cleared, placement flags rewritten
        // to AF_FRONT_PL | AF_INVIEW_PL | AF_LEFT_PL.
        let al = &objs.aliens[idx as usize];
        assert_eq!(al.sflags, ASF_SHADOW);
        assert_eq!(al.flags, AF_FRONT_PL | AF_INVIEW_PL | AF_LEFT_PL);
    }

    #[test]
    fn explosion_partobj_shadow_fields() {
        let mut objs = Objects::init();
        let a = objs.alloc().unwrap(); // slot 0
        let b = objs.alloc().unwrap(); // slot 1 (list head)
        assert_eq!((a, b), (0, 1));
        {
            let al = &mut objs.aliens[b as usize];
            al.shape = 9;
            al.flags = AFEXP;
            al.sflags = ASF_PARTOBJ;
            al.count = 12;
            al.sbyte1 = 34; // particle amount
            al.sbyte3 = 56; // type
        }
        objs.aliens[a as usize].shape = 0; // skipped: no shape

        let mut out = Vec::new();
        build_list(&mut objs, 0, 0, 0, 0, GF_NOZREMOVE, &|_| None, &mut out);
        // Active list is newest-first: only slot 1 emitted.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].explosion_cnt, 12);
        assert_eq!(out[0].shad_y, 1); // alien index
        assert_eq!(out[0].shad_x, 34);
        assert_eq!(out[0].shad_z, 56);
        assert_eq!(out[0].obj_id, 2);

        // ASF4_NOPOLYEXP suppresses the face-explosion count.
        objs.aliens[b as usize].sflags4 = ASF4_NOPOLYEXP;
        objs.aliens[b as usize].flags = AFEXP; // rebuild cleared placement bits
        out.clear();
        build_list(&mut objs, 0, 0, 0, 0, GF_NOZREMOVE, &|_| None, &mut out);
        assert_eq!(out[0].explosion_cnt, 0);
    }
}
