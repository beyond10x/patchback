---
format: aep.planning-md/3
id: approval-record:ess-pin-0-80-0
kind: approval-record
status: approved
title: ESS pin moved to 0.80.0
relations:
- decides: story:workspace-scaffold
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-01T15:19:58Z", actor: "human:timo", revision: 2, executor: "agent:wave-c1-coordinator"}
---
# ESS pin moved to 0.80.0

## The rule

The operator's workspace instructions (Timo, 2026-10-29): a repository's `ess` pin moves to the
newest ESS release when a change touches that repository anyway, in the same wave. Wave c1
touches Patchback (approval-record:wave-c1), so the pin rides along.

## The change

- `spec/ess-inputs.yaml`: `requires: ess 0.80.0` (was 0.69.2).
- `.github/workflows/check.yml` and `.github/workflows/pages.yml`: `ESS_PINNED: "0.80.0"`,
  `ESS_SHA256: af8eff703177209628a55188ba3e7c618eb12b805777864b96fd1488198905d4`, the sha256 of
  `ess-0.80.0-x86_64-unknown-linux-gnu.tar.gz` read from the release's `SHA256SUMS`.
- `task studio-types` regenerated `studio/src/generated/types.ts` and `types-report.json`: one
  line each (the generator version).

## What 0.80.0 did with the specification (wave c1 tree at main ab1762c, 2026-11-01)

- `task spec-validate`: exit 0.
- `task spec-synthesize`: 470 scenarios, 27 refusals, the same counts as under 0.69.2.
- `task spec-conform`: 470 scenarios, 470 passed, 0 failed, 0 error, 0 unsupported.
- `task studio-types-check`: exit 0, no drift after the regeneration.

## Commits the approval authorises

One commit on `wave/c1` carrying the pin, the two workflow lines, the regenerated studio types
and this record. It ships with wave c1.
