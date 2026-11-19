use serde::{Deserialize, Serialize};

/// Small deterministic SplitMix64 stream for simulation use.
///
/// This is not cryptographic. Its algorithm is part of the replay/save
/// compatibility contract and must not be silently changed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    pub fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// Returns a value in a non-empty range starting at zero.
    pub fn range(&mut self, upper_exclusive: u64) -> Option<u64> {
        if upper_exclusive == 0 {
            return None;
        }

        let threshold = upper_exclusive.wrapping_neg() % upper_exclusive;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return Some(value % upper_exclusive);
            }
        }
    }
}
