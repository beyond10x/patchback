---
format: aep.planning-md/3
id: approval-record:wave-a5
kind: approval-record
status: approved
title: Wave a5 approved by Timo
relations:
- decides: story:subtractive-voice-render
- decides: story:external-audio-provenance
- decides: story:feature-contract
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T18:56:28Z", actor: "human:timo", revision: 2}
---
# Wave a5 approved by Timo

## Who approved, in their words

Timo, 2026-10-24, in chat to this session, which he named the orchestrator for this repository:

> continue to work on patchback, you are the orchestrator and start up to 3 opus sub-agents for
> implementations, check status, use /planning

The orchestrator acts under the grant in the operator's workspace instructions (class C: a wave;
commits, pushes and merges through the bot; a release through the repository's own process; the
ESS pin). The release after the merged wave follows approval-record:release-process.

## The wave

- `story:subtractive-voice-render` (`patchback-dsp` and the `render` verb),
  `story:external-audio-provenance` (`patchback-audio`, library only) and
  `story:feature-contract` (`patchback-analysis`, library only), in parallel, one Opus
  implementor each, then one Opus adversary pass each and one correction round.
- The ESS pin moves from 0.68.0 to 0.69.0 (`spec/ess-inputs.yaml`, `check.yml`), the rule in
  AGENTS.md § Planning and specification.
- Merge into `wave/a5`, fast-forward `main`, push, release 0.4.0.
- Not in the wave: `story:note-estimation` (shares `patchback-analysis` with feature-contract),
  the studio stories (option B since a4), and the fit stories, which depend on the renderer.

## Decisions taken by the orchestrator

- D1 Render follows `spec/domains/audio.yaml`: the sequence's `patch_id` is not compared, a failed
  render leaves nothing, only a non-positive block size is refused. Output WAV is float32. The
  limiter threshold and the render tail are provisional named constants.
- D2 Provenance is a library-level import request; `patchback analyze` and `compare` keep their
  command lines because DESIGN.md § 11 names no origin, provenance or licence flag (OQ-20 open).
  Provenance and licence are required on every `ExternalRecording` import, as modelled.
- D3 `features/1` band energies are window-major; the windows come from a note's start and end
  given to the extractor; band edges are the analysis crate's `BAND_EDGES_HZ`; the model's
  `CheckFeatureVector` and `AmendFeatureContract` are implemented as modelled.
- D4 No `aep:story-scoper` dispatch: the coordinator wrote the typed scope entries from reading
  the tree, marked `cited` or `inferred`.
