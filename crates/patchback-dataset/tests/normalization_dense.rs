//! The dense files of story:normalization-stats (Da8-30, Da8-31): [`dense::write_split`] writes
//! x.f32 as the z-scores of each sample under the training split's statistics,
//! `((x − mean) / std) as f32` computed in `f64`, with divisor 1 where the standard deviation is
//! 0; y.f32 and order.u32 are unchanged. Each oracle is recomputed here from the statistics' own
//! means and standard deviations and decoded by this file's reader.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_analysis::{
    FeatureContractDefinition, FeatureContractId, FeatureRegistry, NormalizationStats, SplitInput,
    compute_normalization_stats,
};
use patchback_core::prng::NAME;
use patchback_dataset::dense;
use patchback_dataset::{
    DatasetId, DatasetSplitId, DatasetStore, GenerateDataset, PrngAlgorithm, SplitRole, SplitState,
    compute_split_stats,
};

fn generate(store: &mut DatasetStore) -> DatasetId {
    store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm(NAME.to_owned()),
        batch_size: 16,
        feature_contract_id: FeatureContractId(1),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    })
}

/// A registry with `features/1` defined and published, its identity and its version.
fn published() -> (FeatureRegistry, FeatureContractId, String) {
    let definition = FeatureContractDefinition::features_1();
    let version = definition.version.clone();
    let mut registry = FeatureRegistry::new();
    let id = registry.define(definition).unwrap();
    registry.publish(id).unwrap();
    (registry, id, version)
}

/// A split of `role` holding 16 samples, sample `i` with `x(i)` and `y(i)`; seeds are distinct
/// across splits of one dataset.
fn split_of(
    store: &mut DatasetStore,
    dataset: DatasetId,
    role: SplitRole,
    x: impl Fn(u64) -> [f64; 32],
    y: impl Fn(u64) -> [f64; 8],
) -> DatasetSplitId {
    let split = store.open_split(dataset, role, 16).unwrap();
    for i in 0..16u64 {
        let patch = store
            .draw_patch(dataset, split, split.0 * 1_000 + i)
            .unwrap();
        store.add_sample(patch, 60, 24_000, x(i), y(i)).unwrap();
    }
    split
}

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-normalization-dense")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// This file's own decoder: four bytes, least significant first, a value's bits.
fn decode_bits(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "a whole number of 4-byte values");
    (0..bytes.len() / 4)
        .map(|i| {
            u32::from(bytes[4 * i])
                | u32::from(bytes[4 * i + 1]) << 8
                | u32::from(bytes[4 * i + 2]) << 16
                | u32::from(bytes[4 * i + 3]) << 24
        })
        .collect()
}

/// The oracle of Da8-30, from the statistics' means and standard deviations: `(v − mean) / std`
/// in `f64`, divisor 1 where the standard deviation is 0, then `as f32`, row-major.
fn z_bits(rows: &[[f64; 32]], stats: &NormalizationStats) -> Vec<u32> {
    rows.iter()
        .flat_map(|row| {
            row.iter().enumerate().map(|(j, &v)| {
                let std = stats.stds()[j];
                let divisor = if std == 0.0 { 1.0 } else { std };
                ((v - stats.means()[j]) / divisor) as f32
            })
        })
        .map(f32::to_bits)
        .collect()
}

fn train_x(i: u64) -> [f64; 32] {
    std::array::from_fn(|j| {
        if j == 5 {
            3.25
        } else {
            i as f64 + j as f64 / 64.0
        }
    })
}

/// A training split whose feature 5 is 3.25 in every row (standard deviation 0) writes 0.0 for
/// it in every row of x.f32 (the mean subtracted, divisor 1), and every other value is its
/// z-score under the split's own statistics.
#[test]
fn a_train_feature_of_std_0_is_written_as_0() {
    let dir = scratch("std-0");
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let train = split_of(&mut store, dataset, SplitRole::Train, train_x, |_| [0.5; 8]);
    let (registry, contract, version) = published();
    let stats = compute_split_stats(&store, &registry, contract, train, &version).unwrap();
    assert_eq!(
        (stats.means()[5], stats.stds()[5]),
        (3.25, 0.0),
        "premise: feature 5 is constant"
    );

    dense::write_split(&mut store, train, &dir, &stats, dense::create_new).unwrap();
    assert_eq!(store.split(train).unwrap().state, SplitState::Written);
    let x = decode_bits(&fs::read(dir.join("x.f32")).unwrap());
    assert_eq!(x.len(), 16 * 32);
    for row in 0..16 {
        assert_eq!(x[row * 32 + 5], 0.0f32.to_bits(), "row {row}: +0.0");
    }
    let rows: Vec<[f64; 32]> = (0..16).map(train_x).collect();
    assert_eq!(x, z_bits(&rows, &stats));
    assert!(
        x.iter().any(|&bits| f32::from_bits(bits) < -1.0),
        "values are z-scores, not the raw rows (all non-negative)"
    );
}

/// A validation split is written under the training split's statistics, not its own: its x.f32
/// is `((x − train mean) / train std) as f32`, which differs from the z-scores under the
/// validation rows' own statistics in every value; its y.f32 is each `y` value `as f32` and its
/// order.u32 the row indices, as before normalization.
#[test]
fn a_validation_split_is_normalized_under_the_train_statistics() {
    let dir = scratch("validation");
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let val_x =
        |i: u64| -> [f64; 32] { std::array::from_fn(|j| 100.0 + 3.0 * i as f64 + j as f64 / 8.0) };
    let val_y = |i: u64| -> [f64; 8] { std::array::from_fn(|j| (i as f64 + j as f64) / 32.0) };
    let train = split_of(&mut store, dataset, SplitRole::Train, train_x, |_| [0.5; 8]);
    let validation = split_of(&mut store, dataset, SplitRole::Validation, val_x, val_y);
    let (registry, contract, version) = published();
    let stats = compute_split_stats(&store, &registry, contract, train, &version).unwrap();

    dense::write_split(&mut store, validation, &dir, &stats, dense::create_new).unwrap();
    assert_eq!(store.split(validation).unwrap().state, SplitState::Written);
    let x = decode_bits(&fs::read(dir.join("x.f32")).unwrap());
    let rows: Vec<[f64; 32]> = (0..16).map(val_x).collect();
    assert_eq!(x, z_bits(&rows, &stats), "under the train statistics");

    let own = compute_normalization_stats(
        &registry,
        contract,
        &version,
        validation.0,
        Some(SplitInput {
            role: SplitRole::Train,
            rows: &rows,
        }),
    )
    .unwrap();
    let under_own = z_bits(&rows, &own);
    assert!(
        x.iter().zip(&under_own).all(|(a, b)| a != b),
        "no value is the z-score under the validation rows' own statistics"
    );

    let y = decode_bits(&fs::read(dir.join("y.f32")).unwrap());
    let expected_y: Vec<u32> = (0..16)
        .flat_map(|i| val_y(i).map(|v| (v as f32).to_bits()))
        .collect();
    assert_eq!(y, expected_y, "y.f32 unchanged");
    let order = decode_bits(&fs::read(dir.join("order.u32")).unwrap());
    assert_eq!(order, (0..16u32).collect::<Vec<_>>(), "order.u32 unchanged");
}
