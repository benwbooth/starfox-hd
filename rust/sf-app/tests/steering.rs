//! Windowless steering-direction test. Drives the game logic (no wgpu/window)
//! through the opening into on-planet flight, holds LEFT, and checks which way
//! the player's `worldx` moves. From the view/projection matrices, +worldx
//! projects to screen-RIGHT, so a correct LEFT press must DECREASE worldx.

use sf_core::pad;
use sf_game::shell::Shell;

/// The player Arwing is `SHAPE_MYSHIP_4` (id 2).
fn player_worldx(shell: &Shell) -> Option<i16> {
    let mut cur = shell.game.objs.active_head;
    while let Some(i) = cur {
        let al = &shell.game.objs.aliens[i as usize];
        if al.shape == 2 {
            return Some(al.worldx);
        }
        cur = al.next;
    }
    None
}

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

#[test]
fn left_input_moves_ship_screen_left() {
    let mut shell = make_shell();

    // Reach gameplay: tap START periodically (mirrors AutoplayPad).
    for t in 0..500u32 {
        let p = if t % 60 == 0 && t <= 400 { pad::START } else { 0 };
        shell.tick(p);
    }
    let x0 = player_worldx(&shell).expect("player Arwing should exist in gameplay");

    // Hold LEFT.
    for _ in 0..15 {
        shell.tick(pad::LEFT);
    }
    let x_left = player_worldx(&shell).expect("player should still exist after LEFT");

    // Hold RIGHT from the same baseline for comparison.
    let mut shell2 = make_shell();
    for t in 0..500u32 {
        let p = if t % 60 == 0 && t <= 400 { pad::START } else { 0 };
        shell2.tick(p);
    }
    let x0b = player_worldx(&shell2).unwrap();
    for _ in 0..15 {
        shell2.tick(pad::RIGHT);
    }
    let x_right = player_worldx(&shell2).unwrap();

    let dl = x_left as i32 - x0 as i32;
    let dr = x_right as i32 - x0b as i32;
    eprintln!("STEER-TEST  LEFT: {x0}->{x_left} (Δ {dl})   RIGHT: {x0b}->{x_right} (Δ {dr})");

    // +worldx = screen-RIGHT, so LEFT must decrease and RIGHT must increase.
    assert!(dl < 0, "LEFT should move worldx negative (screen-left); Δ={dl} — inverted");
    assert!(dr > 0, "RIGHT should move worldx positive (screen-right); Δ={dr} — inverted");
}
