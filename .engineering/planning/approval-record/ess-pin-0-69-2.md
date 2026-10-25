---
format: aep.planning-md/3
id: approval-record:ess-pin-0-69-2
kind: approval-record
status: approved
title: ESS pin moved to 0.69.2
relations:
- decides: story:workspace-scaffold
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-25T09:28:35Z", actor: "human:timo", revision: 2}
---
# ESS pin moved to 0.69.2

## The rule

AGENTS.md of the operator's workspace (Timo, 2026-10-05): "ALWAYS upgrade to most recent ESS";
never hold ESS back with a pin, and move repositories that pin the `ess` CLI to the newest
release when one ships. The orchestrator acts under the grant in the operator's workspace
instructions (class C: a dependency upgrade, including the ess pin).

## Why now

Patchback pinned `requires: ess 0.68.0` (approval-record:ess-pin-0-68) through waves a1 to a11
while ess 0.69.0 and 0.69.1 regressed on this specification (Timo, 2026-10-24: "ess: if you can
pin down the refression and fix it - that would be amazing"). The fix shipped as ess 0.69.2 on
2026-10-25 (https://github.com/beyond10x/ess/releases/tag/0.69.2, published 09:14:21Z by
`b10x-bot[bot]`; the installed `ess` 0.69.2 delegates to 0.68.0 while the pin says 0.68.0, so
the pin had to move before 0.69.2 could be exercised here).

## The change

- `spec/ess-inputs.yaml`: `requires: ess 0.69.2`.
- `.github/workflows/check.yml` and `.github/workflows/pages.yml`: `ESS_PINNED: "0.69.2"`,
  `ESS_SHA256: 8cc5a3a7ed7c27aacbff131ab1437e18e7d7d1878fee77542b3e0fff7157d414`, the sha256 of
  `ess-0.69.2-x86_64-unknown-linux-gnu.tar.gz` from the release's `SHA256SUMS`.
- `task studio-types` regenerated `studio/src/generated/` with ESS 0.69.2: `types.ts` gains an
  `export const <Name> = { … } as const` companion for every enumerated type (ClampPolicy,
  ModuleFamily, NodeKind, ParameterMapping, ParameterUnit and the others), and the generator
  version and digests in `types.ts`, `source.schema.json` and `types-report.json` move.

## What 0.69.2 did with the specification (wave a12 tree, 2026-10-25)

- `task spec-validate`: 10 files, 130 scenarios, valid.
- `task spec-synthesize`: 470 scenarios (130 authored), 27 refusals; under 0.68.0 the same
  specification synthesized 403 scenarios with 92 refusals.
- `task spec-conform`: 470 scenarios, 470 passed, 0 failed, 0 error, 0 unsupported.
- `task studio-types-check`: no drift after the regeneration.
- `task docs-generate` was not run under the pin alone: in the wave tree it stops on the
  evidence file of a unit not yet merged, which is not an ESS matter; it runs at the wave's gate.

## Commits the approval authorises

One commit on `wave/a12`, separate from the units', as `b10x-bot[bot]` through `b10x-gates bot`.
