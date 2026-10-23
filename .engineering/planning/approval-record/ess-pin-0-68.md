---
format: aep.planning-md/3
id: approval-record:ess-pin-0-68
kind: approval-record
status: draft
title: ESS pin moved to 0.68.0, approved by b10x-chef-orchestrator
relations:
- decides: story:workspace-scaffold
revision: 1
---
# ESS pin moved to 0.68.0, approved by b10x-chef-orchestrator

## The stop

The installed `ess` 0.68.0 refuses `spec/ess-inputs.yaml`, which pinned `requires: ess 0.67.0`, so
the wave a1 gate could not run on the current ESS release. The coordinator asked whether to move the
pin inside wave a1.

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: a dependency upgrade, including the ess pin). The message reads:
"A. Move the ess pin to 0.68.0 in wave a1, as a separate commit: the pin in spec/ess-inputs.yaml
and the version and checksum in check.yml, then regenerate. ... Remove the private 0.67.0 copy from
the gate once 0.68.0 is pinned. ess 0.69.0 will follow, so report anything that 0.68.0 refuses as
`[patchback] ess-gap:`."

## The change

- `spec/ess-inputs.yaml`: `requires: ess 0.68.0`.
- `.github/workflows/check.yml` and `.github/workflows/pages.yml`: `ESS_PINNED: "0.68.0"`,
  `ESS_SHA256: 30000c7a9030793a7964997941d6052e09c168fb444b986ca3ed18961465be3f`, the sha256 of
  `ess-0.68.0-x86_64-unknown-linux-gnu.tar.gz`, equal to the GitHub release asset digest.
- `task studio-types` regenerated `studio/src/generated/` with ESS 0.68.0.

## What 0.68.0 did with the specification

- `task spec-validate`: 10 files, 123 scenarios, valid.
- `task spec-conform`: 382 scenarios, 382 passed, 0 failed, 0 error, 0 unsupported.
- `task docs-generate`: exit 0; no generated site page changed.
- Nothing refused, so no `ess-gap` report. The `Gap (ESS 0.67.0)` comments in `spec/domains/`
  were not rechecked against 0.68.0 and stay as written.

## Commits the approval authorises

One commit on `wave/a1`, separate from the unit's, as `b10x-bot[bot]` through `b10x-gates bot`.
