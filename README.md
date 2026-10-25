# Patchback

Give Patchback a recording of a synth and it gives back a patch: a small, editable DSP program
(oscillator, drive, filter, envelope, gain) plus the note events that play it. It renders that
patch, compares the rendering with the recording, and refines the patch until the two sound
close. The output is a program you can open and tweak, not opaque audio from a neural decoder.

**Status:** Milestones A and B of [DESIGN.md § 13](DESIGN.md) are implemented. `patchback render`,
`analyze`, `compare` and `fit` recover one isolated monophonic synth note without machine learning
(Milestone A); `patchback generate-dataset` writes the regressor's training data, the pinned
[ModelSpec](https://github.com/beyond10x/modelspec) trains it on CPU, `patchback fit --weights`
starts the search from its prediction and `patchback benchmark --kind ml-init` measures the
default start against the seeded one over a seeded holdout (Milestone B). Milestone C, phrases of
several notes, is next. The per-capability status with its evidence is on the
[documentation site](https://beyond10x.github.io/patchback/docs/status), the ESS specification in
[spec/](spec/) and the plan in `.engineering/`. The patch studio prototype in [studio/](studio/)
draws modules, patch graphs and a fit replay from illustrative fixture data (`task studio-dev`).

The documentation site shows how far Patchback has come and lets you hear it.
[Progress](https://beyond10x.github.io/patchback/docs/progress) lists every milestone with its
stories, their status and the release that delivered each one.
[Listen](https://beyond10x.github.io/patchback/docs/listen) plays four synth voices beside what
the default-start fit and the regressor-seeded fit recovered from each recording, and the
regressor's own prediction, with the parameters and losses next to them.

Everything is Rust with no third-party ML, DSP or maths crates. The parameter regressor that
seeds the search is trained with [ModelSpec](https://github.com/beyond10x/modelspec) on its CPU
backend.

## Where to read

| File | What it holds |
| --- | --- |
| [DESIGN.md](DESIGN.md) | The design: patch contract, DSP rules, analysis, dataset, ModelSpec model, closed-loop fit, milestones, ADRs |
| [spec/](spec/) | ESS domains for every noun in the design and the conformance scenarios each story is accepted by |
| [website/docs/](website/docs/) | The documentation site's pages; published at `https://beyond10x.github.io/patchback/` |
| [CHANGELOG.md](CHANGELOG.md) | What changed, per release |
| [AGENTS.md](AGENTS.md) | Rules for agents working in this repository |

## Check and build

```console
task check                                   # specification, conformance, plan, generated pages
npm --prefix website ci --allow-git=root     # site dependencies (npm 12)
npm --prefix website run build               # static site in website/build
```

## Milestones

| Milestone | Delivers |
| --- | --- |
| A | Sound engine without ML: WAV I/O, fixed subtractive voice, analysis, audio loss, bounded search |
| B | Native dataset + ModelSpec M0 regressor, weight export, native predictor |
| C | Monophonic phrases: onset, pitch and gate estimation with confidence |
| D | Candidate topologies, modulation, delayed feedback |
| E | Stems, drums, arrangement; GPU training once ModelSpec ships it |

## License

Apache 2.0, see [LICENSE](LICENSE).
