---
format: aep.planning-md/3
id: review-result:patch-file-format-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the .patch file format
relations:
- reviews: story:patch-file-format
revision: 1
---
# Adversary pass 1 on story:patch-file-format

Reviewer: `aep:adversary` (Opus, effort high), wave a3, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/patch-file-format`, base `744c25033a0feb240cdbe92ac2a22da1599d27d1`.

Verdict: CONFIRMED. Cases executed 246→255, red 6 (in the new
`crates/patchback-core/tests/patch_file_format_adversary.rs`); 3 more cases are green but kill
mutants the unit's own suite let survive. Origin: introduced 9, pre-existing 0.

```findings
- file: crates/patchback-core/src/patch/file.rs
  line: 414
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "write_patch checks re-split tokens instead of field values, so whitespace or newlines in topology_id, parameter or port names produce output the parser refuses or reads back as a different patch."
- file: crates/patchback-core/src/patch/file.rs
  line: 449
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The line bound counts the CR of a CRLF line, so a file at MAX_LINE_BYTES loads with LF and is refused with CRLF although the module says CR is ignored."
- file: crates/patchback-core/src/patch/file.rs
  line: 137
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "For files with two faults the parser answers by line order while LoadPatch in spec/domains/patch.yaml answers by declaration order (DuplicateNodeId and UnsupportedSampleRate before DuplicateKey)."
- file: crates/patchback-core/src/patch/file.rs
  line: 381
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Changing > to >= on the file bound survives the unit suite; no test loaded a file of exactly MAX_FILE_BYTES."
- file: crates/patchback-core/src/patch/file.rs
  line: 426
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Removing the writer's file-size check survives the unit suite; no test wrote a patch whose canonical form exceeds MAX_FILE_BYTES."
- file: crates/patchback-core/src/patch.rs
  line: 161
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Dropping from_rate and the to-end from the canonical edge key survives the unit suite; no test ordered edges that share a from-end."
```

## Routing (coordinator)

- Row 1: correction round. The writer checks each string field against the parser's grammar before
  formatting and refuses with a typed reason; write then parse stays identity.
- Row 2: correction round. The line bound counts bytes without the line terminator, as the module
  doc says.
- Row 3: correction round. The parser reports the refusal `LoadPatch` declares first among the
  faults it finds, as ESS precedence is declaration order; scanning stays bounded.
- Rows 4–6: the adversary's cases stay in the suite and hold the mutants.
