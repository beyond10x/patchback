---
format: aep.planning-md/3
id: story:training-run
kind: story
status: draft
title: CPU training run
relations:
- decomposes: epic:modelspec-regressor
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:modelspec-pin-and-check
- depends_on: story:dense-data-files
revision: 1
---
# CPU training run

## Outcome

`modelspec train --backend cpu --data <train> --eval <val>` runs as a subprocess from Rust on real
generated data for a checked model; the run records its loss curve and completes or fails. A run of
an unchecked model, on unwritten data, or on the Vulkan backend is refused.

## DESIGN.md

§ 1.1, § 7.2, § 8.1, § 8.2 step 1, § 12.2 (ModelSpec bridge gate), § 13 Milestone B.

## Acceptance

- spec/scenarios/training-run-completed.yaml
- spec/scenarios/training-run-refused-unchecked-model.yaml
- spec/scenarios/training-run-refused-unwritten-data.yaml
- spec/scenarios/training-run-failed-and-vulkan-refused.yaml

## Numeric obligations

- The `--eval` split count is a multiple of the batch size (§ 7.2).
- Training loss decreases on the real generated data (§ 12.2).
- Evaluation loss, not `accuracy`, is what is reported (§ 8.1).

## Open questions

- Whether training on a validation or test split, or evaluating on train, is refused.
- The Plan, Bind, Start, Finish command split is a modelling choice not in DESIGN.md.
