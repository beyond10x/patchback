---
format: aep.planning-md/3
id: approval-record:control-events-file
kind: approval-record
status: approved
title: Control events go in their own file (OQ-9, option A)
relations:
- decides: decision-blocker:control-events-file
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T17:18:00Z", actor: "human:timo", revision: 2}
---
# Control events go in their own file

Decider: timo, through `b10x-chef-orchestrator` (Timo, 17:17Z 2026-10-23: "dispatch and control:
modelspec + patchback").

The orchestrator's message, verbatim on this item:

> decision-blocker:control-events-file: A, a separate controls file; Milestone A parser stays as it
> is.

## What it settles

- Option A of decision-blocker:control-events-file (OQ-9): control automation writes its own file,
  `controls.events` version 1, beside `notes.events`.
- `notes.events` version 1, as Milestone A ships it, does not change.
- story:control-automation is no longer blocked by this decision.
