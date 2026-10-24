//! Adversary cases, second pass, for story:feature-contract (wave a5, unit 3), against the
//! correction round: `define` in the model's outcome order without a layout rule (D9), the STFT
//! of a contract's FFT length read at any hop (D10), and the count-mismatch message.
//!
//! Expected values come from `spec/domains/analysis.yaml`, the module documentation of
//! `features.rs` ("each STFT's frames are placed by its own hop") and DESIGN.md § 6.3, not from
//! the code under test.

use std::f64::consts::PI;

use patchback_analysis::{
    Analysis, AnalysisConfig, BandWindow, FeatureContractDefinition, FeatureContractId,
    FeatureError, FeatureRegistry, FrameDescriptors, NoteSpan, PitchEstimate, SAMPLE_RATE_HZ,
    StftAnalysis, StftConfig, analyze,
};

const FRAMES: usize = 48_000;

/// Gate 1024..23552, attack 1024..5824, body 5824..23552, release 23552..33152.
const NOTE: NoteSpan = NoteSpan {
    start_sample: 1_024,
    end_sample: 23_552,
};

fn energy(i: usize, band: usize, offset: f64) -> f64 {
    offset - 2.0 * i as f64 - 3.0 * band as f64
}

/// Frames whose values name their index `i`: band energies `offset - 2 i - 3 band` dB, centroid
/// `100 (i + 1)` Hz.
fn marker_frames(count: usize, offset: f64) -> Vec<FrameDescriptors> {
    (0..count)
        .map(|i| {
            let mut band_energies_db = [0.0; 8];
            for (band, e) in band_energies_db.iter_mut().enumerate() {
                *e = energy(i, band, offset);
            }
            FrameDescriptors {
                rms: 0.25,
                peak: 0.5,
                centroid_hz: 100.0 * (i + 1) as f64,
                rolloff_hz: 5_000.0 + 10.0 * i as f64,
                flatness: 0.001 * i as f64,
                flux: 0.0,
                band_energies_db,
            }
        })
        .collect()
}

fn stft_2048(hop_length: usize, offset: f64) -> StftAnalysis {
    StftAnalysis {
        fft_length: 2_048,
        hop_length,
        frames: marker_frames(FRAMES.div_ceil(hop_length.max(1)), offset),
    }
}

/// An analysis of [`FRAMES`] samples holding the given STFTs of 2048 between empty 512 and
/// 8192 STFTs.
fn hand_built(feature_stfts: Vec<StftAnalysis>) -> Analysis {
    let mut stfts = vec![StftAnalysis {
        fft_length: 512,
        hop_length: 256,
        frames: Vec::new(),
    }];
    stfts.extend(feature_stfts);
    stfts.push(StftAnalysis {
        fft_length: 8_192,
        hop_length: 4_096,
        frames: Vec::new(),
    });
    Analysis {
        sample_rate_hz: SAMPLE_RATE_HZ,
        frame_count: FRAMES,
        rms_dbfs: -9.0,
        peak: 0.5,
        f0: PitchEstimate {
            f0_hz: 440.0,
            confidence: 0.9,
        },
        stfts,
    }
}

fn loud(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (0.5 * (2.0 * PI * 480.0 * i as f64 / SAMPLE_RATE_HZ as f64).sin()) as f32)
        .collect()
}

fn published(definition: FeatureContractDefinition) -> (FeatureRegistry, FeatureContractId) {
    let mut registry = FeatureRegistry::new();
    let id = registry.define(definition).unwrap();
    registry.publish(id).unwrap();
    (registry, id)
}

fn power_mean(frames: std::ops::RangeInclusive<usize>, band: usize, offset: f64) -> f64 {
    let count = frames.clone().count() as f64;
    let sum: f64 = frames
        .map(|i| 10f64.powf(energy(i, band, offset) / 10.0))
        .sum();
    10.0 * (sum / count).log10()
}

/// D10 and the module documentation: "extraction reads an STFT at any hop, by its own hop". At
/// hop 512 frame `i` of the 2048 STFT is centred at `512 i + 1024`, so attack 1024..5824 holds
/// frames 0..=9, body 5824..23552 frames 10..=43, release 23552..33152 frames 44..=62 and the
/// gate frames 0..=43 (centroid mean `100 (1 + 44) / 2 = 2250`). Catches frames placed at the
/// default hop `fft_length / 2` whatever the STFT's own hop.
#[test]
fn frames_of_an_stft_at_another_hop_are_placed_by_that_hop() {
    let samples = loud(FRAMES);
    let (registry, id) = published(FeatureContractDefinition::features_1());
    let values = registry
        .extract(id, &hand_built(vec![stft_2048(512, 0.0)]), &samples, NOTE)
        .unwrap()
        .values;
    for band in 0..8 {
        for (w, frames) in [0..=9, 10..=43, 44..=62].into_iter().enumerate() {
            let expected = power_mean(frames, band, 0.0);
            assert!(
                (values[w * 8 + band] - expected).abs() < 1e-9,
                "window {w} band {band}: {} against {expected}",
                values[w * 8 + band]
            );
        }
    }
    assert!(
        (values[26] - 2_250.0).abs() < 1e-9,
        "centroid {}",
        values[26]
    );
}

/// D10 keys the STFT lookup on the FFT length only, and `analyze` takes a configuration with
/// two STFTs of one length. Pinned here: of two STFTs of 2048, the first in the analysis is
/// read, so the vector does not depend on anything but the analysis' order. Catches a lookup
/// that takes the last match or prefers one hop.
#[test]
fn of_two_stfts_of_the_feature_length_the_first_in_the_analysis_is_read() {
    let samples = loud(FRAMES);
    let (registry, id) = published(FeatureContractDefinition::features_1());
    let extract = |stfts: Vec<StftAnalysis>| {
        registry
            .extract(id, &hand_built(stfts), &samples, NOTE)
            .unwrap()
            .values
    };
    let a = || stft_2048(1_024, 0.0);
    let b = || stft_2048(512, -10.0);
    assert_eq!(extract(vec![a(), b()]), extract(vec![a()]));
    assert_eq!(extract(vec![b(), a()]), extract(vec![b()]));
    assert_ne!(extract(vec![a()]), extract(vec![b()]));

    let analysed = |stfts: &[(usize, usize)]| {
        let config = AnalysisConfig {
            stfts: stfts
                .iter()
                .map(|&(fft_length, hop_length)| StftConfig {
                    fft_length,
                    hop_length,
                })
                .collect(),
        };
        analyze(&samples, SAMPLE_RATE_HZ, &config).unwrap()
    };
    let one = analysed(&[(512, 256), (2_048, 1_024), (2_048, 512), (8_192, 4_096)]);
    let two = analysed(&[(512, 256), (2_048, 1_024), (8_192, 4_096)]);
    assert_eq!(
        registry.extract(id, &one, &samples, NOTE),
        registry.extract(id, &two, &samples, NOTE)
    );
}

/// D10 accepts any hop, and [`Analysis`] is built by hand as well as by `analyze`. A hop no
/// frame centre after the first fits in `usize` is refused like any window without a frame
/// (body 5824..23552 holds none), not a panic on `i * hop + fft_length / 2`.
#[test]
fn an_stft_whose_hop_overflows_the_frame_centre_is_refused_not_a_panic() {
    let samples = loud(FRAMES);
    let (registry, id) = published(FeatureContractDefinition::features_1());
    let mut stft = stft_2048(1_024, 0.0);
    stft.hop_length = usize::MAX;
    let result = registry.extract(id, &hand_built(vec![stft]), &samples, NOTE);
    assert!(
        matches!(
            result,
            Err(FeatureError::FeatureNotComputable { index: 8, .. })
        ),
        "{result:?}"
    );
}

/// `FeatureCountMismatch` after D9 is `features/1`'s only (analysis.yaml: "`features/1` has
/// exactly 32 values"); the message names the version and the count it was refused for and the
/// 32 it was checked against.
#[test]
fn the_count_mismatch_message_names_the_count_and_features_1_s_32() {
    let mut registry = FeatureRegistry::new();
    let mut wrong = FeatureContractDefinition::features_1();
    wrong.value_count = 31;
    let error = registry.define(wrong).unwrap_err();
    assert_eq!(
        error.to_string(),
        "feature contract features/1 with 31 values: features/1 has exactly 32"
    );
}

/// The model's `wrong-count` reads the count only, so after D9 a `features/1` with 32 values
/// is `defined` whatever its layout. Recorded for the judgement finding on DESIGN.md § 6.3: a
/// `features/1` laid out as 40 values extracts 40, which its own check refuses; one with its
/// windows reversed extracts 32 values whose first band is the release window's, which its
/// check admits.
#[test]
fn features_1_with_32_values_is_defined_whatever_its_layout() {
    let samples = loud(FRAMES);
    let analysis = analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()).unwrap();
    let note = NoteSpan {
        start_sample: 0,
        end_sample: 24_000,
    };

    let mut forty = FeatureContractDefinition::features_1();
    forty.scalar_features.extend(forty.scalar_features.clone());
    assert_eq!(forty.value_count, 32);
    assert_eq!(forty.slots().len(), 40);
    let (registry, id) = published(forty);
    let vector = registry.extract(id, &analysis, &samples, note).unwrap();
    assert_eq!(vector.values.len(), 40);
    assert_eq!(
        registry.check_vector(id, "features/1", &vector.values),
        Err(FeatureError::FeatureLengthMismatch { value_count: 40 })
    );

    let (standard, standard_id) = published(FeatureContractDefinition::features_1());
    let reference = standard
        .extract(standard_id, &analysis, &samples, note)
        .unwrap()
        .values;
    let mut reversed = FeatureContractDefinition::features_1();
    reversed.band_windows = vec![BandWindow::Release, BandWindow::Body, BandWindow::Attack];
    let (registry, id) = published(reversed);
    let vector = registry.extract(id, &analysis, &samples, note).unwrap();
    assert_eq!(
        registry.check_vector(id, "features/1", &vector.values),
        Ok(())
    );
    assert_eq!(vector.values[0], reference[16]);
    assert_ne!(vector.values[0], reference[0]);
}

/// D10's consequence, recorded for the judgement finding on DESIGN.md § 6.3 ("FFT size … can be
/// changed only by versioning the feature contract", "consistent overlap/window conventions"):
/// one published contract and one note read at hop 1024 and at hop 512 give two vectors, both
/// extracted. The largest difference is printed.
#[test]
fn one_published_contract_reads_two_hops_into_two_vectors() {
    let samples: Vec<f32> = (0..FRAMES)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE_HZ as f64;
            (0.5 * (-6.0 * t).exp() * (2.0 * PI * 440.0 * t).sin()) as f32
        })
        .collect();
    let at_hop = |hop: usize| {
        let config = AnalysisConfig {
            stfts: [512, 2_048, 8_192]
                .into_iter()
                .map(|fft_length| StftConfig {
                    fft_length,
                    hop_length: if fft_length == 2_048 {
                        hop
                    } else {
                        fft_length / 2
                    },
                })
                .collect(),
        };
        analyze(&samples, SAMPLE_RATE_HZ, &config).unwrap()
    };
    let (registry, id) = published(FeatureContractDefinition::features_1());
    let note = NoteSpan {
        start_sample: 0,
        end_sample: 24_000,
    };
    let one = registry
        .extract(id, &at_hop(1_024), &samples, note)
        .unwrap()
        .values;
    let two = registry
        .extract(id, &at_hop(512), &samples, note)
        .unwrap()
        .values;
    let (index, difference) = one
        .iter()
        .zip(&two)
        .map(|(a, b)| (a - b).abs())
        .enumerate()
        .fold(
            (0, 0.0),
            |best, (i, d)| if d > best.1 { (i, d) } else { best },
        );
    eprintln!("hop 1024 against hop 512: largest |difference| {difference} at index {index}");
    assert!(difference > 0.0);
}
