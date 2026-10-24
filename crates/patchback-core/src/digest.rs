//! SHA-256 (FIPS 180-4) in `std` only, for the digests a fit run records of its inputs
//! (DESIGN.md § 11, `run.meta`: "inputs digests") and for anything else that names a file by
//! its content. `tests/digest.rs` holds the published reference digests (the empty message,
//! `abc`, the two-block message and one million `a`).
//!
//! [`sha256`] digests a whole message; [`Sha256`] digests one in parts and gives the same
//! result. [`hex`] prints a digest as 64 lowercase hexadecimal digits, the form a file records.

/// The digest's name as a file records it, before its hexadecimal value.
pub const NAME: &str = "sha256";

/// The bytes of a digest.
pub const DIGEST_BYTES: usize = 32;

/// The round constants: the first 32 bits of the fractional parts of the cube roots of the
/// first 64 primes.
const K: [u32; 64] = [
    0x428A_2F98,
    0x7137_4491,
    0xB5C0_FBCF,
    0xE9B5_DBA5,
    0x3956_C25B,
    0x59F1_11F1,
    0x923F_82A4,
    0xAB1C_5ED5,
    0xD807_AA98,
    0x1283_5B01,
    0x2431_85BE,
    0x550C_7DC3,
    0x72BE_5D74,
    0x80DE_B1FE,
    0x9BDC_06A7,
    0xC19B_F174,
    0xE49B_69C1,
    0xEFBE_4786,
    0x0FC1_9DC6,
    0x240C_A1CC,
    0x2DE9_2C6F,
    0x4A74_84AA,
    0x5CB0_A9DC,
    0x76F9_88DA,
    0x983E_5152,
    0xA831_C66D,
    0xB003_27C8,
    0xBF59_7FC7,
    0xC6E0_0BF3,
    0xD5A7_9147,
    0x06CA_6351,
    0x1429_2967,
    0x27B7_0A85,
    0x2E1B_2138,
    0x4D2C_6DFC,
    0x5338_0D13,
    0x650A_7354,
    0x766A_0ABB,
    0x81C2_C92E,
    0x9272_2C85,
    0xA2BF_E8A1,
    0xA81A_664B,
    0xC24B_8B70,
    0xC76C_51A3,
    0xD192_E819,
    0xD699_0624,
    0xF40E_3585,
    0x106A_A070,
    0x19A4_C116,
    0x1E37_6C08,
    0x2748_774C,
    0x34B0_BCB5,
    0x391C_0CB3,
    0x4ED8_AA4A,
    0x5B9C_CA4F,
    0x682E_6FF3,
    0x748F_82EE,
    0x78A5_636F,
    0x84C8_7814,
    0x8CC7_0208,
    0x90BE_FFFA,
    0xA450_6CEB,
    0xBEF9_A3F7,
    0xC671_78F2,
];

/// The initial hash value: the first 32 bits of the fractional parts of the square roots of the
/// first 8 primes.
const H0: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// A message block.
const BLOCK_BYTES: usize = 64;

/// A SHA-256 digest in progress: feed it with [`Sha256::update`], read it with
/// [`Sha256::finalize`].
#[derive(Clone, Debug)]
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; BLOCK_BYTES],
    block_len: usize,
    message_bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// The digest of the empty message so far.
    pub const fn new() -> Self {
        Self {
            state: H0,
            block: [0; BLOCK_BYTES],
            block_len: 0,
            message_bytes: 0,
        }
    }

    /// Appends `bytes` to the message.
    pub fn update(&mut self, bytes: &[u8]) {
        self.message_bytes = self.message_bytes.wrapping_add(bytes.len() as u64);
        let mut rest = bytes;
        if self.block_len > 0 {
            let take = rest.len().min(BLOCK_BYTES - self.block_len);
            self.block[self.block_len..self.block_len + take].copy_from_slice(&rest[..take]);
            self.block_len += take;
            rest = &rest[take..];
            if self.block_len < BLOCK_BYTES {
                return;
            }
            compress(&mut self.state, &self.block);
            self.block_len = 0;
        }
        let (blocks, tail) = rest.as_chunks::<BLOCK_BYTES>();
        for block in blocks {
            compress(&mut self.state, block);
        }
        self.block[..tail.len()].copy_from_slice(tail);
        self.block_len = tail.len();
    }

    /// The digest of the message fed so far.
    pub fn finalize(mut self) -> [u8; DIGEST_BYTES] {
        let bit_length = self.message_bytes.wrapping_mul(8);
        self.block[self.block_len] = 0x80;
        self.block_len += 1;
        if self.block_len > BLOCK_BYTES - 8 {
            self.block[self.block_len..].fill(0);
            compress(&mut self.state, &self.block);
            self.block_len = 0;
        }
        self.block[self.block_len..BLOCK_BYTES - 8].fill(0);
        self.block[BLOCK_BYTES - 8..].copy_from_slice(&bit_length.to_be_bytes());
        compress(&mut self.state, &self.block);
        let mut digest = [0; DIGEST_BYTES];
        for (bytes, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *bytes = word.to_be_bytes();
        }
        digest
    }
}

/// One compression of `block` into `state`.
fn compress(state: &mut [u32; 8], block: &[u8; BLOCK_BYTES]) {
    let mut schedule = [0u32; 64];
    for (word, bytes) in schedule.iter_mut().zip(block.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*bytes);
    }
    for t in 16..64 {
        let w15 = schedule[t - 15];
        let w2 = schedule[t - 2];
        let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
        let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
        schedule[t] = schedule[t - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[t - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (k, w) in K.iter().zip(schedule) {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(choice)
            .wrapping_add(*k)
            .wrapping_add(w);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(value);
    }
}

/// The SHA-256 digest of `message`.
pub fn sha256(message: &[u8]) -> [u8; DIGEST_BYTES] {
    let mut digest = Sha256::new();
    digest.update(message);
    digest.finalize()
}

/// `digest` as 64 lowercase hexadecimal digits.
pub fn hex(digest: &[u8; DIGEST_BYTES]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(DIGEST_BYTES * 2);
    for byte in digest {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 0x0F)] as char);
    }
    text
}
