# Patchback

Inverse synthesis for electronic music: from a recording, recover an editable DSP patch and the
note events that drive it, render it, and compare the rendering with the recording. Humans start
at [README.md](README.md); the design is [DESIGN.md](DESIGN.md).

## State

Milestone A in progress (DESIGN.md revision 0.7): the workspace crates, the clap-derived `patchback` CLI (`patch validate`, `render`, `analyze`, `compare`, `fit` and `benchmark` implemented, and Milestone B's `generate-dataset`), the dependency gate, the ModelSpec pin and check, the parameter contracts, WAV import and export with provenance on external recordings, the `.patch` file format, patch graph validation (including the fixed § 5.2 chain) and compilation, the fixed subtractive voice and its renderer, the analysis core, the frozen `features/1` feature contract, the `notes.events` file, the audio comparison loss, the bounded derivative-free fit and the six files of its run (`patch.patch`, `notes.events`, `reconstructed.wav`, `report.txt`, `run.meta` and `trace.events`; story fit-run-artifacts), the engine baseline benchmark over a seeded holdout (`benchmark.txt`, `clips.txt` and listening examples; story engine-baseline), the seeded dataset generator (`dataset.meta`, `samples.meta`, the training split's `normalization.stats` and each split's dense `x.f32` z-scored under those statistics, `y.f32` and `order.u32`; stories dense-data-files and normalization-stats) and the CPU training run of the pinned ModelSpec as a subprocess (`patchback_model::TrainingRun`, no `patchback` verb yet; story training-run) exist; `studio/` holds the patch studio prototype on fixture data. `spec/` holds the ESS domains and
the authored conformance scenarios; `.engineering/` holds the AEP planning store with one epic per
DESIGN.md § 13 milestone and draft stories whose acceptance is a list of named scenarios. The first
implementation slice is DESIGN.md § 15 (Milestone A, no ML).

## Commands

Entry points are the `Taskfile.yml` tasks (go-task):

- `task spec-validate`: `ess specify validate --path . --strict-requires` from inside `spec/`.
- `task spec-synthesize`: `ess verify conform synthesize` over `spec/` and its authored scenarios;
  the suite goes to `$PATCHBACK_SCRATCH` (default `~/.cache/patchback-conform`), never into `spec/`.
- `task spec-conform`: runs that suite against ESS's `interpreted` reference target, which executes
  the model itself. It proves the scenarios agree with the model; it is not evidence about any
  Patchback implementation.
- `task plan-validate`: `aep plan artifact validate`.
- `task acceptance-check`: fails when a story names no acceptance scenario, or names a
  `spec/scenarios/<name>.yaml` that does not exist.
- `task docs-generate`: `patchback-docs generate` rewrites the generated site pages
  (`website/docs/reference/ess/`, `website/docs/status.mdx`, `website/data/`) from `spec/` and the
  capability list in `crates/patchback-docs/src/status.rs`. Run it after every `spec/` change.
- `task docs-check`: `generate --check` plus `cargo test -p patchback-docs`.
- `task rust-test`: `cargo test --locked --workspace`; holds the dependency gate
  (`crates/patchback-cli/tests/dependencies.rs`) and the CLI dispatch tests.
- `task dependency-audit`: runs the dependency gate alone.
- `task check`: all of the above; the gate. CI (`check.yml`) runs it.
- `task release-check TAG=<version>`: refuses unless the tag is a bare version, equals
  `[workspace.package] version` in `Cargo.toml`, and `CHANGELOG.md` has `## [<version>] - <date>`.
- Site build: `npm --prefix website ci --allow-git=root` (npm 12 refuses Git dependencies without
  the flag; CI's Node 22 needs none), then `npm --prefix website run build`. `pages.yml`
  ("Documentation validation") builds it in CI; `b10x-docs-site.yml` ("Documentation site")
  deploys it to `https://beyond10x.github.io/patchback/` after a bot push to `main`.
- Cargo builds run directly, one target per tree, with `CARGO_INCREMENTAL=0`. Never delete `target/` or `website/node_modules/`.

## Planning and specification

- Work is planned in the AEP store under `.engineering/` through the `aep` CLI only
  (`aep plan artifact list` to start). Never edit a store file by hand. Body drafts go in
  `.engineering/drafts/` (git-ignored).
- Spec-first: every noun in DESIGN.md has its ESS domain in `spec/domains/` before a story is
  written around it. A story's `## Acceptance` lists the authored scenarios that decide it, one
  `spec/scenarios/<name>.yaml` per line; `task acceptance-check` holds the names.
- `spec/ess-inputs.yaml` pins the `ess` release; move the pin, and the checksum in
  `.github/workflows/check.yml`, to the newest ESS release when one ships.
- Numeric DSP and analysis properties (finite output, golden waveforms, FFT accuracy, loss
  reduction) are not expressible as ESS scenarios. The scenario names the command outcome; the
  numeric bound is stated in the story body and held by Rust tests when code lands.
- A relation or rule no source settles carries an `UNMAPPED:` marker in the domain and an open
  question in the store; never a guessed value.
- Generated output (`ess generate`, conformance suites and reports) goes outside `spec/`.

## Rules

- DESIGN.md is the authority. A change to an invariant (§ 2), the dependency policy (§ 3), a
  contract (§ 5–8, § 11) or an ADR (§ 14) edits DESIGN.md in the same change and bumps its
  revision line.
- Running code is Rust. No Python, PyTorch, or ML, DSP, maths or serialization crates (DESIGN.md
  § 3); at most one optional audio I/O crate, behind a feature, once live audio is approved. A
  backend is admitted only through an `AUDIO_BACKENDS` entry in
  `crates/patchback-cli/tests/dependencies.rs` with its exact version and reviewed closure, naming
  an AEP approval record (not rejected, superseded or archived) whose body has the line
  ``Approves: `<crate>` <version>``. No ModelSpec crate is admitted (DESIGN.md § 3, § 8.2).
- ModelSpec is pinned to a tested release; never assume a ModelSpec capability its documentation
  does not list as shipped (DESIGN.md § 1.1).
- Fixtures are synthesized by Patchback or carry a recorded licence and provenance.
- Use `Taskfile.yml` (go-task) for entry points; not a Makefile.

## Delivery

All commits and pushes are `b10x-bot[bot]` through `b10x-gates bot --repository
beyond10x/patchback`. Common Gates (`shared-gates.yml`) are enrolled in `gates-policy`.

## Releases

Patchback releases from source at bare-version tags (`0.1.0`, no `v`), after every merged wave;
every crate inherits the workspace version and nothing is on a registry.

- `CHANGELOG.md` follows Keep a Changelog 1.1.0. Changes collect under `## [Unreleased]`, one line
  per change a user sees, naming the public item in backticks and its story id where it fits.
- A release commit on `main` sets `[workspace.package] version` in `Cargo.toml` and `Cargo.lock`,
  turns `## [Unreleased]` into `## [<version>] - <date>` under a new empty `## [Unreleased]`, and
  moves the link references at the bottom. `task release-check TAG=<version>` passes on it.
- `check` and `Shared source gates` are green on that commit before it is tagged.
- The tag is the bot's, on that commit:
  `b10x-gates bot --repository beyond10x/patchback -- tag -a <version> -m "Patchback <version>" <commit>`,
  then `b10x-gates bot --repository beyond10x/patchback -- push origin <version>`.
- `.github/workflows/release.yml` runs on the tag: `release-check`, then `check.yml`, then it builds
  the `patchback` binary for Linux x86_64 and creates the GitHub Release with the version's
  CHANGELOG section as notes, `patchback-<version>-x86_64-unknown-linux-gnu` and its `.sha256`.
- A release is finished when the tag, the green `release` run and the Release with both assets are
  verified. Never move or delete a pushed tag; fix forward with the next version.
