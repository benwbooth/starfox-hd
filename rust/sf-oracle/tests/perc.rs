//! Differential tests for the `percNN` scaling subroutines (val * NN/128-ish)
//! against the ROM. These are used everywhere — camera follow (perc87/75),
//! velocity scaling (perc62), etc. — so a sign/rounding error here is systemic.
//! ABI: A (16-bit) = value in, A = result out.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

fn rom_perc(rom: &[u8], addr: u32, val: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    let exit = call(
        &mut bus,
        addr,
        &Entry { a: val as u16, p: 0x00, ..Default::default() }, // 16-bit A
    );
    exit.c as i16
}

#[test]
fn perc_functions_match_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM not present");
        return;
    };
    let table: [(&str, fn(i16) -> i16); 5] = [
        ("PERC56A_L", sf_strat::common::strat_perc56),
        ("PERC62A_L", sf_strat::common::strat_perc62),
        ("PERC75A_L", sf_strat::common::strat_perc75),
        ("PERC87A_L", sf_strat::common::strat_perc87),
        ("PERC93A_L", sf_strat::common::strat_perc93),
    ];
    let vals = [
        0i16, 1, -1, 100, -100, 255, -255, 500, -500, 4096, -4096, 12345, -12345, 32000, -32000,
    ];
    let mut bad = 0;
    for (name, rust_fn) in table {
        let Some(&addr) = syms.get(name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        let mut n = 0;
        for &v in &vals {
            let rom_r = rom_perc(&rom, addr, v);
            let rust_r = rust_fn(v);
            if rom_r != rust_r {
                bad += 1;
                eprintln!("  {name}({v}): ROM={rom_r} RUST={rust_r}  DIFF");
            }
            n += 1;
        }
        eprintln!("{name}: {n} values checked");
    }
    assert_eq!(bad, 0, "{bad} perc value mismatches ROM vs Rust");
}
