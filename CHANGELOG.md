# Changelog

All notable changes to Patchback are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `patchback-audio`: std-only WAV import and export. Mono or stereo (downmixed by the per-frame
  mean, and recorded) PCM16 or float32 at 48 kHz is read into a clip and written back
  byte-exact; unsupported headers, truncated or corrupt chunks, non-finite samples, files past
  the provisional ten-minute frame bound and PCM16 export of samples outside [-1, 1] are refused.
- The Cargo workspace of DESIGN.md § 4.1: library crates `patchback-core`, `patchback-dsp`,
  `patchback-audio`, `patchback-analysis`, `patchback-dataset`, `patchback-model` and
  `patchback-fit`, and `patchback-cli` with the `patchback` binary, which recognises `patch
  validate`, `render`, `analyze`, `compare` and `fit` but implements none of them yet. No
  application crate takes a third-party crate other than `clap` and its reviewed closure, and a
  test holds that.
- `DESIGN.md` revision 0.3: the technical design. The product and its CLI are named `patchback`,
  ESS is required for the specification only (ADR-008), the patch studio is a developer tool
  outside the runtime dependency policy (ADR-011), and § 17 lists the open questions.
- `spec/`: the ESS specification of the application contracts. It has nine domains (patch, score,
  audio, analysis, dataset, model, fit, dependency, studio) and the authored conformance scenarios
  that accept each story.
- `.engineering/`: the AEP planning store, with the vision, the product requirements, one epic per
  milestone A–E, the patch-studio epic, draft stories and the decision records.
- `website/` and `crates/patchback-docs`: the documentation site, with an animated landing page
  and a dark-only theme, and the generator of its status and specification reference pages.
- `studio/`: the patch studio prototype (Vue 3, TypeScript, Vite, pnpm), which draws the module
  catalogue, patch graphs, a fit replay and the end result from illustrative fixture data.
- `task check`: validates the specification, runs its conformance suite against the interpreted
  reference target, validates the planning store, checks that every story names existing
  acceptance scenarios, and checks the generated documentation pages.
- ModelSpec 0.5.0 is pinned in `models/modelspec.pin`, and `models/patch_regressor.mspec` holds the
  DESIGN.md § 8.1 model; `patchback-model` runs the pinned binary's `modelspec check` on it,
  refuses a binary that reports another version, and keeps a rejected model from training. CI
  installs the pinned binary by checksum and runs that check.
- `patchback-core` parameter contracts: the `subtractive-mono/1` topology with its eight
  contracts of `DESIGN.md` § 5.2 (placeholder defaults), forward and inverse mappings, clamping
  of normalized values with a recorded clamp, and refusal of non-finite values and of invalid
  contracts (including indices other than 0..n-1 and ranges whose width or ratio overflows),
  each refusal naming the topology.

### Changed

- `patchback` parses its command line with clap derive, the one reviewed CLI crate (`DESIGN.md`
  revision 0.4, § 3); help, version and usage errors follow clap (usage errors exit 2), and the
  dependency gate admits `clap` with exactly its reviewed `Cargo.lock` closure.
