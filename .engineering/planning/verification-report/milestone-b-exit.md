---
format: aep.planning-md/3
id: verification-report:milestone-b-exit
kind: verification-report
status: active
title: Milestone B exit not demonstrated by the ML-init benchmark (2026-11-01)
relations:
- verifies: epic:modelspec-regressor
- verifies: initiative:milestone-b
revision: 2
transitions:
- {from: "draft", to: "active", at: "2026-11-01T15:20:53Z", actor: "human:timo", revision: 2}
---
# Milestone B exit: the ML-init benchmark, 2026-11-01

## Verdict

**Not demonstrated.** DESIGN.md § 13 Milestone B's exit is "a ModelSpec-trained regressor
genuinely reduces matching work or improves audio fit". `epic:modelspec-regressor` moved to
implemented on 2026-10-25 (cf62bc3) when its stories were implemented; no benchmark result was
recorded against it (Da12-9 left the exit reading to the owner). This run is that reading, and on
this holdout the regressor's start does not change the fit's outcome beyond noise.

Open finding, accepted by Timo on 2026-11-01 (option A): Milestone C proceeds without the
regressor; this report stays open until a regressor improvement is measured on the same command.
The follow-up is `story:regressor-earns-the-exit`.

## The command

Release 0.11.0's binary (`patchback-0.11.0-x86_64-unknown-linux-gnu`, its `.sha256` checked),
from the repository root at ab1762c:

```console
patchback benchmark --kind ml-init --examples 24 --seed 7 \
  --weights models/demo/regressor.weights \
  --normalization models/demo/normalization.stats --out <scratch>/bench-ml
```

Wall time 19 min 51 s (1,191,214 ms recorded), exit 0.

## benchmark.txt, the summary lines

```text
patchback 0.11.0
kind ml-init
topology subtractive-mono/1
seed 7
example_count 24
max_renders 2000
max_wall_time_ms 60000
max_restarts 4
improvement_threshold 0.0001
weights sha256 a17d810459ccfa9ce41c171661a25d5313d1f9e575125062a17b4e51bf9be926
modelspec 0.5.0 7c45c1adaf6e0cc0ba05f18878b31bfdc57cc529
normalization sha256 6a1772acb04b1a4b992e29fe7804185072677fa82f1534f1e04c6e110a0810c7
feature_contract features/1
default_median_loss 0.4669448190308165
default_search_median_loss 0.0680875682807898
raw_regressor_median_loss 0.3719411437923529
regressor_search_median_loss 0.06042524258497957
default_search_median_evaluations 381
regressor_search_median_evaluations 366
clamped_predictions 5 of 24
```

## Reading

| figure | default start | regressor start |
| --- | --- | --- |
| loss at the start, median | 0.467 | 0.372 |
| loss after search, median | 0.068 | 0.060 |
| evaluations, median | 381 | 366 |
| clips whose search ended lower than the other start's (4 of 24 equal) | 9 of 24 | 11 of 24 |
| clips with fewer evaluations than the other start's | 10 of 24 | 14 of 24 |
| clips lower in loss and in evaluations | | 7 of 24 |
| predictions clamped | | 5 of 24 |

The counts are computed from `clips.txt` below (seeded best loss against best loss, seeded
evaluations against evaluations). The four Listen demos (`website/data/demos.json`, 0.10.0 demo
model) agree: the seeded fit spent more evaluations than the default fit on pluck, pad and bass,
fewer on lead, and ended higher on pluck and pad.

Inferred, not measured: the raw prediction starts 0.095 lower in median loss, and the
coordinate search from either start reaches about the same basin, so the start's advantage is
spent in the first restart. A regressor that helps would have to land inside the basin the
search ends in, or the search would have to use the prediction's uncertainty.

## clips.txt

| clip | default loss | default search best | evaluations | raw regressor loss | seeded best | seeded evaluations | clamped |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0.3728 | 0.1306 | 315 | 0.3190 | 0.0988 | 296 | 0 |
| 1 | 0.5373 | 0.0352 | 402 | 0.6724 | 0.0237 | 331 | 0 |
| 2 | 0.5631 | 0.0284 | 365 | 0.3482 | 0.0911 | 342 | 0 |
| 3 | 0.3620 | 0.0790 | 359 | 0.3719 | 0.0447 | 355 | 0 |
| 4 | 0.3058 | 0.0718 | 304 | 0.2788 | 0.0651 | 332 | 0 |
| 5 | 0.4659 | 0.0711 | 369 | 0.3054 | 0.0474 | 356 | 0 |
| 6 | 0.6320 | 0.0664 | 202 | 0.3934 | 0.0762 | 189 | 0 |
| 7 | 0.5526 | 0.0517 | 573 | 0.4708 | 0.0599 | 672 | 0 |
| 8 | 0.5797 | 0.0954 | 355 | 0.5174 | 0.0724 | 325 | 0 |
| 9 | 0.3793 | 0.1162 | 368 | 0.3363 | 0.1044 | 420 | 0 |
| 10 | 0.3809 | 0.0609 | 398 | 0.1744 | 0.0748 | 366 | 0 |
| 11 | 0.4447 | 0.1018 | 644 | 0.6552 | 0.0509 | 463 | 0 |
| 12 | 0.5276 | 0.0310 | 309 | 0.3131 | 0.0343 | 317 | 0 |
| 13 | 0.3447 | 0.1743 | 295 | 0.4597 | 0.1740 | 331 | 2 |
| 14 | 0.4288 | 0.0434 | 422 | 0.8656 | 0.0555 | 615 | 2 |
| 15 | 0.4731 | 0.0604 | 596 | 0.4137 | 0.0604 | 577 | 1 |
| 16 | 0.4609 | 0.0528 | 636 | 0.3297 | 0.0528 | 687 | 0 |
| 17 | 0.3606 | 0.0681 | 356 | 0.3579 | 0.0732 | 409 | 0 |
| 18 | 0.7431 | 0.1554 | 433 | 0.4745 | 0.1938 | 364 | 1 |
| 19 | 0.5779 | 0.0591 | 680 | 0.5371 | 0.0591 | 675 | 0 |
| 20 | 0.6245 | 0.0756 | 754 | 0.3351 | 0.0313 | 933 | 0 |
| 21 | 0.5765 | 0.0314 | 381 | 0.3903 | 0.0582 | 547 | 0 |
| 22 | 0.6447 | 0.1436 | 409 | 0.3698 | 0.1403 | 371 | 0 |
| 23 | 0.4669 | 0.0801 | 1235 | 0.3747 | 0.0801 | 1057 | 1 |
