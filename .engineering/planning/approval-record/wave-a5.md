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
revision: 7
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
- The ESS pin was to move from 0.68.0 to 0.69.0 (`spec/ess-inputs.yaml`, `check.yml`), the rule
  in AGENTS.md § Planning and specification. It stays at 0.68.0 by decision D5 below.
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
- D5 The ESS pin stays at 0.68.0: `ess verify conform synthesize` on 0.69.0 over `spec/` exceeded
  three times the 0.68.0 baseline (56 s) and was killed after 13 min; the regression is recorded
  in `.engineering/waves/a5.md` § Pre-flight and reported to Timo for the ess repository.
- D6 The refusal order of `ImportWav` in `patchback-audio` is the order ESS documents for a
  command's outcomes (input refusals first in declaration order, then `before_row` externals,
  then existence, then the other externals; `ess` repository
  `website/docs/guides/specify/guards-and-predicates.md` § "Which branch answers"). The first
  implementor's order and the adversary's expectation both differed from it; the correction round
  applies it to the import request and, from the rate check on, to `decode` and `read_wav`.
- D7 An empty provenance or licence string counts as given, as the model's `defined()` says.
  Open point for the owner, no spec change in this wave.
- D8 A silent release window refuses the whole `features/1` vector as `FeatureNotComputable`, as
  modelled. Open point for `story:dataset-generation`: rendered notes with a near-zero release
  will be refused.
- D9 `define` of a feature contract follows the model's outcomes and order: `wrong-count` for
  `features/1` only, then `version-taken`, then `defined`. The first implementor's layout rule
  for other versions is removed; a layout rule for other versions is an open point for the spec.
- D10 Feature extraction accepts an `StftAnalysis` at any hop; the hop is `UNMAPPED` in
  `spec/domains/analysis.yaml`.
- D11 The `out` value in `patchback render`'s success report is escaped the way a refusal's
  message is, so a path holding a newline or a control character keeps one key-value record per
  line (adversary pass 1, finding 2).
- D12 `patchback_dsp::render` refuses a note whose end sample is not after its start as
  `RenderError::InvalidNoteTiming`, mapped to the domain error `patchback.audio.NoteSequenceNotFound`
  and naming `patchback.score.InvalidNoteTiming` in its message: the score domain owns that
  refusal and the CLI's loader answers it first, so no new audio domain error is modelled. The
  check sits after the rate and block-size checks and before the length check. Overlapping notes,
  notes with one start, a MIDI number past 127 and a non-finite or out-of-range velocity render
  finite and go silent after the release (adversary pass 1, finding 3).
- D13 Of two byte-level faults in one WAV file whose `fmt ` and `data` headers are readable (a
  second `fmt ` or `data` chunk, an oversized `fmt `, with a later truncation), the reader
  answers the one that ranks first in `IMPORT_WAV_PRECEDENCE`: the truncation. This extends D6 to
  two byte-level faults and is listed under `### Changed` for 0.4.0 (correction round 1 of
  `story:external-audio-provenance`, decision 2).
- D14 The `out` value of `patchback render`'s report is printed as a refusal prints a path
  (`{path:?}`: double-quoted, `\\`, `\n`, `\"`, `\xNN` for a byte outside UTF-8), so two paths
  never print one record and one function serves refusal and report. Supersedes the unquoted form
  of D11 (adversary pass 2, finding 1).
- D15 Of two `fmt ` chunks, or two `data` chunks, in one WAV file the first supplies the header
  facts the predicates judge; the second is the corruption. The reader's doc comment states it;
  `spec/domains/audio.yaml` does not, and the point is recorded for the specification's owner
  beside D7 (unit 2 adversary pass 2, judgement). No code change.
- D16 `patchback-docs` runs `ess` with `spec/` as its working directory and `--path .`, so an
  `ess` that dispatches on `requires:` finds the pin. The 0.69.0 dispatcher installed for D5's
  measurement refused `ess generate --path ./spec --strict-requires` from the repository root
  ("requires ess 0.68.0 and this is ess 0.69.0"), and `task docs-check` failed on `wave/a5`;
  CI installs the pinned release and was unaffected. Tooling only, no CHANGELOG entry.
