//! Adversary cases for story:dense-data-files (wave a7, pass 1): `WriteDenseFile` and
//! `FinishSplit` against `spec/domains/dataset.yaml` (payloads, every combination of refusals in
//! the model's order), the writer's read-back against misbehaving `write` closures, the `f64 as
//! f32` edges of Da7-11 decoded by a reader of this file's own, hand-built `order.u32` files
//! (Da7-9), and a second `write_split` of one split.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use patchback_analysis::FeatureContractId;
use patchback_core::prng::NAME;
use patchback_dataset::dense::{self, DenseError};
use patchback_dataset::{
    DatasetError, DatasetId, DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset,
    PrngAlgorithm, SplitRole, SplitState, WriteDenseFile,
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

fn f32_bytes(count: usize, value: impl Fn(usize) -> f32) -> Vec<u8> {
    (0..count).flat_map(|i| value(i).to_le_bytes()).collect()
}

/// This file's own decoder: four bytes, least significant first, a value.
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

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-dense-adversary")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// A validation split of `count` samples holding `count` of them, sample `i` with `sample(i)`.
fn filled(
    store: &mut DatasetStore,
    count: i64,
    sample: impl Fn(u64) -> ([f64; 32], [f64; 8]),
) -> DatasetSplitId {
    let dataset = generate(store);
    let split = store
        .open_split(dataset, SplitRole::Validation, count)
        .unwrap();
    for i in 0..count as u64 {
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

/// `spec/domains/dataset.yaml`, `WriteDenseFile`, outcome `byte-count-mismatch`: the payload is
/// `byte_count: input.byte_count`. A command whose byte count agrees with its shape (2,048 for
/// 16 × 32) and whose bytes are short (2,044) is refused, and the refusal carries the command's
/// byte count, not the length of the bytes (implementor's D2).
#[test]
fn adversary_byte_count_mismatch_carries_the_input_byte_count() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let split = store
        .open_split(dataset, SplitRole::Validation, 16)
        .unwrap();
    let short = f32_bytes(16 * 32 - 1, |_| 0.5);
    assert_eq!(short.len(), 2_044);
    assert_eq!(
        store.write_dense_file(input(split, DenseFileKind::X, 16, 32, 2_048), &short),
        Err(DatasetError::ByteCountMismatch {
            byte_count: 2_048,
            rows: 16,
            columns: 32,
        }),
        "the payload's byte_count is input.byte_count (spec/domains/dataset.yaml)"
    );
}

/// The domain's `SplitAlreadyWritten` reads "The split's dense data is already written, so nothing
/// changed". `write_split` of a split already `Written` returns that refusal (its `FinishSplit`),
/// so it must not have written or recorded anything first: no call of `write`, no file in the
/// second directory, the three records of the first run and no more.
#[test]
fn adversary_write_split_of_a_written_split_changes_nothing() {
    let dir = scratch("written-twice");
    let first = dir.join("first");
    let second = dir.join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, plain);
    dense::write_split(&mut store, split, &first, dense::create_new).unwrap();
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);

    let mut calls: Vec<PathBuf> = Vec::new();
    let refused = dense::write_split(&mut store, split, &second, |path: &Path, bytes: &[u8]| {
        calls.push(path.to_path_buf());
        dense::create_new(path, bytes)
    });
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::SplitAlreadyWritten {
                state: SplitState::Written,
                ..
            }))
        ),
        "{refused:?}"
    );
    let recorded = store.dense_files_of(split).len();
    assert_eq!(
        (calls.len(), names(&second), recorded),
        (0, Vec::<String>::new(), 3),
        "SplitAlreadyWritten says nothing changed; written {calls:?}, records {recorded}"
    );
}

/// Every combination of the five `WriteDenseFile` refusals (31 non-empty subsets, for x.f32 and
/// for y.f32) gets the first in the model's order: `wrong-column-count`, `unknown-split`,
/// `sample-count-mismatch`, `byte-count-mismatch`, `non-finite-value`; nothing is recorded.
#[test]
fn adversary_every_combination_of_refusals_gets_the_first_in_the_model_s_order() {
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let split = store
        .open_split(dataset, SplitRole::Validation, 16)
        .unwrap();
    let unknown = DatasetSplitId(99);
    for kind in [DenseFileKind::X, DenseFileKind::Y] {
        for defects in 1u32..32 {
            let has = |bit: u32| defects & (1 << bit) != 0;
            let columns = kind.columns() + i64::from(has(0));
            let target = if has(1) { unknown } else { split };
            let rows: i64 = if has(2) { 15 } else { 16 };
            let byte_count = rows * columns * 4 - if has(3) { 4 } else { 0 };
            let values = usize::try_from(byte_count / 4).unwrap();
            let content = f32_bytes(values, |i| {
                if has(4) && i + 1 == values {
                    f32::NAN
                } else {
                    0.5
                }
            });
            let expected = match defects.trailing_zeros() {
                0 => DatasetError::WrongColumnCount { kind, columns },
                1 => DatasetError::UnknownSplit {
                    dataset_split_id: unknown,
                },
                2 => DatasetError::SampleCountMismatch {
                    rows,
                    sample_count: 16,
                },
                3 => DatasetError::ByteCountMismatch {
                    byte_count,
                    rows,
                    columns,
                },
                _ => DatasetError::NonFiniteValue { kind },
            };
            assert_eq!(
                store.write_dense_file(input(target, kind, rows, columns, byte_count), &content),
                Err(expected),
                "{kind:?} defects {defects:05b}"
            );
        }
    }
    assert!(store.dense_files_of(split).is_empty());
    assert!(store.dense_files_of(unknown).is_empty());
}

/// Da7-11 through the writer, decoded by this file's reader: an `f64` subnormal becomes `0.0`, an
/// `f32` subnormal stays non-zero, `-0.0` keeps its sign bit, and a value just past `f32::MAX`
/// that rounds to `f32::MAX` is finite and admitted; every value of x.f32 and y.f32 is the
/// sample's `as f32`, in draw order.
#[test]
fn adversary_the_f32_edges_round_trip_through_the_writer() {
    let just_past_max = f64::from(f32::MAX) + 2f64.powi(102);
    assert_eq!(
        just_past_max as f32,
        f32::MAX,
        "premise: rounds down to MAX"
    );
    let edges = [1e-310, 1e-40, -0.0, just_past_max, -just_past_max, 0.0];
    let dir = scratch("f32-edges");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, |i| {
        let (mut x, mut y) = plain(i);
        for (j, edge) in edges.iter().enumerate() {
            x[(j + i as usize) % 32] = *edge;
            y[(j + i as usize) % 8] = -*edge;
        }
        (x, y)
    });
    dense::write_split(&mut store, split, &dir, dense::create_new).unwrap();
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);

    let x = decode_bits(&fs::read(dir.join("x.f32")).unwrap());
    let y = decode_bits(&fs::read(dir.join("y.f32")).unwrap());
    let samples = store.samples_in(split);
    let expected_x: Vec<u32> = samples
        .iter()
        .flat_map(|s| s.x.iter().map(|&v| (v as f32).to_bits()))
        .collect();
    let expected_y: Vec<u32> = samples
        .iter()
        .flat_map(|s| s.y.iter().map(|&v| (v as f32).to_bits()))
        .collect();
    assert_eq!(x, expected_x);
    assert_eq!(y, expected_y);
    assert_eq!(x[0], 0, "1e-310 becomes +0.0");
    assert_eq!(x[1], (1e-40f64 as f32).to_bits());
    assert!(x[1] != 0 && x[1] < 0x0080_0000, "an f32 subnormal");
    assert_eq!(x[2], 0x8000_0000, "-0.0 keeps its sign");
    assert_eq!(x[3], f32::MAX.to_bits());
    assert_eq!(x[4], (-f32::MAX).to_bits());
    assert_eq!(y[0], 0x8000_0000, "-(1e-310) becomes -0.0");
}

/// Past the rounding midpoint above `f32::MAX` the value is infinite in the file: refused as
/// `NonFiniteValue` naming the file it is in (x or y, and in y also negative), that file removed,
/// nothing of it recorded.
#[test]
fn adversary_a_value_rounding_to_infinity_is_refused_naming_its_file() {
    let midpoint = f64::from(f32::MAX) + 2f64.powi(103);
    assert!((midpoint as f32).is_infinite(), "premise: ties to infinity");
    for (case, in_x, value, kind, left) in [
        ("x-inf", true, midpoint, DenseFileKind::X, vec![]),
        ("x-nan", true, f64::NAN, DenseFileKind::X, vec![]),
        (
            "y-neg-inf",
            false,
            -midpoint,
            DenseFileKind::Y,
            vec!["x.f32"],
        ),
        ("y-nan", false, f64::NAN, DenseFileKind::Y, vec!["x.f32"]),
    ] {
        let dir = scratch(case);
        let mut store = DatasetStore::new();
        let split = filled(&mut store, 16, |i| {
            let (mut x, mut y) = plain(i);
            if i == 15 {
                if in_x {
                    x[31] = value;
                } else {
                    y[7] = value;
                }
            }
            (x, y)
        });
        let refused = dense::write_split(&mut store, split, &dir, dense::create_new);
        assert!(
            matches!(refused, Err(DenseError::Dataset(DatasetError::NonFiniteValue { kind: k })) if k == kind),
            "{case}: {refused:?}"
        );
        assert_eq!(names(&dir), left, "{case}");
        let kinds: Vec<_> = store.dense_files_of(split).iter().map(|f| f.kind).collect();
        assert!(!kinds.contains(&kind), "{case}: {kinds:?}");
        assert_eq!(store.split(split).unwrap().state, SplitState::Open);
    }
}

/// Hand-built `order.u32` files (Da7-9): a permutation that is not the identity is admitted (the
/// check is N entries, each below N, each once); N + 1 entries, N − 1 entries and a length that is
/// not a multiple of 4 are the model's `ByteCountMismatch` with the file's length, not the
/// writer's `InvalidOrder`; a duplicate with one index missing is `InvalidOrder` at the duplicate,
/// which names no domain error and whose message names the file and the entry. A refused order
/// file is removed; x.f32 and y.f32 stay; the split stays `Open`.
#[test]
fn adversary_hand_built_order_files() {
    let reversed: Vec<u8> = (0..16u32).rev().flat_map(u32::to_le_bytes).collect();
    let longer: Vec<u8> = (0..17u32).flat_map(u32::to_le_bytes).collect();
    let shorter: Vec<u8> = (0..15u32).flat_map(u32::to_le_bytes).collect();
    let mut ragged: Vec<u8> = (0..16u32).flat_map(u32::to_le_bytes).collect();
    ragged.pop();
    let duplicate: Vec<u8> = [0u32, 1, 2, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for (case, order) in [
        ("reversed", &reversed),
        ("longer", &longer),
        ("shorter", &shorter),
        ("ragged", &ragged),
        ("duplicate", &duplicate),
    ] {
        let dir = scratch(&format!("order-{case}"));
        let mut store = DatasetStore::new();
        let split = filled(&mut store, 16, plain);
        let result = dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
            if path.ends_with("order.u32") {
                dense::create_new(path, order)
            } else {
                dense::create_new(path, bytes)
            }
        });
        let order_path = dir.join("order.u32");
        match case {
            "reversed" => {
                assert!(result.is_ok(), "{case}: {result:?}");
                assert_eq!(store.split(split).unwrap().state, SplitState::Written);
                assert_eq!(fs::read(&order_path).unwrap(), reversed);
                continue;
            }
            "duplicate" => {
                let error = result.unwrap_err();
                match &error {
                    DenseError::InvalidOrder {
                        path,
                        entry,
                        value,
                        rows,
                    } => assert_eq!(
                        (path, *entry, *value, *rows),
                        (&order_path, 3, 2, 16),
                        "{case}"
                    ),
                    other => panic!("{case}: {other:?}"),
                }
                assert_eq!(error.domain_error(), None);
                let message = error.to_string();
                assert!(
                    message.contains("order.u32") && message.contains("entry 3 is 2"),
                    "{message}"
                );
            }
            _ => {
                let byte_count = i64::try_from(order.len()).unwrap();
                assert!(
                    matches!(
                        result,
                        Err(DenseError::Dataset(DatasetError::ByteCountMismatch {
                            byte_count: b,
                            rows: 16,
                            columns: 1,
                        })) if b == byte_count
                    ),
                    "{case}: {result:?}"
                );
            }
        }
        assert_eq!(names(&dir), ["x.f32", "y.f32"], "{case}");
        let kinds: Vec<_> = store.dense_files_of(split).iter().map(|f| f.kind).collect();
        assert_eq!(kinds, [DenseFileKind::X, DenseFileKind::Y], "{case}");
        assert_eq!(store.split(split).unwrap().state, SplitState::Open);
    }
}

/// The model verifies the file on disk, whatever `write` did: a `write` that writes nothing is an
/// I/O refusal naming x.f32 (`NotFound`); one that writes the bytes twice is `ByteCountMismatch`
/// with twice the length and the file is removed; one that writes elsewhere is the same I/O
/// refusal, and its stray file is not the writer's to remove. Nothing is recorded in any case.
#[test]
fn adversary_a_misbehaving_write_is_caught_by_the_read_back() {
    let dir = scratch("write-nothing");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, plain);
    let refused = dense::write_split(&mut store, split, &dir, |_: &Path, _: &[u8]| Ok(()));
    match &refused {
        Err(DenseError::Io { path, error }) => {
            assert_eq!(path, &dir.join("x.f32"));
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(refused.unwrap_err().domain_error(), None);
    assert!(store.dense_files_of(split).is_empty());

    let dir = scratch("write-twice");
    let refused = dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
        let mut twice = bytes.to_vec();
        twice.extend_from_slice(bytes);
        dense::create_new(path, &twice)
    });
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::ByteCountMismatch {
                byte_count: 4_096,
                rows: 16,
                columns: 32,
            }))
        ),
        "{refused:?}"
    );
    assert!(names(&dir).is_empty());
    assert!(store.dense_files_of(split).is_empty());

    let dir = scratch("write-elsewhere");
    let refused = dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
        dense::create_new(&path.with_extension("elsewhere"), bytes)
    });
    assert!(
        matches!(&refused, Err(DenseError::Io { path, error }) if path == &dir.join("x.f32") && error.kind() == io::ErrorKind::NotFound),
        "{refused:?}"
    );
    assert_eq!(names(&dir), ["x.elsewhere"]);
    assert!(store.dense_files_of(split).is_empty());
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}

/// Characterisation of D4 and D5 (not a specification): after a partial failure (y.f32 could not
/// be written) x.f32 stays recorded and the split `Open`; a second `write_split` with a `write`
/// that overwrites records a second x.f32 of the split (the model declares no uniqueness of a kind
/// within a split), `DenseFiles` shows both, and `FinishSplit` admits the split; a third
/// `FinishSplit` is `wrong-state`. `WriteDenseFile` of a `Written` split is admitted.
#[test]
fn adversary_characterise_a_second_write_split_after_a_partial_failure() {
    let dir = scratch("second-after-partial");
    let mut store = DatasetStore::new();
    let split = filled(&mut store, 16, plain);
    let refused = dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
        if path.ends_with("y.f32") {
            Err(io::Error::other("refused by the test"))
        } else {
            dense::create_new(path, bytes)
        }
    });
    assert!(matches!(refused, Err(DenseError::Io { .. })), "{refused:?}");
    let kinds = |store: &DatasetStore| -> Vec<DenseFileKind> {
        store.dense_files_of(split).iter().map(|f| f.kind).collect()
    };
    assert_eq!(kinds(&store), [DenseFileKind::X]);
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
    assert_eq!(names(&dir), ["x.f32"]);

    dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
        fs::write(path, bytes)
    })
    .unwrap();
    assert_eq!(
        kinds(&store),
        [
            DenseFileKind::X,
            DenseFileKind::X,
            DenseFileKind::Y,
            DenseFileKind::Order
        ]
    );
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);
    assert!(matches!(
        store.finish_split(split),
        Err(DatasetError::SplitAlreadyWritten { .. })
    ));
    let content = fs::read(dir.join("x.f32")).unwrap();
    assert!(
        store
            .write_dense_file(input(split, DenseFileKind::X, 16, 32, 2_048), &content)
            .is_ok(),
        "WriteDenseFile of a Written split is not refused (D4)"
    );
    assert_eq!(kinds(&store).len(), 5);
}
