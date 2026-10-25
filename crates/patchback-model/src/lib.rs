//! The ModelSpec CLI adapter and the native CPU weight runner (DESIGN.md § 4.1, § 8).
//!
//! The adapter runs the pinned ModelSpec binary as a subprocess (§ 8.2 step 1) and nothing else:
//!
//! 1. [`ModelSpecPin`] is the pin, read from `models/modelspec.pin` ([`PIN_FILE`]), the one file
//!    CI also reads to download and verify the binary.
//! 2. [`locate`] finds a binary: an explicit path, else the `MODELSPEC` variable, else a
//!    `modelspec` executable on `PATH`. A path that is named but missing is
//!    [`ModelSpecError::NotFound`]; the search does not fall through to the next source. A
//!    slash-free relative path (`MODELSPEC=modelspec`, or an empty `PATH` entry) is run as
//!    `./<name>`, so the file located is the file spawned.
//! 3. [`ModelSpec::open`] runs `<binary> --version` and refuses a binary that does not report
//!    `modelspec <version of the given pin>` ([`ModelSpecError::VersionMismatch`]).
//! 4. [`ModelSource::check`] runs `<binary> check <source>` and records the source `Checked` on
//!    exit status 0 and `Rejected`, with the binary's diagnostic, on exit status 1 (domain
//!    `patchback.model`, command `CheckModelSource`). A source is checked once. Everything that
//!    is not the binary's verdict on the model (a source it cannot read, a binary of another pin,
//!    another exit status, a signal, a run past the timeout) is an error and leaves the source
//!    `Unchecked`.
//! 5. [`ModelSource::trainable`] is the guard a training run takes: only a `Checked` source yields
//!    a [`CheckedModel`], which nothing else can construct.
//! 6. [`TrainingRun`] runs `modelspec train` on a checked model and written dense data
//!    (`PlanTrainingRun`, `BindModelSource`, `StartTrainingRun`, `FinishTrainingRun`), refusing
//!    the Vulkan backend before anything is spawned, and records the loss curve it printed
//!    ([`LossCurve`]) and whether it completed or failed (module `training`).
//!
//! Every run of the binary is bounded by a timeout ([`DEFAULT_TIMEOUT`] unless the caller gives
//! one to [`ModelSpec::open_with_timeout`], [`DEFAULT_TRAINING_TIMEOUT`] for a training run unless
//! the caller gives another); past it the process is killed and waited for.

mod training;

pub use training::{
    Backend, DEFAULT_TRAINING_TIMEOUT, LossCurve, OutputError, TrainingError, TrainingFailure,
    TrainingRun, TrainingRunState,
};

use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The pin file, relative to the repository root.
pub const PIN_FILE: &str = "models/modelspec.pin";

/// The pin's key for the checksum of the release asset CI installs.
pub const SHA256_KEY: &str = "sha256-x86_64-unknown-linux-gnu";

/// The variable that names the ModelSpec binary when no explicit path is given.
pub const MODELSPEC_VARIABLE: &str = "MODELSPEC";

/// The executable name searched for on `PATH`.
pub const EXECUTABLE: &str = "modelspec";

/// How long one run of the binary (`--version` or `check`) may take unless the caller sets
/// another bound with [`ModelSpec::open_with_timeout`]. The pinned 0.5.0 binary checks the § 8.1
/// model in well under a second; a minute leaves room for a loaded CI runner.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// The exit status with which `modelspec check` refuses a model.
const REJECTED: i32 = 1;

const REPOSITORY_PIN: &str = include_str!("../../../models/modelspec.pin");

/// The pinned ModelSpec release (domain entity `patchback.model.ModelSpecPin`): its version, the
/// commit it was built from and the SHA-256 of its `x86_64-unknown-linux-gnu` binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSpecPin {
    version: String,
    commit: String,
    sha256: String,
}

/// Why a pin file is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinError {
    /// A line is not `key value`; the 1-based line number.
    Malformed(usize),
    UnknownKey(String),
    DuplicateKey(String),
    MissingKey(&'static str),
    /// The value does not have its key's form: a dotted numeric version, a 40-digit lowercase
    /// hexadecimal commit, a 64-digit lowercase hexadecimal SHA-256.
    InvalidValue(&'static str),
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinError::Malformed(line) => write!(f, "{PIN_FILE}:{line}: expected `key value`"),
            PinError::UnknownKey(key) => write!(f, "{PIN_FILE}: unknown key {key}"),
            PinError::DuplicateKey(key) => write!(f, "{PIN_FILE}: {key} appears twice"),
            PinError::MissingKey(key) => write!(f, "{PIN_FILE}: {key} is missing"),
            PinError::InvalidValue(key) => write!(f, "{PIN_FILE}: {key} has an invalid value"),
        }
    }
}

impl std::error::Error for PinError {}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_version(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn valid(
    value: Option<String>,
    key: &'static str,
    form: impl Fn(&str) -> bool,
) -> Result<String, PinError> {
    let value = value.ok_or(PinError::MissingKey(key))?;
    if form(&value) {
        Ok(value)
    } else {
        Err(PinError::InvalidValue(key))
    }
}

impl ModelSpecPin {
    /// Parse a pin file: one `key value` line each for `version`, `commit` and [`SHA256_KEY`].
    pub fn parse(text: &str) -> Result<ModelSpecPin, PinError> {
        const KEYS: [&str; 3] = ["version", "commit", SHA256_KEY];
        let mut values: [Option<String>; 3] = [None, None, None];
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once(' ')
                .filter(|(_, value)| !value.trim().is_empty())
                .ok_or(PinError::Malformed(index + 1))?;
            let slot = KEYS
                .iter()
                .position(|name| *name == key)
                .ok_or_else(|| PinError::UnknownKey(key.to_owned()))?;
            if values[slot].is_some() {
                return Err(PinError::DuplicateKey(key.to_owned()));
            }
            values[slot] = Some(value.trim().to_owned());
        }
        let [version, commit, sha256] = values;
        let version = valid(version, "version", is_version)?;
        let commit = valid(commit, "commit", |v| is_hex(v, 40))?;
        let sha256 = valid(sha256, SHA256_KEY, |v| is_hex(v, 64))?;
        Ok(ModelSpecPin {
            version,
            commit,
            sha256,
        })
    }

    /// The repository's pin, [`PIN_FILE`], as compiled into this crate.
    pub fn repository() -> Result<ModelSpecPin, PinError> {
        ModelSpecPin::parse(REPOSITORY_PIN)
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// The SHA-256 of the release's `x86_64-unknown-linux-gnu` binary.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Why the adapter could not run the pinned binary. None of these is a verdict on a model.
#[derive(Debug)]
pub enum ModelSpecError {
    /// No binary at the named path, or none on `PATH`; each place looked at.
    NotFound { searched: Vec<String> },
    /// The binary exists and could not be started.
    Spawn { binary: PathBuf, error: io::Error },
    /// `--version` exited unsuccessfully.
    VersionQueryFailed {
        binary: PathBuf,
        code: Option<i32>,
        stderr: String,
    },
    /// `--version` reported something other than `modelspec <pinned>`.
    VersionMismatch {
        binary: PathBuf,
        pinned: String,
        reported: String,
    },
    /// The source cannot be opened as a regular file; nothing was spawned.
    SourceUnreadable { source: PathBuf, error: io::Error },
    /// A signal ended the run, so the binary gave no verdict.
    Signalled { binary: PathBuf },
    /// `check` exited with a status that is neither 0 (accepted) nor 1 (refused), such as a usage
    /// error (2).
    CheckFailed {
        binary: PathBuf,
        code: i32,
        diagnostic: String,
    },
    /// The run outlasted the timeout and was killed.
    TimedOut { binary: PathBuf, timeout: Duration },
    /// Waiting for the process or reading its output failed.
    Io { binary: PathBuf, error: io::Error },
}

impl fmt::Display for ModelSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelSpecError::NotFound { searched } => {
                write!(
                    f,
                    "no modelspec binary found; searched {}",
                    searched.join(", ")
                )
            }
            ModelSpecError::Spawn { binary, error } => {
                write!(f, "cannot run {}: {error}", binary.display())
            }
            ModelSpecError::VersionQueryFailed {
                binary,
                code,
                stderr,
            } => write!(
                f,
                "{} --version failed ({}): {stderr}",
                binary.display(),
                code.map_or("killed by a signal".to_owned(), |c| format!("exit {c}"))
            ),
            ModelSpecError::VersionMismatch {
                binary,
                pinned,
                reported,
            } => write!(
                f,
                "{} reports {reported:?}; the pin is modelspec {pinned}",
                binary.display()
            ),
            ModelSpecError::SourceUnreadable { source, error } => {
                write!(
                    f,
                    "cannot read the model source {}: {error}",
                    source.display()
                )
            }
            ModelSpecError::Signalled { binary } => {
                write!(f, "{} was ended by a signal", binary.display())
            }
            ModelSpecError::CheckFailed {
                binary,
                code,
                diagnostic,
            } => write!(
                f,
                "{} check exited {code}, which is not a verdict: {diagnostic}",
                binary.display()
            ),
            ModelSpecError::TimedOut { binary, timeout } => write!(
                f,
                "{} ran longer than {timeout:?} and was killed",
                binary.display()
            ),
            ModelSpecError::Io { binary, error } => {
                write!(f, "running {}: {error}", binary.display())
            }
        }
    }
}

impl std::error::Error for ModelSpecError {}

fn is_executable_file(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Find the ModelSpec binary from the process environment: see [`locate_with`].
pub fn locate(explicit: Option<&Path>) -> Result<PathBuf, ModelSpecError> {
    let variable = std::env::var_os(MODELSPEC_VARIABLE);
    let path = std::env::var_os("PATH");
    locate_with(explicit, variable.as_deref(), path.as_deref())
}

/// Find the ModelSpec binary: `explicit` if given, else `modelspec_variable` (the value of
/// `MODELSPEC`) if set and not empty, else the first executable `modelspec` in `path_variable`
/// (the value of `PATH`). A named path is used as named, or refused when nothing is there. A
/// returned path without a slash is relative to the working directory; [`ModelSpec::open`] runs it
/// as `./<name>`, never through a `PATH` lookup.
pub fn locate_with(
    explicit: Option<&Path>,
    modelspec_variable: Option<&OsStr>,
    path_variable: Option<&OsStr>,
) -> Result<PathBuf, ModelSpecError> {
    let named = explicit.map(Path::to_path_buf).or_else(|| {
        modelspec_variable
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    });
    if let Some(named) = named {
        return if named.is_file() {
            Ok(named)
        } else {
            Err(ModelSpecError::NotFound {
                searched: vec![named.display().to_string()],
            })
        };
    }
    let mut searched = Vec::new();
    for directory in std::env::split_paths(path_variable.unwrap_or_default()) {
        let candidate = directory.join(EXECUTABLE);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
        searched.push(candidate.display().to_string());
    }
    if searched.is_empty() {
        searched.push(format!("{EXECUTABLE} on an empty PATH"));
    }
    Err(ModelSpecError::NotFound { searched })
}

/// The path to hand to `Command::new`: a single relative component (`modelspec`) gets `./`, since
/// `Command::new` would otherwise look it up on `PATH` and spawn a file other than the one located.
fn program(binary: &Path) -> PathBuf {
    let mut components = binary.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Path::new(".").join(binary),
        _ => binary.to_path_buf(),
    }
}

/// What a finished run produced.
struct Run {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Read a pipe to its end on its own thread and send the bytes, so neither stream can fill and
/// stall the child while the other is read.
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = match pipe {
            Some(mut pipe) => pipe.read_to_end(&mut bytes).map(|_| bytes),
            None => Ok(bytes),
        };
        let _ = sender.send(result);
    });
    receiver
}

const POLL: Duration = Duration::from_millis(10);

/// Run `binary` with `arguments` and wait at most `timeout` for it to exit and close its output.
/// Past the deadline the child is killed and waited for. A `timeout` too long to add to the
/// current instant (such as `Duration::MAX`) sets no deadline: the child is waited for.
fn run(binary: &Path, arguments: &[&OsStr], timeout: Duration) -> Result<Run, ModelSpecError> {
    let failed = |error| ModelSpecError::Io {
        binary: binary.to_path_buf(),
        error,
    };
    let timed_out = || ModelSpecError::TimedOut {
        binary: binary.to_path_buf(),
        timeout,
    };
    let deadline = Instant::now().checked_add(timeout);
    let mut child = Command::new(program(binary))
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ModelSpecError::Spawn {
            binary: binary.to_path_buf(),
            error,
        })?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = loop {
        let polled = child.try_wait();
        let now = Instant::now();
        match polled {
            Ok(Some(status)) => break status,
            Ok(None) if deadline.is_none_or(|deadline| now < deadline) => {
                thread::sleep(deadline.map_or(POLL, |deadline| POLL.min(deadline - now)))
            }
            outcome => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match outcome {
                    Err(error) => failed(error),
                    _ => timed_out(),
                });
            }
        }
    };
    // A process the child left behind can hold the pipes open after the child exits; the
    // deadline bounds that wait too.
    let collect = |receiver: mpsc::Receiver<io::Result<Vec<u8>>>| {
        let received = match deadline {
            Some(deadline) => receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| timed_out())?,
            None => receiver.recv().unwrap_or_else(|_| {
                Err(io::Error::other("the output reader ended without a result"))
            }),
        };
        received.map_err(failed)
    };
    let stdout = collect(stdout)?;
    let stderr = collect(stderr)?;
    Ok(Run {
        status,
        stdout,
        stderr,
    })
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

/// A ModelSpec binary that reported the version of the pin it was opened with.
#[derive(Clone, Debug)]
pub struct ModelSpec {
    binary: PathBuf,
    pin: ModelSpecPin,
    timeout: Duration,
}

/// What `modelspec check` decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Exit status 0.
    Checked,
    /// Exit status 1, the binary's refusal; `code` is that status. `diagnostic` is the binary's
    /// stderr, or its stdout when stderr is empty, trimmed.
    Rejected { code: i32, diagnostic: String },
}

impl ModelSpec {
    /// [`ModelSpec::open_with_timeout`] with [`DEFAULT_TIMEOUT`].
    pub fn open(binary: &Path, pin: &ModelSpecPin) -> Result<ModelSpec, ModelSpecError> {
        ModelSpec::open_with_timeout(binary, pin, DEFAULT_TIMEOUT)
    }

    /// Run `binary --version` and accept the binary only if it reports `modelspec <version>` for
    /// the `pin` given here. That is whichever pin the caller passes:
    /// [`ModelSpecPin::repository`] is the repository's pin, and refusing a source checked under
    /// another pin when it is trained belongs to the training run (domain error `PinMismatch`).
    /// `timeout` bounds this run and every later [`ModelSpec::check`].
    pub fn open_with_timeout(
        binary: &Path,
        pin: &ModelSpecPin,
        timeout: Duration,
    ) -> Result<ModelSpec, ModelSpecError> {
        let output = run(binary, &[OsStr::new("--version")], timeout)?;
        if !output.status.success() {
            return Err(ModelSpecError::VersionQueryFailed {
                binary: binary.to_path_buf(),
                code: output.status.code(),
                stderr: text(&output.stderr),
            });
        }
        let reported = text(&output.stdout);
        if reported != format!("{EXECUTABLE} {}", pin.version()) {
            return Err(ModelSpecError::VersionMismatch {
                binary: binary.to_path_buf(),
                pinned: pin.version().to_owned(),
                reported,
            });
        }
        Ok(ModelSpec {
            binary: binary.to_path_buf(),
            pin: pin.clone(),
            timeout,
        })
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// The pin whose version the binary reported.
    pub fn pin(&self) -> &ModelSpecPin {
        &self.pin
    }

    /// The version the binary reported.
    pub fn version(&self) -> &str {
        self.pin.version()
    }

    /// The bound on each run of the binary.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Run `modelspec check <source>` and report the binary's verdict as it gave it: exit 0 is
    /// [`CheckOutcome::Checked`], exit 1 [`CheckOutcome::Rejected`], and any other exit, a signal
    /// or a run past the timeout is an error. This does not look at the source first; the binary
    /// also exits 1 for a path it cannot read, which is why [`ModelSource::check`] refuses such a
    /// source before it gets here.
    pub fn check(&self, source: &Path) -> Result<CheckOutcome, ModelSpecError> {
        let output = run(
            &self.binary,
            &[OsStr::new("check"), source.as_os_str()],
            self.timeout,
        )?;
        let stderr = text(&output.stderr);
        let diagnostic = if stderr.is_empty() {
            text(&output.stdout)
        } else {
            stderr
        };
        match output.status.code() {
            Some(0) => Ok(CheckOutcome::Checked),
            Some(REJECTED) => Ok(CheckOutcome::Rejected {
                code: REJECTED,
                diagnostic,
            }),
            Some(code) => Err(ModelSpecError::CheckFailed {
                binary: self.binary.clone(),
                code,
                diagnostic,
            }),
            None => Err(ModelSpecError::Signalled {
                binary: self.binary.clone(),
            }),
        }
    }
}

/// Open `source` as a regular file, so a path the binary could not read is refused before it runs.
fn readable(source: &Path) -> Result<(), ModelSpecError> {
    let refused = |error| ModelSpecError::SourceUnreadable {
        source: source.to_path_buf(),
        error,
    };
    let metadata = File::open(source)
        .and_then(|file| file.metadata())
        .map_err(refused)?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(refused(io::Error::other("not a regular file")))
    }
}

/// The lifecycle of a model source (domain `patchback.model.ModelSource.State`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelSourceState {
    Unchecked,
    Checked,
    Rejected,
}

/// A `.mspec` source (domain entity `patchback.model.ModelSource`), such as
/// `models/patch_regressor.mspec` (§ 8.1).
#[derive(Clone, Debug)]
pub struct ModelSource {
    path: PathBuf,
    model_name: String,
    batch_size: usize,
    model_spec_pin: ModelSpecPin,
    state: ModelSourceState,
    diagnostic: Option<String>,
}

/// Why [`ModelSource::check`] recorded nothing. The source stays as it was.
#[derive(Debug)]
pub enum CheckError {
    /// The source already carries a check result (domain error `ModelSourceAlreadyChecked`).
    ModelSourceAlreadyChecked { state: ModelSourceState },
    /// The binary was opened under another pin than the one the source was registered with;
    /// the two versions. Nothing was spawned.
    OtherPin { registered: String, binary: String },
    /// The source could not be read, or the binary gave no verdict on it.
    ModelSpec(ModelSpecError),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::ModelSourceAlreadyChecked { state } => {
                write!(f, "the model source is already {state:?}")
            }
            CheckError::OtherPin { registered, binary } => write!(
                f,
                "the model source is registered under modelspec {registered}; the binary is modelspec {binary}"
            ),
            CheckError::ModelSpec(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CheckError {}

/// A training run asked for a source `modelspec check` did not accept (domain error
/// `ModelNotChecked`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelNotChecked {
    pub state: ModelSourceState,
}

impl fmt::Display for ModelNotChecked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the model source is {:?}; only a Checked source is trained",
            self.state
        )
    }
}

impl std::error::Error for ModelNotChecked {}

/// A source the pinned binary accepted. Only [`ModelSource::trainable`] makes one, so a training
/// run that takes a `CheckedModel` cannot be handed an unchecked or rejected source.
#[derive(Clone, Copy, Debug)]
pub struct CheckedModel<'a> {
    source: &'a ModelSource,
}

impl CheckedModel<'_> {
    pub fn path(&self) -> &Path {
        &self.source.path
    }

    pub fn model_name(&self) -> &str {
        &self.source.model_name
    }

    pub fn batch_size(&self) -> usize {
        self.source.batch_size
    }

    /// The pin whose binary checked the source.
    pub fn model_spec_pin(&self) -> &ModelSpecPin {
        &self.source.model_spec_pin
    }

    /// The version of that pin.
    pub fn model_spec_version(&self) -> &str {
        self.source.model_spec_pin.version()
    }
}

/// The source a [`CheckedModel`] was made from, so [`TrainingRun::bind`] takes either.
impl AsRef<ModelSource> for CheckedModel<'_> {
    fn as_ref(&self) -> &ModelSource {
        self.source
    }
}

impl AsRef<ModelSource> for ModelSource {
    fn as_ref(&self) -> &ModelSource {
        self
    }
}

impl ModelSource {
    /// Register a source, `Unchecked`, under the pin whose binary will check it (domain command
    /// `RegisterModelSource`, field `model_spec_pin_id`).
    pub fn register(
        path: impl Into<PathBuf>,
        model_name: &str,
        batch_size: usize,
        model_spec_pin: &ModelSpecPin,
    ) -> ModelSource {
        ModelSource {
            path: path.into(),
            model_name: model_name.to_owned(),
            batch_size,
            model_spec_pin: model_spec_pin.clone(),
            state: ModelSourceState::Unchecked,
            diagnostic: None,
        }
    }

    /// The pin the source was registered under.
    pub fn model_spec_pin(&self) -> &ModelSpecPin {
        &self.model_spec_pin
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn state(&self) -> ModelSourceState {
        self.state
    }

    /// The binary's diagnostic when the source is `Rejected`.
    pub fn diagnostic(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }

    /// Check the source with the binary of its pin and record the verdict once (domain command
    /// `CheckModelSource`). Before anything is spawned, a binary of another pin and a source that
    /// cannot be opened as a regular file are refused; those, and every run that ends without a
    /// verdict ([`ModelSpec::check`]), leave the source `Unchecked`.
    pub fn check(&mut self, modelspec: &ModelSpec) -> Result<ModelSourceState, CheckError> {
        if self.state != ModelSourceState::Unchecked {
            return Err(CheckError::ModelSourceAlreadyChecked { state: self.state });
        }
        if modelspec.pin() != &self.model_spec_pin {
            return Err(CheckError::OtherPin {
                registered: self.model_spec_pin.version().to_owned(),
                binary: modelspec.version().to_owned(),
            });
        }
        readable(&self.path).map_err(CheckError::ModelSpec)?;
        match modelspec.check(&self.path).map_err(CheckError::ModelSpec)? {
            CheckOutcome::Checked => self.state = ModelSourceState::Checked,
            CheckOutcome::Rejected { diagnostic, .. } => {
                self.state = ModelSourceState::Rejected;
                self.diagnostic = Some(diagnostic);
            }
        }
        Ok(self.state)
    }

    /// The guard a training run passes: the source as a [`CheckedModel`], or [`ModelNotChecked`].
    pub fn trainable(&self) -> Result<CheckedModel<'_>, ModelNotChecked> {
        match self.state {
            ModelSourceState::Checked => Ok(CheckedModel { source: self }),
            state => Err(ModelNotChecked { state }),
        }
    }
}
