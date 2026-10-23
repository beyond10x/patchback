---
format: aep.planning-md/3
id: approval-record:release-process
kind: approval-record
status: draft
title: Patchback releases after every merged wave, approved by b10x-chef-orchestrator
relations:
- decides: epic:sound-engine
revision: 1
---
# Patchback releases often, approved by b10x-chef-orchestrator

## Who approved, in their words

Message from session `b10x-chef-orchestrator`, 2026-10-23, under the grant in the operator's
workspace instructions (class C: commit, PR, merge, release). It quotes the operator: "adopt
patchback, connectors-definitions - they are also meant to release often", then "adopt both, and
then trigger them to do some work". The items read:

1. "Finish wave a2 as planned."
2. "Add the release process that patchback AGENTS.md:72-73 names: the modelspec pattern (copy
   ~/beyond10x/modelspec/.github/workflows/release.yml), bare-version tags, Keep a Changelog 1.1.0."
3. "Cut 0.1.0 from main once a2 is merged and green. Release again after each merged wave."

## The change

- `.github/workflows/release.yml`: modelspec's workflow on bare-version tags: `task release-check`,
  then `check.yml`, then the `patchback` binary for Linux x86_64 and its `.sha256` on a GitHub
  Release whose notes are the version's CHANGELOG section.
- `Taskfile.yml`: `task release-check TAG=<version>`, modelspec's task unchanged.
- `AGENTS.md`: a Releases section with the release commit, the bot tag and when a release is
  finished; a release follows every merged wave.
- First release: 0.1.0 from `main` after wave a2.
