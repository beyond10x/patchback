# Changelog

All notable changes to Patchback are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
