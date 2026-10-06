//! Native decoder against every artwork stream reachable from authored scene
//! load requests. The fixture contains source operands, never expected pixels.

use sf2_data::compression::decode_artwork;
use sf_oracle::gsu::Gsu;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

include!("fixtures/sf2_scene_artwork_packets.rs");

fn file_offset(address: u32) -> usize {
    ((address >> 16) as usize) * 0x8000 + (address as usize & 0x7FFF)
}

#[test]
fn scene_artwork_roster_retains_exact_original_loader_bytes() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut digest = Sha256::new();
    for &(address, length) in SOURCE_SPANS {
        let offset = file_offset(address);
        digest.update(&rom[offset..offset + length]);
    }
    assert_eq!(format!("{:x}", digest.finalize()), SOURCE_SHA256);
    for &(call, helper, end, destination, transfer, length) in PACKETS {
        let offset = file_offset(call);
        assert_eq!(rom[offset], 0x20, "original JSR at {call:06X}");
        assert_eq!(&rom[offset + 1..offset + 3], helper.to_le_bytes());
        assert_eq!(&rom[offset + 3..offset + 6], &end.to_le_bytes()[..3]);
        assert_eq!(&rom[offset + 6..offset + 8], destination.to_le_bytes());
        assert_eq!(
            &rom[offset + 8..offset + 10],
            (transfer as u16).to_le_bytes()
        );
        assert_eq!(
            u16::from_be_bytes(
                rom[file_offset(end) - 2..file_offset(end)]
                    .try_into()
                    .unwrap()
            ),
            length as u16
        );
    }
}

#[test]
fn all_scene_artwork_matches_original_decompression_in_both_loader_buffers() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut tested = BTreeSet::new();
    for &(call, helper, end, _, transfer, length) in PACKETS {
        let destination = if helper == 0xD674 { 0x3B50 } else { 0x5B50 };
        if !tested.insert((end, destination)) {
            continue;
        }
        let decoded = decode_artwork(&rom[..file_offset(end)])
            .unwrap_or_else(|error| panic!("artwork for call {call:06X}: {error:?}"));
        assert_eq!(decoded.len(), length);
        assert!(transfer <= length);
        let mut source = Gsu::new(rom.clone());
        source.ram.fill(0xA5);
        for (offset, value) in [
            (0x2C, destination as u16),
            (0x68, end as u16),
            (0x6A, (end >> 16) as u16),
            (0xA2, 0),
        ] {
            source.ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        source.run_with_limit(1, 0xD9FF, 5_000_000);
        assert!(
            source.last_run_steps < 5_000_000,
            "source decode did not finish"
        );
        assert_eq!(
            decoded,
            source.ram[destination..destination + length],
            "stream {end:06X}"
        );
        assert_eq!(source.ram[destination - 1], 0xA5);
        assert_eq!(source.ram[destination + length], 0xA5);
    }
    assert_eq!(tested.len(), 32);
}
