//! The weight export of a completed training run (domain command `patchback.model.ExportWeights`,
//! entity `WeightArtifact`; DESIGN.md § 8.2 steps 2–3, ADR-006) and its reader.
//!
//! [`WeightArtifact::export`] reads the stdout a `Completed` [`TrainingRun`] holds and answers in
//! this order (Da9-7):
//!
//! 1. the run's state: anything but `Completed` is `TrainingNotCompleted` (Da9-8; the run's
//!    `output_error` is not consulted);
//! 2. the grammar of the whole stdout (Da9-5): every line is one of
//!    `init @<model>.<name>: Tensor<f32, [<r>, <c>]> = [<v>, <v>, …]`,
//!    `step <k> loss <v>` (step numbers increasing), `eval loss=<v> accuracy=<v>` (at most one)
//!    and `param @<model>.<name>: Tensor<f32, [<r>, <c>]> = [<v>, <v>, …]` (`<model>` and
//!    `<name>` ASCII letters, digits and `_`; Da9-19), with exactly two `param` lines; anything
//!    else, an empty line included, is `UnrecognizedTrainingOutput`.
//!    A value is read with `f32::from_str` (Da9-4); a token it does not take is unrecognized
//!    output. At most one value past the expected count is kept, every token is read (Da9-12);
//! 3. the names: the first `param` line is w1 and its whole name must be [`W1_NAME`], the
//!    second [`W2_NAME`] (Da9-6, Da9-19), else `UnrecognizedParameter` with both names as
//!    printed (`@Other.w1` included); an `init` line's name is not checked;
//! 4. the declared shapes against [`W1_SHAPE`] and [`W2_SHAPE`] (`WrongWeightShape`);
//! 5. the value counts against [`W1_ELEMENTS`] and [`W2_ELEMENTS`] (`WrongElementCount`);
//! 6. every value finite (`NonFiniteWeight`);
//! 7. the write: a new file (an existing one is [`ExportError::Io`]), removed again if writing
//!    it fails (Da9-11). A refusal writes nothing.
//!
//! The file, format [`FORMAT_VERSION`] (Da9-3), is a header of seven `\n`-terminated lines in
//! this order, an empty line, then the payload:
//!
//! ```text
//! weights/1
//! model PatchRegressor
//! modelspec <version> <commit>
//! features <feature contract>
//! w1 32 64
//! w2 64 8
//! sha256 <64 lowercase hexadecimal digits>
//!
//! <w1's 2,048 values, then w2's 512, each a little-endian f32, in the order printed>
//! ```
//!
//! The `modelspec` line is the run's pin (Da9-14), the `features` line the contract the caller
//! gives, unchecked (Da9-9), and the checksum the SHA-256 of the 10,240 payload bytes (Da9-2).
//! [`WeightArtifact::read`] reads the header within [`MAX_HEADER_BYTES`] and checks each line in
//! order, then the payload's length, its checksum and that every value is finite; any failure is
//! [`ReadError::Corrupt`] (`CorruptWeightArtifact`, which `Predict` declares; Da9-10).

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use patchback_core::digest;

use crate::{ModelSpecPin, TrainingRun, TrainingRunState, is_hex, is_version};

/// The first header line of the artifact: its format and version.
pub const FORMAT_VERSION: &str = "weights/1";
/// The model the artifact's parameters belong to, as the `param` lines print its name (Da9-16).
pub const MODEL_NAME: &str = "PatchRegressor";
/// The first parameter, as the pinned ModelSpec prints its name.
pub const W1_NAME: &str = "@PatchRegressor.w1";
/// The second parameter, as the pinned ModelSpec prints its name.
pub const W2_NAME: &str = "@PatchRegressor.w2";
/// `[rows, columns]` of w1 (DESIGN.md § 8.1).
pub const W1_SHAPE: [usize; 2] = [32, 64];
/// `[rows, columns]` of w2 (DESIGN.md § 8.1).
pub const W2_SHAPE: [usize; 2] = [64, 8];
/// The values of w1.
pub const W1_ELEMENTS: usize = W1_SHAPE[0] * W1_SHAPE[1];
/// The values of w2.
pub const W2_ELEMENTS: usize = W2_SHAPE[0] * W2_SHAPE[1];
/// The payload: w1's values then w2's, each a little-endian `f32`.
pub const PAYLOAD_BYTES: usize = (W1_ELEMENTS + W2_ELEMENTS) * 4;
/// The most bytes the header, its closing empty line included, may take. Provisional (Da9-11).
pub const MAX_HEADER_BYTES: usize = 1024;
/// The file name recommended for an artifact; the path is the caller's. Provisional (Da9-3).
pub const RECOMMENDED_FILE_NAME: &str = "regressor.weights";

/// Why `ExportWeights` wrote nothing. Every variant but the last two names a declared error of
/// `spec/domains/model.yaml` ([`ExportError::domain_error`]).
#[derive(Debug)]
pub enum ExportError {
    /// The run is not `Completed`.
    TrainingNotCompleted { state: TrainingRunState },
    /// A line of the run's stdout is outside the documented grammar (`line`, 1-based), or the
    /// stdout does not hold two `param` lines (`line` `None`).
    UnrecognizedTrainingOutput {
        line: Option<usize>,
        reason: &'static str,
    },
    /// The first `param` line is not w1 or the second not w2; the names as printed.
    UnrecognizedParameter { w1_name: String, w2_name: String },
    /// The declared shapes are not `[32, 64]` and `[64, 8]`.
    WrongWeightShape {
        w1_rows: usize,
        w1_columns: usize,
        w2_rows: usize,
        w2_columns: usize,
    },
    /// The lines hold another number of values than 2,048 and 512.
    WrongElementCount {
        w1_elements: usize,
        w2_elements: usize,
    },
    /// A value is NaN or infinite: the parameter, the value's 0-based index and the value.
    NonFiniteWeight {
        name: &'static str,
        index: usize,
        value: f32,
    },
    /// Not a domain error: the header cannot hold the given values (a feature contract that is
    /// not one printable ASCII token, or a header past [`MAX_HEADER_BYTES`]); nothing was written.
    Unwritable { reason: String },
    /// Not a domain error: the file exists, or could not be created or written (a file this
    /// export created is removed again).
    Io { path: PathBuf, error: io::Error },
}

impl ExportError {
    /// The declared errors [`WeightArtifact::export`] emits, in the order `ExportWeights` declares
    /// them. `UnknownTrainingRun` has no counterpart: the run is a value (Da8-21).
    pub const DOMAIN_ERRORS: &'static [&'static str] = &[
        "patchback.model.WrongWeightShape",
        "patchback.model.WrongElementCount",
        "patchback.model.TrainingNotCompleted",
        "patchback.model.UnrecognizedParameter",
        "patchback.model.UnrecognizedTrainingOutput",
        "patchback.model.NonFiniteWeight",
    ];

    /// The declared error this refusal is, or `None` for a failure that is not the model's.
    pub fn domain_error(&self) -> Option<&'static str> {
        Some(match self {
            ExportError::WrongWeightShape { .. } => "patchback.model.WrongWeightShape",
            ExportError::WrongElementCount { .. } => "patchback.model.WrongElementCount",
            ExportError::TrainingNotCompleted { .. } => "patchback.model.TrainingNotCompleted",
            ExportError::UnrecognizedParameter { .. } => "patchback.model.UnrecognizedParameter",
            ExportError::UnrecognizedTrainingOutput { .. } => {
                "patchback.model.UnrecognizedTrainingOutput"
            }
            ExportError::NonFiniteWeight { .. } => "patchback.model.NonFiniteWeight",
            ExportError::Unwritable { .. } | ExportError::Io { .. } => return None,
        })
    }

    /// One refusal of each declared error, in [`ExportError::DOMAIN_ERRORS`] order.
    pub fn examples() -> Vec<ExportError> {
        vec![
            ExportError::WrongWeightShape {
                w1_rows: 64,
                w1_columns: 32,
                w2_rows: 64,
                w2_columns: 8,
            },
            ExportError::WrongElementCount {
                w1_elements: 2048,
                w2_elements: 511,
            },
            ExportError::TrainingNotCompleted {
                state: TrainingRunState::Running,
            },
            ExportError::UnrecognizedParameter {
                w1_name: "@PatchRegressor.b1".to_owned(),
                w2_name: W2_NAME.to_owned(),
            },
            ExportError::UnrecognizedTrainingOutput {
                line: Some(5),
                reason: "not an `init`, `step`, `param` or `eval` line",
            },
            ExportError::NonFiniteWeight {
                name: W2_NAME,
                index: 7,
                value: f32::NAN,
            },
        ]
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::TrainingNotCompleted { state } => write!(
                f,
                "the training run is {}; weights are exported only from a Completed run",
                state.name()
            ),
            ExportError::UnrecognizedTrainingOutput {
                line: Some(line),
                reason,
            } => write!(
                f,
                "training output line {line} is not documented output of the pinned ModelSpec: {reason}"
            ),
            ExportError::UnrecognizedTrainingOutput { line: None, reason } => write!(
                f,
                "the training output is not documented output of the pinned ModelSpec: {reason}"
            ),
            ExportError::UnrecognizedParameter { w1_name, w2_name } => write!(
                f,
                "the param lines name {w1_name} and {w2_name}, not {W1_NAME} and {W2_NAME}"
            ),
            ExportError::WrongWeightShape {
                w1_rows,
                w1_columns,
                w2_rows,
                w2_columns,
            } => write!(
                f,
                "w1 is [{w1_rows}, {w1_columns}] and w2 [{w2_rows}, {w2_columns}], not [{}, {}] and [{}, {}]",
                W1_SHAPE[0], W1_SHAPE[1], W2_SHAPE[0], W2_SHAPE[1]
            ),
            ExportError::WrongElementCount {
                w1_elements,
                w2_elements,
            } => write!(
                f,
                "w1 holds {w1_elements} values and w2 {w2_elements}, not {W1_ELEMENTS} and {W2_ELEMENTS}"
            ),
            ExportError::NonFiniteWeight { name, index, value } => {
                write!(f, "{name} value {index} is {value}, not finite")
            }
            ExportError::Unwritable { reason } => {
                write!(f, "the weight artifact cannot be written: {reason}")
            }
            ExportError::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for ExportError {}

/// Why an artifact was not read.
#[derive(Debug)]
pub enum ReadError {
    /// The bytes are not a `weights/1` artifact whose checksum verifies (domain error
    /// `CorruptWeightArtifact`).
    Corrupt { reason: String },
    /// Not a domain error: the file could not be opened or read.
    Io { path: PathBuf, error: io::Error },
}

impl ReadError {
    /// The declared error [`WeightArtifact::read`] emits; `Predict` declares it (Da9-10).
    pub const DOMAIN_ERRORS: &'static [&'static str] = &["patchback.model.CorruptWeightArtifact"];

    /// The declared error this refusal is, or `None` for a failure that is not the model's.
    pub fn domain_error(&self) -> Option<&'static str> {
        match self {
            ReadError::Corrupt { .. } => Some("patchback.model.CorruptWeightArtifact"),
            ReadError::Io { .. } => None,
        }
    }

    /// One refusal of each declared error, in [`ReadError::DOMAIN_ERRORS`] order.
    pub fn examples() -> Vec<ReadError> {
        vec![ReadError::Corrupt {
            reason: "the payload's SHA-256 is not the header's".to_owned(),
        }]
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Corrupt { reason } => write!(f, "corrupt weight artifact: {reason}"),
            ReadError::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for ReadError {}

/// A checked weight artifact (domain entity `patchback.model.WeightArtifact`): made only by
/// [`WeightArtifact::export`] and [`WeightArtifact::read`], so w1 holds 2,048 finite values, w2
/// 512, and the checksum is their payload's.
#[derive(Clone, Debug, PartialEq)]
pub struct WeightArtifact {
    w1: Vec<f32>,
    w2: Vec<f32>,
    model: String,
    modelspec_version: String,
    modelspec_commit: String,
    feature_contract: String,
    checksum: String,
}

/// One `init` or `param` line as printed: its name, declared shape, number of values and the
/// values kept.
struct Param {
    name: String,
    rows: usize,
    columns: usize,
    count: usize,
    values: Vec<f32>,
}

/// A decimal number as ModelSpec prints one: ASCII digits, no sign, no leading zero.
fn decimal(text: &str) -> Option<u64> {
    let canonical = !text.is_empty()
        && text.bytes().all(|b| b.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    canonical.then(|| text.parse().ok()).flatten()
}

/// The rest of an `init` or `param` line after its first word: `@<model>.<name>:
/// Tensor<f32, [<r>, <c>]> = [<values>]`. Every value token is read; the first `keep` are kept.
fn tensor(rest: &str, keep: usize) -> Result<Param, &'static str> {
    let (name, rest) = rest
        .split_once(": ")
        .ok_or("no `: ` after the parameter name")?;
    let is_identifier = |text: &str| {
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    };
    let well_formed = name
        .strip_prefix('@')
        .and_then(|name| name.split_once('.'))
        .is_some_and(|(model, parameter)| is_identifier(model) && is_identifier(parameter));
    if !well_formed {
        return Err("the parameter name is not `@<model>.<name>`");
    }
    let (shape, values) = rest
        .strip_prefix("Tensor<f32, [")
        .and_then(|rest| rest.split_once("]> = ["))
        .ok_or("the type is not `Tensor<f32, [<rows>, <columns>]> = [`")?;
    let dimension = |text: &str| decimal(text).and_then(|value| usize::try_from(value).ok());
    let (rows, columns) = shape
        .split_once(", ")
        .and_then(|(rows, columns)| Some((dimension(rows)?, dimension(columns)?)))
        .ok_or("the shape is not `[<rows>, <columns>]`")?;
    let values = values
        .strip_suffix(']')
        .ok_or("the line does not end with the value list's `]`")?;
    let mut kept = Vec::with_capacity(keep.min(W1_ELEMENTS + 1));
    let mut count = 0;
    if !values.is_empty() {
        for token in values.split(", ") {
            let value: f32 = token
                .parse()
                .map_err(|_| "a value is not an f32 as `f32::from_str` reads one")?;
            if kept.len() < keep {
                kept.push(value);
            }
            count += 1;
        }
    }
    Ok(Param {
        name: name.to_owned(),
        rows,
        columns,
        count,
        values: kept,
    })
}

/// The rest of a `step` line: `<k> loss <v>`; returns `k`.
fn step(rest: &str) -> Option<u64> {
    let mut words = rest.split(' ');
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some(step), Some("loss"), Some(loss), None) => {
            loss.parse::<f64>().ok()?;
            decimal(step)
        }
        _ => None,
    }
}

/// The rest of an `eval` line: `loss=<v> accuracy=<v>`.
fn eval(rest: &str) -> bool {
    let number = |word: Option<&str>, key: &str| {
        word.and_then(|word| word.strip_prefix(key))
            .is_some_and(|value| value.parse::<f64>().is_ok())
    };
    let mut words = rest.split(' ');
    number(words.next(), "loss=") && number(words.next(), "accuracy=") && words.next().is_none()
}

/// The grammar of the whole stdout (Da9-5): the two `param` lines, w1's then w2's as printed.
fn parse_output(stdout: &str) -> Result<[Param; 2], ExportError> {
    let mut params: Vec<Param> = Vec::with_capacity(2);
    let mut last_step: Option<u64> = None;
    let mut evaluated = false;
    for (index, line) in stdout.split_terminator('\n').enumerate() {
        let refuse = move |reason: &'static str| ExportError::UnrecognizedTrainingOutput {
            line: Some(index + 1),
            reason,
        };
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "init" => {
                tensor(rest, 0).map_err(refuse)?;
            }
            "param" => {
                let keep = match params.len() {
                    0 => W1_ELEMENTS + 1,
                    1 => W2_ELEMENTS + 1,
                    _ => return Err(refuse("a third `param` line")),
                };
                params.push(tensor(rest, keep).map_err(refuse)?);
            }
            "step" => {
                let number = step(rest).ok_or_else(|| refuse("not `step <k> loss <v>`"))?;
                if last_step.is_some_and(|last| number <= last) {
                    return Err(refuse("a step number not greater than the one before"));
                }
                last_step = Some(number);
            }
            "eval" => {
                if evaluated {
                    return Err(refuse("a second `eval` line"));
                }
                if !eval(rest) {
                    return Err(refuse("not `eval loss=<v> accuracy=<v>`"));
                }
                evaluated = true;
            }
            _ => return Err(refuse("not an `init`, `step`, `param` or `eval` line")),
        }
    }
    let found = params.len();
    <[Param; 2]>::try_from(params).map_err(|_| ExportError::UnrecognizedTrainingOutput {
        line: None,
        reason: if found == 0 {
            "it holds no `param` line"
        } else {
            "it holds one `param` line, not two"
        },
    })
}

/// The checks of Da9-7 after the run's state: the grammar, the names, the shapes, the counts and
/// finiteness; w1's and w2's values as printed.
fn weights_from_output(stdout: &str) -> Result<(Vec<f32>, Vec<f32>), ExportError> {
    let [w1, w2] = parse_output(stdout)?;
    if w1.name != W1_NAME || w2.name != W2_NAME {
        return Err(ExportError::UnrecognizedParameter {
            w1_name: w1.name,
            w2_name: w2.name,
        });
    }
    if [w1.rows, w1.columns] != W1_SHAPE || [w2.rows, w2.columns] != W2_SHAPE {
        return Err(ExportError::WrongWeightShape {
            w1_rows: w1.rows,
            w1_columns: w1.columns,
            w2_rows: w2.rows,
            w2_columns: w2.columns,
        });
    }
    if w1.count != W1_ELEMENTS || w2.count != W2_ELEMENTS {
        return Err(ExportError::WrongElementCount {
            w1_elements: w1.count,
            w2_elements: w2.count,
        });
    }
    for (name, values) in [(W1_NAME, &w1.values), (W2_NAME, &w2.values)] {
        if let Some((index, value)) = values
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(ExportError::NonFiniteWeight {
                name,
                index,
                value: *value,
            });
        }
    }
    Ok((w1.values, w2.values))
}

/// A header value: one or more printable ASCII bytes, no space.
fn is_token(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_graphic())
}

fn payload(w1: &[f32], w2: &[f32]) -> Vec<u8> {
    w1.iter()
        .chain(w2)
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn header(artifact: &WeightArtifact) -> String {
    format!(
        "{FORMAT_VERSION}\nmodel {}\nmodelspec {} {}\nfeatures {}\nw1 {} {}\nw2 {} {}\nsha256 {}\n\n",
        artifact.model,
        artifact.modelspec_version,
        artifact.modelspec_commit,
        artifact.feature_contract,
        W1_SHAPE[0],
        W1_SHAPE[1],
        W2_SHAPE[0],
        W2_SHAPE[1],
        artifact.checksum
    )
}

/// The file's bytes: the header, then the payload.
fn encode(artifact: &WeightArtifact) -> Vec<u8> {
    let mut bytes = header(artifact).into_bytes();
    bytes.extend(payload(&artifact.w1, &artifact.w2));
    bytes
}

fn corrupt(reason: impl Into<String>) -> ReadError {
    ReadError::Corrupt {
        reason: reason.into(),
    }
}

/// The artifact `bytes` hold: the header checked line by line within [`MAX_HEADER_BYTES`], then
/// the payload's length, its checksum and every value finite.
fn decode(bytes: &[u8]) -> Result<WeightArtifact, ReadError> {
    let first = format!("{FORMAT_VERSION}\n");
    if !bytes.starts_with(first.as_bytes()) {
        return Err(corrupt(format!(
            "the first line is not `{FORMAT_VERSION}` (an unknown format or version)"
        )));
    }
    let window = &bytes[..bytes.len().min(MAX_HEADER_BYTES)];
    let end = window
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .map(|at| at + 2)
        .ok_or_else(|| {
            corrupt(format!(
                "no header closed by an empty line within {MAX_HEADER_BYTES} bytes"
            ))
        })?;
    let text = std::str::from_utf8(&bytes[first.len()..end - 1])
        .map_err(|_| corrupt("the header is not UTF-8"))?;
    // The header lines after the first, in their fixed order.
    let lines: Vec<&str> = text.split_terminator('\n').collect();
    fn field<'a>(lines: &[&'a str], index: usize, key: &str) -> Result<&'a str, ReadError> {
        lines
            .get(index)
            .and_then(|line| line.strip_prefix(key))
            .and_then(|rest| rest.strip_prefix(' '))
            .ok_or_else(|| corrupt(format!("header line {} is not a `{key}` line", index + 2)))
    }
    let fixed = |index: usize, key: &str, value: String| {
        if field(&lines, index, key)? == value {
            Ok(())
        } else {
            Err(corrupt(format!("the `{key}` line is not `{key} {value}`")))
        }
    };
    fixed(0, "model", MODEL_NAME.to_owned())?;
    let (version, commit) = field(&lines, 1, "modelspec")?
        .split_once(' ')
        .filter(|(version, commit)| is_version(version) && is_hex(commit, 40))
        .ok_or_else(|| corrupt("the `modelspec` line is not `modelspec <version> <commit>`"))?;
    let feature_contract = field(&lines, 2, "features")?;
    if !is_token(feature_contract) {
        return Err(corrupt("the `features` line does not hold one token"));
    }
    fixed(3, "w1", format!("{} {}", W1_SHAPE[0], W1_SHAPE[1]))?;
    fixed(4, "w2", format!("{} {}", W2_SHAPE[0], W2_SHAPE[1]))?;
    let checksum = field(&lines, 5, "sha256")?;
    if !is_hex(checksum, 64) {
        return Err(corrupt(
            "the `sha256` line does not hold 64 lowercase hexadecimal digits",
        ));
    }
    if lines.len() != 6 {
        return Err(corrupt("the header has a line after `sha256`"));
    }
    let payload = &bytes[end..];
    if payload.len() != PAYLOAD_BYTES {
        return Err(corrupt(format!(
            "the payload is {} bytes, not {PAYLOAD_BYTES}",
            payload.len()
        )));
    }
    let actual = digest::hex(&digest::sha256(payload));
    if actual != checksum {
        return Err(corrupt(format!(
            "the payload's SHA-256 is {actual}, the header's {checksum}"
        )));
    }
    let values: Vec<f32> = payload
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect();
    if let Some(index) = values.iter().position(|value| !value.is_finite()) {
        return Err(corrupt(format!("payload value {index} is not finite")));
    }
    let (w1, w2) = values.split_at(W1_ELEMENTS);
    Ok(WeightArtifact {
        w1: w1.to_vec(),
        w2: w2.to_vec(),
        model: MODEL_NAME.to_owned(),
        modelspec_version: version.to_owned(),
        modelspec_commit: commit.to_owned(),
        feature_contract: feature_contract.to_owned(),
        checksum: checksum.to_owned(),
    })
}

/// Writes `bytes` to a file that must not exist yet through `write`; a failed write removes the
/// file this call created (Da9-11, the `generate_dataset.rs` precedent).
fn write_new(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> Result<(), ExportError> {
    let failed = |error| ExportError::Io {
        path: path.to_path_buf(),
        error,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => io::Error::new(
                io::ErrorKind::AlreadyExists,
                "already exists; it is never overwritten",
            ),
            _ => error,
        })
        .map_err(failed)?;
    if let Err(error) = write(&mut file, bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(failed(error));
    }
    Ok(())
}

impl WeightArtifact {
    fn new(w1: Vec<f32>, w2: Vec<f32>, pin: &ModelSpecPin, feature_contract: &str) -> Self {
        let checksum = digest::hex(&digest::sha256(&payload(&w1, &w2)));
        WeightArtifact {
            w1,
            w2,
            model: MODEL_NAME.to_owned(),
            modelspec_version: pin.version().to_owned(),
            modelspec_commit: pin.commit().to_owned(),
            feature_contract: feature_contract.to_owned(),
            checksum,
        }
    }

    /// `ExportWeights`: the w1 and w2 a `Completed` run printed, checked in the order of the
    /// module documentation and written to `path`, a new file, for `feature_contract` (the
    /// version the caller gives, such as `patchback_analysis::FEATURES_1_VERSION`; unchecked,
    /// Da9-9). The `Ok` is `WeightsExported`. A refusal writes nothing.
    pub fn export(
        run: &TrainingRun,
        feature_contract: &str,
        path: &Path,
    ) -> Result<WeightArtifact, ExportError> {
        if run.state() != TrainingRunState::Completed {
            return Err(ExportError::TrainingNotCompleted { state: run.state() });
        }
        let (w1, w2) = weights_from_output(run.stdout())?;
        if !is_token(feature_contract) {
            return Err(ExportError::Unwritable {
                reason: format!(
                    "the feature contract {feature_contract:?} is not one printable ASCII token"
                ),
            });
        }
        let artifact = WeightArtifact::new(w1, w2, run.pin(), feature_contract);
        let bytes = encode(&artifact);
        let header_bytes = bytes.len() - PAYLOAD_BYTES;
        if header_bytes > MAX_HEADER_BYTES {
            return Err(ExportError::Unwritable {
                reason: format!(
                    "the header takes {header_bytes} bytes, more than {MAX_HEADER_BYTES}"
                ),
            });
        }
        write_new(path, &bytes, |file, bytes| {
            file.write_all(bytes).and_then(|()| file.sync_all())
        })?;
        Ok(artifact)
    }

    /// Reads the artifact at `path`, at most [`MAX_HEADER_BYTES`] + [`PAYLOAD_BYTES`] + 1 bytes
    /// of it: a file that is not a `weights/1` artifact whose checksum verifies is
    /// [`ReadError::Corrupt`].
    pub fn read(path: &Path) -> Result<WeightArtifact, ReadError> {
        let failed = |error| ReadError::Io {
            path: path.to_path_buf(),
            error,
        };
        let file = File::open(path).map_err(failed)?;
        let limit = (MAX_HEADER_BYTES + PAYLOAD_BYTES + 1) as u64;
        let mut bytes = Vec::new();
        file.take(limit).read_to_end(&mut bytes).map_err(failed)?;
        decode(&bytes)
    }

    /// w1's 2,048 values, in the order printed (whether that is row-major is ModelSpec's; Da9-3).
    pub fn w1(&self) -> &[f32] {
        &self.w1
    }

    /// w2's 512 values, as printed.
    pub fn w2(&self) -> &[f32] {
        &self.w2
    }

    /// The model name, [`MODEL_NAME`].
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The version of the ModelSpec pin the run was planned under.
    pub fn modelspec_version(&self) -> &str {
        &self.modelspec_version
    }

    /// The commit of that pin.
    pub fn modelspec_commit(&self) -> &str {
        &self.modelspec_commit
    }

    /// The feature-contract version given at export.
    pub fn feature_contract(&self) -> &str {
        &self.feature_contract
    }

    /// The SHA-256 of the payload, as 64 lowercase hexadecimal digits.
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

/// The parser on fixed strings and the reader on bytes. The scenarios, which drive
/// [`WeightArtifact::export`] through a [`TrainingRun`], are `tests/weight_export_scenarios.rs`.
#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use super::*;
    use crate::{LossCurve, ModelSpecPin, OutputError};

    fn values(count: usize, salt: usize) -> Vec<f32> {
        (0..count)
            .map(|index| ((index * 7919 + salt * 104_729) % 4001) as f32 / 3001.0 - 0.6667)
            .collect()
    }

    fn w1() -> Vec<f32> {
        values(W1_ELEMENTS, 1)
    }

    fn w2() -> Vec<f32> {
        values(W2_ELEMENTS, 2)
    }

    fn tokens(values: &[f32]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn line(kind: &str, name: &str, shape: [usize; 2], tokens: &[String]) -> String {
        format!(
            "{kind} {name}: Tensor<f32, [{}, {}]> = [{}]",
            shape[0],
            shape[1],
            tokens.join(", ")
        )
    }

    /// The documented stdout of a 4-step run given `--eval`, one entry a line: the two `init`
    /// lines (lines 1 and 2), the `step` lines (3 and 4), `param` w1 (5) and w2 (6), `eval` (7).
    fn documented() -> Vec<String> {
        vec![
            line("init", W1_NAME, W1_SHAPE, &tokens(&values(W1_ELEMENTS, 3))),
            line("init", W2_NAME, W2_SHAPE, &tokens(&values(W2_ELEMENTS, 4))),
            "step 0 loss 1.0717294".to_owned(),
            "step 4 loss 0.8882302".to_owned(),
            line("param", W1_NAME, W1_SHAPE, &tokens(&w1())),
            line("param", W2_NAME, W2_SHAPE, &tokens(&w2())),
            "eval loss=0.9350907802581787 accuracy=0.0625".to_owned(),
        ]
    }

    fn join(lines: &[String]) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    fn bits(values: &[f32]) -> Vec<u32> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    /// The refusal of `stdout`, without the 2,560 values of an accepted one.
    fn refusal(stdout: &str) -> ExportError {
        match weights_from_output(stdout) {
            Err(error) => error,
            Ok(_) => panic!("accepted: {:?}", &stdout[..stdout.len().min(120)]),
        }
    }

    fn unrecognized(stdout: &str) -> (Option<usize>, &'static str) {
        match refusal(stdout) {
            ExportError::UnrecognizedTrainingOutput { line, reason } => (line, reason),
            other => panic!("{other:?}"),
        }
    }

    /// `documented()` with the line at `index` (0-based) replaced by `with(line)`.
    fn edited(index: usize, with: impl FnOnce(&str) -> String) -> String {
        let mut lines = documented();
        lines[index] = with(&lines[index]);
        join(&lines)
    }

    /// `line` with its value list (between `= [` and the last `]`) replaced by `with(values)`.
    fn in_values(line: &str, with: impl FnOnce(&str) -> String) -> String {
        let (head, values) = line.split_once("= [").unwrap();
        let values = values.strip_suffix(']').unwrap();
        format!("{head}= [{}]", with(values))
    }

    fn inserted(index: usize, extra: &str) -> String {
        let mut lines = documented();
        lines.insert(index, extra.to_owned());
        join(&lines)
    }

    #[test]
    fn the_documented_stdout_is_read_whole() {
        let (read_w1, read_w2) = weights_from_output(&join(&documented())).unwrap();
        assert_eq!(bits(&read_w1), bits(&w1()));
        assert_eq!(bits(&read_w2), bits(&w2()));

        // The last line without its terminator is still a line.
        let mut stdout = join(&documented());
        stdout.pop();
        assert!(weights_from_output(&stdout).is_ok());
    }

    /// Da9-8: a run that completed is read whatever its loss curve says, as long as every line
    /// is in the grammar: no `init`, `step` or `eval` line, or a NaN loss, still export.
    #[test]
    fn the_loss_lines_are_in_the_grammar_and_not_the_exports_verdict() {
        let lines = documented();
        let params = join(&lines[4..6]);
        assert_eq!(LossCurve::parse(&params), Err(OutputError::NoStepLine));
        assert!(weights_from_output(&params).is_ok());
        let nan_loss = inserted(4, "step 8 loss NaN");
        assert!(LossCurve::parse(&nan_loss).is_ok());
        assert!(weights_from_output(&nan_loss).is_ok());
        assert!(
            weights_from_output(&join(&lines[..6])).is_ok(),
            "no eval line"
        );
    }

    #[test]
    fn an_extra_or_empty_line_is_unrecognized_output() {
        for extra in [
            "checkpoint written to run.msckpt",
            "",
            " ",
            "\r",
            "step",
            "step 8",
            "Step 8 loss 0.5",
            "param",
            "init",
            "eval",
            "# comment",
            "warning: lr 0.01",
        ] {
            let (line, _) = unrecognized(&inserted(4, extra));
            assert_eq!(line, Some(5), "{extra:?}");
        }
        // An empty line at the end, after the last line's terminator.
        let stdout = format!("{}\n", join(&documented()));
        assert_eq!(unrecognized(&stdout).0, Some(8));
        // CRLF line ends: the first line already carries a `\r`.
        let stdout = documented().join("\r\n");
        assert_eq!(unrecognized(&stdout).0, Some(1));
    }

    #[test]
    fn a_malformed_param_line_is_unrecognized_output() {
        let w1_head = "param @PatchRegressor.w1: Tensor<f32, [32, 64]> = [";
        for (from, to) in [
            ("param @", "param  @"),
            ("param @", " param @"),
            ("param @", "params @"),
            ("@PatchRegressor.w1:", "@PatchRegressor.:"),
            ("@PatchRegressor.w1:", "@.w1:"),
            ("@PatchRegressor.w1:", "@PatchRegressor:"),
            ("@PatchRegressor.w1:", "@Patch.Regressor.w1:"),
            ("@PatchRegressor.w1:", "@Patch Regressor.w1:"),
            ("@PatchRegressor.w1:", "@PatchRegressor.w-1:"),
            ("@PatchRegressor.w1:", "PatchRegressor.w1:"),
            ("@PatchRegressor.w1:", "@PatchRegressor.w1 :"),
            ("Tensor<f32,", "Tensor<f64,"),
            ("[32, 64]", "[32,64]"),
            ("[32, 64]", "[32, 64, 1]"),
            ("[32, 64]", "[+32, 64]"),
            ("[32, 64]", "[032, 64]"),
            ("[32, 64]", "[32]"),
            ("[32, 64]", "[, 64]"),
            ("[32, 64]", "[99999999999999999999999, 64]"),
            ("> = [", ">= ["),
            ("= [", "= [ "),
        ] {
            let stdout = edited(4, |line| {
                assert!(line.starts_with(w1_head));
                line.replacen(from, to, 1)
            });
            assert_eq!(unrecognized(&stdout).0, Some(5), "{from:?} -> {to:?}");
        }
        for separator in [",", ",  ", ", , ", " , ", ";"] {
            let stdout = edited(4, |line| {
                in_values(line, |values| values.replacen(", ", separator, 1))
            });
            assert_eq!(unrecognized(&stdout).0, Some(5), "{separator:?}");
        }
        for tail in ["", "]]", "] ", ", ]", "]x"] {
            let stdout = edited(4, |line| format!("{}{tail}", &line[..line.len() - 1]));
            assert_eq!(unrecognized(&stdout).0, Some(5), "{tail:?}");
        }
    }

    #[test]
    fn a_token_f32_from_str_does_not_parse_is_unrecognized_output() {
        let mut tokens = tokens(&w1());
        for bad in ["0.1x", "", "0x10", "1_000", "1,5", "--1", "١"] {
            tokens[100] = bad.to_owned();
            let stdout = edited(4, |_| line("param", W1_NAME, W1_SHAPE, &tokens));
            assert_eq!(unrecognized(&stdout).0, Some(5), "{bad:?}");
        }
        // In an `init` line as well, though its values are not exported.
        let stdout = edited(0, |line| {
            in_values(line, |values| values.replacen(", ", ", nope, ", 1))
        });
        assert_eq!(unrecognized(&stdout).0, Some(1));
    }

    #[test]
    fn a_missing_or_third_param_line_is_unrecognized_output() {
        let lines = documented();
        let without_w2: Vec<String> = lines.iter().take(5).chain(&lines[6..]).cloned().collect();
        assert_eq!(unrecognized(&join(&without_w2)).0, None);
        let without_both: Vec<String> = lines[..4].iter().chain(&lines[6..]).cloned().collect();
        assert_eq!(unrecognized(&join(&without_both)).0, None);
        assert_eq!(unrecognized("").0, None);

        let third = line(
            "param",
            "@PatchRegressor.w3",
            [8, 1],
            &tokens(&values(8, 5)),
        );
        let mut with_third = lines.clone();
        with_third.push(third);
        assert_eq!(unrecognized(&join(&with_third)).0, Some(8));
        let mut w2_twice = lines.clone();
        w2_twice.insert(6, lines[5].clone());
        assert_eq!(unrecognized(&join(&w2_twice)).0, Some(7));
    }

    /// Every line `LossCurve::parse` calls malformed (training.rs, Da8-40) fails the grammar,
    /// so a run whose `output_error` names a malformed line exports nothing (Da9-8).
    #[test]
    fn every_malformed_loss_line_is_unrecognized_output() {
        for extra in [
            "step 4 loss abc",
            "step x loss 0.5",
            "step -1 loss 0.5",
            "step 0 loss 0.5 extra",
            "step 0 loss",
            "step 4 loss 0.5",
            "step 3 loss 0.5",
            "eval loss=0.x accuracy=1",
            "eval loss=0.5 accuracy=?",
            "eval loss=0.5",
        ] {
            let stdout = inserted(4, extra);
            assert!(
                matches!(
                    LossCurve::parse(&stdout),
                    Err(OutputError::Malformed { line: 5, .. })
                ),
                "{extra:?}"
            );
            assert_eq!(unrecognized(&stdout).0, Some(5), "{extra:?}");
        }
        let mut lines = documented();
        lines.push("eval loss=0.4 accuracy=1".to_owned());
        let stdout = join(&lines);
        assert!(matches!(
            LossCurve::parse(&stdout),
            Err(OutputError::Malformed { line: 8, .. })
        ));
        assert_eq!(unrecognized(&stdout).0, Some(8));
    }

    #[test]
    fn a_bias_or_a_swap_is_an_unrecognized_parameter() {
        let stdout = edited(4, |line| line.replacen(W1_NAME, "@PatchRegressor.b1", 1));
        match refusal(&stdout) {
            ExportError::UnrecognizedParameter { w1_name, w2_name } => {
                assert_eq!(
                    (w1_name.as_str(), w2_name.as_str()),
                    ("@PatchRegressor.b1", W2_NAME)
                )
            }
            other => panic!("{other:?}"),
        }
        let mut lines = documented();
        lines.swap(4, 5);
        match refusal(&join(&lines)) {
            ExportError::UnrecognizedParameter { w1_name, w2_name } => {
                assert_eq!((w1_name.as_str(), w2_name.as_str()), (W2_NAME, W1_NAME))
            }
            other => panic!("{other:?}"),
        }
    }

    /// Da9-19: a name is `@<ident>.<ident>` in the grammar, and the names step compares each
    /// `param` name in full, so a parameter of another model is an unrecognized parameter.
    #[test]
    fn a_param_of_another_model_is_an_unrecognized_parameter() {
        for (index, name, expected) in [
            (4, "@Other.w1", ("@Other.w1", W2_NAME)),
            (5, "@Other.w2", (W1_NAME, "@Other.w2")),
            (4, "@PatchRegressor2.w1", ("@PatchRegressor2.w1", W2_NAME)),
        ] {
            let current = if index == 4 { W1_NAME } else { W2_NAME };
            let stdout = edited(index, |line| line.replacen(current, name, 1));
            match refusal(&stdout) {
                ExportError::UnrecognizedParameter { w1_name, w2_name } => {
                    assert_eq!((w1_name.as_str(), w2_name.as_str()), expected, "{name}")
                }
                other => panic!("{name}: {other:?}"),
            }
        }
    }

    /// Da9-19, Da9-6: an `init` line's name is not checked, so `init @Other.w1` reads.
    #[test]
    fn an_init_of_another_model_is_read() {
        let stdout = edited(0, |line| line.replacen(W1_NAME, "@Other.w1", 1));
        let (read_w1, read_w2) = weights_from_output(&stdout).unwrap();
        assert_eq!(bits(&read_w1), bits(&w1()));
        assert_eq!(bits(&read_w2), bits(&w2()));
    }

    #[test]
    fn a_declared_shape_other_than_32_64_and_64_8_is_a_wrong_shape() {
        let transposed = edited(4, |line| line.replacen("[32, 64]", "[64, 32]", 1));
        assert!(matches!(
            refusal(&transposed),
            ExportError::WrongWeightShape {
                w1_rows: 64,
                w1_columns: 32,
                w2_rows: 64,
                w2_columns: 8
            }
        ));
        let w2 = edited(5, |line| line.replacen("[64, 8]", "[8, 64]", 1));
        assert!(matches!(
            refusal(&w2),
            ExportError::WrongWeightShape {
                w1_rows: 32,
                w1_columns: 64,
                w2_rows: 8,
                w2_columns: 64
            }
        ));
    }

    #[test]
    fn a_count_other_than_2048_and_512_is_a_wrong_element_count() {
        let short = tokens(&w2()[..511]);
        let stdout = edited(5, |_| line("param", W2_NAME, W2_SHAPE, &short));
        assert!(matches!(
            refusal(&stdout),
            ExportError::WrongElementCount {
                w1_elements: 2048,
                w2_elements: 511
            }
        ));
        let long = tokens(&values(5000, 6));
        let stdout = edited(4, |_| line("param", W1_NAME, W1_SHAPE, &long));
        assert!(matches!(
            refusal(&stdout),
            ExportError::WrongElementCount {
                w1_elements: 5000,
                w2_elements: 512
            }
        ));
        let stdout = edited(5, |_| line("param", W2_NAME, W2_SHAPE, &[]));
        assert!(matches!(
            refusal(&stdout),
            ExportError::WrongElementCount {
                w1_elements: 2048,
                w2_elements: 0
            }
        ));
    }

    /// Da9-12: the parser keeps at most one value past the expected count, and still reads every
    /// token of the line, so the count it reports is the line's and a bad token past the kept
    /// values is still unrecognized output.
    #[test]
    fn the_parser_keeps_at_most_one_value_past_the_count() {
        let long = tokens(&values(5000, 6));
        let stdout = edited(4, |_| line("param", W1_NAME, W1_SHAPE, &long));
        let Ok([w1, w2]) = parse_output(&stdout) else {
            panic!("the grammar admits a long line");
        };
        assert_eq!((w1.count, w1.values.len()), (5000, W1_ELEMENTS + 1));
        assert_eq!((w2.count, w2.values.len()), (W2_ELEMENTS, W2_ELEMENTS));
        let mut bad = long.clone();
        bad[4000] = "x".to_owned();
        let stdout = edited(4, |_| line("param", W1_NAME, W1_SHAPE, &bad));
        assert_eq!(unrecognized(&stdout).0, Some(5));
        let mut nan = long;
        nan[2500] = "NaN".to_owned();
        let stdout = edited(4, |_| line("param", W1_NAME, W1_SHAPE, &nan));
        assert!(matches!(
            refusal(&stdout),
            ExportError::WrongElementCount {
                w1_elements: 5000,
                ..
            }
        ));
    }

    #[test]
    fn inf_and_nan_tokens_are_non_finite_weights() {
        for (token, index, in_w1) in [
            ("inf", 7, false),
            ("-inf", 0, true),
            ("NaN", 511, false),
            ("infinity", 2047, true),
            ("1e39", 3, true),
        ] {
            let (position, name, values) = if in_w1 {
                (4, W1_NAME, w1())
            } else {
                (5, W2_NAME, w2())
            };
            let mut tokens = tokens(&values);
            tokens[index] = token.to_owned();
            let shape = if in_w1 { W1_SHAPE } else { W2_SHAPE };
            let stdout = edited(position, |_| line("param", name, shape, &tokens));
            match refusal(&stdout) {
                ExportError::NonFiniteWeight {
                    name: found,
                    index: at,
                    value,
                } => {
                    assert_eq!((found, at), (name, index), "{token}");
                    assert!(!value.is_finite(), "{token}");
                }
                other => panic!("{token}: {other:?}"),
            }
        }
    }

    /// Da9-7: the grammar, then the names, the shapes, the counts and finiteness; of two defects
    /// the earlier one answers.
    #[test]
    fn the_refusals_answer_in_order() {
        let mut lines = documented();
        lines[4] = lines[4].replacen(W1_NAME, "@PatchRegressor.b1", 1);
        let bias = lines.clone();
        lines.insert(2, "note".to_owned());
        assert_eq!(
            unrecognized(&join(&lines)).0,
            Some(3),
            "grammar before names"
        );

        let mut lines = bias;
        lines[4] = lines[4].replacen("[32, 64]", "[64, 32]", 1);
        assert!(
            matches!(
                refusal(&join(&lines)),
                ExportError::UnrecognizedParameter { .. }
            ),
            "names before shapes"
        );

        let short = tokens(&w2()[..511]);
        let mut lines = documented();
        lines[4] = lines[4].replacen("[32, 64]", "[64, 32]", 1);
        lines[5] = line("param", W2_NAME, W2_SHAPE, &short);
        assert!(
            matches!(refusal(&join(&lines)), ExportError::WrongWeightShape { .. }),
            "shapes before counts"
        );

        let mut short_nan = short;
        short_nan[0] = "NaN".to_owned();
        let mut lines = documented();
        lines[5] = line("param", W2_NAME, W2_SHAPE, &short_nan);
        assert!(
            matches!(
                refusal(&join(&lines)),
                ExportError::WrongElementCount { .. }
            ),
            "counts before finiteness"
        );
    }

    fn pin() -> ModelSpecPin {
        ModelSpecPin::repository().unwrap()
    }

    fn artifact() -> WeightArtifact {
        WeightArtifact::new(w1(), w2(), &pin(), "features/1")
    }

    fn payload() -> Vec<u8> {
        w1().iter()
            .chain(&w2())
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn header(checksum: &str, features: &str) -> String {
        let pin = pin();
        format!(
            "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures {features}\nw1 32 64\nw2 64 8\nsha256 {checksum}\n\n",
            pin.version(),
            pin.commit()
        )
    }

    fn corrupt(bytes: &[u8]) -> String {
        match decode(bytes) {
            Err(ReadError::Corrupt { reason }) => reason,
            Err(other) => panic!("{other:?}"),
            Ok(_) => panic!("decoded"),
        }
    }

    #[test]
    fn an_artifact_is_its_header_and_payload_and_reads_back_equal() {
        let payload = payload();
        assert_eq!(payload.len(), PAYLOAD_BYTES);
        assert_eq!(PAYLOAD_BYTES, 10_240);
        let checksum = patchback_core::digest::hex(&patchback_core::digest::sha256(&payload));
        let artifact = artifact();
        assert_eq!(artifact.checksum(), checksum);
        assert_eq!(artifact.model(), "PatchRegressor");
        assert_eq!(artifact.modelspec_version(), pin().version());
        assert_eq!(artifact.modelspec_commit(), pin().commit());
        assert_eq!(artifact.feature_contract(), "features/1");
        let bytes = encode(&artifact);
        let header = header(&checksum, "features/1");
        assert_eq!(&bytes[..header.len()], header.as_bytes());
        assert_eq!(&bytes[header.len()..], payload.as_slice());
        let read = decode(&bytes).unwrap();
        assert_eq!(read, artifact);
        assert_eq!(bits(read.w1()), bits(&w1()));
        assert_eq!(bits(read.w2()), bits(&w2()));
    }

    #[test]
    fn a_flipped_payload_byte_is_corrupt() {
        let bytes = encode(&artifact());
        let start = bytes.len() - PAYLOAD_BYTES;
        for at in [start, start + 4 * W1_ELEMENTS, bytes.len() - 1] {
            let mut flipped = bytes.clone();
            flipped[at] ^= 0x01;
            assert!(corrupt(&flipped).contains("SHA-256"), "byte {at}");
        }
    }

    /// Da9-2: the checksum covers the payload, and the header is checked by the parser. A flip
    /// in a fixed part of the header (the keys, the constant lines, the separators, the
    /// checksum, the empty line) is corrupt; a flip that leaves a version, commit or feature
    /// value well-formed reads, with that value changed, and is listed here.
    #[test]
    fn a_flipped_header_byte_is_corrupt_unless_it_leaves_a_value_well_formed() {
        let artifact = artifact();
        let bytes = encode(&artifact);
        let header_len = bytes.len() - PAYLOAD_BYTES;
        let text = std::str::from_utf8(&bytes[..header_len]).unwrap();
        let span = |prefix: &str, value: &str| {
            let start = text.find(prefix).unwrap() + prefix.len();
            start..start + value.len()
        };
        let version = span("\nmodelspec ", artifact.modelspec_version());
        let commit = span(
            &format!("{} ", artifact.modelspec_version()),
            artifact.modelspec_commit(),
        );
        let features = span("\nfeatures ", artifact.feature_contract());
        let mut readable = Vec::new();
        for at in 0..header_len {
            let mut flipped = bytes.clone();
            flipped[at] ^= 0x01;
            match decode(&flipped) {
                Err(ReadError::Corrupt { .. }) => {}
                Err(other) => panic!("byte {at}: {other:?}"),
                Ok(read) => {
                    assert!(
                        version.contains(&at) || commit.contains(&at) || features.contains(&at),
                        "byte {at} ({:?}) flipped reads",
                        bytes[at] as char
                    );
                    assert_ne!(read, artifact, "byte {at}");
                    readable.push(at);
                }
            }
        }
        assert!(
            !readable.is_empty(),
            "a version digit flipped is another version"
        );
        assert!(
            corrupt(&{
                let mut flipped = bytes.clone();
                flipped[0] ^= 0x01;
                flipped
            })
            .contains("weights/1")
        );
    }

    #[test]
    fn a_truncated_or_longer_payload_is_corrupt() {
        let bytes = encode(&artifact());
        assert!(corrupt(&bytes[..bytes.len() - 1]).contains("payload"));
        assert!(corrupt(&bytes[..bytes.len() - PAYLOAD_BYTES]).contains("payload"));
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(corrupt(&longer).contains("payload"));
    }

    #[test]
    fn an_unknown_version_or_a_malformed_header_is_corrupt() {
        let payload = payload();
        let checksum = patchback_core::digest::hex(&patchback_core::digest::sha256(&payload));
        let good = header(&checksum, "features/1");
        let with = |header: &str| [header.as_bytes(), &payload].concat();
        assert!(decode(&with(&good)).is_ok());
        let lines: Vec<&str> = good.split_inclusive('\n').collect();
        let edits: Vec<(String, String)> = vec![
            ("weights/1\n".into(), "weights/2\n".into()),
            ("weights/1\n".into(), "weights/1 \n".into()),
            ("model PatchRegressor\n".into(), "model Other\n".into()),
            ("model PatchRegressor\n".into(), "".into()),
            ("w1 32 64\n".into(), "w1 64 32\n".into()),
            ("w2 64 8\n".into(), "w2 64 8\nw2 64 8\n".into()),
            ("w1 32 64\nw2 64 8\n".into(), "w2 64 8\nw1 32 64\n".into()),
            ("features features/1\n".into(), "features \n".into()),
            ("features features/1\n".into(), "features a b\n".into()),
            (
                format!("sha256 {checksum}\n"),
                format!("sha256 {}\n", checksum.to_uppercase()),
            ),
            (
                format!("sha256 {checksum}\n"),
                format!("sha256 {}\n", &checksum[1..]),
            ),
            (
                format!("sha256 {checksum}\n"),
                format!("sha256 {checksum}\nextra 1\n"),
            ),
            (lines[2].to_owned(), "modelspec 0.5.0\n".into()),
            (
                lines[2].to_owned(),
                lines[2].replacen("modelspec ", "modelspec v", 1),
            ),
            ("\n\n".into(), "\n\r\n".into()),
        ];
        for (from, to) in edits {
            assert!(good.contains(&from), "{from:?}");
            let reason = corrupt(&with(&good.replacen(&from, &to, 1)));
            assert!(!reason.is_empty(), "{from:?} -> {to:?}");
        }
        assert!(corrupt(&with(&good.replacen("weights/1", "weights/2", 1))).contains("weights/1"));
    }

    /// Da9-11: the header, its empty line included, is read within 1,024 bytes.
    #[test]
    fn a_header_past_1024_bytes_is_corrupt() {
        let payload = payload();
        let checksum = patchback_core::digest::hex(&patchback_core::digest::sha256(&payload));
        let base = header(&checksum, "").len();
        let at_limit = "f".repeat(MAX_HEADER_BYTES - base);
        let header_text = header(&checksum, &at_limit);
        assert_eq!(header_text.len(), MAX_HEADER_BYTES);
        let read = decode(&[header_text.as_bytes(), &payload].concat()).unwrap();
        assert_eq!(read.feature_contract(), at_limit);
        let past = format!("{at_limit}f");
        let reason = corrupt(&[header(&checksum, &past).as_bytes(), &payload].concat());
        assert!(reason.contains("1024"), "{reason}");
    }

    #[test]
    fn a_non_finite_value_under_a_matching_checksum_is_corrupt() {
        let mut payload = payload();
        payload[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        let checksum = patchback_core::digest::hex(&patchback_core::digest::sha256(&payload));
        let bytes = [header(&checksum, "features/1").as_bytes(), &payload].concat();
        assert!(corrupt(&bytes).contains("finite"));
    }

    /// Da9-11: create-new, and a write that fails removes the file it created.
    #[test]
    fn a_failed_write_removes_the_file_and_an_existing_file_is_kept() {
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("patchback-model-weights-unit");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(RECOMMENDED_FILE_NAME);
        let result = write_new(&path, b"abc", |file, bytes| {
            file.write_all(&bytes[..1])?;
            Err(io::Error::other("the disk is full"))
        });
        assert!(matches!(result, Err(ExportError::Io { .. })), "{result:?}");
        assert!(!path.exists(), "the half-written file is removed");

        fs::write(&path, b"keep").unwrap();
        let result = write_new(&path, b"abc", |file, bytes| file.write_all(bytes));
        match result {
            Err(ExportError::Io { path: at, error }) => {
                assert_eq!(at, path);
                assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(fs::read(&path).unwrap(), b"keep");

        let fresh = dir.join("fresh.weights");
        write_new(&fresh, b"abc", |file, bytes| file.write_all(bytes)).unwrap();
        assert_eq!(fs::read(&fresh).unwrap(), b"abc");
    }

    #[test]
    fn the_errors_name_their_domain_errors() {
        assert_eq!(
            ExportError::examples()
                .iter()
                .map(|error| error.domain_error().unwrap())
                .collect::<Vec<_>>(),
            ExportError::DOMAIN_ERRORS
        );
        assert_eq!(
            ReadError::examples()
                .iter()
                .map(|error| error.domain_error().unwrap())
                .collect::<Vec<_>>(),
            ReadError::DOMAIN_ERRORS
        );
        assert_eq!(
            ReadError::DOMAIN_ERRORS,
            ["patchback.model.CorruptWeightArtifact"]
        );
        let io = || io::Error::other("x");
        assert_eq!(
            ExportError::Io {
                path: PathBuf::new(),
                error: io()
            }
            .domain_error(),
            None
        );
        assert_eq!(
            ExportError::Unwritable {
                reason: String::new()
            }
            .domain_error(),
            None
        );
        assert_eq!(
            ReadError::Io {
                path: PathBuf::new(),
                error: io()
            }
            .domain_error(),
            None
        );
    }
}
