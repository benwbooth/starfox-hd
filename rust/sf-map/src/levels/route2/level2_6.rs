//! MAP_ID_2_6 — Venom 2 Highway and the Route-2 Andross finale.
//!
//! Direct transcription of LEVEL2_6.ASM, MAP2_6A.ASM, CL_COLON.ASM,
//! TRUCKER.ASM, and the shared FINALMAP.ASM tail.

use super::rc::*;
use super::submaps::{self, TruckerPtrs};
use super::Route2Level;
use crate::builder::MapBuilder;
use crate::consts::{al, cb, wm, DEG180, DEG360, MEDPSPEED};

struct Map26Ptrs {
    bonus0: u16,
    bonus1: u16,
    trucker: TruckerPtrs,
}

/// MAP2_6A.ASM — the complete colony highway, including every traffic and
/// obstacle placement before the Mad Trucker encounter.
fn append_map2_6a(b: &mut MapBuilder) -> Map26Ptrs {
    b.label("level2_6.map2_6a");

    b.mapobj(800, -100, -60, 1800, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(0, 10, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(800, -100, -60, 2600, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, -200, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -100, -60, 3200, SH_WALL_4, IS_HARD180YR);
    b.special(0, -200, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(800, -100, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);

    b.skillfly_init();
    b.skillfly_set_default(-150, -60, 4000);
    b.mapobj(0, -200, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);

    b.mapobj(0, 80, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1600, 20, -100, 3500, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(800, -100, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 80, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1600, -20, -40, 3500, SH_CAR_0, IS_AIRCAR4);

    b.skillfly_set_default(-150, -60, 4000);
    b.mapobj(0, -200, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 80, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1600, -1000, 60, 3500, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(0, 80, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(0, -1000, 60, 2000, SH_CAR_0, IS_AIRCAR1);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.skillfly_set_default(-150, -60, 4000);
    b.mapobj(0, -200, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 80, -60, 4000, SH_BOU_1B, IS_HARD180YR);

    b.cspecial(1800, -1000, 60, 3500, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(800, -90, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.cspecial(0, -800, 60, 1000, SH_CAR_0, IS_AIRCAR1);
    b.mapobj(800, -80, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -70, -60, 4000, SH_WALL_4, IS_HARD180YR);
    let bonus0 = b.mapcode65816_inline();
    b.mapobj(0, 60, -50, 1500, SH_ITEM_7, IS_ITEM7);
    b.label("level2_6.map2_6a.skillfly_bonus_0_skip");
    b.mapobj(800, -60, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -50, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -40, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, 60, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(800, -60, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(1200, 60, -60, 4000, SH_WALL_4, IS_HARD180YR);

    b.cspecial(1600, -1000, 60, 3000, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(800, -60, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.cspecial(1600, 0, -60, 3500, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(800, 60, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(1000, -60, -60, 4000, SH_WALL_4, IS_HARD180YR);

    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1500, -1000, 60, 3000, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.special(1500, -1000, 60, 3000, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, 0, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, 0, -40, 3000, SH_CAR_1, IS_TRUCK2);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, 50, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.special(1000, -120, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, 20, -40, 3000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, -50, -40, 3000, SH_CAR_1, IS_TRUCK2);
    for _ in 0..3 {
        b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    }

    b.cspecial(0, -200, -40, 4000, SH_CAR_1, IS_TRUCK1);
    b.mapobj(0, 0, -110, 4000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(200, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(400, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 80, -40, 4000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, 0, -110, 4000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(200, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(400, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4200, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -200, -110, 4000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(0, 0, -110, 4000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(200, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(400, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.cspecial(0, -200, -40, 4000, SH_CAR_1, IS_TRUCK1);
    b.pathobj(0, 0, -110, 4000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.mapobj(200, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(400, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 0, -110, 4000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(200, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.skillfly_set_default(-180, -60, 4000);
    b.mapobj(400, -100, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, 0, -60, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(2000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    b.label("level2_6.map2_6a.walls");
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.maploop("level2_6.map2_6a.walls", 4);
    b.cspecial(0, 0, -10, 4000, SH_CAR_0, IS_AIRCAR4);
    let bonus1 = b.mapcode65816_inline();
    b.mapobj(0, 0, -50, 1000, SH_ITEM_5, IS_ITEM5);
    b.label("level2_6.map2_6a.skillfly_bonus_1_skip");
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, 0, -10, 4000, SH_CAR_0, IS_AIRCAR4);
    b.cspecial(1500, -1000, 60, 4000, SH_CAR_0, IS_AIRCAR4);

    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, -1000, -90, 200, SH_CAR_0, IS_AIRCAR3);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, -1000, -90, 200, SH_CAR_0, IS_AIRCAR2);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(1000, -500, -90, 500, SH_CAR_0, IS_AIRCAR5);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.label("level2_6.map2_6a.walls2");
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.cspecial(1000, -1000, -90, 200, SH_CAR_0, IS_AIRCAR2);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.maploop("level2_6.map2_6a.walls2", 2);
    b.cspecial(0, -1000, 60, 1400, SH_CAR_0, IS_AIRCAR1);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(1000, -500, -90, 500, SH_CAR_0, IS_AIRCAR5);
    b.mapobj(1000, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(0, -300, -60, 4000, SH_BOU_1B, IS_HARD180YR);
    b.mapobj(1000, -500, -90, 500, SH_CAR_0, IS_AIRCAR5);
    b.special(1000, 20, -100, 3500, SH_CAR_0, IS_AIRCAR4);
    b.mapobj(1000, -1000, -90, 200, SH_CAR_0, IS_AIRCAR2);
    b.special(1000, -1000, 60, 1000, SH_CAR_0, IS_AIRCAR1);
    b.mapobj(5000, -500, -90, 500, SH_CAR_0, IS_AIRCAR5);
    b.mapobj(0, 80, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, -40, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, -160, -60, 4000, SH_WALL_4, IS_HARD180YR);
    b.mapobj(0, -280, -60, 4000, SH_WALL_4, IS_HARD180YR);

    // TRUCKER.ASM is textually included here; its `.continue` falls through
    // into MAP2_6A's victory sequence.
    let trucker = submaps::append_trucker_submap(b);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 15 * 2);
    b.setbgm(0x12);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.setvarb24(wm::M_METERS, 1);
    b.maprts();

    Map26Ptrs {
        bonus0,
        bonus1,
        trucker,
    }
}

fn pipe_shape(kind: u8) -> u16 {
    match kind {
        0 => SH_PIPE_0,
        1 => SH_PIPE_1,
        2 => SH_PIPE_2,
        3 => SH_PIPE_3,
        4 => SH_PIPE_4,
        5 => SH_PIPE_5,
        6 => SH_PIPE_6,
        _ => panic!("invalid CL_COLON pipe kind {kind}"),
    }
}

fn colony_pipe(
    b: &mut MapBuilder,
    pdist: i32,
    y_units: i32,
    z_units: i32,
    pitch_steps: i32,
    flip: i32,
    kind: u8,
) {
    const PIPE_SCALE: i32 = 16;
    b.mapobj(
        0,
        0,
        -60 + y_units * PIPE_SCALE,
        pdist + z_units * PIPE_SCALE,
        pipe_shape(kind),
        IS_GND,
    );
    b.setalvarb(al::ROTX, (DEG360 / 12) * pitch_steps);
    b.setalvarb(al::ROTZ, DEG180 * flip);
}

/// CL_COLON.ASM — complete colony-clear pipe fly-through.
fn append_cl_colon(b: &mut MapBuilder) {
    const PIPE_WAIT: i32 = 40 * 16;
    let mut pdist = 960 + 20 * 16;
    let pipe_wait = |b: &mut MapBuilder, pdist: &mut i32| {
        b.mapwait(PIPE_WAIT);
        *pdist -= PIPE_WAIT;
    };

    b.mapplayercantdie();
    b.mapif_builtin(cb::IS_PLAYER_DEAD, "level2_6.cl_colon.after_tocslow");
    b.mapcodejsl_builtin(cb::SET_PLAYER_TOCSLOW_L);
    b.label("level2_6.cl_colon.after_tocslow");
    b.mapobj(400, 0, -60, 4200, SH_PIPE_8_0, IS_NOCOLL);
    b.mapobj(800, 0, -60, 4200, SH_PIPE_8_0, IS_NOCOLL);
    b.mapobj(0, 0, -60, 4200, SH_PIPE_8, IS_COLONYEXIT);
    b.mapwait(4000);

    b.setbg(BG_2_6B);
    b.initbg();
    // BGS.ASM bg_2_6b_1 owns this pstrat; the Rust background VM exposes it
    // through the same callback bridge used for WASHENT.
    b.mapcodejsl_builtin(cb::SET_PLAYER_CLEAR_COLONY_L);

    colony_pipe(b, pdist, 0, 0, 0, 0, 0);
    colony_pipe(b, pdist, -11, 40, -1, 0, 2);
    colony_pipe(b, pdist, -40, 70, -2, 1, 2);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, -69, 100, -1, 0, 3);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, -80, 140, 0, 1, 0);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, -69, 180, 1, 0, 3);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, -40, 210, 2, 1, 2);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 0, 221, 3, 0, 5);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 40, 221, 3, 0, 4);
    colony_pipe(b, pdist, 80, 232, 2, 0, 2);
    colony_pipe(b, pdist, 109, 262, 1, 1, 3);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 120, 302, 0, 0, 0);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 109, 342, -1, 1, 3);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 80, 371, -2, 0, 2);
    pipe_wait(b, &mut pdist);
    colony_pipe(b, pdist, 69, 382, -3, 0, 3);
    colony_pipe(b, pdist, 40, 393, -2, 1, 2);
    colony_pipe(b, pdist, 11, 423, -1, 1, 2);
    colony_pipe(b, pdist, 0, 463, 0, 0, 0);
    pipe_wait(b, &mut pdist);

    for (z_units, rotz) in [(533, 0), (633, -12), (733, -25), (833, -42), (933, -56)] {
        colony_pipe(b, pdist, 0, z_units, 0, 0, 6);
        b.setalvarb(al::ROTZ, rotz);
        if z_units != 933 {
            pipe_wait(b, &mut pdist);
        }
    }
}

pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    // LEVEL2_6.ASM wrapper.
    b.mapjsr("level2_6.map2_6a");
    b.mapwait(4000);
    append_cl_colon(&mut b);
    b.mapwait(2000);
    b.mapgoto("level2_6.final.tunnel");

    let map26 = append_map2_6a(&mut b);
    let (final_cantdie, final_cleanup) =
        crate::levels::route3::common::append_finalmap_content(&mut b, "level2_6.final", 2);

    b.resolve();
    let (data, labels) = b.finish();

    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (map26.bonus0, "level2_6_skillfly_bonus0_guard"),
            (map26.bonus1, "level2_6_skillfly_bonus1_guard"),
            (map26.trucker.biker_check, "trucker_biker_check"),
            (map26.trucker.approach_sound, "level1_1_mapwaitboss_trigse"),
            (map26.trucker.trigger, "trucker_trigger_check"),
            (final_cantdie, "level1_1_mapwaitboss_cantdie"),
            (final_cleanup, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_contains_highway_colony_and_finale() {
        let level = build();
        for wanted in [
            "level2_6.map2_6a",
            "level2_6.map2_6a.walls",
            "level2_6.trucker.loop",
            "level2_6.final.tunnel",
            "level2_6.final.bosswait.loop",
        ] {
            assert!(
                level.level.labels.iter().any(|(name, _)| name == wanted),
                "missing {wanted}"
            );
        }
        assert_eq!(level.inline.len(), 7);
    }
}
