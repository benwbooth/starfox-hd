//! End-of-game / tally-screen text object spawns (ROM MAIN.ASM makeendobj*).
//!
//! Spawns a `zaco_4` path-text object at the player (or view) position plus
//! an XY offset, with a message pointer in `al_coltab` and colour in
//! `al_depthoffset`. Digit helpers (`makeendobjn` / `makenumt`) map 0..=9 to
//! the `msg_0` table base before calling the spawn body.

use sf_game::alien::{ASF4_TEXTOBJ, ASF_COLLDISABLE};
use sf_game::score;
use sf_game::shell::EndingScorePart;
use sf_game::Game;
use sf_path::ids::{
    PATH_ID_AVE, PATH_ID_AVEN, PATH_ID_FADEINTOTAL, PATH_ID_STAGE1, PATH_ID_STAGE2, PATH_ID_STAGE3,
    PATH_ID_STAGE4, PATH_ID_STAGE5, PATH_ID_STAGE6, PATH_ID_STAGE7, PATH_ID_TOTAL, PATH_ID_TOTALN,
};

use crate::common::strat_make_obj;

/// `al_sflags3` textobj (C ASF3_TEXTOBJ / path_adapter) — same bit as
/// [`sf_game::alien::ASF3_LOCKON`] in this port's remapping.
const ASF3_TEXTOBJ: u8 = 0x40;

/// Shape id for tally text carriers (ROM `#zaco_4`).
pub const SH_ZACO_4: u16 = 105;

/// ROM `msg_0` table base — digit N uses `msg_0 + 2*N` (word index into
/// the MARIO text bank). HD stores the digit index in `coltab` low byte
/// with high byte marking a digit message (`0x4D00 | digit`).
pub const MSG_DIGIT_TAG: u16 = 0x4D00;
pub const MSG_TAG_MASK: u16 = 0xFF00;
/// Semantic message tags consumed by the HD 3D text renderer. These keep the
/// shipping port independent of source data addresses while retaining the
/// original path-text object representation.
pub const MSG_PERCENT_TAG: u16 = 0x4E00;
pub const MSG_TOTAL_LABEL_TAG: u16 = 0x4F00;
pub const MSG_AVERAGE_LABEL_TAG: u16 = 0x4F01;
pub const MSG_STAGE_LABEL_TAG: u16 = 0x4F02;

/// Colour / type constants from MAIN.ASM makeendobj variants.
pub const END_OBJ_COLOUR: u8 = 3; // dark yellow
pub const END_OBJ_COLOUR2: u8 = 5; // dark blue
pub const END_OBJ_TY: u8 = 3;
pub const NUM_OBJ_COLOUR: u8 = 14; // white
pub const NUM_OBJ_TY: u8 = 7;
/// Z offset for makeendobj / makenum (MAIN.ASM).
pub const END_OBJ_Z: i16 = 4000;
/// Z offset for makeendobj2 (`totalzpos` equ 3600).
pub const END_OBJ2_Z: i16 = 3600;
/// Y bump for makenum (MAIN.ASM:950).
pub const NUM_OBJ_Y: i16 = 1100;
/// X step after each makenum digit (MAIN.ASM:956 — cla1 += 150).
pub const NUM_OBJ_X_STEP: i16 = 150;
const PATH_TEXT_STRATEGY_INDEX: usize = 228;
const FINAL_SCORE_X: i16 = 500;
const FINAL_SCORE_Y: i16 = 750;
const FINAL_SCORE_DIGIT_Y: i16 = FINAL_SCORE_Y + 150;
const FINAL_SCORE_AVERAGE_LABEL_Y: i16 = FINAL_SCORE_Y + 300;
const FINAL_SCORE_AVERAGE_VALUE_Y: i16 = FINAL_SCORE_Y + 450;
const PARADE_STAGE_LABEL_X: i16 = -500;
const PARADE_STAGE_NUMBER_X: i16 = 0;
const PARADE_STAGE_SCORE_X: i16 = 600;
const PARADE_STAGE_FIRST_Y: i16 = -550;
const PARADE_STAGE_ROW_STEP: i16 = 200;
const PARADE_TOTAL_LABEL_Y: i16 = -1_600;
const PARADE_TOTAL_VALUE_Y: i16 = -1_400;
const PARADE_AVERAGE_LABEL_Y: i16 = -850;
const PARADE_AVERAGE_VALUE_Y: i16 = -650;

/// Parameters for a tally text spawn.
#[derive(Debug, Clone, Copy)]
pub struct EndObjSpawn {
    pub msg: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub offset_z: i16,
    pub colour: u8,
    pub ty: u8,
    pub path_id: u16,
    /// When true, copy from view/player; when false, player only (makeendobj2).
    pub from_player: bool,
}

/// ROM `makenumt` digit → message word: `asl; adc #msg_0`.
pub fn makenumt_msg(digit: u16) -> u16 {
    MSG_DIGIT_TAG | ((digit & 0xFF) << 1)
}

/// ROM `checkifiamend` (MAIN.ASM:630): if `a == specptr` (stage count),
/// arm `c_type = 30` so the next stagescore pass runs the total-score
/// sequence. Returns the (possibly updated) controller-type latch.
pub fn check_if_i_am_end(stage_index: u16, stage_count: u16, c_type: &mut u8) -> bool {
    if stage_index == stage_count {
        *c_type = 30;
        true
    } else {
        false
    }
}

fn spawn_text_obj(g: &mut Game, sp: EndObjSpawn) -> Option<u16> {
    let idx = strat_make_obj(g, SH_ZACO_4)?;
    // Anchor: player if present, else origin (viewpt / playpt on ROM).
    let (bx, by, bz) = g
        .objs
        .player()
        .map(|p| (p.worldx, p.worldy, p.worldz))
        .unwrap_or((0, 0, 0));
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.sflags3 |= ASF3_TEXTOBJ;
        al.sflags4 |= ASF4_TEXTOBJ;
        al.depthoffset = sp.colour as i16;
        al.coltab = sp.msg;
        al.ty = sp.ty;
        al.tx = 0;
        al.sword2 = sp.path_id as i16; // path catalog id until resolve
        al.worldx = bx.wrapping_add(sp.offset_x);
        al.worldy = by.wrapping_add(sp.offset_y);
        al.worldz = bz.wrapping_add(sp.offset_z);
        al.hp = 10;
        al.ap = 8;
    }
    // Resolve path start if hooks know the catalog.
    let start = g.hooks.resolve_path_start(sp.path_id);
    if start != 0 {
        g.objs.aliens[idx as usize].sword2 = start as i16;
    }
    if let Some(path_text) = g
        .world
        .istrats
        .get(PATH_TEXT_STRATEGY_INDEX)
        .copied()
        .flatten()
    {
        g.objs.aliens[idx as usize].stratptr = Some(path_text);
    }
    let _ = sp.from_player;
    Some(idx)
}

/// ROM `makeendobj` (MAIN.ASM:965) — colour 3, path `total`, Z+4000.
pub fn makeendobj(g: &mut Game, msg: u16, ox: i16, oy: i16) -> Option<u16> {
    spawn_text_obj(
        g,
        EndObjSpawn {
            msg,
            offset_x: ox,
            offset_y: oy,
            offset_z: END_OBJ_Z,
            colour: END_OBJ_COLOUR,
            ty: END_OBJ_TY,
            path_id: PATH_ID_TOTAL,
            from_player: true,
        },
    )
}

fn makeendobj_on_path(g: &mut Game, msg: u16, ox: i16, oy: i16, path_id: u16) -> Option<u16> {
    spawn_text_obj(
        g,
        EndObjSpawn {
            msg,
            offset_x: ox,
            offset_y: oy,
            offset_z: END_OBJ_Z,
            colour: END_OBJ_COLOUR,
            ty: END_OBJ_TY,
            path_id,
            from_player: true,
        },
    )
}

fn stage_path(row: u8) -> u16 {
    match row {
        0 => PATH_ID_STAGE1,
        1 => PATH_ID_STAGE2,
        2 => PATH_ID_STAGE3,
        3 => PATH_ID_STAGE4,
        4 => PATH_ID_STAGE5,
        5 => PATH_ID_STAGE6,
        _ => PATH_ID_STAGE7,
    }
}

fn spawn_score_digits_on_path(g: &mut Game, total_score: u16, y: i16, path_id: u16) {
    let (hundreds, tens, ones) = score::score_digits(total_score);
    if hundreds != 0 {
        let _ = makeendobj_on_path(g, makenumt_msg(hundreds), -300, y, path_id);
    }
    let _ = makeendobj_on_path(g, makenumt_msg(tens), -150, y, path_id);
    let _ = makeendobj_on_path(g, makenumt_msg(ones), 0, y, path_id);
    let _ = makeendobj_on_path(g, makenumt_msg(0), 150, y, path_id);
    let _ = makeendobj_on_path(g, makenumt_msg(0), 300, y, path_id);
}

/// ROM `makeendobjn` — digit then [`makeendobj`].
pub fn makeendobjn(g: &mut Game, digit: u16, ox: i16, oy: i16) -> Option<u16> {
    makeendobj(g, makenumt_msg(digit), ox, oy)
}

/// ROM `makeendobj2` (MAIN.ASM:996) — colour 5, path `fadeintotal`, Z+totalzpos.
pub fn makeendobj2(g: &mut Game, msg: u16, ox: i16, oy: i16) -> Option<u16> {
    spawn_text_obj(
        g,
        EndObjSpawn {
            msg,
            offset_x: ox,
            offset_y: oy,
            offset_z: END_OBJ2_Z,
            colour: END_OBJ_COLOUR2,
            ty: END_OBJ_TY,
            path_id: PATH_ID_FADEINTOTAL,
            from_player: true,
        },
    )
}

/// ROM `makeendobjn2` — digit then [`makeendobj2`].
pub fn makeendobjn2(g: &mut Game, digit: u16, ox: i16, oy: i16) -> Option<u16> {
    makeendobj2(g, makenumt_msg(digit), ox, oy)
}

/// ROM `makenump` (MAIN.ASM:928) — white digit on `total` path; advances
/// `cla1` (current X) by 150. Returns `(obj, new_cla1)`.
pub fn makenump(g: &mut Game, msg: u16, cla1: i16) -> (Option<u16>, i16) {
    let idx = spawn_text_obj(
        g,
        EndObjSpawn {
            msg,
            offset_x: cla1,
            offset_y: NUM_OBJ_Y,
            offset_z: END_OBJ_Z,
            colour: NUM_OBJ_COLOUR,
            ty: NUM_OBJ_TY,
            path_id: PATH_ID_TOTAL,
            from_player: true,
        },
    );
    (idx, cla1.wrapping_add(NUM_OBJ_X_STEP))
}

/// ROM `makenumt` — digit then [`makenump`].
pub fn makenumt(g: &mut Game, digit: u16, cla1: i16) -> (Option<u16>, i16) {
    makenump(g, makenumt_msg(digit), cla1)
}

/// Emit one timed part of the source `maketotalscore2` presentation.
pub fn spawn_final_score_part(
    g: &mut Game,
    part: EndingScorePart,
    total_score: u16,
    average_score: u16,
) {
    match part {
        EndingScorePart::StageScore {
            stage_number,
            score,
            row,
        } => {
            let path = stage_path(row);
            let y = PARADE_STAGE_FIRST_Y
                .wrapping_add(PARADE_STAGE_ROW_STEP.wrapping_mul(i16::from(row)));
            let _ = makeendobj_on_path(g, MSG_STAGE_LABEL_TAG, PARADE_STAGE_LABEL_X, y, path);
            let _ = makeendobj_on_path(
                g,
                makenumt_msg(u16::from(stage_number)),
                PARADE_STAGE_NUMBER_X,
                y,
                path,
            );
            let _ = makeendobj_on_path(
                g,
                MSG_PERCENT_TAG | u16::from(score),
                PARADE_STAGE_SCORE_X,
                y,
                path,
            );
        }
        EndingScorePart::ParadeTotalLabel => {
            let _ = makeendobj_on_path(
                g,
                MSG_TOTAL_LABEL_TAG,
                0,
                PARADE_TOTAL_LABEL_Y,
                PATH_ID_TOTAL,
            );
        }
        EndingScorePart::ParadeTotalValue => {
            spawn_score_digits_on_path(g, total_score, PARADE_TOTAL_VALUE_Y, PATH_ID_TOTALN);
        }
        EndingScorePart::ParadeAverageLabel => {
            let _ = makeendobj_on_path(
                g,
                MSG_AVERAGE_LABEL_TAG,
                0,
                PARADE_AVERAGE_LABEL_Y,
                PATH_ID_AVE,
            );
        }
        EndingScorePart::ParadeAverageValue => {
            let _ = makeendobj_on_path(
                g,
                MSG_PERCENT_TAG | average_score.min(100),
                0,
                PARADE_AVERAGE_VALUE_Y,
                PATH_ID_AVEN,
            );
        }
        EndingScorePart::TotalLabel => {
            let _ = makeendobj2(g, MSG_TOTAL_LABEL_TAG, FINAL_SCORE_X, FINAL_SCORE_Y);
        }
        EndingScorePart::TotalValue => {
            let (hundreds, tens, ones) = score::score_digits(total_score);
            if hundreds != 0 {
                let _ = makeendobjn2(g, hundreds, FINAL_SCORE_X - 300, FINAL_SCORE_DIGIT_Y);
            }
            let _ = makeendobjn2(g, tens, FINAL_SCORE_X - 150, FINAL_SCORE_DIGIT_Y);
            let _ = makeendobjn2(g, ones, FINAL_SCORE_X, FINAL_SCORE_DIGIT_Y);
            let _ = makeendobjn2(g, 0, FINAL_SCORE_X + 150, FINAL_SCORE_DIGIT_Y);
            let _ = makeendobjn2(g, 0, FINAL_SCORE_X + 300, FINAL_SCORE_DIGIT_Y);
        }
        EndingScorePart::AverageLabel => {
            let _ = makeendobj2(
                g,
                MSG_AVERAGE_LABEL_TAG,
                FINAL_SCORE_X,
                FINAL_SCORE_AVERAGE_LABEL_Y,
            );
        }
        EndingScorePart::AverageValue => {
            let message = MSG_PERCENT_TAG | average_score.min(100);
            let _ = makeendobj2(g, message, FINAL_SCORE_X, FINAL_SCORE_AVERAGE_VALUE_Y);
        }
    }
}
