---
format: aep.planning-md/3
id: decision-blocker:control-events-file
kind: decision-blocker
status: open
title: 'Control events: own file or events/2 shared with notes.events'
relations:
- blocks: story:control-automation
revision: 2
---
# Control events: own file or events/2 shared with notes.events

Decider: timo

story:note-events and story:control-automation both leave open which file carries control events
(DESIGN.md § 17, OQ-9). It is a § 11 file-artifact contract, so the decision is the operator's.
It blocks story:control-automation (Milestone D) only; wave a3 and story:note-events (wave a4) do
not wait for it, because note-events writes `notes.events` version 1 either way.

## Options

| | option | what it does | cost |
|---|---|---|---|
| A | own file, `controls.events` version 1 | `notes.events` stays version 1 as Milestone A ships it; control automation adds a second line-oriented file beside it | a fit run and `render` read two event files |
| B | `events/2` shared with notes | one file carries notes and controls; story:control-automation owns version 2 | Milestone D reopens the Milestone A parser and every reader of version 1 |

## Recommendation

A: nothing shipped in Milestone A changes, and § 11 already lists one file per artifact.

This waits for Timo; no Milestone A or B story depends on it.
