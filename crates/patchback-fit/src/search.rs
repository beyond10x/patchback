//! The bounded derivative-free fit of DESIGN.md § 9.2 within one known topology: [`fit`].
//!
//! # Refusals
//!
//! In the order of `StartFit`'s outcomes (`spec/domains/fit.yaml`), before anything is rendered:
//!
//! 1. A bound that is not positive: [`FitError::InvalidSearchBound`] naming it
//!    ([`FitBounds::check`]: renders, then wall time, then restarts).
//! 2. A topology the renderer does not know (`patchback_core::patch::known_topology`):
//!    [`FitError::TopologyNotFound`]. The reference and the note sequence are values here; their
//!    existence is the caller's check (`patchback fit` answers it by path).
//! 3. The reference is analysed (`patchback_analysis::analyze_clip`, the default
//!    configuration): `SilentInput` is [`FitError::ReferenceSilent`]; `PitchAmbiguous`,
//!    `ClipTooShortForPitch` and `PitchOutOfRange` are [`FitError::PitchAmbiguous`] (D-a6-4); any
//!    other refusal (an empty clip, a rate other than 48 kHz, a clip past the analysis duration
//!    bound, a non-finite sample) would refuse every comparison too and is passed on as
//!    [`FitError::Compare`].
//!
//! # The search
//!
//! The wall-time clock starts when [`fit`] is entered, before the refusals, so it covers the
//! reference analysis too. The run starts ([`FitRun::start`]) once the refusals have passed. A
//! point is a vector of normalized values in `[0, 1]`, one per parameter contract, at the
//! contract's index.
//!
//! - **Restarts.** Restart 0 starts at the default point ([`default_point`]: every parameter at
//!   its contract default, the fixed safe default patch of § 9.2 step 1). Restart `r >= 1` starts
//!   at the point whose value at index `i` is the `i`-th draw of
//!   `Xoshiro256StarStar::new(s_r).next_f64()` (`patchback_core::prng`), in index order, where
//!   `s_r` is the `r`-th output of `SplitMix64::new(seed)`, numbered from 1: restart 1 takes the
//!   first `next_u64`, restart 2 the second ([`restart_point`]). Nearby seeds and restarts so
//!   start apart (the earlier `seed ^ r` gave seed 1 restart 2 and seed 2 restart 1 one start).
//!   At most `max_restarts` restarts run. A restart `r >= 1` begins only while no bound is
//!   reached (see [Stopping](#stopping)), and its start is then evaluated without a second
//!   check, so every restart that began rendered at least its start.
//! - **Evaluation.** A point becomes the patch [`patch_at`] builds, validated and compiled
//!   (`patchback_core::patch`); it is rendered with `config.render` against the note sequence
//!   (`patchback_dsp::render`), the render is cut to the reference's length or padded with
//!   silence to it, and that clip is compared with the reference ([`crate::compare`]). The run
//!   counts the result: a valid loss through [`FitRun::evaluate`], a refused render or
//!   comparison through [`FitRun::reject`] (D-a6-6). A patch that validation refuses fails the
//!   run with `RendererFault` (unreachable: [`patch_at`] keeps every value in its contract's
//!   range).
//! - **Coordinate search.** From the restart's start, a sweep visits the parameters in index
//!   order; for each it tries the current point with that value raised by the step, then
//!   lowered by it, each clamped to `[0, 1]` (a value the clamp leaves unchanged is not
//!   rendered). The first candidate whose loss is strictly lower than the current point's
//!   becomes the current point and the sweep moves to the next parameter. The step starts at
//!   [`INITIAL_STEP`]. After a sweep:
//!   - no candidate was better: the step halves, and the restart ends when it falls below
//!     [`MIN_STEP`];
//!   - the sweep lowered the current loss by less than `improvement_threshold`: the restart
//!     ends;
//!   - otherwise the next sweep runs with the same step.
//!
//!   A rejected start counts as no loss, which any valid loss improves on.
//! - **Retention.** The run retains a candidate only when its loss is strictly lower than the
//!   best so far ([`FitRun::evaluate`]), whatever the restart; an improvement below the
//!   threshold is still retained (OQ-14 stays open).
//!
//! # Stopping
//!
//! The bounds are checked before every render but the run's first and before every restart
//! `r >= 1` begins. The run stops at the first of:
//!
//! - **`RenderBudgetExhausted`**: a render is due and the run has counted `max_renders`
//!   evaluations. Then `evaluations == max_renders`.
//! - **`TimeBudgetExhausted`**: a render is due, it is not the run's first candidate, and the
//!   wall time since [`fit`] was entered has reached `max_wall_time_ms`: no render starts once
//!   the bound is reached. The run's first candidate (the default point of restart 0) is always
//!   evaluated, so a time bound stops a run instead of failing it, and the run holds a patch
//!   whenever that candidate was valid. A render in progress is not interrupted. The recorded
//!   `wall_time_ms` is the whole run's, from entry to completion, the reference analysis
//!   included, so it may exceed the bound by at most the reference analysis and one evaluation
//!   (the first, or the one running when the bound passed; Da6-29). The model (`FitRun` in
//!   `spec/domains/fit.yaml`) bounds `evaluations`, not `wall_time_ms`; DESIGN.md § 9.2 calls
//!   the budget an offline upper bound, not a real-time guarantee (Da6-22).
//! - **`ImprovementBelowThreshold`**: the early end of the restart loop: a restart `r >= 1`
//!   ended having lowered the run's best loss by less than `improvement_threshold` (by nothing
//!   at all included); later restarts do not run.
//! - **`RestartsExhausted`**: the loop completed its `max_restarts` restarts without that. With
//!   `max_restarts` 1 a natural end is always `RestartsExhausted`, whatever the last sweep
//!   improved (Da6-23).
//!
//! The run then completes with that reason ([`FitRun::complete`]). A run that retained no valid
//! candidate cannot complete; it fails with `NoValidCandidate` and [`fit`] returns
//! [`FitError::NoValidCandidate`].
//!
//! # Determinism
//!
//! The search reads no clock except for the wall-time bound, and no other state: the same
//! reference, note sequence, topology and configuration give the same log, patch and render on
//! every run and platform the renderer and the comparison are bit-exact on (§ 2 invariant 5),
//! unless the wall-time bound stops the run.

use std::fmt;
use std::time::Instant;

use patchback_analysis::{AnalysisConfig, AnalysisError, analyze_clip};
use patchback_audio::{AudioClip, WavError};
use patchback_core::parameter::{SetParameterError, Topology};
use patchback_core::patch::validate::{FIXED_CHAIN_EDGES, FIXED_CHAIN_KINDS};
use patchback_core::patch::{
    EdgeDecl, FORMAT_VERSION, NodeDecl, NodeKind, ParameterValue, Patch, Port, PortRate,
    SUPPORTED_SAMPLE_RATE_HZ, compile, known_topology, validate,
};
use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};
use patchback_core::score::NoteSequence;
use patchback_dsp::{RenderConfig, RenderError, RenderReport, render};

use crate::run::{EvaluationOutcome, FailureReason, FitBounds, FitRun, StopReason};
use crate::{CompareError, FitError, LossComponents, compare};

/// The step of the first sweep of every restart, in normalized units (D-a6-2).
pub const INITIAL_STEP: f64 = 0.25;

/// A restart ends when its step halves below this: the steps are 0.25, 0.125, 0.0625 and
/// 0.03125. Provisional: DESIGN.md § 9.2 names no smallest step.
pub const MIN_STEP: f64 = 1.0 / 32.0;

/// Everything a fit is configured with besides its inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitConfig {
    pub bounds: FitBounds,
    /// The seed of the restarts' starting points.
    pub seed: u64,
    /// The render configuration of every candidate.
    pub render: RenderConfig,
}

impl Default for FitConfig {
    /// The default bounds, seed 0 and the default render configuration (256-frame blocks).
    fn default() -> Self {
        Self {
            bounds: FitBounds::default(),
            seed: 0,
            render: RenderConfig::default(),
        }
    }
}

/// Why a candidate was counted as rejected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rejection {
    /// The renderer refused the patch with the note sequence.
    Render(RenderError),
    /// The rendered samples did not make a clip.
    Audio(WavError),
    /// The comparison refused the rendered clip.
    Compare(CompareError),
    /// The loss or a component was not finite and non-negative.
    NonFiniteLoss,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::Audio(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::Compare(error) => write!(f, "{}: {error}", error.domain_error()),
            Self::NonFiniteLoss => f.write_str("the loss is not finite and non-negative"),
        }
    }
}

/// One counted evaluation, as § 9.1 asks every candidate to be logged.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    /// 1 for the first evaluation; also the run's evaluation count after it.
    pub sequence: u64,
    /// The restart it belongs to, from 0.
    pub restart: u32,
    pub outcome: EvaluationOutcome,
    /// The candidate's normalized values, at their contracts' indices; [`patch_at`] gives its
    /// patch.
    pub normalized: Vec<f64>,
    /// The loss components, for a candidate that was not rejected.
    pub components: Option<LossComponents>,
    /// The weighted total, for a candidate that was not rejected.
    pub total: Option<f64>,
    /// The run's best loss after this evaluation.
    pub best_after: Option<f64>,
    /// Why the candidate was rejected, for a rejected one.
    pub rejection: Option<Rejection>,
}

/// A completed fit: the run, its best patch and render, and the log of every evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct FitOutcome {
    /// The completed run; its candidate is the [`Evaluation::sequence`] of the retained best.
    pub run: FitRun<u64>,
    /// The restarts that began; each rendered at least its start.
    pub restarts: u32,
    /// The best candidate's normalized values.
    pub normalized: Vec<f64>,
    /// The best patch, in physical values ([`patch_at`] of [`Self::normalized`]).
    pub patch: Patch,
    /// The clip the best patch was scored on: its render cut or padded to the reference's
    /// length. Comparing it with the reference gives the best loss again.
    pub render: AudioClip,
    /// What the renderer reported for the best patch's render.
    pub render_report: RenderReport,
    /// Every counted evaluation, in order.
    pub log: Vec<Evaluation>,
}

/// The default point of `topology`: every parameter at its contract's normalized default.
pub fn default_point(topology: &Topology) -> Vec<f64> {
    let mut point = vec![0.0; topology.contracts().len()];
    for contract in topology.contracts() {
        if let Some(value) = point.get_mut(contract.index) {
            *value = contract.default_normalized;
        }
    }
    point
}

/// The starting point of restart `restart >= 1` of a run seeded with `seed`, `len` values: the
/// first `len` draws of `Xoshiro256StarStar::new(s)`, where `s` is the `restart`-th output of
/// `SplitMix64::new(seed)` (module documentation). Restart 0 starts at [`default_point`], never
/// here.
pub fn restart_point(seed: u64, restart: u32, len: usize) -> Vec<f64> {
    let mut mix = SplitMix64::new(seed);
    let mut restart_seed = 0;
    for _ in 0..restart {
        restart_seed = mix.next_u64();
    }
    let mut prng = Xoshiro256StarStar::new(restart_seed);
    (0..len).map(|_| prng.next_f64()).collect()
}

/// The patch of the normalized point `normalized`: the fixed chain of the topology
/// (`FIXED_CHAIN_KINDS` as nodes 1 to 7 in their order, `FIXED_CHAIN_EDGES` between them, each
/// port's rate the one its kind declares), each parameter at the physical value its contract
/// maps `normalized[contract.index]` to. A value outside `[0, 1]` is clamped
/// (`ParameterContract::apply`); a missing one is the contract's default; the physical value is
/// held inside the contract's range against rounding at its ends. Refused only for a NaN or
/// infinite value, or a node parameter with no contract in `topology`.
pub fn patch_at(topology: &Topology, normalized: &[f64]) -> Result<Patch, SetParameterError> {
    let value_of = |path: &str| -> Result<f64, SetParameterError> {
        let contract =
            topology
                .contract(path)
                .ok_or_else(|| SetParameterError::UnknownParameterPath {
                    path: path.to_owned(),
                })?;
        let p = normalized
            .get(contract.index)
            .copied()
            .unwrap_or(contract.default_normalized);
        let physical = contract.apply(p)?.physical;
        let low = contract.physical_at_zero.min(contract.physical_at_one);
        let high = contract.physical_at_zero.max(contract.physical_at_one);
        Ok(physical.clamp(low, high))
    };
    let id_of = |kind: NodeKind| -> u32 {
        FIXED_CHAIN_KINDS
            .iter()
            .position(|k| *k == kind)
            .map_or(0, |position| position as u32 + 1)
    };
    let rate_of = |ports: &[Port], name: &str| -> PortRate {
        ports
            .iter()
            .find(|port| port.name == name)
            .map_or(PortRate::Audio, |port| port.rate)
    };
    let mut nodes = Vec::with_capacity(FIXED_CHAIN_KINDS.len());
    for kind in FIXED_CHAIN_KINDS {
        let mut parameters = Vec::new();
        for path in kind.parameter_paths() {
            parameters.push(ParameterValue {
                name: (*path).to_owned(),
                value: value_of(path)?,
            });
        }
        nodes.push(NodeDecl {
            id: id_of(kind),
            kind,
            parameters,
        });
    }
    let edges = FIXED_CHAIN_EDGES
        .iter()
        .map(|edge| EdgeDecl {
            from_node: id_of(edge.from_kind),
            from_port: edge.from_port.to_owned(),
            from_rate: rate_of(edge.from_kind.output_ports(), edge.from_port),
            to_node: id_of(edge.to_kind),
            to_port: edge.to_port.to_owned(),
            to_rate: rate_of(edge.to_kind.input_ports(), edge.to_port),
        })
        .collect();
    Ok(Patch {
        format_version: FORMAT_VERSION,
        sample_rate_hz: SUPPORTED_SAMPLE_RATE_HZ,
        topology_id: topology.name().to_owned(),
        nodes,
        edges,
    })
}

/// Refuses a reference the fit cannot run against (module documentation, step 3).
fn check_reference(reference: &AudioClip) -> Result<(), FitError> {
    match analyze_clip(reference, &AnalysisConfig::default()) {
        Ok(_) => Ok(()),
        Err(cause @ AnalysisError::SilentInput { .. }) => Err(FitError::ReferenceSilent { cause }),
        Err(
            cause @ (AnalysisError::PitchAmbiguous { .. }
            | AnalysisError::ClipTooShortForPitch { .. }
            | AnalysisError::PitchOutOfRange { .. }),
        ) => Err(FitError::PitchAmbiguous { cause }),
        Err(error) => Err(FitError::Compare(CompareError::Analysis(error))),
    }
}

/// Why the search stopped early: a bound, or a renderer fault with its message.
enum Halt {
    Stop(StopReason),
    Fault(String),
}

/// How much `after` lowers `before`: infinite from no loss to a loss, 0 without a loss after.
fn gain(before: Option<f64>, after: Option<f64>) -> f64 {
    match (before, after) {
        (Some(before), Some(after)) => before - after,
        (None, Some(_)) => f64::INFINITY,
        (_, None) => 0.0,
    }
}

/// The best candidate's patch and render, kept beside the run.
struct Best {
    normalized: Vec<f64>,
    patch: Patch,
    render: AudioClip,
    report: RenderReport,
}

/// The state of one fit while it searches.
struct Search<'a> {
    reference: &'a AudioClip,
    notes: &'a NoteSequence,
    topology: &'a Topology,
    config: &'a FitConfig,
    started: Instant,
    run: FitRun<u64>,
    restart: u32,
    log: Vec<Evaluation>,
    best: Option<Best>,
}

impl Search<'_> {
    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The bound that refuses the next render or the next restart, if one does. The run's first
    /// candidate is never asked about (module documentation).
    fn bound_reached(&self) -> Option<StopReason> {
        let bounds = &self.config.bounds;
        if self.run.evaluations() >= bounds.max_renders {
            return Some(StopReason::RenderBudgetExhausted);
        }
        let limit = u64::try_from(bounds.max_wall_time_ms).unwrap_or(0);
        if self.elapsed_ms() >= limit {
            return Some(StopReason::TimeBudgetExhausted);
        }
        None
    }

    /// Renders and scores `point` unless a bound refuses it; its loss, or `None` when it was
    /// rejected.
    fn evaluate(&mut self, point: &[f64]) -> Result<Option<f64>, Halt> {
        if let Some(reason) = self.bound_reached() {
            return Err(Halt::Stop(reason));
        }
        self.measure(point)
    }

    /// Renders and scores `point` without asking the bounds: the run's first candidate, and the
    /// start of a restart that [`Self::restarts`] has just let begin.
    fn measure(&mut self, point: &[f64]) -> Result<Option<f64>, Halt> {
        let patch =
            patch_at(self.topology, point).map_err(|error| Halt::Fault(error.to_string()))?;
        let validated = validate(&patch)
            .map_err(|error| Halt::Fault(format!("{}: {error}", error.spec_error())))?;
        let plan = compile(&validated);
        let sequence = u64::try_from(self.run.evaluations()).unwrap_or(0) + 1;
        let measured = render(&plan, self.notes, &self.config.render)
            .map_err(Rejection::Render)
            .and_then(|rendered| {
                let mut samples = rendered.samples;
                samples.resize(self.reference.frame_count(), 0.0);
                let clip = AudioClip::from_mono(samples).map_err(Rejection::Audio)?;
                let comparison = compare(self.reference, &clip).map_err(Rejection::Compare)?;
                Ok((comparison, clip, rendered.report))
            });
        let (outcome, scored, loss) = match measured {
            Ok((comparison, clip, report)) => {
                let outcome = self
                    .run
                    .evaluate(sequence, comparison.components, comparison.total)
                    .map_err(halt)?;
                if outcome.retained() {
                    self.best = Some(Best {
                        normalized: point.to_vec(),
                        patch,
                        render: clip,
                        report,
                    });
                }
                if outcome == EvaluationOutcome::Rejected {
                    (outcome, Err(Rejection::NonFiniteLoss), None)
                } else {
                    (outcome, Ok(comparison), Some(comparison.total))
                }
            }
            Err(rejection) => (self.run.reject().map_err(halt)?, Err(rejection), None),
        };
        self.log.push(Evaluation {
            sequence,
            restart: self.restart,
            outcome,
            normalized: point.to_vec(),
            components: scored.as_ref().ok().map(|c| c.components),
            total: scored.as_ref().ok().map(|c| c.total),
            best_after: self.run.best_loss(),
            rejection: scored.err(),
        });
        Ok(loss)
    }

    /// One restart's coordinate search from `start`, which is evaluated unasked (module
    /// documentation).
    fn descend(&mut self, start: Vec<f64>) -> Result<(), Halt> {
        let threshold = self.config.bounds.improvement_threshold;
        let mut current_loss = self.measure(&start)?;
        let mut current = start;
        let mut step = INITIAL_STEP;
        loop {
            let sweep_start = current_loss;
            for index in 0..current.len() {
                for direction in [1.0, -1.0] {
                    let value = (current[index] + direction * step).clamp(0.0, 1.0);
                    if value == current[index] {
                        continue;
                    }
                    let mut candidate = current.clone();
                    candidate[index] = value;
                    let loss = self.evaluate(&candidate)?;
                    let lower = match (loss, current_loss) {
                        (Some(loss), Some(current)) => loss < current,
                        (Some(_), None) => true,
                        (None, _) => false,
                    };
                    if lower {
                        current = candidate;
                        current_loss = loss;
                        break;
                    }
                }
            }
            let improvement = gain(sweep_start, current_loss);
            if improvement > 0.0 {
                if improvement < threshold {
                    return Ok(());
                }
            } else {
                step /= 2.0;
                if step < MIN_STEP {
                    return Ok(());
                }
            }
        }
    }

    /// The restarts, until a bound stops the run or they end (module documentation).
    fn restarts(&mut self) -> Result<StopReason, Halt> {
        let bounds = self.config.bounds;
        let len = self.topology.contracts().len();
        let count = u32::try_from(bounds.max_restarts).unwrap_or(u32::MAX);
        for restart in 0..count {
            if restart >= 1
                && let Some(reason) = self.bound_reached()
            {
                return Ok(reason);
            }
            self.restart = restart;
            let before = self.run.best_loss();
            let start = if restart == 0 {
                default_point(self.topology)
            } else {
                restart_point(self.config.seed, restart, len)
            };
            self.descend(start)?;
            if restart >= 1 && gain(before, self.run.best_loss()) < bounds.improvement_threshold {
                return Ok(StopReason::ImprovementBelowThreshold);
            }
        }
        Ok(StopReason::RestartsExhausted)
    }
}

/// A command the search admitted beforehand was refused: only the render budget can be, and
/// [`Search::bound_reached`] checks it first.
fn halt(error: FitError) -> Halt {
    match error {
        FitError::RenderBudgetExhausted => Halt::Stop(StopReason::RenderBudgetExhausted),
        error => Halt::Fault(error.to_string()),
    }
}

/// Fits a patch of `topology` to `reference`, driven by `notes`, within `config`'s bounds
/// (module documentation). Returns the completed run with its best patch, render and log, or
/// the refusal; a run that fails returns [`FitError::NoValidCandidate`] or
/// [`FitError::RendererFault`].
pub fn fit(
    reference: &AudioClip,
    notes: &NoteSequence,
    topology: &Topology,
    config: &FitConfig,
) -> Result<FitOutcome, FitError> {
    let started = Instant::now();
    config.bounds.check()?;
    if known_topology(topology.name()).is_none() {
        return Err(FitError::TopologyNotFound {
            topology: topology.name().to_owned(),
        });
    }
    check_reference(reference)?;
    let mut search = Search {
        reference,
        notes,
        topology,
        config,
        started,
        run: FitRun::start(config.bounds)?,
        restart: 0,
        log: Vec::new(),
        best: None,
    };
    let ended = search.restarts();
    let wall_time_ms = search.elapsed_ms();
    let reason = match ended {
        Ok(reason) | Err(Halt::Stop(reason)) => reason,
        Err(Halt::Fault(message)) => {
            search
                .run
                .fail(FailureReason::RendererFault, wall_time_ms)?;
            return Err(FitError::RendererFault { message });
        }
    };
    match search.run.complete(reason, wall_time_ms) {
        Ok(()) => {}
        Err(FitError::NoValidCandidate) => {
            search
                .run
                .fail(FailureReason::NoValidCandidate, wall_time_ms)?;
            return Err(FitError::NoValidCandidate);
        }
        Err(error) => return Err(error),
    }
    let Some(best) = search.best else {
        return Err(FitError::NoValidCandidate);
    };
    Ok(FitOutcome {
        run: search.run,
        restarts: search.restart + 1,
        normalized: best.normalized,
        patch: best.patch,
        render: best.render,
        render_report: best.report,
        log: search.log,
    })
}
