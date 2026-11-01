---
format: aep.planning-md/3
id: review-result:studio-replay-adversary-2
kind: review-result
status: active
title: Adversary pass 2 on the studio patch graph and fit replay
relations:
- reviews: story:studio-patch-graph
- reviews: story:studio-fit-replay
revision: 1
---
unit: story:studio-patch-graph + story:studio-fit-replay, commits 846c30e + 9b150d1 (base 1623311), working tree of patchback-impl-studio-replay
verdict: red
cases: executed 63→72, red 8
origin: introduced 8 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path, my scratch directory `the adversary scratch directory c1/studio-replay/adv2/`
needs-coordinator: yes. Only CI can show whether `studio-fixtures-check` gives the same bytes on another machine. The corepack hash check was not run, because this machine has no corepack and no network.

Eight new cases fail and two pass. Six failures are in the exporter: correction 1's new refusal mapping is incomplete, and on hand-made `trace.events` files the error order disagrees with the domain. The other two are in the studio: every export breaks the ESS Decimal pattern, and the loss chart crashes on a very long run.

## 1. `git --no-pager diff --stat`

Empty: no tracked file changed. `git status --short` shows two new files, both tests:
```
?? crates/patchback-studio/tests/adversary_2.rs
?? studio/src/schema.adv2.test.ts
```
I also briefly created `studio/src/probe.adv2.test.ts` (a stack-trace probe) and deleted it.

## 2. Cases added, each run alone before the suite

`cargo test --locked -p patchback-studio --test adversary_2` exited 101 with 6 failed. Each case, with the line it printed:

- **`adversary_2_a_negative_sequence_is_invariant`** (red). DESIGN.md § 16 says a sequence below 1 is `invariant` in both the exporter and the loader. The exporter says `trace`:
  `assertion left == right failed: trace: trace.events: line 3: "-1" is not a number  left: "trace" right: "invariant"`
- **`adversary_2_a_negative_restart_is_invariant`** (red). The commit claims `restart >= 0` is `invariant` in both. The exporter says `trace`, with the same message.
- **`adversary_2_a_rejected_frame_with_a_loss_is_inconsistent_before_out_of_order`** (red). In the domain, `AppendFrame` tries `rejected-with-loss` before `out-of-order`, and the loader (`document.ts:341`) does too:
  `patchback.studio.FrameOutOfOrder: ... the frame of sequence 2 is not above every sequence the trace holds: line 5 ...  left: "patchback.studio.FrameOutOfOrder" right: "patchback.studio.InconsistentFrame"`
- **`adversary_2_a_retained_frame_whose_loss_is_not_its_best_is_inconsistent_before_best_worsened`** (red). The domain tries `retained-not-best` before `best-worsened`:
  `patchback.studio.BestLossWorsened: ... line 4: the best loss 0.9 is above the previous line's 0.645; it never rises  left: "patchback.studio.BestLossWorsened" right: "patchback.studio.InconsistentFrame"`
- **`adversary_2_a_repeated_sequence_is_out_of_order_whatever_the_field_separator`** (red). With spaces the repeated sequence is `FrameOutOfOrder`. With a tab after `evaluation`, which `parse_trace` accepts:
  `trace: trace.events: line 5: expected sequence numbers 1, 2, 3 and on, in order  left: "trace" right: "patchback.studio.FrameOutOfOrder"`
- **`adversary_2_a_trace_of_another_topology_is_run_before_its_grammar`** (red). The module docs say refusal 4 (`run`) comes before refusal 7 (`trace`):
  `trace: trace.events: the trace ends before its completed line  left: "trace" right: "run"`

`pnpm exec vitest run src/schema.adv2.test.ts` exited 1 with 2 failed and 1 passed:

- **`every member of the committed exports validates against studio/src/generated/source.schema.json`** (red). It checks both exports against `source.schema.json`. The only violations are three decimals in exponent form:
  ```
  "modules[4].parameters[0].physical_at_zero: \"1e-3\" does not match ^-?(0|[1-9][0-9]*)(\\.[0-9]+)?$"
  "modules[4].parameters[1].physical_at_zero: \"5e-3\" does not match ..."
  "modules[4].parameters[3].physical_at_zero: \"5e-3\" does not match ..."
  ```
  `sample-run.json` and `export-replayed-lines.json` show the same three.
- **`draws 130 000 frames without an error`** (red). It fails with `RangeError: Maximum call stack size exceeded`. The probe's stack trace puts it at `LossChart.vue:20:18`, where `Math.max(0, ...props.frames.map(...))` spreads every frame into one call. Node's limit is between 100 000 frames (works) and 130 000 (fails).
- **`is drawn by the evolution view and the result view without an error`** (green). A replay where every frame was rejected draws in both views.

## 3. Suite runs, after the cases existed

- `cargo test --locked --no-fail-fast -p patchback-studio` exited 101.
  - Per target: lib 1 passed, main 0, adversary_1 2 passed, adversary_2 6 failed, fit_replay 17 passed, patch_graph 12 passed.
  - Re-run without my file (`--lib --bins --test adversary_1 --test fit_replay --test patch_graph`): 32 passed, exit 0.
- `pnpm test` exited 1: `Test Files 1 failed | 6 passed (7)`, `Tests 2 failed | 32 passed (34)`.
  - Re-run without my file (`--exclude 'src/**/*.adv2.test.ts'`): `Tests 31 passed (31)`, exit 0.
- `task studio-fixtures-check` exited 0 in 77 s (the fit itself took 76.3 s, 400 evaluations, `RenderBudgetExhausted`). The fresh export matched `sample-run.json` byte for byte, and the task removed its own temporary directory.
  - A first attempt exited 127 before running anything because `/usr/bin/time` does not exist here.

Totals: 63 cases before, 72 after, 8 red.

## 4. Findings (tree 9b150d1 plus my two test files)

| file:line | verdict | origin | what was measured | what reaches it |
|---|---|---|---|---|
| `crates/patchback-studio/src/export.rs:328` | CONFIRMED | introduced | A negative sequence or restart is answered `trace` (the number fails to parse), not `invariant`. This contradicts DESIGN.md § 16 and the commit message; the loader's `bounded` says `invariant`. | A hand-edited `trace.events` |
| `crates/patchback-studio/src/export.rs:307` | CONFIRMED | introduced | `parse_trace` checks the sequence before the `-` marker, so a rejected line that carries a loss and repeats a sequence is `FrameOutOfOrder`. Domain and loader say `InconsistentFrame`. | Hand-made trace files only |
| `crates/patchback-studio/src/export.rs:312` | CONFIRMED | introduced | A retained line whose loss is not its best, with that best rising, is `BestLossWorsened`. The total-equals-best check only runs in `Replay::append`, after parsing succeeds. Domain and loader say `InconsistentFrame`. | Hand-made trace files only |
| `crates/patchback-studio/src/export.rs:342` | CONFIRMED | introduced | `held_sequence` only counts lines that start with `"evaluation "`, but `parse_trace` splits on any ASCII whitespace. With tabs, a repeated sequence becomes `trace`. | A hand-written trace; the fit always writes single spaces |
| `crates/patchback-studio/src/export.rs:174` | CONFIRMED | introduced | The module docs promise refusal 4 (`run`, another topology) before 7 (`trace`). The code parses the trace first, so an unclosed trace of another topology is `trace`. | Mismatched run files; fixing the doc order is enough |
| `crates/patchback-studio/src/graph.rs:273` | NEEDS-CHANGE | introduced | `format_decimal` picks exponent form whenever it is shorter (`1e-3`, `5e-3`), which breaks the ESS Decimal pattern in `source.schema.json`. Losses below about 1e-3 would break it too. The loader's `decimal()` (`document.ts:106`) accepts exponents, so nothing noticed. | Every export, including the bundled sample. Likely fix: write plain `format!("{v}")` in the exporter (Rust's `Display` round-trips and never uses an exponent) and make the loader enforce the schema pattern |
| `studio/src/components/LossChart.vue:20` | INFEASIBLE | introduced | Spreading the frames into `Math.max` throws a `RangeError` at 130 000 frames. | `--max-renders` accepts that many, but the default is 2000 and no run was shown to reach 130 000 evaluations. The line exists at the base, but only the new file picker can feed it a long run |
| `crates/patchback-studio/tests/fit_replay.rs:492` | INFEASIBLE | introduced | The fixture is blessed when `PATCHBACK_STUDIO_BLESS` is set to anything, including `0` or empty. The test then compares the fixture with itself and cannot fail. | `task check` blesses only if the calling shell exports the variable. Nothing in `Taskfile.yml` or the workflows sets it, and this shell does not have it |

## 5. Attacked and not broken

- **pnpm integrity:**
  - The hex in `packageManager` is exactly the coordinator's base64 integrity decoded (`bd682d5d…6f03b`).
  - The `sed` pattern requires `+sha512.` followed by 128 lowercase hex digits; anything else leaves the spec empty and the step exits 1.
  - From my reading of corepack's source, a spec that carries a hash makes corepack compare the tarball's hash and skip the signature path, so a wrong hash fails. I did not run this.
  - The local pnpm 11.9.0 package ships `bin/pnpm.cjs`, the file corepack's pnpm ≥ 6 range launches.
- **Format version:** the loader refuses a different `format` as `malformed` (`document.ts:363`, tested at `document.test.ts:56`).
- **Schema, apart from the decimals:** UUID patterns, enums, required members, no extra properties, integer types and the two `minimum`s all pass for both committed exports.
- **`check_retained`:** a real fit passes it (the fixtures check above). The existing tests catch two mutants: comparing the first retained frame instead of the last, and a value one ulp off.
- **Zero frames:** the exporter refuses as `run` ("retains no candidate") and the loader draws it, as the DESIGN text describes. A replay where every frame was rejected draws in both views (green case).
- **Dynamic import:** only `store.ts` imports the sample outside the tests, and Vite 8's default build target supports top-level await. I did not run `studio-build`.
- **Same-machine determinism:** a fresh fit and export gave identical bytes (section 3).
- **Different-machine determinism:** not settled. The fit's numbers go through the system math library (`tan`, `tanh`, `exp`, `powf`, `cos`). This machine runs glibc 2.44; CI's `ubuntu-latest` has an older glibc. The existing golden tests use tolerances, and the demo digests are 16-bit PCM, so this is the first gate that needs bit-identical floating-point results across machines. CI's first `studio-fixtures-check` will be the evidence.
- **Lease:** I did not take a worktree session lease. I made no git writes and ran no `aep plan artifact` command.

## 6. Paths written outside the worktree

- `the adversary scratch directory c1/studio-replay/adv2/` contains `rust-red.log`, `rust-suite.log`, `rust-deselected.log`, `vitest-red.log`, `vitest-suite.log`, `vitest-deselected.log` and `fixtures-check.log`.
- `task studio-fixtures-check` created and removed `a temporary patchback-studio-fixtures directory under the cache`.
- Inside the worktree, cargo wrote test scratch under `target/tmp/` and the build output in `target/`.

## 7. Findings block

```findings
[
  {"file": "crates/patchback-studio/src/export.rs", "line": 328, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a trace line with a negative sequence or restart is refused as trace, not invariant, contradicting DESIGN.md 16 and correction 1's claim that the exporter and loader name a broken TraceFrame invariant alike (test: adversary_2_a_negative_sequence_is_invariant, adversary_2_a_negative_restart_is_invariant)"},
  {"file": "crates/patchback-studio/src/export.rs", "line": 307, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a rejected line carrying a loss with a repeated sequence is exported as FrameOutOfOrder, while AppendFrame's outcome order and the studio loader answer InconsistentFrame (test: adversary_2_a_rejected_frame_with_a_loss_is_inconsistent_before_out_of_order)"},
  {"file": "crates/patchback-studio/src/export.rs", "line": 312, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a retained line whose loss is not its rising best loss is exported as BestLossWorsened, while AppendFrame's retained-not-best outcome and the studio loader answer InconsistentFrame (test: adversary_2_a_retained_frame_whose_loss_is_not_its_best_is_inconsistent_before_best_worsened)"},
  {"file": "crates/patchback-studio/src/export.rs", "line": 342, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "held_sequence counts only lines starting with \"evaluation \" although parse_trace splits on any ASCII whitespace, so a tab-separated trace's repeated sequence is refused as trace instead of FrameOutOfOrder (test: adversary_2_a_repeated_sequence_is_out_of_order_whatever_the_field_separator)"},
  {"file": "crates/patchback-studio/src/export.rs", "line": 174, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the module documentation promises refusal 4 run (trace of another topology) before refusal 7 trace, but the trace is parsed first, so an unclosed trace of another topology is refused as trace (test: adversary_2_a_trace_of_another_topology_is_run_before_its_grammar)"},
  {"file": "crates/patchback-studio/src/graph.rs", "line": 273, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "format_decimal writes exponent form when shorter (1e-3, 5e-3), so every export, the bundled sample included, has decimals that fail the ESS Decimal pattern of studio/src/generated/source.schema.json, and the loader's decimal() accepts them (test: studio/src/schema.adv2.test.ts > every member of the committed exports validates against studio/src/generated/source.schema.json)"},
  {"file": "studio/src/components/LossChart.vue", "line": 20, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "Math.max spread over every frame throws RangeError at 130000 frames; --max-renders permits such a run but none was shown to reach it (test: studio/src/schema.adv2.test.ts > draws 130 000 frames without an error)"},
  {"file": "crates/patchback-studio/tests/fit_replay.rs", "line": 492, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the rejected-frame fixture is blessed when PATCHBACK_STUDIO_BLESS has any value, including 0 or empty, and the test then compares the fixture with itself; task check blesses only if the caller's environment exports it, which nothing in Taskfile.yml or the workflows does (test: none, judgement)"}
]
```

The home-directory paths in the header and section on paths written outside the worktree were replaced by descriptions by the coordinator, because the security gate refuses home paths in commits.
