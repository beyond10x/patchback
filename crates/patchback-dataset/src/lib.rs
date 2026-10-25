//! Deterministic training examples (DESIGN.md § 4.1, § 7.1): the model of
//! `spec/domains/dataset.yaml`, the seeded generator of `patchback generate-dataset`, and the
//! writer of the ModelSpec dense-data files (§ 7.2).
//!
//! # The model
//!
//! [`DatasetStore`] holds the records of the model, and its methods are the commands
//! `GenerateDataset`, `OpenSplit`, `DrawPatch`, `AddSample`, `OpenDiagnosticSet`,
//! `AddDiagnosticExample`, `WriteDenseFile` and `FinishSplit`, each refusing with a
//! [`DatasetError`] that names a declared error, in the order the model declares its outcomes.
//! Two refusals go beyond the model: `DrawPatch` refuses an unknown dataset and a split the
//! dataset does not hold (see [`DatasetStore::draw_patch`]). A [`Sample`] carries its `x` (32
//! `features/1` values) and `y` (8 normalized parameters) as `f64`; a [`DiagnosticExample`]
//! carries its patch seed, note and [`DiagnosticCause`]. A [`DatasetSplit`] is
//! [`SplitState::Open`] until `FinishSplit` finds its x.f32 and y.f32 verified
//! ([`DenseDataFile`]), then [`SplitState::Written`].
//!
//! # The dense files
//!
//! [`dense::write_split`] writes a split's `x.f32`, `y.f32` and `order.u32` from its samples in
//! draw order, verifies each by reading it back through `WriteDenseFile`, and finishes the split.
//! A split that is not `Open` is refused with `SplitAlreadyWritten` before anything is written.
//!
//! # The generator
//!
//! [`Generator`] draws patch seeds from `SplitMix64(seed)`; from each seed's own
//! `xoshiro256**` stream a [`PatchDraw`] (eight normalized parameters, a MIDI note, a gate); the
//! [`Pipeline`] renders it with `patchback-dsp` and extracts `features/1` with
//! `patchback-analysis`. A vector the contract admits is a sample of the split being filled
//! (train, validation, test, each to its exact count); a limited render or a refused extraction is
//! a diagnostic example, and the next seed is drawn. [`SplitCounts::from_samples`] derives the
//! three counts from `--samples`. [`meta`] writes the text files.
//!
//! # Provisional constants
//!
//! DESIGN.md § 7.1 declares no training distribution and no draw bound (§ 17, OQ-19): each such
//! value is a named constant, provisional until the owner sets it: [`MIDI_LOWEST`],
//! [`MIDI_HIGHEST`], [`GATE_SHORTEST_SECONDS`], [`GATE_LONGEST_SECONDS`], [`NOTE_VELOCITY`] and
//! [`MAX_DRAWS_PER_SAMPLE`]. [`SAMPLES_REFERENCE`] and [`HELD_OUT_REFERENCE`] are § 7.1's first
//! dataset, which the split counts scale.

pub mod dense;
mod generate;
pub mod meta;
mod model;

pub use generate::{
    BATCH_SIZE, GATE_LONGEST_SECONDS, GATE_SHORTEST_SECONDS, GenerateError, Generated, Generator,
    HELD_OUT_REFERENCE, MAX_DRAWS_PER_SAMPLE, MAX_SAMPLES, MIDI_HIGHEST, MIDI_LOWEST,
    NOTE_VELOCITY, PatchDraw, Pipeline, Rendering, SAMPLES_REFERENCE, SplitCounts,
};
pub use model::{
    Dataset, DatasetError, DatasetId, DatasetSplit, DatasetSplitId, DatasetStore, DenseDataFile,
    DenseDataFileId, DenseFileKind, DiagnosticCase, DiagnosticCause, DiagnosticExample,
    DiagnosticExampleId, DiagnosticKind, DiagnosticSet, DiagnosticSetId, GenerateDataset,
    PrngAlgorithm, Sample, SampleId, SampledPatch, SampledPatchId, SplitRole, SplitState,
    WriteDenseFile, is_batch_multiple,
};
