//! Oracle A/B parity: the pure-Rust `sf-spc` engine must be sample-identical
//! to the snes_spc C++ FFI oracle on real Star Fox audio.
//!
//! Requires the `ffi-oracle` feature (so BOTH backends are linked in one
//! binary). Run with:
//!   cargo test -p sf-audio --features ffi-oracle --test parity
#![cfg(feature = "ffi-oracle")]

use sf_audio::backend::SpcEngine;
use sf_audio::{boot, ffi, ffi_spc, native};
use std::path::PathBuf;

const SAMPLE_RATE: usize = 32000;
const FRAMES_PER_CALLBACK: usize = 1024;
const SECONDS: usize = 10;

fn asset_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SF_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    if dir.join("snd").join("SGSOUND0.BIN").is_file() {
        Some(dir)
    } else {
        eprintln!("skipping: no sound data at {}", dir.display());
        None
    }
}

/// Unfiltered stereo play, common to both backends.
trait Raw {
    fn raw(&mut self, out: &mut [i16]);
}
impl Raw for native::Spc {
    fn raw(&mut self, out: &mut [i16]) {
        native::Spc::play_raw(self, out).expect("native play");
    }
}
impl Raw for ffi_spc::Spc {
    fn raw(&mut self, out: &mut [i16]) {
        ffi_spc::Spc::play_raw(self, out).expect("ffi play");
    }
}

/// Render `seconds` of raw (pre-filter) audio, driving the BGM start handshake
/// exactly like `SpcPlayer::generate` does (once per callback).
fn render_raw<E: SpcEngine + Raw>(eng: &mut E, cmd: u8, seconds: usize) -> (Vec<i16>, Option<u8>) {
    let callbacks = seconds * SAMPLE_RATE / FRAMES_PER_CALLBACK;
    let mut buf = vec![0i16; FRAMES_PER_CALLBACK * 2];
    let mut out = Vec::with_capacity(callbacks * buf.len());
    let mut state = 0u8; // 0 pending, 1 await echo, 2 idle
    let mut echoed = None;
    for _ in 0..callbacks {
        match state {
            0 => {
                eng.write_port(0, 0, cmd as i32);
                state = 1;
            }
            1 => {
                if eng.read_port(0, 0) as u8 == cmd {
                    echoed = Some(cmd);
                    eng.write_port(0, 0, 0);
                    state = 2;
                } else {
                    eng.write_port(0, 0, cmd as i32);
                }
            }
            _ => {}
        }
        eng.raw(&mut buf);
        out.extend_from_slice(&buf);
    }
    (out, echoed)
}

/// Filter a raw stream with the native Rust `sf-spc` filter (default gain/bass).
fn filter_native(raw: &[i16]) -> Vec<i16> {
    let mut f = sf_spc::Filter::new();
    let mut v = raw.to_vec();
    f.run(&mut v);
    v
}

/// Filter a raw stream with the C++ oracle `SPC_Filter` (default gain/bass).
fn filter_ffi(raw: &[i16]) -> Vec<i16> {
    unsafe {
        let f = ffi::spc_filter_new();
        ffi::spc_filter_set_gain(f, ffi::SPC_FILTER_GAIN_UNIT);
        ffi::spc_filter_set_bass(f, ffi::SPC_FILTER_BASS_NORM);
        let mut v = raw.to_vec();
        ffi::spc_filter_run(f, v.as_mut_ptr(), v.len() as i32);
        ffi::spc_filter_delete(f);
        v
    }
}

struct Divergence {
    first: Option<usize>,
    max_abs: i64,
    rms: f64,
    len: usize,
}

fn diff(a: &[i16], b: &[i16]) -> Divergence {
    assert_eq!(a.len(), b.len(), "stream length mismatch");
    let mut first = None;
    let mut max_abs = 0i64;
    let mut sq = 0f64;
    for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
        let d = (x as i64 - y as i64).abs();
        if d != 0 && first.is_none() {
            first = Some(i);
        }
        max_abs = max_abs.max(d);
        sq += (d * d) as f64;
    }
    Divergence {
        first,
        max_abs,
        rms: (sq / a.len() as f64).sqrt(),
        len: a.len(),
    }
}

fn run_oracle_ab(track: u8, label: &str) {
    let Some(dir) = asset_dir() else { return };

    // Native engine (default backend).
    let mut n = native::Spc::new();
    n.reset();
    n.filter_clear();
    boot::load_track(&mut n, track, &dir).expect("native boot");
    let (n_raw, n_echo) = render_raw(&mut n, boot::track_command(track), SECONDS);

    // FFI oracle.
    let mut o = ffi_spc::Spc::new();
    o.reset();
    o.filter_clear();
    boot::load_track(&mut o, track, &dir).expect("ffi boot");
    let (o_raw, o_echo) = render_raw(&mut o, boot::track_command(track), SECONDS);

    assert_eq!(n_echo, o_echo, "{label}: BGM echo mismatch");
    assert!(n_echo.is_some(), "{label}: driver never echoed start cmd");

    let pre = diff(&n_raw, &o_raw);
    eprintln!(
        "{label} PRE-filter: {} samples, first_div={:?} max_abs={} rms={:.4}",
        pre.len, pre.first, pre.max_abs, pre.rms
    );
    assert!(
        pre.first.is_none(),
        "{label}: pre-filter divergence at sample {:?} (max_abs={}, rms={:.4})",
        pre.first,
        pre.max_abs,
        pre.rms
    );

    let n_post = filter_native(&n_raw);
    let o_post = filter_ffi(&o_raw);
    let post = diff(&n_post, &o_post);
    eprintln!(
        "{label} POST-filter: first_div={:?} max_abs={} rms={:.4}",
        post.first, post.max_abs, post.rms
    );
    assert!(
        post.first.is_none(),
        "{label}: post-filter divergence at sample {:?} (max_abs={}, rms={:.4})",
        post.first,
        post.max_abs,
        post.rms
    );

    // Sanity: this is real, audible signal (not silence).
    let peak = n_post.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
    assert!(peak > 1000, "{label}: peak {peak} too quiet — not real audio");
    eprintln!("{label}: BIT-EXACT over {SECONDS}s (peak={peak})");
}

#[test]
fn oracle_ab_title_track2() {
    // Track 2 = SND_TITLE (single self-contained file SGSOUND7, cmd $12).
    run_oracle_ab(boot::SND_TITLE, "title(2)");
}

#[test]
fn oracle_ab_corneria_track9() {
    // Track 9 = SND_11 (Corneria: driver + SGSOUND1/2 + SGSOUND3/A/1, cmd $03).
    run_oracle_ab(boot::SND_11, "corneria(9)");
}

// ---------------------------------------------------------------------------
// Port-protocol A/B: the IPL upload + idle timeline must read back identical
// port values at identical timestamps on both engines.
// ---------------------------------------------------------------------------

fn ipl_timeline<E: SpcEngine>(eng: &mut E, steps: usize) -> Vec<(i32, i32, i32)> {
    use sf_audio::spc::IPL_ROM;
    eng.init_rom(&IPL_ROM);
    eng.reset();
    let mut buf = vec![0i16; 4096];
    unsafe { eng.set_output(buf.as_mut_ptr(), 4096) };
    let mut rec = Vec::with_capacity(steps);
    for i in 0..steps {
        let p0 = eng.read_port(0, 0);
        let p1 = eng.read_port(0, 1);
        rec.push((i as i32, p0, p1));
        eng.end_frame(64);
        if eng.sample_count() >= 4096 - 64 {
            unsafe { eng.set_output(buf.as_mut_ptr(), 4096) };
        }
    }
    rec
}

#[test]
fn port_protocol_ipl_timeline() {
    let mut n = native::Spc::new();
    let mut o = ffi_spc::Spc::new();
    let nt = ipl_timeline(&mut n, 4000);
    let ot = ipl_timeline(&mut o, 4000);
    let first = nt.iter().zip(ot.iter()).position(|(a, b)| a != b);
    assert!(
        first.is_none(),
        "IPL port timeline diverged at step {:?}: native={:?} ffi={:?}",
        first,
        first.map(|i| nt[i]),
        first.map(|i| ot[i]),
    );
    // The IPL must reach ready ($AA/$BB) on both, proving the ROM + timers run.
    assert!(
        nt.iter().any(|&(_, p0, p1)| p0 == 0xAA && p1 == 0xBB),
        "IPL never signalled ready"
    );
    eprintln!("port protocol: {} steps identical", nt.len());
}
