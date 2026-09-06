//! Native decoder for the backward-compressed artwork streams.
//!
//! The stream ends with its output length and a sentinel-terminated bit word.
//! Literal bytes and overlapping back-references fill the output from the end.
//! This is data decoding only: no CPU, GSU, or runtime scene state is involved.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    OutputOverrun,
    InvalidReference,
}

struct Bits<'a> {
    data: &'a [u8],
    cursor: usize,
    word: u32,
}

impl Bits<'_> {
    fn bit(&mut self) -> Result<u32, DecodeError> {
        let mut bit = self.word & 1;
        self.word >>= 1;
        if self.word == 0 {
            self.cursor = self.cursor.checked_sub(4).ok_or(DecodeError::Truncated)?;
            let word =
                u32::from_be_bytes(self.data[self.cursor..self.cursor + 4].try_into().unwrap());
            bit = word & 1;
            self.word = (word >> 1) | 0x8000_0000;
        }
        Ok(bit)
    }

    fn take(&mut self, count: usize) -> Result<usize, DecodeError> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | self.bit()? as usize;
        }
        Ok(value)
    }
}

/// Decode a stream whose end is the end of `data`. Earlier unrelated bytes
/// may precede it, as the source format carries no compressed-length field.
/// Invalid/truncated input returns an error; output is bounded by the stored
/// 16-bit uncompressed length.
pub fn decode_artwork(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let start = data.len().checked_sub(8).ok_or(DecodeError::Truncated)?;
    let length = usize::from(u16::from_be_bytes(
        data[data.len() - 2..].try_into().unwrap(),
    ));
    let mut bits = Bits {
        data,
        cursor: start,
        word: u32::from_be_bytes(data[start..start + 4].try_into().unwrap()),
    };
    let mut output = vec![0; length];
    let mut cursor = length;
    while cursor != 0 {
        let mut literals = bits.take(3)?;
        if literals == 7 {
            literals = if bits.bit()? == 0 {
                bits.take(4)? + 7
            } else {
                let count = bits.take(10)?;
                if count == 0 {
                    bits.take(18)?
                } else {
                    count
                }
            };
        }
        if literals > cursor {
            return Err(DecodeError::OutputOverrun);
        }
        for _ in 0..literals {
            cursor -= 1;
            output[cursor] = bits.take(8)? as u8;
        }
        if cursor == 0 {
            break;
        }
        let command = bits.take(2)?;
        let count = match command {
            0 => 2,
            1 => 3,
            2 => 4,
            _ => match bits.take(2)? {
                0 => 5,
                1 => 6,
                2 => bits.take(2)? + 7,
                _ => bits.take(8)?,
            },
        };
        let width = match command {
            0 => 8,
            1 => {
                if bits.bit()? != 0 {
                    8
                } else {
                    14
                }
            }
            _ => {
                if bits.bit()? == 0 {
                    16
                } else if bits.bit()? != 0 {
                    8
                } else {
                    12
                }
            }
        };
        let offset = bits.take(width)?;
        if count == 0 || count > cursor {
            return Err(DecodeError::OutputOverrun);
        }
        if offset == 0 || cursor - 1 + offset >= output.len() {
            return Err(DecodeError::InvalidReference);
        }
        for _ in 0..count {
            cursor -= 1;
            output[cursor] = output[cursor + offset];
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(bits: &mut Vec<u32>, value: usize, width: usize) {
        bits.extend((0..width).rev().map(|shift| ((value >> shift) & 1) as u32));
    }

    fn stream(bits: &[u32], length: u16) -> Vec<u8> {
        let first = bits.len() % 32;
        let pack = |chunk: &[u32]| {
            chunk
                .iter()
                .enumerate()
                .fold(0u32, |word, (index, bit)| word | (bit << index))
        };
        let mut data = Vec::new();
        for chunk in bits[first..].chunks_exact(32).rev() {
            data.extend_from_slice(&pack(chunk).to_be_bytes());
        }
        data.extend_from_slice(&(pack(&bits[..first]) | (1 << first)).to_be_bytes());
        data.extend_from_slice(&[0, 0]);
        data.extend_from_slice(&length.to_be_bytes());
        data
    }

    #[test]
    fn literal_runs_cross_refill_boundaries_and_extended_lengths() {
        for count in [1, 6, 7, 22, 23, 1023, 1024] {
            let mut bits = Vec::new();
            field(&mut bits, count.min(7), 3);
            if count >= 7 {
                if count <= 22 {
                    field(&mut bits, 0, 1);
                    field(&mut bits, count - 7, 4);
                } else {
                    field(&mut bits, 1, 1);
                    field(&mut bits, if count < 1024 { count } else { 0 }, 10);
                    if count >= 1024 {
                        field(&mut bits, count, 18);
                    }
                }
            }
            let expected: Vec<_> = (0..count).map(|i| i as u8).collect();
            for &byte in expected.iter().rev() {
                field(&mut bits, byte as usize, 8);
            }
            assert_eq!(decode_artwork(&stream(&bits, count as u16)), Ok(expected));
        }
    }

    #[test]
    fn backward_references_overlap_previously_decoded_output() {
        let mut bits = Vec::new();
        for (value, width) in [(1, 3), (65, 8), (2, 2), (1, 1), (1, 1), (1, 8)] {
            field(&mut bits, value, width);
        }
        assert_eq!(decode_artwork(&stream(&bits, 5)), Ok(b"AAAAA".to_vec()));
        assert_eq!(
            decode_artwork(&stream(&bits, 3)),
            Err(DecodeError::OutputOverrun)
        );
    }

    #[test]
    fn malformed_streams_are_bounded_errors() {
        for size in 0..8 {
            assert_eq!(decode_artwork(&vec![0; size]), Err(DecodeError::Truncated));
        }
        let mut bits = Vec::new();
        for (value, width) in [(0, 3), (0, 2), (1, 8)] {
            field(&mut bits, value, width);
        }
        assert_eq!(
            decode_artwork(&stream(&bits, 2)),
            Err(DecodeError::InvalidReference)
        );
        assert_eq!(
            decode_artwork(&[0, 0, 0, 1, 0, 0, 0, 1]),
            Err(DecodeError::Truncated)
        );
    }
}
