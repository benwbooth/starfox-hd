//! SPC700 playback front-end (C oracle: `src/audio/spc_player.c`).
//!
//! The Star Fox SPC sound driver runs inside the emulator.  The game thread
//! communicates with it via APU port writes (engine sounds on port 1,
//! nearest-object sounds on port 2, SFX on port 3).  Because the audio
//! callback runs on a separate thread, all port writes are queued and flushed
//! at the start of each `generate` invocation — same timestamps as the C
//! (time 0 of the audio frame).

use crate::boot::{self, BootError};
use crate::spc::Spc;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// C PORT_QUEUE_SIZE: ring of 128 slots, one kept empty => 127 usable.
const PORT_QUEUE_CAPACITY: usize = 128 - 1;

/// BGM start handshake state (C `s_bgm_state`: 0 = command pending,
/// 1 = sent awaiting the driver's echo on port 0, 2 = idle).
///
/// The original runs this in the IRQ handler every frame regardless of game
/// state, so it lives on the always-running audio thread (once per
/// `generate`) rather than in the per-state game tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BgmState {
    Pending,
    AwaitEcho,
    Idle,
}

struct PlayerInner {
    spc: Spc,
    /// Port write queue (game thread -> audio thread), C `s_port_queue`.
    queue: VecDeque<(u8, u8)>,
    bgm_cmd: u8,
    bgm_state: BgmState,
    /// Last BGM command the driver echoed back (test observability only;
    /// the C oracle logs this to stderr).
    last_bgm_echo: Option<u8>,
}

impl PlayerInner {
    /// C `bgm_handshake_step` (startmus in IRQ.ASM): write the start command
    /// to port 0, re-send until the driver echoes it, then ack with 0.
    fn bgm_handshake_step(&mut self) {
        match self.bgm_state {
            BgmState::Pending => {
                self.spc.write_port(0, 0, self.bgm_cmd as i32); // .start
                self.bgm_state = BgmState::AwaitEcho;
            }
            BgmState::AwaitEcho => {
                if self.spc.read_port(0, 0) as u8 == self.bgm_cmd {
                    self.last_bgm_echo = Some(self.bgm_cmd);
                    self.spc.write_port(0, 0, 0); // .check: ack, clear
                    self.bgm_state = BgmState::Idle;
                } else {
                    // re-send until seen
                    self.spc.write_port(0, 0, self.bgm_cmd as i32);
                }
            }
            BgmState::Idle => {}
        }
    }
}

/// Shared handle to the SPC player.  Clone one for the audio callback and
/// keep one on the game thread; all methods lock internally, mirroring the
/// C's single `s_mutex`.
#[derive(Clone)]
pub struct SpcPlayer {
    inner: Arc<Mutex<PlayerInner>>,
}

impl SpcPlayer {
    /// C `SpcPlayer_Init` (always runs at SPC native 32000 Hz).
    pub fn new() -> Self {
        let mut spc = Spc::new();
        spc.reset();
        spc.filter_clear();
        SpcPlayer {
            inner: Arc::new(Mutex::new(PlayerInner {
                spc,
                queue: VecDeque::with_capacity(PORT_QUEUE_CAPACITY),
                bgm_cmd: 0,
                bgm_state: BgmState::Idle,
                last_bgm_echo: None,
            })),
        }
    }

    /// C `SpcPlayer_WritePort` (game thread): queued and applied at the
    /// start of the next `generate` call.  Silently dropped when the queue
    /// is full, like the C ring.
    pub fn write_port(&self, port: u8, value: u8) {
        if port >= 4 {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        if inner.queue.len() < PORT_QUEUE_CAPACITY {
            inner.queue.push_back((port, value));
        }
    }

    /// C `SpcPlayer_SendCommand`: cmd on port 0, param on port 1.
    pub fn send_command(&self, cmd: u8, param: u8) {
        self.write_port(0, cmd);
        self.write_port(1, param);
    }

    /// C `SpcPlayer_StartBgm`: queue a BGM start command (startbgm
    /// semantics: bgm_music + bgmcnt=0).  The audio thread performs the
    /// port-0 write/echo handshake.
    pub fn start_bgm(&self, cmd: u8) {
        let mut inner = self.inner.lock().unwrap();
        inner.bgm_cmd = cmd;
        inner.bgm_state = BgmState::Pending;
    }

    /// C `SpcPlayer_ReadPort`: read at time 0 — approximate but safe; the
    /// SPC driver updates ports asynchronously as it runs in the callback.
    pub fn read_port(&self, port: u8) -> u8 {
        if port >= 4 {
            return 0;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.spc.read_port(0, port as i32) as u8
    }

    /// C `SpcPlayer_Generate` (audio callback thread).  `buffer` is
    /// interleaved stereo (`buffer.len()` = 2 * frame count, must be even).
    ///
    /// Flushes any pending port writes at time 0 (start of this audio
    /// frame), runs the BGM handshake once (~once per frame), then generates
    /// samples through the bass/gain filter.
    pub fn generate(&self, buffer: &mut [i16]) {
        let mut inner = self.inner.lock().unwrap();

        // Flush queued port writes at time 0 (beginning of this frame).
        while let Some((port, value)) = inner.queue.pop_front() {
            inner.spc.write_port(0, port as i32, value as i32);
        }

        // Run the BGM start handshake once per callback (~once per frame).
        inner.bgm_handshake_step();

        // Generate stereo samples at SPC native 32000 Hz, filtered.
        if inner.spc.play_filtered(buffer).is_err() {
            buffer.fill(0);
        }
    }

    /// C `SpcPlayer_LoadTrack` (game thread): holds the lock for the entire
    /// IPL boot upload so the audio callback doesn't generate samples from a
    /// half-initialized SPC.  Clears the port queue (stale writes from
    /// before the reboot are invalid) and cancels any in-flight BGM
    /// handshake; the caller queues the new track's start command after the
    /// reboot.
    pub fn load_track(&self, track_id: u8, asset_dir: &Path) -> Result<(), BootError> {
        let mut inner = self.inner.lock().unwrap();
        let result = boot::load_track(&mut inner.spc, track_id, asset_dir);
        inner.queue.clear();
        inner.bgm_state = BgmState::Idle;
        result
    }

    /// Load an explicit source-game upload program for offline verification.
    /// Shipping builds do not include this oracle-only player.
    pub fn load_files(
        &self,
        files: &[&'static str],
        asset_dir: &Path,
        driver_entry: u16,
    ) -> Result<(), BootError> {
        let mut inner = self.inner.lock().unwrap();
        let result = boot::load_files(&mut inner.spc, files, asset_dir, driver_entry);
        inner.queue.clear();
        inner.bgm_state = BgmState::Idle;
        result
    }

    /// True when no BGM start handshake is pending or awaiting an echo.
    pub fn bgm_idle(&self) -> bool {
        self.inner.lock().unwrap().bgm_state == BgmState::Idle
    }

    /// Last BGM command the driver echoed back on port 0 (None until the
    /// first completed handshake).
    pub fn last_bgm_echo(&self) -> Option<u8> {
        self.inner.lock().unwrap().last_bgm_echo
    }
}

impl Default for SpcPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_queue_drains_on_generate() {
        let player = SpcPlayer::new();
        // Queue a few writes; they must not hit the SPC until generate().
        player.write_port(1, 0xC0);
        player.write_port(2, 0x55);
        let mut buf = [0i16; 128];
        player.generate(&mut buf);
        // After generate the queue is drained (writes applied at time 0).
        assert!(player.inner.lock().unwrap().queue.is_empty());
    }

    #[test]
    fn port_queue_drops_when_full() {
        let player = SpcPlayer::new();
        for i in 0..200 {
            player.write_port(3, i as u8);
        }
        assert_eq!(
            player.inner.lock().unwrap().queue.len(),
            PORT_QUEUE_CAPACITY,
            "ring keeps 127 entries like the C (128 slots, one empty)"
        );
    }

    #[test]
    fn invalid_port_ignored() {
        let player = SpcPlayer::new();
        player.write_port(4, 0xFF);
        assert!(player.inner.lock().unwrap().queue.is_empty());
        assert_eq!(player.read_port(4), 0);
    }
}
