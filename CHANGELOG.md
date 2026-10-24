# Changelog

All notable changes to Patchback are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/beyond10x/patchback/compare/0.5.0...HEAD
[0.5.0]: https://github.com/beyond10x/patchback/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/beyond10x/patchback/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/beyond10x/patchback/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/beyond10x/patchback/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/beyond10x/patchback/releases/tag/0.1.0
