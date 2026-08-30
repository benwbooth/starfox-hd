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
const PPU_DOTS_PER_LINE: u64 = 341;
const VIDEO_LINES_PER_FRAME: u64 = 262;
const MASTER_CLOCKS_PER_PPU_DOT: u64 = 4;
const VIDEO_FRAME_MASTER_CLOCKS: u64 =
    PPU_DOTS_PER_LINE * VIDEO_LINES_PER_FRAME * MASTER_CLOCKS_PER_PPU_DOT;
const RETAIL_FRAME_COUNTER: u32 = 0x1200;
const RETAIL_FRAME_COUNTER_RESET: u32 = 0x02_D960;
const RETAIL_FRAME_COUNTER_RESET_COMPLETE: u32 = 0x02_D963;
const RETAIL_FRAME_COUNTER_SAMPLE: u32 = 0x02_DA78;
const RETAIL_FRAME_RATE_SAMPLE_COMPLETE: u32 = 0x02_DA7E;
const RETAIL_STRATEGIES_COMPLETE: u32 = 0x02_DA08;
const RETAIL_PRE_TRANSFER_WORK_COMPLETE: u32 = 0x02_DA3B;
const RETAIL_FIRST_TRANSFER_READY: u32 = 0x02_DA40;
const RETAIL_SECOND_TRANSFER_WAIT_BEGIN: u32 = 0x02_DA4C;
const RETAIL_SECOND_TRANSFER_READY: u32 = 0x02_DA53;
const RETAIL_SCENE_RENDER_BEGIN: u32 = 0x02_DA65;
const RETAIL_SCENE_RENDER_COMPLETE: u32 = 0x02_DA69;
const FRAME_COUNTER_RESET_SIGNATURE: [u8; 3] = [0x9C, 0x00, 0x12];
const FRAME_COUNTER_SAMPLE_SIGNATURE: [u8; 6] = [0xAD, 0x00, 0x12, 0x8D, 0xE3, 0x14];

fn frame_bound(name: &str, default: u16) -> u16 {
    std::env::var(name)
        .ok()
        .map(|value| value.parse().expect("retail timing frame must be decimal"))
        .unwrap_or(default)
}

fn largest_job(jobs: &[GsuRunEvent]) -> Option<&GsuRunEvent> {
    jobs.iter().max_by_key(|job| job.exit_tick - job.entry_tick)
}

fn lorom_offset(address: u32) -> usize {
    let bank = usize::try_from(address >> 16 & 0x7F).expect("LoROM bank must fit");
    let cpu_address = usize::try_from(address & 0xFFFF).expect("LoROM address must fit");
    assert!(cpu_address >= 0x8000, "oracle code address must be LoROM");
    bank * 0x8000 + (cpu_address - 0x8000)
}

fn assert_retail_signature(rom: &[u8], address: u32, expected: &[u8]) {
    let offset = lorom_offset(address);
    assert_eq!(
        rom.get(offset..offset + expected.len()),
        Some(expected),
        "retail timing instruction signature changed at {address:06X}",
    );
}

fn raster_irq_phase(master_clock: u64, irq_scanline: u16) -> u64 {
    let irq_master_clock = u64::from(irq_scanline) * PPU_DOTS_PER_LINE * MASTER_CLOCKS_PER_PPU_DOT;
    assert!(
        irq_master_clock < VIDEO_FRAME_MASTER_CLOCKS,
        "retail IRQ scanline must lie inside the video frame",
    );
    (master_clock % VIDEO_FRAME_MASTER_CLOCKS + VIDEO_FRAME_MASTER_CLOCKS - irq_master_clock)
        % VIDEO_FRAME_MASTER_CLOCKS
}

fn reach_timeline_marker(
    retail: &mut RetailMachine,
    input: u16,
    marker: u32,
    max_video_frames: u32,
    start_master_clock: u64,
    marker_name: &str,
) -> u64 {
    assert!(
        retail
            .tick_until_cpu_execution(input, marker, max_video_frames)
            .unwrap_or_else(|error| panic!("retail {marker_name}: {error}")),
        "retail did not reach {marker_name}",
    );
    retail.master_clock() - start_master_clock
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
    assert_retail_signature(
        &rom,
        RETAIL_FRAME_COUNTER_RESET,
        &FRAME_COUNTER_RESET_SIGNATURE,
    );
    assert_retail_signature(
        &rom,
        RETAIL_FRAME_COUNTER_SAMPLE,
        &FRAME_COUNTER_SAMPLE_SIGNATURE,
    );
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

    // `transfer_l` reset `framec` before this DOSTRATS boundary. Move to the
    // following reset so every emitted row covers the entire reset-to-sample
    // interval, including strategy, draw-list, sprite, collision, and 3D work.
    let initial_input = if routed {
        corneria_attack_carrier_input(retail.peek16(WORK_RAM | RETAIL_GAMEFRAME))
    } else {
        0
    };
    assert!(
        retail
            .tick_until_cpu_execution(
                initial_input,
                RETAIL_FRAME_COUNTER_RESET_COMPLETE,
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
            )
            .expect("initial frame-counter reset"),
        "retail did not reach the first complete frame-counter reset",
    );
    assert_eq!(
        retail.peek8(WORK_RAM | RETAIL_FRAME_COUNTER),
        0,
        "frame counter must be zero immediately after its retail reset",
    );

    println!(
        "route,scene_frame,input,entry_motion,sampled_motion,frame_counter_at_sample,elapsed_master_clocks,start_display_phase,start_irq_scanline,start_irq_phase,irq_pending_at_reset,irq_masked_at_reset,elapsed_display_frames,strategies_begin_clock,strategies_complete_clock,pre_transfer_work_complete_clock,first_transfer_ready_clock,second_transfer_wait_begin_clock,second_transfer_ready_clock,scene_render_begin_clock,scene_render_complete_clock,job_count,job_master_clocks,job_program_fetch_clocks,job_memory_wait_clocks,job_asset_wait_clocks,job_multiply_clocks,job_pixel_clocks,largest_job_master_clocks,largest_job_program_fetch_clocks,largest_job_memory_wait_clocks,largest_job_asset_wait_clocks,largest_job_multiply_clocks,largest_job_pixel_clocks"
    );
    loop {
        let reset_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let Some(scene_frame) = reset_frame.checked_add(1) else {
            break;
        };
        let input = if routed {
            corneria_attack_carrier_input(reset_frame)
        } else {
            0
        };
        let entry_motion = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
        let start_master_clock = retail.master_clock();
        let start_video_frame = retail.video_frame();
        let (
            _,
            irq_enabled_at_reset,
            start_irq_scanline,
            irq_pending_at_reset,
            irq_masked_at_reset,
            _,
            _,
            _,
        ) = retail.timing_debug_state();
        assert!(
            irq_enabled_at_reset,
            "gameplay timing capture requires the retail raster IRQ",
        );
        let start_irq_phase = raster_irq_phase(start_master_clock, start_irq_scanline);
        let previous_job_sequence = retail
            .gsu_recent_runs()
            .last()
            .map_or(0, |job| job.sequence);
        let max_video_frames = if reset_frame == CORNERIA_AUDIO_UPLOAD_FRAME {
            MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
        } else {
            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
        };
        let strategies_begin = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_DOSTRATS,
            max_video_frames,
            start_master_clock,
            "strategy update start",
        );
        let strategies_complete = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_STRATEGIES_COMPLETE,
            max_video_frames,
            start_master_clock,
            "strategy update completion",
        );
        let pre_transfer_work_complete = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_PRE_TRANSFER_WORK_COMPLETE,
            max_video_frames,
            start_master_clock,
            "pre-transfer work completion",
        );
        let first_transfer_ready = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_FIRST_TRANSFER_READY,
            max_video_frames,
            start_master_clock,
            "first transfer readiness",
        );
        let second_transfer_wait_begin = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_SECOND_TRANSFER_WAIT_BEGIN,
            max_video_frames,
            start_master_clock,
            "second transfer wait start",
        );
        let second_transfer_ready = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_SECOND_TRANSFER_READY,
            max_video_frames,
            start_master_clock,
            "second transfer readiness",
        );
        let scene_render_begin = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_SCENE_RENDER_BEGIN,
            max_video_frames,
            start_master_clock,
            "scene render start",
        );
        let scene_render_complete = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_SCENE_RENDER_COMPLETE,
            max_video_frames,
            start_master_clock,
            "scene render completion",
        );
        let elapsed_master_clocks = reach_timeline_marker(
            &mut retail,
            input,
            RETAIL_FRAME_RATE_SAMPLE_COMPLETE,
            max_video_frames,
            start_master_clock,
            "frame-rate sample completion",
        );
        assert_eq!(
            retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
            scene_frame,
            "retail update must advance exactly one scene frame",
        );
        let sampled_motion = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
        let frame_counter_at_sample = retail.peek8(WORK_RAM | RETAIL_FRAME_COUNTER);
        assert_eq!(
            sampled_motion, frame_counter_at_sample,
            "retail framerate must copy the live frame counter",
        );
        let (_, irq_enabled_at_sample, sampled_irq_scanline, _, _, _, _, _) =
            retail.timing_debug_state();
        assert!(irq_enabled_at_sample, "retail raster IRQ became disabled");
        assert_eq!(
            sampled_irq_scanline, start_irq_scanline,
            "retail raster IRQ scanline changed inside one timing interval",
        );
        let jobs = retail
            .gsu_recent_runs()
            .into_iter()
            .filter(|job| job.sequence > previous_job_sequence)
            .collect::<Vec<_>>();
        if (first_scene..=last_scene).contains(&scene_frame) {
            let elapsed_display_frames = retail.video_frame() - start_video_frame;
            let crossed_irq_boundaries =
                (start_irq_phase + elapsed_master_clocks) / VIDEO_FRAME_MASTER_CLOCKS;
            assert_eq!(
                u64::from(frame_counter_at_sample),
                crossed_irq_boundaries,
                "frame counter must equal gameplay raster IRQ boundaries crossed",
            );
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
                "{},{scene_frame},{input},{entry_motion},{sampled_motion},{frame_counter_at_sample},{elapsed_master_clocks},{},{start_irq_scanline},{start_irq_phase},{},{},{elapsed_display_frames},{strategies_begin},{strategies_complete},{pre_transfer_work_complete},{first_transfer_ready},{second_transfer_wait_begin},{second_transfer_ready},{scene_render_begin},{scene_render_complete},{},{job_master_clocks},{},{},{},{},{},{largest_master_clocks},{},{},{},{},{}",
                u8::from(routed),
                start_master_clock % VIDEO_FRAME_MASTER_CLOCKS,
                u8::from(irq_pending_at_reset),
                u8::from(irq_masked_at_reset),
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
        assert!(
            retail
                .tick_until_cpu_execution(
                    input,
                    RETAIL_FRAME_COUNTER_RESET_COMPLETE,
                    MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                )
                .expect("next frame-counter reset"),
            "retail did not reset the counter after scene frame {scene_frame}",
        );
        assert_eq!(
            retail.peek8(WORK_RAM | RETAIL_FRAME_COUNTER),
            0,
            "frame counter must be zero at the next capture start",
        );
    }
}
