//! The reference outputs of `SplitMix64` and `xoshiro256**` (<https://prng.di.unimi.it/>), and
//! the bounds of `next_f64`.

use patchback_core::prng::{NAME, SplitMix64, Xoshiro256StarStar};

/// The first five outputs of SplitMix64 from the seed 0, as the reference C code gives them.
const SPLITMIX64_FROM_0: [u64; 5] = [
    0xE220_A839_7B1D_CDAF,
    0x6E78_9E6A_A1B9_65F4,
    0x06C4_5D18_8009_454F,
    0xF88B_B8A8_724C_81EC,
    0x1B39_896A_51A8_749B,
];

/// The first four outputs of xoshiro256** from the state `[1, 2, 3, 4]`, as the reference C code
/// gives them.
const XOSHIRO_FROM_1234: [u64; 4] = [11_520, 0, 1_509_978_240, 1_215_971_899_390_074_240];

#[test]
fn splitmix64_from_seed_0_gives_the_reference_outputs() {
    let mut mix = SplitMix64::new(0);
    let outputs: Vec<u64> = (0..5).map(|_| mix.next_u64()).collect();
    assert_eq!(outputs, SPLITMIX64_FROM_0);
}

#[test]
fn xoshiro256starstar_from_the_state_1_2_3_4_gives_the_reference_outputs() {
    let mut prng = Xoshiro256StarStar::from_state([1, 2, 3, 4]);
    let outputs: Vec<u64> = (0..4).map(|_| prng.next_u64()).collect();
    assert_eq!(outputs, XOSHIRO_FROM_1234);
}

#[test]
fn a_seed_becomes_the_state_through_four_splitmix64_outputs() {
    let prng = Xoshiro256StarStar::new(0);
    assert_eq!(prng.state(), SPLITMIX64_FROM_0[..4]);
    let mut mix = SplitMix64::new(0x5EED);
    let expected = [
        mix.next_u64(),
        mix.next_u64(),
        mix.next_u64(),
        mix.next_u64(),
    ];
    assert_eq!(Xoshiro256StarStar::new(0x5EED).state(), expected);
}

#[test]
fn the_same_seed_gives_the_same_sequence_and_another_seed_another() {
    let mut a = Xoshiro256StarStar::new(42);
    let mut b = Xoshiro256StarStar::new(42);
    let mut c = Xoshiro256StarStar::new(43);
    let from_a: Vec<u64> = (0..1_000).map(|_| a.next_u64()).collect();
    let from_b: Vec<u64> = (0..1_000).map(|_| b.next_u64()).collect();
    let from_c: Vec<u64> = (0..1_000).map(|_| c.next_u64()).collect();
    assert_eq!(from_a, from_b);
    assert_ne!(from_a, from_c);
}

#[test]
fn the_all_zero_state_is_replaced_by_the_state_of_seed_0() {
    assert_eq!(
        Xoshiro256StarStar::from_state([0; 4]),
        Xoshiro256StarStar::new(0)
    );
    let mut prng = Xoshiro256StarStar::from_state([0; 4]);
    assert_ne!(prng.next_u64(), 0, "the generator left the all-zero state");
}

#[test]
fn next_f64_stays_in_the_unit_interval_and_is_exact_in_f64() {
    let mut prng = Xoshiro256StarStar::new(7);
    for _ in 0..100_000 {
        let x = prng.next_f64();
        assert!((0.0..1.0).contains(&x), "{x}");
        assert_eq!(
            x * (1u64 << 53) as f64,
            (x * (1u64 << 53) as f64).trunc(),
            "{x}"
        );
    }
    let largest = (u64::MAX >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
    assert!(largest < 1.0, "the largest value {largest} is below 1");
}

#[test]
fn the_name_is_what_a_dataset_records() {
    assert_eq!(NAME, "xoshiro256**");
}
