---
format: aep.planning-md/3
id: decision-blocker:control-events-file
kind: decision-blocker
status: open
title: 'Control events: own file or events/2 shared with notes.events'
relations:
- blocks: story:control-automation
revision: 1
---
# Control events: own file or events/2 shared with notes.events

story:note-events and story:control-automation both leave open who owns the `.events` file for
control events. If controls share `notes.events`, Milestone D reopens the Milestone A parser.

Cleared when one is chosen: a separate controls file (no Milestone A rework) or a versioned
`events/2` owned by story:control-automation. Decide before control-automation is proposed.
