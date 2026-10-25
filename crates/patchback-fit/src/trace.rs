//! `trace.events`, the per-candidate log of a fit run (DESIGN.md § 9.1 "log its component values
//! for every candidate", § 11), its writer [`write_trace`] and its parser [`parse_trace`]. The
//! trace is written from the run's evaluation log after the run has completed (Da7-2); a failed
//! run writes none. Whether a failed run keeps a trace, and the grammar's final form, stay open
//! (DESIGN.md § 17, OQ-12).
//!
//! # Grammar
//!
//! UTF-8 text of lines, each ended by `\n` (the parser also takes `\r\n`, and a last line with
//! no terminator); fields are separated by one space (the parser takes any run of ASCII
//! whitespace). No comments, no blank lines. The lines, in this order:
//!
//! ```text
//! patchback-fit-trace 1                                         the header: the format version
//! topology <topology> <path>...                                 the topology, its parameter paths in index order
//! evaluation <sequence> <evaluations> <restart> <outcome> <topology> <losses> <best> <value>...
//! ...                                                           one line per evaluation, in order
//! completed <stop_reason>                                       how the run ended
//! ```
//!
//! An `evaluation` line carries the fields of `spec/domains/studio.yaml` `TraceFrame`:
//!
//! - `<sequence>`: the evaluation's number, from 1 ([`crate::Evaluation::sequence`]), each line
//!   one more than the line before. `TraceFrame.sequence` counts from 0; it is this value less 1.
//! - `<evaluations>`: the run's evaluation count after it (`FitRun.evaluations`), at least 1:
//!   always equal to `<sequence>`, since every evaluation, valid or rejected, is counted. It is
//!   written anyway, and a line where it differs is refused.
//! - `<restart>`: the restart it belongs to, from 0.
//! - `<outcome>`: `first-retained`, `improved`, `not-improved` or `rejected`, the names of
//!   `spec/domains/fit.yaml` ([`EvaluationOutcome::name`]).
//! - `<topology>`: the topology the candidate is a patch of, the one the `topology` line names.
//! - `<losses>`: for a valid candidate five reals, the `log_spectral`, `envelope`, `transient`
//!   and `band_energy` components and the weighted total (DESIGN.md § 9.1); for a rejected one
//!   the single marker `-`.
//! - `<best>`: the run's best loss after it, or `-` while no candidate has been retained. Any
//!   number of `rejected` lines may carry `-` before the first numeric best; a `first-retained`,
//!   `improved` or `not-improved` line carries a number, and a `rejected` line before the first
//!   `first-retained` line carries `-`. From the first numeric best on every line, rejected or
//!   not, carries a number, never above the previous line's: the best loss never rises
//!   (`TraceFrame.best_loss`) and never returns to none.
//! - `<value>...`: the candidate's normalized parameter values, one per path of the `topology`
//!   line, in its order. With the topology they are the candidate patch
//!   ([`crate::patch_at`]).
//!
//! Integers are ASCII digits only. Reals are written in the shortest form that reads back to the
//! same `f64` (`patchback_core::patch::format_decimal`) and read with the `.patch` grammar's
//! finite decimal (`patchback_core::patch::parse_decimal`), so a written trace parses back to the
//! log bit for bit.
//!
//! # Refusals
//!
//! [`parse_trace`] answers the first fault, line by line, with a [`TraceError`] naming the line:
//! a line longer than [`TRACE_MAX_LINE_BYTES`] bytes (without its terminator) is refused before
//! it is split; a header other than `patchback-fit-trace <version>`, or a version other than 1,
//! is refused on line 1; an integer or real that does not read is a malformed number; a line out
//! of order, with the wrong field count, an unknown keyword, outcome or stop reason, a repeated
//! path, a sequence number out of order, an evaluation count of 0, a topology other than the
//! run's, `-` where a loss belongs or a loss where `-` belongs, or any line after the closing
//! line, is a malformed record. Five lines no run writes have their own reasons, each naming
//! the values: an `<evaluations>` other than the line's `<sequence>` is
//! [`TraceReason::EvaluationCountNotSequence`]; a numeric `<best>` above the previous evaluation
//! line's is [`TraceReason::BestLossRose`]; a `<best>` of `-` after a numeric one is
//! [`TraceReason::BestLossLost`]; a `<best>` of `-` on a `first-retained`, `improved` or
//! `not-improved` line with no numeric best before it is
//! [`TraceReason::RetainedWithoutBestLoss`]; a numeric `<best>` on a `rejected` line before the
//! first `first-retained` line is [`TraceReason::BestLossBeforeRetained`]. A text that ends
//! before its closing line is [`TraceReason::Incomplete`]. The number of evaluation lines is the
//! run's render budget's and is not bounded here.
//!
//! [`write_trace`] parses what it wrote before returning it, so it never returns a text
//! [`parse_trace`] refuses.

use std::fmt::{self, Write as _};

use patchback_core::parameter::Topology;
use patchback_core::patch::{format_decimal, parse_decimal};

use crate::{Evaluation, EvaluationOutcome, LossComponents, StopReason};

/// The first field of the header line.
pub const TRACE_HEADER: &str = "patchback-fit-trace";

/// The `trace.events` format version this module reads and writes.
pub const TRACE_FORMAT_VERSION: u32 = 1;

/// The longest line, in bytes without its terminator. Provisional (OQ-12): an evaluation line of
/// the eight `subtractive-mono/1` parameters with every real at its longest (24 bytes, as
/// `-1.2345678901234567e-308`) is about 440 bytes.
pub const TRACE_MAX_LINE_BYTES: usize = 4_096;

const TOPOLOGY: &str = "topology";
const EVALUATION: &str = "evaluation";
const COMPLETED: &str = "completed";
const MARKER: &str = "-";

const OUTCOMES: [EvaluationOutcome; 4] = [
    EvaluationOutcome::FirstRetained,
    EvaluationOutcome::Improved,
    EvaluationOutcome::NotImproved,
    EvaluationOutcome::Rejected,
];

/// A parsed `trace.events`.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    /// The topology the `topology` line names.
    pub topology: String,
    /// Its parameter paths, in index order.
    pub paths: Vec<String>,
    /// One per `evaluation` line, in order.
    pub records: Vec<TraceRecord>,
    /// Why the run stopped: the closing line's.
    pub stop_reason: StopReason,
}

/// One `evaluation` line (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRecord {
    /// The evaluation's number, from 1: `<sequence>`.
    pub sequence: u64,
    /// The run's evaluation count after it, equal to `sequence`: `<evaluations>`.
    pub evaluations: u64,
    /// The restart it belongs to, from 0: `<restart>`.
    pub restart: u32,
    /// What the evaluation did to the run: `<outcome>`.
    pub outcome: EvaluationOutcome,
    /// The topology the candidate is a patch of, the `topology` line's: `<topology>`.
    pub topology: String,
    /// The four components, for a valid candidate.
    pub components: Option<LossComponents>,
    /// The weighted total, for a valid candidate.
    pub total: Option<f64>,
    /// The run's best loss after it, once a candidate has been retained.
    pub best_after: Option<f64>,
    /// The normalized values, one per path.
    pub normalized: Vec<f64>,
}

/// Why a `trace.events` text was refused (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub enum TraceReason {
    /// The header names a version this module does not read.
    UnknownFormatVersion { version: u32 },
    /// A line that is not the record its place calls for; `expected` says what was.
    MalformedRecord { expected: &'static str },
    /// An `evaluation` line whose `<evaluations>` is not its `<sequence>`: no run writes it.
    EvaluationCountNotSequence { sequence: u64, evaluations: u64 },
    /// An `evaluation` line whose `<best>` is above the previous `evaluation` line's: the best
    /// loss never rises (`spec/domains/studio.yaml` `TraceFrame`).
    BestLossRose { previous: f64, best: f64 },
    /// An `evaluation` line whose `<best>` is `-` after a line with a numeric best: once a
    /// candidate is retained, the run always has a best loss.
    BestLossLost { previous: f64 },
    /// A `first-retained`, `improved` or `not-improved` line whose `<best>` is `-`, with no
    /// numeric best before it: a retained candidate gives the run a best loss.
    RetainedWithoutBestLoss { outcome: EvaluationOutcome },
    /// A `rejected` line with a numeric `<best>` before the first `first-retained` line: no
    /// candidate has scored that best loss.
    BestLossBeforeRetained { best: f64 },
    /// A token that is not an integer or a finite real of the grammar.
    MalformedNumber { token: String },
    /// A line longer than [`TRACE_MAX_LINE_BYTES`].
    LineTooLong { bytes: usize },
    /// The text ended before its closing line.
    Incomplete,
}

impl fmt::Display for TraceReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormatVersion { version } => write!(
                f,
                "unknown {TRACE_HEADER} version {version}; only {TRACE_FORMAT_VERSION} is read"
            ),
            Self::MalformedRecord { expected } => write!(f, "expected {expected}"),
            Self::EvaluationCountNotSequence {
                sequence,
                evaluations,
            } => write!(
                f,
                "the evaluation count {evaluations} is not the sequence number {sequence}"
            ),
            Self::BestLossRose { previous, best } => write!(
                f,
                "the best loss {} is above the previous line's {}; it never rises",
                format_decimal(*best),
                format_decimal(*previous)
            ),
            Self::BestLossLost { previous } => write!(
                f,
                "the best loss is `{MARKER}` after the previous line's {}; once a candidate is \
                 retained there is always one",
                format_decimal(*previous)
            ),
            Self::RetainedWithoutBestLoss { outcome } => write!(
                f,
                "the best loss is `{MARKER}` on a `{}` line; a retained candidate gives the run \
                 one",
                outcome.name()
            ),
            Self::BestLossBeforeRetained { best } => write!(
                f,
                "the best loss {} is on a `{}` line before the first `{}` line; no candidate \
                 has scored it",
                format_decimal(*best),
                EvaluationOutcome::Rejected.name(),
                EvaluationOutcome::FirstRetained.name()
            ),
            Self::MalformedNumber { token } => write!(f, "{token:?} is not a number"),
            Self::LineTooLong { bytes } => write!(
                f,
                "the line has {bytes} bytes, more than the bound of {TRACE_MAX_LINE_BYTES}"
            ),
            Self::Incomplete => write!(f, "the trace ends before its `{COMPLETED}` line"),
        }
    }
}

/// A refused `trace.events`: the 1-based line, or none for a condition of the whole text, and
/// the reason.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceError {
    /// The 1-based line refused, or none for a condition of the whole text.
    pub line: Option<usize>,
    /// Why it was refused.
    pub reason: TraceReason,
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.reason),
            None => self.reason.fmt(f),
        }
    }
}

impl std::error::Error for TraceError {}

/// Writes the trace of a completed run of `topology` from its evaluation log and its stop
/// reason (module documentation), or the refusal [`parse_trace`] gives the text: an evaluation
/// whose normalized values do not match the topology's parameters, a valid outcome without its
/// losses, a non-finite value, a line past the bound, a best loss that rises or returns to none,
/// a retained candidate without a best loss, a rejected one with a best loss before the first
/// retained (a log [`crate::fit`] makes has none of these).
pub fn write_trace(
    topology: &Topology,
    log: &[Evaluation],
    stop_reason: StopReason,
) -> Result<String, TraceError> {
    let mut contracts: Vec<_> = topology.contracts().iter().collect();
    contracts.sort_by_key(|contract| contract.index);
    let name = topology.name();
    let mut text = String::new();
    // Writing to a String cannot fail.
    let _ = writeln!(text, "{TRACE_HEADER} {TRACE_FORMAT_VERSION}");
    text.push_str(TOPOLOGY);
    text.push(' ');
    text.push_str(name);
    for contract in &contracts {
        text.push(' ');
        text.push_str(&contract.path);
    }
    text.push('\n');
    for evaluation in log {
        let _ = write!(
            text,
            "{EVALUATION} {sequence} {sequence} {} {} {name}",
            evaluation.restart,
            evaluation.outcome.name(),
            sequence = evaluation.sequence,
        );
        match (evaluation.outcome, evaluation.components, evaluation.total) {
            (EvaluationOutcome::Rejected, _, _) | (_, None, _) | (_, _, None) => {
                text.push(' ');
                text.push_str(MARKER);
            }
            (_, Some(components), Some(total)) => {
                for value in [
                    components.log_spectral,
                    components.envelope,
                    components.transient,
                    components.band_energy,
                    total,
                ] {
                    text.push(' ');
                    text.push_str(&format_decimal(value));
                }
            }
        }
        text.push(' ');
        match evaluation.best_after {
            Some(best) => text.push_str(&format_decimal(best)),
            None => text.push_str(MARKER),
        }
        for value in &evaluation.normalized {
            text.push(' ');
            text.push_str(&format_decimal(*value));
        }
        text.push('\n');
    }
    let _ = writeln!(text, "{COMPLETED} {}", stop_reason.name());
    parse_trace(&text)?;
    Ok(text)
}

fn malformed(line: usize, expected: &'static str) -> TraceError {
    TraceError {
        line: Some(line),
        reason: TraceReason::MalformedRecord { expected },
    }
}

fn integer<T: std::str::FromStr>(line: usize, token: &str) -> Result<T, TraceError> {
    let number = || TraceError {
        line: Some(line),
        reason: TraceReason::MalformedNumber {
            token: token.to_owned(),
        },
    };
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(number());
    }
    token.parse().map_err(|_| number())
}

fn real(line: usize, token: &str) -> Result<f64, TraceError> {
    parse_decimal(token).ok_or_else(|| TraceError {
        line: Some(line),
        reason: TraceReason::MalformedNumber {
            token: token.to_owned(),
        },
    })
}

fn real_or_marker(line: usize, token: &str) -> Result<Option<f64>, TraceError> {
    if token == MARKER {
        Ok(None)
    } else {
        real(line, token).map(Some)
    }
}

const EVALUATION_SHAPE: &str = "evaluation <sequence> <evaluations> <restart> <outcome> \
                                <topology>, five losses or `-`, the best loss or `-`, and one \
                                value per parameter path";

fn refused(line: usize, reason: TraceReason) -> TraceError {
    TraceError {
        line: Some(line),
        reason,
    }
}

/// Parses one `evaluation` line, `fields` split, number `line`, after the records `previous`.
fn evaluation(
    line: usize,
    fields: &[&str],
    topology: &str,
    paths: usize,
    previous: &[TraceRecord],
) -> Result<TraceRecord, TraceError> {
    let outcome = fields
        .get(4)
        .and_then(|token| OUTCOMES.into_iter().find(|o| o.name() == *token))
        .ok_or_else(|| malformed(line, EVALUATION_SHAPE))?;
    let losses = if outcome == EvaluationOutcome::Rejected {
        1
    } else {
        5
    };
    if fields.len() != 6 + losses + 1 + paths {
        return Err(malformed(line, EVALUATION_SHAPE));
    }
    let sequence: u64 = integer(line, fields[1])?;
    let evaluations: u64 = integer(line, fields[2])?;
    let restart: u32 = integer(line, fields[3])?;
    if usize::try_from(sequence).ok() != previous.len().checked_add(1) {
        return Err(malformed(line, "sequence numbers 1, 2, 3 and on, in order"));
    }
    if evaluations == 0 {
        return Err(malformed(line, "an evaluation count of at least 1"));
    }
    if evaluations != sequence {
        return Err(refused(
            line,
            TraceReason::EvaluationCountNotSequence {
                sequence,
                evaluations,
            },
        ));
    }
    if fields[5] != topology {
        return Err(malformed(line, "the topology the topology line names"));
    }
    let (components, total) = if outcome == EvaluationOutcome::Rejected {
        if fields[6] != MARKER {
            return Err(malformed(
                line,
                "`-` for the losses of a rejected candidate",
            ));
        }
        (None, None)
    } else {
        let mut values = [0.0; 5];
        for (value, token) in values.iter_mut().zip(&fields[6..11]) {
            *value = real(line, token)?;
        }
        let [log_spectral, envelope, transient, band_energy, total] = values;
        (
            Some(LossComponents {
                log_spectral,
                envelope,
                transient,
                band_energy,
            }),
            Some(total),
        )
    };
    let best = 6 + losses;
    let best_after = real_or_marker(line, fields[best])?;
    if let Some(previous) = previous.last().and_then(|record| record.best_after) {
        match best_after {
            None => return Err(refused(line, TraceReason::BestLossLost { previous })),
            Some(best) if best > previous => {
                return Err(refused(line, TraceReason::BestLossRose { previous, best }));
            }
            Some(_) => {}
        }
    } else if best_after.is_none() && outcome != EvaluationOutcome::Rejected {
        return Err(refused(
            line,
            TraceReason::RetainedWithoutBestLoss { outcome },
        ));
    }
    if let Some(best) = best_after
        && outcome == EvaluationOutcome::Rejected
        && !previous
            .iter()
            .any(|record| record.outcome == EvaluationOutcome::FirstRetained)
    {
        return Err(refused(line, TraceReason::BestLossBeforeRetained { best }));
    }
    let normalized = fields[best + 1..]
        .iter()
        .map(|token| real(line, token))
        .collect::<Result<Vec<f64>, TraceError>>()?;
    Ok(TraceRecord {
        sequence,
        evaluations,
        restart,
        outcome,
        topology: topology.to_owned(),
        components,
        total,
        best_after,
        normalized,
    })
}

/// Parses a `trace.events` text (module documentation).
pub fn parse_trace(text: &str) -> Result<Trace, TraceError> {
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut header: Option<(String, Vec<String>)> = None;
    let mut records = Vec::new();
    let mut end: Option<StopReason> = None;
    if !body.is_empty() || text.ends_with('\n') {
        for (index, raw) in body.split('\n').enumerate() {
            let line = index + 1;
            let content = raw.strip_suffix('\r').unwrap_or(raw);
            if content.len() > TRACE_MAX_LINE_BYTES {
                return Err(TraceError {
                    line: Some(line),
                    reason: TraceReason::LineTooLong {
                        bytes: content.len(),
                    },
                });
            }
            let fields: Vec<&str> = content.split_ascii_whitespace().collect();
            if end.is_some() {
                return Err(malformed(line, "nothing after the closing line"));
            }
            if line == 1 {
                let [name, version] = fields[..] else {
                    return Err(malformed(
                        line,
                        "the header `patchback-fit-trace <version>`",
                    ));
                };
                if name != TRACE_HEADER {
                    return Err(malformed(
                        line,
                        "the header `patchback-fit-trace <version>`",
                    ));
                }
                let version: u32 = integer(line, version)?;
                if version != TRACE_FORMAT_VERSION {
                    return Err(TraceError {
                        line: Some(line),
                        reason: TraceReason::UnknownFormatVersion { version },
                    });
                }
                continue;
            }
            let Some((topology, paths)) = &header else {
                let [TOPOLOGY, name, paths @ ..] = &fields[..] else {
                    return Err(malformed(line, "`topology <topology> <path>...`"));
                };
                let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
                for (at, path) in paths.iter().enumerate() {
                    if paths[..at].contains(path) {
                        return Err(malformed(line, "distinct parameter paths"));
                    }
                }
                header = Some(((*name).to_owned(), paths));
                continue;
            };
            match fields.first() {
                Some(&EVALUATION) => {
                    records.push(evaluation(line, &fields, topology, paths.len(), &records)?);
                }
                Some(&COMPLETED) => {
                    let reason = match &fields[..] {
                        [_, reason] => StopReason::ALL.into_iter().find(|r| r.name() == *reason),
                        _ => None,
                    };
                    end = Some(reason.ok_or_else(|| {
                        malformed(line, "`completed <stop_reason>` naming a stop reason")
                    })?);
                }
                _ => {
                    return Err(malformed(
                        line,
                        "an `evaluation` line or the `completed` line",
                    ));
                }
            }
        }
    }
    match (header, end) {
        (Some((topology, paths)), Some(stop_reason)) => Ok(Trace {
            topology,
            paths,
            records,
            stop_reason,
        }),
        _ => Err(TraceError {
            line: None,
            reason: TraceReason::Incomplete,
        }),
    }
}
