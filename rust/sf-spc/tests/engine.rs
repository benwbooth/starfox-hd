//! Standalone sf-spc tests (no oracle needed): IPL boot handshake, run
//! determinism, and output-filter behavior.

use sf_spc::{Filter, SnesSpc, IPL_ROM};

/// Drive the IPL boot far enough that it signals ready ($AA/$BB) on the ports,
/// exercising the CPU, timers, IPL ROM mapping, and port register plumbing.
#[test]
fn ipl_boots_to_ready() {
    let mut spc = SnesSpc::new();
    spc.init_rom(&IPL_ROM);
    spc.reset();
    let mut buf = vec![0i16; 4096];
    unsafe { spc.set_output(buf.as_mut_ptr(), 4096) };

    let mut ready = false;
    for _ in 0..2000 {
        let p0 = spc.read_port(0, 0);
        let p1 = spc.read_port(0, 1);
        if p0 == 0xAA && p1 == 0xBB {
            ready = true;
            break;
        }
        spc.end_frame(64);
        if spc.sample_count() >= 4096 - 64 {
            unsafe { spc.set_output(buf.as_mut_ptr(), 4096) };
        }
    }
    assert!(ready, "IPL never signalled ready ($AA/$BB)");
}

/// The same starting state must produce identical output every run.
#[test]
fn playback_is_deterministic() {
    let render = || {
        let mut spc = SnesSpc::new();
        spc.init_rom(&IPL_ROM);
        spc.reset();
        let mut out = vec![0i16; 4096];
        // Boot to ready then generate some samples deterministically.
        let mut buf = vec![0i16; 4096];
        unsafe { spc.set_output(buf.as_mut_ptr(), 4096) };
        for _ in 0..500 {
            spc.end_frame(64);
            if spc.sample_count() >= 4096 - 64 {
                unsafe { spc.set_output(buf.as_mut_ptr(), 4096) };
            }
        }
        spc.play(out.len() as i32, out.as_mut_ptr());
        out
    };
    assert_eq!(render(), render(), "engine output is non-deterministic");
}

/// Filter is a linear-ish transform: silence in -> silence out; clears reset.
#[test]
fn filter_silence_and_clear() {
    let mut f = Filter::new();
    let mut buf = vec![0i16; 512];
    f.run(&mut buf);
    assert!(buf.iter().all(|&s| s == 0), "silence must stay silent");

    // A step input produces a decaying response; after clear, state resets so
    // an identical input yields an identical response.
    let step: Vec<i16> = (0..512).map(|i| if i % 2 == 0 { 4000 } else { -4000 }).collect();
    let mut a = step.clone();
    f.clear();
    f.run(&mut a);
    let mut b = step.clone();
    f.clear();
    f.run(&mut b);
    assert_eq!(a, b, "filter not repeatable after clear()");
    assert!(a.iter().any(|&s| s != 0), "filter produced only silence");
}

/// Reset returns the engine to a well-defined, repeatable state.
#[test]
fn reset_is_repeatable() {
    let mut spc = SnesSpc::new();
    spc.init_rom(&IPL_ROM);
    spc.reset();
    let a0 = spc.read_port(0, 0);
    spc.reset();
    let a1 = spc.read_port(0, 0);
    assert_eq!(a0, a1, "reset not repeatable");
}
