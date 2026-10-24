//! The feature contract `features/1` against the acceptance scenarios of story:feature-contract
//! (`spec/scenarios/feature-contract-*.yaml`), one test per scenario, plus the numeric
//! obligations of the story: 32 finite values in contract order, window-major band energies,
//! repeatable extraction, and a structured refusal where a feature cannot be computed.

use std::f64::consts::PI;
use std::ops::Range;

use patchback_analysis::{
    ANALYSIS_FFT_LENGTHS, ATTACK_WINDOW_FRAMES, Analysis, AnalysisConfig, AnalysisError,
    BAND_COUNT, BandWindow, FEATURES_1_BAND_WINDOWS, FEATURES_1_SCALARS, FEATURES_1_VALUE_COUNT,
    FEATURES_1_VERSION, FeatureContractDefinition, FeatureContractId, FeatureContractState,
    FeatureError, FeatureRegistry, FeatureSlot, LOG_FLOOR_DB, NotComputableReason, NoteSpan,
    RELEASE_OFFSET_FRAMES, RELEASE_WINDOW_FRAMES, SAMPLE_RATE_HZ, ScalarFeature, analyze,
    analyze_and_extract, attack_envelope_slope, extract_features, zero_crossing_rate,
};

const RATE: f64 = SAMPLE_RATE_HZ as f64;

/// Adds `amplitude sin(2 pi f i / rate)` to `samples[i]` for `i` in `range`.
fn add_sine(samples: &mut [f32], range: Range<usize>, frequency_hz: f64, amplitude: f64) {
    for (i, sample) in samples
        .iter_mut()
        .enumerate()
        .take(range.end)
        .skip(range.start)
    {
        *sample += (amplitude * (2.0 * PI * frequency_hz * i as f64 / RATE).sin()) as f32;
    }
}

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    let mut samples = vec![0.0; count];
    add_sine(&mut samples, 0..count, frequency_hz, amplitude);
    samples
}

fn noise(count: usize) -> Vec<f32> {
    let mut state: u64 = 11;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5) as f32
        })
        .collect()
}

/// The note every extraction test plays: gate from 0 to half a second.
const NOTE: NoteSpan = NoteSpan {
    start_sample: 0,
    end_sample: 24_000,
};

fn definition(value_count: usize, fft_lengths: &[usize]) -> FeatureContractDefinition {
    FeatureContractDefinition {
        version: FEATURES_1_VERSION.to_string(),
        value_count,
        band_count: 8,
        band_windows: vec![BandWindow::Attack, BandWindow::Body, BandWindow::Release],
        scalar_features: vec![
            ScalarFeature::Rms,
            ScalarFeature::Peak,
            ScalarFeature::SpectralCentroid,
            ScalarFeature::Rolloff,
            ScalarFeature::SpectralFlatness,
            ScalarFeature::ZeroCrossingRate,
            ScalarFeature::AttackEnvelopeSlope,
            ScalarFeature::NormalizedMidiPitch,
        ],
        fft_lengths: fft_lengths.to_vec(),
    }
}

fn published() -> (FeatureRegistry, FeatureContractId) {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    (registry, id)
}

fn analysed(samples: &[f32]) -> Analysis {
    analyze(samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()).unwrap()
}

/// The index of a value of `features/1` in its vector.
fn index_of(slot: FeatureSlot) -> usize {
    FeatureContractDefinition::features_1()
        .slots()
        .iter()
        .position(|s| *s == slot)
        .unwrap()
}

/// feature-contract-published: `features/1` is defined as a draft with 32 values, eight bands
/// over attack, body and release, the eight scalars in DESIGN.md's order and the FFT lengths
/// 512, 2048, 8192, then published.
#[test]
fn feature_contract_published() {
    let features_1 = FeatureContractDefinition::features_1();
    assert_eq!(features_1, definition(32, &[512, 2_048, 8_192]));
    assert_eq!(features_1.version, FEATURES_1_VERSION);
    assert_eq!(FEATURES_1_VERSION, "features/1");
    assert_eq!(features_1.value_count, FEATURES_1_VALUE_COUNT);
    assert_eq!(FEATURES_1_VALUE_COUNT, 32);
    assert_eq!(features_1.band_count, BAND_COUNT);
    assert_eq!(features_1.band_windows, FEATURES_1_BAND_WINDOWS);
    assert_eq!(features_1.scalar_features, FEATURES_1_SCALARS);
    assert_eq!(features_1.fft_lengths, ANALYSIS_FFT_LENGTHS);

    let mut registry = FeatureRegistry::new();
    let id = registry.define(features_1.clone()).unwrap();
    let contract = registry.get(id).unwrap();
    assert_eq!(contract.id(), id);
    assert_eq!(contract.state(), FeatureContractState::Draft);
    assert_eq!(contract.definition(), &features_1);

    registry.publish(id).unwrap();
    let contract = registry.get(id).unwrap();
    assert_eq!(contract.state(), FeatureContractState::Published);
    assert_eq!(contract.definition(), &features_1);
    assert_eq!(registry.find(FEATURES_1_VERSION).map(|c| c.id()), Some(id));
}

/// feature-contract-wrong-count: `features/1` defined with 31 values is refused and no contract
/// exists.
#[test]
fn feature_contract_wrong_count() {
    let mut registry = FeatureRegistry::new();
    let mut wrong = definition(31, &[512, 2_048, 8_192]);
    wrong.scalar_features.pop();
    assert_eq!(
        registry.define(wrong),
        Err(FeatureError::FeatureCountMismatch {
            version: "features/1".to_string(),
            value_count: 31,
        })
    );
    assert!(registry.is_empty());
    assert!(registry.find(FEATURES_1_VERSION).is_none());
}

/// The model's `wrong-count` fires for `features/1` only: any other version is `defined` with the
/// count it carries, even one that is not its bands over its windows plus its scalars (a layout
/// rule is an open point for the spec). Its extracted vector holds the layout's 32 values, which
/// the contract's own check refuses as a length mismatch.
#[test]
fn a_version_other_than_features_1_is_defined_with_the_count_it_carries() {
    let mut registry = FeatureRegistry::new();
    let mut other = definition(33, &[512, 2_048, 8_192]);
    other.version = "features/2".to_string();
    let id = registry.define(other.clone()).unwrap();
    let contract = registry.get(id).unwrap();
    assert_eq!(contract.state(), FeatureContractState::Draft);
    assert_eq!(contract.definition(), &other);
    assert_eq!(contract.definition().value_count, 33);

    registry.publish(id).unwrap();
    let samples = sine(440.0, 0.5, 48_000);
    let vector = registry
        .extract(id, &analysed(&samples), &samples, NOTE)
        .unwrap();
    assert_eq!(vector.values.len(), 32);
    assert_eq!(
        registry
            .get(id)
            .unwrap()
            .check("features/2", &vector.values),
        Err(FeatureError::FeatureLengthMismatch { value_count: 32 })
    );
}

/// feature-contract-frozen: a draft is amended, then published; amending it, publishing it again
/// and redefining its version are refused, and nothing changes.
#[test]
fn feature_contract_frozen() {
    let mut registry = FeatureRegistry::new();
    let id = registry.define(definition(32, &[512, 2_048])).unwrap();
    registry.amend(id, vec![512, 2_048, 8_192]).unwrap();
    assert_eq!(
        registry.get(id).unwrap().definition().fft_lengths,
        [512, 2_048, 8_192]
    );
    registry.publish(id).unwrap();

    assert_eq!(
        registry.amend(id, vec![1_024]),
        Err(FeatureError::FeatureContractFrozen {
            feature_contract_id: id
        })
    );
    assert_eq!(
        registry.publish(id),
        Err(FeatureError::FeatureContractFrozen {
            feature_contract_id: id
        })
    );
    assert_eq!(
        registry.define(definition(32, &[1_024])),
        Err(FeatureError::FeatureVersionTaken {
            version: "features/1".to_string()
        })
    );
    let contract = registry.get(id).unwrap();
    assert_eq!(contract.state(), FeatureContractState::Published);
    assert_eq!(contract.definition(), &definition(32, &[512, 2_048, 8_192]));
    assert_eq!(registry.len(), 1);
}

/// A draft's version is taken as well: it is defined once, then amended, never redefined.
#[test]
fn a_draft_version_is_not_defined_twice() {
    let mut registry = FeatureRegistry::new();
    registry.define(definition(32, &[512])).unwrap();
    assert_eq!(
        registry.define(definition(32, &[2_048])),
        Err(FeatureError::FeatureVersionTaken {
            version: "features/1".to_string()
        })
    );
}

/// Every command on a contract the registry does not hold is refused as not found.
#[test]
fn an_unknown_contract_is_not_found() {
    let (mut registry, id) = published();
    let unknown = FeatureContractId(id.0 + 1);
    let not_found = Err(FeatureError::FeatureContractNotFound {
        feature_contract_id: unknown,
    });
    assert_eq!(registry.amend(unknown, vec![512]), not_found);
    assert_eq!(registry.publish(unknown), not_found);
    assert_eq!(
        registry.check_vector(unknown, "features/1", &[0.0; 32]),
        not_found
    );
    let samples = sine(440.0, 0.5, 48_000);
    assert_eq!(
        registry
            .extract(unknown, &analysed(&samples), &samples, NOTE)
            .map(|_| ()),
        not_found
    );
    assert_eq!(registry.get(unknown).map(|_| ()), not_found);
}

/// feature-contract-vector-checks: a 32-value `features/1` vector is admitted; one of version
/// `features/2`, one of 31 values and one with a NaN are refused.
#[test]
fn feature_contract_vector_checks() {
    let (registry, id) = published();
    let values = vec![0.25; 32];
    assert_eq!(registry.check_vector(id, "features/1", &values), Ok(()));
    assert_eq!(
        registry.check_vector(id, "features/2", &values),
        Err(FeatureError::FeatureVersionMismatch {
            version: "features/2".to_string()
        })
    );
    assert_eq!(
        registry.check_vector(id, "features/1", &values[..31]),
        Err(FeatureError::FeatureLengthMismatch { value_count: 31 })
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut poisoned = values.clone();
        poisoned[7] = bad;
        assert_eq!(
            registry.check_vector(id, "features/1", &poisoned),
            Err(FeatureError::NonFiniteFeature {
                feature_contract_id: id,
                index: 7
            })
        );
    }
    // the outcomes in the model's order: version before length before finiteness
    assert_eq!(
        registry.check_vector(id, "features/2", &[f64::NAN; 31]),
        Err(FeatureError::FeatureVersionMismatch {
            version: "features/2".to_string()
        })
    );
    assert_eq!(
        registry.check_vector(id, "features/1", &[f64::NAN; 31]),
        Err(FeatureError::FeatureLengthMismatch { value_count: 31 })
    );
}

/// feature-contract-extract: extraction under a draft is refused; under the published contract it
/// yields 32 finite values; a feature that cannot be computed and a non-finite feature are
/// refused.
#[test]
fn feature_contract_extract() {
    let samples = sine(440.0, 0.5, 48_000);
    let analysis = analysed(&samples);

    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    assert_eq!(
        registry.extract(id, &analysis, &samples, NOTE),
        Err(FeatureError::FeatureContractNotPublished {
            feature_contract_id: id
        })
    );

    registry.publish(id).unwrap();
    let vector = registry.extract(id, &analysis, &samples, NOTE).unwrap();
    assert_eq!(vector.feature_contract_id, id);
    assert_eq!(vector.version, "features/1");
    assert_eq!(vector.values.len(), 32);
    assert!(vector.values.iter().all(|v| v.is_finite()), "{vector:?}");
    assert_eq!(
        registry.check_vector(id, &vector.version, &vector.values),
        Ok(())
    );

    // not computable: an analysis whose f0 estimate is below the confidence threshold
    let mut ambiguous = analysis.clone();
    ambiguous.f0.confidence = 0.2;
    assert_eq!(
        registry.extract(id, &ambiguous, &samples, NOTE),
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Scalar(ScalarFeature::NormalizedMidiPitch),
            index: 31,
            reason: NotComputableReason::PitchAmbiguous { confidence: 0.2 },
        })
    );

    // non-finite: an analysis whose spectral centroid overflowed in one frame of the note
    let mut overflowed = analysis.clone();
    let stft = overflowed
        .stfts
        .iter_mut()
        .find(|s| s.fft_length == 2_048)
        .unwrap();
    stft.frames[3].centroid_hz = f64::INFINITY;
    assert_eq!(
        registry.extract(id, &overflowed, &samples, NOTE),
        Err(FeatureError::NonFiniteFeature {
            feature_contract_id: id,
            index: 26
        })
    );
}

/// The slots of `features/1`: 24 band energies window-major (attack bands 0-7, body 0-7,
/// release 0-7), then the eight scalars in DESIGN.md's order.
#[test]
fn the_slots_are_window_major_then_the_scalars() {
    let slots = FeatureContractDefinition::features_1().slots();
    assert_eq!(slots.len(), 32);
    for (w, window) in [BandWindow::Attack, BandWindow::Body, BandWindow::Release]
        .into_iter()
        .enumerate()
    {
        for band in 0..8 {
            assert_eq!(slots[w * 8 + band], FeatureSlot::Band { window, band });
        }
    }
    for (s, scalar) in FEATURES_1_SCALARS.into_iter().enumerate() {
        assert_eq!(slots[24 + s], FeatureSlot::Scalar(scalar));
    }
    assert_eq!(
        FEATURES_1_SCALARS,
        [
            ScalarFeature::Rms,
            ScalarFeature::Peak,
            ScalarFeature::SpectralCentroid,
            ScalarFeature::Rolloff,
            ScalarFeature::SpectralFlatness,
            ScalarFeature::ZeroCrossingRate,
            ScalarFeature::AttackEnvelopeSlope,
            ScalarFeature::NormalizedMidiPitch,
        ]
    );
}

/// The windows of a note: attack from the note's start for [`ATTACK_WINDOW_FRAMES`], body to
/// the note's end, release from [`RELEASE_OFFSET_FRAMES`] past the end for
/// [`RELEASE_WINDOW_FRAMES`], each cut at the clip's end.
#[test]
fn the_windows_follow_the_note() {
    let note = NoteSpan {
        start_sample: 1_000,
        end_sample: 20_000,
    };
    assert_eq!(
        note.window(BandWindow::Attack, 48_000),
        1_000..1_000 + ATTACK_WINDOW_FRAMES
    );
    assert_eq!(
        note.window(BandWindow::Body, 48_000),
        1_000 + ATTACK_WINDOW_FRAMES..20_000
    );
    let release = 20_000 + RELEASE_OFFSET_FRAMES;
    assert_eq!(
        note.window(BandWindow::Release, 48_000),
        release..release + RELEASE_WINDOW_FRAMES
    );
    assert_eq!(note.window(BandWindow::Release, 25_000), release..25_000);
    let short = NoteSpan {
        start_sample: 0,
        end_sample: 100,
    };
    assert_eq!(short.window(BandWindow::Attack, 48_000), 0..100);
    assert!(short.window(BandWindow::Body, 48_000).is_empty());
}

/// 32 values, each finite, in contract order: the scalars of a steady 440 Hz sine at amplitude
/// 0.5 are where the contract puts them.
#[test]
fn extracted_values_are_finite_and_in_contract_order() {
    let samples = sine(440.0, 0.5, 48_000);
    let (registry, id) = published();
    let vector = registry
        .extract(id, &analysed(&samples), &samples, NOTE)
        .unwrap();
    assert_eq!(vector.values.len(), 32);
    assert!(vector.values.iter().all(|v| v.is_finite()), "{vector:?}");
    let value = |scalar| vector.values[index_of(FeatureSlot::Scalar(scalar))];

    let rms_dbfs = 20.0 * (0.5 / 2f64.sqrt()).log10();
    assert!(
        (value(ScalarFeature::Rms) - rms_dbfs).abs() < 0.01,
        "{vector:?}"
    );
    assert!((value(ScalarFeature::Peak) - 20.0 * 0.5f64.log10()).abs() < 0.01);
    assert!((value(ScalarFeature::SpectralCentroid) - 440.0).abs() < 100.0);
    assert!((value(ScalarFeature::Rolloff) - 440.0).abs() < 100.0);
    assert!(value(ScalarFeature::SpectralFlatness) < 0.05);
    let zcr = 2.0 * 440.0 / RATE;
    assert!((value(ScalarFeature::ZeroCrossingRate) - zcr).abs() < 1e-3);
    assert!(value(ScalarFeature::AttackEnvelopeSlope) > 0.0);
    assert!((value(ScalarFeature::NormalizedMidiPitch) - 69.0 / 127.0).abs() < 1e-3);

    // every window holds the sine in the 375-750 Hz band, the loudest of its eight
    for w in 0..3 {
        let bands = &vector.values[w * 8..w * 8 + 8];
        let loudest = (0..8)
            .max_by(|a, b| bands[*a].total_cmp(&bands[*b]))
            .unwrap();
        assert_eq!(loudest, 2, "window {w}: {bands:?}");
    }
}

/// Window-major order holds on the vector itself: a 480 Hz note (a period of exactly 100
/// samples) whose attack carries its second harmonic, 960 Hz (band 3, 750-1500 Hz), that its
/// release lacks, and whose release carries its fourth, 1920 Hz (band 4, 1500-3000 Hz), that its
/// attack lacks, puts each where window-major order says: attack band 3 at index 3, release band
/// 4 at index 20. The body window's first and last frames straddle the attack and the release,
/// so the body is held to 15 dB.
#[test]
fn band_energies_are_window_major() {
    let mut samples = vec![0.0f32; 48_000];
    let attack = NOTE.window(BandWindow::Attack, samples.len());
    let release = NOTE.window(BandWindow::Release, samples.len());
    add_sine(&mut samples, 0..NOTE.end_sample, 480.0, 0.5);
    add_sine(&mut samples, attack, 960.0, 0.3);
    add_sine(&mut samples, release.clone(), 480.0, 0.2);
    add_sine(&mut samples, release, 1_920.0, 0.2);
    let (registry, id) = published();
    let values = registry
        .extract(id, &analysed(&samples), &samples, NOTE)
        .unwrap()
        .values;
    let at = |window, band| values[index_of(FeatureSlot::Band { window, band })];
    assert_eq!(
        index_of(FeatureSlot::Band {
            window: BandWindow::Attack,
            band: 3
        }),
        3
    );
    assert_eq!(
        index_of(FeatureSlot::Band {
            window: BandWindow::Release,
            band: 4
        }),
        20
    );
    assert!(values[3] > values[16 + 3] + 20.0, "{values:?}");
    assert!(values[3] > values[8 + 3] + 15.0, "{values:?}");
    assert!(values[20] > values[4] + 20.0, "{values:?}");
    assert!(values[20] > values[8 + 4] + 15.0, "{values:?}");
    assert_eq!(values[3], at(BandWindow::Attack, 3));
    assert_eq!(values[20], at(BandWindow::Release, 4));
}

/// The same clip, analysis and windows give the same vector, bit for bit, and so does analysing
/// the clip again.
#[test]
fn extraction_is_repeatable() {
    let mut samples = sine(330.0, 0.4, 48_000);
    add_sine(&mut samples, 0..48_000, 660.0, 0.1);
    let (registry, id) = published();
    let analysis = analysed(&samples);
    let first = registry.extract(id, &analysis, &samples, NOTE).unwrap();
    let second = registry.extract(id, &analysis, &samples, NOTE).unwrap();
    let third = registry
        .extract(id, &analysed(&samples), &samples, NOTE)
        .unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&first.values), bits(&second.values));
    assert_eq!(bits(&first.values), bits(&third.values));
}

/// A silent window is not computable: its band energies are refused, not reported at the floor.
#[test]
fn a_silent_release_window_is_not_computable() {
    let mut samples = vec![0.0f32; 48_000];
    add_sine(&mut samples, 0..NOTE.end_sample, 440.0, 0.5);
    let (registry, id) = published();
    let release = NOTE.window(BandWindow::Release, samples.len());
    assert_eq!(
        registry.extract(id, &analysed(&samples), &samples, NOTE),
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Band {
                window: BandWindow::Release,
                band: 0
            },
            index: 16,
            reason: NotComputableReason::SilentWindow {
                start_sample: release.start,
                end_sample: release.end,
                rms_dbfs: LOG_FLOOR_DB,
            },
        })
    );
}

/// An unpitched clip is not computable: noise has no f0, and analysing and extracting it ends in
/// the pitch feature's refusal, carrying the analysis' own diagnostic.
#[test]
fn an_unpitched_clip_is_not_computable() {
    let samples = noise(48_000);
    let (registry, id) = published();
    let contract = registry.get(id).unwrap();
    match analyze_and_extract(contract, &samples, SAMPLE_RATE_HZ, NOTE) {
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Scalar(ScalarFeature::NormalizedMidiPitch),
            index: 31,
            reason: NotComputableReason::Analysis(AnalysisError::PitchAmbiguous { .. }),
        }) => {}
        other => panic!("{other:?}"),
    }
}

/// [`analyze_and_extract`] of a pitched clip equals [`extract_features`] of its analysis, and it
/// refuses a draft before analysing anything.
#[test]
fn analyze_and_extract_is_analysis_then_extraction() {
    let samples = sine(440.0, 0.5, 48_000);
    let (registry, id) = published();
    let contract = registry.get(id).unwrap();
    assert_eq!(
        analyze_and_extract(contract, &samples, SAMPLE_RATE_HZ, NOTE),
        extract_features(contract, &analysed(&samples), &samples, NOTE)
    );
    let mut drafts = FeatureRegistry::new();
    let draft = drafts
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    assert_eq!(
        analyze_and_extract(drafts.get(draft).unwrap(), &[], SAMPLE_RATE_HZ, NOTE),
        Err(FeatureError::FeatureContractNotPublished {
            feature_contract_id: draft
        })
    );
}

/// The inputs extraction cannot use are refused with what is wrong: a note outside the clip, a
/// clip that is not the analysis' clip, an analysis without an STFT the contract names, and a
/// note too short to hold a body window.
#[test]
fn unusable_inputs_are_not_computable() {
    let samples = sine(440.0, 0.5, 48_000);
    let analysis = analysed(&samples);
    let (registry, id) = published();
    let reason = |result: Result<_, FeatureError>| match result {
        Err(FeatureError::FeatureNotComputable { reason, .. }) => reason,
        other => panic!("{other:?}"),
    };
    let outside = NoteSpan {
        start_sample: 40_000,
        end_sample: 50_000,
    };
    assert_eq!(
        reason(registry.extract(id, &analysis, &samples, outside)),
        NotComputableReason::NoteOutsideClip {
            start_sample: 40_000,
            end_sample: 50_000,
            frame_count: 48_000
        }
    );
    let backwards = NoteSpan {
        start_sample: 10_000,
        end_sample: 10_000,
    };
    assert!(matches!(
        reason(registry.extract(id, &analysis, &samples, backwards)),
        NotComputableReason::NoteOutsideClip { .. }
    ));
    assert_eq!(
        reason(registry.extract(id, &analysis, &samples[..47_000], NOTE)),
        NotComputableReason::ClipMismatch {
            frame_count: 47_000,
            analysis_frame_count: 48_000
        }
    );
    let mut without = analysis.clone();
    without.stfts.retain(|s| s.fft_length != 8_192);
    assert_eq!(
        reason(registry.extract(id, &without, &samples, NOTE)),
        NotComputableReason::StftMissing { fft_length: 8_192 }
    );
    let short = NoteSpan {
        start_sample: 0,
        end_sample: ATTACK_WINDOW_FRAMES + 100,
    };
    assert_eq!(
        registry.extract(id, &analysis, &samples, short),
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Band {
                window: BandWindow::Body,
                band: 0
            },
            index: 8,
            reason: NotComputableReason::WindowTooShort {
                start_sample: ATTACK_WINDOW_FRAMES,
                end_sample: ATTACK_WINDOW_FRAMES + 100
            },
        })
    );
}

/// An analysis whose f0 is absent (not a positive finite frequency) or outside the searched range
/// gives no pitch feature.
#[test]
fn an_absent_pitch_is_not_computable() {
    let samples = sine(440.0, 0.5, 48_000);
    let analysis = analysed(&samples);
    let (registry, id) = published();
    for f0_hz in [0.0, -440.0, f64::NAN, 20_000.0] {
        let mut absent = analysis.clone();
        absent.f0.f0_hz = f0_hz;
        match registry.extract(id, &absent, &samples, NOTE) {
            Err(FeatureError::FeatureNotComputable {
                feature: FeatureSlot::Scalar(ScalarFeature::NormalizedMidiPitch),
                index: 31,
                reason: NotComputableReason::PitchAbsent { .. },
            }) => {}
            other => panic!("{f0_hz}: {other:?}"),
        }
    }
}

/// The zero-crossing rate is sign changes per sample pair: a sine of `f` Hz crosses `2 f` times a
/// second.
#[test]
fn zero_crossing_rate_counts_sign_changes() {
    assert_eq!(zero_crossing_rate(&[]), Ok(0.0));
    assert_eq!(zero_crossing_rate(&[0.5]), Ok(0.0));
    assert_eq!(zero_crossing_rate(&[1.0, -1.0, 1.0, -1.0, 1.0]), Ok(1.0));
    assert_eq!(zero_crossing_rate(&[1.0, 1.0, -1.0, -1.0, 1.0]), Ok(0.5));
    let rate = zero_crossing_rate(&sine(1_000.0, 0.5, 48_000)).unwrap();
    assert!((rate - 2_000.0 / RATE).abs() < 1e-4, "{rate}");
    assert_eq!(
        zero_crossing_rate(&[0.0, f32::NAN]),
        Err(AnalysisError::NonFiniteSamples { index: 1 })
    );
}

/// The attack-envelope slope: a sudden onset rises faster than a slow ramp to the same level.
#[test]
fn attack_envelope_slope_is_steeper_for_a_sudden_onset() {
    let sudden = sine(440.0, 0.5, ATTACK_WINDOW_FRAMES);
    let ramp: Vec<f32> = sudden
        .iter()
        .enumerate()
        .map(|(i, s)| s * (i as f32 / ATTACK_WINDOW_FRAMES as f32))
        .collect();
    let sudden = attack_envelope_slope(&sudden, SAMPLE_RATE_HZ).unwrap();
    let ramp = attack_envelope_slope(&ramp, SAMPLE_RATE_HZ).unwrap();
    assert!(sudden.is_finite() && ramp.is_finite());
    assert!(ramp > 0.0);
    assert!(sudden > 10.0 * ramp, "{sudden} {ramp}");
    assert_eq!(attack_envelope_slope(&[], SAMPLE_RATE_HZ), Ok(0.0));
    assert_eq!(
        attack_envelope_slope(&[f32::INFINITY], SAMPLE_RATE_HZ),
        Err(AnalysisError::NonFiniteSamples { index: 0 })
    );
}
