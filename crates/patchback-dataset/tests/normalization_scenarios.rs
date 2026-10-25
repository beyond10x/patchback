//! The three acceptance scenarios of story:normalization-stats
//! (`spec/scenarios/normalization-stats-{train-only,contract-gates,apply}.yaml`), step by step, one
//! test per scenario file named after it. Every scenario opens with `patchback.dataset`'s
//! `GenerateDataset` and `OpenSplit`, which [`DatasetStore`] carries; the analysis commands run
//! through [`compute_split_stats`], which reads the split's role and rows from the store, and
//! [`NormalizationStats::apply`]. Each step names its `at:` time in the scenario.
//!
//! The scenarios open their splits without samples. Each test adds a few samples to the splits
//! (`DrawPatch`, `AddSample`; beyond the scenario) so that the statistics computed are of real
//! rows; the outcomes are the scenario's either way.

use std::path::PathBuf;

use patchback_analysis::{
    FEATURES_1_VERSION, FeatureContractDefinition, FeatureContractId, FeatureRegistry,
    NormalizationError, NormalizationStats,
};
use patchback_core::prng::NAME;
use patchback_dataset::{
    DatasetId, DatasetSplitId, DatasetStore, GenerateDataset, PrngAlgorithm, SplitRole,
    compute_split_stats,
};

/// Step `08:59:50`: `GenerateDataset` as every scenario gives it, seed 42, batch size 16,
/// `data/`, under the contract `5a000000-…-0001`, which no step defines (here 0x5a, outside the
/// registry: nothing compares the dataset's contract with the statistics', Da8-17).
fn generate(store: &mut DatasetStore) -> DatasetId {
    store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm(NAME.to_owned()),
        batch_size: 16,
        feature_contract_id: FeatureContractId(0x5a),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    })
}

/// Beyond the scenario: three samples in `split`, sample `k` (0, 1, 2) feature `i` equal to
/// `offset + k + i / 8`.
fn fill(store: &mut DatasetStore, dataset: DatasetId, split: DatasetSplitId, offset: f64) {
    for k in 0..3_u64 {
        let seed = split.0 * 1_000 + k;
        let patch = store.draw_patch(dataset, split, seed).unwrap();
        let x: [f64; 32] = std::array::from_fn(|i| offset + k as f64 + i as f64 / 8.0);
        store.add_sample(patch, 60, 24_000, x, [0.5; 8]).unwrap();
    }
}

/// `DefineFeatureContract` of `features/1` (32 values, eight bands over attack, body and
/// release, the eight scalars, FFT lengths 512, 2048, 8192), as each scenario gives it.
fn define(registry: &mut FeatureRegistry) -> FeatureContractId {
    let definition = FeatureContractDefinition::features_1();
    assert_eq!(definition.version, "features/1");
    assert_eq!(definition.value_count, 32);
    assert_eq!(definition.band_count, 8);
    assert_eq!(definition.fft_lengths, [512, 2048, 8192]);
    registry.define(definition).expect("defined")
}

/// normalization-stats-train-only: statistics are computed from the training split under
/// `features/1`; the validation and test splits and an unknown split are refused.
#[test]
fn normalization_stats_train_only() {
    let mut store = DatasetStore::new();
    let mut registry = FeatureRegistry::new();

    // 08:59:50 GenerateDataset → generated
    let dataset = generate(&mut store);
    // 08:59:51-53 OpenSplit Train 50,016, Validation 4,992, Test 4,992 → opened
    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    let validation = store
        .open_split(dataset, SplitRole::Validation, 4_992)
        .unwrap();
    let test = store.open_split(dataset, SplitRole::Test, 4_992).unwrap();
    fill(&mut store, dataset, train, 0.0);
    fill(&mut store, dataset, validation, 10.0);
    fill(&mut store, dataset, test, 20.0);

    // 09:00:00 DefineFeatureContract → defined; 09:00:01 PublishFeatureContract → published
    let contract = define(&mut registry);
    registry.publish(contract).expect("published");

    // 09:00:02 ComputeNormalizationStats (contract, train, features/1) → computed
    let stats =
        compute_split_stats(&store, &registry, contract, train, FEATURES_1_VERSION).unwrap();
    assert_eq!(stats.feature_contract_id(), contract);
    assert_eq!(stats.dataset_split_id(), train.0);
    assert_eq!(stats.feature_contract_version(), "features/1");
    assert_eq!(stats.rows(), 3);
    // the training rows' values only: k + i / 8 for k = 0, 1, 2
    for i in 0..32 {
        assert_eq!(stats.means()[i], 1.0 + i as f64 / 8.0, "{i}");
        assert_eq!(stats.stds()[i], (2.0_f64 / 3.0).sqrt(), "{i}");
    }

    // 09:00:03 (validation) and 09:00:04 (test) → not-training-split, StatsFromNonTrainingSplit
    // with the split's role
    for (split, role) in [(validation, SplitRole::Validation), (test, SplitRole::Test)] {
        let refused = compute_split_stats(&store, &registry, contract, split, FEATURES_1_VERSION);
        assert_eq!(
            refused,
            Err(NormalizationError::StatsFromNonTrainingSplit { split_role: role })
        );
        assert_eq!(
            refused.unwrap_err().domain_error(),
            "patchback.analysis.StatsFromNonTrainingSplit"
        );
    }

    // 09:00:05 an unknown split (5d000000-…-0099) → no-such-split, DatasetSplitNotFound; no
    // statistics (no NormalizationStatsComputed)
    let unknown = DatasetSplitId(99);
    let refused = compute_split_stats(&store, &registry, contract, unknown, FEATURES_1_VERSION);
    assert_eq!(
        refused,
        Err(NormalizationError::DatasetSplitNotFound {
            dataset_split_id: 99
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.DatasetSplitNotFound"
    );

    // assert: NormalizationStatsSet contains {dataset_split_id: train, split_role: Train}
    let set: Vec<&NormalizationStats> = vec![&stats];
    assert!(
        set.iter()
            .any(|s| s.dataset_split_id() == train.0 && s.split_role() == SplitRole::Train)
    );
}

/// normalization-stats-contract-gates: statistics under an unknown contract, a draft contract, or
/// a version the contract does not carry are refused.
#[test]
fn normalization_stats_contract_gates() {
    let mut store = DatasetStore::new();
    let mut registry = FeatureRegistry::new();

    // 08:59:50 GenerateDataset → generated; 08:59:51 OpenSplit Train 50,016 → opened
    let dataset = generate(&mut store);
    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    fill(&mut store, dataset, train, 0.0);

    // 09:00:00 ComputeNormalizationStats under an unknown contract (fc000000-…-0099) →
    // no-such-contract, FeatureContractNotFound
    let unknown = FeatureContractId(99);
    let refused = compute_split_stats(&store, &registry, unknown, train, FEATURES_1_VERSION);
    assert_eq!(
        refused,
        Err(NormalizationError::FeatureContractNotFound {
            feature_contract_id: unknown
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.FeatureContractNotFound"
    );

    // 09:00:01 DefineFeatureContract → defined (a draft)
    let contract = define(&mut registry);

    // 09:00:02 ComputeNormalizationStats under the draft → contract-draft,
    // FeatureContractNotPublished
    let refused = compute_split_stats(&store, &registry, contract, train, FEATURES_1_VERSION);
    assert_eq!(
        refused,
        Err(NormalizationError::FeatureContractNotPublished {
            feature_contract_id: contract
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.FeatureContractNotPublished"
    );

    // 09:00:03 PublishFeatureContract → published
    registry.publish(contract).expect("published");

    // 09:00:04 ComputeNormalizationStats naming features/2 → version-mismatch,
    // FeatureVersionMismatch with the version named
    let refused = compute_split_stats(&store, &registry, contract, train, "features/2");
    assert_eq!(
        refused,
        Err(NormalizationError::FeatureVersionMismatch {
            version: "features/2".to_owned()
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.FeatureVersionMismatch"
    );
}

/// normalization-stats-apply: statistics under `features/1` normalize `features/1` features
/// unchanged; `features/2` features are refused. Unknown statistics have no counterpart (below).
#[test]
fn normalization_stats_apply() {
    let mut store = DatasetStore::new();
    let mut registry = FeatureRegistry::new();

    // 08:59:50 GenerateDataset → generated; 08:59:51 OpenSplit Train 50,016 → opened
    let dataset = generate(&mut store);
    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    fill(&mut store, dataset, train, 0.0);

    // 09:00:00 DefineFeatureContract → defined; 09:00:01 PublishFeatureContract → published
    let contract = define(&mut registry);
    registry.publish(contract).expect("published");

    // 09:00:02 ComputeNormalizationStats → computed
    let stats =
        compute_split_stats(&store, &registry, contract, train, FEATURES_1_VERSION).unwrap();
    let before = stats.clone();

    // 09:00:03 ApplyNormalization (stats, features/1) → normalized; the statistics are read and
    // left unchanged
    let x: [f64; 32] = std::array::from_fn(|i| 2.0 + i as f64 / 8.0);
    let normalized = stats.apply(FEATURES_1_VERSION, &x).unwrap();
    let one_sd = 1.0 / (2.0_f64 / 3.0).sqrt();
    for (i, value) in normalized.iter().enumerate() {
        assert_eq!(*value, one_sd, "{i}: one standard deviation above the mean");
    }
    assert_eq!(stats, before);

    // 09:00:04 ApplyNormalization (stats, features/2) → contract-mismatch,
    // NormalizationContractMismatch
    let refused = stats.apply("features/2", &x);
    assert_eq!(
        refused,
        Err(NormalizationError::NormalizationContractMismatch {
            feature_contract_version: "features/2".to_owned()
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.analysis.NormalizationContractMismatch"
    );
    assert_eq!(stats, before);

    // 09:00:05 ApplyNormalization of unknown statistics (5e000000-…-0099) → no-such-stats,
    // NormalizationStatsNotFound: no library counterpart. `apply` is a method of the statistics
    // value, so no call can name statistics that do not exist; the analysis crate's domain test
    // lists the error as not emitted, with that reason (`NORMALIZATION_NOT_EMITTED`).
}
