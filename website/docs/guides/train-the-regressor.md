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
data/train/x.f32         N × 32 f32 feature matrix, one sample per row
data/train/y.f32         N × 8  f32 normalized target matrix
data/train/order.u32     optional, checked sequence of sample indices
data/val/x.f32
data/val/y.f32
```

The files are row-major, little-endian `f32`, named after the model's `input` and `target`.
The writer checks the exact byte counts, the sample counts and that no value is NaN or
infinite.

## Check and train

The model source is `models/patch_regressor.mspec`, shown on the
[regressor page](../concepts/regressor.md). Check it, inspect its training program, then
train on the CPU:

```bash
modelspec check models/patch_regressor.mspec
modelspec ir models/patch_regressor.mspec --stage training
modelspec train models/patch_regressor.mspec --backend cpu \
  --data data/train --eval data/val
```

The leading dimension in the model is the minibatch size, not the dataset size, and the
validation count must be a multiple of it. Without `order.u32`, ModelSpec reads the samples in
order, cycling. For a regression like this one, judge the run by its evaluation loss and the
audio tests, not by the `accuracy` value `--eval` also prints.

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
