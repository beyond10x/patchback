//! The fit run of `spec/domains/fit.yaml` (`FitRun`, lifecycle Running → Completed | Failed) and
//! its commands as methods, each answering its outcomes in the model's order:
//!
//! | Method | Command | Outcomes, in order |
//! |---|---|---|
//! | [`FitRun::start`] | `StartFit` | `invalid-render-bound`, `invalid-time-bound`, `invalid-restart-bound`, `started` |
//! | [`FitRun::evaluate`] | `EvaluateCandidate` | `not-running`, `budget-exhausted`, `first-retained`, `improved`, `not-improved` |
//! | [`FitRun::reject`] | `RejectCandidate` | `not-running`, `budget-exhausted`, `rejected` |
//! | [`FitRun::complete`] | `CompleteFit` | `no-valid-candidate`, `completed`, `wrong-state` |
//! | [`FitRun::fail`] | `FailFit` | `failed`, `wrong-state` |
//!
//! `no-such-run` (`FitRunNotFound`) has no counterpart: a run is a value, so a command cannot
//! name a run that does not exist. The other `StartFit` outcomes (unknown references, note
//! confidence, the weight artifact, silence and pitch) are [`crate::fit`]'s and the CLI's.
//!
//! The run is generic over its candidate `C`, the model's `best_patch_id`: whatever identifies
//! the retained candidate to the caller ([`crate::fit`] uses the evaluation's sequence number).
//!
//! One guard goes beyond the model: [`FitRun::evaluate`] counts a candidate whose total or any
//! component is NaN, infinite or negative as rejected, so a numerically invalid loss never
//! replaces the best (DESIGN.md § 2 invariant 4, § 9.2 step 5), whoever offers it.

use std::fmt;

use crate::{FitError, LossComponents};

/// The lifecycle state of a run (`FitRun.State`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitState {
    Running,
    Completed,
    Failed,
}

impl FitState {
    /// The state's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

impl fmt::Display for FitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a completed run stopped (`StopReason`, DESIGN.md § 9.2 step 4, § 12.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    RenderBudgetExhausted,
    TimeBudgetExhausted,
    RestartsExhausted,
    ImprovementBelowThreshold,
}

impl StopReason {
    pub const ALL: [Self; 4] = [
        Self::RenderBudgetExhausted,
        Self::TimeBudgetExhausted,
        Self::RestartsExhausted,
        Self::ImprovementBelowThreshold,
    ];

    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::RenderBudgetExhausted => "RenderBudgetExhausted",
            Self::TimeBudgetExhausted => "TimeBudgetExhausted",
            Self::RestartsExhausted => "RestartsExhausted",
            Self::ImprovementBelowThreshold => "ImprovementBelowThreshold",
        }
    }
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a run failed without a patch (`FailureReason`, DESIGN.md § 9.2 step 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureReason {
    NoValidCandidate,
    RendererFault,
}

impl FailureReason {
    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::NoValidCandidate => "NoValidCandidate",
            Self::RendererFault => "RendererFault",
        }
    }
}

impl fmt::Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The bound an [`FitError::InvalidSearchBound`] names (`SearchBound`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchBound {
    RenderEvaluations,
    WallTime,
    Restarts,
}

impl SearchBound {
    /// The variant's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::RenderEvaluations => "RenderEvaluations",
            Self::WallTime => "WallTime",
            Self::Restarts => "Restarts",
        }
    }
}

impl fmt::Display for SearchBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What one counted evaluation did to the run: the outcomes of `EvaluateCandidate`
/// (`first-retained`, `improved`, `not-improved`) and of `RejectCandidate` (`rejected`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluationOutcome {
    /// The first valid candidate: both the loss before and the retained best.
    FirstRetained,
    /// A strictly lower loss than the retained best, which it replaces.
    Improved,
    /// An equal or higher loss; the best is unchanged.
    NotImproved,
    /// A numerically invalid candidate (a refused render or comparison, a non-finite or negative
    /// loss); counted, never retained.
    Rejected,
}

impl EvaluationOutcome {
    /// The outcome's name in `spec/domains/fit.yaml`.
    pub fn name(self) -> &'static str {
        match self {
            Self::FirstRetained => "first-retained",
            Self::Improved => "improved",
            Self::NotImproved => "not-improved",
            Self::Rejected => "rejected",
        }
    }

    /// Whether the candidate became the retained best.
    pub fn retained(self) -> bool {
        matches!(self, Self::FirstRetained | Self::Improved)
    }
}

impl fmt::Display for EvaluationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The bounds of a run (`FitRun.max_renders`, `max_wall_time_ms`, `max_restarts`,
/// `improvement_threshold`). Signed, as the model types them, so a negative bound can be stated
/// and refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitBounds {
    /// The most render evaluations the run counts.
    pub max_renders: i64,
    /// The wall time after which no further render starts, in milliseconds.
    pub max_wall_time_ms: i64,
    /// The number of restarts of the coordinate search.
    pub max_restarts: i64,
    /// A restart that lowers the best loss by less than this ends the search; a sweep that
    /// improves by less than this ends its restart ([`crate::fit`]).
    pub improvement_threshold: f64,
}

/// The default of `--max-renders`: DESIGN.md § 9.2's "up to 2,000 render evaluations per note".
/// Provisional (D-a6-1).
pub const DEFAULT_MAX_RENDERS: i64 = 2_000;
/// The default of `--max-wall-time-ms`: one minute. Provisional (D-a6-1); DESIGN.md § 12.3
/// names no value.
pub const DEFAULT_MAX_WALL_TIME_MS: i64 = 60_000;
/// The default of `--max-restarts`: four, as the scenarios carry. Provisional (D-a6-1).
pub const DEFAULT_MAX_RESTARTS: i64 = 4;
/// The default of `--improvement-threshold`, in units of the § 9.1 loss. Provisional (D-a6-1,
/// OQ-14).
pub const DEFAULT_IMPROVEMENT_THRESHOLD: f64 = 1e-4;

impl Default for FitBounds {
    fn default() -> Self {
        Self {
            max_renders: DEFAULT_MAX_RENDERS,
            max_wall_time_ms: DEFAULT_MAX_WALL_TIME_MS,
            max_restarts: DEFAULT_MAX_RESTARTS,
            improvement_threshold: DEFAULT_IMPROVEMENT_THRESHOLD,
        }
    }
}

impl FitBounds {
    /// Refuses a bound that is not positive, the first in the model's order: renders, wall time,
    /// restarts (`StartFit` `invalid-render-bound`, `invalid-time-bound`,
    /// `invalid-restart-bound`). The threshold is not checked: the model refuses none.
    pub fn check(&self) -> Result<(), FitError> {
        for (bound, value) in [
            (SearchBound::RenderEvaluations, self.max_renders),
            (SearchBound::WallTime, self.max_wall_time_ms),
            (SearchBound::Restarts, self.max_restarts),
        ] {
            if value <= 0 {
                return Err(FitError::InvalidSearchBound { bound, value });
            }
        }
        Ok(())
    }
}

/// One fit run: its bounds, its counters, its retained best candidate and how it ended.
#[derive(Clone, Debug, PartialEq)]
pub struct FitRun<C> {
    bounds: FitBounds,
    state: FitState,
    evaluations: i64,
    initial_loss: Option<f64>,
    best_loss: Option<f64>,
    best: Option<C>,
    best_components: Option<LossComponents>,
    stop_reason: Option<StopReason>,
    failure_reason: Option<FailureReason>,
    wall_time_ms: Option<u64>,
}

/// Whether a loss is one a valid candidate can have: finite and non-negative, each component
/// too (§ 9.1: every component is finite and non-negative for every accepted input).
fn valid_loss(components: &LossComponents, total: f64) -> bool {
    [
        total,
        components.log_spectral,
        components.envelope,
        components.transient,
        components.band_energy,
    ]
    .iter()
    .all(|value| value.is_finite() && *value >= 0.0)
}

impl<C> FitRun<C> {
    /// Starts a run Running with no evaluation, or refuses its bounds ([`FitBounds::check`]).
    pub fn start(bounds: FitBounds) -> Result<Self, FitError> {
        bounds.check()?;
        Ok(Self {
            bounds,
            state: FitState::Running,
            evaluations: 0,
            initial_loss: None,
            best_loss: None,
            best: None,
            best_components: None,
            stop_reason: None,
            failure_reason: None,
            wall_time_ms: None,
        })
    }

    /// The refusals `EvaluateCandidate` and `RejectCandidate` share, in order: a run that is not
    /// Running, then a spent render budget.
    fn admit(&self) -> Result<(), FitError> {
        if self.state != FitState::Running {
            return Err(FitError::FitNotRunning { state: self.state });
        }
        if self.evaluations >= self.bounds.max_renders {
            return Err(FitError::RenderBudgetExhausted);
        }
        Ok(())
    }

    /// Counts one evaluation of `candidate` with this loss and retains it when it is the first
    /// valid candidate or strictly better than the retained best (`EvaluateCandidate`). A
    /// refused candidate is not counted. A loss that is not finite and non-negative is counted
    /// as [`EvaluationOutcome::Rejected`] (module documentation).
    pub fn evaluate(
        &mut self,
        candidate: C,
        components: LossComponents,
        total: f64,
    ) -> Result<EvaluationOutcome, FitError> {
        self.admit()?;
        self.evaluations += 1;
        if !valid_loss(&components, total) {
            return Ok(EvaluationOutcome::Rejected);
        }
        let outcome = match self.best_loss {
            None => {
                self.initial_loss = Some(total);
                EvaluationOutcome::FirstRetained
            }
            Some(best) if best > total => EvaluationOutcome::Improved,
            Some(_) => return Ok(EvaluationOutcome::NotImproved),
        };
        self.best_loss = Some(total);
        self.best = Some(candidate);
        self.best_components = Some(components);
        Ok(outcome)
    }

    /// Counts one evaluation of a numerically invalid candidate; the best is unchanged
    /// (`RejectCandidate`). A refused candidate is not counted.
    pub fn reject(&mut self) -> Result<EvaluationOutcome, FitError> {
        self.admit()?;
        self.evaluations += 1;
        Ok(EvaluationOutcome::Rejected)
    }

    /// Ends a Running run with its retained best, recording why it stopped and its wall time
    /// (`CompleteFit`). Refused with [`FitError::NoValidCandidate`] when nothing was retained
    /// (the run stays Running and can only fail), with [`FitError::FitNotRunning`] once ended.
    pub fn complete(&mut self, stop_reason: StopReason, wall_time_ms: u64) -> Result<(), FitError> {
        if self.state == FitState::Running && self.best_loss.is_none() {
            return Err(FitError::NoValidCandidate);
        }
        if self.state != FitState::Running {
            return Err(FitError::FitNotRunning { state: self.state });
        }
        self.state = FitState::Completed;
        self.stop_reason = Some(stop_reason);
        self.wall_time_ms = Some(wall_time_ms);
        Ok(())
    }

    /// Ends a Running run without a patch (`FailFit`); refused with [`FitError::FitNotRunning`]
    /// once ended.
    pub fn fail(&mut self, reason: FailureReason, wall_time_ms: u64) -> Result<(), FitError> {
        if self.state != FitState::Running {
            return Err(FitError::FitNotRunning { state: self.state });
        }
        self.state = FitState::Failed;
        self.failure_reason = Some(reason);
        self.wall_time_ms = Some(wall_time_ms);
        Ok(())
    }

    pub fn bounds(&self) -> &FitBounds {
        &self.bounds
    }

    pub fn state(&self) -> FitState {
        self.state
    }

    /// Render evaluations counted so far, valid and invalid alike.
    pub fn evaluations(&self) -> i64 {
        self.evaluations
    }

    /// The loss of the first valid candidate: the report's loss before.
    pub fn initial_loss(&self) -> Option<f64> {
        self.initial_loss
    }

    /// The loss of the retained best: the report's loss after.
    pub fn best_loss(&self) -> Option<f64> {
        self.best_loss
    }

    /// The retained best candidate.
    pub fn best(&self) -> Option<&C> {
        self.best.as_ref()
    }

    pub fn best_components(&self) -> Option<&LossComponents> {
        self.best_components.as_ref()
    }

    pub fn stop_reason(&self) -> Option<StopReason> {
        self.stop_reason
    }

    pub fn failure_reason(&self) -> Option<FailureReason> {
        self.failure_reason
    }

    /// The wall time recorded when the run ended.
    pub fn wall_time_ms(&self) -> Option<u64> {
        self.wall_time_ms
    }
}
