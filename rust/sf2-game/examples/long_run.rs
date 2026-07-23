use sf2_data::map::SCRIPT_ROOTS;
use sf2_game::oracle_compat::Game;

fn main() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repo.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let selected_root = arguments
        .iter()
        .enumerate()
        .find(|(index, value)| {
            value.as_str() != "--release"
                && value.as_str() != "--playable"
                && value.as_str() != "--autoplay"
                && value.as_str() != "--dump-objects"
                && arguments.get(index.wrapping_sub(1)).map(String::as_str) != Some("--frames")
                && value.as_str() != "--frames"
        })
        .map(|(_, value)| value.parse::<usize>().expect("root index"));
    let release_phases = arguments.iter().any(|value| value == "--release");
    let playable = arguments.iter().any(|value| value == "--playable");
    let autoplay = arguments.iter().any(|value| value == "--autoplay");
    let dump_objects = arguments.iter().any(|value| value == "--dump-objects");
    let frame_limit = arguments
        .iter()
        .position(|value| value == "--frames")
        .and_then(|index| arguments.get(index + 1))
        .map(|value| value.parse::<usize>().expect("frame count"))
        .unwrap_or(20_000);
    for root in 0..SCRIPT_ROOTS.len() {
        if selected_root.is_some_and(|selected| selected != root) {
            continue;
        }
        let mut game = if playable {
            Game::from_playable_root(rom.clone(), root).expect("valid playable root")
        } else {
            Game::from_root(rom.clone(), root).expect("valid generated root")
        };
        let mut result = Ok(());
        for frame in 0..frame_limit {
            // Alternating edges accept every recovered external phase gate
            // without manufacturing a direct VM continuation.
            let pad = if (release_phases && frame & 1 == 0)
                || (autoplay && frame <= 400 && frame % 60 == 0)
            {
                sf_core::pad::START
            } else if autoplay && frame > 400 && frame % 8 < 4 {
                sf_core::pad::Y
            } else {
                0
            };
            result = game.tick(pad);
            if selected_root.is_some() && (frame < 16 || result.is_err()) {
                let cursor = game.map_cursor();
                eprintln!(
                    "frame={} cursor={:02X}:{:04X} counter={:04X} objects={} result={result:?}",
                    game.frame,
                    cursor.bank,
                    cursor.address,
                    game.map_counter(),
                    game.active_objects().len(),
                );
            }
            if result.is_err() {
                break;
            }
        }
        let cursor = game.map_cursor();
        let camera = game.camera();
        println!(
            "root={root:02} start={:02X}:{:04X} frame={} cursor={:02X}:{:04X} counter={:04X} objects={} draws={} camera=({},{},{})/({},{},{}) messages={} result={result:?}",
            SCRIPT_ROOTS[root].address.bank,
            SCRIPT_ROOTS[root].address.address,
            game.frame,
            cursor.bank,
            cursor.address,
            game.map_counter(),
            game.active_objects().len(),
            game.render_records().map_or(0, |records| records.len()),
            camera.x,
            camera.y,
            camera.z,
            camera.rotation_x,
            camera.rotation_y,
            camera.rotation_z,
            game.messages.len(),
        );
        for anchor in [0x033Fu16, 0x037E] {
            eprintln!(
                "  anchor={anchor:04X} xyz=({},{},{}) rot=({:02X},{:02X},{:02X})",
                game.memory.read_word(anchor + 0x0C) as i16,
                game.memory.read_word(anchor + 0x0E) as i16,
                game.memory.read_word(anchor + 0x10) as i16,
                game.memory.read_byte(anchor + 0x12),
                game.memory.read_byte(anchor + 0x14),
                game.memory.read_byte(anchor + 0x16),
            );
        }
        if result.is_err() || dump_objects {
            for object in game.active_objects() {
                let strategy = u32::from(game.memory.read_byte(object + 0x19))
                    | (u32::from(game.memory.read_byte(object + 0x1A)) << 8)
                    | (u32::from(game.memory.read_byte(object + 0x1B)) << 16);
                eprintln!(
                    "  object={object:04X} shape={:04X} xyz=({},{},{}) rot=({:02X},{:02X},{:02X}) path={:04X} strategy={strategy:06X} hp/ap={:02X}/{:02X} vars27/28={:02X}/{:02X} flags={:02X}/{:02X}/{:02X}/{:02X}",
                    game.memory.read_word(object + 0x04),
                    game.memory.read_word(object + 0x0C) as i16,
                    game.memory.read_word(object + 0x0E) as i16,
                    game.memory.read_word(object + 0x10) as i16,
                    game.memory.read_byte(object + 0x12),
                    game.memory.read_byte(object + 0x14),
                    game.memory.read_byte(object + 0x16),
                    game.memory.read_word(object + 0x2B),
                    game.memory.read_byte(object + 0x2D),
                    game.memory.read_byte(object + 0x2E),
                    game.memory.read_byte(object + 0x27),
                    game.memory.read_byte(object + 0x28),
                    game.memory.read_byte(object + 0x09),
                    game.memory.read_byte(object + 0x21),
                    game.memory.read_byte(object + 0x23),
                    game.memory.read_byte(object + 0x25),
                );
            }
        }
    }
}
