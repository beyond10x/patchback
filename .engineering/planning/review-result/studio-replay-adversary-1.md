---
format: aep.planning-md/3
id: review-result:studio-replay-adversary-1
kind: review-result
status: active
title: 'Adversary pass 1: patch graph and fit replay (wave c1, unit 2)'
relations:
- reviews: story:studio-patch-graph
- reviews: story:studio-fit-replay
revision: 1
---
unit: story:studio-patch-graph + story:studio-fit-replay, commit 846c30e (base 1623311), working tree of patchback-impl-studio-replay
verdict: red
cases: executed 51→54, red 3
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (my scratch directory .../c1/studio-replay/adv1)
needs-coordinator: no

Three new cases fail; nothing else in the unit broke. Two show the exporter giving the wrong error name for some sequence numbers. One shows the evolution view crashing on a replay with no frames.

## 1. What I touched

`git diff --stat` is empty: no tracked file changed. `git status` shows only two new files, both tests:
```
?? crates/patchback-studio/tests/adversary_1.rs
?? studio/src/replay.adv1.test.ts
```

## 2. The cases I added (each run alone first)

**`adversary_1_a_gap_in_the_sequence_is_not_frame_out_of_order`** (`crates/patchback-studio/tests/adversary_1.rs:31`), red. A trace numbered 1, 2, 4 is exported as `FrameOutOfOrder`. The domain uses that error only when the trace already holds a sequence at or above the new one, and 4 is above 1 and 2. Refusing the gap is fine; the name and message are wrong:
```
assertion `left != right` failed: patchback.studio.FrameOutOfOrder: trace of trace.events: the frame of sequence 4 is not above every sequence the trace holds: line 5: expected sequence numbers 1, 2, 3 and on, in order
```

**`adversary_1_a_first_frame_of_sequence_zero_is_not_frame_out_of_order`** (`adversary_1.rs:46`), red. A first frame numbered 0 is exported as `FrameOutOfOrder`. The trace holds nothing yet, so out-of-order cannot apply. What it breaks is the invariant `sequence >= 1`, which this unit itself added to `spec/domains/studio.yaml`. The studio's loader answers the same input as `malformed`, and its own test asserts that (`studio/src/replay.test.ts`). So the exporter and the loader name the same input differently. The existing test `fit_replay.rs:366-369` locks in the exporter's answer. I left it as it is.
```
assertion `left != right` failed: patchback.studio.FrameOutOfOrder: trace of trace.events: the frame of sequence 0 is not above every sequence the trace holds: line 3: expected sequence numbers 1, 2, 3 and on, in order
```
Likely fix: in `trace_refusal` (`export.rs:274-279`), map `SEQUENCE_ORDER` to `FrameOutOfOrder` only when the line's sequence is at or below the number of records already read. Otherwise answer `trace`, or `malformed` for 0.

**`is refused by the loader or drawn by the evolution view without an error`** (`studio/src/replay.adv1.test.ts:20`), red. A document with `trace.frames = []` passes `checkDocument`. Mounting `EvolutionView` then throws, because the view indexes `frames[-1]` (`index` starts at `last = -1`, and the template reads `frames[last]!.evaluation`):
```
- []
+ [
+   "TypeError: Cannot read properties of undefined (reading 'evaluation')",
+ ]
```
The exporter produces such a document. Running the built `patchback-studio export` on a run whose `trace.events` is just the header, the topology line and `completed RenderBudgetExhausted` exits 0 and writes `"frames":[]` with `ended: Completed`.

## 3. Suite runs (after the cases existed)

- `cargo test --locked --no-fail-fast -p patchback-studio` exits 101. Results by test file:
  - lib: 1 passed
  - main: 0
  - adversary_1: 2 failed
  - fit_replay: 13 passed
  - patch_graph: 12 passed
  - Without my file that is 26 cases; with it, 28. My first run without `--no-fail-fast` stopped after adversary_1, so I reran.
- `pnpm test` exits 1: `Test Files 1 failed | 4 passed (5)`, `Tests 1 failed | 25 passed (26)`. Without my file that is 25 cases.
- `task studio-fixtures-check` exits 0 after 101 s; the fit's own wall time was 76.6 s. A fresh fit of `demos/pluck` exported to the same bytes as `studio/fixtures/sample-run.json`, and the task removed its temporary directory.

## 4. Findings

| file:line | verdict | origin | what was measured | what reaches it |
|---|---|---|---|---|
| `crates/patchback-studio/src/export.rs:274` | CONFIRMED | introduced | a gap in sequences is reported as `FrameOutOfOrder`, with a false message (case 1) | a hand-edited or damaged `trace.events`; no real fit writes a gap (the parser requires 1, 2, 3 and so on) |
| `crates/patchback-studio/src/export.rs:274` | CONFIRMED | introduced | sequence 0 is `FrameOutOfOrder` in the exporter but `malformed` in the studio loader, and it breaks the unit's new `sequence >= 1` invariant (case 2) | the same: hand-made trace files only |
| `studio/src/views/EvolutionView.vue:17` | INFEASIBLE | introduced | a zero-frame replay is accepted and then crashes the view (case 3) | a file chosen in the picker, which `document.ts` says is "refused, not drawn wrongly"; also a hand-written `trace.events` passed through the exporter. A real fit never ends with zero evaluations: `FitRun::complete` refuses a run with nothing retained (`run.rs:328`) |

The origin is `introduced` for all three because the exporter, the loader and the view are new in this diff.

## 5. Claims I attacked and could not break

- **Byte determinism:** keys come out sorted (serde_json is built without `preserve_order`), every decimal is a string, and identities come from the file bytes, not the path. The `patchback <version>` line is left out of the run's identity, so a release does not change the sample.
- **Refusals of the scenario errors:** each is answered, in the domain's order, by `graph::check` / `Replay`, and by the TypeScript loader for hand-picked files. `FeedbackNotAtDelay` cannot come out of the exporter: the drawing marks feedback only on edges that leave a Delay node.
- **Replay rules** (sequence order, best loss, retained loss, rejected frame): the exporter and the loader apply them in the same precedence order as the domain.
- **DOM node count:** the frame table shows a 21-row window, and the loss chart and the trajectories draw one SVG path per series.
- **Bundled sample:** it equals a refit (section 3). Its last retained frame has the same `patch_id` as the drawn graph. It has no rejected frames, so it never exercises that branch.
- **Dependency gate:** an application crate that reaches `patchback-studio`, including as a dev-dependency, is refused. The gate does not check the tooling crates' own dependencies, but that was already true for `patchback-docs` and DESIGN.md § 3 now states it.
- **The three known gaps:** I did not re-attack them.

## 6. Paths written outside the worktree

The adversary's scratch directory `c1/studio-replay/adv1/` contains:
- the run logs: `rust-red.log`, `rust-suite.log`, `vitest-red.log`, `vitest-suite.log`, `fixtures-check.log`
- `probe-empty/run/{patch.patch,run.meta,trace.events}` and `probe-empty/out.json`, from the zero-frame probe

The fixtures check also created and removed its own temporary directory under the cache.

## 7. Findings block

```findings
[
  {
    "file": "crates/patchback-studio/src/export.rs",
    "line": 274,
    "category": "contract-drift",
    "severity": "note",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "a trace.events sequence gap (1, 2, 4) is exported as patchback.studio.FrameOutOfOrder, whose domain rule and message (\"not above every sequence the trace holds\") do not apply to a sequence above every held one (test: adversary_1_a_gap_in_the_sequence_is_not_frame_out_of_order)"
  },
  {
    "file": "crates/patchback-studio/src/export.rs",
    "line": 274,
    "category": "contract-drift",
    "severity": "warning",
    "verdict": "CONFIRMED",
    "origin": "introduced",
    "message": "a first frame of sequence 0 is FrameOutOfOrder in the exporter but malformed in the studio loader, and violates the unit's own TraceFrame invariant sequence >= 1 rather than the out-of-order rule (test: adversary_1_a_first_frame_of_sequence_zero_is_not_frame_out_of_order)"
  },
  {
    "file": "studio/src/views/EvolutionView.vue",
    "line": 17,
    "category": "boundary",
    "severity": "warning",
    "verdict": "INFEASIBLE",
    "origin": "introduced",
    "message": "a replay with zero frames passes checkDocument (and patchback-studio export writes one from a frameless trace.events), then EvolutionView throws reading frames[-1].evaluation; reached only by a hand-made file, never by a real fit (test: studio/src/replay.adv1.test.ts > adversary 1: a replay without frames > is refused by the loader or drawn by the evolution view without an error)"
  }
]
```

The third verdict was returned as PLAUSIBLE and corrected by the adversary to INFEASIBLE after the store refused it; the findings block is the adversary's JSON re-serialization, requested after two refusals caused by the coordinator's brief.
