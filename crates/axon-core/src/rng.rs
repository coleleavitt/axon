/// A small, dependency-free, seedable PRNG (SplitMix64) for reproducible
/// stochastic routing.
///
/// The core deliberately pulls in no `rand` crate: an agent SDK must be
/// replayable — a bug report, a regression test, and CI all require that the
/// same goal plus the same seed yields the same trajectory — so every source of
/// randomness is seeded and deterministic.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

/// The seed a default [`Runtime`](crate::Runtime) uses, so even unconfigured
/// runs are reproducible.
pub const DEFAULT_SEED: u64 = 0x5EED_C0DE_1234_ABCD;

impl Rng {
    pub const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next pseudo-random `u64` (SplitMix64).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A pseudo-random `f32` uniformly distributed in `[0.0, 1.0)`.
    pub fn next_f32(&mut self) -> f32 {
        // The top 24 bits fill the f32 mantissa exactly, giving a uniform value.
        let bits = (self.next_u64() >> 40) as u32;
        bits as f32 / (1u32 << 24) as f32
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::seeded(DEFAULT_SEED)
    }
}
