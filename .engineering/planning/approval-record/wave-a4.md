---
format: aep.planning-md/3
id: approval-record:wave-a4
kind: approval-record
status: approved
title: Wave a4 approved by b10x-chef-orchestrator
relations:
- decides: story:patch-review-fixes
- decides: story:note-events
- decides: story:audio-comparison
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T18:30:33Z", actor: "human:timo", revision: 2}
---
# Wave a4 approved by b10x-chef-orchestrator

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: a wave; commits, merges and a release through the bot). First:
"Wave a4 approved under the grant, with the two carried items: propose its units to me in one
message with your recommendation, I answer at once." Then, on the proposal (option A, units 1-3):

> wave a4: A approved under the grant: units 1-3, one implementor each plus an adversary pass, then
> merge and release 0.3.0. note-events covers notes only; control events go to their own file
> (approval-record:control-events-file). One target per tree, discard-cache each tree at merge.

## The wave

- `story:patch-review-fixes` (the two findings carried from wave a3), `story:note-events` (notes
  only; `controls.events` stays with story:control-automation) and `story:audio-comparison`, in
  parallel, one implementor each, then one adversary pass each.
- Merge into `wave/a4`, fast-forward `main`, release 0.3.0.
- Not in the wave: the studio views (option B) and story:subtractive-voice-render, which needs
  note-events merged first.
