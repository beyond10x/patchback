//! The seeded generator (DESIGN.md § 7.1, § 2 invariant 5, § 12.1 isolated splits, § 12.2 "no
//! invalid samples"): the split counts of `--samples`, the draw of one patch and note from a
//! patch seed, the order and isolation of the splits, the diagnostic set and the draw bound.
//!
//! Most cases drive [`Generator::run_with`] with a stand-in for the render and extraction, so a
//! refusal of each kind can be placed at a chosen draw; the cases that render go through the real
//! [`Pipeline`] (`patchback-dsp` and `features/1`), on 16 samples a split.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use patchback_analysis::{
    AnalysisError, FeatureContractId, FeatureError, FeatureSlot, NotComputableReason, ScalarFeature,
};
use patchback_core::parameter::{ParameterMapping, Topology};
use patchback_core::prng::{SplitMix64, Xoshiro256StarStar};
use patchback_dataset::{
    DatasetError, DiagnosticCause, DiagnosticKind, GATE_LONGEST_SECONDS, GATE_SHORTEST_SECONDS,
    GenerateError, Generated, Generator, MAX_DRAWS_PER_SAMPLE, MAX_SAMPLES, MIDI_HIGHEST,
    MIDI_LOWEST, PatchDraw, Pipeline, Rendering, SplitCounts, SplitRole,
};
use patchback_dsp::RenderError;

fn counts(train: i64, validation: i64, test: i64) -> SplitCounts {
    SplitCounts {
        train,
        validation,
        test,
    }
}

fn generator(seed: u64, counts: SplitCounts) -> Generator {
    Generator::new(seed, counts, PathBuf::from("data/"))
}

/// A stand-in feature vector that names its patch seed, so a sample can be traced to its draw.
fn features_of(draw: &PatchDraw) -> [f64; 32] {
    let mut x = [0.0; 32];
    x[0] = draw.patch_seed as f64;
    x
}

fn silence() -> DiagnosticCause {
    DiagnosticCause::from_feature(FeatureError::FeatureNotComputable {
        feature: FeatureSlot::Scalar(ScalarFeature::Rms),
        index: 24,
        reason: NotComputableReason::Analysis(AnalysisError::SilentInput { rms_dbfs: -90.0 }),
    })
    .unwrap()
}

fn noise() -> DiagnosticCause {
    DiagnosticCause::from_feature(FeatureError::NonFiniteFeature {
        feature_contract_id: FeatureContractId(1),
        index: 3,
    })
    .unwrap()
}

// ---------------------------------------------------------------- the split counts (D-a6-10)

#[test]
fn sixty_thousand_samples_are_the_first_dataset_of_section_7_1() {
    assert_eq!(
        SplitCounts::from_samples(60_000),
        counts(50_016, 4_992, 4_992)
    );
}

#[test]
fn forty_eight_samples_are_one_batch_a_split() {
    assert_eq!(SplitCounts::from_samples(48), counts(16, 16, 16));
}

#[test]
fn the_held_out_splits_never_go_below_one_batch() {
    assert_eq!(SplitCounts::from_samples(47), counts(15, 16, 16));
    assert_eq!(SplitCounts::from_samples(20), counts(-12, 16, 16));
    assert_eq!(SplitCounts::from_samples(0), counts(-32, 16, 16));
    // 12,000 · 4,992 / 60,000 = 998.4 → 62 batches → 992.
    assert_eq!(SplitCounts::from_samples(12_000), counts(10_016, 992, 992));
}

#[test]
fn the_largest_sample_count_does_not_overflow() {
    let counts = SplitCounts::from_samples(u64::MAX);
    assert!(counts.validation > 0 && counts.validation % 16 == 0);
    assert_eq!(counts.validation, counts.test);
    assert_eq!(
        counts.train,
        i64::MAX,
        "saturates, and an odd count is refused"
    );

    // Up to `MAX_SAMPLES`, the most `--samples` admits (Da6-26), nothing saturates: the train
    // count is the rule's, computed here in u128.
    assert_eq!(MAX_SAMPLES, i64::MAX as u64);
    let samples = u128::from(MAX_SAMPLES);
    let held_out = (samples * 4_992 / 60_000 / 16 * 16).max(16);
    let largest = SplitCounts::from_samples(MAX_SAMPLES);
    assert_eq!(i128::from(largest.validation), held_out as i128);
    assert_eq!(i128::from(largest.test), held_out as i128);
    assert_eq!(
        i128::from(largest.train),
        samples as i128 - 2 * held_out as i128
    );
}

/// Da6-19, Da6-27: a split's directory under `<out>` (DESIGN.md § 7.2, `data/val/`) and its record
/// name in `dataset.meta` and on stdout are two names; only the validation split's differ.
#[test]
fn a_split_is_written_under_its_directory_and_counted_under_its_record_name() {
    let names: Vec<(SplitRole, &str, &str)> = SplitRole::ALL
        .iter()
        .map(|&role| (role, role.dir(), role.key()))
        .collect();
    assert_eq!(
        names,
        [
            (SplitRole::Train, "train", "train"),
            (SplitRole::Validation, "val", "validation"),
            (SplitRole::Test, "test", "test"),
        ]
    );
}

#[test]
fn a_count_that_is_not_a_batch_multiple_is_refused_before_anything_renders() {
    let mut calls = 0;
    let refused = generator(42, SplitCounts::from_samples(47))
        .run_with(FeatureContractId(1), |draw| {
            calls += 1;
            Ok(Rendering::Sample(features_of(draw)))
        })
        .map(|_| ());
    assert_eq!(
        refused,
        Err(GenerateError::Dataset(
            DatasetError::SplitCountNotBatchMultiple {
                role: SplitRole::Train,
                sample_count: 15,
                batch_size: 16,
            }
        ))
    );
    assert_eq!(calls, 0);
}

// ---------------------------------------------------------- the draw of a patch (D-a6-8, 9)

#[test]
fn a_patch_draws_its_parameters_then_its_note_then_its_gate_from_its_own_stream() {
    for seed in [0, 1, 7, 42, u64::MAX] {
        let draw = PatchDraw::from_seed(seed);
        let mut prng = Xoshiro256StarStar::new(seed);
        let y: Vec<f64> = (0..8).map(|_| prng.next_f64()).collect();
        assert_eq!(draw.y.to_vec(), y);
        let midi = 36 + (prng.next_f64() * 49.0).floor() as u8;
        assert_eq!(draw.midi_note, midi);
        let seconds = 0.25 * 6f64.powf(prng.next_f64());
        assert_eq!(draw.gate_samples, (seconds * 48_000.0).round() as u64);
        assert_eq!(draw.patch_seed, seed);
        assert_eq!(PatchDraw::from_seed(seed), draw, "a seed names one draw");
    }
}

#[test]
fn draws_stay_in_the_declared_training_distribution() {
    assert_eq!((MIDI_LOWEST, MIDI_HIGHEST), (36, 84));
    assert_eq!((GATE_SHORTEST_SECONDS, GATE_LONGEST_SECONDS), (0.25, 1.5));
    let mut notes = HashSet::new();
    let mut means = [0.0f64; 8];
    let mut log_gate_mean = 0.0f64;
    let n = 4_000;
    let mut mix = SplitMix64::new(3);
    for _ in 0..n {
        let draw = PatchDraw::from_seed(mix.next_u64());
        assert!(draw.y.iter().all(|y| (0.0..1.0).contains(y)), "{draw:?}");
        assert!((36..=84).contains(&draw.midi_note), "{draw:?}");
        assert!((12_000..=72_000).contains(&draw.gate_samples), "{draw:?}");
        notes.insert(draw.midi_note);
        for (mean, y) in means.iter_mut().zip(draw.y) {
            *mean += y / n as f64;
        }
        let gate = draw.gate_samples as f64 / 48_000.0;
        log_gate_mean += (gate / 0.25).ln() / 6f64.ln() / n as f64;
    }
    assert_eq!(notes.len(), 49, "every note from 36 to 84 is drawn");
    for mean in means {
        assert!(
            (mean - 0.5).abs() < 0.02,
            "uniform in normalized space: {means:?}"
        );
    }
    // Log-uniform gates: the gate's position on the log axis is uniform, mean 0.5.
    assert!((log_gate_mean - 0.5).abs() < 0.02, "{log_gate_mean}");
}

/// § 7.1's stratification: uniform normalized values are uniform in log-time and log-frequency
/// for the exponential contracts, so half of the attacks lie below the geometric mean of the range.
#[test]
fn exponential_parameters_are_stratified_on_the_log_axis() {
    let topology = Topology::subtractive_mono();
    let mut mix = SplitMix64::new(11);
    let draws: Vec<PatchDraw> = (0..4_000)
        .map(|_| PatchDraw::from_seed(mix.next_u64()))
        .collect();
    for contract in topology.contracts() {
        if contract.mapping != ParameterMapping::Exponential {
            continue;
        }
        let geometric_mean = (contract.physical_at_zero * contract.physical_at_one).sqrt();
        let below = draws
            .iter()
            .filter(|draw| contract.to_physical(draw.y[contract.index]) < geometric_mean)
            .count();
        let share = below as f64 / draws.len() as f64;
        assert!((share - 0.5).abs() < 0.03, "{}: {share}", contract.path);
    }
}

// ------------------------------------------------------ the splits, in order and isolated

#[test]
fn patch_seeds_come_from_splitmix64_of_the_dataset_seed_in_split_order() {
    let generated = generator(42, counts(32, 16, 16))
        .run_with(FeatureContractId(1), |draw| {
            Ok(Rendering::Sample(features_of(draw)))
        })
        .unwrap();
    let mut mix = SplitMix64::new(42);
    for role in SplitRole::ALL {
        let seeds: Vec<u64> = generated
            .samples(role)
            .iter()
            .map(|sample| generated.patch_seed_of(sample))
            .collect();
        let expected: Vec<u64> = (0..seeds.len()).map(|_| mix.next_u64()).collect();
        assert_eq!(seeds, expected, "{role:?}");
        for sample in generated.samples(role) {
            assert_eq!(sample.x[0], generated.patch_seed_of(sample) as f64);
            assert_eq!(
                sample.y,
                PatchDraw::from_seed(generated.patch_seed_of(sample)).y
            );
        }
    }
    assert_eq!(generated.draws, 64);
    let sizes: Vec<usize> = SplitRole::ALL
        .iter()
        .map(|&role| generated.samples(role).len())
        .collect();
    assert_eq!(sizes, [32, 16, 16]);
}

#[test]
fn a_refused_patch_goes_to_the_diagnostic_set_and_the_next_seed_is_drawn() {
    let mut n = 0;
    let generated = generator(5, counts(16, 16, 16))
        .run_with(FeatureContractId(1), |draw| {
            n += 1;
            Ok(match n % 4 {
                1 => Rendering::Diagnostic(silence()),
                2 => Rendering::Diagnostic(noise()),
                3 => Rendering::Diagnostic(DiagnosticCause::limited(3)),
                _ => Rendering::Sample(features_of(draw)),
            })
        })
        .unwrap();
    assert_eq!(generated.draws, 4 * 48);
    assert_eq!(generated.diagnostic_count(DiagnosticKind::Silence), 48);
    assert_eq!(generated.diagnostic_count(DiagnosticKind::Noise), 48);
    assert_eq!(generated.diagnostic_count(DiagnosticKind::Clipping), 48);

    let mut mix = SplitMix64::new(5);
    let all: Vec<u64> = (0..4 * 48).map(|_| mix.next_u64()).collect();
    let diagnostic: Vec<u64> = generated
        .diagnostics()
        .iter()
        .map(|example| example.patch_seed)
        .collect();
    let expected: Vec<u64> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 4 != 3)
        .map(|(_, &seed)| seed)
        .collect();
    assert_eq!(
        diagnostic, expected,
        "every refused draw is recorded, in order"
    );
    for example in generated.diagnostics() {
        let draw = PatchDraw::from_seed(example.patch_seed);
        assert_eq!(
            (example.midi_note, example.gate_samples),
            (draw.midi_note, draw.gate_samples)
        );
    }

    let in_splits: HashSet<u64> = SplitRole::ALL
        .iter()
        .flat_map(|&role| generated.samples(role))
        .map(|sample| generated.patch_seed_of(sample))
        .collect();
    assert!(
        diagnostic.iter().all(|seed| !in_splits.contains(seed)),
        "a diagnostic example is never a sample"
    );
    assert_eq!(in_splits.len(), 48);
}

#[test]
fn the_draws_are_bounded_and_running_out_is_refused() {
    let wanted = counts(16, 16, 16);
    assert_eq!(MAX_DRAWS_PER_SAMPLE, 4);
    assert_eq!(generator(1, wanted).max_draws, 4 * 48);

    let mut calls = 0;
    let mut bounded = generator(1, wanted);
    bounded.max_draws = 20;
    let refused = bounded
        .run_with(FeatureContractId(1), |_| {
            calls += 1;
            Ok(Rendering::Diagnostic(silence()))
        })
        .map(|_| ());
    assert_eq!(
        refused,
        Err(GenerateError::Exhausted {
            draws: 20,
            samples: 0,
            wanted: 48,
        })
    );
    assert_eq!(calls, 20);
}

#[test]
fn a_render_fault_is_an_error_not_a_diagnostic() {
    let refused = generator(1, counts(16, 16, 16))
        .run_with(FeatureContractId(1), |_| {
            Err(GenerateError::Render(RenderError::RenderTooLong {
                frame_count: u64::MAX,
            }))
        })
        .map(|_| ());
    assert_eq!(
        refused,
        Err(GenerateError::Render(RenderError::RenderTooLong {
            frame_count: u64::MAX
        }))
    );
}

// ------------------------------------------------------------- the classification (D-a6-12)

#[test]
fn silence_refusals_are_silence_and_every_other_value_refusal_is_noise() {
    let not_computable = |reason| FeatureError::FeatureNotComputable {
        feature: FeatureSlot::Scalar(ScalarFeature::Rms),
        index: 24,
        reason,
    };
    let silence = [
        NotComputableReason::SilentWindow {
            start_sample: 0,
            end_sample: 10,
            rms_dbfs: -80.0,
        },
        NotComputableReason::Analysis(AnalysisError::SilentInput { rms_dbfs: -80.0 }),
    ];
    for reason in silence {
        let cause = DiagnosticCause::from_feature(not_computable(reason)).unwrap();
        assert_eq!(cause.kind(), DiagnosticKind::Silence, "{reason:?}");
    }
    let noise = [
        NotComputableReason::Analysis(AnalysisError::PitchAmbiguous { confidence: 0.1 }),
        NotComputableReason::Analysis(AnalysisError::PitchOutOfRange { bound_hz: 2_000.0 }),
        NotComputableReason::PitchAmbiguous { confidence: 0.1 },
        NotComputableReason::PitchAbsent { f0_hz: 0.0 },
        NotComputableReason::WindowTooShort {
            start_sample: 0,
            end_sample: 0,
        },
    ];
    for reason in noise {
        let cause = DiagnosticCause::from_feature(not_computable(reason)).unwrap();
        assert_eq!(cause.kind(), DiagnosticKind::Noise, "{reason:?}");
    }
    let non_finite = FeatureError::NonFiniteFeature {
        feature_contract_id: FeatureContractId(1),
        index: 0,
    };
    assert_eq!(
        DiagnosticCause::from_feature(non_finite).unwrap().kind(),
        DiagnosticKind::Noise
    );
    assert_eq!(
        DiagnosticCause::from_render(RenderError::NonFiniteRender { frame: 3 })
            .unwrap()
            .kind(),
        DiagnosticKind::Noise
    );
    assert_eq!(DiagnosticCause::limited(1).kind(), DiagnosticKind::Clipping);
}

#[test]
fn faults_of_the_contract_or_the_render_are_not_diagnostics() {
    let faults = [
        FeatureError::FeatureContractNotPublished {
            feature_contract_id: FeatureContractId(1),
        },
        FeatureError::FeatureContractNotFound {
            feature_contract_id: FeatureContractId(1),
        },
        FeatureError::FeatureVersionMismatch {
            version: "features/2".to_owned(),
        },
        FeatureError::FeatureLengthMismatch { value_count: 31 },
    ];
    for fault in faults {
        assert_eq!(
            DiagnosticCause::from_feature(fault.clone()),
            None,
            "{fault:?}"
        );
    }
    for fault in [
        RenderError::UnsupportedSampleRate {
            sample_rate_hz: 44_100,
        },
        RenderError::InvalidBlockSize {
            block_size_frames: 0,
        },
        RenderError::InvalidNoteTiming {
            start_sample: 5,
            end_sample: 5,
        },
        RenderError::RenderTooLong {
            frame_count: u64::MAX,
        },
    ] {
        assert_eq!(DiagnosticCause::from_render(fault), None, "{fault:?}");
    }
}

// ------------------------------------------------------------------ through the real renderer

/// The draw of `seed` with its parameters replaced.
fn with_parameters(seed: u64, y: [f64; 8], midi_note: u8) -> PatchDraw {
    let mut draw = PatchDraw::from_seed(seed);
    draw.y = y;
    draw.midi_note = midi_note;
    draw
}

#[test]
fn a_hot_patch_is_a_clipping_example() {
    let pipeline = Pipeline::new();
    // attack, decay, sustain, release, cutoff, Q, drive, output gain: full drive, a resonant
    // peak, no attenuation.
    let hot = with_parameters(1, [0.0, 0.99, 0.99, 0.1, 0.2, 0.99, 0.99, 0.99], 45);
    match pipeline.run(&hot).unwrap() {
        Rendering::Diagnostic(cause) => assert_eq!(cause.kind(), DiagnosticKind::Clipping),
        Rendering::Sample(_) => panic!("a limited render is no sample"),
    }
}

#[test]
fn a_patch_below_the_silence_threshold_is_a_silence_example() {
    let pipeline = Pipeline::new();
    // No sustain, the shortest decay and release, the lowest cutoff and gain, the highest note.
    let quiet = with_parameters(2, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 84);
    match pipeline.run(&quiet).unwrap() {
        Rendering::Diagnostic(cause) => assert_eq!(cause.kind(), DiagnosticKind::Silence),
        Rendering::Sample(x) => panic!("a silent patch is no sample: {x:?}"),
    }
}

#[test]
fn a_plain_patch_is_a_sample_of_32_finite_features() {
    let pipeline = Pipeline::new();
    let plain = with_parameters(3, [0.3, 0.5, 0.7, 0.4, 0.7, 0.1, 0.2, 0.7], 57);
    match pipeline.run(&plain).unwrap() {
        Rendering::Sample(x) => {
            assert!(x.iter().all(|v| v.is_finite()), "{x:?}");
            let contract = pipeline.contract();
            assert_eq!(contract.definition().version, "features/1");
            contract.check("features/1", &x).unwrap();
        }
        Rendering::Diagnostic(cause) => panic!("refused: {cause}"),
    }
}

/// One real generation of 16 samples a split from seed 42, shared by the cases below.
fn seed_42() -> &'static Generated {
    static GENERATED: OnceLock<Generated> = OnceLock::new();
    GENERATED.get_or_init(|| generator(42, counts(16, 16, 16)).run().unwrap())
}

#[test]
fn a_real_generation_fills_each_split_with_valid_isolated_samples() {
    let generated = seed_42();
    let contract = Pipeline::new();
    let mut seen = HashSet::new();
    for role in SplitRole::ALL {
        let samples = generated.samples(role);
        assert_eq!(samples.len(), 16, "{role:?}");
        for sample in samples {
            assert!(sample.x.iter().all(|v| v.is_finite()), "{sample:?}");
            contract.contract().check("features/1", &sample.x).unwrap();
            assert!(
                sample.y.iter().all(|y| (0.0..1.0).contains(y)),
                "{sample:?}"
            );
            let seed = generated.patch_seed_of(sample);
            assert!(seen.insert(seed), "patch seed {seed} is in two places");
            let draw = PatchDraw::from_seed(seed);
            assert_eq!(
                (sample.y, sample.midi_note, sample.gate_samples),
                (draw.y, draw.midi_note, draw.gate_samples)
            );
        }
    }
    for example in generated.diagnostics() {
        assert!(
            seen.insert(example.patch_seed),
            "diagnostic seed {} is also a sample",
            example.patch_seed
        );
        assert_eq!(example.kind, example.cause.kind());
        eprintln!(
            "diagnostic {:?} seed {} note {} gate {}: {}",
            example.kind,
            example.patch_seed,
            example.midi_note,
            example.gate_samples,
            example.cause
        );
    }
    let diagnostics: usize = [
        DiagnosticKind::Silence,
        DiagnosticKind::Noise,
        DiagnosticKind::Clipping,
    ]
    .iter()
    .map(|&kind| generated.diagnostic_count(kind))
    .sum();
    assert_eq!(generated.draws as usize, 48 + diagnostics);
    eprintln!(
        "seed 42, 48 samples: {} draws, silence {}, noise {}, clipping {}",
        generated.draws,
        generated.diagnostic_count(DiagnosticKind::Silence),
        generated.diagnostic_count(DiagnosticKind::Noise),
        generated.diagnostic_count(DiagnosticKind::Clipping)
    );
}

#[test]
fn the_same_seed_gives_the_same_samples_in_one_process() {
    let again = generator(42, counts(16, 16, 16)).run().unwrap();
    assert_eq!(digest(&again), digest(seed_42()));
}

/// FNV-1a over the bits of every sample's seed, note, gate, `x` and `y` in split order, and of
/// every diagnostic example's seed and kind.
fn digest(generated: &Generated) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bits: u64| {
        for byte in bits.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for role in SplitRole::ALL {
        for sample in generated.samples(role) {
            eat(generated.patch_seed_of(sample));
            eat(u64::from(sample.midi_note));
            eat(sample.gate_samples);
            sample.x.iter().for_each(|v| eat(v.to_bits()));
            sample.y.iter().for_each(|v| eat(v.to_bits()));
        }
    }
    for example in generated.diagnostics() {
        eat(example.patch_seed);
        eat(example.kind as u64);
    }
    format!("{hash:016x}")
}

const CHILD: &str = "PATCHBACK_DATASET_DIGEST_CHILD";
const DIGEST_TEST: &str = "the_same_seed_gives_the_same_samples_in_another_process";

/// § 2 invariant 5 across processes: this test binary runs itself again as a child, which
/// generates the same dataset and prints its digest.
#[test]
fn the_same_seed_gives_the_same_samples_in_another_process() {
    if std::env::var_os(CHILD).is_some() {
        println!("\ndigest {}", digest(seed_42()));
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", DIGEST_TEST, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let child = stdout
        .lines()
        .find_map(|line| line.split_once("digest ").map(|(_, digest)| digest.trim()))
        .unwrap_or_else(|| panic!("the child printed no digest: {stdout}"));
    assert_eq!(child, digest(seed_42()));
}
