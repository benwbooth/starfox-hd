//! Read-only original-code probe for entropy draws within an opening traversal.
//! SF2_OPENING_VISIT_UPDATE selects the one-based traversal; 101 is the logo
//! release, where the refresh can interrupt a glyph between its random draws.
use sf_oracle::RetailMachine;

fn main() {
    const CONTROLLER: u32 = 0x0DBCCF;
    const WRAM: u32 = 0x7E0000;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut machine =
        RetailMachine::new(std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).unwrap());
    let update: u32 = std::env::var("SF2_OPENING_VISIT_UPDATE")
        .map(|value| value.parse().expect("positive traversal number"))
        .unwrap_or(101);
    assert!(update > 0);
    for _ in 0..update {
        assert!(machine
            .tick_until_cpu_execution(0, CONTROLLER, 240)
            .unwrap());
    }
    let random = |machine: &RetailMachine| -> [u8; 4] {
        std::array::from_fn(|i| machine.peek8(WRAM + 0xE0 + i as u32))
    };
    println!("update={update} initial random={:?}", random(&machine));
    let markers = [
        CONTROLLER, 0x7F3519, 0x7F3565, 0x7F3531, 0x7F357D, 0x7F058F, 0x7F7BD4,
    ];
    let mut visits = 0;
    loop {
        let pc = machine
            .tick_until_cpu_execution_any(0, &markers, 240)
            .unwrap()
            .unwrap();
        let current = machine.peek16(WRAM + 0x12C7);
        if [0x7F3531, 0x7F357D].contains(&pc) {
            visits += 1;
        }
        println!(
            "pc={pc:06X} visits={visits} x={:04X} current={current:04X} slot={:?} random={:?}",
            machine.cpu_x(),
            sf2_game::object::object_index(current),
            random(&machine)
        );
        if pc == CONTROLLER {
            break;
        }
    }
}
