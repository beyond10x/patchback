---
format: aep.planning-md/3
id: review-result:patch-graph-validation-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on patch graph validation
relations:
- reviews: story:patch-graph-validation
revision: 1
---
# Adversary pass 1 on story:patch-graph-validation

Reviewer: `aep:adversary` (Opus, effort high), wave a3, 2026-10-23. Subject: the implementor's
uncommitted working tree on `impl/patch-graph-validation`, base
`b8fb69885acbc85631d62aa452fb7290e7fa6831`.

Verdict: NEEDS-CHANGE. Cases executed 313→323, red 6 (in the new
`crates/patchback-core/tests/patch_graph_validation_adversary.rs` and
`crates/patchback-cli/tests/patch_validate_adversary.rs`); one more case kills a mutant the suite
let survive. Origin: introduced 5, pre-existing 0.

```findings
- file: crates/patchback-core/src/patch/validate.rs
  line: 160
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "known_topology matches the TopologyName while spec/domains/patch.yaml types Patch.topology_id as the Uuid identity, so every acceptance scenario's patch as written is refused with UnknownTopology."
- file: crates/patchback-core/src/patch/validate.rs
  line: 436
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A duplicated edge (DESIGN.md 12.1 duplicates) validates and compiles to two inputs reading one buffer on one port, and the CLI exits 0 on such a file."
- file: crates/patchback-cli/src/patch_validate.rs
  line: 43
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "A file with an unknown topology or a kind outside the catalogue plus a malformed number is refused as MalformedNumber, against LoadPatch's declared order."
- file: crates/patchback-core/src/patch/compile.rs
  line: 173
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Removing the input port-order sort leaves all 38 patch_graph_validation tests green."
- file: crates/patchback-core/src/patch/validate.rs
  line: 390
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The IncompatiblePortRates refusal prints an Control port cannot feed a Audio port."
```

## Routing (coordinator)

- Row 1: a `.patch` file names its topology by `TopologyName` (`subtractive-mono/1`), as DESIGN.md
  § 5.1 (`topology_id: String`) and § 11 (human-readable) say; the Uuid is the model's identity.
  `spec/domains/patch.yaml` states that mapping at `LoadPatch`. The adversary's first case is
  rewritten to that rule: the declared name validates, a Uuid in the file is UnknownTopology.
- Row 2: an input port takes at most one edge. A duplicated edge and two sources into one input
  are both refused with one new error; DESIGN.md § 5.1 states the rule (revision 0.6).
- Row 3: loading reports `LoadPatch`'s declared order, the topology and catalogue outcomes
  included, through one load function the CLI calls.
- Row 4: the case stays. Row 5: the refusal text names the rates without an article.
