//! Title-screen demo Arwing regression tests.
//!
//! Bug report: "leave the intro running and the ship spins ever faster".
//! Root cause was two coupled port bugs:
//! 1. `strat_title_tick` spun `al_roty += 1` from a zero pose; the ROM
//!    `tit_strat` (ENDSEQ.ASM:1805-1809) rolls `al_rotz += 2` from a tilted
//!    init pose (`tit_istrat`, ENDSEQ.ASM:1799-1804).
//! 2. The camera treated the title demo ship as the player and tracked its
//!    rotation. The retail title instead has an invisible passive player:
//!    its view advances at a fixed rate while its orientation stays zero.
//!    Tracking the demo ship yawed the camera until behind-camera culling
//!    removed it.
//!
//! These tests drive the Shell headlessly (no SDL) and pin the ROM
//! behavior: the ship persists for the complete authored title hold, rolls Z
//! at a constant 2/tick, keeps the tilted pose, and never rotates the camera.

use sf_core::pad;
use sf_game::shell::{GameState, Shell, INTRO_INPUT_DELAY_TICKS, TITLE_ATTRACT_DURATION_TICKS};

const TITLE_SHIP_SHAPE: u16 = 225;

fn make_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, newmap| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, newmap);
    }));
    shell
}

/// Collect (slot, rotx, roty, rotz) of every active object.
fn active_objs(shell: &Shell) -> Vec<(u16, u8, u8, u8)> {
    let mut out = Vec::new();
    let mut cur = shell.game.objs.active_head;
    while let Some(i) = cur {
        let al = &shell.game.objs.aliens[i as usize];
        if al.shape == TITLE_SHIP_SHAPE {
            out.push((i, al.rotx, al.roty, al.rotz));
        }
        cur = al.next;
    }
    out
}

fn advance_to_title(shell: &mut Shell) {
    shell.tick(0);
    shell.tick(0);
    while shell.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
        shell.tick(pad::A);
    }
    while shell.state() != GameState::Title {
        shell.tick(0);
    }
    shell.tick(0);
}

#[test]
fn title_ship_rolls_rom_faithfully_for_the_authored_hold() {
    let mut shell = make_shell();
    advance_to_title(&mut shell);

    // Title load + mapwait(800): the demo ship spawns around tick 30.
    for _ in 0..40 {
        shell.tick(0);
    }
    let start = active_objs(&shell);
    assert_eq!(
        start.len(),
        1,
        "title map spawns exactly one demo ship (frame {}, map pointer {}, map count {}, loaded {:?})",
        shell.game.vars.gameframe,
        shell.game.vars.mapptr,
        shell.game.vars.mapcnt,
        shell.game.world.loaded_map_id,
    );
    let (_, rotx0, roty0, rotz0) = start[0];

    // tit_istrat display pose. The ROM bytes (ENDSEQ.ASM:1800-1802) are
    // rotx=-17 (0xEF), roty=96, but they are calibrated for the ROM's title
    // view. The source passive player advances camera depth while keeping
    // outvx/outvy at zero, so the port's title camera orientation is fixed.
    // Under that orientation the certified ZXY model matrix renders the ship
    // broadside, and the ROM roll (rotz) collapses it edge-on to a vertical
    // spike. strat_title_init rotates the display pose so the roll axis points
    // toward the camera and remains a solid 3/4 barrel roll. rotz still
    // advances 2 per tick, verbatim from the ROM tit_strat.
    assert_eq!(rotx0, 48, "display pitch (nose toward fixed title camera)");
    assert_eq!(roty0, 32, "diagonal 3/4 display yaw");

    // The ship must neither vanish (old behind-cull free at ~tick 99) nor
    // change its per-tick roll rate before ENDSEQ's title timeout.
    let mut prev_rotz = rotz0;
    let remaining_title_ticks =
        TITLE_ATTRACT_DURATION_TICKS.saturating_sub(shell.game.vars.gameframe + 1);
    for t in 0..remaining_title_ticks {
        shell.tick(0);
        assert_eq!(shell.state(), GameState::Title);
        let objs = active_objs(&shell);
        assert_eq!(objs.len(), 1, "demo ship vanished at +{t} ticks");
        let (_, rotx, roty, rotz) = objs[0];
        assert_eq!(rotx, 48, "pitch must stay fixed");
        assert_eq!(roty, 32, "yaw must stay fixed (ROM rolls Z, not Y)");
        assert_eq!(
            rotz.wrapping_sub(prev_rotz),
            2,
            "tit_strat rolls exactly +2/tick (ENDSEQ.ASM:1807), no faster"
        );
        prev_rotz = rotz;

        // The ship must actually be submitted for drawing every tick.
        assert!(
            !shell.draw_list().is_empty(),
            "demo ship dropped from the draw list at +{t} ticks"
        );
    }
}

#[test]
fn title_camera_keeps_the_retail_fixed_orientation() {
    let mut shell = make_shell();
    advance_to_title(&mut shell);
    for _ in 0..40 {
        shell.tick(0);
    }
    let cam0 = shell.frame().camera;
    // The passive presentation player keeps outvx/outvy at zero, so the
    // camera must not rotate with the demo ship. Its depth still advances by
    // the fixed playercred view step.
    assert_eq!((cam0.rx, cam0.ry, cam0.rz), (0, 0, 0));
    let remaining_title_ticks =
        TITLE_ATTRACT_DURATION_TICKS.saturating_sub(shell.game.vars.gameframe + 1);
    for _ in 0..remaining_title_ticks {
        shell.tick(0);
        let cam = shell.frame().camera;
        assert_eq!(
            (cam.x, cam.y, cam.rx, cam.ry, cam.rz),
            (cam0.x, cam0.y, 0, 0, 0),
            "title camera orientation must stay fixed while the demo ship rolls"
        );
    }
}
