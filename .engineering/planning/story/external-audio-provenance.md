---
format: aep.planning-md/3
id: story:external-audio-provenance
kind: story
status: draft
title: Provenance for external audio
relations:
- decomposes: epic:sound-engine
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:wav-io
- depends_on: story:workspace-scaffold
scope:
- confidence: inferred
  path: crates/patchback-audio
revision: 2
---
# Provenance for external audio

## Outcome

An external recording is imported only with its provenance and licence recorded beside the clip;
an import missing either is refused. Clips synthesized by Patchback carry their origin. This keeps
every fixture either synthesized or licensed with provenance.

## DESIGN.md

§ 6.1, § 12.3 (provenance for test fixtures), AGENTS.md fixture rule.

## Acceptance

- spec/scenarios/external-audio-provenance-accepted.yaml
- spec/scenarios/external-audio-provenance-missing.yaml
- spec/scenarios/external-audio-provenance-licence-missing.yaml

## Numeric obligations

None; the WAV numeric bounds are held by wav-io.

## Open questions

- Provenance required on every external import (modelled) or only for files under
  `tests/fixtures/`.
- No CLI flag in § 11 states a recording's origin, provenance or licence.
- Whether a synthesized clip must name the render it came from.
