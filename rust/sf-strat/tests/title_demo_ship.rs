//! Title-screen demo Arwing regression tests.
//!
//! Bug report: "leave the intro running and the ship spins ever faster".
//! Root cause was two coupled port bugs:
//! 1. `strat_title_tick` spun `al_roty += 1` from a zero pose; the ROM
//!    `tit_strat` (ENDSEQ.ASM:1805-1809) rolls `al_rotz += 2` from a tilted
//!    init pose (`tit_istrat`, ENDSEQ.ASM:1799-1804).
//! 2. The camera treated the title demo ship (slot 0, Obj_GetPlayer
//!    fallback) as the player and tracked its rot; the ROM `getview_l`
//!    (GAME.ASM:42-47) drives view rotation from the outvx/outvy
//!    accumulators (zero at the title), so its title camera is static.
//!    The tracking camera yawed with the ship until the behind-camera cull
//!    freed it (~5 s in), leaving a bare title screen.
//!
//! These tests drive the Shell headlessly (no SDL) and pin the ROM
//! behavior: ship persists indefinitely, rolls Z at a constant 2/tick,
//! keeps the tilted pose, and the camera never moves.

use sf_game::shell::Shell;

fn make_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, newmap| {
        if let Some(idx) = sf_strat::player::strat_spawn_player(game) {
            if newmap == sf_map::catalog::map_id::M1_1
                || newmap == sf_map::catalog::map_id::M2_1
                || newmap == sf_map::catalog::map_id::M3_1
            {
                sf_strat::player::strat_player_opening_init(game, idx);
            }
        }
    }));
    shell
}

/// Collect (slot, rotx, roty, rotz) of every active object.
fn active_objs(shell: &Shell) -> Vec<(u16, u8, u8, u8)> {
    let mut out = Vec::new();
    let mut cur = shell.game.objs.active_head;
    while let Some(i) = cur {
        let al = &shell.game.objs.aliens[i as usize];
        out.push((i, al.rotx, al.roty, al.rotz));
        cur = al.next;
    }
    out
}

#[test]
fn title_ship_rolls_rom_faithfully_and_persists() {
    let mut shell = make_shell();

    // Boot + title load + mapwait(800): the demo ship spawns around tick 30.
    for _ in 0..40 {
        shell.tick(0);
    }
    let start = active_objs(&shell);
    assert_eq!(start.len(), 1, "title map spawns exactly one demo ship");
    let (_, rotx0, roty0, rotz0) = start[0];

    // tit_istrat display pose. The ROM bytes (ENDSEQ.ASM:1800-1802) are
    // rotx=-17 (0xEF), roty=96, but they are calibrated for the ROM's title
    // view. The port's title camera is pinned static at (0,0,0) (no player
    // object -> getview_l accumulators zero; see title_camera_is_static
    // below), and under that camera the certified ZXY model matrix renders
    // the ship broadside, so the ROM roll (rotz) collapses it edge-on to a
    // vertical spike. strat_title_init rotates the display pose ~90 deg so
    // the roll axis points at the camera and the roll stays a solid 3/4
    // barrel roll (see strat_title_init doc comment). rotz still advances 2
    // per tick, verbatim from the ROM tit_strat.
    assert_eq!(rotx0, 48, "display pitch (nose toward static title camera)");
    assert_eq!(roty0, 32, "diagonal 3/4 display yaw");

    // Leave the "intro" running a long time (5000 ticks = 250 s of game
    // time). The ship must neither vanish (old behind-cull free at ~tick
    // 99) nor change its per-tick roll rate.
    let mut prev_rotz = rotz0;
    for t in 0..5000u32 {
        shell.tick(0);
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
fn title_camera_is_static() {
    let mut shell = make_shell();
    for _ in 0..40 {
        shell.tick(0);
    }
    let cam0 = shell.frame().camera;
    // ROM getview_l at the title: outvx/outvy = 0 -> view rotation zero,
    // camera (0,0,-outdist). It must not rotate with the demo ship.
    assert_eq!((cam0.rx, cam0.ry, cam0.rz), (0, 0, 0));
    for _ in 0..1000 {
        shell.tick(0);
        let cam = shell.frame().camera;
        assert_eq!(
            (cam.x, cam.y, cam.z, cam.rx, cam.ry, cam.rz),
            (cam0.x, cam0.y, cam0.z, 0, 0, 0),
            "title camera must stay fixed while the demo ship rolls"
        );
    }
}
