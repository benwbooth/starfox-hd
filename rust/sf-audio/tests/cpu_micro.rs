//! CPU micro cross-checks: run poked SPC-700 programs exercising tricky
//! semantics (DAA/DAS, MUL/DIV flags, wrapping direct-page ops) on BOTH the
//! native `sf-spc` engine and the C++ FFI oracle from an identical loaded
//! state, and assert the resulting registers/flags match.
//!
//! Each program sets up inputs, runs the op under test, then stores
//! A→$10, X→$11, Y→$12, PSW→$13 and halts (STOP). We compare $10..$14.
//!
//! Requires the `ffi-oracle` feature.
#![cfg(feature = "ffi-oracle")]

use sf_audio::{ffi_spc, native};

/// Build a minimal SPC file image: program at $0200, PC=$0200, timers/ROM off.
/// `pokes` are (addr, byte) written to RAM before run.
fn build_spc(program: &[u8], pokes: &[(u16, u8)]) -> Vec<u8> {
    const RAM_OFF: usize = 0x100;
    let mut f = vec![0u8; 0x10200];
    let sig = b"SNES-SPC700 Sound File Data v0.30\x1A\x1A";
    f[..sig.len()].copy_from_slice(sig);
    // registers
    let pc: u16 = 0x0200;
    f[37] = (pc & 0xFF) as u8; // pcl
    f[38] = (pc >> 8) as u8; // pch
    f[39] = 0; // a
    f[40] = 0; // x
    f[41] = 0; // y
    f[42] = 0; // psw
    f[43] = 0xEF; // sp
                  // RAM: control reg off (ROM + timers disabled)
    f[RAM_OFF + 0xF1] = 0x00;
    // program
    for (i, &b) in program.iter().enumerate() {
        f[RAM_OFF + 0x0200 + i] = b;
    }
    for &(addr, b) in pokes {
        f[RAM_OFF + addr as usize] = b;
    }
    f
}

/// Standard capture epilogue: store A/X/Y/PSW to $10..$13 then STOP.
fn with_capture(mut prog: Vec<u8>) -> Vec<u8> {
    prog.extend_from_slice(&[
        0xC4, 0x10, // MOV $10, A
        0xD8, 0x11, // MOV $11, X
        0xCB, 0x12, // MOV $12, Y
        0x0D, // PUSH PSW
        0xAE, // POP A
        0xC4, 0x13, // MOV $13, A
        0xFF, // STOP
    ]);
    prog
}

fn run_native(program: &[u8], pokes: &[(u16, u8)]) -> [u8; 4] {
    let data = build_spc(program, pokes);
    let mut spc = native::Spc::new();
    spc.debug_load_spc(&data).expect("native load_spc");
    for _ in 0..8 {
        spc.end_frame(20000);
    }
    let ram = spc.ram_mut();
    [ram[0x10], ram[0x11], ram[0x12], ram[0x13]]
}

fn run_ffi(program: &[u8], pokes: &[(u16, u8)]) -> [u8; 4] {
    let data = build_spc(program, pokes);
    let mut spc = ffi_spc::Spc::new();
    spc.debug_load_spc(&data).expect("ffi load_spc");
    for _ in 0..8 {
        spc.end_frame(20000);
    }
    let ram = spc.ram_mut();
    [ram[0x10], ram[0x11], ram[0x12], ram[0x13]]
}

/// `MOV A,#psw ; PUSH A ; POP PSW` — set all flags from a byte.
fn set_psw(psw: u8) -> Vec<u8> {
    vec![0xE8, psw, 0x2D, 0x8E]
}

fn check(name: &str, program: Vec<u8>, pokes: &[(u16, u8)]) {
    let prog = with_capture(program);
    let n = run_native(&prog, pokes);
    let o = run_ffi(&prog, pokes);
    assert_eq!(
        n, o,
        "{name}: native {n:02X?} != oracle {o:02X?} (A,X,Y,PSW)"
    );
}

#[test]
fn daa_das_cross_check() {
    // DAA (0xDF) and DAS (0xBE) over a grid of A and carry/half-carry PSW.
    let a_vals = [0x00u8, 0x09, 0x0A, 0x5F, 0x99, 0x9A, 0xFF, 0x1B, 0x66, 0xAB];
    // PSW bits: C=0x01, H=0x08 (independent of others here).
    let psw_vals = [0x00u8, 0x01, 0x08, 0x09];
    let mut count = 0;
    for &a in &a_vals {
        for &p in &psw_vals {
            for &op in &[0xDFu8, 0xBE] {
                let mut prog = set_psw(p);
                prog.extend_from_slice(&[0xE8, a]); // MOV A,#a
                prog.push(op); // DAA / DAS
                check("daa_das", prog, &[]);
                count += 1;
            }
        }
    }
    eprintln!("daa/das: {count} cases match oracle");
}

#[test]
fn mul_cross_check() {
    // MUL YA (0xCF): Y*A -> YA, sets N/Z from result high byte.
    let vals = [0x00u8, 0x01, 0x02, 0x10, 0x7F, 0x80, 0xFF, 0x55, 0xAA, 0x33];
    let mut count = 0;
    for &y in &vals {
        for &a in &vals {
            let prog = vec![0x8D, y, 0xE8, a, 0xCF]; // MOV Y,#y; MOV A,#a; MUL
            check("mul", prog, &[]);
            count += 1;
        }
    }
    eprintln!("mul: {count} cases match oracle");
}

#[test]
fn div_cross_check() {
    // DIV YA,X (0x9E): includes the overflow branch (Y >= X*2) and edge X.
    let ya_vals = [0x0000u16, 0x0001, 0x00FF, 0x0100, 0x1234, 0x7FFF, 0xFFFF, 0x8000, 0x0ABC];
    let x_vals = [0x01u8, 0x02, 0x03, 0x10, 0x7F, 0x80, 0xFF, 0x55];
    let mut count = 0;
    for &ya in &ya_vals {
        let y = (ya >> 8) as u8;
        let a = (ya & 0xFF) as u8;
        for &x in &x_vals {
            let prog = vec![0x8D, y, 0xE8, a, 0xCD, x, 0x9E]; // Y=y; A=a; X=x; DIV
            check("div", prog, &[]);
            count += 1;
        }
    }
    eprintln!("div: {count} cases match oracle");
}

#[test]
fn wrapping_dp_cross_check() {
    // Direct-page 16-bit ops that wrap the low byte from $FF to $00.
    // MOVW YA,$FF  (reads $FF low, $00 high)
    check(
        "movw_ya_wrap",
        vec![0xBA, 0xFF],
        &[(0xFF, 0x34), (0x00, 0x12)],
    );
    // MOVW $FF,YA  then read back via separate program is harder; instead test
    // INCW $FF (increments the 16-bit value at $FF/$00, wrapping page).
    // Set YA via MOV, store, INCW.
    check("incw_wrap", vec![0x3A, 0xFF], &[(0xFF, 0xFF), (0x00, 0x12)]);
    check("decw_wrap", vec![0x1A, 0xFF], &[(0xFF, 0x00), (0x00, 0x13)]);
    // ADDW YA,$FF and CMPW/SUBW with wrap.
    check(
        "addw_wrap",
        vec![0x8D, 0x00, 0xE8, 0x80, 0x7A, 0xFF], // Y=0;A=0x80; ADDW YA,$FF
        &[(0xFF, 0x90), (0x00, 0x7F)],
    );
    check(
        "subw_wrap",
        vec![0x8D, 0x40, 0xE8, 0x00, 0x9A, 0xFF], // Y=0x40;A=0x00; SUBW YA,$FF
        &[(0xFF, 0x01), (0x00, 0x20)],
    );
    check(
        "cmpw_wrap",
        vec![0x8D, 0x12, 0xE8, 0x34, 0x5A, 0xFF], // Y=0x12;A=0x34; CMPW YA,$FF
        &[(0xFF, 0x34), (0x00, 0x12)],
    );
    // For INCW/DECW/ADDW the result lives in memory; also capture the memory.
    eprintln!("wrapping dp: all cases match oracle");
}

#[test]
fn adc_sbc_flag_cross_check() {
    // ADC/SBC immediate over carry + operand grid (V/H/C/N/Z flags).
    let vals = [0x00u8, 0x01, 0x7F, 0x80, 0xFF, 0x40, 0xC0];
    let mut count = 0;
    for &a in &vals {
        for &b in &vals {
            for &carry in &[0x00u8, 0x01] {
                for &op in &[0x88u8, 0xA8] {
                    // ADC #imm / SBC #imm
                    let mut prog = set_psw(carry);
                    prog.extend_from_slice(&[0xE8, a, op, b]);
                    check("adc_sbc", prog, &[]);
                    count += 1;
                }
            }
        }
    }
    eprintln!("adc/sbc: {count} cases match oracle");
}
