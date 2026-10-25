//! Adversary case for story:normalization-stats (wave a8, pass 2), dataset side: a training
//! column constant at a non-dyadic value (0.1) has std 0 by the constant rule (Da8-44), and
//! `dense::write_split` writes +0.0 for it in every row of x.f32, bit for bit, with the read-back
//! passing and the split `Written`.

use std::fs;
use std::path::{Path, PathBuf};

use patchback_analysis::{FeatureContractDefinition, FeatureContractId, FeatureRegistry};
use patchback_core::prng::NAME;
use patchback_dataset::dense;
use patchback_dataset::{
    DatasetStore, GenerateDataset, PrngAlgorithm, SplitRole, SplitState, compute_split_stats,
};

#[test]
fn a_non_dyadic_constant_training_column_is_written_as_plus_zero() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-normalization-adversary")
        .join("constant-0.1");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();

    let mut store = DatasetStore::new();
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm(NAME.to_owned()),
        batch_size: 16,
        feature_contract_id: FeatureContractId(1),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let train = store.open_split(dataset, SplitRole::Train, 16).unwrap();
    let x = |i: u64| -> [f64; 32] {
        std::array::from_fn(|j| {
            if j == 5 {
                0.1
            } else {
                i as f64 * 0.3 + j as f64
            }
        })
    };
    for i in 0..16u64 {
        let patch = store.draw_patch(dataset, train, 7_000 + i).unwrap();
        store.add_sample(patch, 60, 24_000, x(i), [0.5; 8]).unwrap();
    }
    let definition = FeatureContractDefinition::features_1();
    let version = definition.version.clone();
    let mut registry = FeatureRegistry::new();
    let contract = registry.define(definition).unwrap();
    registry.publish(contract).unwrap();
    let stats = compute_split_stats(&store, &registry, contract, train, &version).unwrap();
    assert_eq!(
        (stats.means()[5].to_bits(), stats.stds()[5].to_bits()),
        (0.1f64.to_bits(), 0),
        "mean 0.1 bit for bit, std +0.0"
    );

    dense::write_split(&mut store, train, &dir, &stats, dense::create_new)
        .expect("the read-back passes");
    assert_eq!(store.split(train).unwrap().state, SplitState::Written);
    let bytes = fs::read(dir.join("x.f32")).unwrap();
    assert_eq!(bytes.len(), 16 * 32 * 4);
    for row in 0..16 {
        let at = (row * 32 + 5) * 4;
        assert_eq!(bytes[at..at + 4], [0, 0, 0, 0], "row {row}: +0.0");
    }
}
