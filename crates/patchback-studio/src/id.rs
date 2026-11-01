//! The identities of an export (approval-record:wave-c1, Dc1-24): version-8 UUIDs (RFC 9562,
//! custom) made from the SHA-256 of a domain tag and the bytes they name, so the same run's
//! files always give the same identities and an export is byte-reproducible.
//!
//! The digest is of the tag's UTF-8 bytes, a zero byte, then each part as its length in bytes
//! (eight bytes, little-endian) followed by the part, so no two different lists of parts give
//! one message. Its first 16 bytes are the UUID, with the version nibble set to 8 and the
//! variant bits to `10`.

use patchback_core::digest::Sha256;

/// The tag of a patch's identity: the bytes of its `.patch` file.
pub const PATCH: &str = "patchback.patch.Patch";
/// The tag of a topology's identity: its name.
pub const TOPOLOGY: &str = "patchback.patch.Topology";
/// The tag of a drawing's identity: the bytes of the patch file it draws.
pub const GRAPH_VIEW: &str = "patchback.studio.GraphView";
/// The tag of a fit run's identity.
pub const FIT_RUN: &str = "patchback.fit.FitRun";
/// The tag of a trace's identity.
pub const TRACE: &str = "patchback.studio.EvolutionTrace";
/// The tag of a frame's identity.
pub const FRAME: &str = "patchback.studio.TraceFrame";

/// The version-8 UUID of `parts` under `tag` (module documentation), lower-case and hyphenated.
pub fn derive(tag: &str, parts: &[&[u8]]) -> String {
    let mut digest = Sha256::new();
    digest.update(tag.as_bytes());
    digest.update(&[0]);
    for part in parts {
        digest.update(&(part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    let hash = digest.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (at, byte) in bytes.iter().enumerate() {
        if matches!(at, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_derived_identity_is_a_version_8_uuid_and_depends_on_tag_and_parts() {
        let id = derive(PATCH, &[b"abc"]);
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "8");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"), "{id}");
        assert_eq!(id, derive(PATCH, &[b"abc"]));
        assert_ne!(id, derive(TOPOLOGY, &[b"abc"]));
        assert_ne!(derive(PATCH, &[b"ab", b"c"]), derive(PATCH, &[b"a", b"bc"]));
    }
}
