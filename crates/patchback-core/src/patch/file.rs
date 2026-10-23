//! The `.patch` file: a line-oriented, versioned text format (DESIGN.md § 11), its parser and
//! its canonical writer, with std only.
//!
//! # Grammar
//!
//! A file is UTF-8 text of lines, each ended by `\n` or `\r\n`; the terminator is not part of
//! the line, and the last line may have none. Fields are separated by ASCII whitespace (one space
//! in the canonical form). There are no comments and no blank lines: both are refused.
//!
//! ```text
//! patchback-patch 1                       the header, first line: the format version
//! sample_rate_hz 48000                    patch key, exactly once
//! topology_id subtractive-mono/1          patch key, exactly once
//! node <id> <kind>                        one line per node
//! param <node id> <name> <value>          one line per parameter, after its node's line
//! edge <from> <port> <rate> <to> <port> <rate>   one line per edge
//! ```
//!
//! After the header the records may come in any order, except that a `param` line follows the
//! `node` line it names.
//!
//! - `<id>`, `<node id>`, the version and the sample rate are integers: ASCII digits only, no
//!   sign, fitting a `u32`.
//! - `<kind>` is a [`NodeKind`](super::NodeKind) variant name (`SawOscillator`), `<rate>` a
//!   [`PortRate`](super::PortRate) variant name (`Audio`, `Control`, `Event`).
//! - `<name>` (a parameter or a port) is `[a-z][a-z0-9_]*`: parameter paths are bare snake_case
//!   names until DESIGN.md § 17 OQ-2 decides otherwise.
//! - `topology_id` is one or more of `[A-Za-z0-9._/-]`, so both a UUID and a topology name fit.
//! - `<value>` is a finite decimal ([`parse_decimal`]): an optional sign, one or more digits, an
//!   optional `.` followed by one or more digits, an optional exponent `e` or `E` with an
//!   optional sign and one or more digits; the value must be finite after parsing (`1e400` is
//!   refused, `1e-400` reads as zero). `nan`, `inf`, hexadecimal, `.5`, `5.` and the empty token
//!   are refused.
//!
//! # Bounds
//!
//! The four bounds are provisional (DESIGN.md § 17, OQ-6, open): [`MAX_LINE_BYTES`] (bytes of a
//! line without its terminator), [`MAX_FILE_BYTES`], [`MAX_NODES`] and [`MAX_EDGES`]. A reader
//! takes at most `MAX_FILE_BYTES + 1` bytes and refuses the file if it got them; a line past the
//! line bound is neither decoded nor split; the node and edge counts are checked before a node or
//! an edge is stored, and a node or edge past its bound is not stored.
//!
//! # Refusals
//!
//! Any refusal loads nothing: the result is a [`PatchFileError`] naming the line (none for a
//! condition of the whole file) and a [`PatchFileReason`], which names the error of the ESS domain
//! it is an instance of ([`PatchFileReason::spec_error`]).
//!
//! # Precedence
//!
//! ESS answers a command with the first outcome it declares whose guard holds. So a file with
//! several faults is refused with the fault whose error `LoadPatch` (`spec/domains/patch.yaml`)
//! declares first ([`LOAD_PATCH_PRECEDENCE`], [`PatchFileReason::precedence`]); among faults of
//! one rank, the one on the earliest line, and a fault of the whole file (a missing key) after
//! every line. The parser scans every line to find them, with the bounds above still in force.
//!
//! Two faults end the scan instead, because past them no other fault can be found:
//!
//! - [`PatchFileReason::FileTooLarge`] is answered alone, before any line is read: the reader
//!   stopped after `MAX_FILE_BYTES + 1` bytes, so the rest of the file is unseen and the faults
//!   it holds cannot be ranked. Answering a truncated last line instead would rank a fault the
//!   file does not have.
//! - A fault on line 1 is answered alone: the header names the grammar of every later line, so
//!   without a version-1 header no later line has a meaning to be faulty in.
//!
//! A fault is not counted twice: a line that cannot be read (past the line bound or not UTF-8)
//! may have declared a key or a node, so after one no key is reported missing and no parameter
//! line is reported for an undeclared node; after a refused `node` line (including one past the
//! node bound) no parameter line is reported for an undeclared node; a key whose line was refused
//! is not also reported missing. A line with several faults gives the highest-ranked of them.
//!
//! # Canonical form
//!
//! [`write_patch`] emits the header, `sample_rate_hz`, `topology_id`, then each node in
//! identifier order followed by its parameters in name order, then the edges in the order of
//! [`Patch::canonical`](super::Patch::canonical). A value is written by [`format_decimal`], the
//! shortest text that parses back to the same `f64`. Write then parse gives the canonical patch
//! bit for bit; parse then write of a canonical file gives the same bytes.

use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::{
    EdgeDecl, FORMAT_VERSION, NodeDecl, NodeKind, ParameterValue, Patch, PortRate,
    SUPPORTED_SAMPLE_RATE_HZ,
};

/// The first field of the header line.
pub const HEADER: &str = "patchback-patch";

/// The longest line, in bytes without its `\n`. Provisional (OQ-6).
pub const MAX_LINE_BYTES: usize = 256;

/// The largest file, in bytes. Provisional (OQ-6).
pub const MAX_FILE_BYTES: usize = 1 << 20;

/// The most nodes a file may declare. Provisional (OQ-6).
pub const MAX_NODES: usize = 256;

/// The most edges a file may declare. Provisional (OQ-6).
pub const MAX_EDGES: usize = 1024;

/// The errors of `LoadPatch` a parser refusal names, in the order `spec/domains/patch.yaml`
/// declares their outcomes: the precedence of refusals (module documentation).
pub const LOAD_PATCH_PRECEDENCE: [&str; 7] = [
    "patchback.patch.UnknownFormatVersion",
    "patchback.patch.UnsupportedSampleRate",
    "patchback.patch.DuplicateNodeId",
    "patchback.patch.DuplicateKey",
    "patchback.patch.MalformedRecord",
    "patchback.patch.MalformedNumber",
    "patchback.patch.LengthBoundExceeded",
];

const SAMPLE_RATE_KEY: &str = "sample_rate_hz";
const TOPOLOGY_KEY: &str = "topology_id";

/// Why a file is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchFileReason {
    /// The first line is not `patchback-patch <version>`.
    MissingFormatVersion,
    /// The header names a version this parser does not read.
    UnknownFormatVersion { format_version: u32 },
    /// The first field of a line names no record of the format.
    UnknownRecord { record: String },
    /// A line holds no field.
    BlankLine,
    /// A record has more or fewer fields than its grammar.
    WrongFieldCount {
        record: String,
        expected: usize,
        found: usize,
    },
    /// A patch key is absent from the file.
    MissingKey { key: String },
    /// The header or a patch key appears twice.
    DuplicateKey { key: String },
    /// Two nodes share one identifier.
    DuplicateNodeId { node: u32 },
    /// A node sets one parameter twice.
    DuplicateParameter { node: u32, name: String },
    /// A parameter line names a node not declared on an earlier line.
    UnknownParameterNode { node: u32 },
    /// A number is not an integer or finite decimal of the grammar.
    MalformedNumber { token: String },
    /// A parameter name, port name or topology identifier is not of the grammar.
    MalformedName { name: String },
    /// The sample rate is not 48000 Hz.
    UnsupportedSampleRate { sample_rate_hz: u32 },
    /// A port rate token is not `Audio`, `Control` or `Event`.
    UnsupportedPortRate { rate: String },
    /// A node kind token names no [`NodeKind`].
    UnknownNodeKind { kind: String },
    /// A line is not UTF-8.
    NotUtf8,
    /// A line is longer than [`MAX_LINE_BYTES`].
    LineTooLong { bytes: usize },
    /// The file is larger than [`MAX_FILE_BYTES`].
    FileTooLarge,
    /// A node beyond [`MAX_NODES`].
    TooManyNodes,
    /// An edge beyond [`MAX_EDGES`].
    TooManyEdges,
}

impl PatchFileReason {
    /// The error of the ESS domain `patchback.patch` this refusal is an instance of
    /// (`LoadPatch`'s outcomes).
    pub fn spec_error(&self) -> &'static str {
        match self {
            Self::UnknownFormatVersion { .. } => "patchback.patch.UnknownFormatVersion",
            Self::DuplicateKey { .. } | Self::DuplicateParameter { .. } => {
                "patchback.patch.DuplicateKey"
            }
            Self::DuplicateNodeId { .. } => "patchback.patch.DuplicateNodeId",
            Self::MalformedNumber { .. } => "patchback.patch.MalformedNumber",
            Self::UnsupportedSampleRate { .. } => "patchback.patch.UnsupportedSampleRate",
            Self::LineTooLong { .. }
            | Self::FileTooLarge
            | Self::TooManyNodes
            | Self::TooManyEdges => "patchback.patch.LengthBoundExceeded",
            Self::MissingFormatVersion
            | Self::UnknownRecord { .. }
            | Self::BlankLine
            | Self::WrongFieldCount { .. }
            | Self::MissingKey { .. }
            | Self::UnknownParameterNode { .. }
            | Self::MalformedName { .. }
            | Self::UnsupportedPortRate { .. }
            | Self::UnknownNodeKind { .. }
            | Self::NotUtf8 => "patchback.patch.MalformedRecord",
        }
    }

    /// The rank of this refusal: the index of its [`spec_error`](Self::spec_error) in
    /// [`LOAD_PATCH_PRECEDENCE`]; lower is answered first.
    pub fn precedence(&self) -> usize {
        let error = self.spec_error();
        LOAD_PATCH_PRECEDENCE
            .iter()
            .position(|declared| *declared == error)
            .unwrap_or(LOAD_PATCH_PRECEDENCE.len())
    }
}

impl fmt::Display for PatchFileReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFormatVersion => {
                write!(f, "the first line is not `{HEADER} <format version>`")
            }
            Self::UnknownFormatVersion { format_version } => write!(
                f,
                "format version {format_version} is not read; this parser reads version {FORMAT_VERSION}"
            ),
            Self::UnknownRecord { record } => write!(f, "`{record}` is not a record"),
            Self::BlankLine => f.write_str("blank line"),
            Self::WrongFieldCount {
                record,
                expected,
                found,
            } => write!(
                f,
                "a `{record}` record has {expected} fields, this one has {found}"
            ),
            Self::MissingKey { key } => write!(f, "the patch key `{key}` is missing"),
            Self::DuplicateKey { key } => write!(f, "`{key}` appears twice"),
            Self::DuplicateNodeId { node } => write!(f, "node {node} is declared twice"),
            Self::DuplicateParameter { node, name } => {
                write!(f, "node {node} sets parameter `{name}` twice")
            }
            Self::UnknownParameterNode { node } => {
                write!(
                    f,
                    "parameter for node {node}, which no earlier line declares"
                )
            }
            Self::MalformedNumber { token } => {
                write!(f, "`{token}` is not a finite decimal number of the format")
            }
            Self::MalformedName { name } => write!(f, "`{name}` is not a name of the format"),
            Self::UnsupportedSampleRate { sample_rate_hz } => write!(
                f,
                "sample rate {sample_rate_hz} Hz is not supported; only {SUPPORTED_SAMPLE_RATE_HZ} Hz is"
            ),
            Self::UnsupportedPortRate { rate } => {
                write!(f, "`{rate}` is not a port rate (Audio, Control, Event)")
            }
            Self::UnknownNodeKind { kind } => write!(f, "`{kind}` is not a node kind"),
            Self::NotUtf8 => f.write_str("the line is not UTF-8"),
            Self::LineTooLong { bytes } => write!(
                f,
                "the line has {bytes} bytes, more than the bound of {MAX_LINE_BYTES}"
            ),
            Self::FileTooLarge => {
                write!(
                    f,
                    "the file is larger than the bound of {MAX_FILE_BYTES} bytes"
                )
            }
            Self::TooManyNodes => write!(f, "more than {MAX_NODES} nodes"),
            Self::TooManyEdges => write!(f, "more than {MAX_EDGES} edges"),
        }
    }
}

/// A refused `.patch` file: the 1-based line, or none for a condition of the whole file, and the
/// reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchFileError {
    pub line: Option<usize>,
    pub reason: PatchFileReason,
}

impl fmt::Display for PatchFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.reason),
            None => self.reason.fmt(f),
        }
    }
}

impl std::error::Error for PatchFileError {}

/// A `.patch` file that could not be read, or was read and refused.
#[derive(Debug)]
pub enum ReadPatchError {
    Io(io::Error),
    Format(PatchFileError),
}

impl fmt::Display for ReadPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read the patch file: {error}"),
            Self::Format(error) => write!(f, "patch file refused: {error}"),
        }
    }
}

impl std::error::Error for ReadPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Format(error) => Some(error),
        }
    }
}

impl From<io::Error> for ReadPatchError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Parses a `.patch` file held in memory.
pub fn parse_patch(text: &str) -> Result<Patch, PatchFileError> {
    parse_bytes(text.as_bytes())
}

/// Reads and parses a `.patch` file, taking at most `MAX_FILE_BYTES + 1` bytes from `reader`.
pub fn read_patch(reader: impl Read) -> Result<Patch, ReadPatchError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    parse_bytes(&bytes).map_err(ReadPatchError::Format)
}

/// Opens and parses a `.patch` file ([`read_patch`]).
pub fn read_patch_file(path: impl AsRef<Path>) -> Result<Patch, ReadPatchError> {
    read_patch(File::open(path)?)
}

/// Writes the canonical form of `patch`. Each string field (the topology identifier, parameter
/// and port names, kind and rate tokens) is checked against the grammar before it is formatted,
/// and each formatted line is then checked by the parser, so the writer never emits a file the
/// parser refuses; the refusal is returned instead, naming the line of the canonical form.
pub fn write_patch(patch: &Patch) -> Result<String, PatchFileError> {
    let patch = patch.canonical();
    let mut writer = Writer::default();
    writer.line(format_args!("{HEADER} {}", patch.format_version))?;
    writer.line(format_args!("{SAMPLE_RATE_KEY} {}", patch.sample_rate_hz))?;
    let topology = writer.field(topology_id(&patch.topology_id))?;
    writer.line(format_args!("{TOPOLOGY_KEY} {topology}"))?;
    for node in &patch.nodes {
        let kind = writer.field(kind_token(node.kind))?;
        writer.line(format_args!("node {} {kind}", node.id))?;
        for parameter in &node.parameters {
            let name = writer.field(name(&parameter.name))?;
            writer.line(format_args!(
                "param {} {name} {}",
                node.id,
                format_decimal(parameter.value)
            ))?;
        }
    }
    for edge in &patch.edges {
        let from_port = writer.field(name(&edge.from_port))?;
        let from_rate = writer.field(rate_token(edge.from_rate))?;
        let to_port = writer.field(name(&edge.to_port))?;
        let to_rate = writer.field(rate_token(edge.to_rate))?;
        writer.line(format_args!(
            "edge {} {from_port} {from_rate} {} {to_port} {to_rate}",
            edge.from_node, edge.to_node,
        ))?;
    }
    writer.finish()
}

/// Parses a finite decimal of the grammar (module documentation); `None` for any other token.
pub fn parse_decimal(token: &str) -> Option<f64> {
    let bytes = token.as_bytes();
    let mut at = 0;
    let digits = |at: &mut usize| {
        let start = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at > start
    };
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        at += 1;
    }
    if !digits(&mut at) {
        return None;
    }
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        if !digits(&mut at) {
            return None;
        }
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        if !digits(&mut at) {
            return None;
        }
    }
    if at != bytes.len() {
        return None;
    }
    let value: f64 = token.parse().ok()?;
    value.is_finite().then_some(value)
}

/// The shortest decimal text that parses back to `value` bit for bit: the plain form, or the
/// exponent form when it is strictly shorter. Both are Rust's shortest round-trip forms. A
/// non-finite value gives a token [`parse_decimal`] refuses.
pub fn format_decimal(value: f64) -> String {
    let plain = format!("{value}");
    let exponent = format!("{value:e}");
    if exponent.len() < plain.len() {
        exponent
    } else {
        plain
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<Patch, PatchFileError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(PatchFileError {
            line: None,
            reason: PatchFileReason::FileTooLarge,
        });
    }
    let mut parser = Parser::default();
    let mut faults = Faults::default();
    for (index, chunk) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        let line = index + 1;
        let raw = match chunk.strip_suffix(b"\n") {
            Some(content) => content.strip_suffix(b"\r").unwrap_or(content),
            None => chunk,
        };
        if let Err(reason) = parser.line(raw) {
            let error = PatchFileError {
                line: Some(line),
                reason,
            };
            if line == 1 {
                return Err(error);
            }
            faults.note(error);
        }
    }
    parser.finish(faults)
}

/// The fault answered so far: the first-declared, then the earliest.
#[derive(Default)]
struct Faults(Option<PatchFileError>);

impl Faults {
    fn note(&mut self, error: PatchFileError) {
        if self
            .0
            .as_ref()
            .is_none_or(|kept| error.reason.precedence() < kept.reason.precedence())
        {
            self.0 = Some(error);
        }
    }
}

/// The highest-ranked of the faults of one line, the first listed among equals.
fn first_declared(faults: Vec<PatchFileReason>) -> Result<(), PatchFileReason> {
    match faults.into_iter().min_by_key(PatchFileReason::precedence) {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}

/// The value of a field, or none with its fault kept among the line's faults.
fn keep<T>(faults: &mut Vec<PatchFileReason>, field: Result<T, PatchFileReason>) -> Option<T> {
    field.map_err(|reason| faults.push(reason)).ok()
}

/// The canonical writer: each field is checked before it is formatted, and each line goes
/// through the parser before it is kept.
#[derive(Default)]
struct Writer {
    text: String,
    lines: usize,
    parser: Parser,
}

impl Writer {
    /// A field checked against the grammar, refused on the line it would be written to.
    fn field<T>(&self, checked: Result<T, PatchFileReason>) -> Result<T, PatchFileError> {
        checked.map_err(|reason| PatchFileError {
            line: Some(self.lines + 1),
            reason,
        })
    }

    fn line(&mut self, args: fmt::Arguments<'_>) -> Result<(), PatchFileError> {
        let start = self.text.len();
        // Writing to a String cannot fail.
        let _ = self.text.write_fmt(args);
        self.lines += 1;
        let line = self.lines;
        self.parser
            .line(&self.text.as_bytes()[start..])
            .map_err(|reason| PatchFileError {
                line: Some(line),
                reason,
            })?;
        self.text.push('\n');
        Ok(())
    }

    fn finish(self) -> Result<String, PatchFileError> {
        self.parser.finish(Faults::default())?;
        if self.text.len() > MAX_FILE_BYTES {
            return Err(PatchFileError {
                line: None,
                reason: PatchFileReason::FileTooLarge,
            });
        }
        Ok(self.text)
    }
}

#[derive(Default)]
struct Parser {
    format_version: Option<u32>,
    sample_rate_hz: Option<u32>,
    sample_rate_seen: bool,
    topology_id: Option<String>,
    topology_seen: bool,
    nodes: Vec<NodeDecl>,
    node_index: HashMap<u32, usize>,
    parameter_keys: HashSet<(u32, String)>,
    edges: Vec<EdgeDecl>,
    /// A line could not be read, so what it declared is unknown.
    unreadable_line: bool,
    /// A `node` line was refused, so the nodes declared are not all known.
    node_refused: bool,
}

impl Parser {
    fn line(&mut self, raw: &[u8]) -> Result<(), PatchFileReason> {
        if raw.len() > MAX_LINE_BYTES {
            self.unreadable_line = true;
            return Err(PatchFileReason::LineTooLong { bytes: raw.len() });
        }
        let Ok(text) = std::str::from_utf8(raw) else {
            self.unreadable_line = true;
            return Err(PatchFileReason::NotUtf8);
        };
        let fields: Vec<&str> = text.split_ascii_whitespace().collect();
        if self.format_version.is_none() {
            return self.header(&fields);
        }
        let Some(&record) = fields.first() else {
            return Err(PatchFileReason::BlankLine);
        };
        match record {
            HEADER => Err(PatchFileReason::DuplicateKey {
                key: HEADER.to_owned(),
            }),
            SAMPLE_RATE_KEY => self.sample_rate(&fields),
            TOPOLOGY_KEY => self.topology(&fields),
            "node" => {
                let result = self.node(&fields);
                if result.is_err() {
                    self.node_refused = true;
                }
                result
            }
            "param" => self.param(&fields),
            "edge" => self.edge(&fields),
            other => Err(PatchFileReason::UnknownRecord {
                record: other.to_owned(),
            }),
        }
    }

    fn header(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        if fields.first() != Some(&HEADER) || fields.len() < 2 {
            return Err(PatchFileReason::MissingFormatVersion);
        }
        expect_fields(fields, 2)?;
        let format_version = parse_u32(fields[1])?;
        if format_version != FORMAT_VERSION {
            return Err(PatchFileReason::UnknownFormatVersion { format_version });
        }
        self.format_version = Some(format_version);
        Ok(())
    }

    fn sample_rate(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        let mut faults = Vec::new();
        if std::mem::replace(&mut self.sample_rate_seen, true) {
            faults.push(PatchFileReason::DuplicateKey {
                key: SAMPLE_RATE_KEY.to_owned(),
            });
        }
        let value = expect_fields(fields, 2).and_then(|()| parse_u32(fields[1]));
        match value {
            Ok(sample_rate_hz) if sample_rate_hz != SUPPORTED_SAMPLE_RATE_HZ => {
                faults.push(PatchFileReason::UnsupportedSampleRate { sample_rate_hz });
            }
            Ok(sample_rate_hz) if faults.is_empty() => self.sample_rate_hz = Some(sample_rate_hz),
            Ok(_) => {}
            Err(reason) => faults.push(reason),
        }
        first_declared(faults)
    }

    fn topology(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        let mut faults = Vec::new();
        if std::mem::replace(&mut self.topology_seen, true) {
            faults.push(PatchFileReason::DuplicateKey {
                key: TOPOLOGY_KEY.to_owned(),
            });
        }
        match expect_fields(fields, 2).and_then(|()| topology_id(fields[1])) {
            Ok(id) if faults.is_empty() => self.topology_id = Some(id),
            Ok(_) => {}
            Err(reason) => faults.push(reason),
        }
        first_declared(faults)
    }

    fn node(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        expect_fields(fields, 3)?;
        let mut faults = Vec::new();
        let id = keep(&mut faults, parse_u32(fields[1]));
        let kind = NodeKind::from_name(fields[2]);
        if kind.is_none() {
            faults.push(PatchFileReason::UnknownNodeKind {
                kind: fields[2].to_owned(),
            });
        }
        if let Some(id) = id.filter(|id| self.node_index.contains_key(id)) {
            faults.push(PatchFileReason::DuplicateNodeId { node: id });
        }
        let (Some(id), Some(kind), true) = (id, kind, faults.is_empty()) else {
            return first_declared(faults);
        };
        if self.nodes.len() >= MAX_NODES {
            return Err(PatchFileReason::TooManyNodes);
        }
        self.node_index.insert(id, self.nodes.len());
        self.nodes.push(NodeDecl {
            id,
            kind,
            parameters: Vec::new(),
        });
        Ok(())
    }

    fn param(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        expect_fields(fields, 4)?;
        let mut faults = Vec::new();
        let node = keep(&mut faults, parse_u32(fields[1]));
        let name = keep(&mut faults, name(fields[2]));
        let value = parse_decimal(fields[3]);
        if value.is_none() {
            faults.push(PatchFileReason::MalformedNumber {
                token: fields[3].to_owned(),
            });
        }
        let index = node.and_then(|node| self.node_index.get(&node).copied());
        if let (Some(node), None) = (node, index)
            && !self.node_refused
            && !self.unreadable_line
        {
            faults.push(PatchFileReason::UnknownParameterNode { node });
        }
        if let (Some(node), Some(name), Some(_)) = (node, &name, index)
            && self.parameter_keys.contains(&(node, name.clone()))
        {
            faults.push(PatchFileReason::DuplicateParameter {
                node,
                name: name.clone(),
            });
        }
        let (Some(node), Some(name), Some(value), Some(index), true) =
            (node, name, value, index, faults.is_empty())
        else {
            return first_declared(faults);
        };
        self.parameter_keys.insert((node, name.clone()));
        self.nodes[index]
            .parameters
            .push(ParameterValue { name, value });
        Ok(())
    }

    fn edge(&mut self, fields: &[&str]) -> Result<(), PatchFileReason> {
        expect_fields(fields, 7)?;
        let mut faults = Vec::new();
        let from_node = keep(&mut faults, parse_u32(fields[1]));
        let from_port = keep(&mut faults, name(fields[2]));
        let from_rate = keep(&mut faults, port_rate(fields[3]));
        let to_node = keep(&mut faults, parse_u32(fields[4]));
        let to_port = keep(&mut faults, name(fields[5]));
        let to_rate = keep(&mut faults, port_rate(fields[6]));
        let (
            Some(from_node),
            Some(from_port),
            Some(from_rate),
            Some(to_node),
            Some(to_port),
            Some(to_rate),
        ) = (from_node, from_port, from_rate, to_node, to_port, to_rate)
        else {
            return first_declared(faults);
        };
        if self.edges.len() >= MAX_EDGES {
            return Err(PatchFileReason::TooManyEdges);
        }
        self.edges.push(EdgeDecl {
            from_node,
            from_port,
            from_rate,
            to_node,
            to_port,
            to_rate,
        });
        Ok(())
    }

    fn finish(self, mut faults: Faults) -> Result<Patch, PatchFileError> {
        let missing = |key: &str| PatchFileError {
            line: None,
            reason: PatchFileReason::MissingKey {
                key: key.to_owned(),
            },
        };
        let Some(format_version) = self.format_version else {
            return Err(PatchFileError {
                line: Some(1),
                reason: PatchFileReason::MissingFormatVersion,
            });
        };
        if !self.unreadable_line {
            if !self.sample_rate_seen {
                faults.note(missing(SAMPLE_RATE_KEY));
            }
            if !self.topology_seen {
                faults.note(missing(TOPOLOGY_KEY));
            }
        }
        if let Some(error) = faults.0 {
            return Err(error);
        }
        Ok(Patch {
            format_version,
            sample_rate_hz: self
                .sample_rate_hz
                .ok_or_else(|| missing(SAMPLE_RATE_KEY))?,
            topology_id: self.topology_id.ok_or_else(|| missing(TOPOLOGY_KEY))?,
            nodes: self.nodes,
            edges: self.edges,
        })
    }
}

fn expect_fields(fields: &[&str], expected: usize) -> Result<(), PatchFileReason> {
    if fields.len() == expected {
        Ok(())
    } else {
        Err(PatchFileReason::WrongFieldCount {
            record: fields.first().copied().unwrap_or_default().to_owned(),
            expected,
            found: fields.len(),
        })
    }
}

pub(super) fn parse_u32(token: &str) -> Result<u32, PatchFileReason> {
    let malformed = || PatchFileReason::MalformedNumber {
        token: token.to_owned(),
    };
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed());
    }
    token.parse().map_err(|_| malformed())
}

/// A parameter or port name: `[a-z][a-z0-9_]*`.
fn name(token: &str) -> Result<String, PatchFileReason> {
    let mut bytes = token.bytes();
    let well_formed = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if well_formed {
        Ok(token.to_owned())
    } else {
        Err(PatchFileReason::MalformedName {
            name: token.to_owned(),
        })
    }
}

/// A topology identifier: one or more of `[A-Za-z0-9._/-]`.
pub(super) fn topology_id(token: &str) -> Result<String, PatchFileReason> {
    let well_formed = !token.is_empty()
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-'));
    if well_formed {
        Ok(token.to_owned())
    } else {
        Err(PatchFileReason::MalformedName {
            name: token.to_owned(),
        })
    }
}

fn port_rate(token: &str) -> Result<PortRate, PatchFileReason> {
    PortRate::from_name(token).ok_or_else(|| PatchFileReason::UnsupportedPortRate {
        rate: token.to_owned(),
    })
}

/// The token the writer emits for `kind`, checked to read back as `kind`.
fn kind_token(kind: NodeKind) -> Result<&'static str, PatchFileReason> {
    let token = kind.name();
    if NodeKind::from_name(token) == Some(kind) {
        Ok(token)
    } else {
        Err(PatchFileReason::UnknownNodeKind {
            kind: token.to_owned(),
        })
    }
}

/// The token the writer emits for `rate`, checked to read back as `rate`.
fn rate_token(rate: PortRate) -> Result<&'static str, PatchFileReason> {
    let token = rate.name();
    port_rate(token)
        .ok()
        .filter(|read| *read == rate)
        .map(|_| token)
        .ok_or_else(|| PatchFileReason::UnsupportedPortRate {
            rate: token.to_owned(),
        })
}
