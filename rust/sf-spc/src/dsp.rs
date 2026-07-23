//! S-DSP: 8-voice SNES sound DSP.
//!
//! Fresh Rust implementation with the exact register/timing/gaussian
//! semantics of snes_spc 0.9.0's `SPC_DSP.cpp` (the bit-exact oracle).
//! Runs one 32-clock "sample" as 32 discrete phase steps (`GEN_DSP_TIMING`),
//! so KON/KOFF timing, ENDX/OUTX/ENVX write latency, BRR decode, ADSR/GAIN
//! rate tables, noise LFSR, pitch modulation and the 8-tap echo FIR all land
//! on the same clock they do on hardware.

// DSP register addresses (global)
pub const R_MVOLL: usize = 0x0C;
pub const R_EVOLL: usize = 0x2C;
pub const R_KON: usize = 0x4C;
pub const R_KOFF: usize = 0x5C;
pub const R_FLG: usize = 0x6C;
pub const R_ENDX: usize = 0x7C;
pub const R_EFB: usize = 0x0D;
pub const R_PMON: usize = 0x2D;
pub const R_NON: usize = 0x3D;
pub const R_EON: usize = 0x4D;
pub const R_DIR: usize = 0x5D;
pub const R_ESA: usize = 0x6D;
pub const R_EDL: usize = 0x7D;
pub const R_FIR: usize = 0x0F;

// Voice register offsets
const V_VOLL: usize = 0x00;
const V_PITCHL: usize = 0x02;
const V_PITCHH: usize = 0x03;
const V_SRCN: usize = 0x04;
const V_ADSR0: usize = 0x05;
const V_ADSR1: usize = 0x06;
const V_GAIN: usize = 0x07;
const V_ENVX: usize = 0x08;
const V_OUTX: usize = 0x09;

pub const REGISTER_COUNT: usize = 128;
const VOICE_COUNT: usize = 8;
const BRR_BUF_SIZE: usize = 12;
const BRR_BLOCK_SIZE: i32 = 9;
const ECHO_HIST_SIZE: usize = 8;
pub const EXTRA_SIZE: usize = 16;

const INITIAL_REGS: [u8; REGISTER_COUNT] = [
    0x45, 0x8B, 0x5A, 0x9A, 0xE4, 0x82, 0x1B, 0x78, 0x00, 0x00, 0xAA, 0x96, 0x89, 0x0E, 0xE0, 0x80,
    0x2A, 0x49, 0x3D, 0xBA, 0x14, 0xA0, 0xAC, 0xC5, 0x00, 0x00, 0x51, 0xBB, 0x9C, 0x4E, 0x7B, 0xFF,
    0xF4, 0xFD, 0x57, 0x32, 0x37, 0xD9, 0x42, 0x22, 0x00, 0x00, 0x5B, 0x3C, 0x9F, 0x1B, 0x87, 0x9A,
    0x6F, 0x27, 0xAF, 0x7B, 0xE5, 0x68, 0x0A, 0xD9, 0x00, 0x00, 0x9A, 0xC5, 0x9C, 0x4E, 0x7B, 0xFF,
    0xEA, 0x21, 0x78, 0x4F, 0xDD, 0xED, 0x24, 0x14, 0x00, 0x00, 0x77, 0xB1, 0xD1, 0x36, 0xC1, 0x67,
    0x52, 0x57, 0x46, 0x3D, 0x59, 0xF4, 0x87, 0xA4, 0x00, 0x00, 0x7E, 0x44, 0x9C, 0x4E, 0x7B, 0xFF,
    0x75, 0xF5, 0x06, 0x97, 0x10, 0xC3, 0x24, 0xBB, 0x00, 0x00, 0x7B, 0x7A, 0xE0, 0x60, 0x12, 0x0F,
    0xF7, 0x74, 0x1C, 0xE5, 0x39, 0x3D, 0x73, 0xC1, 0x00, 0x00, 0x7A, 0xB3, 0xFF, 0x4E, 0x7B, 0xFF,
];

// Gaussian interpolation table (copied verbatim from SPC_DSP.cpp).
static GAUSS: [i16; 512] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2,
    2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10,
    10, 11, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 15, 16, 16, 17, 17, 18, 19, 19, 20, 20, 21, 21,
    22, 23, 23, 24, 24, 25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 36, 36, 37, 38, 39,
    40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 58, 59, 60, 61, 62, 64, 65,
    66, 67, 69, 70, 71, 73, 74, 76, 77, 78, 80, 81, 83, 84, 86, 87, 89, 90, 92, 94, 95, 97, 99,
    100, 102, 104, 106, 107, 109, 111, 113, 115, 117, 118, 120, 122, 124, 126, 128, 130, 132, 134,
    137, 139, 141, 143, 145, 147, 150, 152, 154, 156, 159, 161, 163, 166, 168, 171, 173, 175, 178,
    180, 183, 186, 188, 191, 193, 196, 199, 201, 204, 207, 210, 212, 215, 218, 221, 224, 227, 230,
    233, 236, 239, 242, 245, 248, 251, 254, 257, 260, 263, 267, 270, 273, 276, 280, 283, 286, 290,
    293, 297, 300, 304, 307, 311, 314, 318, 321, 325, 328, 332, 336, 339, 343, 347, 351, 354, 358,
    362, 366, 370, 374, 378, 381, 385, 389, 393, 397, 401, 405, 410, 414, 418, 422, 426, 430, 434,
    439, 443, 447, 451, 456, 460, 464, 469, 473, 477, 482, 486, 491, 495, 499, 504, 508, 513, 517,
    522, 527, 531, 536, 540, 545, 550, 554, 559, 563, 568, 573, 577, 582, 587, 592, 596, 601, 606,
    611, 615, 620, 625, 630, 635, 640, 644, 649, 654, 659, 664, 669, 674, 678, 683, 688, 693, 698,
    703, 708, 713, 718, 723, 728, 732, 737, 742, 747, 752, 757, 762, 767, 772, 777, 782, 787, 792,
    797, 802, 806, 811, 816, 821, 826, 831, 836, 841, 846, 851, 855, 860, 865, 870, 875, 880, 884,
    889, 894, 899, 904, 908, 913, 918, 923, 927, 932, 937, 941, 946, 951, 955, 960, 965, 969, 974,
    978, 983, 988, 992, 997, 1001, 1005, 1010, 1014, 1019, 1023, 1027, 1032, 1036, 1040, 1045,
    1049, 1053, 1057, 1061, 1066, 1070, 1074, 1078, 1082, 1086, 1090, 1094, 1098, 1102, 1106, 1109,
    1113, 1117, 1121, 1125, 1128, 1132, 1136, 1139, 1143, 1146, 1150, 1153, 1157, 1160, 1164, 1167,
    1170, 1174, 1177, 1180, 1183, 1186, 1190, 1193, 1196, 1199, 1202, 1205, 1207, 1210, 1213, 1216,
    1219, 1221, 1224, 1227, 1229, 1232, 1234, 1237, 1239, 1241, 1244, 1246, 1248, 1251, 1253, 1255,
    1257, 1259, 1261, 1263, 1265, 1267, 1269, 1270, 1272, 1274, 1275, 1277, 1279, 1280, 1282, 1283,
    1284, 1286, 1287, 1288, 1290, 1291, 1292, 1293, 1294, 1295, 1296, 1297, 1297, 1298, 1299, 1300,
    1300, 1301, 1302, 1302, 1303, 1303, 1303, 1304, 1304, 1304, 1304, 1304, 1305, 1305,
];

const SIMPLE_COUNTER_RANGE: i32 = 2048 * 5 * 3; // 30720

static COUNTER_RATES: [u32; 32] = [
    (SIMPLE_COUNTER_RANGE + 1) as u32, // never fires
    2048,
    1536,
    1280,
    1024,
    768,
    640,
    512,
    384,
    320,
    256,
    192,
    160,
    128,
    96,
    80,
    64,
    48,
    40,
    32,
    24,
    20,
    16,
    12,
    10,
    8,
    6,
    5,
    4,
    3,
    2,
    1,
];

static COUNTER_OFFSETS: [u32; 32] = [
    1, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 536, 0, 1040,
    536, 0, 1040, 536, 0, 1040, 536, 0, 1040, 0, 0,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvMode {
    Release,
    Attack,
    Decay,
    Sustain,
}

#[derive(Clone)]
struct Voice {
    buf: [i32; BRR_BUF_SIZE * 2],
    buf_pos: usize,
    interp_pos: i32,
    brr_addr: i32,
    brr_offset: i32,
    vbit: i32,
    kon_delay: i32,
    env_mode: EnvMode,
    env: i32,
    hidden_env: i32,
    t_envx_out: u8,
}

impl Voice {
    fn new() -> Self {
        Voice {
            buf: [0; BRR_BUF_SIZE * 2],
            buf_pos: 0,
            interp_pos: 0,
            brr_addr: 0,
            brr_offset: 1,
            vbit: 0,
            kon_delay: 0,
            env_mode: EnvMode::Release,
            env: 0,
            hidden_env: 0,
            t_envx_out: 0,
        }
    }
}

/// Output write target, mirroring snes_spc's `out`/`out_end`/`extra` scheme.
struct DspOut {
    external: bool,
    ext_ptr: *mut i16, // valid per caller contract; may point into caller buffer
    ext_cap: i32,      // stereo samples writable before overflow into `extra`
    extra_base: i32,   // start index into `extra` when !external
    extra: [i16; EXTRA_SIZE],
    idx: i32, // samples written since out_begin
}

pub struct Dsp {
    regs: [u8; REGISTER_COUNT],
    echo_hist: [[i32; 2]; ECHO_HIST_SIZE * 2],
    echo_hist_pos: usize,
    every_other_sample: i32,
    kon: i32,
    noise: i32,
    counter: i32,
    echo_offset: i32,
    echo_length: i32,
    phase: i32,
    kon_check: bool,
    new_kon: i32,
    endx_buf: u8,
    envx_buf: u8,
    outx_buf: u8,

    t_pmon: i32,
    t_non: i32,
    t_eon: i32,
    t_dir: i32,
    t_koff: i32,
    t_brr_next_addr: i32,
    t_adsr0: i32,
    t_brr_header: i32,
    t_brr_byte: i32,
    t_srcn: i32,
    t_esa: i32,
    t_echo_enabled: i32,
    t_dir_addr: i32,
    t_pitch: i32,
    t_output: i32,
    t_looped: i32,
    t_echo_ptr: i32,
    t_main_out: [i32; 2],
    t_echo_out: [i32; 2],
    t_echo_in: [i32; 2],

    voices: [Voice; VOICE_COUNT],

    ram: *mut u8,
    mute_mask: i32,
    out: DspOut,
}

#[inline]
fn clamp16(v: i32) -> i32 {
    if v as i16 as i32 != v {
        (v >> 31) ^ 0x7FFF
    } else {
        v
    }
}

impl Dsp {
    pub fn new(ram: *mut u8) -> Self {
        let mut d = Dsp {
            regs: [0; REGISTER_COUNT],
            echo_hist: [[0; 2]; ECHO_HIST_SIZE * 2],
            echo_hist_pos: 0,
            every_other_sample: 0,
            kon: 0,
            noise: 0,
            counter: 0,
            echo_offset: 0,
            echo_length: 0,
            phase: 0,
            kon_check: false,
            new_kon: 0,
            endx_buf: 0,
            envx_buf: 0,
            outx_buf: 0,
            t_pmon: 0,
            t_non: 0,
            t_eon: 0,
            t_dir: 0,
            t_koff: 0,
            t_brr_next_addr: 0,
            t_adsr0: 0,
            t_brr_header: 0,
            t_brr_byte: 0,
            t_srcn: 0,
            t_esa: 0,
            t_echo_enabled: 0,
            t_dir_addr: 0,
            t_pitch: 0,
            t_output: 0,
            t_looped: 0,
            t_echo_ptr: 0,
            t_main_out: [0; 2],
            t_echo_out: [0; 2],
            t_echo_in: [0; 2],
            voices: std::array::from_fn(|_| Voice::new()),
            ram,
            mute_mask: 0,
            out: DspOut {
                external: false,
                ext_ptr: std::ptr::null_mut(),
                ext_cap: 0,
                extra_base: 0,
                extra: [0; EXTRA_SIZE],
                idx: 0,
            },
        };
        d.set_output_none();
        d.reset();
        d
    }

    // ---- RAM access (shared 64K, honest raw pointer like the C++ oracle) ----
    #[inline]
    fn rb(&self, addr: i32) -> u8 {
        unsafe { *self.ram.add((addr & 0xFFFF) as usize) }
    }
    #[inline]
    fn wb(&self, addr: i32, v: u8) {
        unsafe { *self.ram.add((addr & 0xFFFF) as usize) = v }
    }
    #[inline]
    fn get_le16(&self, addr: i32) -> i32 {
        self.rb(addr) as i32 | ((self.rb(addr + 1) as i32) << 8)
    }
    #[inline]
    fn get_le16sa(&self, addr: i32) -> i32 {
        self.get_le16(addr) as i16 as i32
    }
    #[inline]
    fn set_le16(&self, addr: i32, data: i32) {
        self.wb(addr, data as u8);
        self.wb(addr + 1, (data >> 8) as u8);
    }

    // ---- register access ----
    #[inline]
    fn reg(&self, r: usize) -> i32 {
        self.regs[r] as i32
    }
    #[inline]
    fn vreg(&self, vi: usize, r: usize) -> i32 {
        self.regs[vi * 0x10 + r] as i32
    }
    #[inline]
    fn vreg_set(&mut self, vi: usize, r: usize, v: u8) {
        self.regs[vi * 0x10 + r] = v;
    }

    pub fn read(&self, addr: usize) -> i32 {
        self.regs[addr] as i32
    }

    pub fn write(&mut self, addr: usize, data: i32) {
        self.regs[addr] = data as u8;
        match addr & 0x0F {
            V_ENVX => self.envx_buf = data as u8,
            V_OUTX => self.outx_buf = data as u8,
            0x0C => {
                if addr == R_KON {
                    self.new_kon = data & 0xFF;
                }
                if addr == R_ENDX {
                    self.endx_buf = 0;
                    self.regs[R_ENDX] = 0;
                }
            }
            _ => {}
        }
    }

    #[allow(dead_code)] // mirrors the oracle API; unused by Star Fox playback
    pub fn mute_voices(&mut self, mask: i32) {
        self.mute_mask = mask;
    }

    #[allow(dead_code)] // mirrors the oracle API
    pub fn check_kon(&mut self) -> bool {
        let old = self.kon_check;
        self.kon_check = false;
        old
    }

    pub fn extra(&self) -> &[i16; EXTRA_SIZE] {
        &self.out.extra
    }

    pub fn dsp_sample_count(&self) -> i32 {
        self.out.idx
    }

    pub fn output_external(&self) -> bool {
        self.out.external
    }
    pub fn output_ext_cap(&self) -> i32 {
        self.out.ext_cap
    }

    // ---- output arming ----
    pub fn set_output_external(&mut self, ptr: *mut i16, cap: i32) {
        self.out.external = true;
        self.out.ext_ptr = ptr;
        self.out.ext_cap = cap;
        self.out.extra_base = 0;
        self.out.idx = 0;
    }
    pub fn set_output_extra(&mut self, base: i32) {
        self.out.external = false;
        self.out.ext_ptr = std::ptr::null_mut();
        self.out.ext_cap = 0;
        self.out.extra_base = base;
        self.out.idx = 0;
    }
    pub fn set_output_none(&mut self) {
        self.set_output_extra(0);
    }
    /// Mutable access to the internal extra buffer (for SNES_SPC set_output copy).
    pub fn extra_mut(&mut self) -> &mut [i16; EXTRA_SIZE] {
        &mut self.out.extra
    }

    #[inline]
    fn write_sample(&mut self, l: i32, r: i32) {
        let l16 = l as i16;
        let r16 = r as i16;
        if self.out.external {
            let i = self.out.idx;
            if i < self.out.ext_cap {
                unsafe {
                    *self.out.ext_ptr.add(i as usize) = l16;
                    *self.out.ext_ptr.add(i as usize + 1) = r16;
                }
            } else {
                let e = (i - self.out.ext_cap) as usize & (EXTRA_SIZE - 1);
                self.out.extra[e] = l16;
                self.out.extra[e + 1] = r16;
            }
        } else {
            let e = ((self.out.extra_base + self.out.idx) as usize) & (EXTRA_SIZE - 1);
            self.out.extra[e] = l16;
            self.out.extra[e + 1] = r16;
        }
        self.out.idx += 2;
    }

    // ---- reset / load ----
    pub fn reset(&mut self) {
        self.load(&INITIAL_REGS);
    }

    pub fn soft_reset(&mut self) {
        self.regs[R_FLG] = 0xE0;
        self.soft_reset_common();
    }

    fn soft_reset_common(&mut self) {
        self.noise = 0x4000;
        self.echo_hist_pos = 0;
        self.every_other_sample = 1;
        self.echo_offset = 0;
        self.phase = 0;
        self.counter = 0;
    }

    pub fn load(&mut self, regs: &[u8; REGISTER_COUNT]) {
        self.regs.copy_from_slice(regs);
        // Reset internal (non-register) state; C++ zeroes region up to `ram`.
        self.echo_hist = [[0; 2]; ECHO_HIST_SIZE * 2];
        self.echo_hist_pos = 0;
        self.kon = 0;
        self.echo_offset = 0;
        self.echo_length = 0;
        self.endx_buf = 0;
        self.envx_buf = 0;
        self.outx_buf = 0;
        self.t_pmon = 0;
        self.t_non = 0;
        self.t_eon = 0;
        self.t_koff = 0;
        self.t_brr_next_addr = 0;
        self.t_adsr0 = 0;
        self.t_brr_header = 0;
        self.t_brr_byte = 0;
        self.t_srcn = 0;
        self.t_echo_enabled = 0;
        self.t_dir_addr = 0;
        self.t_pitch = 0;
        self.t_output = 0;
        self.t_looped = 0;
        self.t_echo_ptr = 0;
        self.t_main_out = [0; 2];
        self.t_echo_out = [0; 2];
        self.t_echo_in = [0; 2];
        self.kon_check = false;
        for (i, v) in self.voices.iter_mut().enumerate() {
            *v = Voice::new();
            v.brr_offset = 1;
            v.vbit = 1 << i;
        }
        self.new_kon = self.reg(R_KON);
        self.t_dir = self.reg(R_DIR);
        self.t_esa = self.reg(R_ESA);
        self.soft_reset_common();
    }

    // ---- gaussian interpolation ----
    #[inline]
    fn interpolate(&self, vi: usize) -> i32 {
        let v = &self.voices[vi];
        let offset = (v.interp_pos >> 4 & 0xFF) as usize;
        let fwd = |i: usize| GAUSS[255 - offset + i] as i32;
        let rev = |i: usize| GAUSS[offset + i] as i32;
        let base = ((v.interp_pos >> 12) as usize + v.buf_pos) as usize;
        let g = |k: usize| v.buf[base + k];
        let mut out;
        out = (fwd(0) * g(0)) >> 11;
        out += (fwd(256) * g(1)) >> 11;
        out += (rev(256) * g(2)) >> 11;
        out = out as i16 as i32;
        out += (rev(0) * g(3)) >> 11;
        out = clamp16(out);
        out & !1
    }

    // ---- counters ----
    #[inline]
    fn run_counters(&mut self) {
        self.counter -= 1;
        if self.counter < 0 {
            self.counter = SIMPLE_COUNTER_RANGE - 1;
        }
    }
    #[inline]
    fn read_counter(&self, rate: i32) -> u32 {
        ((self.counter as u32).wrapping_add(COUNTER_OFFSETS[rate as usize]))
            % COUNTER_RATES[rate as usize]
    }

    // ---- envelope ----
    fn run_envelope(&mut self, vi: usize) {
        let mut env = self.voices[vi].env;
        if self.voices[vi].env_mode == EnvMode::Release {
            env -= 0x8;
            if env < 0 {
                env = 0;
            }
            self.voices[vi].env = env;
            return;
        }

        let mut rate;
        let mut env_data = self.vreg(vi, V_ADSR1);
        if self.t_adsr0 & 0x80 != 0 {
            // ADSR
            if matches!(self.voices[vi].env_mode, EnvMode::Decay | EnvMode::Sustain) {
                env -= 1;
                env -= env >> 8;
                rate = env_data & 0x1F;
                if self.voices[vi].env_mode == EnvMode::Decay {
                    rate = (self.t_adsr0 >> 3 & 0x0E) + 0x10;
                }
            } else {
                // attack
                rate = (self.t_adsr0 & 0x0F) * 2 + 1;
                env += if rate < 31 { 0x20 } else { 0x400 };
            }
        } else {
            // GAIN
            env_data = self.vreg(vi, V_GAIN);
            let mode = env_data >> 5;
            if mode < 4 {
                env = env_data * 0x10;
                rate = 31;
            } else {
                rate = env_data & 0x1F;
                if mode == 4 {
                    env -= 0x20;
                } else if mode < 6 {
                    env -= 1;
                    env -= env >> 8;
                } else {
                    env += 0x20;
                    if mode > 6 && (self.voices[vi].hidden_env as u32) >= 0x600 {
                        env += 0x8 - 0x20;
                    }
                }
            }
        }

        // Sustain level
        if (env >> 8) == (env_data >> 5) && self.voices[vi].env_mode == EnvMode::Decay {
            self.voices[vi].env_mode = EnvMode::Sustain;
        }
        self.voices[vi].hidden_env = env;
        if (env as u32) > 0x7FF {
            env = if env < 0 { 0 } else { 0x7FF };
            if self.voices[vi].env_mode == EnvMode::Attack {
                self.voices[vi].env_mode = EnvMode::Decay;
            }
        }
        if self.read_counter(rate) == 0 {
            self.voices[vi].env = env;
        }
    }

    // ---- BRR decode ----
    fn decode_brr(&mut self, vi: usize) {
        let brr_addr = self.voices[vi].brr_addr;
        let brr_offset = self.voices[vi].brr_offset;
        let mut nybbles =
            self.t_brr_byte * 0x100 + self.rb((brr_addr + brr_offset + 1) & 0xFFFF) as i32;
        let header = self.t_brr_header;

        let buf_pos = self.voices[vi].buf_pos;
        let mut pos = buf_pos;
        let mut new_buf_pos = buf_pos + 4;
        if new_buf_pos >= BRR_BUF_SIZE {
            new_buf_pos = 0;
        }
        self.voices[vi].buf_pos = new_buf_pos;

        for _ in 0..4 {
            let mut s = (nybbles as i16 as i32) >> 12;
            let shift = header >> 4;
            s = (s << shift) >> 1;
            if shift >= 0xD {
                s = (s >> 25) << 11;
            }

            let filter = header & 0x0C;
            let p1 = self.voices[vi].buf[pos + BRR_BUF_SIZE - 1];
            let p2 = self.voices[vi].buf[pos + BRR_BUF_SIZE - 2] >> 1;
            if filter >= 8 {
                s += p1;
                s -= p2;
                if filter == 8 {
                    s += p2 >> 4;
                    s += (p1 * -3) >> 6;
                } else {
                    s += (p1 * -13) >> 7;
                    s += (p2 * 3) >> 4;
                }
            } else if filter != 0 {
                s += p1 >> 1;
                s += (-p1) >> 5;
            }

            s = clamp16(s);
            s = (s * 2) as i16 as i32;
            self.voices[vi].buf[pos + BRR_BUF_SIZE] = s;
            self.voices[vi].buf[pos] = s;

            pos += 1;
            nybbles <<= 4;
        }
    }

    // ---- misc clocks ----
    fn misc_27(&mut self) {
        self.t_pmon = self.reg(R_PMON) & 0xFE;
    }
    fn misc_28(&mut self) {
        self.t_non = self.reg(R_NON);
        self.t_eon = self.reg(R_EON);
        self.t_dir = self.reg(R_DIR);
    }
    fn misc_29(&mut self) {
        self.every_other_sample ^= 1;
        if self.every_other_sample != 0 {
            self.new_kon &= !self.kon;
        }
    }
    fn misc_30(&mut self) {
        if self.every_other_sample != 0 {
            self.kon = self.new_kon;
            self.t_koff = self.reg(R_KOFF) | self.mute_mask;
        }
        self.run_counters();
        // Noise
        if self.read_counter(self.reg(R_FLG) & 0x1F) == 0 {
            let feedback = (self.noise << 13) ^ (self.noise << 14);
            self.noise = (feedback & 0x4000) ^ (self.noise >> 1);
        }
    }

    // ---- voice clocks ----
    fn voice_v1(&mut self, vi: usize) {
        self.t_dir_addr = self.t_dir * 0x100 + self.t_srcn * 4;
        self.t_srcn = self.vreg(vi, V_SRCN);
    }
    fn voice_v2(&mut self, vi: usize) {
        let mut entry = self.t_dir_addr;
        if self.voices[vi].kon_delay == 0 {
            entry += 2;
        }
        self.t_brr_next_addr = self.get_le16(entry);
        self.t_adsr0 = self.vreg(vi, V_ADSR0);
        self.t_pitch = self.vreg(vi, V_PITCHL);
    }
    fn voice_v3a(&mut self, vi: usize) {
        self.t_pitch += (self.vreg(vi, V_PITCHH) & 0x3F) << 8;
    }
    fn voice_v3b(&mut self, vi: usize) {
        let brr_addr = self.voices[vi].brr_addr;
        let brr_offset = self.voices[vi].brr_offset;
        self.t_brr_byte = self.rb((brr_addr + brr_offset) & 0xFFFF) as i32;
        self.t_brr_header = self.rb(brr_addr) as i32;
    }
    fn voice_v3c(&mut self, vi: usize) {
        // Pitch modulation using previous voice's output
        if self.t_pmon & self.voices[vi].vbit != 0 {
            self.t_pitch += ((self.t_output >> 5) * self.t_pitch) >> 10;
        }

        if self.voices[vi].kon_delay != 0 {
            if self.voices[vi].kon_delay == 5 {
                self.voices[vi].brr_addr = self.t_brr_next_addr;
                self.voices[vi].brr_offset = 1;
                self.voices[vi].buf_pos = 0;
                self.t_brr_header = 0;
                self.kon_check = true;
            }
            self.voices[vi].env = 0;
            self.voices[vi].hidden_env = 0;
            self.voices[vi].interp_pos = 0;
            self.voices[vi].kon_delay -= 1;
            if self.voices[vi].kon_delay & 3 != 0 {
                self.voices[vi].interp_pos = 0x4000;
            }
            self.t_pitch = 0;
        }

        // Gaussian interpolation
        let mut output = self.interpolate(vi);
        if self.t_non & self.voices[vi].vbit != 0 {
            output = (self.noise * 2) as i16 as i32;
        }
        self.t_output = (output * self.voices[vi].env) >> 11 & !1;
        self.voices[vi].t_envx_out = (self.voices[vi].env >> 4) as u8;

        // Immediate silence due to end of sample or soft reset
        if self.reg(R_FLG) & 0x80 != 0 || (self.t_brr_header & 3) == 1 {
            self.voices[vi].env_mode = EnvMode::Release;
            self.voices[vi].env = 0;
        }

        if self.every_other_sample != 0 {
            if self.t_koff & self.voices[vi].vbit != 0 {
                self.voices[vi].env_mode = EnvMode::Release;
            }
            if self.kon & self.voices[vi].vbit != 0 {
                self.voices[vi].kon_delay = 5;
                self.voices[vi].env_mode = EnvMode::Attack;
            }
        }

        if self.voices[vi].kon_delay == 0 {
            self.run_envelope(vi);
        }
    }

    #[inline]
    fn voice_output(&mut self, vi: usize, ch: usize) {
        let amp = (self.t_output * self.vreg(vi, V_VOLL + ch) as i8 as i32) >> 7;
        self.t_main_out[ch] += amp;
        self.t_main_out[ch] = clamp16(self.t_main_out[ch]);
        if self.t_eon & self.voices[vi].vbit != 0 {
            self.t_echo_out[ch] += amp;
            self.t_echo_out[ch] = clamp16(self.t_echo_out[ch]);
        }
    }

    fn voice_v4(&mut self, vi: usize) {
        self.t_looped = 0;
        if self.voices[vi].interp_pos >= 0x4000 {
            self.decode_brr(vi);
            self.voices[vi].brr_offset += 2;
            if self.voices[vi].brr_offset >= BRR_BLOCK_SIZE {
                self.voices[vi].brr_addr = (self.voices[vi].brr_addr + BRR_BLOCK_SIZE) & 0xFFFF;
                if self.t_brr_header & 1 != 0 {
                    self.voices[vi].brr_addr = self.t_brr_next_addr;
                    self.t_looped = self.voices[vi].vbit;
                }
                self.voices[vi].brr_offset = 1;
            }
        }

        self.voices[vi].interp_pos = (self.voices[vi].interp_pos & 0x3FFF) + self.t_pitch;
        if self.voices[vi].interp_pos > 0x7FFF {
            self.voices[vi].interp_pos = 0x7FFF;
        }
        self.voice_output(vi, 0);
    }
    fn voice_v5(&mut self, vi: usize) {
        self.voice_output(vi, 1);
        let mut endx_buf = self.reg(R_ENDX) | self.t_looped;
        if self.voices[vi].kon_delay == 5 {
            endx_buf &= !self.voices[vi].vbit;
        }
        self.endx_buf = endx_buf as u8;
    }
    fn voice_v6(&mut self, _vi: usize) {
        self.outx_buf = (self.t_output >> 8) as u8;
    }
    fn voice_v7(&mut self, vi: usize) {
        self.regs[R_ENDX] = self.endx_buf;
        self.envx_buf = self.voices[vi].t_envx_out;
    }
    fn voice_v8(&mut self, vi: usize) {
        self.vreg_set(vi, V_OUTX, self.outx_buf);
    }
    fn voice_v9(&mut self, vi: usize) {
        self.vreg_set(vi, V_ENVX, self.envx_buf);
    }
    fn voice_v3(&mut self, vi: usize) {
        self.voice_v3a(vi);
        self.voice_v3b(vi);
        self.voice_v3c(vi);
    }

    // ---- echo ----
    #[inline]
    fn echo_fir(&self, i: usize, ch: usize) -> i32 {
        self.echo_hist[self.echo_hist_pos + i][ch]
    }
    #[inline]
    fn calc_fir(&self, i: usize, ch: usize) -> i32 {
        (self.echo_fir(i + 1, ch) * self.reg(R_FIR + i * 0x10) as i8 as i32) >> 6
    }
    fn echo_read(&mut self, ch: usize) {
        let s = self.get_le16sa(self.t_echo_ptr + ch as i32 * 2);
        self.echo_hist[self.echo_hist_pos][ch] = s >> 1;
        self.echo_hist[self.echo_hist_pos + 8][ch] = s >> 1;
    }
    fn echo_22(&mut self) {
        self.echo_hist_pos += 1;
        if self.echo_hist_pos >= ECHO_HIST_SIZE {
            self.echo_hist_pos = 0;
        }
        self.t_echo_ptr = (self.t_esa * 0x100 + self.echo_offset) & 0xFFFF;
        self.echo_read(0);
        self.t_echo_in[0] = self.calc_fir(0, 0);
        self.t_echo_in[1] = self.calc_fir(0, 1);
    }
    fn echo_23(&mut self) {
        self.t_echo_in[0] += self.calc_fir(1, 0) + self.calc_fir(2, 0);
        self.t_echo_in[1] += self.calc_fir(1, 1) + self.calc_fir(2, 1);
        self.echo_read(1);
    }
    fn echo_24(&mut self) {
        self.t_echo_in[0] += self.calc_fir(3, 0) + self.calc_fir(4, 0) + self.calc_fir(5, 0);
        self.t_echo_in[1] += self.calc_fir(3, 1) + self.calc_fir(4, 1) + self.calc_fir(5, 1);
    }
    fn echo_25(&mut self) {
        let mut l = self.t_echo_in[0] + self.calc_fir(6, 0);
        let mut r = self.t_echo_in[1] + self.calc_fir(6, 1);
        l = l as i16 as i32;
        r = r as i16 as i32;
        l += self.calc_fir(7, 0) as i16 as i32;
        r += self.calc_fir(7, 1) as i16 as i32;
        self.t_echo_in[0] = clamp16(l) & !1;
        self.t_echo_in[1] = clamp16(r) & !1;
    }
    #[inline]
    fn echo_output(&self, ch: usize) -> i32 {
        let out = ((self.t_main_out[ch] * self.reg(R_MVOLL + ch * 0x10) as i8 as i32) >> 7) as i16
            as i32
            + ((self.t_echo_in[ch] * self.reg(R_EVOLL + ch * 0x10) as i8 as i32) >> 7) as i16
                as i32;
        clamp16(out)
    }
    fn echo_26(&mut self) {
        self.t_main_out[0] = self.echo_output(0);
        let mut l = self.t_echo_out[0]
            + ((self.t_echo_in[0] * self.reg(R_EFB) as i8 as i32) >> 7) as i16 as i32;
        let mut r = self.t_echo_out[1]
            + ((self.t_echo_in[1] * self.reg(R_EFB) as i8 as i32) >> 7) as i16 as i32;
        l = clamp16(l);
        r = clamp16(r);
        self.t_echo_out[0] = l & !1;
        self.t_echo_out[1] = r & !1;
    }
    fn echo_27(&mut self) {
        let mut l = self.t_main_out[0];
        let mut r = self.echo_output(1);
        self.t_main_out[0] = 0;
        self.t_main_out[1] = 0;
        if self.reg(R_FLG) & 0x40 != 0 {
            l = 0;
            r = 0;
        }
        self.write_sample(l, r);
    }
    fn echo_28(&mut self) {
        self.t_echo_enabled = self.reg(R_FLG);
    }
    fn echo_write(&mut self, ch: usize) {
        if self.t_echo_enabled & 0x20 == 0 {
            self.set_le16(self.t_echo_ptr + ch as i32 * 2, self.t_echo_out[ch]);
        }
        self.t_echo_out[ch] = 0;
    }
    fn echo_29(&mut self) {
        self.t_esa = self.reg(R_ESA);
        if self.echo_offset == 0 {
            self.echo_length = (self.reg(R_EDL) & 0x0F) * 0x800;
        }
        self.echo_offset += 4;
        if self.echo_offset >= self.echo_length {
            self.echo_offset = 0;
        }
        self.echo_write(0);
        self.t_echo_enabled = self.reg(R_FLG);
    }
    fn echo_30(&mut self) {
        self.echo_write(1);
    }

    // ---- phase machine ----
    #[inline]
    fn run_phase(&mut self, phase: i32) {
        match phase {
            0 => {
                self.voice_v5(0);
                self.voice_v2(1);
            }
            1 => {
                self.voice_v6(0);
                self.voice_v3(1);
            }
            2 => {
                // V7_V4_V1(0): V7(0) V1(3) V4(1)
                self.voice_v7(0);
                self.voice_v1(3);
                self.voice_v4(1);
            }
            3 => {
                // V8_V5_V2(0): V8(0) V5(1) V2(2)
                self.voice_v8(0);
                self.voice_v5(1);
                self.voice_v2(2);
            }
            4 => {
                // V9_V6_V3(0): V9(0) V6(1) V3(2)
                self.voice_v9(0);
                self.voice_v6(1);
                self.voice_v3(2);
            }
            5 => {
                self.voice_v7(1);
                self.voice_v1(4);
                self.voice_v4(2);
            }
            6 => {
                self.voice_v8(1);
                self.voice_v5(2);
                self.voice_v2(3);
            }
            7 => {
                self.voice_v9(1);
                self.voice_v6(2);
                self.voice_v3(3);
            }
            8 => {
                self.voice_v7(2);
                self.voice_v1(5);
                self.voice_v4(3);
            }
            9 => {
                self.voice_v8(2);
                self.voice_v5(3);
                self.voice_v2(4);
            }
            10 => {
                self.voice_v9(2);
                self.voice_v6(3);
                self.voice_v3(4);
            }
            11 => {
                self.voice_v7(3);
                self.voice_v1(6);
                self.voice_v4(4);
            }
            12 => {
                self.voice_v8(3);
                self.voice_v5(4);
                self.voice_v2(5);
            }
            13 => {
                self.voice_v9(3);
                self.voice_v6(4);
                self.voice_v3(5);
            }
            14 => {
                self.voice_v7(4);
                self.voice_v1(7);
                self.voice_v4(5);
            }
            15 => {
                self.voice_v8(4);
                self.voice_v5(5);
                self.voice_v2(6);
            }
            16 => {
                self.voice_v9(4);
                self.voice_v6(5);
                self.voice_v3(6);
            }
            17 => {
                self.voice_v1(0);
                self.voice_v7(5);
                self.voice_v4(6);
            }
            18 => {
                self.voice_v8(5);
                self.voice_v5(6);
                self.voice_v2(7);
            }
            19 => {
                self.voice_v9(5);
                self.voice_v6(6);
                self.voice_v3(7);
            }
            20 => {
                self.voice_v1(1);
                self.voice_v7(6);
                self.voice_v4(7);
            }
            21 => {
                self.voice_v8(6);
                self.voice_v5(7);
                self.voice_v2(0);
            }
            22 => {
                self.voice_v3a(0);
                self.voice_v9(6);
                self.voice_v6(7);
                self.echo_22();
            }
            23 => {
                self.voice_v7(7);
                self.echo_23();
            }
            24 => {
                self.voice_v8(7);
                self.echo_24();
            }
            25 => {
                self.voice_v3b(0);
                self.voice_v9(7);
                self.echo_25();
            }
            26 => {
                self.echo_26();
            }
            27 => {
                self.misc_27();
                self.echo_27();
            }
            28 => {
                self.misc_28();
                self.echo_28();
            }
            29 => {
                self.misc_29();
                self.echo_29();
            }
            30 => {
                self.misc_30();
                self.voice_v3c(0);
                self.echo_30();
            }
            31 => {
                self.voice_v4(0);
                self.voice_v1(2);
            }
            _ => unreachable!(),
        }
    }

    pub fn run(&mut self, clocks: i32) {
        debug_assert!(clocks > 0);
        let mut phase = self.phase;
        for _ in 0..clocks {
            self.run_phase(phase);
            phase = (phase + 1) & 31;
        }
        self.phase = phase;
    }
}
