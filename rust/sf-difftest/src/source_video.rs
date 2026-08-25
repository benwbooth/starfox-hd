use crate::TraceError;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SOURCE_FRAME_WIDTH: usize = 256;
pub const SOURCE_FRAME_HEIGHT: usize = 224;
pub const SOURCE_FRAME_RGB_BYTES: usize = SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT * 3;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceVideoDivergence {
    pub sequence: u64,
    pub retail_video_frame: u64,
    pub differing_pixels: usize,
    pub first_position: [usize; 2],
    pub retail_color: [u8; 3],
    pub native_color: [u8; 3],
}

/// Read a binary PPM and retain the 256-by-224 source playfield beginning at
/// `crop_top`. The crop is a presentation boundary only; no source-machine
/// storage enters native game state.
pub fn read_source_rgb_ppm(path: impl AsRef<Path>, crop_top: usize) -> Result<Vec<u8>, TraceError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    let mut newline_count = 0;
    let header_end = bytes
        .iter()
        .position(|byte| {
            if *byte == b'\n' {
                newline_count += 1;
            }
            newline_count == 3
        })
        .map(|position| position + 1)
        .ok_or_else(|| TraceError::new(format!("{}: incomplete PPM header", path.display())))?;
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|error| {
        TraceError::new(format!("{}: invalid PPM header: {error}", path.display()))
    })?;
    let mut fields = header.split_ascii_whitespace();
    if fields.next() != Some("P6") {
        return Err(TraceError::new(format!(
            "{}: expected binary P6 PPM",
            path.display()
        )));
    }
    let width = parse_dimension(fields.next(), path, "width")?;
    let height = parse_dimension(fields.next(), path, "height")?;
    if fields.next() != Some("255") || fields.next().is_some() {
        return Err(TraceError::new(format!(
            "{}: expected 8-bit PPM depth",
            path.display()
        )));
    }
    if width != SOURCE_FRAME_WIDTH || height < crop_top + SOURCE_FRAME_HEIGHT {
        return Err(TraceError::new(format!(
            "{}: image is {width}x{height}; source crop requires {}x{} at row {crop_top}",
            path.display(),
            SOURCE_FRAME_WIDTH,
            SOURCE_FRAME_HEIGHT,
        )));
    }
    let pixels = &bytes[header_end..];
    let expected_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| TraceError::new(format!("{}: PPM dimensions overflow", path.display())))?;
    if pixels.len() != expected_bytes {
        return Err(TraceError::new(format!(
            "{}: PPM payload has {} bytes; expected {expected_bytes}",
            path.display(),
            pixels.len()
        )));
    }
    let row_bytes = SOURCE_FRAME_WIDTH * 3;
    let crop_start = crop_top * row_bytes;
    let crop_end = crop_start + SOURCE_FRAME_RGB_BYTES;
    Ok(pixels[crop_start..crop_end].to_vec())
}

fn parse_dimension(field: Option<&str>, path: &Path, name: &str) -> Result<usize, TraceError> {
    field
        .ok_or_else(|| TraceError::new(format!("{}: missing PPM {name}", path.display())))?
        .parse::<usize>()
        .map_err(|error| {
            TraceError::new(format!("{}: invalid PPM {name}: {error}", path.display()))
        })
}

pub fn hash_rgb(rgb: &[u8]) -> u64 {
    rgb.iter().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01B3)
    })
}

pub fn write_source_rgb_ppm(path: impl AsRef<Path>, rgb: &[u8]) -> Result<(), TraceError> {
    let path = path.as_ref();
    if rgb.len() != SOURCE_FRAME_RGB_BYTES {
        return Err(TraceError::new(format!(
            "source frame has {} RGB bytes; expected {SOURCE_FRAME_RGB_BYTES}",
            rgb.len()
        )));
    }
    let mut ppm = format!("P6\n{SOURCE_FRAME_WIDTH} {SOURCE_FRAME_HEIGHT}\n255\n").into_bytes();
    ppm.extend_from_slice(rgb);
    std::fs::write(path, ppm)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))
}

pub fn compare_source_rgb(
    sequence: u64,
    retail_video_frame: u64,
    retail: &[u8],
    native: &[u8],
) -> Result<Option<SourceVideoDivergence>, TraceError> {
    for (name, pixels) in [("retail", retail), ("native", native)] {
        if pixels.len() != SOURCE_FRAME_RGB_BYTES {
            return Err(TraceError::new(format!(
                "{name} source frame has {} RGB bytes; expected {SOURCE_FRAME_RGB_BYTES}",
                pixels.len()
            )));
        }
    }
    let mut differing_pixels = 0;
    let mut first = None;
    for (index, (expected, actual)) in retail
        .chunks_exact(3)
        .zip(native.chunks_exact(3))
        .enumerate()
    {
        if expected == actual {
            continue;
        }
        differing_pixels += 1;
        first.get_or_insert((
            index,
            [expected[0], expected[1], expected[2]],
            [actual[0], actual[1], actual[2]],
        ));
    }
    Ok(first.map(
        |(index, retail_color, native_color)| SourceVideoDivergence {
            sequence,
            retail_video_frame,
            differing_pixels,
            first_position: [index % SOURCE_FRAME_WIDTH, index / SOURCE_FRAME_WIDTH],
            retail_color,
            native_color,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_source_frames_have_no_divergence() {
        let rgb = vec![7; SOURCE_FRAME_RGB_BYTES];
        assert_eq!(compare_source_rgb(2, 19, &rgb, &rgb).unwrap(), None);
    }

    #[test]
    fn reports_the_earliest_pixel_and_complete_difference_count() {
        let retail = vec![0; SOURCE_FRAME_RGB_BYTES];
        let mut native = retail.clone();
        native[(SOURCE_FRAME_WIDTH + 3) * 3..(SOURCE_FRAME_WIDTH + 3) * 3 + 3]
            .copy_from_slice(&[1, 2, 3]);
        native[(SOURCE_FRAME_WIDTH + 5) * 3] = 4;
        assert_eq!(
            compare_source_rgb(4, 27, &retail, &native).unwrap(),
            Some(SourceVideoDivergence {
                sequence: 4,
                retail_video_frame: 27,
                differing_pixels: 2,
                first_position: [3, 1],
                retail_color: [0, 0, 0],
                native_color: [1, 2, 3],
            })
        );
    }

    #[test]
    fn rejects_incomplete_source_frames() {
        let error = compare_source_rgb(0, 0, &[0; 3], &[0; 3]).unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("expected {SOURCE_FRAME_RGB_BYTES}")));
    }
}
