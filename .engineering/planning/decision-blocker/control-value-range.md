---
format: aep.planning-md/3
id: decision-blocker:control-value-range
kind: decision-blocker
status: open
title: 'Control value outside [0,1]: clamp (DESIGN 5.2) or refuse (scenario)'
relations:
- blocks: story:control-automation
revision: 1
---
# Control value outside [0, 1]: clamp or refuse

story:control-automation says normalized values are clamped to [0, 1], as DESIGN.md § 5.2 and
story:parameter-contracts do. `spec/scenarios/control-automation-automated.yaml` refuses 1.3 with
`ControlValueOutOfRange`. Story and scenario contradict.

Cleared when either the scenario clamps and records (per § 5.2) or the refusal is recorded as a
deliberate deviation in DESIGN.md.
