---
title: The regressor
sidebar_position: 5
description: A fixed-shape two-layer network, trained with ModelSpec on audio Patchback renders itself, that predicts where the patch search starts.
lede: "A fixed-shape two-layer network trained with ModelSpec on Patchback's own renderings proposes where the search starts."
source: "DESIGN.md § 1.1, § 7, § 8 (ADR-001, ADR-006)"
---

# The regressor

The regressor answers one question: given the 32 [features](./listening.md#features1) of a
note, which eight parameter values should the search try first? It is a fixed-shape network
with one hidden layer, specified in ModelSpec's language and trained by
[ModelSpec](https://beyond10x.github.io/modelspec/)
([GitHub](https://github.com/beyond10x/modelspec)) on its CPU backend.

```text
model PatchRegressor {
    param w1: Tensor<f32, [32, 64]> = xavier_uniform();
    param w2: Tensor<f32, [64, 8]> = xavier_uniform();

    fn forward(x: Tensor<f32, [16, 32]>) -> Tensor<f32, [16, 8]> {
        let h = relu(matmul(x, w1));
        return matmul(h, w2);
    }
}

train FitPatches {
    model = PatchRegressor;
    input x: Tensor<f32, [16, 32]>;
    target y: Tensor<f32, [16, 8]>;
    loss = mse(model.forward(x), y);
    optimizer = sgd(lr = 0.01);
    steps = 5000;
    seed = 42;
    device = cpu;
}
```

The model uses only operations ModelSpec documents as shipped: `matmul`, `relu`, `mse` and
SGD, with static shapes. The leading dimension, 16, is the minibatch size. The step count and
learning rate are experiment parameters, not tuned values.

## Trained on its own renderings

Patchback does not scrape tracks for training data. It samples valid patches, renders each
with a one-note event program through the same engine that later renders fits, extracts
`features/1` and writes the pair: 32 features in, 8 normalized parameters out. The labels and
the audio therefore always agree. The [training guide](../guides/train-the-regressor.md) walks
through the dataset and the ModelSpec commands.

## From training to prediction

ModelSpec trains the model; Patchback runs it. Between the two sits a narrow, version-locked
bridge:

1. Patchback runs `modelspec train` as a subprocess.
2. It reads only the documented parameter outputs for `w1` and `w2` of the pinned ModelSpec
   version, and checks names, shapes (`[32, 64]` and `[64, 8]`), element counts, finiteness and
   the exit status. Output it does not recognize is refused.
3. It stores the weights as a checksummed, versioned little-endian `f32` artifact, together
   with the model and feature-contract versions.
4. A small native predictor, `patchback_model::Predictor`, runs the two dense layers and the ReLU
   in `f32` on one normalized `features/1` vector, clamps the outputs to [0, 1] and counts the
   predictions that needed it. Golden tests compare its output with independently computed
   reference results, and on a pinned ModelSpec run its error on the evaluation rows matches
   ModelSpec's own `eval loss`.

When ModelSpec ships its own inference or checkpoint interface and it passes equivalence tests,
it replaces this bridge. The bridge is not a general neural-network framework, and Patchback
adds no ML library to work around anything ModelSpec does not do yet.

## What the prediction is worth

The prediction only starts the [search](./closed-loop.md#the-search). Its outputs may leave
`[0, 1]`; they are clamped with a warning, and how often that happens is measured. The
regressor earns its place when a fit seeded by it beats a fit from the default patch. The ML-init
benchmark (`patchback benchmark --kind ml-init`) compares four arms over one seeded holdout: the
default patch, the default patch refined by search, the raw prediction, and the prediction
refined by search, with the same search bounds and seed for both searches. It records the four
median losses, the two searches' median evaluations and the clamped predictions, and no verdict;
reading them is the owner's call.

GPU training moves to ModelSpec's GPU backend once that backend ships and passes CPU/GPU
equivalence tests.
