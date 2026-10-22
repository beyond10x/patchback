---
format: aep.planning-md/3
id: executable-system-specification:patchback-contracts
kind: executable-system-specification
status: draft
title: Patchback application contracts (ESS)
relations:
- serves: vision:patchback
revision: 1
---
# Patchback contracts (ESS)

The executable specification of Patchback's application contracts: `spec/system.yaml`, listed with
every domain file and every authored scenario in `spec/ess-inputs.yaml`.

| Domain | File | Nouns |
| --- | --- | --- |
| `patchback.patch` | `spec/domains/patch.yaml` | Patch, Node, Edge, Topology |
| `patchback.score` | `spec/domains/score.yaml` | NoteSequence |
| `patchback.audio` | `spec/domains/audio.yaml` | AudioClip, Render |
| `patchback.analysis` | `spec/domains/analysis.yaml` | Analysis, FeatureContract, NormalizationStats |
| `patchback.dataset` | `spec/domains/dataset.yaml` | Dataset, DatasetSplit, DenseDataFile |
| `patchback.model` | `spec/domains/model.yaml` | ModelSpecPin, ModelSource, TrainingRun, WeightArtifact |
| `patchback.fit` | `spec/domains/fit.yaml` | FitRun, Candidate, Comparison, Benchmark |
| `patchback.dependency` | `spec/domains/dependency.yaml` | DependencyAudit |

Validation: `task spec-validate`. Conformance against the interpreted reference target:
`task spec-conform`. There is no implementation yet, so this artifact stays `validated` until a
Rust target answers the suite.
