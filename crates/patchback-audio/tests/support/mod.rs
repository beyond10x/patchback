//! Hand-built RIFF/WAVE byte buffers. Every fixture is synthesized here; none is downloaded.

#![allow(dead_code)]

/// A 16-byte `fmt ` body whose block align and byte rate agree with the other fields.
pub fn fmt_body(tag: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let block_align = u32::from(channels) * u32::from(bits) / 8;
    let mut body = Vec::new();
    body.extend_from_slice(&tag.to_le_bytes());
    body.extend_from_slice(&channels.to_le_bytes());
    body.extend_from_slice(&rate.to_le_bytes());
    body.extend_from_slice(&(rate * block_align).to_le_bytes());
    body.extend_from_slice(&(block_align as u16).to_le_bytes());
    body.extend_from_slice(&bits.to_le_bytes());
    body
}

/// A chunk with the size its body has, padded to an even length.
pub fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A chunk header that declares `size` whatever follows it.
pub fn chunk_header(id: &[u8; 4], size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(id);
    out.extend_from_slice(&size.to_le_bytes());
    out
}

/// A RIFF/WAVE file around `chunks` whose RIFF size is exact.
pub fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    riff_declaring((body.len() + 4) as u32, &body)
}

/// A RIFF/WAVE file declaring `riff_size` around `body`, whatever its length.
pub fn riff_declaring(riff_size: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(body);
    out
}

pub fn pcm16_data(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

pub fn f32_data(samples: &[f32]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// A complete 48 kHz PCM16 file with `channels` interleaved channels.
pub fn pcm16_wav(channels: u16, samples: &[i16]) -> Vec<u8> {
    riff(&[
        chunk(b"fmt ", &fmt_body(1, channels, 48_000, 16)),
        chunk(b"data", &pcm16_data(samples)),
    ])
}

/// A complete 48 kHz float32 file with `channels` interleaved channels.
pub fn f32_wav(channels: u16, samples: &[f32]) -> Vec<u8> {
    riff(&[
        chunk(b"fmt ", &fmt_body(3, channels, 48_000, 32)),
        chunk(b"data", &f32_data(samples)),
    ])
}
