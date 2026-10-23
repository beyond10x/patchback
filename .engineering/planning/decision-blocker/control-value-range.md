---
format: aep.planning-md/3
id: decision-blocker:control-value-range
kind: decision-blocker
status: cleared
title: 'Control value outside [0,1]: clamp (DESIGN 5.2) or refuse (scenario)'
relations:
- blocks: story:control-automation
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-23T13:58:16Z", actor: "human:timo", revision: 3}
---
# Control value outside [0, 1]: clamp or refuse

story:control-automation says normalized values are clamped to [0, 1], as DESIGN.md § 5.2 and
story:parameter-contracts do. `spec/scenarios/control-automation-automated.yaml` refused 1.3 with
`ControlValueOutOfRange`. Story and scenario contradicted.

## Cleared

DESIGN.md revision 0.3 § 10 settles it: "a value outside `[0,1]` is clamped with a warning, not
refused, and NaN/Infinity is refused". The scenario now clamps 1.3 (`outcome: clamped`, event
`patchback.score.ControlValuesClamped`, `control-automation-automated.yaml:67-73`). What stays open
is whether the clamp rewrites the stored value or applies at render: OQ-27, an `UNMAPPED:` marker at
`spec/domains/score.yaml:425`, not this blocker.

## Confirmed

`b10x-chef-orchestrator`, 2026-10-23, for Timo: "clamp and record, per DESIGN.md 5.2 and
story:parameter-contracts". The scenario already clamps; nothing changed.
