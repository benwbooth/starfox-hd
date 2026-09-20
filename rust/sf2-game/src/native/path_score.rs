//! Selected-player score retained across scene publication. Pickup rewards
//! saturate only its low word; they do not carry into the retained high byte.

/// Three-byte score copied into the player at $06:82A4 and published back at
/// $03:C6F7. The results service at $0D:F792 adds the published value to its
/// other score component. This is distinct from a whole-mission summary.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerScore(u32);

impl PlayerScore {
    pub const fn from_parts(low_points: u16, high_points: u8) -> Self {
        Self(low_points as u32 | ((high_points as u32) << u16::BITS))
    }

    pub const fn points(self) -> u32 {
        self.0
    }

    /// $7F:A9F3. Addition clamps the low word at 65535 independently of
    /// the high byte. Ordinary full-score saturating arithmetic is wrong.
    pub(crate) fn award_path_points(&mut self, amount: u16) {
        let low_points = (self.0 as u16).saturating_add(amount);
        self.0 = (self.0 & !u32::from(u16::MAX)) | u32::from(low_points);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_low_score_and_reward_width_obeys_the_word_saturation_boundary() {
        for value in 0..=u16::MAX {
            for (old, amount) in [(value, 0), (value, 1), (value, 100), (value, 32768),
                (value, u16::MAX), (0, value), (1, value), (32768, value), (u16::MAX, value)] {
                let mut score = PlayerScore::from_parts(old, 173);
                let sum = u32::from(old) + u32::from(amount);
                let expected_low = if sum > 65535 { 65535 } else { sum };
                score.award_path_points(amount);
                assert_eq!(score.points(), 173 * 65536 + expected_low);
            }
        }
    }

    #[test]
    fn every_high_byte_is_preserved_through_overflow_and_repeated_awards() {
        for high in 0..=u8::MAX {
            let mut score = PlayerScore::from_parts(65534, high);
            assert_eq!(score.points(), u32::from(high) * 65536 + 65534);
            for amount in [1, 1, 0, 100, 65535] {
                score.award_path_points(amount);
                assert_eq!(score, PlayerScore::from_parts(65535, high));
            }
        }
    }
}
