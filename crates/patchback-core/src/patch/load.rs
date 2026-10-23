//! Loading a `.patch` file the way `patchback.patch.LoadPatch` then `ValidatePatch` answer it
//! (`spec/domains/patch.yaml`): one function, [`load_patch_file`], reads the file, parses it,
//! checks the graph and returns the [`ValidatedPatch`], or the one refusal those commands declare
//! first.
//!
//! # Precedence
//!
//! `LoadPatch` declares its outcomes in this order: unknown version, unsupported rate, duplicate
//! node identifier, duplicate key, **unknown topology, kind outside the catalogue**, malformed
//! record, malformed number, length bound. The parser ([`read_patch`]) ranks every fault but the
//! two in bold, which are about what the file names rather than how it is written. So when the
//! parser refuses a file with a fault ranked after them, the lines it could read are scanned for
//! the topology and the node kinds, and an unknown topology, then a kind outside its catalogue
//! (the lowest node identifier first), is answered instead.
//!
//! The parser's exceptions stand: a file past [`MAX_FILE_BYTES`] and a fault on line 1 are
//! answered alone. A line the parser refused is not read for a topology or a kind (a fault is not
//! counted twice), and without a readable `topology_id` line there is no topology to check.
//!
//! A file the parser accepts goes through [`validate`], which runs the same checks in the same
//! order, then `ValidatePatch`'s.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use super::file::{parse_u32, topology_id};
use super::validate::admits;
use super::{
    LOAD_PATCH_PRECEDENCE, MAX_FILE_BYTES, MAX_LINE_BYTES, NodeKind, PatchFileError,
    PatchFileReason, ReadPatchError, ValidatedPatch, ValidationError, known_topology, read_patch,
    validate,
};

/// Why a `.patch` file did not load.
#[derive(Debug)]
pub enum LoadPatchError {
    /// The file could not be read.
    Io(io::Error),
    /// The parser refused the file ([`read_patch`]); no fault the file names outranks it.
    File(PatchFileError),
    /// The topology or a node kind the file names, or the graph, is refused ([`validate`]).
    Graph(ValidationError),
}

impl LoadPatchError {
    /// The error of the ESS domain `patchback.patch` the refusal is an instance of; none for a
    /// file that could not be read.
    pub fn spec_error(&self) -> Option<&'static str> {
        match self {
            Self::Io(_) => None,
            Self::File(error) => Some(error.reason.spec_error()),
            Self::Graph(error) => Some(error.spec_error()),
        }
    }
}

impl fmt::Display for LoadPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::File(error) => error.fmt(f),
            Self::Graph(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LoadPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::File(error) => Some(error),
            Self::Graph(error) => Some(error),
        }
    }
}

impl From<io::Error> for LoadPatchError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Opens, parses and validates a `.patch` file ([`load_patch`]).
pub fn load_patch_file(path: impl AsRef<Path>) -> Result<ValidatedPatch, LoadPatchError> {
    load_patch(File::open(path)?)
}

/// Reads at most `MAX_FILE_BYTES + 1` bytes from `reader`, parses and validates them, and answers
/// with the refusal `LoadPatch` then `ValidatePatch` declare first (module documentation).
pub fn load_patch(reader: impl Read) -> Result<ValidatedPatch, LoadPatchError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    match read_patch(bytes.as_slice()) {
        Ok(patch) => validate(&patch).map_err(LoadPatchError::Graph),
        Err(ReadPatchError::Io(error)) => Err(LoadPatchError::Io(error)),
        Err(ReadPatchError::Format(error)) => Err(match outranking(&bytes, &error) {
            Some(graph) => LoadPatchError::Graph(graph),
            None => LoadPatchError::File(error),
        }),
    }
}

/// The unknown topology or the kind outside the catalogue the file names, when `LoadPatch`
/// declares it before the parser's `refusal`.
fn outranking(bytes: &[u8], refusal: &PatchFileError) -> Option<ValidationError> {
    let malformed_record = LOAD_PATCH_PRECEDENCE
        .iter()
        .position(|error| *error == "patchback.patch.MalformedRecord")
        .unwrap_or(LOAD_PATCH_PRECEDENCE.len());
    if refusal.reason == PatchFileReason::FileTooLarge
        || refusal.line == Some(1)
        || refusal.reason.precedence() < malformed_record
    {
        return None;
    }
    let mut topology: Option<String> = None;
    let mut kinds: Vec<(u32, NodeKind)> = Vec::new();
    for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if index == 0 || line.len() > MAX_LINE_BYTES {
            continue;
        }
        let Ok(text) = std::str::from_utf8(line) else {
            continue;
        };
        let fields: Vec<&str> = text.split_ascii_whitespace().collect();
        match fields.as_slice() {
            ["topology_id", token] => {
                if let Ok(name) = topology_id(token) {
                    topology.get_or_insert(name);
                }
            }
            ["node", id, kind] => {
                if let (Ok(id), Some(kind)) = (parse_u32(id), NodeKind::from_name(kind)) {
                    kinds.push((id, kind));
                }
            }
            _ => {}
        }
    }
    let name = topology?;
    let Some(known) = known_topology(&name) else {
        return Some(ValidationError::UnknownTopology { topology_id: name });
    };
    kinds
        .into_iter()
        .filter(|(_, kind)| !admits(&known, *kind))
        .min_by_key(|(id, _)| *id)
        .map(|(node, kind)| ValidationError::KindOutsideCatalogue { node, kind })
}
