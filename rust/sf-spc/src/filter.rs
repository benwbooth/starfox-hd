//! SNES output filter (port of `SPC_Filter.cpp`): a two-point low-pass FIR
//! plus a per-channel leaky-integrator high-pass, with gain. Bit-exact.

pub const GAIN_UNIT: i32 = 0x100;
pub const BASS_NONE: i32 = 0;
pub const BASS_NORM: i32 = 8;
pub const BASS_MAX: i32 = 31;

const GAIN_BITS: i32 = 8;

#[derive(Clone, Copy, Default)]
struct Chan {
    p1: i32,
    pp1: i32,
    sum: i32,
}

pub struct Filter {
    gain: i32,
    bass: i32,
    enabled: bool,
    ch: [Chan; 2],
}

impl Filter {
    pub fn new() -> Self {
        Filter {
            gain: GAIN_UNIT,
            bass: BASS_NORM,
            enabled: true,
            ch: [Chan::default(); 2],
        }
    }

    pub fn clear(&mut self) {
        self.ch = [Chan::default(); 2];
    }

    pub fn set_gain(&mut self, gain: i32) {
        self.gain = gain;
    }

    pub fn set_bass(&mut self, bass: i32) {
        self.bass = bass;
    }

    pub fn enable(&mut self, b: bool) {
        self.enabled = b;
    }

    /// Filters `io` (interleaved stereo, even length) in place.
    pub fn run(&mut self, io: &mut [i16]) {
        let count = io.len();
        debug_assert!(count & 1 == 0);
        let gain = self.gain;
        if self.enabled {
            let bass = self.bass;
            // Process both channels; channel c uses io[c], io[c+2], ...
            for c in 0..2 {
                let chan = &mut self.ch[c];
                let mut sum = chan.sum;
                let mut pp1 = chan.pp1;
                let mut p1 = chan.p1;
                let mut i = c;
                while i < count {
                    let cur = io[i] as i32;
                    // Low-pass FIR (0.25, 0.75)
                    let f = cur + p1;
                    p1 = cur * 3;
                    // High-pass leaky integrator
                    let delta = f - pp1;
                    pp1 = f;
                    let s = sum >> (GAIN_BITS + 2);
                    sum += (delta * gain) - (sum >> bass);
                    // Clamp to 16 bits
                    let s = if s as i16 as i32 != s {
                        (s >> 31) ^ 0x7FFF
                    } else {
                        s
                    };
                    io[i] = s as i16;
                    i += 2;
                }
                chan.p1 = p1;
                chan.pp1 = pp1;
                chan.sum = sum;
            }
        } else if gain != GAIN_UNIT {
            for s in io.iter_mut() {
                let v = (*s as i32 * gain) >> GAIN_BITS;
                let v = if v as i16 as i32 != v {
                    (v >> 31) ^ 0x7FFF
                } else {
                    v
                };
                *s = v as i16;
            }
        }
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}
