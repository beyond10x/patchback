//! The ModelSpec CLI adapter and the native CPU weight runner (DESIGN.md § 4.1, § 8).
//!
//! The adapter runs the pinned ModelSpec binary as a subprocess (§ 8.2 step 1) and nothing else:
//!
//! 1. [`ModelSpecPin`] is the pin, read from `models/modelspec.pin` ([`PIN_FILE`]), the one file
//!    CI also reads to download and verify the binary.
//! 2. [`locate`] finds a binary: an explicit path, else the `MODELSPEC` variable, else a
//!    `modelspec` executable on `PATH`. A path that is named but missing is
//!    [`ModelSpecError::NotFound`]; the search does not fall through to the next source.
//! 3. [`ModelSpec::open`] runs `<binary> --version` and refuses a binary that does not report
//!    `modelspec <pinned version>` ([`ModelSpecError::VersionMismatch`]), so no other release runs.
//! 4. [`ModelSource::check`] runs `<binary> check <source>` and records the source `Checked` on
//!    exit status 0 and `Rejected`, with the binary's diagnostic, otherwise (domain
//!    `patchback.model`, command `CheckModelSource`). A source is checked once.
//! 5. [`ModelSource::trainable`] is the guard a training run takes: only a `Checked` source yields
//!    a [`CheckedModel`], which nothing else can construct.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The pin file, relative to the repository root.
pub const PIN_FILE: &str = "models/modelspec.pin";

/// The pin's key for the checksum of the release asset CI installs.
pub const SHA256_KEY: &str = "sha256-x86_64-unknown-linux-gnu";

/// The variable that names the ModelSpec binary when no explicit path is given.
pub const MODELSPEC_VARIABLE: &str = "MODELSPEC";

/// The executable name searched for on `PATH`.
pub const EXECUTABLE: &str = "modelspec";

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
/// (the value of `PATH`). A named path is used as named, or refused when nothing is there.
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

fn run(binary: &Path, arguments: &[&OsStr]) -> Result<Output, ModelSpecError> {
    Command::new(binary)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ModelSpecError::Spawn {
            binary: binary.to_path_buf(),
            error,
        })
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

/// A ModelSpec binary that reported the pinned version.
#[derive(Clone, Debug)]
pub struct ModelSpec {
    binary: PathBuf,
    version: String,
}

/// What `modelspec check` decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Exit status 0.
    Checked,
    /// Any other exit; `code` is `None` when a signal ended the process. `diagnostic` is the
    /// binary's stderr, or its stdout when stderr is empty, trimmed.
    Rejected {
        code: Option<i32>,
        diagnostic: String,
    },
}

impl ModelSpec {
    /// Run `binary --version` and accept the binary only if it reports `modelspec <pinned>`.
    pub fn open(binary: &Path, pin: &ModelSpecPin) -> Result<ModelSpec, ModelSpecError> {
        let output = run(binary, &[OsStr::new("--version")])?;
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
            version: pin.version().to_owned(),
        })
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// The pinned version the binary reported.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Run `modelspec check <source>`.
    pub fn check(&self, source: &Path) -> Result<CheckOutcome, ModelSpecError> {
        let output = run(&self.binary, &[OsStr::new("check"), source.as_os_str()])?;
        if output.status.success() {
            return Ok(CheckOutcome::Checked);
        }
        let stderr = text(&output.stderr);
        let diagnostic = if stderr.is_empty() {
            text(&output.stdout)
        } else {
            stderr
        };
        Ok(CheckOutcome::Rejected {
            code: output.status.code(),
            diagnostic,
        })
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
    state: ModelSourceState,
    model_spec_version: Option<String>,
    diagnostic: Option<String>,
}

/// Why [`ModelSource::check`] recorded nothing.
#[derive(Debug)]
pub enum CheckError {
    /// The source already carries a check result (domain error `ModelSourceAlreadyChecked`).
    ModelSourceAlreadyChecked { state: ModelSourceState },
    /// The binary could not be run; the source stays `Unchecked`.
    ModelSpec(ModelSpecError),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::ModelSourceAlreadyChecked { state } => {
                write!(f, "the model source is already {state:?}")
            }
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
    model_spec_version: &'a str,
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

    /// The pinned ModelSpec version whose binary checked the source.
    pub fn model_spec_version(&self) -> &str {
        self.model_spec_version
    }
}

impl ModelSource {
    /// Register a source, `Unchecked` (domain command `RegisterModelSource`).
    pub fn register(path: impl Into<PathBuf>, model_name: &str, batch_size: usize) -> ModelSource {
        ModelSource {
            path: path.into(),
            model_name: model_name.to_owned(),
            batch_size,
            state: ModelSourceState::Unchecked,
            model_spec_version: None,
            diagnostic: None,
        }
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

    /// Check the source with the pinned binary and record the result once (domain command
    /// `CheckModelSource`).
    pub fn check(&mut self, modelspec: &ModelSpec) -> Result<ModelSourceState, CheckError> {
        if self.state != ModelSourceState::Unchecked {
            return Err(CheckError::ModelSourceAlreadyChecked { state: self.state });
        }
        match modelspec.check(&self.path).map_err(CheckError::ModelSpec)? {
            CheckOutcome::Checked => self.state = ModelSourceState::Checked,
            CheckOutcome::Rejected { diagnostic, .. } => {
                self.state = ModelSourceState::Rejected;
                self.diagnostic = Some(diagnostic);
            }
        }
        self.model_spec_version = Some(modelspec.version().to_owned());
        Ok(self.state)
    }

    /// The guard a training run passes: the source as a [`CheckedModel`], or [`ModelNotChecked`].
    pub fn trainable(&self) -> Result<CheckedModel<'_>, ModelNotChecked> {
        match (self.state, self.model_spec_version.as_deref()) {
            (ModelSourceState::Checked, Some(model_spec_version)) => Ok(CheckedModel {
                source: self,
                model_spec_version,
            }),
            (state, _) => Err(ModelNotChecked { state }),
        }
    }
}
