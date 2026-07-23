//! Differential test harness: compares per-tick state dumps between the C
//! oracle (`build/starfox-hd`, env `SF_STATE_DUMP=<path>`) and the Rust port.
//!
//! Dump format (one line per record, whitespace-separated):
//!   `T <tick> <pad> <state> <ndraw>`  — tick header
//!   `E <shape> <x> <y> <z> <rx> <ry> <rz> <flags>` — one draw entry
//!
//! Usage: `sf-difftest <oracle.dump> <candidate.dump> [--max-diffs N]`
//! Exit code 0 = identical, 1 = divergence (first divergent tick reported),
//! 2 = usage/parse error.

use std::fmt::Write as _;
use std::fs;
use std::process::ExitCode;

#[derive(Debug, Default, PartialEq, Eq)]
struct Tick {
    tick: u64,
    pad: u16,
    state: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, Eq)]
struct Entry {
    shape: u16,
    x: i32,
    y: i32,
    z: i32,
    rx: i16,
    ry: i16,
    rz: i16,
    flags: u16,
}

fn parse(path: &str) -> Result<Vec<Tick>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut ticks: Vec<Tick> = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        let mut it = line.split_ascii_whitespace();
        match it.next() {
            Some("T") => {
                let mut next = |what: &str| -> Result<i64, String> {
                    it.next()
                        .ok_or_else(|| format!("{path}:{}: missing {what}", lineno + 1))?
                        .parse::<i64>()
                        .map_err(|e| format!("{path}:{}: bad {what}: {e}", lineno + 1))
                };
                ticks.push(Tick {
                    tick: next("tick")? as u64,
                    pad: next("pad")? as u16,
                    state: next("state")? as u32,
                    entries: Vec::new(),
                });
                // trailing ndraw is advisory; entry lines are authoritative
            }
            Some("E") => {
                let t = ticks
                    .last_mut()
                    .ok_or_else(|| format!("{path}:{}: E before T", lineno + 1))?;
                let mut next = |what: &str| -> Result<i64, String> {
                    it.next()
                        .ok_or_else(|| format!("{path}:{}: missing {what}", lineno + 1))?
                        .parse::<i64>()
                        .map_err(|e| format!("{path}:{}: bad {what}: {e}", lineno + 1))
                };
                t.entries.push(Entry {
                    shape: next("shape")? as u16,
                    x: next("x")? as i32,
                    y: next("y")? as i32,
                    z: next("z")? as i32,
                    rx: next("rx")? as i16,
                    ry: next("ry")? as i16,
                    rz: next("rz")? as i16,
                    flags: next("flags")? as u16,
                });
            }
            _ => {} // ignore unknown/comment lines
        }
    }
    Ok(ticks)
}

fn describe_tick(t: &Tick) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "tick {} pad {:#06x} state {} entries {}",
        t.tick,
        t.pad,
        t.state,
        t.entries.len()
    );
    s
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paths = Vec::new();
    let mut max_diffs = 10usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--max-diffs" => {
                i += 1;
                max_diffs = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(10);
            }
            p => paths.push(p.to_string()),
        }
        i += 1;
    }
    if paths.len() != 2 {
        eprintln!("usage: sf-difftest <oracle.dump> <candidate.dump> [--max-diffs N]");
        return ExitCode::from(2);
    }

    let (oracle, cand) = match (parse(&paths[0]), parse(&paths[1])) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("parse error: {e}");
            return ExitCode::from(2);
        }
    };

    let common = oracle.len().min(cand.len());
    let mut diffs = 0usize;
    for k in 0..common {
        let (a, b) = (&oracle[k], &cand[k]);
        if a != b {
            if diffs == 0 {
                println!("FIRST DIVERGENCE at record {k}:");
                println!("  oracle:    {}", describe_tick(a));
                println!("  candidate: {}", describe_tick(b));
                let n = a.entries.len().min(b.entries.len());
                for e in 0..n {
                    if a.entries[e] != b.entries[e] {
                        println!("  entry {e}: oracle {:?}", a.entries[e]);
                        println!("  entry {e}: cand   {:?}", b.entries[e]);
                        break;
                    }
                }
            }
            diffs += 1;
            if diffs >= max_diffs {
                break;
            }
        }
    }
    if oracle.len() != cand.len() {
        println!(
            "length mismatch: oracle {} ticks, candidate {} ticks",
            oracle.len(),
            cand.len()
        );
        diffs += 1;
    }

    if diffs == 0 {
        println!("OK: {} ticks identical", common);
        ExitCode::SUCCESS
    } else {
        println!("{diffs}+ divergent ticks (of {common} compared)");
        ExitCode::from(1)
    }
}
