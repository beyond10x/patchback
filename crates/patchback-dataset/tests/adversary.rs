//! Adversary cases for story:dataset-generation (wave a6, pass 1): model boundaries the unit's own
//! cases leave out, the one-note program and the note span the pipeline extracts over (neither is
//! pinned by a literal elsewhere), and a measurement of the `PitchAmbiguous` diagnostics of the
//! seed-42 generation.

use std::path::PathBuf;

use patchback_analysis::{
    AnalysisConfig, AnalysisError, FeatureContractId, FeatureError, NotComputableReason, NoteSpan,
    analyze, analyze_and_extract, estimate_f0,
};
use patchback_core::parameter::Topology;
use patchback_core::patch::{compile, validate};
use patchback_core::prng::NAME;
use patchback_dataset::{
    DatasetError, DatasetStore, DiagnosticKind, GenerateDataset, Generator, PatchDraw, Pipeline,
    PrngAlgorithm, Rendering, SampledPatchId, SplitCounts, SplitRole,
};
use patchback_dsp::{RenderConfig, render};

fn dataset(store: &mut DatasetStore) -> patchback_dataset::DatasetId {
    store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm(NAME.to_owned()),
        batch_size: 16,
        feature_contract_id: FeatureContractId(1),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    })
}

// ------------------------------------------------------------------ model boundaries

#[test]
fn adversary_open_split_refuses_every_count_that_is_not_a_positive_batch_multiple() {
    let mut store = DatasetStore::new();
    let id = dataset(&mut store);
    for count in [50_008, 15, 1, 0, -1, i64::MIN, i64::MAX, 50_000 + 8] {
        assert_eq!(
            store.open_split(id, SplitRole::Train, count),
            Err(DatasetError::SplitCountNotBatchMultiple {
                role: SplitRole::Train,
                sample_count: count,
                batch_size: 16,
            }),
            "{count}"
        );
    }
    assert_eq!(store.splits_of(id).count(), 0);
    // The largest positive multiple of 16 an i64 holds opens.
    let largest = i64::MAX - 15;
    let split = store.open_split(id, SplitRole::Train, largest).unwrap();
    assert_eq!(store.split(split).unwrap().sample_count, largest);
}

#[test]
fn adversary_a_seed_drawn_into_test_is_refused_in_train_and_in_validation() {
    let mut store = DatasetStore::new();
    let id = dataset(&mut store);
    let train = store.open_split(id, SplitRole::Train, 16).unwrap();
    let validation = store.open_split(id, SplitRole::Validation, 16).unwrap();
    let test = store.open_split(id, SplitRole::Test, 16).unwrap();
    store.draw_patch(id, test, 99).unwrap();
    for split in [train, validation, test] {
        assert_eq!(
            store.draw_patch(id, split, 99),
            Err(DatasetError::PatchAlreadyDrawn {
                patch_seed: 99,
                dataset_split_id: split,
            })
        );
    }
    assert_eq!(store.patches_of(id).count(), 1);
}

#[test]
fn adversary_a_patch_of_another_store_with_the_same_seed_is_unknown() {
    let mut first = DatasetStore::new();
    let mut second = DatasetStore::new();
    let a = dataset(&mut first);
    dataset(&mut second);
    let train = first.open_split(a, SplitRole::Train, 16).unwrap();
    let patch = first.draw_patch(a, train, 7).unwrap();
    assert_eq!(patch, SampledPatchId(1));
    assert_eq!(
        second.add_sample(patch, 60, 24_000, [0.0; 32], [0.0; 8]),
        Err(DatasetError::UnknownPatch {
            sampled_patch_id: patch
        })
    );
}

// ------------------------------------------------------------------ the one-note program (D-a6-9)

/// D-a6-9: one note, at sample 0, ending at the gate, velocity 1.0, at 48 kHz. No other case
/// reads `PatchDraw::notes`, so a velocity or a start other than these passes the suite.
#[test]
fn adversary_the_note_starts_at_sample_zero_ends_at_the_gate_at_velocity_one() {
    for seed in [0, 42, u64::MAX] {
        let draw = PatchDraw::from_seed(seed);
        let notes = draw.notes();
        assert_eq!(notes.sample_rate_hz, 48_000);
        assert_eq!(notes.notes.len(), 1);
        let note = &notes.notes[0];
        assert_eq!(note.start_sample, 0, "{seed}");
        assert_eq!(note.end_sample, draw.gate_samples, "{seed}");
        assert_eq!(note.midi_note, draw.midi_note, "{seed}");
        assert_eq!(note.velocity, 1.0, "{seed}");
    }
}

// ------------------------------------------------------------------ the extraction span

/// The pipeline extracts `features/1` over the note, samples `0..gate`, not over the whole render
/// (which runs on through the release and the tail). No other case compares `x` with an
/// extraction made outside the pipeline, so a span over the whole render passes the suite.
#[test]
fn adversary_the_sample_x_is_the_extraction_over_the_note_span() {
    let pipeline = Pipeline::new();
    let mut draw = PatchDraw::from_seed(3);
    draw.y = [0.3, 0.5, 0.7, 0.4, 0.7, 0.1, 0.2, 0.7];
    draw.midi_note = 57;
    let x = match pipeline.run(&draw).unwrap() {
        Rendering::Sample(x) => x,
        Rendering::Diagnostic(cause) => panic!("refused: {cause}"),
    };
    let validated = validate(&draw.patch(&Topology::subtractive_mono())).unwrap();
    let rendered = render(
        &compile(&validated),
        &draw.notes(),
        &RenderConfig::default(),
    )
    .unwrap();
    let gate = usize::try_from(draw.gate_samples).unwrap();
    assert!(
        rendered.samples.len() > gate,
        "the render runs past the gate"
    );
    let over_note = analyze_and_extract(
        pipeline.contract(),
        &rendered.samples,
        48_000,
        NoteSpan {
            start_sample: 0,
            end_sample: gate,
        },
    )
    .unwrap();
    assert_eq!(x.to_vec(), over_note.values);
    let over_render = analyze_and_extract(
        pipeline.contract(),
        &rendered.samples,
        48_000,
        NoteSpan {
            start_sample: 0,
            end_sample: rendered.samples.len(),
        },
    )
    .map(|vector| vector.values);
    assert_ne!(
        over_render,
        Ok(over_note.values.clone()),
        "the span changes x (or refuses it), so this case tells the two apart"
    );
}

// ------------------------------------------------------------------ PitchAmbiguous at 0.500

fn pitch_confidence(error: &FeatureError) -> Option<f64> {
    match error {
        FeatureError::FeatureNotComputable {
            reason:
                NotComputableReason::Analysis(AnalysisError::PitchAmbiguous { confidence })
                | NotComputableReason::PitchAmbiguous { confidence },
            ..
        } => Some(*confidence),
        _ => None,
    }
}

fn render_draw(draw: &PatchDraw) -> Vec<f32> {
    let validated = validate(&draw.patch(&Topology::subtractive_mono())).unwrap();
    render(
        &compile(&validated),
        &draw.notes(),
        &RenderConfig::default(),
    )
    .unwrap()
    .samples
}

fn outcome(samples: &[f32]) -> String {
    match estimate_f0(samples, 48_000) {
        Ok(estimate) => format!(
            "f0 {:.2} Hz conf {:.3}",
            estimate.f0_hz, estimate.confidence
        ),
        Err(AnalysisError::PitchAmbiguous { confidence }) => format!("ambiguous {confidence:.3}"),
        Err(error) => format!("{error}"),
    }
}

/// A measurement, not a finding of this unit: the seed-42 generation of 16 samples a split, its
/// `Noise` examples, and for three refused at a confidence of exactly 0.5 the parameters, the
/// note's f0, `analyze` over the whole render, and `estimate_f0` after changing one parameter
/// group at a time. The assertions hold what was measured; the rest is printed.
#[test]
fn adversary_characterise_pitch_ambiguous_at_one_half() {
    let generated = Generator::new(
        42,
        SplitCounts {
            train: 16,
            validation: 16,
            test: 16,
        },
        PathBuf::from("data/"),
    )
    .run()
    .unwrap();
    let topology = Topology::subtractive_mono();
    let noise: Vec<_> = generated
        .diagnostics()
        .into_iter()
        .filter(|e| e.kind == DiagnosticKind::Noise)
        .collect();
    let confidences: Vec<Option<f64>> = noise
        .iter()
        .map(|e| e.cause.feature_error().and_then(pitch_confidence))
        .collect();
    let half: Vec<_> = noise
        .iter()
        .zip(&confidences)
        .filter(|(_, c)| **c == Some(0.5))
        .map(|(e, _)| *e)
        .collect();
    eprintln!(
        "noise {}: PitchAmbiguous {}, exactly 0.5 {}",
        noise.len(),
        confidences.iter().filter(|c| c.is_some()).count(),
        half.len()
    );
    let describe = |seed: u64, draw: &PatchDraw| {
        let physical: Vec<String> = topology
            .contracts()
            .iter()
            .map(|c| format!("{}={:.4}", c.path, c.to_physical(draw.y[c.index])))
            .collect();
        let f0 = 440.0 * 2f64.powf((f64::from(draw.midi_note) - 69.0) / 12.0);
        let cutoff = topology
            .contract("filter_cutoff")
            .unwrap()
            .to_physical(draw.y[4]);
        format!(
            "seed {seed} midi {} f0 {f0:.2} Hz period {:.3} gate {} cutoff/f0 {:.3} | {}",
            draw.midi_note,
            48_000.0 / f0,
            draw.gate_samples,
            cutoff / f0,
            physical.join(" ")
        )
    };
    for (example, confidence) in noise.iter().zip(&confidences) {
        let draw = PatchDraw::from_seed(example.patch_seed);
        eprintln!(
            "noise conf {confidence:?}: {}",
            describe(example.patch_seed, &draw)
        );
    }
    for role in SplitRole::ALL {
        for sample in generated.samples(role) {
            let seed = generated.patch_seed_of(sample);
            eprintln!(
                "sample {role:?}: {}",
                describe(seed, &PatchDraw::from_seed(seed))
            );
        }
    }
    assert!(half.len() >= 3, "three examples at 0.5");
    for example in half.iter().take(3) {
        let draw = PatchDraw::from_seed(example.patch_seed);
        let samples = render_draw(&draw);
        let analysed = analyze(&samples, 48_000, &AnalysisConfig::default());
        eprintln!("--- {}", describe(example.patch_seed, &draw));
        eprintln!(
            "  analyze(whole render, {} samples): {:?}",
            samples.len(),
            analysed.as_ref().map(|a| (a.f0.f0_hz, a.f0.confidence))
        );
        assert_eq!(
            analysed.map(|_| ()),
            Err(AnalysisError::PitchAmbiguous { confidence: 0.5 })
        );
        let gate = usize::try_from(draw.gate_samples).unwrap();
        eprintln!("  estimate_f0(note span): {}", outcome(&samples[..gate]));
        let probes: [(&str, &[(usize, f64)]); 6] = [
            ("drive 0 dB", &[(6, 0.0)]),
            ("resonance_q 0.5", &[(5, 0.0)]),
            ("cutoff 16 kHz", &[(4, 0.999_999)]),
            ("cutoff x0.5 (normalized -0.1)", &[(4, -0.1)]),
            ("flat envelope", &[(0, 0.0), (2, 0.999_999)]),
            ("output_gain 0 dB", &[(7, 0.999_999)]),
        ];
        for (name, changes) in probes {
            let mut probe = draw;
            for &(index, value) in changes {
                probe.y[index] = if value < 0.0 {
                    (probe.y[index] + value).max(0.0)
                } else {
                    value
                };
            }
            eprintln!("  {name}: {}", outcome(&render_draw(&probe)));
        }
    }
}
