# Patchback

Give Patchback a recording of a synth and it gives back a patch: a small, editable DSP program
(oscillator, drive, filter, envelope, gain) plus the note events that play it. It renders that
patch, compares the rendering with the recording, and refines the patch until the two sound
close. The output is a program you can open and tweak, not opaque audio from a neural decoder.

**Status:** Milestone A in progress. `patchback patch validate` checks a `.patch` file and
`patchback analyze` measures a WAV; rendering, comparison and fitting are not implemented yet. The
ESS specification is in [spec/](spec/) and the plan in `.engineering/`. The first milestone works on
one isolated monophonic synth note and uses no machine learning. The patch studio prototype in [studio/](studio/) draws modules,
patch graphs and a fit replay from illustrative fixture data (`task studio-dev`).

Everything is Rust with no third-party ML, DSP or maths crates. The parameter regressor that
seeds the search is trained with [ModelSpec](https://github.com/beyond10x/modelspec) on its CPU
backend.

## Where to read

| File | What it holds |
| --- | --- |
| [DESIGN.md](DESIGN.md) | The design: patch contract, DSP rules, analysis, dataset, ModelSpec model, closed-loop fit, milestones, ADRs |
| [spec/](spec/) | ESS domains for every noun in the design and the conformance scenarios each story is accepted by |
| [website/docs/](website/docs/) | The documentation site's pages; published at `https://beyond10x.github.io/patchback/` once the first push to `main` deploys it |
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
