---
format: aep.planning-md/3
id: product-requirements:patchback
kind: product-requirements
status: draft
title: Patchback product requirements
relations:
- serves: vision:patchback
- derived_from: vision:patchback
revision: 1
---
# Product requirements: Patchback

Status: draft. Serves the Patchback vision. Source: DESIGN.md revision 0.2; every requirement cites
its section. Where DESIGN.md is silent the text says "open".

## 1. Problem

A recording of an electronic instrument carries no recipe and no score. Patchback recovers an
editable DSP patch (modules, connections, parameters) and the note events that drive it, renders
them deterministically, and reports how close the rendering is to the recording (DESIGN.md § 1).
The output is a runnable program, not PCM (ADR-005). Sounds are non-identifiable: several programs
can match, so the target is audible match, validity and simplicity (§ 9.3).

## 2. Users and jobs to be done

| User | Job | Source |
| --- | --- | --- |
| Electronic musician | Turn a recording of one synth note, later a phrase, into a patch I can open and tweak | § 1, § 13 |
| Sound designer | Get a starting patch and reconstructed WAV with a report saying how close it is | § 1, § 11 |
| Engineer or researcher | Re-run a fit and get the same result; compare default, ML-init and ML-init-plus-search | § 2, § 13 B |

Which of these users is first is open; DESIGN.md names none.

## 3. Scope per milestone

### Milestone A: sound engine, no ML (§ 13 A, § 15)

| ID | Requirement | Source |
| --- | --- | --- |
| FR-A1 | Read and write mono WAV with `std` only; accept PCM16 or float32 at 48 kHz, stereo downmixed explicitly to mono; validate chunks, sizes, channel counts, frame limits and finite samples | § 3, § 6.1 |
| FR-A2 | Define a patch as typed nodes and edges with stable module and parameter identifiers, ports with explicit rate (audio, control, event) | § 5.1 |
| FR-A3 | Validate the graph before compilation: identifiers, parameters, port rates, cycles, scheduling, buffer assignment | § 5.1, § 2 invariant 3 |
| FR-A4 | Render the fixed subtractive voice (Saw oscillator, drive, lowpass, VCA, ADSR, output gain) from note events, at 48 kHz, block size configurable with default 256 | § 5.2, § 5.3 |
| FR-A5 | Document domain, default, unit, mapping and clamping for each of the eight parameters; implement forward and inverse mappings; clamp to [0,1]; reject NaN and infinity | § 2 invariant 2, § 5.2 |
| FR-A6 | Reject unsupported sample rates without silent conversion | § 5.3 |
| FR-A7 | Report protective output clipping or limiting; never use it silently to hide instability | § 5.3 |
| FR-A8 | Implement radix-2 FFT and inverse FFT, Hann windows, STFT, RMS and peak envelopes internally | § 6.2 |
| FR-A9 | Compute the comparison loss as a weighted sum of log-spectral, envelope, transient and band-energy terms, after time alignment, and log every component per candidate | § 9.1 |
| FR-A10 | Refine a patch by seeded, bounded restarts and coordinate search within the known topology, keep only improvements, preserve the best valid patch, stop on a render budget (up to 2,000 per note) or a threshold | § 9.2 |
| FR-A11 | Read and write versioned, line-oriented `.patch`, `.events` and run metadata; reject unknown versions, duplicate keys or identifiers, malformed numbers, unsupported rates and unbounded lengths | § 11 |
| FR-A12 | Provide the commands `generate-dataset`, `patch validate`, `render`, `analyze`, `fit`, `compare`; a fit run writes `patch.patch`, `notes.events`, `reconstructed.wav`, `report.txt`, `run.meta` | § 11 |
| FR-A13 | Demo: generated reference WAV plus known note events in, recovered patch, reconstructed WAV and report out, with a deterministic loss improvement over the default patch | § 15 |

### Milestone B: native data and ModelSpec M0 (§ 13 B)

| ID | Requirement | Source |
| --- | --- | --- |
| FR-B1 | Freeze `features/1` (32 values: 24 band energies over attack, body, release windows; 8 scalar features) and the eight-parameter mapping before dataset generation | § 6.3, § 13 B |
| FR-B2 | Generate paired `(x: [32], y: [8])` data deterministically with an internal PRNG, recording algorithm and seed; split by patch identity | § 7.1 |
| FR-B3 | Write ModelSpec dense data as row-major little-endian f32 (`x.f32`, `y.f32`, optional `order.u32`), verifying byte counts, finite values, matching counts | § 7.2 |
| FR-B4 | Check and train the fixed-shape MLP regressor on the ModelSpec CPU backend, pinned to a tested version; record loss curves and holdout behaviour | § 8.1, § 13 B |
| FR-B5 | Run `modelspec train` as a subprocess, accept only the documented weight output of the pinned version, validate names, shapes, counts and finiteness, and store a checksummed, versioned f32 artifact | § 8.2 |
| FR-B6 | Run the two dense layers and ReLU in a native predictor, with golden tests against independently computed matmul results | § 8.2 |
| FR-B7 | Save training normalization statistics and reuse them unchanged at inference; test data never affects them | § 6.3 |
| FR-B8 | Clamp out-of-range regressor outputs with an explicit warning and measure how often it happens | § 8.1 |
| FR-B9 | Compare default, ML-init and ML-init-plus-search; report the raw-model-only result | § 12.2, § 13 B |

### Milestone C: from notes to phrases (§ 13 C, § 10)

| ID | Requirement | Source |
| --- | --- | --- |
| FR-C1 | Estimate monophonic onsets, pitch, gate lengths and tails, with confidence and ambiguity diagnostics | § 6.2, § 10 V1 |
| FR-C2 | Emit a sample-timestamped event sequence (`NoteEvent`) and one editable patch for a short bass or lead phrase | § 10, § 13 C |
| FR-C3 | Surface timing and pitch uncertainty and low-confidence refusals instead of guessing | § 6.2, § 13 C |

### Milestone D: module choice and modulation (§ 13 D)

| ID | Requirement | Source |
| --- | --- | --- |
| FR-D1 | Add bounded candidate topologies one module family at a time: Saw, Square, Sine, filter families, LFO, delay | § 5.4, § 13 D |
| FR-D2 | Select topology by candidate rendering and ranking before any unrestricted graph generation | § 13 D, ADR-003 |
| FR-D3 | Add time-varying control automation (`ControlEvent`) with explicit control interpolation | § 5.4, § 10 |
| FR-D4 | Allow a cycle only through a stateful Delay node of at least one sample; limit feedback gain, monitor runaway energy, fail closed | § 5.4 |
| FR-D5 | Penalize complexity (nodes, edges, control routes) and keep a ranked candidate set when solutions sound equivalent | § 9.3 |

### Milestone E: larger musical scope (§ 13 E)

| ID | Requirement | Source |
| --- | --- | --- |
| FR-E1 | Handle one instrument stem with repeated notes, then simple polyphony, then multiple stems and arrangement | § 10 V2, V3 |
| FR-E2 | Add drum and effect modules, repeated-pattern analysis and arrangement events | § 13 E |
| FR-E3 | Specify any sequence-generation model separately from sound reconstruction | § 10, § 13 E |
| FR-E4 | Move supported training to the ModelSpec GPU backend only once upstream ships it and it passes regression tests | § 13 E, ADR-010 |

## 4. Non-functional requirements

| ID | Requirement | Source |
| --- | --- | --- |
| NFR-1 | Determinism: fixed seed, build, input and execution configuration give reproducible offline results within the numerical contract | § 2 invariant 5 |
| NFR-2 | The patch that is output executes in the same renderer used to generate training data | § 2 invariant 1 |
| NFR-3 | Audio processing rejects or safely handles non-finite input and never produces non-finite output | § 2 invariant 4 |
| NFR-4 | Bounds on input duration, graph size, voice count, note count, FFT length, memory and optimizer time | § 12.3 |
| NFR-5 | Reject corrupt models, non-finite features, impossible sample rates, empty audio and untrusted graphs | § 12.3 |
| NFR-6 | Dependency policy: Rust standard library; no ML, tensor, numerical, DSP or serialization crates; at most one audio I/O backend, optional and behind a feature; ModelSpec pinned; CI audits dependency changes | § 3, § 2 invariant 7 |
| NFR-7 | Audio callback code, when added, does not allocate, block, lock or do file or network I/O | § 2 invariant 6, § 12.3 |
| NFR-8 | Provenance: analyze only recordings the project is authorized to use; keep provenance for test fixtures | § 12.3 |
| NFR-9 | Versioned artifacts, feature contracts and numerical behaviour; parsers reject unknown versions | § 2, § 11 |
| NFR-10 | ESS types the contracts in `spec/` and is never a runtime, ML or executable subsystem | § 3, ADR-008 |

## 5. Success metrics

| Metric | Target | Source |
| --- | --- | --- |
| Median normalized audio loss reduction vs default patch, seeded holdout | at least 20%; a go/no-go experiment target, not a guaranteed result | § 12.2 |
| Wall-clock time per fit | recorded with every comparison; no target | § 12.2 |
| Render evaluations per fit | recorded; initial upper bound 2,000 per note, tunable | § 9.2, § 12.2 |
| Listening examples | recorded alongside numerical metrics | § 12.2 |
| Raw-model-only vs default vs search result | all three reported | § 12.2 |
| Held-out sound matching, external audio, dependency gate | evidence per the § 12.2 acceptance table | § 12.2 |
| Fixed sample-rate, build and device configuration when comparing runs | required | § 12.2 |

## 6. Out of scope

- Source separation from a mastered stereo mix (full-mix separation remains a separate research
  problem) (§ 2, § 13 E).
- Polyphonic or multi-instrument transcription, drum recognition, genre-level arrangement in the
  MVP (§ 2).
- Arbitrary user-defined DSP code and arbitrary feedback loops (§ 2).
- Real-time training, low-latency inference, DAW-plugin integration (§ 2).
- Recovering the exact historical patch (§ 2).
- Differentiable synthesis; a separate later research track (§ 4, ADR-002).
- Any service, database, HTTP backend or UI framework in the first release (§ 4.1).
- Live audio I/O until approved (ADR-007).

## 7. Open questions

DESIGN.md leaves these open or marks them as proposals.

| Question | Options | Source |
| --- | --- | --- |
| Which audio I/O backend, if live audio is approved | none (offline only); one backend such as `cpal` behind a feature | § 3, ADR-007 |
| Comparison loss weights | keep 0.45/0.25/0.15/0.15; retune after benchmarking | § 9.1 |
| Parameter ranges and mappings | keep the example contract; change by versioning the mapping | § 5.2 |
| Dataset size and split | 50,016 / 4,992 / 4,992; adjust to measured CPU cost | § 7.1 |
| Model steps and learning rate | 5000 steps, lr 0.01 as starting values; tune as experiment parameters | § 8.1 |
| Optimizer render budget | 2,000 per note; adjust after benchmark | § 9.2 |
| Replace the weight-export bridge | keep until ModelSpec ships a stable inference or checkpoint interface and equivalence tests pass | § 8.2, ADR-006 |
| Interoperable file format instead of the line-oriented one | keep line-oriented; revisit only if the dependency rule changes | § 11 |
| Topology and parameters learned together | fixed graph, then candidate selection, then structured decoder | ADR-003 |
| GPU timing | after documented upstream implementation and CPU/GPU equivalence tests | ADR-010 |
| First user segment and distribution of releases | open; DESIGN.md is silent | n/a |
