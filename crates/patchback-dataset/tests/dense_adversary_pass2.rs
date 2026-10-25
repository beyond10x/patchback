//! Adversary cases for story:dense-data-files (wave a7, pass 2) after the correction (Da7-17,
//! Da7-18, Da7-19): `ByteCountMismatch` carries the command's values whatever the content's
//! length (longer, equal but inconsistent, empty), shapes the model refuses earlier (negative,
//! past `u32::MAX`, a product that wraps an `i64`), the non-finite rule that is x.f32 and y.f32
//! only, and `write_split` of a split that is not `Open` or was left half-written.

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

/// A validation split opened with `count` samples and holding none.
fn opened(store: &mut DatasetStore, count: i64) -> DatasetSplitId {
    let dataset = generate(store);
    store
        .open_split(dataset, SplitRole::Validation, count)
        .unwrap()
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

fn mismatch(byte_count: i64, rows: i64, columns: i64) -> Result<(), DatasetError> {
    Err(DatasetError::ByteCountMismatch {
        byte_count,
        rows,
        columns,
    })
}

/// `count` bytes of 0x3f (each four of them a finite `f32`, 0.746…).
fn bytes(count: usize) -> Vec<u8> {
    vec![0x3f; count]
}

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-dataset-dense-adversary-pass2")
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

/// A validation split of 16 samples holding all 16.
fn filled(store: &mut DatasetStore) -> DatasetSplitId {
    let dataset = generate(store);
    let split = store
        .open_split(dataset, SplitRole::Validation, 16)
        .unwrap();
    for i in 0..16u64 {
        let patch = store.draw_patch(dataset, split, i).unwrap();
        let x = std::array::from_fn(|j| i as f64 + j as f64 / 64.0);
        let y = std::array::from_fn(|j| j as f64 / 8.0);
        store.add_sample(patch, 60, 24_000, x, y).unwrap();
    }
    split
}

/// Da7-17, the content-length branch with content longer than the byte count: the refusal
/// carries the command's byte count (2,048 for x, 512 for y, 64 for order), never the content's.
#[test]
fn adversary2_content_longer_than_the_byte_count_carries_the_input() {
    let mut store = DatasetStore::new();
    let split = opened(&mut store, 16);
    for (kind, columns, byte_count) in [
        (DenseFileKind::X, 32, 2_048),
        (DenseFileKind::Y, 8, 512),
        (DenseFileKind::Order, 1, 64),
    ] {
        for extra in [1usize, 4, 2_048] {
            let content = bytes(byte_count as usize + extra);
            assert_eq!(
                store
                    .write_dense_file(input(split, kind, 16, columns, byte_count), &content)
                    .map(drop),
                mismatch(byte_count, 16, columns),
                "{kind:?} with {} bytes",
                content.len()
            );
        }
    }
    assert!(store.dense_files_of(split).is_empty());
}

/// Da7-17, the arithmetic branch: content exactly as long as the command's byte count, the byte
/// count not rows × columns × 4 (short, long, zero with empty content on a 16-row split). The
/// refusal carries the command's values. Both branches now return the same value, so which of
/// them answers is not observable.
#[test]
fn adversary2_a_byte_count_the_content_agrees_with_but_the_shape_does_not() {
    let mut store = DatasetStore::new();
    let split = opened(&mut store, 16);
    for (kind, columns, consistent) in [
        (DenseFileKind::X, 32, 2_048i64),
        (DenseFileKind::Y, 8, 512),
        (DenseFileKind::Order, 1, 64),
    ] {
        for byte_count in [consistent - 4, consistent - 1, consistent + 4, 0] {
            let content = bytes(byte_count as usize);
            assert_eq!(
                store
                    .write_dense_file(input(split, kind, 16, columns, byte_count), &content)
                    .map(drop),
                mismatch(byte_count, 16, columns),
                "{kind:?} byte count {byte_count}"
            );
        }
    }
    assert!(store.dense_files_of(split).is_empty());
}

/// Negative shapes with a consistent byte count are refused by the earlier outcomes, with the
/// command's values: a negative column count is `WrongColumnCount`, negative rows are
/// `SampleCountMismatch` (the split's count is positive), never `ByteCountMismatch` and never
/// recorded.
#[test]
fn adversary2_negative_shapes_are_refused_before_the_byte_count() {
    let mut store = DatasetStore::new();
    let split = opened(&mut store, 16);
    assert_eq!(
        store
            .write_dense_file(input(split, DenseFileKind::X, -16, 32, -2_048), &[])
            .map(drop),
        Err(DatasetError::SampleCountMismatch {
            rows: -16,
            sample_count: 16
        })
    );
    assert_eq!(
        store
            .write_dense_file(input(split, DenseFileKind::X, -16, -32, 2_048), &[])
            .map(drop),
        Err(DatasetError::WrongColumnCount {
            kind: DenseFileKind::X,
            columns: -32
        })
    );
    assert_eq!(
        store
            .write_dense_file(
                input(split, DenseFileKind::Y, i64::MIN, 8, i64::MIN),
                &bytes(512)
            )
            .map(drop),
        Err(DatasetError::SampleCountMismatch {
            rows: i64::MIN,
            sample_count: 16
        })
    );
    assert!(store.dense_files_of(split).is_empty());
}

/// Rows past `u32::MAX` on a split opened that large (the model bounds no split count): the
/// arithmetic is exact, so a consistent byte count passes it and the content, which cannot be
/// that long, is `ByteCountMismatch` with the command's values; a byte count of 0 for 2^61 rows of
/// 32 columns (rows × columns × 4 = 2^68, 0 in a wrapping `i64`) is refused, not admitted with
/// empty content; and for the largest batch multiple of `i64`, whose product passes `i64::MAX`,
/// every byte count is refused, without a panic.
#[test]
fn adversary2_rows_past_u32_max_are_refused_with_the_command_s_values() {
    let mut store = DatasetStore::new();
    let rows = 1i64 << 32;
    let split = opened(&mut store, rows);
    let consistent = rows * 32 * 4;
    assert_eq!(
        store
            .write_dense_file(
                input(split, DenseFileKind::X, rows, 32, consistent),
                &bytes(2_048)
            )
            .map(drop),
        mismatch(consistent, rows, 32)
    );

    let mut store = DatasetStore::new();
    let rows = 1i64 << 61;
    let split = opened(&mut store, rows);
    assert_eq!(
        rows.wrapping_mul(32).wrapping_mul(4),
        0,
        "premise: wraps to 0"
    );
    assert_eq!(
        store
            .write_dense_file(input(split, DenseFileKind::X, rows, 32, 0), &[])
            .map(drop),
        mismatch(0, rows, 32)
    );

    let mut store = DatasetStore::new();
    let rows = i64::MAX - 15;
    let split = opened(&mut store, rows);
    for byte_count in [i64::MAX, i64::MIN, 0, rows.wrapping_mul(32).wrapping_mul(4)] {
        assert_eq!(
            store
                .write_dense_file(input(split, DenseFileKind::X, rows, 32, byte_count), &[])
                .map(drop),
            mismatch(byte_count, rows, 32),
            "byte count {byte_count}"
        );
    }
    assert!(store.dense_files_of(split).is_empty());
}

/// `non-finite-value` is "an x.f32 or y.f32 value is NaN or Infinity": the model reads no `f32`
/// in `order.u32`, so an order file whose every entry has the bits of a NaN or an infinity is
/// recorded (its entries are the writer's check, Da7-9, not the model's).
#[test]
fn adversary2_order_u32_bits_that_read_as_non_finite_f32_are_admitted_by_the_model() {
    let mut store = DatasetStore::new();
    let split = opened(&mut store, 16);
    for bits in [0xffff_ffffu32, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000] {
        assert!(!f32::from_bits(bits).is_finite(), "premise: {bits:#x}");
        let content: Vec<u8> = (0..16).flat_map(|_| bits.to_le_bytes()).collect();
        let recorded =
            store.write_dense_file(input(split, DenseFileKind::Order, 16, 1, 64), &content);
        assert!(recorded.is_ok(), "{bits:#x}: {recorded:?}");
    }
    let kinds: Vec<_> = store.dense_files_of(split).iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [DenseFileKind::Order; 4]);
}

/// Da7-18 exactly: `write_split` of a `Written` split returns
/// `SplitAlreadyWritten { dataset_split_id, state: Written }`, the domain's name, a message that
/// says nothing changed; it calls `write` never, leaves the first run's files byte for byte and
/// its three records alone. An unknown split is still `UnknownSplit`, also without a write.
#[test]
fn adversary2_write_split_of_a_written_split_names_the_split_and_writes_nothing() {
    let dir = scratch("written");
    let mut store = DatasetStore::new();
    let split = filled(&mut store);
    dense::write_split(&mut store, split, &dir, dense::create_new).unwrap();
    let before: Vec<(String, Vec<u8>)> = names(&dir)
        .into_iter()
        .map(|name| {
            let content = fs::read(dir.join(&name)).unwrap();
            (name, content)
        })
        .collect();
    let records_before = store.dense_files_of(split).len();

    let mut calls = 0;
    let refused = dense::write_split(&mut store, split, &dir, |_: &Path, _: &[u8]| {
        calls += 1;
        Ok(())
    });
    match &refused {
        Err(DenseError::Dataset(error)) => assert_eq!(
            error,
            &DatasetError::SplitAlreadyWritten {
                dataset_split_id: split,
                state: SplitState::Written,
            }
        ),
        other => panic!("{other:?}"),
    }
    let error = refused.unwrap_err();
    assert_eq!(
        error.domain_error(),
        Some("patchback.dataset.SplitAlreadyWritten")
    );
    assert!(error.to_string().contains("nothing changed"), "{error}");
    assert_eq!(calls, 0);
    let after: Vec<(String, Vec<u8>)> = names(&dir)
        .into_iter()
        .map(|name| {
            let content = fs::read(dir.join(&name)).unwrap();
            (name, content)
        })
        .collect();
    assert_eq!(after, before);
    assert_eq!((records_before, store.dense_files_of(split).len()), (3, 3));
    assert_eq!(store.split(split).unwrap().state, SplitState::Written);

    let unknown = DatasetSplitId(split.0 + 1);
    let refused = dense::write_split(&mut store, unknown, &dir, |_: &Path, _: &[u8]| {
        calls += 1;
        Ok(())
    });
    assert!(
        matches!(
            refused,
            Err(DenseError::Dataset(DatasetError::UnknownSplit { dataset_split_id }))
                if dataset_split_id == unknown
        ),
        "{refused:?}"
    );
    assert_eq!(calls, 0);
}

/// Characterisation (not a specification): a split left `Open` by a partial failure (x.f32
/// verified and recorded, y.f32 not written) and given to `write_split` again with `create_new`
/// is refused at its first file, x.f32, as `AlreadyExists` (an I/O refusal, no domain error);
/// the existing x.f32 keeps its bytes and is not removed, the one x record stays, nothing else is
/// written and the split stays `Open`. The writer cannot finish such a split with `create_new`.
#[test]
fn adversary2_characterise_a_second_create_new_write_split_after_a_partial_failure() {
    let dir = scratch("partial-then-create-new");
    let mut store = DatasetStore::new();
    let split = filled(&mut store);
    let refused = dense::write_split(&mut store, split, &dir, |path: &Path, bytes: &[u8]| {
        if path.ends_with("y.f32") {
            Err(io::Error::other("refused by the test"))
        } else {
            dense::create_new(path, bytes)
        }
    });
    assert!(matches!(refused, Err(DenseError::Io { .. })), "{refused:?}");
    let x_before = fs::read(dir.join("x.f32")).unwrap();
    assert_eq!(x_before.len(), 2_048);

    let again = dense::write_split(&mut store, split, &dir, dense::create_new);
    match &again {
        Err(DenseError::Io { path, error }) => {
            assert_eq!(path, &dir.join("x.f32"));
            assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(again.unwrap_err().domain_error(), None);
    assert_eq!(names(&dir), ["x.f32"]);
    assert_eq!(fs::read(dir.join("x.f32")).unwrap(), x_before);
    let kinds: Vec<_> = store.dense_files_of(split).iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [DenseFileKind::X]);
    assert_eq!(store.split(split).unwrap().state, SplitState::Open);
}
