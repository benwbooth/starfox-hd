//! SPC700 IPL boot protocol (C oracle: `src/audio/spc_boot.c`).
//!
//! Uploads the Star Fox sound driver and BGM data into the SPC700 emulator
//! via the authentic SNES IPL ROM handshake protocol (SOUND.ASM sbootapu),
//! parsing the self-describing sub-block BIN format used by Star Fox's sound
//! data files: repeated [len:2 LE][dest:2 LE][data...] blocks terminated by
//! [00 00][exec:2 LE].  The per-file exec word is ignored; after all files of
//! a track are uploaded, execution is transferred to the driver entry point
//! ($0400), exactly like sbootapu does.

use crate::backend::SpcEngine;
use crate::spc::IPL_ROM;
use std::fmt;
use std::path::Path;

// Oracle tooling and shipping playback share one typed catalog, preventing
// the offline renderer from silently naming a cue differently from runtime.
pub use crate::catalog::{
    SND_10, SND_11, SND_12, SND_13, SND_13B, SND_14, SND_15, SND_16, SND_20, SND_21, SND_22,
    SND_23, SND_24, SND_25, SND_26, SND_30, SND_31, SND_32, SND_33, SND_34, SND_35, SND_36, SND_37,
    SND_BHOLE, SND_CONTINUE, SND_ENDSEQ, SND_GAMEOVER, SND_INIT, SND_INTRO, SND_MAP, SND_OPS,
    SND_SPECIAL, SND_STAFF, SND_TITLE, SND_TRACK_COUNT, SND_TRAINING,
};

/// ROM `SOUND5LEN` ($009424) — byte length of `SGSOUND5.BIN` (tunnel/intro
/// sample bank). Data symbol, not a callable; kept for ledger parity.
pub const SOUND5_LEN: u32 = 0x009424;

// ---------------------------------------------------------------------------
// Track-to-file mapping table (C `s_track_files`, from SOUND.ASM sndtbl).
// SGSOUND0.BIN (the driver) is always loaded first, separately.
//
// Label -> file map (BANK/INCBINS.ASM incsnd + BANK8/9/10 + SHBANKS.ASM):
//   sound0=SGSOUND0  sound1=SGSOUND1  orchestra=SGSOUND2  band=SGSOUND3
//   sound4=SGSOUND4  sound5=SGSOUND5  sound6=SGSOUND6  titlebgm=SGSOUND7
//   bgmendseq=SGSOUND8  bgmcontinue=SGSOUND9  bgmstaff=SGSOUNDA
//   corneria=SGBGMA  meteor_base=SGBGMB  titania=SGBGMC  fortuna=SGBGMD
//   macbeth=SGBGME  asteroid_field=SGBGMF  space_armada=SGBGMG
//   venom_base_2=SGBGMH  sector_x_z=SGBGMI  sector_y=SGBGMJ
//   venom_base_1_3=SGBGMK  black_holebgm=SGBGML  bgmintro=SGBGMM
//   ops_bgm=SGBGMN  bgmtraining=SGBGMO  out_of_this_dimension=SGBGMP
//   boss_corneria=SGBGM1  boss_fortuna=SGBGM2  boss_macbeth=SGBGM3
//   boss_titania=SGBGM4  boss_meteor=SGBGM5  boss_space_armada=SGBGM6
//   boss_asteroid=SGBGM7  boss_venom_base_2=SGBGM8  boss_venom_base_1=SGBGM9
//   venom_andross=SGBGM10  game_overbgm=SGBGM11
// ---------------------------------------------------------------------------
#[rustfmt::skip]
const TRACK_FILES: [&[&str]; SND_TRACK_COUNT as usize] = [
    /* SND_INIT     */ &["SGSOUND0.BIN"],
    /* SND_INTRO    */ &["SGSOUND1.BIN", "SGSOUND2.BIN", "SGBGMM.BIN"],
    /* SND_TITLE    */ &["SGSOUND7.BIN"],
    /* SND_OPS      */ &["SGSOUND1.BIN", "SGBGMN.BIN"],
    /* SND_TRAINING */ &["SGSOUND3.BIN", "SGBGMO.BIN"],
    /* SND_MAP      */ &["SGSOUND2.BIN", "SGSOUND6.BIN"],
    /* SND_CONTINUE */ &["SGSOUND9.BIN"],
    /* SND_BHOLE    */ &["SGBGML.BIN"],
    /* SND_10       */ &["SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN"],
    /* SND_11       */ &["SGSOUND1.BIN", "SGSOUND3.BIN", "SGBGMA.BIN", "SGBGM1.BIN"],
    /* SND_12       */ &["SGBGMF.BIN", "SGBGM7.BIN"],
    /* SND_13       */ &["SGBGMG.BIN", "SGBGM6.BIN"],
    /* SND_13B      */ &["SGBGMG.BIN", "SGBGM6.BIN"],
    /* SND_14       */ &["SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMB.BIN", "SGBGM5.BIN"],
    /* SND_15       */ &["SGBGMF.BIN", "SGBGM8.BIN"],
    /* SND_16       */ &["SGBGMK.BIN", "SGBGM8.BIN", "SGBGM10.BIN"],
    /* SND_20       */ &["SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN"],
    /* SND_21       */ &["SGSOUND1.BIN", "SGSOUND3.BIN"],
    /* SND_22       */ &["SGBGMI.BIN", "SGBGM7.BIN"],
    /* SND_23       */ &["SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMC.BIN", "SGBGM4.BIN"],
    /* SND_24       */ &["SGBGMJ.BIN", "SGBGM7.BIN"],
    /* SND_25       */ &["SGBGMF.BIN", "SGBGM8.BIN"],
    /* SND_26       */ &["SGBGMH.BIN", "SGBGM8.BIN", "SGBGM10.BIN"],
    /* SND_30       */ &["SGBGMA.BIN", "SGBGM1.BIN", "SGSOUND5.BIN"],
    /* SND_31       */ &["SGSOUND1.BIN", "SGSOUND3.BIN"],
    /* SND_32       */ &["SGBGMF.BIN", "SGBGM7.BIN"],
    /* SND_33       */ &["SGSOUND3.BIN", "SGSOUND4.BIN", "SGBGMD.BIN", "SGBGM2.BIN"],
    /* SND_34       */ &["SGBGMI.BIN", "SGBGM6.BIN"],
    /* SND_35       */ &["SGSOUND3.BIN", "SGBGME.BIN", "SGBGM3.BIN"],
    /* SND_36       */ &["SGBGMF.BIN", "SGBGM9.BIN"],
    /* SND_37       */ &["SGBGMK.BIN", "SGBGM9.BIN", "SGBGM10.BIN"],
    /* SND_ENDSEQ   */ &["SGSOUND8.BIN"],
    /* SND_STAFF    */ &["SGSOUNDA.BIN"],
    /* SND_GAMEOVER */ &["SGBGM11.BIN"],
    /* SND_SPECIAL  */ &["SGSOUND3.BIN", "SGBGMP.BIN"],
];

/// BGM start command for a track (C `SpcBoot_TrackCommand`).
/// Out-of-table values are raw driver commands, returned unchanged.
pub fn track_command(track_id: u8) -> u8 {
    crate::catalog::track_start_cue(track_id)
}

// ---------------------------------------------------------------------------
// Common-bank prerequisites (C `track_needs_common_banks`).
//
// The original never clears APU RAM between boots: sbootapu re-enters the IPL
// with the $FF command and uploads only the listed files, so gameplay rows
// like `snd_data 12,3,asteroid_field,boss_asteroid` (a few KB of sequence
// data at $E000+) rely on the common sample banks staying resident:
//   SGSOUND1 (sound1): sample dir $3C00, BRR data $4000-$B360 (SFX + base)
//   SGSOUND2 (orchestra): dir $3C60, data $B360-$DCF0, insts $3D00,
//                         song table + fanfares $F342-$FDE2
// Because we power-cycle the SPC on every load, gameplay tracks must
// re-upload those banks first.  Rows that carry their own bank (band =
// SGSOUND3 overlays orchestra's range, sound5 overlays both) upload it
// afterwards and win, matching the original's final RAM state.  UI tracks
// (title/intro/ops/endseq/staff) are self-contained.
// ---------------------------------------------------------------------------
fn track_needs_common_banks(track_id: u8) -> bool {
    !matches!(
        track_id,
        SND_INIT | SND_INTRO | SND_TITLE | SND_OPS | SND_ENDSEQ | SND_STAFF
    )
}

// ---------------------------------------------------------------------------
// IPL handshake timing (C IPL_STEP / IPL_TIMEOUT / BOOT_BUF_SIZE).
// ---------------------------------------------------------------------------
const IPL_STEP: i32 = 64; // SPC clocks per polling step (~2 sample pairs)
const IPL_TIMEOUT: u32 = 50_000; // max polling steps waiting for a port echo
const BOOT_BUF_SIZE: usize = 4096; // dummy output buffer during boot upload

/// SPC driver entry point (C SPC_DRIVER_ENTRY).
pub const SPC_DRIVER_ENTRY: u16 = 0x0400;

#[derive(Debug)]
pub enum BootError {
    /// Track ID >= SND_TRACK_COUNT (C: "invalid track ID").
    InvalidTrack(u8),
    /// SGSOUND0.BIN (the driver) could not be read — no audio possible.
    MissingDriver(std::io::Error),
    /// An upload program must begin with a sound-driver file.
    EmptyProgram,
    /// Timed out waiting for the IPL ready signal ($AA/$BB).
    ReadyTimeout,
    /// Timed out waiting for a port-0 echo of `val` during `what`.
    EchoTimeout { what: &'static str, val: u8 },
    /// A sub-block header claims more bytes than remain in the file.
    Malformed { file: &'static str },
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BootError::InvalidTrack(id) => write!(f, "SPC Boot: invalid track ID {id}"),
            BootError::MissingDriver(e) => {
                write!(
                    f,
                    "SPC Boot: cannot load driver SGSOUND0.BIN ({e}) — no audio"
                )
            }
            BootError::EmptyProgram => write!(f, "SPC Boot: empty upload program"),
            BootError::ReadyTimeout => write!(f, "SPC Boot: timeout waiting for IPL ready"),
            BootError::EchoTimeout { what, val } => {
                write!(
                    f,
                    "SPC Boot: timeout waiting for echo ${val:02X} during {what}"
                )
            }
            BootError::Malformed { file } => {
                write!(f, "SPC Boot: sub-block overflows file {file}")
            }
        }
    }
}

impl std::error::Error for BootError {}

// ---------------------------------------------------------------------------
// Boot context: SPC + rotating dummy output buffer.  Boxed so the pointer
// handed to spc_set_output stays stable for the whole upload.
// ---------------------------------------------------------------------------
struct Booter<'a, S: SpcEngine> {
    spc: &'a mut S,
    buf: Box<[i16; BOOT_BUF_SIZE]>,
}

impl<'a, S: SpcEngine> Booter<'a, S> {
    fn new(spc: &'a mut S) -> Self {
        let mut b = Booter {
            spc,
            buf: Box::new([0i16; BOOT_BUF_SIZE]),
        };
        b.arm_output();
        b
    }

    fn arm_output(&mut self) {
        unsafe {
            self.spc
                .set_output(self.buf.as_mut_ptr(), BOOT_BUF_SIZE as i32)
        };
    }

    /// Advance the SPC emulator by a small time step, keeping the output
    /// buffer rotating so it never overflows (C `spc_advance`).
    fn advance(&mut self, clocks: i32) {
        self.spc.end_frame(clocks);
        // After end_frame the internal time resets to 0.  Re-arm the output
        // buffer so subsequent calls don't overflow.
        if self.spc.sample_count() >= (BOOT_BUF_SIZE as i32 - 64) {
            self.arm_output();
        }
    }

    /// Wait for the IPL ROM ready signal ($AA on port0, $BB on port1)
    /// (C `ipl_wait_ready`).
    fn wait_ready(&mut self) -> Result<(), BootError> {
        for _ in 0..IPL_TIMEOUT {
            let p0 = self.spc.read_port(0, 0);
            let p1 = self.spc.read_port(0, 1);
            if p0 == 0xAA && p1 == 0xBB {
                return Ok(());
            }
            self.advance(IPL_STEP);
        }
        Err(BootError::ReadyTimeout)
    }

    /// Wait for port 0 to echo a specific value (C `ipl_wait_echo`).
    fn wait_echo(&mut self, val: u8, what: &'static str) -> Result<(), BootError> {
        for _ in 0..IPL_TIMEOUT {
            let p0 = self.spc.read_port(0, 0) as u8;
            if p0 == val {
                return Ok(());
            }
            self.advance(IPL_STEP);
        }
        Err(BootError::EchoTimeout { what, val })
    }

    /// Upload one data block via the IPL port protocol (C `ipl_upload_block`;
    /// SOUND.ASM sboot_entry4 / sboot_repeat / sboot_loop).
    ///
    /// `cmd` holds the "block command" value written to port 0 to announce
    /// the block ($CC for the very first block after IPL ready; afterwards
    /// derived from the last byte index).  Updated for the next block.
    fn upload_block(&mut self, dest: u16, data: &[u8], cmd: &mut u8) -> Result<(), BootError> {
        // Announce the block: dest -> ports 2/3, non-zero -> port 1 (data
        // follows), block command -> port 0.  The IPL echoes the command.
        self.spc.write_port(0, 2, (dest & 0xFF) as i32);
        self.spc.write_port(0, 3, (dest >> 8) as i32);
        self.spc
            .write_port(0, 1, if data.is_empty() { 0 } else { 1 });
        self.spc.write_port(0, 0, *cmd as i32);
        self.wait_echo(*cmd, "block header")?;

        // Send bytes: data -> port 1, index -> port 0; wait for the IPL to
        // echo the index back after storing the byte.  Index restarts at 0
        // each block and wraps mod 256 (the IPL tracks the carry into the
        // dest high byte).
        let mut idx: u8 = 0;
        for &byte in data {
            self.spc.write_port(0, 1, byte as i32);
            self.spc.write_port(0, 0, idx as i32);
            self.wait_echo(idx, "block byte")?;
            idx = idx.wrapping_add(1);
        }

        // Next block command = last index + 4, skipping 0 (sboot_zero).
        let mut next = idx.wrapping_add(3); // idx == last_index + 1
        if next == 0 {
            next = 4;
        }
        *cmd = next;
        Ok(())
    }

    /// Upload all sub-blocks of one BIN file (C `ipl_upload_file`).  Stops at
    /// the [00 00] terminator (the trailing per-file exec word is ignored,
    /// exactly like sboot_entry3).
    fn upload_file(
        &mut self,
        file: &'static str,
        data: &[u8],
        cmd: &mut u8,
    ) -> Result<(), BootError> {
        let mut pos = 0usize;

        while pos + 4 <= data.len() {
            let count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            let dest = u16::from_le_bytes([data[pos + 2], data[pos + 3]]);
            pos += 4;

            if count == 0 {
                break; // terminator; 'dest' is the ignored per-file exec addr
            }

            if pos + count > data.len() {
                return Err(BootError::Malformed { file });
            }

            self.upload_block(dest, &data[pos..pos + count], cmd)?;
            pos += count;
        }

        Ok(())
    }

    /// Transfer control to an SPC address (C `ipl_start_exec`; SOUND.ASM
    /// sboot_entry3 .end): exec addr -> ports 2/3, zero -> port 1 (jump, not
    /// data), command -> port 0.  The IPL echoes the command and jumps.
    /// Afterwards ports 1-3 are zeroed like the original (stz apu_port1/2/3).
    fn start_exec(&mut self, addr: u16, cmd: u8) -> Result<(), BootError> {
        self.spc.write_port(0, 2, (addr & 0xFF) as i32);
        self.spc.write_port(0, 3, (addr >> 8) as i32);
        self.spc.write_port(0, 1, 0);
        self.spc.write_port(0, 0, cmd as i32);
        self.wait_echo(cmd, "exec")?;

        self.spc.write_port(0, 1, 0);
        self.spc.write_port(0, 2, 0);
        self.spc.write_port(0, 3, 0);
        Ok(())
    }
}

/// Read a BIN file from `<asset_dir>/snd/` (C `load_bin_file`, minus the
/// SDL exe-relative fallback: the caller supplies the asset dir explicitly,
/// no cwd assumptions).
fn load_bin_file(asset_dir: &Path, filename: &str) -> std::io::Result<Vec<u8>> {
    std::fs::read(asset_dir.join("snd").join(filename))
}

/// Upload a track's sound bank and start the driver (C `SpcBoot_LoadTrack`).
///
/// `asset_dir` is the game data directory; sound data lives at
/// `<asset_dir>/snd/`.
pub fn load_track<S: SpcEngine>(
    spc: &mut S,
    track_id: u8,
    asset_dir: &Path,
) -> Result<(), BootError> {
    if track_id >= SND_TRACK_COUNT {
        return Err(BootError::InvalidTrack(track_id));
    }

    // Build the upload list: driver, resident common banks (if the track
    //    needs them), then the track's own files.  Duplicates are skipped so
    //    rows that already list sound1/band don't upload twice; later files
    //    intentionally overwrite earlier ones (band over orchestra, etc.).
    // -----------------------------------------------------------------------
    let mut upload_list: Vec<&'static str> = Vec::with_capacity(12);
    upload_list.push("SGSOUND0.BIN");
    if track_needs_common_banks(track_id) {
        upload_list.push("SGSOUND1.BIN");
        upload_list.push("SGSOUND2.BIN");
    }
    for &file in TRACK_FILES[track_id as usize] {
        if !upload_list.contains(&file) && upload_list.len() < 12 {
            upload_list.push(file);
        }
    }

    load_files(spc, &upload_list, asset_dir, SPC_DRIVER_ENTRY)
}

/// Upload an explicitly selected source-game sound program for offline
/// verification and start its driver.
///
/// This is the game-neutral form of [`load_track`]. It is used by oracle
/// tooling for Star Fox 2, whose retail host selects variable lists of upload
/// blobs rather than SF1's fixed track table. The shipping runtime does not
/// compile this module and only consumes rendered PCM.
pub fn load_files<S: SpcEngine>(
    spc: &mut S,
    upload_list: &[&'static str],
    asset_dir: &Path,
    driver_entry: u16,
) -> Result<(), BootError> {
    if upload_list.is_empty() {
        return Err(BootError::EmptyProgram);
    }

    // Power-cycle the sound processor and let its immutable boot program come
    // up. The retail host re-enters that program between scene loads; a reset
    // reaches the same upload state because this routine supplies the complete
    // resident program explicitly.
    spc.init_rom(&IPL_ROM);
    spc.reset();

    let mut booter = Booter::new(spc);
    booter.wait_ready()?;

    // First block is announced with the boot protocol's start token.
    let mut command: u8 = 0xCC;
    for (index, &file) in upload_list.iter().enumerate() {
        let data = match load_bin_file(asset_dir, file) {
            Ok(d) => d,
            Err(e) => {
                if index == 0 {
                    return Err(BootError::MissingDriver(e));
                }
                eprintln!("SPC Boot: skipping missing file {file} ({e})");
                continue;
            }
        };
        booter.upload_file(file, &data, &mut command)?;
    }

    booter.start_exec(driver_entry, command)?;

    // Let the driver run its init before any BGM/SFX commands are written.
    // On real hardware at least a frame passes before startmus (IRQ) writes
    // port 0; the driver samples the ports as its idle baseline during init,
    // so a command written too early is never seen as a change.  ~130 ms.
    for _ in 0..64 {
        booter.advance(2048);
    }

    // Detach the boot buffer so the emulator retains no pointer into it
    // (the next play_filtered call re-arms the output internally).
    drop(booter);
    spc.detach_output();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_command_table_matches_oracle() {
        // Spot checks against spc_boot.c s_track_bgm_cmd.
        assert_eq!(track_command(SND_TITLE), 0x12);
        assert_eq!(track_command(SND_MAP), 0x01);
        assert_eq!(track_command(SND_10), 0x10);
        assert_eq!(track_command(SND_13), 0x09);
        assert_eq!(track_command(SND_GAMEOVER), 0x0C);
        // Out-of-table values are raw driver commands (>= SND_TRACK_COUNT).
        assert_eq!(track_command(0xF0), 0xF0);
        assert_eq!(track_command(35), 35);
    }

    #[test]
    fn common_banks_gate_matches_oracle() {
        for t in [
            SND_INIT, SND_INTRO, SND_TITLE, SND_OPS, SND_ENDSEQ, SND_STAFF,
        ] {
            assert!(!track_needs_common_banks(t), "track {t} is self-contained");
        }
        for t in [
            SND_TRAINING,
            SND_MAP,
            SND_CONTINUE,
            SND_BHOLE,
            SND_11,
            SND_GAMEOVER,
        ] {
            assert!(track_needs_common_banks(t), "track {t} needs common banks");
        }
    }

    #[test]
    fn sound5_len_matches_asset() {
        // ROM SOUND5LEN $009424 — byte length of SGSOUND5.BIN.
        assert_eq!(SOUND5_LEN, 0x009424);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/snd/SGSOUND5.BIN");
        if path.is_file() {
            let meta = std::fs::metadata(&path).expect("SGSOUND5.BIN meta");
            assert_eq!(meta.len(), SOUND5_LEN as u64);
        }
    }
}
