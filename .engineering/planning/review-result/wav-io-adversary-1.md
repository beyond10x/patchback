---
format: aep.planning-md/3
id: review-result:wav-io-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on WAV import and export
relations:
- reviews: story:wav-io
revision: 1
---
# Adversary pass 1 on story:wav-io

Reviewer: `aep:adversary` (Opus), wave a2, 2026-10-23. Subject: unit commit
`c691926ff70e946335dbe6bec77fe55186426fe2` on `impl/wav-io`.

Verdict: INFEASIBLE (no caller reaches either finding today). Cases executed 98→107, red 2 (in the
new `crates/patchback-audio/tests/adversary.rs`). Origin: introduced 2, pre-existing 0.

```findings
- file: crates/patchback-audio/src/read.rs
  line: 107
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "read_wav sizes input from file metadata, so a complete WAV read through a pipe or /dev/fd path is refused as TruncatedRiff(Header) instead of being read or refused as unsupported input; no caller exists yet."
- file: crates/patchback-audio/src/read.rs
  line: 160
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Non-RIFF input shorter than 12 bytes (OggS, an ID3 header) is reported as TruncatedRiff, which the domain defines as a RIFF or chunk size running past the end of the file, instead of CorruptRiffChunk(NotRiff)."
```

## Routing (coordinator)

- Row 1: correction round. Non-seekable input (a pipe, `/dev/fd/N`) is read sequentially up to the
  allocation and frame bounds, with the same refusals as a regular file; DESIGN.md § 11 names
  `--input` paths that a shell may hand over as a pipe. The adversary's case goes green.
- Row 2: correction round. The `RIFF` magic is compared on whatever bytes exist before the length
  check; short non-RIFF input is `NotRiff`.
