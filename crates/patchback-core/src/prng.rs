//! Deterministic pseudo-random numbers for the dataset generator and the fit's restarts
//! (DESIGN.md § 7.1, § 9.2): `xoshiro256**` seeded through `SplitMix64`, both in `std` only
//! and bit-exact on every platform, so a seed names one sequence for good.
//!
//! The algorithms are Blackman and Vigna's (<https://prng.di.unimi.it/>): `SplitMix64` is the
//! seeder they recommend, `xoshiro256**` their all-purpose generator. `tests/prng.rs` holds the
//! published reference outputs of both, `SplitMix64` from the seed 0 and `xoshiro256**` from the
//! state `[1, 2, 3, 4]`.

/// The generator's name as a dataset records it (`PrngAlgorithm` in `spec/domains/dataset.yaml`).
pub const NAME: &str = "xoshiro256**";

/// SplitMix64: 64 bits of state, one 64-bit output per step. It seeds [`Xoshiro256StarStar`] and
/// derives one seed per patch from a dataset's seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// The generator whose state is `seed`.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// xoshiro256**: 256 bits of state, period 2^256 - 1, one 64-bit output per step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Xoshiro256StarStar {
    s: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Seeded from `seed` through four consecutive outputs of [`SplitMix64`], as its authors
    /// recommend.
    pub fn new(seed: u64) -> Self {
        let mut mix = SplitMix64::new(seed);
        Self::from_state([
            mix.next_u64(),
            mix.next_u64(),
            mix.next_u64(),
            mix.next_u64(),
        ])
    }

    /// The generator whose state is `s`. The all-zero state, which the generator would never
    /// leave, is replaced by the state [`new(0)`](Self::new) gives.
    pub fn from_state(s: [u64; 4]) -> Self {
        if s == [0; 4] {
            return Self::new(0);
        }
        Self { s }
    }

    /// The current state.
    pub const fn state(&self) -> [u64; 4] {
        self.s
    }

    /// The next output.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// A value in `[0, 1)` from the top 53 bits of the next output: one of 2^53 equally spaced
    /// values, each exact in an `f64`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}
