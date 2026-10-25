---
title: Train the regressor
sidebar_position: 3
description: Generate a deterministic paired dataset with Patchback, check and train the PatchRegressor with ModelSpec on the CPU, and hand the weights back to Patchback.
lede: "Render a paired dataset with Patchback, train the regressor with ModelSpec on the CPU, and let Patchback run the exported weights."
source: "DESIGN.md § 7, § 8"
---

# Train the regressor

The regressor gives the fit a better place to start. Training it takes two tools: Patchback
renders the dataset, [ModelSpec](https://beyond10x.github.io/modelspec/)
([GitHub](https://github.com/beyond10x/modelspec)) trains the model on it.

## Generate the dataset

```bash
patchback generate-dataset --seed 42 --samples 60000 --out data/
```

Patchback samples valid normalized patches and one-note event programs from its seeded
generator, renders each through its engine, extracts [`features/1`](../concepts/listening.md#features1)
and writes the pairs. The same seed gives the same dataset. Log-time and log-frequency
parameters are stratified rather than sampled uniformly, and pitches and gate lengths stay in
a declared range and are kept as metadata.

The 60,000 patches split into 50,016 for training, 4,992 for validation and 4,992 for testing,
each a multiple of the batch size of 16. The split is by patch identity: one patch never
appears in two splits, even at different notes. Silence, noise and clipping cases go to a
separate diagnostic set.

```text
data/normalization.stats per-feature mean and standard deviation of the train split
data/train/x.f32         N × 32 f32 z-scored feature matrix, one sample per row
data/train/y.f32         N × 8  f32 normalized target matrix
data/train/order.u32     N u32 sample indices, one per row
data/val/x.f32
data/val/y.f32
data/val/order.u32
```

The files are row-major, little-endian `f32`, named after the model's `input` and `target`.
The writer checks the exact byte counts, the sample counts and that no value is NaN or
infinite. `x.f32` is already normalized: every split, validation and test included, holds its
features z-scored under the training split's statistics, which are recorded in
[`normalization.stats`](../reference/files.md#normalizationstats). ModelSpec reads the
directories as written.

## Check and train

The model source is `models/patch_regressor.mspec`, shown on the
[regressor page](../concepts/regressor.md). Check it, inspect its training program, then
train on the CPU:

```bash
modelspec check models/patch_regressor.mspec
modelspec ir models/patch_regressor.mspec --stage training
modelspec train models/patch_regressor.mspec --backend cpu \
  --data data/train --eval data/val --steps 3126
```

The leading dimension in the model is the minibatch size of 16, not the dataset size, and the
validation count must be a multiple of it. Patchback writes one `order.u32` index per row, and
ModelSpec 0.5.0 reads steps × 16 of them, so pass `--steps` with steps × 16 at most the train
split's row count: for the 50,016-row split, `--steps 3126` at most. Without `--steps` the
model's own 5,000 steps need 80,000 indices, and ModelSpec refuses the run with exit status 1:

```text
E-DATA-008: the order file holds fewer than steps · B indices
  indices: 50016
  needed: 80000
```

The run prints a loss line per reported step and, with `--eval`, one evaluation line. On a
64-row training split with `--steps 4` and 16 validation rows, the pinned binary printed:

```text
step 0 loss 1.0717294
step 4 loss 0.8882302
eval loss=0.9350907802581787 accuracy=0.0625
```

For a regression like this one, judge the run by its evaluation loss and the audio tests, not
by the `accuracy` value `--eval` also prints.

Patchback runs the same command as a subprocess through `patchback_model::TrainingRun`, a
library call with no `patchback` command yet, under a 600 s timeout (provisional). The exit
status alone decides the run: `Completed` means ModelSpec exited 0, and any other status, a
signal or the timeout make it `Failed`, `E-DATA-008` included. Check `output_error()` before
reading the loss curve. A missing `step` line, or a malformed line, leaves the curve empty and
says which line; a missing `eval` line on a run given `--eval` keeps the curve and reports only
that.

## Hand the weights to Patchback

ModelSpec has no inference or checkpoint interface Patchback relies on, so Patchback's model
adapter runs the same `modelspec train` as a subprocess, against the pinned ModelSpec version.
It reads only the documented `param @PatchRegressor.w1` and `.w2` outputs and checks the
names, the shapes `[32, 64]` and `[64, 8]`, the element counts, that every value is finite and
the exit status. Output it does not recognize is refused, not approximated. The weights are
stored as a checksummed, versioned little-endian `f32` artifact together with the model and
feature-contract versions, and Patchback's native predictor runs them.

## Did it help

A trained regressor is worth keeping when fits seeded with it beat fits from the default
patch. Patchback's benchmark over a seeded holdout set compares three results: the default
patch, the raw prediction, and the prediction refined by search.
