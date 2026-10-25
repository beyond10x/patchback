//! One `modelspec train` run (domain entity `patchback.model.TrainingRun`; DESIGN.md § 8.1,
//! § 8.2 step 1).
//!
//! A [`TrainingRun`] is a value, and the four commands of the domain are its methods, in the
//! domain's order: [`TrainingRun::plan`] (`PlanTrainingRun`), [`TrainingRun::bind`]
//! (`BindModelSource`), [`TrainingRun::start`] (`StartTrainingRun`) and [`TrainingRun::finish`]
//! (`FinishTrainingRun`). Each refuses with a [`TrainingError`] naming a declared error, the
//! run's own state first (as the scenarios decide it) and then the outcomes in their declared
//! order; a refusal changes nothing.
//!
//! - `plan` refuses [`Backend::Vulkan`] with `BackendNotShipped`: the pin file records no shipped
//!   Vulkan backend (Da8-22), so `GpuEquivalenceNotRecorded`, the outcome after it, is never
//!   reached.
//! - `bind` takes the [`ModelSource`] (or a [`CheckedModel`] of one) and refuses, in the order the
//!   domain's interpreter answers (Da8-39): a run past `Planned` (`TrainingRunPastPlanning`),
//!   then a source that is not `Checked` (`ModelNotChecked`, through
//!   [`ModelSource::trainable`]), then a source checked under another pin than the run's
//!   (`PinMismatch`).
//! - `start` reads the splits from a [`DatasetStore`] (`UnknownDataSplit`, `DataNotWritten`,
//!   `EvalNotWritten`, `EvalCountNotBatchMultiple` against the bound model's batch size), refuses
//!   a binary of another pin than the run's, then runs `modelspec train <source> --backend cpu
//!   --data <dir> [--eval <dir>] --steps <n>` through the crate's bounded subprocess path and
//!   holds what the child did. A split's directory is the one its `x.f32` was written in. The
//!   binary's own refusals (such as E-DATA-008, an `order.u32` shorter than steps × batch size)
//!   are not pre-empted: they fail the run (Da8-24).
//! - `finish` records the child's exit, and the exit status alone decides the state, as
//!   `FinishTrainingRun` declares (Da8-40): `Completed` on exit status 0 whatever the output,
//!   `Failed` otherwise (another status, a signal, the timeout), with the [`TrainingFailure`].
//!   The stdout of a run that exited 0 is parsed into its loss curve ([`LossCurve`]) and
//!   evaluation loss; a stdout without a `step` line, with a malformed one, or without the `eval`
//!   line `--eval` asked for is recorded beside the state as an [`OutputError`]
//!   ([`TrainingRun::output_error`]). The child's stdout and stderr stay on the value.
//!
//! No split-role guard is applied (the domain marks it `UNMAPPED`; Da8-23); the run records the
//! roles it was given. `UnknownPin`, `UnknownModelSource` and `UnknownTrainingRun` name records
//! this crate does not keep, so it has no counterpart for them (Da8-21).
//!
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use patchback_dataset::{
    DatasetSplit, DatasetSplitId, DatasetStore, DenseFileKind, SplitRole, SplitState,
    is_batch_multiple,
};

use crate::{
    CheckedModel, ModelNotChecked, ModelSource, ModelSpec, ModelSpecError, ModelSpecPin, run,
};

/// How long one `modelspec train` run may take unless the caller gives another bound.
/// Provisional (Da8-26): the time of a 5000-step CPU run of the § 8.1 model is not measured.
pub const DEFAULT_TRAINING_TIMEOUT: Duration = Duration::from_secs(600);

/// The `--backend` of `modelspec train` (domain enum `patchback.model.Backend`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    Cpu,
    Vulkan,
}

impl Backend {
    /// The variant name of the model.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Cpu => "Cpu",
            Backend::Vulkan => "Vulkan",
        }
    }

    /// The value `modelspec train --backend` takes.
    pub fn flag(self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Vulkan => "vulkan",
        }
    }
}

/// The lifecycle of a run (domain `patchback.model.TrainingRun.State`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrainingRunState {
    Planned,
    ModelBound,
    /// Started: the child ran, and its exit is not yet recorded.
    Running,
    Completed,
    Failed,
}

impl TrainingRunState {
    /// The state name of the model.
    pub fn name(self) -> &'static str {
        match self {
            TrainingRunState::Planned => "Planned",
            TrainingRunState::ModelBound => "ModelBound",
            TrainingRunState::Running => "Running",
            TrainingRunState::Completed => "Completed",
            TrainingRunState::Failed => "Failed",
        }
    }
}

/// Why a stdout does not hold the loss curve the run asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputError {
    /// No `step <k> loss <v>` line.
    NoStepLine,
    /// A line that starts with `step` or `eval` and is not `step <k> loss <v>` or
    /// `eval loss=<v> accuracy=<v>`, a `step` line whose number is not greater than the one
    /// before it, or a second `eval` line; its 1-based number and its text.
    Malformed { line: usize, text: String },
    /// The run was given `--eval`, and the stdout holds no `eval` line ([`TrainingRun::finish`];
    /// [`LossCurve::parse`] does not know whether `--eval` was given).
    NoEvalLine,
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::NoStepLine => f.write_str("the output holds no `step <k> loss <v>` line"),
            OutputError::NoEvalLine => {
                f.write_str("the run was given --eval and the output holds no `eval` line")
            }
            OutputError::Malformed { line, text } => {
                write!(f, "output line {line} is malformed: {text:?}")
            }
        }
    }
}

/// The loss curve of a run as `modelspec train` prints it: each `step <k> loss <v>` line in order
/// (step 0, every 50th step and the last; a step number not greater than the one before is
/// refused as malformed), and the `eval loss=<v> accuracy=<v>` line when the run
/// had `--eval`. The `accuracy` is read and dropped: § 8.1 reports the evaluation loss. Other
/// lines (`init`, `param`) are not read here.
#[derive(Clone, Debug, PartialEq)]
pub struct LossCurve {
    pub losses: Vec<(u64, f64)>,
    pub eval_loss: Option<f64>,
}

impl LossCurve {
    pub fn parse(stdout: &str) -> Result<LossCurve, OutputError> {
        let mut losses = Vec::new();
        let mut eval_loss = None;
        for (index, line) in stdout.lines().enumerate() {
            let malformed = || OutputError::Malformed {
                line: index + 1,
                text: line.to_owned(),
            };
            let mut words = line.split(' ');
            match words.next() {
                Some("step") => {
                    let parsed = match (words.next(), words.next(), words.next(), words.next()) {
                        (Some(step), Some("loss"), Some(loss), None) => step
                            .parse::<u64>()
                            .ok()
                            .filter(|step| losses.last().is_none_or(|(last, _)| step > last))
                            .zip(loss.parse::<f64>().ok()),
                        _ => None,
                    };
                    losses.push(parsed.ok_or_else(malformed)?);
                }
                Some("eval") => {
                    let parsed = match (words.next(), words.next(), words.next()) {
                        (Some(loss), Some(accuracy), None) if eval_loss.is_none() => loss
                            .strip_prefix("loss=")
                            .and_then(|v| v.parse::<f64>().ok())
                            .filter(|_| {
                                accuracy
                                    .strip_prefix("accuracy=")
                                    .is_some_and(|v| v.parse::<f64>().is_ok())
                            }),
                        _ => None,
                    };
                    eval_loss = Some(parsed.ok_or_else(malformed)?);
                }
                _ => {}
            }
        }
        if losses.is_empty() {
            return Err(OutputError::NoStepLine);
        }
        Ok(LossCurve { losses, eval_loss })
    }
}

/// Why a started run ended `Failed`.
#[derive(Clone, Debug, PartialEq)]
pub enum TrainingFailure {
    /// The child exited with a status other than 0.
    Exit { code: i32 },
    /// A signal ended the child.
    Signalled,
    /// The child outlasted the timeout and was killed.
    TimedOut { timeout: Duration },
    /// Waiting for the child or reading its output failed.
    Io { message: String },
}

impl fmt::Display for TrainingFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainingFailure::Exit { code } => write!(f, "modelspec train exited {code}"),
            TrainingFailure::Signalled => f.write_str("modelspec train was ended by a signal"),
            TrainingFailure::TimedOut { timeout } => {
                write!(
                    f,
                    "modelspec train ran longer than {timeout:?} and was killed"
                )
            }
            TrainingFailure::Io { message } => write!(f, "modelspec train: {message}"),
        }
    }
}

/// Why a command on a [`TrainingRun`] changed nothing. Every variant but the last three names a
/// declared error of `spec/domains/model.yaml` ([`TrainingError::domain_error`]).
#[derive(Debug)]
pub enum TrainingError {
    /// `PlanTrainingRun`: the pin records no shipped Vulkan backend.
    BackendNotShipped {
        backend: Backend,
        model_spec_version: String,
    },
    /// `BindModelSource`: the source is not `Checked` ([`ModelSource::trainable`]).
    ModelNotChecked(ModelNotChecked),
    /// `BindModelSource`: the source was checked under another pin than the run's.
    PinMismatch {
        run_pin: Box<ModelSpecPin>,
        model_pin: Box<ModelSpecPin>,
    },
    /// `BindModelSource`: the run is past `Planned`.
    TrainingRunPastPlanning { state: TrainingRunState },
    /// `StartTrainingRun`: the store holds no split of this identity for `--data`.
    UnknownDataSplit { dataset_split_id: DatasetSplitId },
    /// `StartTrainingRun`: the `--data` split is not `Written`.
    DataNotWritten {
        dataset_split_id: DatasetSplitId,
        state: SplitState,
    },
    /// `StartTrainingRun`: the `--eval` split is unknown (`state` `None`) or not `Written`.
    EvalNotWritten {
        eval_split_id: DatasetSplitId,
        state: Option<SplitState>,
    },
    /// `StartTrainingRun`: the `--eval` split's count is not a multiple of the model's batch size.
    EvalCountNotBatchMultiple {
        eval_split_id: DatasetSplitId,
        sample_count: i64,
        batch_size: usize,
    },
    /// `StartTrainingRun`: the run is not `ModelBound`.
    TrainingRunNotReady { state: TrainingRunState },
    /// `FinishTrainingRun`: the run is not `Running`.
    TrainingRunFinished { state: TrainingRunState },
    /// Not a domain error: `start` was handed a binary of another pin than the run's; nothing was
    /// spawned.
    OtherPin { run: String, binary: String },
    /// Not a domain error: a `Written` split without a recorded `x.f32`, so it has no directory.
    /// The store's `FinishSplit` admits no such split.
    MissingDenseFile { dataset_split_id: DatasetSplitId },
    /// Not a domain error: the binary could not be started; the run is still `ModelBound`.
    ModelSpec(ModelSpecError),
}

impl From<ModelNotChecked> for TrainingError {
    fn from(error: ModelNotChecked) -> Self {
        TrainingError::ModelNotChecked(error)
    }
}

impl TrainingError {
    /// The declared errors this crate emits, in the order the four commands declare them.
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.model.BackendNotShipped",
        "patchback.model.ModelNotChecked",
        "patchback.model.PinMismatch",
        "patchback.model.TrainingRunPastPlanning",
        "patchback.model.UnknownDataSplit",
        "patchback.model.DataNotWritten",
        "patchback.model.EvalNotWritten",
        "patchback.model.EvalCountNotBatchMultiple",
        "patchback.model.TrainingRunNotReady",
        "patchback.model.TrainingRunFinished",
    ];

    /// The declared error this refusal is, or `None` for a failure that is not the model's.
    pub fn domain_error(&self) -> Option<&'static str> {
        Some(match self {
            TrainingError::BackendNotShipped { .. } => "patchback.model.BackendNotShipped",
            TrainingError::ModelNotChecked(_) => "patchback.model.ModelNotChecked",
            TrainingError::PinMismatch { .. } => "patchback.model.PinMismatch",
            TrainingError::TrainingRunPastPlanning { .. } => {
                "patchback.model.TrainingRunPastPlanning"
            }
            TrainingError::UnknownDataSplit { .. } => "patchback.model.UnknownDataSplit",
            TrainingError::DataNotWritten { .. } => "patchback.model.DataNotWritten",
            TrainingError::EvalNotWritten { .. } => "patchback.model.EvalNotWritten",
            TrainingError::EvalCountNotBatchMultiple { .. } => {
                "patchback.model.EvalCountNotBatchMultiple"
            }
            TrainingError::TrainingRunNotReady { .. } => "patchback.model.TrainingRunNotReady",
            TrainingError::TrainingRunFinished { .. } => "patchback.model.TrainingRunFinished",
            TrainingError::OtherPin { .. }
            | TrainingError::MissingDenseFile { .. }
            | TrainingError::ModelSpec(_) => return None,
        })
    }

    /// One refusal of each declared error, in [`TrainingError::DOMAIN_ERRORS`] order.
    pub fn examples() -> Vec<TrainingError> {
        let pin = |version: &str, digit: &str| ModelSpecPin {
            version: version.to_owned(),
            commit: digit.repeat(40),
            sha256: digit.repeat(64),
        };
        let split = DatasetSplitId(1);
        vec![
            TrainingError::BackendNotShipped {
                backend: Backend::Vulkan,
                model_spec_version: "0.5.0".to_owned(),
            },
            TrainingError::ModelNotChecked(ModelNotChecked {
                state: crate::ModelSourceState::Rejected,
            }),
            TrainingError::PinMismatch {
                run_pin: Box::new(pin("0.5.0", "a")),
                model_pin: Box::new(pin("0.4.0", "b")),
            },
            TrainingError::TrainingRunPastPlanning {
                state: TrainingRunState::ModelBound,
            },
            TrainingError::UnknownDataSplit {
                dataset_split_id: split,
            },
            TrainingError::DataNotWritten {
                dataset_split_id: split,
                state: SplitState::Open,
            },
            TrainingError::EvalNotWritten {
                eval_split_id: DatasetSplitId(2),
                state: None,
            },
            TrainingError::EvalCountNotBatchMultiple {
                eval_split_id: DatasetSplitId(2),
                sample_count: 24,
                batch_size: 16,
            },
            TrainingError::TrainingRunNotReady {
                state: TrainingRunState::Planned,
            },
            TrainingError::TrainingRunFinished {
                state: TrainingRunState::Completed,
            },
        ]
    }
}

impl fmt::Display for TrainingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainingError::BackendNotShipped {
                backend,
                model_spec_version,
            } => write!(
                f,
                "the {} backend is not shipped in modelspec {model_spec_version}",
                backend.name()
            ),
            TrainingError::ModelNotChecked(error) => error.fmt(f),
            TrainingError::PinMismatch { run_pin, model_pin } => write!(
                f,
                "the model source was checked under modelspec {} ({}); the run uses modelspec {} ({})",
                model_pin.version(),
                model_pin.commit(),
                run_pin.version(),
                run_pin.commit()
            ),
            TrainingError::TrainingRunPastPlanning { state } => write!(
                f,
                "the training run is {}; only a Planned run binds a model source",
                state.name()
            ),
            TrainingError::UnknownDataSplit { dataset_split_id } => {
                write!(f, "no {dataset_split_id} to train on")
            }
            TrainingError::DataNotWritten {
                dataset_split_id,
                state,
            } => write!(
                f,
                "{dataset_split_id} is {}; only a Written split is trained on",
                state.name()
            ),
            TrainingError::EvalNotWritten {
                eval_split_id,
                state: None,
            } => write!(f, "no {eval_split_id} to evaluate on"),
            TrainingError::EvalNotWritten {
                eval_split_id,
                state: Some(state),
            } => write!(
                f,
                "{eval_split_id} is {}; only a Written split is evaluated on",
                state.name()
            ),
            TrainingError::EvalCountNotBatchMultiple {
                eval_split_id,
                sample_count,
                batch_size,
            } => write!(
                f,
                "{eval_split_id} holds {sample_count} samples, not a multiple of the batch size {batch_size}"
            ),
            TrainingError::TrainingRunNotReady { state } => write!(
                f,
                "the training run is {}; only a ModelBound run starts",
                state.name()
            ),
            TrainingError::TrainingRunFinished { state } => write!(
                f,
                "the training run is {}; only a Running run is finished",
                state.name()
            ),
            TrainingError::OtherPin { run, binary } => write!(
                f,
                "the training run uses modelspec {run}; the binary is modelspec {binary}"
            ),
            TrainingError::MissingDenseFile { dataset_split_id } => {
                write!(f, "{dataset_split_id} has no recorded x.f32")
            }
            TrainingError::ModelSpec(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TrainingError {}

/// How the child ended, held from `start` until `finish` records it.
#[derive(Clone, Debug, PartialEq)]
enum Exit {
    Code(i32),
    Signalled,
    TimedOut(Duration),
    Io(String),
}

/// One training run (domain entity `patchback.model.TrainingRun`): see the module documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainingRun {
    pin: ModelSpecPin,
    backend: Backend,
    state: TrainingRunState,
    model_source: Option<PathBuf>,
    batch_size: Option<usize>,
    data_split_id: Option<DatasetSplitId>,
    data_role: Option<SplitRole>,
    eval_split_id: Option<DatasetSplitId>,
    eval_role: Option<SplitRole>,
    steps: Option<u32>,
    exit: Option<Exit>,
    exit_status: Option<i32>,
    stdout: String,
    stderr: String,
    losses: Vec<(u64, f64)>,
    eval_loss: Option<f64>,
    failure: Option<TrainingFailure>,
    output_error: Option<OutputError>,
}

/// The directory of a split's dense files: the one its `x.f32` was written in.
fn split_dir(store: &DatasetStore, split: &DatasetSplit) -> Result<PathBuf, TrainingError> {
    let id = split.dataset_split_id;
    let files = store.dense_files_of(id);
    let x = files
        .iter()
        .find(|file| file.kind == DenseFileKind::X)
        .ok_or(TrainingError::MissingDenseFile {
            dataset_split_id: id,
        })?;
    Ok(match x.path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    })
}

impl TrainingRun {
    /// `PlanTrainingRun`: a `Planned` run on `backend` under `pin`. [`Backend::Vulkan`] is
    /// refused `BackendNotShipped`; nothing is spawned.
    pub fn plan(backend: Backend, pin: &ModelSpecPin) -> Result<TrainingRun, TrainingError> {
        if backend == Backend::Vulkan {
            return Err(TrainingError::BackendNotShipped {
                backend,
                model_spec_version: pin.version().to_owned(),
            });
        }
        Ok(TrainingRun {
            pin: pin.clone(),
            backend,
            state: TrainingRunState::Planned,
            model_source: None,
            batch_size: None,
            data_split_id: None,
            data_role: None,
            eval_split_id: None,
            eval_role: None,
            steps: None,
            exit: None,
            exit_status: None,
            stdout: String::new(),
            stderr: String::new(),
            losses: Vec::new(),
            eval_loss: None,
            failure: None,
            output_error: None,
        })
    }

    /// `BindModelSource` of a [`ModelSource`] (or of a [`CheckedModel`], which names its source):
    /// refused, in this order, for a run past `Planned` (`TrainingRunPastPlanning`), a source that
    /// is not `Checked` (`ModelNotChecked`) and a source checked under another pin than the run's
    /// (`PinMismatch`); else the run is `ModelBound` to the source.
    pub fn bind<S: AsRef<ModelSource> + ?Sized>(
        &mut self,
        source: &S,
    ) -> Result<(), TrainingError> {
        if self.state != TrainingRunState::Planned {
            return Err(TrainingError::TrainingRunPastPlanning { state: self.state });
        }
        let model: CheckedModel<'_> = source.as_ref().trainable()?;
        if model.model_spec_pin() != &self.pin {
            return Err(TrainingError::PinMismatch {
                run_pin: Box::new(self.pin.clone()),
                model_pin: Box::new(model.model_spec_pin().clone()),
            });
        }
        self.model_source = Some(model.path().to_path_buf());
        self.batch_size = Some(model.batch_size());
        self.state = TrainingRunState::ModelBound;
        Ok(())
    }

    /// `StartTrainingRun`: refused, in this order, for a run that is not `ModelBound`
    /// (`TrainingRunNotReady`), an unknown `data` split (`UnknownDataSplit`), one not `Written`
    /// (`DataNotWritten`), an `eval` split unknown or not `Written` (`EvalNotWritten`) or whose
    /// count is not a multiple of the bound model's batch size (`EvalCountNotBatchMultiple`), and
    /// `modelspec` of another pin than the run's ([`TrainingError::OtherPin`]). Then it runs
    /// `modelspec train <source> --backend cpu --data <dir> [--eval <dir>] --steps <steps>`,
    /// bounded by `timeout`, and the run is `Running` with the child's exit held for
    /// [`TrainingRun::finish`]. A binary that cannot be started leaves the run `ModelBound`.
    pub fn start(
        &mut self,
        store: &DatasetStore,
        data: DatasetSplitId,
        eval: Option<DatasetSplitId>,
        modelspec: &ModelSpec,
        steps: u32,
        timeout: Duration,
    ) -> Result<(), TrainingError> {
        if self.state != TrainingRunState::ModelBound {
            return Err(TrainingError::TrainingRunNotReady { state: self.state });
        }
        let (Some(source), Some(batch_size)) = (self.model_source.clone(), self.batch_size) else {
            return Err(TrainingError::TrainingRunNotReady { state: self.state });
        };
        let data_split = store.split(data).ok_or(TrainingError::UnknownDataSplit {
            dataset_split_id: data,
        })?;
        if data_split.state != SplitState::Written {
            return Err(TrainingError::DataNotWritten {
                dataset_split_id: data,
                state: data_split.state,
            });
        }
        let eval_split = match eval {
            None => None,
            Some(eval_split_id) => {
                let split = store.split(eval_split_id);
                match split.map(|s| s.state) {
                    Some(SplitState::Written) => {}
                    state => {
                        return Err(TrainingError::EvalNotWritten {
                            eval_split_id,
                            state,
                        });
                    }
                }
                split
            }
        };
        if let Some(split) = eval_split {
            let batch = u64::try_from(batch_size).unwrap_or(0);
            if !is_batch_multiple(split.sample_count, batch) {
                return Err(TrainingError::EvalCountNotBatchMultiple {
                    eval_split_id: split.dataset_split_id,
                    sample_count: split.sample_count,
                    batch_size,
                });
            }
        }
        if modelspec.pin() != &self.pin {
            return Err(TrainingError::OtherPin {
                run: self.pin.version().to_owned(),
                binary: modelspec.version().to_owned(),
            });
        }
        let data_dir = split_dir(store, data_split)?;
        let eval_dir = eval_split
            .map(|split| split_dir(store, split))
            .transpose()?;

        let steps_text = steps.to_string();
        let mut arguments: Vec<&OsStr> = vec![
            OsStr::new("train"),
            source.as_os_str(),
            OsStr::new("--backend"),
            OsStr::new(self.backend.flag()),
            OsStr::new("--data"),
            data_dir.as_os_str(),
        ];
        if let Some(eval_dir) = &eval_dir {
            arguments.extend([OsStr::new("--eval"), eval_dir.as_os_str()]);
        }
        arguments.extend([OsStr::new("--steps"), OsStr::new(&steps_text)]);
        let exit = match run(modelspec.binary(), &arguments, timeout) {
            Ok(output) => {
                self.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                self.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                output.status.code().map_or(Exit::Signalled, Exit::Code)
            }
            Err(error @ ModelSpecError::Spawn { .. }) => {
                return Err(TrainingError::ModelSpec(error));
            }
            Err(ModelSpecError::TimedOut { timeout, .. }) => Exit::TimedOut(timeout),
            Err(error) => Exit::Io(error.to_string()),
        };
        self.data_split_id = Some(data);
        self.data_role = Some(data_split.role);
        self.eval_split_id = eval_split.map(|split| split.dataset_split_id);
        self.eval_role = eval_split.map(|split| split.role);
        self.steps = Some(steps);
        self.exit = Some(exit);
        self.state = TrainingRunState::Running;
        Ok(())
    }

    /// `FinishTrainingRun`: refused `TrainingRunFinished` for a run that is not `Running`. Else
    /// the child's exit is recorded, and the exit status alone decides the state: `Completed` on
    /// exit status 0, `Failed` with its [`TrainingFailure`] otherwise. On exit status 0 the stdout
    /// is parsed into the loss curve, and what is wrong with it (no `step` line, a malformed line,
    /// no `eval` line on a run given `--eval`) is recorded as
    /// [`TrainingRun::output_error`]. Returns the state reached.
    pub fn finish(&mut self) -> Result<TrainingRunState, TrainingError> {
        let (TrainingRunState::Running, Some(exit)) = (self.state, self.exit.take()) else {
            return Err(TrainingError::TrainingRunFinished { state: self.state });
        };
        let failure = match exit {
            Exit::Code(0) => {
                self.exit_status = Some(0);
                match LossCurve::parse(&self.stdout) {
                    Ok(curve) => {
                        if self.eval_split_id.is_some() && curve.eval_loss.is_none() {
                            self.output_error = Some(OutputError::NoEvalLine);
                        }
                        self.losses = curve.losses;
                        self.eval_loss = curve.eval_loss;
                    }
                    Err(error) => self.output_error = Some(error),
                }
                None
            }
            Exit::Code(code) => {
                self.exit_status = Some(code);
                Some(TrainingFailure::Exit { code })
            }
            Exit::Signalled => Some(TrainingFailure::Signalled),
            Exit::TimedOut(timeout) => Some(TrainingFailure::TimedOut { timeout }),
            Exit::Io(message) => Some(TrainingFailure::Io { message }),
        };
        self.state = if failure.is_some() {
            TrainingRunState::Failed
        } else {
            TrainingRunState::Completed
        };
        self.failure = failure;
        Ok(self.state)
    }

    pub fn state(&self) -> TrainingRunState {
        self.state
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The pin the run was planned under.
    pub fn pin(&self) -> &ModelSpecPin {
        &self.pin
    }

    /// The bound model's `.mspec` path, once bound.
    pub fn model_source(&self) -> Option<&Path> {
        self.model_source.as_deref()
    }

    pub fn data_split_id(&self) -> Option<DatasetSplitId> {
        self.data_split_id
    }

    /// The role of the `--data` split, as given (no role is refused; Da8-23).
    pub fn data_role(&self) -> Option<SplitRole> {
        self.data_role
    }

    pub fn eval_split_id(&self) -> Option<DatasetSplitId> {
        self.eval_split_id
    }

    /// The role of the `--eval` split, as given.
    pub fn eval_role(&self) -> Option<SplitRole> {
        self.eval_role
    }

    /// The `--steps` the run was started with.
    pub fn steps(&self) -> Option<u32> {
        self.steps
    }

    /// The child's exit status, once `finish` recorded it; `None` for a child that a signal or
    /// the timeout ended.
    pub fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    /// The child's stdout as it printed it (the `param` lines included).
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// The child's stderr as it printed it.
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    /// The `(step, loss)` lines of a `Completed` run, in order.
    pub fn losses(&self) -> &[(u64, f64)] {
        &self.losses
    }

    /// The evaluation loss of a `Completed` run given `--eval`.
    pub fn eval_loss(&self) -> Option<f64> {
        self.eval_loss
    }

    /// Why a `Failed` run failed.
    pub fn failure(&self) -> Option<&TrainingFailure> {
        self.failure.as_ref()
    }

    /// What is wrong with the stdout of a run that exited 0, recorded beside its `Completed`
    /// state (Da8-40); `None` when the loss curve, and the `eval` line `--eval` asked for, were
    /// read.
    pub fn output_error(&self) -> Option<&OutputError> {
        self.output_error.as_ref()
    }
}
