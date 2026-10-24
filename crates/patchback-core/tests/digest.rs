//! The reference digests of SHA-256 (FIPS 180-4 and its published examples), the agreement of
//! the one-shot and the incremental forms, and the hexadecimal form.

use patchback_core::digest::{DIGEST_BYTES, NAME, Sha256, hex, sha256};

/// FIPS 180-4 § B.1: the digest of `abc`.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// FIPS 180-4 § B.2: the digest of the 56-byte two-block message.
const TWO_BLOCKS: &str = "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1";

/// The digest of the empty message.
const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// FIPS 180-4 § B.3: the digest of one million `a`.
const MILLION_A: &str = "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";

#[test]
fn the_fips_examples_give_their_published_digests() {
    assert_eq!(hex(&sha256(b"abc")), ABC);
    assert_eq!(
        hex(&sha256(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        TWO_BLOCKS
    );
    assert_eq!(hex(&sha256(b"")), EMPTY);
    let million = vec![b'a'; 1_000_000];
    assert_eq!(hex(&sha256(&million)), MILLION_A);
}

#[test]
fn feeding_the_message_in_parts_gives_the_one_shot_digest() {
    let message: Vec<u8> = (0..=255u8).cycle().take(1_000).collect();
    let whole = sha256(&message);
    for part in [1usize, 3, 55, 56, 63, 64, 65, 127, 128, 999] {
        let mut digest = Sha256::new();
        for chunk in message.chunks(part) {
            digest.update(chunk);
        }
        assert_eq!(digest.finalize(), whole, "part size {part}");
    }
    let mut empty = Sha256::new();
    empty.update(b"");
    assert_eq!(empty.finalize(), sha256(b""));
}

#[test]
fn every_padding_boundary_matches_the_one_shot_form() {
    for length in 0..=130usize {
        let message = vec![0x5Au8; length];
        let mut digest = Sha256::new();
        digest.update(&message[..length / 2]);
        digest.update(&message[length / 2..]);
        assert_eq!(digest.finalize(), sha256(&message), "length {length}");
    }
}

#[test]
fn the_hexadecimal_form_is_64_lowercase_digits_of_the_32_bytes() {
    let mut digest = [0u8; DIGEST_BYTES];
    digest[0] = 0xAB;
    digest[31] = 0x0F;
    let text = hex(&digest);
    assert_eq!(text.len(), 64);
    assert!(text.starts_with("ab00"));
    assert!(text.ends_with("000f"));
    assert_eq!(NAME, "sha256");
    assert_eq!(Sha256::default().finalize(), sha256(b""));
}
