//! The three acceptance scenarios of story:dense-data-files
//! (`spec/scenarios/dense-data-files-*.yaml`), step by step against the model of
//! `spec/domains/dataset.yaml` ([`DatasetStore::write_dense_file`] and
//! [`DatasetStore::finish_split`]), then through the writer ([`dense::write_split`]) on hand-built
//! sample sets of the scenarios' 4,992 rows, each under the training statistics of
//! `train_stats` (Da8-30).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use patchback_analysis::{
    FEATURES_1_VALUE_COUNT, FeatureContractDefinition, FeatureContractId, FeatureRegistry,
    NormalizationStats, SplitInput, compute_normalization_stats,
};
use patchback_core::parameter::Topology;
use patchback_core::prng::NAME;
use patchback_dataset::dense::{self, DenseError};
use patchback_dataset::{
    DatasetError, DatasetId, DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset,
    MAX_SAMPLES, PrngAlgorithm, SplitRole, SplitState, WriteDenseFile,
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

/// The scenarios' validation split of 4,992 samples.
fn validation(store: &mut DatasetStore) -> DatasetSplitId {
    let dataset = generate(store);
    store
        .open_split(dataset, SplitRole::Validation, ROWS)
        .unwrap()
}

const ROWS: i64 = 4_992;

fn input(
    split: DatasetSplitId,
    kind: DenseFileKind,
    rows: i64,
    columns: i64,
    byte_count: i64,
) -> WriteDenseFile {
    WriteDenseFile {
        dataset_split_id: split,
        kind,
        path: PathBuf::from("data/val").join(kind.file_name()),
        rows,
        columns,
        byte_count,
    }
}

/// `count` little-endian `f32` values, `value(i)` each.
fn f32_content(count: usize, value: impl Fn(usize) -> f32) -> Vec<u8> {
    (0..count).flat_map(|i| value(i).to_le_bytes()).collect()
}

/// The order file of `rows` rows: 0 to rows − 1, little-endian `u32`.
fn order_content(rows: u32) -> Vec<u8> {
    (0..rows).flat_map(u32::to_le_bytes).collect()
}

/// dense-data-files-written: x.f32 (4,992 × 32, 638,976 bytes), y.f32 (4,992 × 8, 159,744 bytes)
/// and order.u32 (4,992 × 1, 19,968 bytes) are written; FinishSplit moves the split to Written;
/// a second FinishSplit is refused with SplitAlreadyWritten.
#[test]
fn scenario_written() {
    let mut store = DatasetStore::new();
    let split = validation(&mut store);
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);

    let x = store
        .write_dense_file(
            input(split, DenseFileKind::X, ROWS, 32, 638_976),
            &f32_content(4_992 * 32, |i| i as f32 / 7.0),
        )
        .unwrap();
    let y = store
        .write_dense_file(
            input(split, DenseFileKind::Y, ROWS, 8, 159_744),
            &f32_content(4_992 * 8, |i| (i % 8) as f32 / 8.0),
        )
        .unwrap();
    let order = store
        .write_dense_file(
            input(split, DenseFileKind::Order, ROWS, 1, 19_968),
            &order_content(4_992),
        )
        .unwrap();
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);

    let recorded: Vec<_> = store
        .dense_files_of(split)
        .iter()
        .map(|file| {
            (
                file.dense_data_file_id,
                file.dataset_split_id,
                file.kind,
                file.path.clone(),
                file.rows,
                file.columns,
                file.byte_count,
            )
        })
        .collect();
    assert_eq!(
        recorded,
        [
            (
                x,
                split,
                DenseFileKind::X,
                PathBuf::from("data/val/x.f32"),
                ROWS,
                32,
                638_976
            ),
            (
                y,
                split,
                DenseFileKind::Y,
                PathBuf::from("data/val/y.f32"),
                ROWS,
                8,
                159_744
            ),
            (
                order,
                split,
                DenseFileKind::Order,
                PathBuf::from("data/val/order.u32"),
                ROWS,
                1,
                19_968
            ),
        ]
    );
    assert_eq!(store.dense_file(y).unwrap().kind, DenseFileKind::Y);

    assert_eq!(store.finish_split(split), Ok(()));
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);

    let again = store.finish_split(split);
    assert_eq!(
        again,
        Err(DatasetError::SplitAlreadyWritten {
            dataset_split_id: split,
            state: SplitState::Written,
        })
    );
    assert_eq!(
        again.unwrap_err().domain_error(),
        "patchback.dataset.SplitAlreadyWritten"
    );
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);
}

/// dense-data-files-shape-mismatch: an x.f32 of 31 columns and a y.f32 of 32 are refused with
/// WrongColumnCount, an x.f32 of 4,991 rows for the 4,992-sample split with SampleCountMismatch,
/// and a file of an unknown split with UnknownSplit; nothing is recorded.
#[test]
fn scenario_shape_mismatch() {
    let mut store = DatasetStore::new();
    let split = validation(&mut store);

    let refused = store.write_dense_file(
        input(split, DenseFileKind::X, ROWS, 31, 618_984),
        &f32_content(4_992 * 31, |_| 0.5),
    );
    assert_eq!(
        refused,
        Err(DatasetError::WrongColumnCount {
            kind: DenseFileKind::X,
            columns: 31,
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.WrongColumnCount"
    );

    assert_eq!(
        store.write_dense_file(
            input(split, DenseFileKind::Y, ROWS, 32, 638_976),
            &f32_content(4_992 * 32, |_| 0.5),
        ),
        Err(DatasetError::WrongColumnCount {
            kind: DenseFileKind::Y,
            columns: 32,
        })
    );

    let refused = store.write_dense_file(
        input(split, DenseFileKind::X, 4_991, 32, 638_848),
        &f32_content(4_991 * 32, |_| 0.5),
    );
    assert_eq!(
        refused,
        Err(DatasetError::SampleCountMismatch {
            rows: 4_991,
            sample_count: ROWS,
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.SampleCountMismatch"
    );

    let unknown = DatasetSplitId(0xfd);
    let refused = store.write_dense_file(
        input(unknown, DenseFileKind::X, ROWS, 32, 638_976),
        &f32_content(4_992 * 32, |_| 0.5),
    );
    assert_eq!(
        refused,
        Err(DatasetError::UnknownSplit {
            dataset_split_id: unknown
        })
    );

    assert!(
        store.dense_files_of(split).is_empty(),
        "nothing is recorded"
    );
    assert!(store.dense_files_of(unknown).is_empty());
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}

/// The model's outcome order: the column count is checked before the split, the split before
/// the rows, the rows before the byte count, the byte count before the values.
#[test]
fn write_dense_file_refuses_in_the_model_s_order() {
    let mut store = DatasetStore::new();
    let split = validation(&mut store);
    let unknown = DatasetSplitId(99);
    let nan = f32_content(4_992 * 31, |_| f32::NAN);
    assert_eq!(
        store.write_dense_file(input(unknown, DenseFileKind::X, 3, 31, 1), &nan),
        Err(DatasetError::WrongColumnCount {
            kind: DenseFileKind::X,
            columns: 31
        }),
        "wrong-column-count before unknown-split"
    );
    assert_eq!(
        store.write_dense_file(input(unknown, DenseFileKind::Order, 3, 1, 1), &nan),
        Err(DatasetError::UnknownSplit {
            dataset_split_id: unknown
        }),
        "unknown-split before sample-count-mismatch"
    );
    assert_eq!(
        store.write_dense_file(input(split, DenseFileKind::Y, 3, 8, 1), &nan),
        Err(DatasetError::SampleCountMismatch {
            rows: 3,
            sample_count: ROWS
        }),
        "sample-count-mismatch before byte-count-mismatch"
    );
    assert_eq!(
        store.write_dense_file(input(split, DenseFileKind::Y, ROWS, 8, 1), &nan),
        Err(DatasetError::ByteCountMismatch {
            byte_count: 1,
            rows: ROWS,
            columns: 8
        }),
        "byte-count-mismatch before non-finite-value"
    );
    for kind in DenseFileKind::ALL {
        for columns in [0, -1, 2, 31, 33, i64::MAX] {
            if columns == kind.columns() {
                continue;
            }
            assert_eq!(
                store.write_dense_file(input(split, kind, ROWS, columns, 0), &[]),
                Err(DatasetError::WrongColumnCount { kind, columns }),
                "{kind:?} {columns}"
            );
        }
    }
    assert!(store.dense_files_of(split).is_empty());
}

/// dense-data-files-content-refused: an x.f32 of 638,975 bytes is refused with
/// ByteCountMismatch, a y.f32 holding NaN with NonFiniteValue, and FinishSplit of a split with no
/// dense file with MissingDenseFile.
#[test]
fn scenario_content_refused() {
    let mut store = DatasetStore::new();
    let split = validation(&mut store);

    let mut truncated = f32_content(4_992 * 32, |_| 0.25);
    truncated.pop();
    assert_eq!(truncated.len(), 638_975);
    let refused = store.write_dense_file(
        input(split, DenseFileKind::X, ROWS, 32, 638_975),
        &truncated,
    );
    assert_eq!(
        refused,
        Err(DatasetError::ByteCountMismatch {
            byte_count: 638_975,
            rows: ROWS,
            columns: 32,
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.ByteCountMismatch"
    );

    let with_nan = f32_content(4_992 * 8, |i| if i == 4_000 { f32::NAN } else { 0.5 });
    let refused =
        store.write_dense_file(input(split, DenseFileKind::Y, ROWS, 8, 159_744), &with_nan);
    assert_eq!(
        refused,
        Err(DatasetError::NonFiniteValue {
            kind: DenseFileKind::Y
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.NonFiniteValue"
    );

    let refused = store.finish_split(split);
    assert_eq!(
        refused,
        Err(DatasetError::MissingDenseFile {
            dataset_split_id: split
        })
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        "patchback.dataset.MissingDenseFile"
    );
    assert!(
        store.dense_files_of(split).is_empty(),
        "nothing is recorded"
    );
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}

/// The byte count the model checks is the bytes it is given: a count that agrees with the shape
/// but not with the content is refused, carrying the command's byte count as the domain's payload
/// says (`byte_count: input.byte_count`, Da7-17), and so is every infinity, in x and in y.
#[test]
fn the_content_itself_is_verified() {
    let mut store = DatasetStore::new();
    let split = validation(&mut store);
    let short = f32_content(4_992 * 32 - 1, |_| 0.5);
    assert_eq!(short.len(), 638_972);
    assert_eq!(
        store.write_dense_file(input(split, DenseFileKind::X, ROWS, 32, 638_976), &short),
        Err(DatasetError::ByteCountMismatch {
            byte_count: 638_976,
            rows: ROWS,
            columns: 32,
        })
    );
    for (kind, columns, bytes) in [
        (DenseFileKind::X, 32, 638_976),
        (DenseFileKind::Y, 8, 159_744),
    ] {
        let count = 4_992 * columns as usize;
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for at in [0, count / 2, count - 1] {
                let content = f32_content(count, |i| if i == at { bad } else { 1.0 });
                assert_eq!(
                    store.write_dense_file(input(split, kind, ROWS, columns, bytes), &content),
                    Err(DatasetError::NonFiniteValue { kind }),
                    "{kind:?} {bad} at {at}"
                );
            }
        }
    }
    assert!(store.dense_files_of(split).is_empty());
}

/// FinishSplit needs a verified x.f32 and a verified y.f32 of this split: either alone, the order
/// file alone, or the files of another split are MissingDenseFile; an unknown split is
/// UnknownSplit.
#[test]
fn finish_split_needs_both_x_and_y_of_its_own_split() {
    let x = |store: &mut DatasetStore, split| {
        store
            .write_dense_file(
                input(split, DenseFileKind::X, 16, 32, 2_048),
                &f32_content(16 * 32, |_| 0.5),
            )
            .unwrap()
    };
    let y = |store: &mut DatasetStore, split| {
        store
            .write_dense_file(
                input(split, DenseFileKind::Y, 16, 8, 512),
                &f32_content(16 * 8, |_| 0.5),
            )
            .unwrap()
    };
    let order = |store: &mut DatasetStore, split| {
        store
            .write_dense_file(
                input(split, DenseFileKind::Order, 16, 1, 64),
                &order_content(16),
            )
            .unwrap()
    };
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let train = store.open_split(dataset, SplitRole::Train, 16).unwrap();
    let test = store.open_split(dataset, SplitRole::Test, 16).unwrap();
    let missing = |split| {
        Err(DatasetError::MissingDenseFile {
            dataset_split_id: split,
        })
    };

    order(&mut store, train);
    assert_eq!(store.finish_split(train), missing(train));
    x(&mut store, train);
    assert_eq!(store.finish_split(train), missing(train), "x alone");
    y(&mut store, test);
    assert_eq!(
        store.finish_split(train),
        missing(train),
        "y of another split"
    );
    assert_eq!(store.finish_split(test), missing(test), "y alone");
    y(&mut store, train);
    assert_eq!(store.finish_split(train), Ok(()));
    assert_eq!(store.split(train).unwrap().state, SplitState::Written);
    assert_eq!(store.split(test).unwrap().state, SplitState::Open);
    x(&mut store, test);
    assert_eq!(store.finish_split(test), Ok(()), "order.u32 is optional");

    let unknown = DatasetSplitId(7);
    assert_eq!(
        store.finish_split(unknown),
        Err(DatasetError::UnknownSplit {
            dataset_split_id: unknown
        })
    );
}

/// The column counts are the model's (32, 8, 1), taken from the `features/1` contract and the
/// topology's parameters; the file names are DESIGN.md § 7.2's; `MAX_SAMPLES` is the most rows
/// `order.u32` indexes (Da7-10).
#[test]
fn the_kinds_carry_the_model_s_columns_and_file_names() {
    assert_eq!(
        DenseFileKind::ALL,
        [DenseFileKind::X, DenseFileKind::Y, DenseFileKind::Order]
    );
    let names: Vec<_> = DenseFileKind::ALL
        .iter()
        .map(|kind| (kind.name(), kind.file_name(), kind.columns()))
        .collect();
    assert_eq!(
        names,
        [
            ("X", "x.f32", 32),
            ("Y", "y.f32", 8),
            ("Order", "order.u32", 1)
        ]
    );
    assert_eq!(
        DenseFileKind::X.columns(),
        i64::try_from(FEATURES_1_VALUE_COUNT).unwrap()
    );
    assert_eq!(
        DenseFileKind::Y.columns(),
        i64::try_from(Topology::subtractive_mono().contracts().len()).unwrap()
    );
    assert_eq!(MAX_SAMPLES, u64::from(u32::MAX));
}

// The writer, on hand-built sample sets.

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-dense")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A validation split of `count` samples holding `samples` of them, sample `i` with
/// `sample(i)`'s `x` and `y`.
fn filled(
    store: &mut DatasetStore,
    count: i64,
    samples: u64,
    sample: impl Fn(u64) -> ([f64; 32], [f64; 8]),
) -> DatasetSplitId {
    let dataset = generate(store);
    let split = store
        .open_split(dataset, SplitRole::Validation, count)
        .unwrap();
    for i in 0..samples {
        let patch = store.draw_patch(dataset, split, i).unwrap();
        let (x, y) = sample(i);
        store.add_sample(patch, 60, 24_000, x, y).unwrap();
    }
    split
}

fn plain(i: u64) -> ([f64; 32], [f64; 8]) {
    let x = std::array::from_fn(|j| i as f64 + j as f64 / 64.0);
    let y = std::array::from_fn(|j| j as f64 / 8.0);
    (x, y)
}

/// The training statistics every writer case passes (Da8-30): those of 16 training rows
/// `plain(i).0` under a published `features/1`, feature `j` of mean `7.5 + j / 64` and standard
/// deviation `√21.25` (4.61…).
fn train_stats() -> NormalizationStats {
    let definition = FeatureContractDefinition::features_1();
    let version = definition.version.clone();
    let mut registry = FeatureRegistry::new();
    let id = registry.define(definition).unwrap();
    registry.publish(id).unwrap();
    let rows: Vec<[f64; 32]> = (0..16).map(|i| plain(i).0).collect();
    let input = SplitInput {
        role: SplitRole::Train,
        rows: &rows,
    };
    compute_normalization_stats(&registry, id, &version, 0, Some(input)).unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// dense-data-files-written through the writer: 4,992 samples give x.f32 of 638,976 bytes, y.f32
/// of 159,744 and order.u32 of 19,968, each recorded, and the split is Written.
#[test]
fn writer_scenario_written() {
    let dir = scratch("written");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, ROWS, 4_992, plain);
    let ids =
        dense::write_split(&mut store, split, &dir, &train_stats(), dense::create_new).unwrap();

    let sizes: Vec<(String, u64)> = names(&dir)
        .into_iter()
        .map(|name| {
            let size = fs::metadata(dir.join(&name)).unwrap().len();
            (name, size)
        })
        .collect();
    assert_eq!(
        sizes,
        [
            ("order.u32".to_owned(), 19_968),
            ("x.f32".to_owned(), 638_976),
            ("y.f32".to_owned(), 159_744),
        ]
    );
    let recorded: Vec<_> = store
        .dense_files_of(split)
        .iter()
        .map(|f| {
            (
                f.dense_data_file_id,
                f.kind,
                f.path.clone(),
                f.rows,
                f.byte_count,
            )
        })
        .collect();
    assert_eq!(
        recorded,
        [
            (ids[0], DenseFileKind::X, dir.join("x.f32"), ROWS, 638_976),
            (ids[1], DenseFileKind::Y, dir.join("y.f32"), ROWS, 159_744),
            (
                ids[2],
                DenseFileKind::Order,
                dir.join("order.u32"),
                ROWS,
                19_968
            ),
        ]
    );
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);
    assert_eq!(
        store.finish_split(split),
        Err(DatasetError::SplitAlreadyWritten {
            dataset_split_id: split,
            state: SplitState::Written,
        })
    );
}

/// dense-data-files-shape-mismatch through the writer: a 4,992-sample split holding 4,991 samples
/// is SampleCountMismatch and an unknown split UnknownSplit; nothing is left written.
#[test]
fn writer_shape_mismatch_leaves_nothing_written() {
    let dir = scratch("shape");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, ROWS, 4_991, plain);
    let refused = dense::write_split(&mut store, split, &dir, &train_stats(), dense::create_new);
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::SampleCountMismatch {
                rows: 4_991,
                sample_count: ROWS
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(
        refused.unwrap_err().domain_error(),
        Some("patchback.dataset.SampleCountMismatch")
    );
    assert!(names(&dir).is_empty(), "{:?}", names(&dir));
    assert!(store.dense_files_of(split).is_empty());
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);

    let unknown = DatasetSplitId(0xfd);
    let mut called = false;
    let refused = dense::write_split(
        &mut store,
        unknown,
        &dir,
        &train_stats(),
        |_: &Path, _: &[u8]| {
            called = true;
            Ok(())
        },
    );
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::UnknownSplit {
                dataset_split_id: DatasetSplitId(0xfd)
            }))
        ),
        "{refused:?}"
    );
    assert!(!called, "nothing is written for an unknown split");
    assert!(names(&dir).is_empty());
}

/// dense-data-files-content-refused through the writer: a NaN in one sample's y is refused with
/// NonFiniteValue after the read-back, and y.f32 is removed; x.f32 of the same split was verified
/// and stays; order.u32 is not written; the split stays Open.
#[test]
fn writer_non_finite_y_is_refused_and_removed() {
    let dir = scratch("nan-y");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, ROWS, 4_992, |i| {
        let (x, mut y) = plain(i);
        if i == 4_000 {
            y[3] = f64::NAN;
        }
        (x, y)
    });
    let refused = dense::write_split(&mut store, split, &dir, &train_stats(), dense::create_new);
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::NonFiniteValue {
                kind: DenseFileKind::Y
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(names(&dir), ["x.f32"]);
    let kinds: Vec<_> = store.dense_files_of(split).iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [DenseFileKind::X]);
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}

/// Da7-11: `f64 → f32` is Rust's `as`, so an `x` whose z-score (Da8-30) is past `f32::MAX`
/// becomes infinity in x.f32; the read-back finds it, the model refuses it, and the file is
/// removed.
#[test]
fn writer_an_x_past_f32_max_is_refused_by_the_read_back() {
    let dir = scratch("overflow-x");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, 16, |i| {
        let (mut x, y) = plain(i);
        if i == 9 {
            x[17] = 1e300;
        }
        (x, y)
    });
    assert!(store.samples_in(split)[9].x[17].is_finite());
    let stats = train_stats();
    let z = (1e300 - stats.means()[17]) / stats.stds()[17];
    assert!(
        z.is_finite() && z > f64::from(f32::MAX),
        "premise: its z-score {z} is finite and past f32::MAX"
    );
    let refused = dense::write_split(&mut store, split, &dir, &stats, dense::create_new);
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::NonFiniteValue {
                kind: DenseFileKind::X
            }))
        ),
        "{refused:?}"
    );
    assert!(names(&dir).is_empty());
    assert!(store.dense_files_of(split).is_empty());
}

/// The writer verifies what is on disk, not what it meant to write: a write that loses the last
/// byte of x.f32 is ByteCountMismatch (638,975 bytes for 4,992 rows), and the file is removed.
#[test]
fn writer_a_truncated_file_is_refused_with_its_byte_count() {
    let dir = scratch("truncated");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, ROWS, 4_992, plain);
    let refused = dense::write_split(
        &mut store,
        split,
        &dir,
        &train_stats(),
        |path: &Path, bytes: &[u8]| dense::create_new(path, &bytes[..bytes.len() - 1]),
    );
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::ByteCountMismatch {
                byte_count: 638_975,
                rows: ROWS,
                columns: 32
            }))
        ),
        "{refused:?}"
    );
    assert!(names(&dir).is_empty());
    assert!(store.dense_files_of(split).is_empty());
}

/// Da7-9: order.u32 is checked after the read-back, N entries, each below N, each once. An
/// entry of N and a repeated entry are refused (`DenseError::InvalidOrder`, the writer's own
/// check: the model declares no refusal for it), the file is removed and the split stays Open.
#[test]
fn writer_an_order_file_that_is_not_a_permutation_is_refused() {
    for (case, corrupt) in [
        ("out-of-range", (5usize, 16u32)),
        ("repeated", (5, 4)),
        ("last-out-of-range", (15, u32::MAX)),
    ] {
        let dir = scratch(&format!("order-{case}"));
        let mut store = DatasetStore::new();
        let split = filled(&mut store, 16, 16, plain);
        let refused = dense::write_split(
            &mut store,
            split,
            &dir,
            &train_stats(),
            |path: &Path, bytes: &[u8]| {
                let mut bytes = bytes.to_vec();
                if path.ends_with("order.u32") {
                    let (entry, value) = corrupt;
                    bytes[entry * 4..entry * 4 + 4].copy_from_slice(&value.to_le_bytes());
                }
                dense::create_new(path, &bytes)
            },
        );
        match refused {
            Err(DenseError::InvalidOrder {
                ref path,
                entry,
                value,
                rows,
            }) => {
                assert_eq!(path, &dir.join("order.u32"), "{case}");
                assert_eq!(
                    (entry, value, rows),
                    (corrupt.0 as u64, corrupt.1, 16),
                    "{case}"
                );
            }
            ref other => panic!("{case}: {other:?}"),
        }
        assert_eq!(refused.unwrap_err().domain_error(), None);
        assert_eq!(names(&dir), ["x.f32", "y.f32"], "{case}");
        assert_eq!(store.split(split).unwrap().state, SplitState::Open);
    }
}

/// A dense file that exists is never overwritten: the writer's `create_new` refuses it and the
/// file keeps its bytes.
#[test]
fn writer_an_existing_file_is_never_overwritten() {
    let dir = scratch("existing");
    fs::write(dir.join("y.f32"), "kept\n").unwrap();
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, 16, plain);
    let refused = dense::write_split(&mut store, split, &dir, &train_stats(), dense::create_new);
    match &refused {
        Err(DenseError::Io { path, error }) => {
            assert_eq!(path, &dir.join("y.f32"));
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read_to_string(dir.join("y.f32")).unwrap(), "kept\n");
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}
