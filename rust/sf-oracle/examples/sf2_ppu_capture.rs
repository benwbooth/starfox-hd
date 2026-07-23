//! Capture the software-PPU output of the persistent retail SF2 machine.
//!
//!     cargo run -p sf-oracle --example sf2_ppu_capture -- 240 /tmp/sf2.ppm

use sf_oracle::RetailMachine;
use std::path::Path;

fn main() {
    let frames = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("frames must be an integer"))
        .unwrap_or(240);
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/sf2-ppu.ppm".to_owned());
    let input_mode = std::env::args().nth(3).unwrap_or_default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let rom_path = root.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut machine = RetailMachine::new(rom);
    if matches!(input_mode.as_str(), "mesen" | "hybrid" | "hybrid-limit") {
        machine.watch_gsu_execution_with_ram_mask(0x01, 0xD9FF, 0x0068, 0x0019_9F9C, 0x00FF_FFFF);
    }
    let mut video_dmas = Vec::new();
    let mut oracle_armed_frame = None;
    for native_frame in 0..frames {
        let pad = match input_mode.as_str() {
            "autostart" if native_frame >= 45 && native_frame % 120 < 12 => 0x1000,
            "rapid" if native_frame >= 40 && native_frame % 23 < 4 => 0x1000,
            "playthrough" if native_frame >= 40 => {
                let phase = native_frame % 23;
                if phase < 4 {
                    0x1000
                } else if (10..14).contains(&phase) {
                    0x8000
                } else if native_frame > 1800 && phase < 9 {
                    0x0800
                } else {
                    0
                }
            }
            "mesen" => {
                let elapsed = oracle_armed_frame.map(|armed| native_frame - armed);
                let start = (native_frame % 180 == 120 || native_frame % 180 == 121)
                    && elapsed.is_none_or(|value| value <= 600);
                let accept =
                    elapsed.is_some_and(|value| value >= 210 && matches!(value % 90, 30 | 31));
                if start {
                    0x1000
                } else if accept {
                    0x8000
                } else {
                    0
                }
            }
            "hybrid" | "hybrid-limit" => {
                let elapsed = oracle_armed_frame.map(|armed| native_frame - armed);
                if elapsed.is_none() && native_frame >= 40 && native_frame % 23 < 4 {
                    0x1000
                } else if elapsed.is_some_and(|value| value >= 210 && matches!(value % 90, 30 | 31))
                {
                    0x8000
                } else {
                    0
                }
            }
            _ => 0,
        };
        machine
            .tick_video_frames(pad, 1)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        if matches!(input_mode.as_str(), "mesen" | "hybrid" | "hybrid-limit")
            && oracle_armed_frame.is_none()
            && machine.gsu_execution_watch_hit()
        {
            oracle_armed_frame = Some(native_frame);
            eprintln!("armed Mesen-equivalent input at native frame {native_frame}");
        }
        let video_frame = machine.video_frame();
        video_dmas.extend(
            machine
                .take_dma_events()
                .into_iter()
                .filter(|event| matches!(event.bbad, 0x04 | 0x18 | 0x22))
                .map(|event| (video_frame, event)),
        );
        if input_mode == "hybrid-limit"
            && machine
                .gsu_run_debug_state()
                .is_some_and(|state| state.4 != 0)
        {
            eprintln!("stopping at first GSU step-limit event: {video_frame}");
            break;
        }
    }
    let frame = machine.ppu_frame();
    let nonzero_vram = frame.vram.iter().filter(|&&value| value != 0).count();
    let nonzero_cgram = frame.cgram.iter().filter(|&&value| value != 0).count();
    let nonzero_oam = frame.oam.iter().filter(|&&value| value != 0).count();
    let nonblack = frame
        .rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[..3] != [0, 0, 0])
        .count();
    println!(
        "frame={} cycles={} master={} pc={:06X} dp00={:02X} plots={} INIDISP={:02X} BGMODE={:02X} TM={:02X} VRAM={} CGRAM={} OAM={} nonblack={}",
        machine.video_frame(),
        machine.cycles(),
        machine.master_clock(),
        machine.pc(),
        machine.peek8(0x7E_0000),
        machine.gsu_plot_count(),
        frame.registers[0x00],
        frame.registers[0x05],
        frame.registers[0x2C],
        nonzero_vram,
        nonzero_cgram,
        nonzero_oam,
        nonblack,
    );
    println!("Mesen-equivalent input arm: {oracle_armed_frame:?}");
    println!(
        "GSU execution watch: {:?}",
        machine.gsu_execution_watch_state()
    );
    println!(
        "GSU execution watch values: {:08X?}",
        machine.gsu_execution_watch_values()
    );
    println!("GSU run debug: {:?}", machine.gsu_run_debug_state());
    println!(
        "GSU entry RAM [$003A,$24C2,$24C4,$0014]: {:04X?}",
        machine.gsu_last_entry_ram_probe()
    );
    println!("GSU recent runs:");
    for event in machine.gsu_recent_runs() {
        println!("  {event:?}");
    }
    if machine.gsu_run_debug_state().is_some_and(|state| state.3) {
        let samples = machine.gsu_last_run_samples();
        println!("GSU limit sample count: {}", samples.len());
        for sample in samples.iter().take(8) {
            println!("  head {sample:X?}");
        }
        for sample in samples
            .iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            println!("  tail {sample:X?}");
        }
    }
    println!("GSU screen state: {:?}", machine.gsu_screen_state());
    println!("PPU scroll: h={:?} v={:?}", frame.bg_hofs, frame.bg_vofs);
    println!(
        "SF2 host: map={:02X}:{:04X} active={:04X} draw_count={}",
        machine.peek8(0x7E_192E),
        machine.peek16(0x7E_1657),
        machine.peek16(0x7E_12A8),
        machine.peek16(0x7E_18C6),
    );
    println!("Timing: {:?}", machine.timing_debug_state());
    println!("Audio: {:?}", machine.audio_debug_state());
    println!("Audio PCM: {:?}", machine.audio_pcm_stats());
    for (video_frame, event) in video_dmas.iter().rev().take(40).rev() {
        println!("video DMA frame={video_frame}: {event:?}");
    }
    let mut ppm = format!("P6\n{} {}\n255\n", frame.width, frame.height).into_bytes();
    for pixel in frame.rgba.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(&output, ppm).unwrap_or_else(|error| panic!("cannot write {output}: {error}"));
    std::fs::write(format!("{output}.vram.bin"), &frame.vram).unwrap();
    std::fs::write(format!("{output}.cgram.bin"), &frame.cgram).unwrap();
    std::fs::write(format!("{output}.oam.bin"), &frame.oam).unwrap();
    std::fs::write(format!("{output}.registers.bin"), frame.registers).unwrap();
    let gsuram: Vec<u8> = (0..0x1_0000)
        .map(|address| machine.peek8(0x70_0000 + address))
        .collect();
    std::fs::write(format!("{output}.gsuram.bin"), gsuram).unwrap();
    if let Some(entry_ram) = machine.gsu_first_cd99_entry_ram() {
        std::fs::write(format!("{output}.cd99-entry.bin"), entry_ram).unwrap();
    }
    if let Some(exit_ram) = machine.gsu_first_cd99_exit_ram() {
        std::fs::write(format!("{output}.cd99-exit.bin"), exit_ram).unwrap();
    }
    if let Some(pc_trace) = machine.gsu_first_cd99_pc_trace() {
        let bytes: Vec<u8> = pc_trace.into_iter().flat_map(u32::to_le_bytes).collect();
        std::fs::write(format!("{output}.cd99-pc-trace.bin"), bytes).unwrap();
    }
    if let Some(register_trace) = machine.gsu_first_cd99_register_trace() {
        std::fs::write(
            format!("{output}.cd99-register-trace.txt"),
            register_trace.join("\n") + "\n",
        )
        .unwrap();
    }
    if let Some(point_states) = machine.gsu_first_cd99_point_states() {
        std::fs::write(
            format!("{output}.cd99-point-states.txt"),
            format!("{point_states:#X?}"),
        )
        .unwrap();
    }
    if let Some(entry_ram) = machine.gsu_first_ce37_entry_ram() {
        std::fs::write(format!("{output}.ce37-entry.bin"), entry_ram).unwrap();
    }
    if let Some(exit_ram) = machine.gsu_first_ce37_exit_ram() {
        std::fs::write(format!("{output}.ce37-exit.bin"), exit_ram).unwrap();
    }
    if let Some(pc_trace) = machine.gsu_first_ce37_pc_trace() {
        let bytes: Vec<u8> = pc_trace.into_iter().flat_map(u32::to_le_bytes).collect();
        std::fs::write(format!("{output}.ce37-pc-trace.bin"), bytes).unwrap();
    }
    if let Some(register_trace) = machine.gsu_first_ce37_register_trace() {
        std::fs::write(
            format!("{output}.ce37-register-trace.txt"),
            register_trace.join("\n") + "\n",
        )
        .unwrap();
    }
    if let Some(register_trace) = machine.gsu_first_d9ff_register_trace() {
        std::fs::write(
            format!("{output}.d9ff-register-trace.txt"),
            register_trace.join("\n") + "\n",
        )
        .unwrap();
    }
    println!("wrote {output}");
}
