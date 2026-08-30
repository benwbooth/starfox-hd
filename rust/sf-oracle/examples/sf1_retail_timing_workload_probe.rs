//! Capture exact retail frame-boundary duration and completed 3D-job costs.
//!
//! All machine-specific observations remain inside `sf-oracle`; the emitted
//! rows are calibration evidence for the shipping port's typed workload model.

#[path = "support/mod.rs"]
mod support;

use sf_oracle::sf1_input::{corneria_attack_carrier_input, corneria_front_end_input};
use sf_oracle::{
    load_retail_rom, GsuRunEvent, RetailMachine, RETAIL_DOSTRATS, RETAIL_FRAMERATE,
    RETAIL_GAMEFRAME,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_FRONT_END_TICK: u32 = 3;
const FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF: u32 = 890;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const CORNERIA_AUDIO_UPLOAD_FRAME: u16 = 186;
const MAX_HANDOFF_BOUNDARIES: usize = 4;
const DEFAULT_FIRST_COMPLETED_SCENE: u16 = 315;
const DEFAULT_LAST_COMPLETED_SCENE: u16 = 322;
const DISPLAY_MASTER_CLOCKS: u64 = 341 * 262 * 4;

fn frame_bound(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .map(|value| value.parse().expect("retail timing frame must be decimal"))
        .unwrap_or(default)
}

fn largest_job(jobs: &[GsuRunEvent]) -> Option<&GsuRunEvent> {
    jobs.iter().max_by_key(|job| job.exit_tick - job.entry_tick)
}

fn main() {
    let first_scene = frame_bound(
        "SF1_RETAIL_TIMING_FIRST_SCENE",
        DEFAULT_FIRST_COMPLETED_SCENE,
    );
    let last_scene = frame_bound("SF1_RETAIL_TIMING_LAST_SCENE", DEFAULT_LAST_COMPLETED_SCENE);
    assert!(
        first_scene <= last_scene,
        "retail timing range must be ordered"
    );
    let routed = std::env::var_os("SF1_RETAIL_TIMING_ROUTE").is_some();
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);

    for tick in 0..FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF {
        retail
            .tick_video_frames(
                corneria_front_end_input(tick),
                VIDEO_FRAMES_PER_FRONT_END_TICK,
            )
            .expect("retail front-end timing");
    }
    assert!(
        retail
            .tick_until_cpu_execution(
                corneria_front_end_input(FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF),
                RETAIL_DOSTRATS,
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
            )
            .expect("initial Corneria timing boundary"),
        "retail did not reach initial Corneria boundary",
    );
    let mut handoff_tick = FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF;
    for _ in 0..MAX_HANDOFF_BOUNDARIES {
        if retail.peek16(WORK_RAM | RETAIL_GAMEFRAME) == 0 {
            break;
        }
        handoff_tick = handoff_tick.saturating_add(1);
        assert!(
            retail
                .tick_until_cpu_execution(
                    corneria_front_end_input(handoff_tick),
                    RETAIL_DOSTRATS,
                    MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                )
                .expect("Corneria frame-zero timing boundary"),
            "retail did not reach Corneria frame zero",
        );
    }
    assert_eq!(retail.peek16(WORK_RAM | RETAIL_GAMEFRAME), 0);

    println!(
        "route,scene_frame,input,entry_motion,next_motion,elapsed_master_clocks,start_display_phase,elapsed_display_frames,job_count,job_master_clocks,job_program_fetch_clocks,job_memory_wait_clocks,job_asset_wait_clocks,job_multiply_clocks,job_pixel_clocks,largest_job_master_clocks,largest_job_program_fetch_clocks,largest_job_memory_wait_clocks,largest_job_asset_wait_clocks,largest_job_multiply_clocks,largest_job_pixel_clocks"
    );
    loop {
        let entry_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let Some(scene_frame) = entry_frame.checked_add(1) else {
            break;
        };
        let input = if routed {
            corneria_attack_carrier_input(entry_frame)
        } else {
            0
        };
        let entry_motion = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
        let start_master_clock = retail.master_clock();
        let start_video_frame = retail.video_frame();
        let previous_job_sequence = retail
            .gsu_recent_runs()
            .last()
            .map_or(0, |job| job.sequence);
        let max_video_frames = if entry_frame == CORNERIA_AUDIO_UPLOAD_FRAME {
            MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
        } else {
            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
        };
        assert!(
            retail
                .tick_until_cpu_execution(input, RETAIL_DOSTRATS, max_video_frames)
                .expect("next Corneria timing boundary"),
            "retail did not finish entry frame {entry_frame}",
        );
        let jobs = retail
            .gsu_recent_runs()
            .into_iter()
            .filter(|job| job.sequence > previous_job_sequence)
            .collect::<Vec<_>>();
        if (first_scene..=last_scene).contains(&scene_frame) {
            let elapsed_master_clocks = retail.master_clock() - start_master_clock;
            let elapsed_display_frames = retail.video_frame() - start_video_frame;
            let job_master_clocks = jobs
                .iter()
                .map(|job| job.exit_tick - job.entry_tick)
                .sum::<u64>();
            let timing = std::array::from_fn::<_, 5, _>(|category| {
                jobs.iter()
                    .map(|job| job.timing_breakdown[category])
                    .sum::<u64>()
            });
            let largest = largest_job(&jobs);
            let largest_master_clocks = largest.map_or(0, |job| job.exit_tick - job.entry_tick);
            let largest_timing = largest.map_or([0; 5], |job| job.timing_breakdown);
            println!(
                "{},{scene_frame},{input},{entry_motion},{},{elapsed_master_clocks},{},{elapsed_display_frames},{},{job_master_clocks},{},{},{},{},{},{largest_master_clocks},{},{},{},{},{}",
                u8::from(routed),
                retail.peek8(WORK_RAM | RETAIL_FRAMERATE),
                start_master_clock % DISPLAY_MASTER_CLOCKS,
                jobs.len(),
                timing[0],
                timing[1],
                timing[2],
                timing[3],
                timing[4],
                largest_timing[0],
                largest_timing[1],
                largest_timing[2],
                largest_timing[3],
                largest_timing[4],
            );
        }
        if scene_frame >= last_scene {
            break;
        }
    }
}
