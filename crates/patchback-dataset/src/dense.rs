//! The ModelSpec dense-data writer of DESIGN.md § 7.2 (Da7-8, Da7-9, Da7-11): for one split, from
//! its samples in draw order, `x.f32` (each sample's 32 `x` values), `y.f32` (its 8 `y` values)
//! and `order.u32` (the row indices 0 to N − 1), each row-major and little-endian, then
//! `FinishSplit`.
//!
//! | File | Row | Bytes |
//! |---|---|---|
//! | `x.f32` | the sample's `features/1` values, `f64 as f32` | N × 32 × 4 |
//! | `y.f32` | the sample's normalized parameters, `f64 as f32` | N × 8 × 4 |
//! | `order.u32` | the row's index, `u32` | N × 4 |
//!
//! A split that is not `Open` is refused before anything is written or recorded
//! (`SplitAlreadyWritten`, Da7-18): its dense data is written, and the refusal says nothing
//! changed. The model's `WriteDenseFile` itself admits a file of a `Written` split (the story's
//! open question); only the writer refuses.
//!
//! Each file is verified by reading it back: [`DatasetStore::write_dense_file`] gets the bytes on
//! disk, with the file's length as the command's byte count, and refuses a byte count that is not
//! rows × columns × 4 and a value that is NaN or infinite (`f64 as f32` rounds to nearest, so a
//! value past `f32::MAX` is infinite here). The order file is checked by the writer before that:
//! N entries (its byte count), each below N, each once. A file that fails is removed and the
//! refusal returned; the files of the split verified before it stay recorded, and the split stays
//! `Open`.
//!
//! A split admits at most `u32::MAX` rows, so that its row count is itself a `u32` ([`crate::MAX_SAMPLES`]).
//! Each file is encoded whole in memory before it is written; bounding that is a later story.

use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::model::{
    DatasetError, DatasetSplitId, DatasetStore, DenseDataFileId, DenseFileKind, Sample, SplitState,
    WriteDenseFile,
};

/// Why a split's dense files were not all written.
#[derive(Debug)]
pub enum DenseError {
    /// The model refused a file or `FinishSplit`; the file is removed.
    Dataset(DatasetError),
    /// A file could not be written, read back or removed.
    Io { path: PathBuf, error: io::Error },
    /// Entry `entry` of `order.u32` is `value`, which is not below `rows` or repeats an earlier
    /// entry (Da7-9, provisional). The model declares no refusal for it (its UNMAPPED comment on
    /// `DenseDataFile`); this one is the writer's own. The file is removed.
    InvalidOrder {
        path: PathBuf,
        entry: u64,
        value: u32,
        rows: u64,
    },
    /// The split holds more than `u32::MAX` samples, so its row count is no `u32`; nothing was written.
    TooManyRows {
        dataset_split_id: DatasetSplitId,
        rows: u64,
    },
}

impl DenseError {
    /// The error of `spec/` this refusal is; none for the writer's own refusals.
    pub fn domain_error(&self) -> Option<&'static str> {
        match self {
            Self::Dataset(error) => Some(error.domain_error()),
            Self::Io { .. } | Self::InvalidOrder { .. } | Self::TooManyRows { .. } => None,
        }
    }
}

impl fmt::Display for DenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dataset(error) => write!(f, "{error}"),
            Self::Io { path, error } => write!(f, "cannot write {path:?}: {error}"),
            Self::InvalidOrder {
                path,
                entry,
                value,
                rows,
            } => write!(
                f,
                "{path:?}: entry {entry} is {value}, which is not below {rows} or repeats an \
                 earlier entry; order.u32 holds each row index once"
            ),
            Self::TooManyRows {
                dataset_split_id,
                rows,
            } => write!(
                f,
                "{dataset_split_id} holds {rows} samples, more than order.u32 indexes ({})",
                u32::MAX
            ),
        }
    }
}

impl std::error::Error for DenseError {}

impl From<DatasetError> for DenseError {
    fn from(error: DatasetError) -> Self {
        Self::Dataset(error)
    }
}

/// Writes `bytes` to `path`, which must not exist yet (`AlreadyExists` otherwise); a file it
/// created and could not fill is removed.
pub fn create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        let _ = fs::remove_file(path);
    }
    written
}

/// Writes, verifies and records the three dense files of the split `dataset_split_id` into `dir`
/// (which must exist), in the order x.f32, y.f32, order.u32, then `FinishSplit`; returns the
/// records' identities in that order.
///
/// `write` puts the bytes of one file on disk ([`create_new`] or a caller's own, which may
/// track what it created). Refused before anything is written or recorded, in this order, for an
/// unknown split (`UnknownSplit`), a split that is not `Open` (`SplitAlreadyWritten`, carrying
/// the split's state: its dense data is written, so nothing changes; Da7-18), and a row count that
/// is no `u32` ([`DenseError::TooManyRows`]); after a write, as the module documentation
/// says.
pub fn write_split(
    store: &mut DatasetStore,
    dataset_split_id: DatasetSplitId,
    dir: &Path,
    mut write: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> Result<[DenseDataFileId; 3], DenseError> {
    let Some(split) = store.split(dataset_split_id) else {
        return Err(DatasetError::UnknownSplit { dataset_split_id }.into());
    };
    if split.state != SplitState::Open {
        return Err(DatasetError::SplitAlreadyWritten {
            dataset_split_id,
            state: split.state,
        }
        .into());
    }
    let rows = store.samples_in(dataset_split_id).len();
    let order_rows = u32::try_from(rows).map_err(|_| DenseError::TooManyRows {
        dataset_split_id,
        rows: rows as u64,
    })?;
    let mut ids = [DenseDataFileId(0); 3];
    for (id, kind) in ids.iter_mut().zip(DenseFileKind::ALL) {
        let path = dir.join(kind.file_name());
        let bytes = encode(kind, &store.samples_in(dataset_split_id), order_rows);
        write(&path, &bytes).map_err(|error| DenseError::Io {
            path: path.clone(),
            error,
        })?;
        drop(bytes);
        *id = verify(store, dataset_split_id, kind, &path, order_rows).inspect_err(|_| {
            let _ = fs::remove_file(&path);
        })?;
    }
    store.finish_split(dataset_split_id)?;
    Ok(ids)
}

/// The bytes of the `kind` file of `samples`, row-major, little-endian.
fn encode(kind: DenseFileKind, samples: &[&Sample], rows: u32) -> Vec<u8> {
    match kind {
        DenseFileKind::X => samples
            .iter()
            .flat_map(|sample| sample.x.iter().flat_map(|&v| (v as f32).to_le_bytes()))
            .collect(),
        DenseFileKind::Y => samples
            .iter()
            .flat_map(|sample| sample.y.iter().flat_map(|&v| (v as f32).to_le_bytes()))
            .collect(),
        DenseFileKind::Order => (0..rows).flat_map(u32::to_le_bytes).collect(),
    }
}

/// Reads the file back, checks an order file's entries, and records it through `WriteDenseFile`.
fn verify(
    store: &mut DatasetStore,
    dataset_split_id: DatasetSplitId,
    kind: DenseFileKind,
    path: &Path,
    rows: u32,
) -> Result<DenseDataFileId, DenseError> {
    let content = fs::read(path).map_err(|error| DenseError::Io {
        path: path.to_path_buf(),
        error,
    })?;
    if kind == DenseFileKind::Order {
        check_order(path, &content, rows)?;
    }
    let input = WriteDenseFile {
        dataset_split_id,
        kind,
        path: path.to_path_buf(),
        rows: i64::from(rows),
        columns: kind.columns(),
        byte_count: i64::try_from(content.len()).unwrap_or(i64::MAX),
    };
    Ok(store.write_dense_file(input, &content)?)
}

/// Da7-9: each entry of an order file of `rows` entries is below `rows` and appears once. A file
/// of another length is left to the model's byte-count rule.
fn check_order(path: &Path, content: &[u8], rows: u32) -> Result<(), DenseError> {
    let (entries, remainder) = content.as_chunks::<4>();
    if entries.len() != rows as usize || !remainder.is_empty() {
        return Ok(());
    }
    let mut seen = vec![false; rows as usize];
    for (entry, bytes) in entries.iter().enumerate() {
        let value = u32::from_le_bytes(*bytes);
        match seen.get_mut(value as usize) {
            Some(seen) if !*seen => *seen = true,
            _ => {
                return Err(DenseError::InvalidOrder {
                    path: path.to_path_buf(),
                    entry: entry as u64,
                    value,
                    rows: u64::from(rows),
                });
            }
        }
    }
    Ok(())
}
