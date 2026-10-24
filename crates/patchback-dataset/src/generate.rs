//! The seeded generator of DESIGN.md § 7.1: patch seeds from the dataset seed, one patch and one
//! note from each patch seed, rendered by `patchback-dsp`, described by `features/1`, and kept as
//! a sample of its split or as an example of the diagnostic set.

use std::fmt;
use std::path::PathBuf;

use patchback_analysis::{
    FeatureContract, FeatureContractDefinition, FeatureContractId, FeatureError, FeatureRegistry,
    NoteSpan, analyze_and_extract,
};
use patchback_core::parameter::{SUBTRACTIVE_MONO, Topology};
use patchback_core::patch::validate::{FIXED_CHAIN_EDGES, FIXED_CHAIN_KINDS};
use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, NodeDecl, NodeKind, ParameterValue, Patch, Port, PortRate,
    ValidationError, compile, validate,
};
use patchback_core::prng::{NAME, SplitMix64, Xoshiro256StarStar};
use patchback_core::score::{NoteEvent, NoteSequence};
use patchback_dsp::{RenderConfig, RenderError, SAMPLE_RATE_HZ, render};

use crate::model::{
    Dataset, DatasetError, DatasetId, DatasetSplitId, DatasetStore, DiagnosticCase,
    DiagnosticCause, DiagnosticExample, DiagnosticKind, DiagnosticSetId, GenerateDataset,
    PrngAlgorithm, Sample, SplitRole, is_batch_multiple,
};

/// The batch size every split count divides: the initial ModelSpec minibatch of § 7.1.
pub const BATCH_SIZE: u64 = 16;

/// The lowest MIDI note of the training distribution (D-a6-9). Provisional: DESIGN.md § 7.1
/// declares no range (§ 17, OQ-19).
pub const MIDI_LOWEST: u8 = 36;

/// The highest MIDI note of the training distribution, inclusive (D-a6-9). Provisional (OQ-19).
pub const MIDI_HIGHEST: u8 = 84;

/// The shortest gate of the training distribution, in seconds (D-a6-9). Provisional (OQ-19).
pub const GATE_SHORTEST_SECONDS: f64 = 0.25;

/// The longest gate of the training distribution, in seconds; gates are drawn log-uniformly
/// between the two (D-a6-9). Provisional (OQ-19).
pub const GATE_LONGEST_SECONDS: f64 = 1.5;

/// The velocity of every training note (D-a6-9). Provisional (OQ-19).
pub const NOTE_VELOCITY: f32 = 1.0;

/// The draws a generation may make for each sample it was asked for: past
/// `MAX_DRAWS_PER_SAMPLE` × the total sample count it stops ([`GenerateError::Exhausted`]), so a
/// distribution in which (almost) every patch is a diagnostic example cannot run forever.
///
/// Provisional: DESIGN.md names no bound. 4 allows three diagnostic examples a sample; the
/// 48-sample generation of seed 42 drew 95 patches (47 diagnostic examples), about 2 a sample.
pub const MAX_DRAWS_PER_SAMPLE: u64 = 4;

/// The total of § 7.1's first dataset, 60,000 samples, of which [`HELD_OUT_REFERENCE`] are
/// validation and as many test: the ratio [`SplitCounts::from_samples`] keeps (D-a6-10).
pub const SAMPLES_REFERENCE: u64 = 60_000;

/// The validation (and test) count of § 7.1's first dataset, 4,992.
pub const HELD_OUT_REFERENCE: u64 = 4_992;

/// The largest sample count a generation admits, `i64::MAX` (Da6-26): every split count of
/// [`SplitCounts::from_samples`] then fits the model's `Integer` (i64) without saturating.
/// `patchback generate-dataset` refuses a larger `--samples` as a usage error.
pub const MAX_SAMPLES: u64 = i64::MAX as u64;

/// The note's patch in the note sequence: the renderer does not compare it with the plan.
const NOTE_PATCH_ID: &str = "00000000-0000-4000-8000-000000000000";

/// The sample counts of the three splits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitCounts {
    pub train: i64,
    pub validation: i64,
    pub test: i64,
}

impl SplitCounts {
    /// D-a6-10: validation and test are each `max(16, floor(samples · 4992 / 60000 / 16) · 16)`
    /// and train is the rest, `samples − 2 · validation`, which may be zero or negative for fewer
    /// than 32 samples (and is then refused by `OpenSplit`). `--samples 60000` gives 50,016 /
    /// 4,992 / 4,992. Computed in `u128`/`i128` without overflow; for every `samples` up to
    /// [`MAX_SAMPLES`] each count is the rule's exactly. Above it (which `--samples` does not
    /// admit) a train count past `i64::MAX` saturates there, an odd count that is refused.
    pub fn from_samples(samples: u64) -> Self {
        let samples = u128::from(samples);
        let batch = u128::from(BATCH_SIZE);
        let batches =
            samples * u128::from(HELD_OUT_REFERENCE) / u128::from(SAMPLES_REFERENCE) / batch;
        let held_out = (batches * batch).max(batch);
        let train = samples as i128 - 2 * held_out as i128;
        let held_out = i64::try_from(held_out).unwrap_or(i64::MAX);
        Self {
            train: i64::try_from(train).unwrap_or(i64::MAX),
            validation: held_out,
            test: held_out,
        }
    }

    pub fn get(&self, role: SplitRole) -> i64 {
        match role {
            SplitRole::Train => self.train,
            SplitRole::Validation => self.validation,
            SplitRole::Test => self.test,
        }
    }

    /// The first count, in fill order, that is not a positive multiple of [`BATCH_SIZE`]: the
    /// refusal `OpenSplit` gives it.
    pub fn check(&self) -> Result<(), DatasetError> {
        for role in SplitRole::ALL {
            let sample_count = self.get(role);
            if !is_batch_multiple(sample_count, BATCH_SIZE) {
                return Err(DatasetError::SplitCountNotBatchMultiple {
                    role,
                    sample_count,
                    batch_size: BATCH_SIZE,
                });
            }
        }
        Ok(())
    }

    /// The samples of the three splits; a count of zero or below adds none.
    pub fn total(&self) -> u64 {
        SplitRole::ALL
            .iter()
            .map(|&role| u64::try_from(self.get(role)).unwrap_or(0))
            .fold(0, u64::saturating_add)
    }
}

/// One patch and its note, drawn from a patch seed (D-a6-8, D-a6-9): from
/// `Xoshiro256StarStar::new(patch_seed)`, the eight normalized parameters in index order, each
/// uniform in `[0, 1)`; then the MIDI note, uniform from [`MIDI_LOWEST`] to [`MIDI_HIGHEST`];
/// then the gate, log-uniform from [`GATE_SHORTEST_SECONDS`] to [`GATE_LONGEST_SECONDS`] and
/// rounded to samples at 48 kHz. The note starts at sample 0 with velocity [`NOTE_VELOCITY`].
///
/// Uniform normalized values are § 7.1's stratification: the exponential contracts (attack,
/// decay, release, filter cutoff) map them uniformly onto log-time and log-frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatchDraw {
    pub patch_seed: u64,
    /// The normalized parameters in the topology's index order: the sample's `y`.
    pub y: [f64; 8],
    pub midi_note: u8,
    pub gate_samples: u64,
}

impl PatchDraw {
    pub fn from_seed(patch_seed: u64) -> Self {
        let mut prng = Xoshiro256StarStar::new(patch_seed);
        let y = [(); 8].map(|()| prng.next_f64());
        let notes = f64::from(MIDI_HIGHEST - MIDI_LOWEST + 1);
        let midi_note = MIDI_LOWEST + (prng.next_f64() * notes).floor() as u8;
        let ratio = GATE_LONGEST_SECONDS / GATE_SHORTEST_SECONDS;
        let seconds = GATE_SHORTEST_SECONDS * ratio.powf(prng.next_f64());
        let gate_samples = (seconds * f64::from(SAMPLE_RATE_HZ)).round() as u64;
        Self {
            patch_seed,
            y,
            midi_note,
            gate_samples,
        }
    }

    /// The fixed chain of `topology` (§ 5.2) with the physical values of [`Self::y`]: node `n` is
    /// the `n`th kind of [`FIXED_CHAIN_KINDS`], every edge of [`FIXED_CHAIN_EDGES`] at the rates
    /// its ports declare. A path the topology lacks is left out, and validation refuses it.
    pub fn patch(&self, topology: &Topology) -> Patch {
        let id = |kind: NodeKind| {
            FIXED_CHAIN_KINDS
                .iter()
                .position(|k| *k == kind)
                .map_or(0, |i| i as u32 + 1)
        };
        let rate = |ports: &[Port], name: &str| {
            ports
                .iter()
                .find(|p| p.name == name)
                .map_or(PortRate::Audio, |p| p.rate)
        };
        let nodes = FIXED_CHAIN_KINDS
            .iter()
            .map(|&kind| NodeDecl {
                id: id(kind),
                kind,
                parameters: kind
                    .parameter_paths()
                    .iter()
                    .filter_map(|path| {
                        let contract = topology.contract(path)?;
                        Some(ParameterValue {
                            name: (*path).to_owned(),
                            value: contract.to_physical(*self.y.get(contract.index)?),
                        })
                    })
                    .collect(),
            })
            .collect();
        let edges = FIXED_CHAIN_EDGES
            .iter()
            .map(|edge| EdgeDecl {
                from_node: id(edge.from_kind),
                from_port: edge.from_port.to_owned(),
                from_rate: rate(edge.from_kind.output_ports(), edge.from_port),
                to_node: id(edge.to_kind),
                to_port: edge.to_port.to_owned(),
                to_rate: rate(edge.to_kind.input_ports(), edge.to_port),
            })
            .collect();
        Patch {
            format_version: FORMAT_VERSION,
            sample_rate_hz: SAMPLE_RATE_HZ,
            topology_id: topology.name().to_owned(),
            nodes,
            edges,
        }
    }

    /// The one-note program: the note from sample 0 to the gate's end.
    pub fn notes(&self) -> NoteSequence {
        NoteSequence {
            format_version: patchback_core::score::FORMAT_VERSION,
            sample_rate_hz: SAMPLE_RATE_HZ,
            patch_id: NOTE_PATCH_ID.to_owned(),
            note_estimation_id: None,
            notes: vec![NoteEvent {
                start_sample: 0,
                end_sample: self.gate_samples,
                midi_note: self.midi_note,
                velocity: NOTE_VELOCITY,
            }],
        }
    }
}

/// What rendering one draw gave: a feature vector, or a diagnostic example.
#[derive(Clone, Debug, PartialEq)]
pub enum Rendering {
    /// The 32 values of `features/1`, checked by the contract.
    Sample([f64; 32]),
    Diagnostic(DiagnosticCause),
}

/// Why a generation produced no dataset.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerateError {
    /// The model refused: a split count that is not a positive multiple of [`BATCH_SIZE`], or
    /// (never for the generator's own draws) another of its refusals.
    Dataset(DatasetError),
    /// The renderer refused for a reason that is no diagnostic ([`DiagnosticCause::from_render`]).
    Render(RenderError),
    /// The feature contract refused for a reason that is no diagnostic
    /// ([`DiagnosticCause::from_feature`]).
    Feature(FeatureError),
    /// The drawn patch did not validate; the draw builds the fixed chain, so this is a defect.
    Patch(ValidationError),
    /// The draws reached [`Generator::max_draws`] with `samples` of the `wanted` samples made:
    /// every other draw was a diagnostic example. `spec/domains/dataset.yaml` declares no such
    /// error; this one is the generator's own.
    Exhausted {
        draws: u64,
        samples: u64,
        wanted: u64,
    },
}

impl GenerateError {
    /// The error of `spec/` this refusal is; none for [`Self::Exhausted`], which the model does
    /// not declare.
    pub fn domain_error(&self) -> Option<&'static str> {
        match self {
            Self::Dataset(error) => Some(error.domain_error()),
            Self::Render(error) => Some(error.domain_error()),
            Self::Feature(error) => Some(error.domain_error()),
            Self::Patch(error) => Some(error.spec_error()),
            Self::Exhausted { .. } => None,
        }
    }
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dataset(error) => write!(f, "{error}"),
            Self::Render(error) => write!(f, "{error}"),
            Self::Feature(error) => write!(f, "{error}"),
            Self::Patch(error) => write!(f, "{error}"),
            Self::Exhausted {
                draws,
                samples,
                wanted,
            } => write!(
                f,
                "generation exhausted: {draws} patch draws gave {samples} of the {wanted} samples \
                 asked for, every other draw a diagnostic example (at most \
                 {MAX_DRAWS_PER_SAMPLE} draws a sample)"
            ),
        }
    }
}

impl std::error::Error for GenerateError {}

impl From<DatasetError> for GenerateError {
    fn from(error: DatasetError) -> Self {
        Self::Dataset(error)
    }
}

/// The render and the feature extraction of a draw: `patchback-dsp` and `features/1` from a
/// registry of its own, in which the contract is defined and published (D-a6-14).
#[derive(Clone, Debug)]
pub struct Pipeline {
    registry: FeatureRegistry,
    contract_id: FeatureContractId,
    topology: Topology,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    pub fn new() -> Self {
        let mut registry = FeatureRegistry::new();
        let contract_id = match registry.define(FeatureContractDefinition::features_1()) {
            Ok(id) => id,
            Err(error) => unreachable!("an empty registry defines features/1: {error}"),
        };
        if let Err(error) = registry.publish(contract_id) {
            unreachable!("a draft is published: {error}");
        }
        Self {
            registry,
            contract_id,
            topology: Topology::subtractive_mono(),
        }
    }

    pub fn feature_contract_id(&self) -> FeatureContractId {
        self.contract_id
    }

    /// The published `features/1`.
    pub fn contract(&self) -> &FeatureContract {
        match self.registry.get(self.contract_id) {
            Ok(contract) => contract,
            Err(error) => unreachable!("the pipeline holds its contract: {error}"),
        }
    }

    /// Renders `draw` and extracts its features over the note's span, in this order:
    ///
    /// 1. the patch is validated and compiled; a refusal is [`GenerateError::Patch`];
    /// 2. the render; its refusal is a diagnostic example or a fault (`DiagnosticCause::from_render`);
    /// 3. a render the protective limiter changed is a [`DiagnosticKind::Clipping`] example, and
    ///    is not analysed;
    /// 4. `analyze_and_extract` of `features/1` over samples `0..gate_samples`, then
    ///    `FeatureContract::check` of the vector; a refusal of either is a diagnostic example or a
    ///    fault (`DiagnosticCause::from_feature`).
    pub fn run(&self, draw: &PatchDraw) -> Result<Rendering, GenerateError> {
        let validated = validate(&draw.patch(&self.topology)).map_err(GenerateError::Patch)?;
        let plan = compile(&validated);
        let rendered = match render(&plan, &draw.notes(), &RenderConfig::default()) {
            Ok(rendered) => rendered,
            Err(error) => {
                return DiagnosticCause::from_render(error)
                    .map(Rendering::Diagnostic)
                    .ok_or(GenerateError::Render(error));
            }
        };
        if rendered.report.limited {
            return Ok(Rendering::Diagnostic(DiagnosticCause::limited(
                rendered.report.limited_samples,
            )));
        }
        let contract = self.contract();
        let span = NoteSpan {
            start_sample: 0,
            end_sample: usize::try_from(draw.gate_samples).unwrap_or(usize::MAX),
        };
        let checked = analyze_and_extract(contract, &rendered.samples, SAMPLE_RATE_HZ, span)
            .and_then(|vector| {
                contract.check(&vector.version, &vector.values)?;
                <[f64; 32]>::try_from(vector.values.as_slice()).map_err(|_| {
                    FeatureError::FeatureLengthMismatch {
                        value_count: vector.values.len(),
                    }
                })
            });
        match checked {
            Ok(x) => Ok(Rendering::Sample(x)),
            Err(error) => DiagnosticCause::from_feature(error.clone())
                .map(Rendering::Diagnostic)
                .ok_or(GenerateError::Feature(error)),
        }
    }
}

/// One generation: the dataset seed, the split counts, the output directory the dataset records,
/// and the bound on its draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Generator {
    pub seed: u64,
    pub counts: SplitCounts,
    pub out_dir: PathBuf,
    /// [`MAX_DRAWS_PER_SAMPLE`] × the total count, unless set otherwise.
    pub max_draws: u64,
}

impl Generator {
    pub fn new(seed: u64, counts: SplitCounts, out_dir: PathBuf) -> Self {
        Self {
            seed,
            counts,
            out_dir,
            max_draws: counts.total().saturating_mul(MAX_DRAWS_PER_SAMPLE),
        }
    }

    /// The generation through the real [`Pipeline`].
    pub fn run(&self) -> Result<Generated, GenerateError> {
        let pipeline = Pipeline::new();
        self.run_with(pipeline.feature_contract_id(), |draw| pipeline.run(draw))
    }

    /// The generation with `render` in place of the pipeline:
    ///
    /// 1. `GenerateDataset` with the seed, `xoshiro256**`, [`BATCH_SIZE`], the contract and
    ///    `subtractive-mono/1`; `OpenSplit` of train, validation and test with their counts (the
    ///    first count that is not a positive batch multiple is refused before anything is drawn);
    ///    `OpenDiagnosticSet`.
    /// 2. Patch seeds are the outputs of `SplitMix64(seed)` in order. The splits are filled in the
    ///    order train, validation, test, each to its exact count: each seed's [`PatchDraw`] is
    ///    rendered; a [`Rendering::Sample`] is `DrawPatch` into the split being filled and
    ///    `AddSample`, a [`Rendering::Diagnostic`] is `AddDiagnosticExample`, and the next seed is
    ///    drawn.
    /// 3. Before each draw, a generation that has made [`Self::max_draws`] draws is refused
    ///    ([`GenerateError::Exhausted`]); an error of `render` ends the generation.
    pub fn run_with(
        &self,
        feature_contract_id: FeatureContractId,
        mut render: impl FnMut(&PatchDraw) -> Result<Rendering, GenerateError>,
    ) -> Result<Generated, GenerateError> {
        let mut store = DatasetStore::new();
        let dataset_id = store.generate_dataset(GenerateDataset {
            seed: self.seed,
            prng_algorithm: PrngAlgorithm(NAME.to_owned()),
            batch_size: BATCH_SIZE,
            feature_contract_id,
            topology_id: SUBTRACTIVE_MONO.to_owned(),
            out_dir: self.out_dir.clone(),
        });
        let mut splits = Vec::with_capacity(SplitRole::ALL.len());
        for role in SplitRole::ALL {
            let split = store.open_split(dataset_id, role, self.counts.get(role))?;
            splits.push((role, split));
        }
        let diagnostic_set_id = store.open_diagnostic_set(dataset_id)?;

        let wanted = self.counts.total();
        let mut seeds = SplitMix64::new(self.seed);
        let mut draws = 0;
        let mut samples = 0;
        for &(role, split) in &splits {
            let count = self.counts.get(role);
            let mut filled = 0;
            while filled < count {
                if draws >= self.max_draws {
                    return Err(GenerateError::Exhausted {
                        draws,
                        samples,
                        wanted,
                    });
                }
                let draw = PatchDraw::from_seed(seeds.next_u64());
                draws += 1;
                match render(&draw)? {
                    Rendering::Sample(x) => {
                        let patch = store.draw_patch(dataset_id, split, draw.patch_seed)?;
                        store.add_sample(patch, draw.midi_note, draw.gate_samples, x, draw.y)?;
                        filled += 1;
                        samples += 1;
                    }
                    Rendering::Diagnostic(cause) => {
                        store.add_diagnostic_example(
                            diagnostic_set_id,
                            DiagnosticCase {
                                patch_seed: draw.patch_seed,
                                midi_note: draw.midi_note,
                                gate_samples: draw.gate_samples,
                                cause,
                            },
                        )?;
                    }
                }
            }
        }
        let splits = [splits[0], splits[1], splits[2]];
        Ok(Generated {
            store,
            dataset_id,
            splits,
            diagnostic_set_id,
            draws,
        })
    }
}

/// A finished generation: the model's records and where the dataset's are among them.
#[derive(Clone, Debug, PartialEq)]
pub struct Generated {
    pub store: DatasetStore,
    pub dataset_id: DatasetId,
    /// The splits in fill order.
    pub splits: [(SplitRole, DatasetSplitId); 3],
    pub diagnostic_set_id: DiagnosticSetId,
    /// The patch seeds drawn: the samples and the diagnostic examples together.
    pub draws: u64,
}

impl Generated {
    pub fn dataset(&self) -> &Dataset {
        match self.store.dataset(self.dataset_id) {
            Some(dataset) => dataset,
            None => unreachable!("a generation holds its dataset"),
        }
    }

    pub fn split_id(&self, role: SplitRole) -> DatasetSplitId {
        let index = SplitRole::ALL.iter().position(|r| *r == role).unwrap_or(0);
        self.splits[index].1
    }

    /// The samples of the split of `role`, in the order they were drawn.
    pub fn samples(&self, role: SplitRole) -> Vec<&Sample> {
        self.store.samples_in(self.split_id(role))
    }

    /// The seed of the patch `sample` was rendered from.
    pub fn patch_seed_of(&self, sample: &Sample) -> u64 {
        match self.store.patch(sample.sampled_patch_id) {
            Some(patch) => patch.patch_seed,
            None => unreachable!("a sample's patch is held"),
        }
    }

    /// The diagnostic examples, in the order they were drawn.
    pub fn diagnostics(&self) -> Vec<&DiagnosticExample> {
        self.store.diagnostic_examples_in(self.diagnostic_set_id)
    }

    pub fn diagnostic_count(&self, kind: DiagnosticKind) -> usize {
        self.diagnostics()
            .iter()
            .filter(|example| example.kind == kind)
            .count()
    }
}
