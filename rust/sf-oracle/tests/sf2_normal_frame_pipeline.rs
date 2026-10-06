//! Source-frame ownership of the opening's ordinary bitmap buffers.
//! The original runs from reset, including its rendering and display services.

use sf2_game::scene_frame::{BitmapBuffer, NormalFrameBuffers};
use sf_oracle::RetailMachine;

const CONTROLLER: u32 = 0x0DBCCF;
const DRAW_FINISHED: u32 = 0x7F7B74;
const TRANSFER_FINISHED: u32 = 0x7F075A;
const ORDINARY_FRAME: u32 = 0x03801F;
const ARTWORK_ENTRY: u32 = 0x03C80B;
const LOAD_GATE: u32 = 0x038228;

fn source_buffer(buffer: BitmapBuffer) -> u16 {
    match buffer {
        BitmapBuffer::First => 0x4000,
        BitmapBuffer::Second => 0xA000,
    }
}

#[test]
fn original_frame_events_preserve_independent_native_buffer_ownership() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut source = RetailMachine::new(rom);
    source.watch_cpu_execution(&[
        CONTROLLER,
        DRAW_FINISHED,
        TRANSFER_FINISHED,
        ORDINARY_FRAME,
        ARTWORK_ENTRY,
        LOAD_GATE,
    ]);
    assert!(source.tick_until_cpu_execution(0, CONTROLLER, 240).unwrap());
    assert_eq!(source.peek16(0x44), 0x4000);
    assert_eq!(source.peek16(0x46), 0xA000);
    source.take_cpu_execution_watch_hits();
    let mut native = NormalFrameBuffers::default();
    // The original queued this frame before reaching the first actor.
    native.begin_frame().unwrap();
    let mut artwork_entries = 0;
    let mut deferred_loads = 0;
    let mut transfer_counts = std::collections::BTreeSet::new();
    for update in 1..=440 {
        assert!(source.tick_until_cpu_execution(0, CONTROLLER, 240).unwrap());
        let events = source.take_cpu_execution_watch_hits();
        for event in [DRAW_FINISHED, ORDINARY_FRAME] {
            assert_eq!(
                events.iter().filter(|&&pc| pc == event).count(),
                1,
                "update {update}, event {event:06X}, events {events:X?}"
            );
        }
        assert_eq!(source.peek8(0x7E1AA6) & 1, 0, "ordinary frame {update}");
        transfer_counts.insert(events.iter().filter(|&&pc| pc == TRANSFER_FINISHED).count());
        for event in events {
            match event {
                DRAW_FINISHED => {
                    native.finish_draw().unwrap();
                }
                TRANSFER_FINISHED => {
                    native.finish_upload().unwrap();
                }
                ORDINARY_FRAME => {
                    native.begin_frame().unwrap();
                }
                LOAD_GATE => {
                    assert!(
                        !native.work_pending(),
                        "source joins both before testing the load gate"
                    );
                    deferred_loads += usize::from(!native.ready_for_scene_load());
                }
                ARTWORK_ENTRY => {
                    assert!(native.ready_for_scene_load());
                    artwork_entries += 1;
                }
                CONTROLLER => {}
                _ => unreachable!(),
            }
        }
        let work = native.next_work();
        assert_eq!(
            source.peek16(0x44),
            source_buffer(work.draw_target),
            "draw owner {update}"
        );
        assert_eq!(
            source.peek16(0x46),
            source_buffer(work.upload_source),
            "upload owner {update}"
        );
    }
    assert_eq!(artwork_entries, 1);
    assert_eq!(deferred_loads, 1);
    // Controller intervals are not transfer intervals: a later frame can
    // upload before the controller runs. Do not drive ownership from parity.
    assert_eq!(transfer_counts, [0, 1, 2].into_iter().collect());
}
