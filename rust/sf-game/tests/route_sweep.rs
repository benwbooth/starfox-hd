//! Headless reproduction sweep for a user-reported "stops/crashes after the
//! title screen". Drives Shell via tick() through Title -> map -> select each of
//! the 3 routes -> gameplay, then ticks frames, catching panics per route. The
//! [state] transition log (Shell::tick) prints which level each route enters.

use sf_core::pad;
use sf_game::shell::{
    GameState, Shell, BRIEFING_INPUT_DELAY_TICKS, INTRO_INPUT_DELAY_TICKS, TITLE_INPUT_DELAY_TICKS,
};

fn drive_route(down_presses: u32, ticks: u32) -> std::thread::Result<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut sh = Shell::new();
        sh.tick(0);
        sh.tick(0);
        while sh.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
            sh.tick(pad::A);
        }
        while sh.state() != GameState::Title {
            sh.tick(0);
        }
        sh.tick(0);
        sh.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
        sh.tick(pad::START);
        while sh.state() == GameState::Title {
            sh.tick(0);
        }
        sh.tick(0);
        sh.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
        sh.tick(pad::START);
        sh.tick(0);
        sh.tick(pad::DOWN);
        sh.tick(0);
        sh.tick(pad::START);
        while sh.state() == GameState::Briefing {
            sh.tick(0);
        }
        sh.tick(0);
        // Navigate the map: DOWN advances whichroute (0->1->2).
        for _ in 0..down_presses {
            sh.tick(pad::DOWN);
            sh.tick(0);
        }
        // Confirm the route, then traverse the authored map close-up and
        // General Pepper briefing before gameplay begins.
        sh.tick(pad::START);
        for tick in 0..1_024 {
            if sh.state() == GameState::Playing {
                break;
            }
            assert_eq!(
                sh.state(),
                GameState::PlanetSelect,
                "route {down_presses} left the planet sequence unexpectedly"
            );
            sh.tick(if tick & 1 == 0 { 0 } else { pad::START });
        }
        assert_eq!(sh.state(), GameState::Playing);
        // Tick gameplay frames.
        for _ in 0..ticks {
            sh.tick(0);
        }
        format!("{:?}", sh.state())
    }))
}

#[test]
fn sweep_all_routes_enters_gameplay_without_panic() {
    let mut failures = Vec::new();
    for down in 0..3u32 {
        eprintln!("=== route via DOWN x{down} ===");
        match drive_route(down, 300) {
            Ok(st) => eprintln!("route(DOWN x{down}): OK, ended in {st}"),
            Err(_) => {
                eprintln!("route(DOWN x{down}): PANICKED");
                failures.push(down);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "routes {failures:?} panicked entering/ticking gameplay (see the panic above)"
    );
}
