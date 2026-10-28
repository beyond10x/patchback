# Changelog

All notable changes to Patchback are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `spec/ess-inputs.yaml` requires ESS 0.75.0, and CI installs that release; the studio's generated
  wire types in `studio/src/generated/` move to it with no type change.

## [0.11.0] - 2026-10-25

### Added

- `patchback benchmark --kind ml-init --weights <path> --normalization <path>` (story
  ml-init-benchmark): the `RecordMlInitResult` of `spec/domains/fit.yaml`, DESIGN.md § 13 B's
  "default vs ML-init vs ML-init-plus-search". `--kind engine-baseline|ml-init` defaults to
  `engine-baseline`, whose files and stdout are the 0.10.0 ones byte for byte. `--weights` and
  `--normalization` each require the other and are usage errors (exit 2) with the engine
  baseline; `--kind ml-init` without them is refused `patchback.fit.WeightArtifactRequired`
  (exit 1). The two files are read once before anything else and refused as `patchback fit`
  refuses them (`patchback.model.UnknownWeightArtifact`, `patchback.model.CorruptWeightArtifact`,
  `patchback.analysis.NormalizationStatsNotFound`, `patchback.fit.FeatureContractMismatch`); the
  benchmark's `weight_artifact_id` is `sha256:<artifact checksum>`. Each clip runs four arms:
  the default patch, the fit from it, the regressor-seeded fit (`patchback_fit::fit_request`,
  same bounds and fit seed, so only restart 0 differs) and the raw prediction scored once
  (`patchback_fit::predicted_arm`). `benchmark.txt` (format version still 1) records
  `kind ml-init`, `weights sha256`, `modelspec <version> <commit>`, `normalization sha256`,
  `feature_contract features/1`, the four median losses (`raw_regressor_median_loss`,
  `regressor_search_median_loss` beside the default two), both median evaluations,
  `clamped_predictions <n> of <clips>` and no reduction and no verdict; `clips.txt` adds
  `raw_loss seeded_best_loss seeded_evaluations seeded_restarts seeded_stop_reason clamped`;
  each example clip adds `predicted.wav` and `seeded.wav`. A clip whose analysis, seeded fit or
  predicted render is refused ends the benchmark as `clip <k>: …`.
- `patchback_fit::Benchmark::record_ml_init_result` and `patchback_fit::MlInitResult` (story
  ml-init-benchmark): `RecordMlInitResult`, refusing `BenchmarkKindMismatch` before
  `BenchmarkStateConflict`, setting the four median losses, both median evaluations, the wall
  time and the listening examples, recording no reduction and no verdict.
- `patchback_fit::predicted_arm` and `patchback_fit::PredictedArmError` (story
  ml-init-benchmark): the default arm's scoring at any normalized point; at the default point it
  is `patchback_fit::default_arm` bit for bit.
- `website/docs/progress.mdx`, the documentation site's Progress page (story
  docs-progress-and-demos): every milestone of DESIGN.md § 13 with its stories, each story's
  status and the release that delivered it, the counts per status and the latest release with
  its date. `patchback-docs generate` writes it from the planning store's files and
  `CHANGELOG.md` read as text; a story's release is the newest CHANGELOG release whose section,
  its lines joined, names `(story <slug>)`, `(stories … <slug> …)` or `story:<slug>`, else the
  first release dated on or after the day of its `implemented` transition, marked "by date";
  dates compare as numbers. Only `## [Unreleased]` is undated: any other heading whose date is
  missing or does not read as `YYYY-MM-DD` (a four-digit year; month and day of one or two
  digits) is refused, naming its version.
  Stories of no epic are listed under "Outside the milestones". The page names titles and
  milestones, never a planning id, and `generate --check` fails when a story's status or release
  changed without regeneration.
- `website/docs/listen.mdx`, the Listen page (story docs-progress-and-demos): four demo voices
  (pluck, pad, bass, lead; `demos/<voice>/voice.patch` and `note.events`, one note each), each
  with four takes to play in the browser (the recording, the default-start fit, the
  regressor-seeded fit and the clamped prediction with its clamp count), the voice's parameters
  beside both fits' recovered parameters and the prediction's, and both fits' start and best
  loss and evaluations.
- `patchback-docs demos fit` (story docs-progress-and-demos), run in release mode by
  `task docs-demos`: renders each demo voice, analyses it once and fits it from the default patch
  (`patchback_fit::fit`) and from the demo model's prediction (`patchback_fit::fit_request`),
  both with `FitConfig::default()` and a 600,000 ms wall-time bound, and writes
  `website/data/demos.json` (`patchback-demos/1`, under 64 KiB, no evaluation logs) with the
  SHA-256 of every input and of every take's 16-bit samples (`pcm16Sha256`, also listed on the
  Listen page). `patchback-docs generate` and `demos render` refuse a `demos.json` made from
  other inputs, naming the changed file, and a take that no longer renders to its recorded
  samples, naming the voice and take. A relative `--out` of `demos fit` and `demos render`
  resolves against `--root`.
- `patchback-docs demos render --out <dir>` (story docs-progress-and-demos): the four takes of
  every voice as 16-bit PCM WAV, 48 kHz, mono, under 2 s each, byte-identical on every run, into
  the git-ignored `website/demo-audio/`. `task docs-generate` and the "Documentation validation"
  workflow run it before the site build, and a missing take fails the build.
- `patchback-docs demos train --modelspec <binary> --dataset <dir> --out <dir>` (story
  docs-progress-and-demos): retrains the demo model as `models/demo/PROVENANCE` records it
  (dataset seed 42, splits 50,016 / 4,992 / 4,992, 3,126 steps at batch 16) with a ModelSpec
  binary the caller supplies, and says whether the artifact's checksum equals the committed one.
- `models/demo/regressor.weights` (`weights/1`, checksum a17d8104…), its
  `normalization.stats` and `PROVENANCE` (story docs-progress-and-demos): the demo model the
  Listen page's seeded fits and predictions use. Whether a retrain reproduces the checksum is
  `not-verified`.

### Fixed

- Docs, from the site review of 2026-10-25: the `patchback fit` examples on the overview, Getting
  started and Fit a recording pages carry the required `--notes`; the fit writes six files, with
  `trace.events`, and `report.txt` holds no confidence record; analysis measures pitch, and onset
  and gate estimation is Milestone C; the regressor seeds the start only with `--weights`; Getting
  started names `clap` beside the standard library.

## [0.10.0] - 2026-10-25

### Added

- `patchback fit --weights <path> --normalization <path>` (story regressor-seeded-fit): the
  `StartFit` of `spec/domains/fit.yaml` seeded by the regressor. Each flag requires the other
  (one alone is a usage error, exit 2); neither given is the 0.9.0 fit byte for byte. The CLI
  loads the `weights/1` artifact with `patchback_model::Predictor::load` (a missing file is
  `patchback.model.UnknownWeightArtifact`, one the reader refuses
  `patchback.model.CorruptWeightArtifact`) and reads the `normalization.stats` file with
  `patchback_analysis::read_stats` (a missing, unreadable, non-UTF-8 or malformed file, or one
  over 1 MiB, is `patchback.analysis.NormalizationStatsNotFound`), then refuses an artifact or
  statistics of another feature contract than `features/1` as
  `patchback.fit.FeatureContractMismatch`, before the reference's silence and pitch. The run's
  first restart starts from the prediction for the reference's first note, its features
  computed as the dataset computes a training example's (`analyze_and_extract` over the
  reference from the note's start to the end of its release window; a first note reaching the
  recording's end, or whose clip is silent or pitch-ambiguous, is refused
  `patchback.fit.NoValidCandidate`), normalized and checked finite as `f64` and `f32`; the
  other restarts start where they did. stdout and `run.meta` say
  `init regressor-prediction`; `run.meta` adds `weights sha256 <payload sha256>`,
  `weights_modelspec <version> <commit>` and `normalization sha256 <sha256 of the file's bytes>`
  after `init` (format version still 1); stdout and `report.txt` end with
  `warning clamped <n> of 8 predictions` when the prediction had outputs outside [0, 1].
- `patchback_fit::fit_request` and `patchback_fit::FitRequest` (story regressor-seeded-fit):
  the fit's entry with an optional `patchback_analysis::Analysis` and optional
  `patchback_fit::SeedInputs` (a `Predictor`, a `NormalizationTable` and the statistics file's
  SHA-256); a given analysis must hold the reference's frame count and sample rate, else
  `FitError::PredictionFailed`; `patchback_fit::fit` is `fit_request` with neither, unchanged.
  `patchback_fit::FitOutcome` gains `init` (`patchback_fit::InitSource`: `DefaultPatch` or
  `RegressorPrediction`) and `seeded` (`Option<patchback_fit::SeededStart>`: the raw features,
  the `Prediction`, the artifact's checksum and ModelSpec version and commit, the statistics'
  SHA-256). `patchback_fit::check_seed_inputs` and `patchback_fit::reference_refusal` are the
  feature-contract check and the reference-analysis refusal mapping, public for the CLI.
  `patchback_fit::INIT_REGRESSOR_PREDICTION` is the new `init` record.
- `patchback_fit::FitError::AnalysisRequired` and `FitError::FeatureContractMismatch`
  (`patchback.fit.AnalysisRequired`, `patchback.fit.FeatureContractMismatch`), and
  `FitError::PredictionFailed`, named `patchback.fit.NoValidCandidate` like `RendererFault`: a
  seeded start that could not be predicted (a feature extraction refusal, a normalized feature
  that is not finite, or a `PredictError` such as `NonFinitePrediction`), the predictor's
  message in its text.

## [0.9.0] - 2026-10-25

### Added

- `patchback_model::Predictor` (story native-predictor): the `Predict` of
  `spec/domains/model.yaml`, a library call with no `patchback` command. `Predictor::load` reads
  a `weights/1` file through `WeightArtifact::read` and refuses a missing file as
  `UnknownWeightArtifact` and a file the reader refuses as `CorruptWeightArtifact`;
  `Predictor::from_artifact` takes an artifact already read. `Predictor::predict` takes one
  normalized `features/1` vector (`&[f64]`, the output of `NormalizationStats::apply`; the
  predictor normalizes nothing) and the caller's feature-contract version, and refuses, in this
  order, a vector of another length than 32 (`WrongFeatureCount`), a contract other than the
  artifact's (`FeatureContractMismatch`) and a value that is not finite as `f64` or as `f32`
  (`NonFiniteFeature`). It then runs `relu(x · w1) · w2` in `f32`, no bias, the weights
  row-major as exported, and returns a `patchback_model::Prediction` (`raw`, `values` clamped to
  [0, 1], and `clamped`, the number of outputs that were outside it, `±inf` included). A NaN
  output is refused as `PredictError::NonFinitePrediction`. `Predictor::clamped_predictions`
  counts the predictions with an output clamped, in memory; nothing is written or printed. On a
  4-step run of the pinned ModelSpec 0.5.0, the predictor's mean squared error over the 16
  evaluation rows is ModelSpec's `eval loss` to a relative 7e-8.

## [0.8.0] - 2026-10-25

### Added

- `patchback_model::WeightArtifact::export` (story weight-export): the `ExportWeights` of
  `spec/domains/model.yaml`, a library call with no `patchback` command. From a `Completed`
  `TrainingRun` it reads the whole stdout of the pinned ModelSpec under one grammar (`init`,
  `step`, `eval` and exactly two `param` lines; any other line, an empty one included, is
  refused), then checks, in this order, the names `@PatchRegressor.w1` and `.w2`, the shapes
  `[32, 64]` and `[64, 8]`, the counts 2,048 and 512 and that every value is finite, and writes
  a new `weights/1` file: a seven-line header (`model PatchRegressor`, the run's ModelSpec version
  and commit, the feature contract the caller gives, the shapes, the SHA-256 of the payload) and
  10,240 bytes of little-endian `f32`, w1 then w2 in the order printed. It refuses with
  `ExportError` (`TrainingNotCompleted`, `UnrecognizedTrainingOutput`, `UnrecognizedParameter`,
  `WrongWeightShape`, `WrongElementCount`, `NonFiniteWeight`); a refusal writes nothing, an
  existing file is never overwritten, and a failed write removes the file it created.
- `patchback_model::WeightArtifact::read` (story weight-export): reads a `weights/1` file back,
  its header within `MAX_HEADER_BYTES` (1,024, provisional), checks every header line in order,
  the payload's length, its SHA-256 and that every value is finite, and refuses anything else as
  `ReadError::Corrupt` (`CorruptWeightArtifact`). With `FORMAT_VERSION`, `MODEL_NAME`,
  `W1_NAME`, `W2_NAME`, `W1_SHAPE`, `W2_SHAPE`, `W1_ELEMENTS`, `W2_ELEMENTS`, `PAYLOAD_BYTES` and
  `RECOMMENDED_FILE_NAME` (`regressor.weights`, provisional).

## [0.7.0] - 2026-10-25

### Added

- `patchback benchmark --out <dir> --examples <n>` (story engine-baseline): the engine baseline
  of DESIGN.md § 12.2. Over a holdout of `--examples` reference clips synthesized from `--seed`
  (one `SplitMix64` stream of `--seed ^ 0x686f6c646f757431`, apart from `generate-dataset`'s,
  gives each clip a patch seed, drawn under the dataset generator's diagnostic rule, then a fit
  seed; Da8-2, Da8-46), it scores the default patch and fits each clip from it, and records the
  lower medians of both losses, the reduction `(default − search) / default × 100`, the verdict
  (`met` at 20 % or more, else `not-met`, both exit 0), the median render evaluations, the wall
  time and three listening examples. `--seed`, `--topology` and `fit`'s bounds and
  `--block-size` apply to every clip. It writes `clips.txt` (`patchback-benchmark-clips 1`),
  `examples/<k>/reference.wav`, `default.wav` and `fit.wav`, and `benchmark.txt`
  (`patchback-benchmark 1`) last, each created new; any of the three already in `--out` is
  refused before any work, and a failed write removes everything the run wrote. A clip whose fit
  is refused or fails, a holdout not drawn within four draws a clip, or a default median of 0
  ends the run with exit 1 and nothing written. The same flags write the same files,
  `benchmark.txt`'s `wall_time_ms` aside.
- `patchback_fit::Benchmark` (story engine-baseline): the `Benchmark` of `spec/domains/fit.yaml`
  as a value. `start` (`StartBenchmark`) refuses `InvalidHoldoutSize`, `WeightArtifactRequired`
  and `TopologyNotFound`, in that order; `record_baseline_result` (`RecordBaselineResult`)
  refuses `BenchmarkKindMismatch`, then `BenchmarkStateConflict`, and completes the benchmark
  with `TargetVerdict::Met` at `TARGET_REDUCTION_PERCENT` (20) or more, else
  `TargetVerdict::NotMet`. With `BenchmarkKind`, `BenchmarkState`, `BaselineResult` and
  `summary`, the `Benchmarks` view's `BenchmarkRow`.
- `patchback_fit::median` and `median_count` (the lower median, the element at `(n − 1) / 2` of
  the sorted values), `reduction_percent`, `default_arm` (the default patch's render cut or
  padded to a reference, and its comparison) with `DefaultArm` and `DefaultArmError`, and
  `LISTENING_EXAMPLE_COUNT` (3, provisional) (story engine-baseline).

- `patchback_analysis::compute_normalization_stats` (story normalization-stats): the mean and
  population standard deviation of each `features/1` value over a split's rows, computed in
  `f64` in two passes, as `NormalizationStats`. The split is a `SplitInput` (its role and rows),
  refused in the model's order: `DatasetSplitNotFound`, `FeatureContractNotFound`,
  `StatsFromNonTrainingSplit` (validation and test alike, OQ-18), `FeatureContractNotPublished`,
  `FeatureVersionMismatch`, and `NonFiniteFeature` for a row the contract refuses or a statistic
  that is not finite. A feature whose values are all bit-equal to the first row's has that value
  as its mean and standard deviation 0 (Da8-44); a split of no rows gives 0 and 0 (provisional).
- `NormalizationStats::apply` and `NormalizationTable::normalize` (story normalization-stats):
  `(x − mean) / std` per value in `f64`, the same at training and inference, with divisor 1
  where the training standard deviation is 0 (provisional). `apply` refuses features of another
  contract version (`NormalizationContractMismatch`) and a non-finite input or result
  (`NonFiniteFeature`); `normalize` is the unchecked arithmetic. Refusals are
  `NormalizationError`, each naming an error of `spec/domains/analysis.yaml`.
- `normalization.stats` (`patchback-normalization-stats 1`, `patchback_analysis::write_stats`
  and `read_stats`, story normalization-stats): `feature_contract`, `split train`, `rows`,
  `std population`, then one `feature <i> <mean> <std>` line per feature in index order, reals in
  Rust's shortest round-trip form and read back bit for bit. The reader refuses an unknown
  version, a malformed record or number, a negative standard deviation, a repeated, missing or
  out-of-order feature, a file that ends early and a line over `STATS_MAX_LINE_BYTES` (256,
  provisional), naming the line (`StatsFileError`, `StatsReason`).
- `patchback_dataset::compute_split_stats` and `training_stats` (story normalization-stats):
  `compute_normalization_stats` over a split of a `DatasetStore`, and over a generation's
  training split under its dataset's contract.
- `patchback_model::TrainingRun` (story training-run): one `modelspec train` run as a value whose
  methods are the domain's commands in order. `plan` refuses `Backend::Vulkan`
  (`BackendNotShipped`) before anything is spawned; `bind` takes a `ModelSource` and refuses a
  run past `Planned`, an unchecked or rejected source (`ModelNotChecked`) and a source checked
  under another pin (`PinMismatch`); `start` reads the `--data` and `--eval` splits from a
  `DatasetStore`, refuses an unknown or unwritten split and an eval count that is not a multiple
  of the batch size, then runs `modelspec train <source> --backend cpu --data <dir> [--eval <dir>]
  --steps <n>` under `DEFAULT_TRAINING_TIMEOUT` (600 s, provisional); `finish` decides the state
  by the exit status alone, `Completed` on 0 whatever the output and `Failed` with a
  `TrainingFailure` otherwise (Da8-40). Refusals are `TrainingError`.
- `patchback_model::LossCurve` (story training-run): the `step <k> loss <v>` lines and the
  `eval loss=<v>` value of `modelspec train`'s stdout, step numbers increasing. A completed run
  whose stdout does not parse says why in `TrainingRun::output_error` (`OutputError::Malformed`
  naming the line, `NoStepLine`, or `NoEvalLine` when `--eval` was given), and keeps the raw
  stdout and stderr.
- The pinned ModelSpec 0.5.0 trains the design model on generated data (story training-run): on
  a 64-row training split with `--steps 4` the loss falls from 1.0717294 to 0.8882302, with an
  evaluation loss of 0.9350907802581787 on 16 validation rows. `TrainingRun::start` passes the
  split directories as written and normalizes nothing (Da8-32).

### Changed

- `patchback generate-dataset` (story normalization-stats, Da8-30) writes
  `<out>/normalization.stats`, the training split's statistics under `features/1`, before any
  split's files, and every split's `x.f32`, validation and test included, now holds the features
  z-scored under those statistics, `((x − mean) / std) as f32` computed in `f64` with divisor 1
  where the standard deviation is 0. `y.f32`, `order.u32` and `dataset.meta` are unchanged. An
  existing `normalization.stats` is refused before the generation like every other file, and a
  failed write removes everything the run created.
- `patchback_dataset::dense::write_split` takes the training statistics (`&NormalizationStats`)
  and writes `x.f32` as their z-scores; no writer of raw features is left (Da8-30). A non-finite
  value before or after the normalization is still refused by the read-back as `NonFiniteValue`.

## [0.6.0] - 2026-10-25

### Added

- `patchback_fit::write_results` (story fit-run-artifacts): writes the six files of a completed
  fit run (DESIGN.md § 11) into a directory: `patch.patch` (the best patch, canonical),
  `notes.events` (the note sequence, canonical), `reconstructed.wav` (the clip the best patch was
  scored on, float32, so `patchback compare` of the reference and it gives the best loss again),
  `report.txt`, `run.meta` and `trace.events`, each created new and never overwritten, in the
  order of `RESULT_FILES`. Every file's bytes are made before the first is written, and a
  creation or write that fails removes every file and directory the call created
  (`ResultsError::Io`). A failed run is refused `FitError::FailedRunHasNoPatch` and a running one
  `FitError::FitStillRunning`, in the order of `WriteFitResults`, before anything is touched
  (`admit_results`). `FitResults` names the directory, the retained candidate, the recovered
  patch, the WAV's path, the warnings and the files written; `RunMeta` is what the caller fills
  (seed, block size, ModelSpec version, SHA-256 of the input files).
- `report.txt` (`patchback-fit-report 1`, `patchback_fit::report_text`): `patchback`,
  `initial_loss`, `best_loss`, the four components, `evaluations`, `restarts`, `stop_reason`,
  `wall_time_ms`, then one `warning` line per warning
  (`warning limiter engaged on <n> of <m> rendered samples` when the limiter changed `<n>` of the
  `<m>` samples of the best patch's whole render, before it is cut or padded to the reference's
  length for `reconstructed.wav`; `FitResults::warnings` carries the same text). No `confidence`
  record (Da7-4).
- `run.meta` (`patchback-fit-run-meta 1`, `patchback_fit::run_meta_text`): `patchback`, `init`,
  `seed`, `topology`, `feature_contract`, `prng`, `modelspec`, `reference sha256 <hex>`,
  `notes sha256 <hex>`, the four bounds and `block_size`.
- `trace.events` (`patchback-fit-trace 1`, `patchback_fit::trace`): the per-candidate log of
  DESIGN.md § 9.1, written from the run's log after it completed: a `topology` line with the
  parameter paths in index order, one `evaluation` line per evaluation (sequence, evaluation
  count, restart, outcome, topology, the four components and total or `-` for a rejected
  candidate, the best loss after it or `-`, the normalized values) and a closing
  `completed <stop_reason>` line. `write_trace` writes it and `parse_trace` reads it back bit for
  bit, refusing an unknown version, a malformed number, a line past `TRACE_MAX_LINE_BYTES`
  (4 096, provisional) and a text without its closing line (`TraceError`). Lines no run writes
  are refused by their own `TraceReason`: an evaluation count other than the sequence number
  (`EvaluationCountNotSequence`), a best loss above the previous line's (`BestLossRose`), a
  best loss of `-` after a numeric one (`BestLossLost`), a best loss of `-` on a
  `first-retained`, `improved` or `not-improved` line (`RetainedWithoutBestLoss`) and a numeric
  best loss on a `rejected` line before the first `first-retained` line
  (`BestLossBeforeRetained`).
- `patchback-dataset` (story dense-data-files): the model's `WriteDenseFile` and `FinishSplit`.
  `DatasetStore::write_dense_file(WriteDenseFile, &[u8])` records a `DenseDataFile` (kind `X`,
  `Y` or `Order`, `DenseFileKind`) only once it has read the file's bytes. It refuses, in the
  model's order, a column count other than 32, 8 or 1 (`WrongColumnCount`), an unknown split
  (`UnknownSplit`), rows other than the split's sample count (`SampleCountMismatch`), a byte count
  that is not rows × columns × 4 or bytes that are not that many (`ByteCountMismatch`, which
  carries the command's `byte_count`, `rows` and `columns` in both cases), and a NaN
  or infinite `f32` in `x.f32` or `y.f32` (`NonFiniteValue`). `DatasetStore::finish_split` moves
  a `DatasetSplit` from `SplitState::Open` to `SplitState::Written`. It refuses an unknown split,
  a split without a verified x.f32 and y.f32 (`MissingDenseFile`), and a split already written
  (`SplitAlreadyWritten`). `DatasetStore::dense_files_of` is the `DenseFiles` view.
- `patchback_dataset::dense::write_split` (story dense-data-files) writes a split's `x.f32`,
  `y.f32` and `order.u32` from its samples in draw order. The files are row-major little-endian,
  and `f64` values become `f32` by Rust's `as`. Each file is read back and recorded through
  `write_dense_file`, then the split is finished. `order.u32` holds the row indices 0 to N − 1,
  and the writer checks that each entry is below N and appears once. A split that is not `Open`
  is refused with `SplitAlreadyWritten` before anything is written or recorded; `write_dense_file`
  itself still admits a file of a `Written` split. A file that fails
  verification is removed and the refusal returned (`DenseError`). `dense::create_new` writes a
  file that must not exist.
- `patchback generate-dataset` (story dense-data-files) writes `x.f32`, `y.f32` and `order.u32`
  into `train/`, `val/` and `test/`. Each split's files follow its `samples.meta`, and
  `dataset.meta` comes last. A 48-sample generation writes 2,048, 512 and 64 bytes a split,
  byte-identical for the same seed. An existing dense file is never overwritten, and a failed
  write or verification removes everything the run wrote.

### Changed

- `patchback fit` (story fit-run-artifacts) writes all six files of the run instead of
  `patch.patch` alone. Any of the six existing in `--out` (a symbolic link counting, dangling or
  not) is refused before the search, naming the first in the order `patch.patch`,
  `notes.events`, `reconstructed.wav`, `report.txt`, `run.meta`, `trace.events`; a write that
  fails after the search removes everything the run wrote. `--input` and `--notes` are each read
  once into memory, so `run.meta`'s digests are of the bytes the run used; an `--input` larger
  than `MAX_INPUT_BYTES` (247 177 216 bytes, provisional) is refused `ClipNotFound`. The stdout
  report and the flags are unchanged.
- `patchback_fit::FitError` gains `FailedRunHasNoPatch` and `FitStillRunning`
  (`spec/domains/fit.yaml` `WriteFitResults`).
- `patchback_dataset::MAX_SAMPLES` is `u32::MAX` (story dense-data-files, Da7-10), down from
  `i64::MAX`: a split admits at most `u32::MAX` rows, so that its row count is a `u32`.
  `patchback generate-dataset --samples`, which bounds all splits together, admits 0 to
  4294967295, and a larger value is a usage error (exit 2). Memory still grows with `--samples`.
- `DatasetSplit` gains `state: SplitState` (story dense-data-files).

## [0.5.0] - 2026-10-25

### Added

- `patchback_core::prng` (wave a6 scaffold): `SplitMix64` and `Xoshiro256StarStar`
  (`xoshiro256**`, the generator DESIGN.md § 7.1 names), `next_u64`, `next_f64` in `[0, 1)`,
  `from_state` and `state`, held by the reference vectors of both generators.
- `patchback_fit::fit` (story bounded-search-fit): the bounded derivative-free fit of DESIGN.md
  § 9.2 within `subtractive-mono/1`. Restart 0 starts at the default patch (`default_point`,
  every parameter at its contract default); restart `r >= 1` at eight draws of
  `Xoshiro256StarStar` seeded with the `r`-th output of `SplitMix64::new(seed)` (`restart_point`),
  so nearby seeds and restarts never share a start. A restart begins only while no bound is
  reached, so `FitOutcome::restarts` counts restarts that rendered. Each restart runs coordinate
  search over the normalized parameters in index order, raising then lowering one parameter by
  the step (`INITIAL_STEP` 0.25, halving after a sweep without improvement, the restart ending
  below `MIN_STEP` 1/32 or after a sweep that improves by less than the threshold); each
  candidate is the patch `patch_at` builds, rendered, cut or padded to the reference's length
  and scored with `compare`. Only a strictly lower finite loss is retained. The run stops at the
  first of `RenderBudgetExhausted`, `TimeBudgetExhausted` (no render starts once the wall time
  since `fit` was entered has reached `max_wall_time_ms`, except the run's first candidate,
  which is always evaluated, so a time bound stops a run instead of failing it; a render in
  progress is not interrupted, so the recorded `wall_time_ms`, the whole run's, may exceed the
  bound by the reference analysis plus one evaluation), `ImprovementBelowThreshold` (a restart after
  the first lowered the best by less than `improvement_threshold`) and `RestartsExhausted` (all
  `max_restarts` restarts ran; with one restart, every natural end). `FitOutcome` carries the
  completed run, the best patch, its normalized values and scored render, and the
  per-evaluation log (`Evaluation`: sequence, restart, outcome, normalized values, components
  and total, best after, `Rejection`). Before the search, a non-positive bound is refused as
  `FitError::InvalidSearchBound`, an unknown topology as `TopologyNotFound`, a silent reference
  as `ReferenceSilent` and an unpitched one as `PitchAmbiguous`; a run without a valid candidate
  fails with `NoValidCandidate`. Same seed and inputs give the same log and patch.
- `patchback_fit::FitRun` (story bounded-search-fit): the fit run of `spec/domains/fit.yaml`
  with its commands as methods: `start` (refuses a non-positive bound, `FitBounds::check`),
  `evaluate` (`first-retained`, `improved` for a strictly lower loss, `not-improved`; a
  non-finite or negative loss is counted as rejected), `reject`, `complete` (refused
  `NoValidCandidate` with nothing retained) and `fail`; a third evaluation of a budget of two is
  refused `RenderBudgetExhausted` and not counted, an ended run refuses everything with
  `FitNotRunning`. `FitError` names the errors of `spec/domains/fit.yaml`
  (`FitError::DOMAIN_ERRORS`, `domain_error`, `examples`). `DEFAULT_MAX_RENDERS` (2 000),
  `DEFAULT_MAX_WALL_TIME_MS` (60 000), `DEFAULT_MAX_RESTARTS` (4),
  `DEFAULT_IMPROVEMENT_THRESHOLD` (0.0001) and `MIN_STEP` are provisional.
- `patchback fit --input <wav> --notes <events> --out <dir> [--max-renders 2000]
  [--max-wall-time-ms 60000] [--max-restarts 4] [--improvement-threshold 0.0001] [--seed 0]
  [--block-size 256]` (story bounded-search-fit): fits `subtractive-mono/1` to the reference
  with its known notes, writes `<out>/patch.patch` (canonical text; the directory is created
  when absent) and prints `out`, `init`, `evaluations`, `restarts`, `initial_loss`, `best_loss`,
  the four loss components, `stop_reason`, `wall_time_ms` and `limited` as `key value` lines.
  Refusals print one line naming `InvalidSearchBound`, `ClipNotFound`, `NoteSequenceNotFound`,
  `ReferenceSilent`, `PitchAmbiguous` or `NoValidCandidate`, exit 1 and write nothing; an
  existing `patch.patch` is never overwritten, and an `--out` that is not a directory (a
  regular file, a symlink that does not resolve to a directory, or a path beneath a file) is
  refused before the search as `cannot write "<path>": is not a directory`. `wall_time_ms` may
  exceed `--max-wall-time-ms` by the reference analysis plus one evaluation.
- `patchback-dataset`: the model of `spec/domains/dataset.yaml` (story dataset-generation).
  `DatasetStore` holds datasets, splits, sampled patches, samples, diagnostic sets and diagnostic
  examples, and its methods are the commands `generate_dataset`, `open_split`, `draw_patch`,
  `add_sample`, `open_diagnostic_set` and `add_diagnostic_example`. A split count that is not a
  positive multiple of the batch size is refused as `SplitCountNotBatchMultiple`, a patch seed
  already drawn into a split of the dataset as `PatchAlreadyDrawn`, and unknown identities as
  `UnknownDataset`, `UnknownSplit`, `UnknownPatch` and `UnknownDiagnosticSet` (`DatasetError`).
  A `Sample` carries `x: [f64; 32]` and `y: [f64; 8]`. A `DiagnosticExample` carries its patch
  seed, its note and the `DiagnosticCause` that put it there.
- `patchback_dataset::Generator` (story dataset-generation) draws patch seeds from `SplitMix64`
  of the dataset seed. From each seed's own `xoshiro256**` stream it draws a `PatchDraw`: eight
  normalized parameters uniform in `[0, 1)`, a MIDI note from `MIDI_LOWEST` 36 to `MIDI_HIGHEST`
  84, and a gate log-uniform from `GATE_SHORTEST_SECONDS` 0.25 to `GATE_LONGEST_SECONDS` 1.5,
  at velocity 1.0 from sample 0. It renders the draw with `patchback-dsp`, extracts and checks
  `features/1`, and fills train, validation and test in that order, each to its exact count. A
  limited render is a `Clipping` diagnostic example. A silent window or input is `Silence`. Any
  other refused or non-finite feature is `Noise`. A diagnostic example never enters a split.
  After `MAX_DRAWS_PER_SAMPLE` (4) draws a sample the generator stops with
  `GenerateError::Exhausted`. `SplitCounts::from_samples` maps `--samples` to the split counts
  (60,000 gives 50,016 / 4,992 / 4,992; 48 gives 16 / 16 / 16), exactly for every count up to
  `MAX_SAMPLES` (`i64::MAX`). `SplitRole::dir` names a split's directory (`train`, `val`,
  `test`, DESIGN.md § 7.2) and `SplitRole::key` its record name (`train`, `validation`,
  `test`). The training distribution and the draw bound are provisional (DESIGN.md OQ-19).
- `patchback generate-dataset --seed <n> --samples <n> --out <dir>` (story dataset-generation)
  writes `dataset.meta` (`patchback-dataset-meta 1`) and one `samples.meta`
  (`patchback-dataset-samples 1`) for each of `train/`, `val/` and `test/`. The output is
  byte-identical for the same seed. It prints `out` and the split and diagnostic counts as
  `key value` lines. `--samples` admits 0 to 9223372036854775807; a larger value is a usage
  error (exit 2). A split count that is not a positive multiple of 16 (`--samples 47`) is
  refused with `patchback.dataset.SplitCountNotBatchMultiple`. An existing `dataset.meta` or
  `samples.meta` is never overwritten. A failed write removes what the run wrote. The dense
  `x.f32`, `y.f32` and `order.u32` files are not written yet (story dense-data-files).

### Changed

- `patchback fit` and `patchback generate-dataset` are implemented: no command of the binary
  exits 3 as not implemented any more.
- `spec/domains/fit.yaml` (approval-record:engine-baseline-holdout): `Benchmark` records its
  holdout as `topology_id`, a seeded set of references rendered within a declared topology, in
  place of `holdout_split_id`, a dataset split; `StartBenchmark` refuses an undeclared topology
  as `TopologyNotFound` for both benchmark kinds, after `invalid-holdout-size` and before
  `weight-artifact-required`, and `BenchmarkStarted`, `WeightArtifactRequired` and the
  `Benchmarks` view carry `topology_id`. Eight `engine-baseline-*.yaml` scenarios pin it; 403
  conformance scenarios pass.
- `spec/scenarios/dataset-generation-count-not-batch-multiple.yaml` refuses 50,008 samples at
  batch size 16; the 50,000 it refused is 16 × 3,125.

## [0.4.0] - 2026-10-24

### Added

- `patchback-dsp`: the DSP of the fixed `subtractive-mono/1` voice (story
  subtractive-voice-render): `PolyBlepSaw` (phase accumulator, polyBLEP, frequency clamped to
  `MAX_OSCILLATOR_HZ`), `Drive` (`tanh(g x) / tanh(g)`), `SvfLowpass` (TPT state-variable
  lowpass, cutoff clamped to `[MIN_CUTOFF_HZ, MAX_CUTOFF_HZ]` below Nyquist, resonance to the
  contract's 0.5 to 12), `Adsr` (linear segments, note-on, retrigger, note-off, zero and
  near-zero times take one sample), `Smoother`, `Limiter` and `OutputStage`, each in `f64` and
  deterministic. `render` walks a compiled `ExecutionPlan` with a `NoteSequence` block by block
  (`DEFAULT_BLOCK_FRAMES` 256; the block size never changes the samples) and returns the samples
  with a `RenderReport` of the frame count, block size, `limited` and the limited sample count;
  a sequence not at 48 kHz, a block size of zero or less, a note that does not end after it
  starts (`RenderError::InvalidNoteTiming`, `NoteSequenceNotFound`, as the sequence loader
  refuses it), a render past `MAX_FRAMES` and a non-finite sample are refused as `RenderError`s
  naming errors of `spec/domains/audio.yaml`. `RENDER_TAIL_FRAMES` (4 800 frames of silence
  after the release, a margin for analysis windows past the note end), `LIMITER_THRESHOLD` (1.0)
  and `SMOOTHING_SECONDS` (0.002) are provisional.
- `patchback render --patch <file> --notes <file> --out <wav> [--block-size <frames>]` (story
  subtractive-voice-render): writes a mono 48 kHz float32 WAV of the last note's end, the
  release and the tail, and prints `out`, `sample_rate_hz`, `block_size_frames`, `frame_count`,
  `limited` and `limited_samples` as `key value` lines (control characters in `out` escaped, as
  in a refusal). Refusals print one line naming `UnsupportedSampleRate`, `InvalidBlockSize`,
  `PatchNotFound`, `NoteSequenceNotFound`, `PatchNotCompiled` (state `Loaded`), `InputTooLong`
  or `NonFiniteRender`, exit 1 and write nothing; an existing `--out` is never overwritten.
- `patchback_audio::WavImport` and `AudioOrigin` (story external-audio-provenance): an import
  request beside the bytes or the file, `WavImport::decode` and `WavImport::read`, records the
  clip's origin, provenance, licence and render id (`AudioClip::origin`, `provenance`, `licence`,
  `render_id`); an `ExternalRecording` without provenance is refused as
  `WavError::ProvenanceMissing`, and one with provenance and without a licence as
  `WavError::LicenceMissing`, on every import, and no clip exists after either. The two guards
  are answered before a byte is read or the file is opened, so a missing, unreadable, truncated
  or corrupt file without provenance is refused for its provenance. The refusals follow the
  order ESS evaluates `ImportWav` in, which `IMPORT_WAV_PRECEDENCE` and `WavError::precedence`
  state: the guards, then an unsupported rate, format or channel count and an empty file, then
  truncated and corrupt files, then the frame bound and non-finite samples. `decode`, `read_wav`
  and the `patchback analyze` and `compare` commands take no request and record no origin.
- `patchback-analysis` feature contract (story feature-contract): `FeatureRegistry` holds
  contracts by version, each defined once as a draft (`define`), amendable in its FFT lengths
  while a draft (`amend`) and frozen by `publish`; `FeatureContractDefinition::features_1` is
  `features/1`, 32 values: eight band energies of `BAND_EDGES_HZ` over attack, body and release,
  window-major, then RMS, peak, spectral centroid, rolloff, spectral flatness, zero-crossing
  rate, attack-envelope slope and normalized MIDI pitch, at FFT lengths 512, 2048 and 8192.
  `extract_features` and `analyze_and_extract` read one `NoteSpan` of an analysed clip into a
  `FeatureVector` of 32 finite values, and `FeatureContract::check` admits a vector only at the
  contract's version and length with every value finite. Refusals are `FeatureError`s naming the
  domain's errors: a draft is `FeatureContractNotPublished`, a silent or too short window or an
  ambiguous or absent pitch is `FeatureNotComputable` with the value and a `NotComputableReason`,
  never an estimate, and a NaN or infinite value is `NonFiniteFeature`. Window bounds
  (`ATTACK_WINDOW_FRAMES`, `RELEASE_OFFSET_FRAMES`, `RELEASE_WINDOW_FRAMES`), the
  `FEATURE_FFT_LENGTH` and the attack-slope constants are provisional.
- `patchback_analysis::zero_crossing_rate` and `attack_envelope_slope` (story
  feature-contract): sign changes per sample, and the RMS-envelope rise to 90 % of its largest
  256-sample block level, in full scale per second.

### Changed

- `patchback render` no longer exits 3 (not implemented); `fit` is the one stub left.
- `patchback_audio::decode` and `read_wav`, and so `patchback analyze` and `patchback compare`
  (story external-audio-provenance): a WAV file with both an unsupported sample rate, sample
  format or channel count or an empty `data` chunk and a truncation or corruption is now refused
  for the rate, format, channel count or empty file, wherever its `fmt ` chunk and `data` chunk
  header can be read. A file whose `fmt ` or `data` header cannot be read is still refused as
  truncated or corrupt. Of a truncation and a corruption the reader finds in one file, the
  truncation is refused, a second `fmt ` or `data` chunk or an oversized `fmt ` included.

## [0.3.0] - 2026-10-23

### Added

- `patchback-core::score`: note events and the line-oriented `notes.events` file (story
  note-events). `parse_events`, `read_events` and `read_events_file` load a `NoteSequence` of
  sample-timestamped `NoteEvent`s, and `write_events` emits the canonical form, bit-exact on read
  back; notes that do not end after they start, overlap, or fall outside MIDI 0 to 127 or velocity
  0 to 1, unknown versions, duplicate keys, malformed numbers, lines outside the grammar and
  lengths past the provisional bounds (`MAX_NOTES` included) are refused with the line, a file
  with several faults by the one `LoadNoteSequence` declares first.
- `patchback compare --reference <wav> --candidate <wav>`, `patchback-fit` and
  `patchback_analysis::cross_correlation` (story audio-comparison): the candidate is aligned by
  up to the provisional 4 800 frames, then the log-spectral, envelope, transient and band-energy
  terms and their weighted sum by the DESIGN.md weights 0.45, 0.25, 0.15 and 0.15 are printed as
  `key value` lines with the lag; clips at different rates and lengths alignment cannot absorb
  are refused with one line and exit 1.

### Fixed

- `patchback patch validate` and `validate` in `patchback-core::patch` (story
  patch-review-fixes): a `subtractive-mono/1` patch that is not the fixed § 5.2 chain is refused
  as `FixedChainMismatch`, naming the missing kinds, the nodes the chain does not use and the
  chain edges no edge carries; a header with an extra field and an unknown version is refused as
  `UnknownFormatVersion`, not `MalformedRecord`; and a version-1 header with an extra field no
  longer hides a higher-ranked fault on a later line, such as `UnsupportedSampleRate`.

## [0.2.0] - 2026-10-23

### Added

- `patchback-core::patch`: the patch types and the line-oriented `.patch` file (story
  patch-file-format). `parse_patch`, `read_patch` and `read_patch_file` load a patch,
  unvalidated, and `write_patch` emits the canonical form, bit-exact on read back; unknown
  versions, duplicate keys or ids, malformed numbers, unsupported rates, lines outside the
  grammar and lengths past the provisional bounds are refused with the line, a file with several
  faults by the one `LoadPatch` declares first.
- `patchback patch validate <file>`, and `validate` and `compile` in `patchback-core::patch`
  (story patch-graph-validation): a loaded patch is checked for identifiers, port rates,
  parameter contracts, ports, cycles, the provisional caps `MAX_GRAPH_NODES`,
  `MAX_GRAPH_EDGES` and `MAX_VOICES`, and one edge per input port (`InputPortTaken`), then
  compiled into a deterministic `ExecutionPlan`; only a `ValidatedPatch` compiles. `load_patch`
  and `load_patch_file` read and validate a file, refusing with the outcome `LoadPatch` declares
  first, unknown topology and a kind outside the catalogue included. A `.patch` names its
  topology by name (`subtractive-mono/1`). The command prints one line and exits 0, or 1 with one
  line naming the error and the line, node or edge.
- `patchback analyze --input <wav> --out <dir>` and `patchback-analysis`: std-only radix-2 FFT,
  Hann-windowed STFT at 512, 2048 and 8192 samples, RMS and peak envelopes, spectral centroid,
  rolloff, flatness, log-band energies, spectral flux and YIN-style f0, written to a versioned
  `analysis.txt`; silence, an ambiguous or out-of-range pitch, a clip too short for the f0 search,
  non-finite samples and sizes past the provisional bounds are refused and nothing is written
  (story:analysis-core).

## [0.1.0] - 2026-10-23

### Added

- `patchback-audio`: std-only WAV import and export. Mono or stereo (downmixed by the per-frame
  mean, and recorded) PCM16 or float32 at 48 kHz is read into a clip and written back
  byte-exact; unsupported headers, truncated or corrupt chunks, non-finite samples, files past
  the provisional ten-minute frame bound and PCM16 export of samples outside [-1, 1] are refused.
- The Cargo workspace of DESIGN.md § 4.1: library crates `patchback-core`, `patchback-dsp`,
  `patchback-audio`, `patchback-analysis`, `patchback-dataset`, `patchback-model` and
  `patchback-fit`, and `patchback-cli` with the `patchback` binary, which recognises `patch
  validate`, `render`, `analyze`, `compare` and `fit` but implements none of them yet. No
  application crate takes a third-party crate other than `clap` and its reviewed closure, and a
  test holds that.
- `DESIGN.md` revision 0.3: the technical design. The product and its CLI are named `patchback`,
  ESS is required for the specification only (ADR-008), the patch studio is a developer tool
  outside the runtime dependency policy (ADR-011), and § 17 lists the open questions.
- `spec/`: the ESS specification of the application contracts. It has nine domains (patch, score,
  audio, analysis, dataset, model, fit, dependency, studio) and the authored conformance scenarios
  that accept each story.
- `.engineering/`: the AEP planning store, with the vision, the product requirements, one epic per
  milestone A–E, the patch-studio epic, draft stories and the decision records.
- `website/` and `crates/patchback-docs`: the documentation site, with an animated landing page
  and a dark-only theme, and the generator of its status and specification reference pages.
- `studio/`: the patch studio prototype (Vue 3, TypeScript, Vite, pnpm), which draws the module
  catalogue, patch graphs, a fit replay and the end result from illustrative fixture data.
- `task dependency-audit`: the dependency gate alone, also run by `task check`. Besides `clap`
  and its reviewed closure it admits at most one approved audio I/O backend, with its own
  reviewed closure and only behind an optional feature, refuses any ModelSpec crate, and names
  each refused package with its reason, one per line.
- `task check`: validates the specification, runs its conformance suite against the interpreted
  reference target, validates the planning store, checks that every story names existing
  acceptance scenarios, and checks the generated documentation pages.
- ModelSpec 0.5.0 is pinned in `models/modelspec.pin`, and `models/patch_regressor.mspec` holds the
  DESIGN.md § 8.1 model; `patchback-model` runs the pinned binary's `modelspec check` on it,
  refuses a binary that reports another version, and keeps a rejected model from training. CI
  installs the pinned binary by checksum and runs that check.
- `patchback-core` parameter contracts: the `subtractive-mono/1` topology with its eight
  contracts of `DESIGN.md` § 5.2 (placeholder defaults), forward and inverse mappings, clamping
  of normalized values with a recorded clamp, and refusal of non-finite values and of invalid
  contracts (including indices other than 0..n-1 and ranges whose width or ratio overflows),
  each refusal naming the topology.

### Changed

- `patchback` parses its command line with clap derive, the one reviewed CLI crate (`DESIGN.md`
  revision 0.4, § 3); help, version and usage errors follow clap (usage errors exit 2), and the
  dependency gate admits `clap` with exactly its reviewed `Cargo.lock` closure.

[Unreleased]: https://github.com/beyond10x/patchback/compare/0.11.0...HEAD
[0.11.0]: https://github.com/beyond10x/patchback/compare/0.10.0...0.11.0
[0.10.0]: https://github.com/beyond10x/patchback/compare/0.9.0...0.10.0
[0.9.0]: https://github.com/beyond10x/patchback/compare/0.8.0...0.9.0
[0.8.0]: https://github.com/beyond10x/patchback/compare/0.7.0...0.8.0
[0.7.0]: https://github.com/beyond10x/patchback/compare/0.6.0...0.7.0
[0.6.0]: https://github.com/beyond10x/patchback/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/beyond10x/patchback/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/beyond10x/patchback/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/beyond10x/patchback/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/beyond10x/patchback/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/beyond10x/patchback/releases/tag/0.1.0
