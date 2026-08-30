//! Emit deterministic native source-render workload features for timing-model
//! calibration.  This probe is read-only: it drives the game through ordinary
//! controller input and never writes simulation state.

#[path = "support/mod.rs"]
mod support;

use sf_game::gameplay_timing::timing_for_update;
use sf_game::shell::{GameState, GameplayEntryPhase};
use sf_map::catalog::map_id;
use sf_oracle::sf1_input::{corneria_attack_carrier_input, corneria_front_end_input};
use sf_render::renderer::{config_from_repo_root, Renderer};
use std::io::Write;

const DEFAULT_FIRST_GAME_FRAME: u16 = 321;
const DEFAULT_LAST_GAME_FRAME: u16 = 321;
const REPLAY_TICK_BUDGET: u32 = 4_000;

fn frame_bound(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .map(|value| value.parse().expect("workload frame bound must be decimal"))
        .unwrap_or(default)
}

fn main() {
    let first_frame = frame_bound("SF1_TIMING_WORKLOAD_FIRST_FRAME", DEFAULT_FIRST_GAME_FRAME);
    let last_frame = frame_bound("SF1_TIMING_WORKLOAD_LAST_FRAME", DEFAULT_LAST_GAME_FRAME);
    assert!(
        first_frame <= last_frame,
        "workload frame range must be ordered"
    );
    let routed = std::env::var_os("SF1_TIMING_WORKLOAD_ROUTE").is_some();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(
        sf_render::SOURCE_FRAME_WIDTH as i32,
        sf_render::SOURCE_FRAME_HEIGHT as i32,
        &config_from_repo_root(repository),
    )
    .expect("headless source workload renderer");
    let mut shell = support::configured_shell();
    let mut sampled_frames = 0usize;
    let mut output = std::env::var_os("SF1_TIMING_WORKLOAD_CSV").map(|path| {
        let mut output = std::io::BufWriter::new(
            std::fs::File::create(path).expect("create source-workload CSV"),
        );
        writeln!(
            output,
            "route,frame,input,motion,presentation,draw_entries,object_passes,face_selections,point_samples,point_writes,polygon_candidates,polygons_drawn,polygon_samples,polygon_writes,line_candidates,lines_drawn,line_samples,line_writes,textured_polygon_candidates,textured_polygons_drawn,texture_samples,texture_writes,scaled_sprite_candidates,scaled_sprites_drawn,scaled_sprite_samples,scaled_sprite_writes"
        )
        .expect("write source-workload CSV header");
        output
    });

    for tick in 0..REPLAY_TICK_BUDGET {
        let active = shell.state() == GameState::Playing
            && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let input = if active && routed {
            corneria_attack_carrier_input(shell.game.vars.gameframe)
        } else if active {
            0
        } else {
            corneria_front_end_input(tick)
        };
        shell.tick(input);
        let frame = shell.frame();
        if frame.gameplay_entry_phase != GameplayEntryPhase::ActiveLevel
            || !(first_frame..=last_frame).contains(&frame.gameframe)
        {
            continue;
        }

        let current_draw_list = shell
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect::<Vec<_>>();
        let _ = support::render_playing_snapshot(&frame, &[], &current_draw_list, &mut renderer);
        let workload = renderer.source_frame_workload();
        let target = timing_for_update(map_id::M1_1, frame.gameframe);
        if let Some(output) = output.as_mut() {
            writeln!(
                output,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                u8::from(routed),
                frame.gameframe,
                input,
                target.motion_refreshes,
                target.presentation_refreshes,
                current_draw_list.len(),
                workload.object_passes,
                workload.face_selections,
                workload.point_samples,
                workload.point_writes,
                workload.polygon_candidates,
                workload.polygons_drawn,
                workload.polygon_samples,
                workload.polygon_writes,
                workload.line_candidates,
                workload.lines_drawn,
                workload.line_samples,
                workload.line_writes,
                workload.textured_polygon_candidates,
                workload.textured_polygons_drawn,
                workload.texture_samples,
                workload.texture_writes,
                workload.scaled_sprite_candidates,
                workload.scaled_sprites_drawn,
                workload.scaled_sprite_samples,
                workload.scaled_sprite_writes,
            )
            .expect("write source-workload CSV row");
        } else {
            println!(
                "timing_workload route={} frame={} input={} motion_target={} presentation_target={} draw_entries={} workload={workload:?}",
                routed,
                frame.gameframe,
                input,
                target.motion_refreshes,
                target.presentation_refreshes,
                current_draw_list.len(),
            );
        }
        sampled_frames += 1;
        if frame.gameframe == last_frame {
            break;
        }
    }

    assert_eq!(
        sampled_frames,
        usize::from(last_frame - first_frame + 1),
        "native workload replay did not cover the requested frame range",
    );
}
