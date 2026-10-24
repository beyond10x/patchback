//! The four acceptance scenarios of story:dataset-generation
//! (`spec/scenarios/dataset-generation-*.yaml`), step by step against the model of
//! `spec/domains/dataset.yaml` as [`DatasetStore`] carries it.

use std::path::PathBuf;

use patchback_analysis::{FeatureContractId, FeatureError, NotComputableReason};
use patchback_core::prng::NAME;
use patchback_dataset::{
    DatasetError, DatasetId, DatasetStore, DiagnosticCase, DiagnosticCause, DiagnosticKind,
    DiagnosticSetId, GenerateDataset, PrngAlgorithm, SampledPatchId, SplitRole,
};

/// `GenerateDataset` as every scenario gives it: seed 42, batch size 16, `data/`.
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

const X: [f64; 32] = [0.25; 32];
const Y: [f64; 8] = [0.5; 8];

/// dataset-generation-seeded-splits: a dataset generated from seed 42 with its PRNG named opens
/// train, validation and test splits of 50,016, 4,992 and 4,992 samples.
#[test]
fn scenario_seeded_splits() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let recorded = store.dataset(dataset).expect("generated");
    assert_eq!(recorded.seed, 42);
    assert_eq!(
        recorded.prng_algorithm,
        PrngAlgorithm("xoshiro256**".to_owned())
    );
    assert_eq!(recorded.batch_size, 16);

    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    let validation = store
        .open_split(dataset, SplitRole::Validation, 4_992)
        .unwrap();
    let test = store.open_split(dataset, SplitRole::Test, 4_992).unwrap();

    let opened: Vec<(SplitRole, i64)> = [train, validation, test]
        .into_iter()
        .map(|id| {
            let split = store.split(id).expect("opened");
            assert_eq!(split.dataset_id, dataset);
            (split.role, split.sample_count)
        })
        .collect();
    assert_eq!(
        opened,
        [
            (SplitRole::Train, 50_016),
            (SplitRole::Validation, 4_992),
            (SplitRole::Test, 4_992)
        ]
    );
    assert!(store.samples_in(train).is_empty(), "a split opens empty");
}

/// dataset-generation-count-not-batch-multiple: a count that is not a multiple of batch size 16
/// is refused with SplitCountNotBatchMultiple; 50,016 opens.
///
/// The refused count is 50,008 (16 × 3,125.5), as the corrected scenario gives it: the model's
/// rule (`sample_count is not a positive multiple of the dataset's batch_size`) and DESIGN.md
/// § 7.1 ("divisible by the initial batch size of 16").
#[test]
fn scenario_count_not_batch_multiple() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);

    let refused = store.open_split(dataset, SplitRole::Train, 50_008);
    assert_eq!(
        refused,
        Err(DatasetError::SplitCountNotBatchMultiple {
            role: SplitRole::Train,
            sample_count: 50_008,
            batch_size: 16,
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.SplitCountNotBatchMultiple"
    );
    assert_eq!(store.splits_of(dataset).count(), 0, "no split was opened");

    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    assert_eq!(store.split(train).unwrap().sample_count, 50_016);
}

/// The model's comment on `count-not-batch-multiple`: a count of zero or below is refused the
/// same way.
#[test]
fn a_count_of_zero_or_below_is_not_a_positive_multiple() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    for count in [0, -16, 15, 17] {
        assert_eq!(
            store.open_split(dataset, SplitRole::Validation, count),
            Err(DatasetError::SplitCountNotBatchMultiple {
                role: SplitRole::Validation,
                sample_count: count,
                batch_size: 16,
            })
        );
    }
}

/// OpenSplit's first outcome: a split of an unknown dataset is refused before its count.
#[test]
fn a_split_of_an_unknown_dataset_is_refused_before_its_count() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let unknown = DatasetId(dataset.0 + 100);
    assert_eq!(
        store.open_split(unknown, SplitRole::Train, 50_000),
        Err(DatasetError::UnknownDataset {
            dataset_id: unknown
        })
    );
}

/// dataset-generation-patch-identity: patch seed 7 drawn into train cannot be drawn into
/// validation (PatchAlreadyDrawn); its notes are added under the one drawn patch.
#[test]
fn scenario_patch_identity() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let train = store.open_split(dataset, SplitRole::Train, 50_016).unwrap();
    let validation = store
        .open_split(dataset, SplitRole::Validation, 4_992)
        .unwrap();

    let patch = store.draw_patch(dataset, train, 7).unwrap();
    let drawn = store.patch(patch).expect("drawn");
    assert_eq!(
        (drawn.dataset_id, drawn.dataset_split_id, drawn.patch_seed),
        (dataset, train, 7)
    );

    let c4 = store.add_sample(patch, 60, 24_000, X, Y).unwrap();
    let c5 = store.add_sample(patch, 72, 24_000, X, Y).unwrap();
    let in_train: Vec<_> = store
        .samples_in(train)
        .iter()
        .map(|s| s.sample_id)
        .collect();
    assert_eq!(in_train, [c4, c5], "both renderings stay with the patch");
    let c4 = store.sample(c4).unwrap();
    assert_eq!(
        (c4.sampled_patch_id, c4.midi_note, c4.gate_samples),
        (patch, 60, 24_000)
    );

    assert_eq!(
        store.draw_patch(dataset, validation, 7),
        Err(DatasetError::PatchAlreadyDrawn {
            patch_seed: 7,
            dataset_split_id: validation,
        })
    );
    assert!(store.samples_in(validation).is_empty());

    let other = store.draw_patch(dataset, validation, 8).unwrap();
    assert_eq!(store.patch(other).unwrap().dataset_split_id, validation);

    let unknown = SampledPatchId(0xff);
    assert_eq!(
        store.add_sample(unknown, 60, 24_000, X, Y),
        Err(DatasetError::UnknownPatch {
            sampled_patch_id: unknown
        })
    );
}

/// A seed is one patch per dataset, in the split it was drawn into too.
#[test]
fn a_patch_seed_cannot_be_drawn_twice_into_one_split() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let train = store.open_split(dataset, SplitRole::Train, 16).unwrap();
    store.draw_patch(dataset, train, 7).unwrap();
    assert_eq!(
        store.draw_patch(dataset, train, 7),
        Err(DatasetError::PatchAlreadyDrawn {
            patch_seed: 7,
            dataset_split_id: train,
        })
    );
}

/// Patch identity is per dataset: another dataset may draw the same seed.
#[test]
fn another_dataset_may_draw_the_same_seed() {
    let mut store = DatasetStore::new();
    let first = generate(&mut store);
    let second = generate(&mut store);
    let first_train = store.open_split(first, SplitRole::Train, 16).unwrap();
    let second_train = store.open_split(second, SplitRole::Train, 16).unwrap();
    store.draw_patch(first, first_train, 7).unwrap();
    store.draw_patch(second, second_train, 7).unwrap();
}

/// DrawPatch into a split the dataset does not hold is refused (the model's Gap note: ESS cannot
/// state this refusal; the implementation makes it with the declared errors).
#[test]
fn a_patch_drawn_into_an_unknown_split_or_dataset_is_refused() {
    let mut store = DatasetStore::new();
    let first = generate(&mut store);
    let second = generate(&mut store);
    let foreign = store.open_split(second, SplitRole::Train, 16).unwrap();
    assert_eq!(
        store.draw_patch(first, foreign, 7),
        Err(DatasetError::UnknownSplit {
            dataset_split_id: foreign
        })
    );
    let unknown = DatasetId(second.0 + 100);
    assert_eq!(
        store.draw_patch(unknown, foreign, 7),
        Err(DatasetError::UnknownDataset {
            dataset_id: unknown
        })
    );
    assert_eq!(store.patches_of(first).count(), 0);
}

/// One diagnostic case of each kind, as the generator records them.
fn case(kind: DiagnosticKind) -> DiagnosticCase {
    let cause = match kind {
        DiagnosticKind::Clipping => DiagnosticCause::limited(12),
        DiagnosticKind::Silence => {
            DiagnosticCause::from_feature(FeatureError::FeatureNotComputable {
                feature: patchback_analysis::FeatureSlot::Scalar(
                    patchback_analysis::ScalarFeature::Rms,
                ),
                index: 24,
                reason: NotComputableReason::Analysis(
                    patchback_analysis::AnalysisError::SilentInput { rms_dbfs: -90.0 },
                ),
            })
            .expect("a silent input is a diagnostic")
        }
        DiagnosticKind::Noise => {
            DiagnosticCause::from_feature(FeatureError::FeatureNotComputable {
                feature: patchback_analysis::FeatureSlot::Scalar(
                    patchback_analysis::ScalarFeature::NormalizedMidiPitch,
                ),
                index: 31,
                reason: NotComputableReason::Analysis(
                    patchback_analysis::AnalysisError::PitchAmbiguous { confidence: 0.2 },
                ),
            })
            .expect("an ambiguous pitch is a diagnostic")
        }
    };
    assert_eq!(cause.kind(), kind);
    DiagnosticCase {
        patch_seed: 9,
        midi_note: 60,
        gate_samples: 24_000,
        cause,
    }
}

/// dataset-generation-diagnostic-set: a dataset's separate diagnostic set takes silence, noise and
/// clipping examples; a diagnostic set for an unknown dataset is refused.
#[test]
fn scenario_diagnostic_set() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let diagnostics = store.open_diagnostic_set(dataset).unwrap();
    assert_eq!(
        store.diagnostic_set(diagnostics).unwrap().dataset_id,
        dataset
    );

    let mut added = Vec::new();
    for kind in [
        DiagnosticKind::Silence,
        DiagnosticKind::Noise,
        DiagnosticKind::Clipping,
    ] {
        let id = store
            .add_diagnostic_example(diagnostics, case(kind))
            .unwrap();
        let example = store.diagnostic_example(id).unwrap();
        assert_eq!(example.diagnostic_set_id, diagnostics);
        assert_eq!(example.kind, kind);
        assert_eq!(
            (example.patch_seed, example.midi_note, example.gate_samples),
            (9, 60, 24_000)
        );
        added.push(id);
    }
    let listed: Vec<_> = store
        .diagnostic_examples_in(diagnostics)
        .iter()
        .map(|e| e.diagnostic_example_id)
        .collect();
    assert_eq!(listed, added);
    assert_eq!(
        store.patches_of(dataset).count(),
        0,
        "no example is a patch"
    );

    let unknown = DatasetId(0xfe);
    assert_eq!(
        store.open_diagnostic_set(unknown),
        Err(DatasetError::UnknownDataset {
            dataset_id: unknown
        })
    );
}

/// AddDiagnosticExample's first outcome.
#[test]
fn an_example_for_an_unknown_diagnostic_set_is_refused() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let set = store.open_diagnostic_set(dataset).unwrap();
    let unknown = DiagnosticSetId(set.0 + 100);
    assert_eq!(
        store.add_diagnostic_example(unknown, case(DiagnosticKind::Noise)),
        Err(DatasetError::UnknownDiagnosticSet {
            diagnostic_set_id: unknown
        })
    );
}
