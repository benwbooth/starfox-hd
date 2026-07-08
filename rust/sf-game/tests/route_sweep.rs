//! Headless reproduction sweep for a user-reported "stops/crashes after the
//! title screen". Drives Shell via tick() through Title -> map -> select each of
//! the 3 routes -> gameplay, then ticks frames, catching panics per route. The
//! [state] transition log (Shell::tick) prints which level each route enters.

use sf_core::pad;
use sf_game::shell::{GameState, Shell};

fn drive_route(down_presses: u32, ticks: u32) -> std::thread::Result<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut sh = Shell::new();
        // Boot -> Title (a couple of ticks to settle).
        for _ in 0..4 {
            if sh.state() == GameState::Title {
                break;
            }
            sh.tick(0);
        }
        // Title -> PlanetSelect (edge-triggered START).
        sh.tick(pad::START);
        sh.tick(0);
        // Navigate the map: DOWN advances whichroute (0->1->2).
        for _ in 0..down_presses {
            sh.tick(pad::DOWN);
            sh.tick(0);
        }
        // Select -> begin gameplay at this route's first level.
        sh.tick(pad::START);
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
