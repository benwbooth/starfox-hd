//! Inspect the real boot-created opening controller and actor-list order.
//! This is an oracle-only diagnostic, not shipping scene state.
use sf_oracle::RetailMachine;

fn main() {
    let limit = std::env::args()
        .nth(1)
        .map(|arg| arg.parse::<u32>().unwrap())
        .unwrap_or(1800);
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut machine = RetailMachine::new(rom);
    let mut previous = None;
    let mut observed = 0;
    for frame in 0..limit {
        machine.tick_video_frames(0, 1).unwrap();
        let byte = |address: u16| machine.peek8(0x7E0000 + u32::from(address));
        let word =
            |address: u16| u16::from_le_bytes([byte(address), byte(address.wrapping_add(1))]);
        let player = word(0x12C3);
        let player_two = word(sf2_game::object::PLAYER_TWO);
        let auxiliary = word(player.wrapping_add(0x2B));
        let script = word(auxiliary.wrapping_add(0x6C13));
        let elapsed = word(auxiliary.wrapping_add(0x6C16));
        let signature = (byte(0xC4), player, auxiliary, script, elapsed);
        if previous == Some(signature) {
            continue;
        }
        previous = Some(signature);
        if script != 0xBEDF {
            continue;
        }
        let mut actors = Vec::new();
        let mut cursor = word(0x12A8);
        while cursor != 0 {
            assert!(
                !actors.iter().any(|(id, _, _, _)| *id == cursor),
                "active-list cycle"
            );
            actors.push((
                cursor,
                word(cursor.wrapping_add(0x2B)),
                word(cursor.wrapping_add(0x19)),
                byte(cursor.wrapping_add(0x1B)),
            ));
            assert!(actors.len() <= 64);
            cursor = word(cursor);
        }
        println!("video={frame} clock={} player={player:04X} player_two={player_two:04X} aux={auxiliary:04X} elapsed={elapsed} cue={} saved_cursor={:04X} player_strategy={:02X}:{:04X} actors={actors:04X?}", byte(0xC4), byte(0x1D72), word(0x1942), byte(player.wrapping_add(0x1B)), word(player.wrapping_add(0x19)));
        observed += 1;
        if observed == 20 {
            return;
        }
    }
    assert!(
        observed > 0,
        "opening script was not reached within {limit} video frames"
    );
}
