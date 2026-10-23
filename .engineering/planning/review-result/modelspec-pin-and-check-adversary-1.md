---
format: aep.planning-md/3
id: review-result:modelspec-pin-and-check-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on the ModelSpec pin and check
relations:
- reviews: story:modelspec-pin-and-check
revision: 1
---
# Adversary pass 1 on story:modelspec-pin-and-check

Reviewer: `aep:adversary` (Opus), wave a2, 2026-10-23. Subject: unit commit
`3fe33595e10ed10fde68c3341b8d376e28387e27` on `impl/modelspec-pin-and-check`.

Verdict: CONFIRMED. Cases executed 78→83, red 3 (all in the new
`crates/patchback-model/tests/adversary.rs`). Origin: introduced 6, pre-existing 0.

```findings
- file: crates/patchback-model/src/lib.rs
  line: 333
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "check() maps every nonzero exit to the terminal Rejected state, and the pinned 0.5.0 binary exits 1 for a source path that does not exist, so a wrong working directory or typo permanently records the model as refused by ModelSpec; fix by refusing a missing source before spawning (a typed error leaving it Unchecked)."
- file: crates/patchback-model/src/lib.rs
  line: 342
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a check process ended by a signal (code None) is recorded as the terminal Rejected verdict instead of a ModelSpecError leaving the source Unchecked; red in adversary.rs, but nothing outside the crate's tests calls check yet."
- file: crates/patchback-model/src/lib.rs
  line: 239
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a bare name in the explicit path or MODELSPEC is accepted because it exists relative to the working directory, but Command::new resolves a bare name through PATH, so the adapter runs a different file than the one it located; prefixing ./ to a slash-free relative path would fix it, and no caller passes a bare name today."
- file: crates/patchback-model/src/lib.rs
  line: 261
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "the modelspec subprocess runs with no timeout and unbounded output capture, so a hung binary blocks the caller forever; DESIGN.md asks for no timeout on check, and nothing calls this crate yet."
- file: crates/patchback-model/src/lib.rs
  line: 298
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "ModelSpec::open accepts any caller-built ModelSpecPin, so a non-repository release can produce a trainable CheckedModel; the doc line 'so no other release runs' holds only with ModelSpecPin::repository(), and PinMismatch must be enforced by the future training story."
- file: crates/patchback-model/src/lib.rs
  line: 439
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the domain's RegisterModelSource takes model_spec_pin_id and the rejected scenario decides UnknownPin and UnknownModelSource, but ModelSource::register takes no pin and the code has neither error; the pin file also lacks vulkan_backend_shipped."
```

## Routing (coordinator)

- Rows 1–3: correction round. A missing or unreadable source is a typed error before spawning and
  leaves the source Unchecked; a check ended by a signal is a ModelSpec error leaving it Unchecked;
  a slash-free relative binary path is run as `./<name>`. The adversary's three red cases go green.
- Row 4: correction round, coordinator decision. `check` gets a bounded wait (a documented default,
  overridable by the caller) implemented with std only; a timeout kills the child and is a typed
  error leaving the source Unchecked. Output capture stays unbounded: the binary is pinned and
  checksummed.
- Row 5: correction round for the doc only. The doc states that `open` runs the release of the pin
  it is given and that `ModelSpecPin::repository()` is the repository's pin; enforcing the pin at
  training is story:training-run's (PinMismatch).
- Row 6: correction round, partial. `ModelSource::register` takes the pin it is checked against and
  the checked model records it. `UnknownPin` and `UnknownModelSource` are lookups in a store that
  does not exist yet; they stay domain-only until one does. `vulkan_backend_shipped` is not added
  to the pin file: whether the 0.5.0 Linux binary ships Vulkan is unknown, so the pin file records
  no value (UNMAPPED in the domain).
