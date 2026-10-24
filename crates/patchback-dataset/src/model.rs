//! The model of `spec/domains/dataset.yaml` as plain types: the entities, their identities, the
//! errors, and [`DatasetStore`], whose methods are the commands `GenerateDataset`, `OpenSplit`,
//! `DrawPatch`, `AddSample`, `OpenDiagnosticSet` and `AddDiagnosticExample`, each answering its
//! outcomes in the order the model declares them.
//!
//! The domain's identities are Uuids; here each is a number the store hands out from 1, in the
//! order the store created its records, as `patchback_analysis::FeatureContractId` is.

use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

use patchback_analysis::{AnalysisError, FeatureContractId, FeatureError, NotComputableReason};
use patchback_dsp::RenderError;

macro_rules! identity {
    ($(#[$doc:meta])* $name:ident, $label:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($label, " #{}"), self.0)
            }
        }
    };
}

identity!(
    /// `patchback.dataset.DatasetId`.
    DatasetId,
    "dataset"
);
identity!(
    /// `patchback.dataset.DatasetSplitId`.
    DatasetSplitId,
    "split"
);
identity!(
    /// `patchback.dataset.SampledPatchId`.
    SampledPatchId,
    "sampled patch"
);
identity!(
    /// `patchback.dataset.SampleId`.
    SampleId,
    "sample"
);
identity!(
    /// `patchback.dataset.DiagnosticSetId`.
    DiagnosticSetId,
    "diagnostic set"
);
identity!(
    /// `patchback.dataset.DiagnosticExampleId`.
    DiagnosticExampleId,
    "diagnostic example"
);

/// `patchback.dataset.PrngAlgorithm`: the name of the generator a dataset was drawn with,
/// `xoshiro256**` ([`patchback_core::prng::NAME`]) for every dataset this crate generates.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrngAlgorithm(pub String);

/// `patchback.dataset.SplitRole`: the three splits of § 7.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SplitRole {
    Train,
    Validation,
    Test,
}

impl SplitRole {
    /// The roles in the order the generator fills them.
    pub const ALL: [Self; 3] = [Self::Train, Self::Validation, Self::Test];

    /// The variant name of the model.
    pub fn name(self) -> &'static str {
        match self {
            Self::Train => "Train",
            Self::Validation => "Validation",
            Self::Test => "Test",
        }
    }

    /// The split's record name: its key in `dataset.meta` and in what `patchback generate-dataset`
    /// prints (`train`, `validation`, `test`). Not its directory: see [`dir`](Self::dir).
    pub fn key(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Test => "test",
        }
    }

    /// The split's directory under a dataset's output directory (`train`, `val`, `test`; DESIGN.md
    /// § 7.2, `data/val/x.f32`), where its `samples.meta` and dense files go.
    pub fn dir(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "val",
            Self::Test => "test",
        }
    }
}

impl fmt::Display for SplitRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `patchback.dataset.DiagnosticKind`: the rejection cases § 7.1 keeps apart from the splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticKind {
    Silence,
    Noise,
    Clipping,
}

impl DiagnosticKind {
    pub const ALL: [Self; 3] = [Self::Silence, Self::Noise, Self::Clipping];

    /// The variant name of the model.
    pub fn name(self) -> &'static str {
        match self {
            Self::Silence => "Silence",
            Self::Noise => "Noise",
            Self::Clipping => "Clipping",
        }
    }

    /// The kind's key in `dataset.meta`, after `diagnostic_`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Silence => "silence",
            Self::Noise => "noise",
            Self::Clipping => "clipping",
        }
    }
}

/// Why a rendered patch is a diagnostic example rather than a sample (decision D-a6-12). Only the
/// constructors build one, so every cause is a diagnostic and has its [`kind`](Self::kind): a
/// refusal that is a fault of the render or of the feature contract has no cause.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticCause(Cause);

#[derive(Clone, Debug, PartialEq)]
enum Cause {
    Limited { limited_samples: u64 },
    Render(RenderError),
    Feature(FeatureError),
}

impl DiagnosticCause {
    /// The render's protective limiter changed `limited_samples` samples: [`DiagnosticKind::Clipping`].
    pub fn limited(limited_samples: u64) -> Self {
        Self(Cause::Limited { limited_samples })
    }

    /// The renderer's refusal when it is a diagnostic: a non-finite sample is
    /// [`DiagnosticKind::Noise`]. Every other refusal (rate, block size, note timing, length) is a
    /// fault of the request, not of the patch, and has no cause.
    pub fn from_render(error: RenderError) -> Option<Self> {
        match error {
            RenderError::NonFiniteRender { .. } => Some(Self(Cause::Render(error))),
            RenderError::UnsupportedSampleRate { .. }
            | RenderError::InvalidBlockSize { .. }
            | RenderError::InvalidNoteTiming { .. }
            | RenderError::RenderTooLong { .. } => None,
        }
    }

    /// The feature contract's refusal when it is a diagnostic: a value that cannot be computed
    /// (`FeatureNotComputable`) or is not finite (`NonFiniteFeature`). Every other refusal (an
    /// unknown, draft or mismatched contract) is a fault and has no cause.
    pub fn from_feature(error: FeatureError) -> Option<Self> {
        match error {
            FeatureError::FeatureNotComputable { .. } | FeatureError::NonFiniteFeature { .. } => {
                Some(Self(Cause::Feature(error)))
            }
            FeatureError::FeatureVersionTaken { .. }
            | FeatureError::FeatureCountMismatch { .. }
            | FeatureError::FeatureContractFrozen { .. }
            | FeatureError::FeatureContractNotFound { .. }
            | FeatureError::FeatureContractNotPublished { .. }
            | FeatureError::FeatureVersionMismatch { .. }
            | FeatureError::FeatureLengthMismatch { .. } => None,
        }
    }

    /// D-a6-12: `Clipping` when the limiter changed a sample; `Silence` for a silent window or a
    /// silent input; `Noise` for every other value that cannot be computed or is not finite.
    pub fn kind(&self) -> DiagnosticKind {
        match &self.0 {
            Cause::Limited { .. } => DiagnosticKind::Clipping,
            Cause::Feature(FeatureError::FeatureNotComputable {
                reason:
                    NotComputableReason::SilentWindow { .. }
                    | NotComputableReason::Analysis(AnalysisError::SilentInput { .. }),
                ..
            }) => DiagnosticKind::Silence,
            Cause::Render(_) | Cause::Feature(_) => DiagnosticKind::Noise,
        }
    }

    /// The refusal that put the patch in the diagnostic set; none for a limited render, which is
    /// no refusal.
    pub fn render_error(&self) -> Option<RenderError> {
        match &self.0 {
            Cause::Render(error) => Some(*error),
            _ => None,
        }
    }

    pub fn feature_error(&self) -> Option<&FeatureError> {
        match &self.0 {
            Cause::Feature(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for DiagnosticCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Cause::Limited { limited_samples } => write!(
                f,
                "the protective limiter changed {limited_samples} samples"
            ),
            Cause::Render(error) => write!(f, "{}: {error}", error.domain_error()),
            Cause::Feature(error) => write!(f, "{}: {error}", error.domain_error()),
        }
    }
}

/// `GenerateDataset`'s input.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerateDataset {
    pub seed: u64,
    pub prng_algorithm: PrngAlgorithm,
    pub batch_size: u64,
    pub feature_contract_id: FeatureContractId,
    /// The topology's name, `subtractive-mono/1` (`patchback.patch.TopologyId`).
    pub topology_id: String,
    pub out_dir: PathBuf,
}

/// `patchback.dataset.Dataset`: one run of the generator, in state `Generated`.
#[derive(Clone, Debug, PartialEq)]
pub struct Dataset {
    pub dataset_id: DatasetId,
    pub seed: u64,
    pub prng_algorithm: PrngAlgorithm,
    pub batch_size: u64,
    pub feature_contract_id: FeatureContractId,
    pub topology_id: String,
    pub out_dir: PathBuf,
}

/// `patchback.dataset.DatasetSplit`, in state `Open` (its dense files, and the state `Written`,
/// are story:dense-data-files).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetSplit {
    pub dataset_split_id: DatasetSplitId,
    pub dataset_id: DatasetId,
    pub role: SplitRole,
    /// The model's `Integer`: the count as opened, a positive multiple of the batch size.
    pub sample_count: i64,
}

/// `patchback.dataset.SampledPatch`: a patch seed drawn into one split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampledPatch {
    pub sampled_patch_id: SampledPatchId,
    pub dataset_id: DatasetId,
    pub dataset_split_id: DatasetSplitId,
    pub patch_seed: u64,
}

/// `patchback.dataset.Sample`: one `(x, y)` pair of a drawn patch, with its note as metadata.
/// Its split is its patch's.
///
/// `x` holds the 32 `features/1` values and `y` the eight normalized parameters, both as `f64`,
/// the values the extractor and the draw produced; the `f32` files ModelSpec reads are
/// story:dense-data-files.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub sample_id: SampleId,
    pub sampled_patch_id: SampledPatchId,
    pub midi_note: u8,
    /// The gate in samples at 48 kHz.
    pub gate_samples: u64,
    pub x: [f64; 32],
    pub y: [f64; 8],
}

/// `patchback.dataset.DiagnosticSet`: a dataset's rejection cases, apart from its splits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticSet {
    pub diagnostic_set_id: DiagnosticSetId,
    pub dataset_id: DatasetId,
}

/// What `AddDiagnosticExample` records beside the model's `kind`: the patch seed, the note, and
/// the cause, from which the kind follows ([`DiagnosticCause::kind`]).
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticCase {
    pub patch_seed: u64,
    pub midi_note: u8,
    pub gate_samples: u64,
    pub cause: DiagnosticCause,
}

/// `patchback.dataset.DiagnosticExample`: one rejected patch. It names no split and no sampled
/// patch, so it is never a sample.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticExample {
    pub diagnostic_example_id: DiagnosticExampleId,
    pub diagnostic_set_id: DiagnosticSetId,
    pub kind: DiagnosticKind,
    pub patch_seed: u64,
    pub midi_note: u8,
    pub gate_samples: u64,
    pub cause: DiagnosticCause,
}

/// A refusal of the dataset model; each names an error of `spec/domains/dataset.yaml`
/// ([`Self::domain_error`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatasetError {
    /// `patchback.dataset.SplitCountNotBatchMultiple`: the count is not a positive multiple of
    /// the dataset's batch size; no split was opened.
    SplitCountNotBatchMultiple {
        role: SplitRole,
        sample_count: i64,
        batch_size: u64,
    },
    /// `patchback.dataset.UnknownDataset`.
    UnknownDataset { dataset_id: DatasetId },
    /// `patchback.dataset.UnknownSplit`: no split of the dataset carries this identity.
    UnknownSplit { dataset_split_id: DatasetSplitId },
    /// `patchback.dataset.UnknownDiagnosticSet`.
    UnknownDiagnosticSet { diagnostic_set_id: DiagnosticSetId },
    /// `patchback.dataset.PatchAlreadyDrawn`: the seed is drawn into a split of the dataset.
    PatchAlreadyDrawn {
        patch_seed: u64,
        dataset_split_id: DatasetSplitId,
    },
    /// `patchback.dataset.UnknownPatch`.
    UnknownPatch { sampled_patch_id: SampledPatchId },
}

impl DatasetError {
    /// Every error of `spec/domains/dataset.yaml` this crate emits.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.dataset.SplitCountNotBatchMultiple",
        "patchback.dataset.UnknownDataset",
        "patchback.dataset.UnknownSplit",
        "patchback.dataset.UnknownDiagnosticSet",
        "patchback.dataset.PatchAlreadyDrawn",
        "patchback.dataset.UnknownPatch",
    ];

    /// The error of `spec/domains/dataset.yaml` this refusal is.
    pub fn domain_error(&self) -> &'static str {
        match self {
            Self::SplitCountNotBatchMultiple { .. } => {
                "patchback.dataset.SplitCountNotBatchMultiple"
            }
            Self::UnknownDataset { .. } => "patchback.dataset.UnknownDataset",
            Self::UnknownSplit { .. } => "patchback.dataset.UnknownSplit",
            Self::UnknownDiagnosticSet { .. } => "patchback.dataset.UnknownDiagnosticSet",
            Self::PatchAlreadyDrawn { .. } => "patchback.dataset.PatchAlreadyDrawn",
            Self::UnknownPatch { .. } => "patchback.dataset.UnknownPatch",
        }
    }

    /// One value of every variant. The `match` has no wildcard, so a new variant does not compile
    /// until it is listed here, and `tests/domain.rs` then checks its name.
    pub fn examples() -> Vec<Self> {
        let examples = vec![
            Self::SplitCountNotBatchMultiple {
                role: SplitRole::Train,
                sample_count: 50_008,
                batch_size: 16,
            },
            Self::UnknownDataset {
                dataset_id: DatasetId(254),
            },
            Self::UnknownSplit {
                dataset_split_id: DatasetSplitId(3),
            },
            Self::UnknownDiagnosticSet {
                diagnostic_set_id: DiagnosticSetId(2),
            },
            Self::PatchAlreadyDrawn {
                patch_seed: 7,
                dataset_split_id: DatasetSplitId(2),
            },
            Self::UnknownPatch {
                sampled_patch_id: SampledPatchId(255),
            },
        ];
        for example in &examples {
            match example {
                Self::SplitCountNotBatchMultiple { .. }
                | Self::UnknownDataset { .. }
                | Self::UnknownSplit { .. }
                | Self::UnknownDiagnosticSet { .. }
                | Self::PatchAlreadyDrawn { .. }
                | Self::UnknownPatch { .. } => {}
            }
        }
        examples
    }
}

impl fmt::Display for DatasetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SplitCountNotBatchMultiple {
                role,
                sample_count,
                batch_size,
            } => write!(
                f,
                "the {role} split of {sample_count} samples is not a positive multiple of the \
                 batch size {batch_size}"
            ),
            Self::UnknownDataset { dataset_id } => write!(f, "no {dataset_id}"),
            Self::UnknownSplit { dataset_split_id } => {
                write!(f, "no {dataset_split_id} in the dataset")
            }
            Self::UnknownDiagnosticSet { diagnostic_set_id } => write!(f, "no {diagnostic_set_id}"),
            Self::PatchAlreadyDrawn {
                patch_seed,
                dataset_split_id,
            } => write!(
                f,
                "patch seed {patch_seed} is already drawn into a split of the dataset; it cannot \
                 be drawn into {dataset_split_id}"
            ),
            Self::UnknownPatch { sampled_patch_id } => write!(f, "no {sampled_patch_id}"),
        }
    }
}

impl std::error::Error for DatasetError {}

/// Whether `sample_count` is a positive multiple of `batch_size` (`OpenSplit`'s external
/// decision); a batch size of 0 divides no count.
pub fn is_batch_multiple(sample_count: i64, batch_size: u64) -> bool {
    sample_count > 0 && batch_size > 0 && (sample_count as u64).is_multiple_of(batch_size)
}

/// The records of the model, every dataset and its children, in creation order. A record of
/// identity `n` is the `n`th of its kind, so a lookup by identity is an index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DatasetStore {
    datasets: Vec<Dataset>,
    splits: Vec<DatasetSplit>,
    patches: Vec<SampledPatch>,
    samples: Vec<Sample>,
    diagnostic_sets: Vec<DiagnosticSet>,
    diagnostic_examples: Vec<DiagnosticExample>,
    /// The patch seeds drawn into each dataset: `DrawPatch`'s patch-identity rule.
    drawn: HashSet<(DatasetId, u64)>,
}

/// The record of identity `id` (1-based) of `records`.
fn at<T>(records: &[T], id: u64) -> Option<&T> {
    let index = usize::try_from(id.checked_sub(1)?).ok()?;
    records.get(index)
}

impl DatasetStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// `GenerateDataset`: the dataset with its seed and PRNG recorded. It is never refused.
    pub fn generate_dataset(&mut self, input: GenerateDataset) -> DatasetId {
        let dataset_id = DatasetId(self.datasets.len() as u64 + 1);
        self.datasets.push(Dataset {
            dataset_id,
            seed: input.seed,
            prng_algorithm: input.prng_algorithm,
            batch_size: input.batch_size,
            feature_contract_id: input.feature_contract_id,
            topology_id: input.topology_id,
            out_dir: input.out_dir,
        });
        dataset_id
    }

    /// `OpenSplit`: refused, in this order, for an unknown dataset and for a count that is not a
    /// positive multiple of its batch size ([`is_batch_multiple`]).
    pub fn open_split(
        &mut self,
        dataset_id: DatasetId,
        role: SplitRole,
        sample_count: i64,
    ) -> Result<DatasetSplitId, DatasetError> {
        let dataset = self.known_dataset(dataset_id)?;
        if !is_batch_multiple(sample_count, dataset.batch_size) {
            return Err(DatasetError::SplitCountNotBatchMultiple {
                role,
                sample_count,
                batch_size: dataset.batch_size,
            });
        }
        let dataset_split_id = DatasetSplitId(self.splits.len() as u64 + 1);
        self.splits.push(DatasetSplit {
            dataset_split_id,
            dataset_id,
            role,
            sample_count,
        });
        Ok(dataset_split_id)
    }

    /// `DrawPatch`: refused when `patch_seed` is drawn into any split of the dataset
    /// (`PatchAlreadyDrawn`).
    ///
    /// Before that, and beyond the model, an unknown dataset (`UnknownDataset`) and a split the
    /// dataset does not hold (`UnknownSplit`) are refused: the model's `DrawPatch` notes that ESS
    /// 0.67.0 cannot state these two refusals (its Gap comment), and drawing a patch into a split
    /// of another dataset would break the isolation the command exists for.
    pub fn draw_patch(
        &mut self,
        dataset_id: DatasetId,
        dataset_split_id: DatasetSplitId,
        patch_seed: u64,
    ) -> Result<SampledPatchId, DatasetError> {
        self.known_dataset(dataset_id)?;
        if self
            .split(dataset_split_id)
            .is_none_or(|split| split.dataset_id != dataset_id)
        {
            return Err(DatasetError::UnknownSplit { dataset_split_id });
        }
        if !self.drawn.insert((dataset_id, patch_seed)) {
            return Err(DatasetError::PatchAlreadyDrawn {
                patch_seed,
                dataset_split_id,
            });
        }
        let sampled_patch_id = SampledPatchId(self.patches.len() as u64 + 1);
        self.patches.push(SampledPatch {
            sampled_patch_id,
            dataset_id,
            dataset_split_id,
            patch_seed,
        });
        Ok(sampled_patch_id)
    }

    /// `AddSample`: refused for a patch the store does not hold (`UnknownPatch`). The model
    /// declares no other refusal: the generator checks `x` against the feature contract and draws
    /// `y` in `[0, 1)` before it adds a sample.
    pub fn add_sample(
        &mut self,
        sampled_patch_id: SampledPatchId,
        midi_note: u8,
        gate_samples: u64,
        x: [f64; 32],
        y: [f64; 8],
    ) -> Result<SampleId, DatasetError> {
        if self.patch(sampled_patch_id).is_none() {
            return Err(DatasetError::UnknownPatch { sampled_patch_id });
        }
        let sample_id = SampleId(self.samples.len() as u64 + 1);
        self.samples.push(Sample {
            sample_id,
            sampled_patch_id,
            midi_note,
            gate_samples,
            x,
            y,
        });
        Ok(sample_id)
    }

    /// `OpenDiagnosticSet`: refused for an unknown dataset.
    pub fn open_diagnostic_set(
        &mut self,
        dataset_id: DatasetId,
    ) -> Result<DiagnosticSetId, DatasetError> {
        self.known_dataset(dataset_id)?;
        let diagnostic_set_id = DiagnosticSetId(self.diagnostic_sets.len() as u64 + 1);
        self.diagnostic_sets.push(DiagnosticSet {
            diagnostic_set_id,
            dataset_id,
        });
        Ok(diagnostic_set_id)
    }

    /// `AddDiagnosticExample`: refused for an unknown diagnostic set. The model's `kind` is the
    /// case's [`DiagnosticCause::kind`], so a kind never disagrees with its cause.
    pub fn add_diagnostic_example(
        &mut self,
        diagnostic_set_id: DiagnosticSetId,
        case: DiagnosticCase,
    ) -> Result<DiagnosticExampleId, DatasetError> {
        if self.diagnostic_set(diagnostic_set_id).is_none() {
            return Err(DatasetError::UnknownDiagnosticSet { diagnostic_set_id });
        }
        let diagnostic_example_id = DiagnosticExampleId(self.diagnostic_examples.len() as u64 + 1);
        self.diagnostic_examples.push(DiagnosticExample {
            diagnostic_example_id,
            diagnostic_set_id,
            kind: case.cause.kind(),
            patch_seed: case.patch_seed,
            midi_note: case.midi_note,
            gate_samples: case.gate_samples,
            cause: case.cause,
        });
        Ok(diagnostic_example_id)
    }

    fn known_dataset(&self, dataset_id: DatasetId) -> Result<&Dataset, DatasetError> {
        self.dataset(dataset_id)
            .ok_or(DatasetError::UnknownDataset { dataset_id })
    }

    pub fn dataset(&self, id: DatasetId) -> Option<&Dataset> {
        at(&self.datasets, id.0)
    }

    pub fn split(&self, id: DatasetSplitId) -> Option<&DatasetSplit> {
        at(&self.splits, id.0)
    }

    /// The splits of a dataset, in the order they were opened.
    pub fn splits_of(&self, dataset_id: DatasetId) -> impl Iterator<Item = &DatasetSplit> {
        self.splits
            .iter()
            .filter(move |s| s.dataset_id == dataset_id)
    }

    pub fn patch(&self, id: SampledPatchId) -> Option<&SampledPatch> {
        at(&self.patches, id.0)
    }

    /// The patches drawn into the splits of a dataset, in the order they were drawn.
    pub fn patches_of(&self, dataset_id: DatasetId) -> impl Iterator<Item = &SampledPatch> {
        self.patches
            .iter()
            .filter(move |p| p.dataset_id == dataset_id)
    }

    pub fn sample(&self, id: SampleId) -> Option<&Sample> {
        at(&self.samples, id.0)
    }

    /// The samples of a split, in the order they were added: its patches' samples.
    pub fn samples_in(&self, dataset_split_id: DatasetSplitId) -> Vec<&Sample> {
        self.samples
            .iter()
            .filter(|sample| {
                self.patch(sample.sampled_patch_id)
                    .is_some_and(|p| p.dataset_split_id == dataset_split_id)
            })
            .collect()
    }

    pub fn diagnostic_set(&self, id: DiagnosticSetId) -> Option<&DiagnosticSet> {
        at(&self.diagnostic_sets, id.0)
    }

    pub fn diagnostic_example(&self, id: DiagnosticExampleId) -> Option<&DiagnosticExample> {
        at(&self.diagnostic_examples, id.0)
    }

    /// The examples of a diagnostic set, in the order they were added.
    pub fn diagnostic_examples_in(&self, id: DiagnosticSetId) -> Vec<&DiagnosticExample> {
        self.diagnostic_examples
            .iter()
            .filter(|e| e.diagnostic_set_id == id)
            .collect()
    }
}
