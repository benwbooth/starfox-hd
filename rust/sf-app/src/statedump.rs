//! SF_STATE_DUMP writer — byte-identical to the C oracle's per-tick trace.
//!
//! Port (C oracle): `src/main.c` `StateDump_Tick` (main.c:126):
//! `"T <tick> <pad> <state> <ndraw>\n"` then one
//! `"E <shape> <x> <y> <z> <rx> <ry> <rz> <flags>\n"` per entry.

use sf_core::DrawListEntry;
use sf_render::draw_list::DrawListEntry as RenderEntry;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct StateDump {
    out: BufWriter<File>,
    tick: u64,
}

impl StateDump {
    /// C: `SDL_getenv("SF_STATE_DUMP")` + fopen (main.c:147-151).
    pub fn from_env() -> Option<StateDump> {
        let path = std::env::var("SF_STATE_DUMP").ok()?;
        if path.is_empty() {
            return None;
        }
        match File::create(Path::new(&path)) {
            Ok(f) => Some(StateDump {
                out: BufWriter::new(f),
                tick: 0,
            }),
            Err(e) => {
                eprintln!("SF_STATE_DUMP: cannot open {path}: {e}");
                None
            }
        }
    }

    /// C `StateDump_Tick` (main.c:126-137). `state` is the numeric
    /// GameState code, `pad` is g_pad1 for this tick.
    pub fn tick(&mut self, pad: u16, state: u8, list: &[DrawListEntry]) {
        let _ = writeln!(self.out, "T {} {} {} {}", self.tick, pad, state, list.len());
        self.tick += 1;
        for e in list {
            let _ = writeln!(
                self.out,
                "E {} {} {} {} {} {} {} {}",
                e.shape_id, e.x, e.y, e.z, e.rx, e.ry, e.rz, e.flags
            );
        }
    }

    /// Native SF2 uses the renderer's equivalent draw-list boundary. Keeping
    /// this adapter here lets the same human-readable trace inspect either
    /// game without copying renderer records back into game state.
    pub fn tick_render(&mut self, pad: u16, state: u8, list: &[RenderEntry]) {
        let _ = writeln!(self.out, "T {} {} {} {}", self.tick, pad, state, list.len());
        self.tick += 1;
        for e in list {
            let _ = writeln!(
                self.out,
                "E {} {} {} {} {} {} {} {}",
                e.shape_id, e.x, e.y, e.z, e.rx, e.ry, e.rz, e.flags
            );
        }
    }

    pub fn flush(&mut self) {
        let _ = self.out.flush();
    }
}
