//! Adversary cases for story:feature-contract (wave a5, unit 3). Each case drives the feature
//! contract from `spec/domains/analysis.yaml`, the five `spec/scenarios/feature-contract-*.yaml`
//! or the module documentation of `features.rs`, with expected values written from those
//! documents rather than from the code under test.
//!
//! Most extraction cases read a hand-built [`Analysis`] whose `FEATURE_FFT_LENGTH` frames carry
//! marker values, so which frames a window takes, how they are averaged and which slot a value
//! lands in are pinned to literals.

use std::f64::consts::PI;

use patchback_analysis::{
    Analysis, AnalysisConfig, BandWindow, FeatureContractDefinition, FeatureContractId,
    FeatureError, FeatureRegistry, FeatureSlot, FrameDescriptors, LOG_FLOOR_DB,
    NotComputableReason, NoteSpan, PitchEstimate, SAMPLE_RATE_HZ, SILENCE_THRESHOLD_DBFS,
    StftAnalysis, StftConfig, analyze,
};

const FRAMES: usize = 48_000;
const HOP: usize = 1_024;

/// A note whose attack ends between two frame centres and whose end lies exactly on the centre of
/// frame 22 (1024 * 22 + 1024 = 23552). Frame `i` of the 2048 STFT is centred at `1024 (i + 1)`.
/// Attack 1024..5824 takes frames 0..=4, body 5824..23552 frames 5..=21, release
/// 23552..33152 frames 22..=31 (the frame on the boundary belongs to the window it starts), gate
/// 1024..23552 frames 0..=21.
const NOTE: NoteSpan = NoteSpan {
    start_sample: 1_024,
    end_sample: 23_552,
};

fn energy(i: usize, band: usize) -> f64 {
    -2.0 * i as f64 - 3.0 * band as f64
}

fn marker_frames(count: usize) -> Vec<FrameDescriptors> {
    (0..count)
        .map(|i| {
            let mut band_energies_db = [0.0; 8];
            for (band, e) in band_energies_db.iter_mut().enumerate() {
                *e = energy(i, band);
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

fn hand_built(frame_count: usize) -> Analysis {
    Analysis {
        sample_rate_hz: SAMPLE_RATE_HZ,
        frame_count,
        rms_dbfs: -9.0,
        peak: 0.5,
        f0: PitchEstimate {
            f0_hz: 440.0,
            confidence: 0.9,
        },
        stfts: vec![
            StftAnalysis {
                fft_length: 512,
                hop_length: 256,
                frames: Vec::new(),
            },
            StftAnalysis {
                fft_length: 2_048,
                hop_length: HOP,
                frames: marker_frames(frame_count.div_ceil(HOP)),
            },
            StftAnalysis {
                fft_length: 8_192,
                hop_length: 4_096,
                frames: Vec::new(),
            },
        ],
    }
}

fn loud(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (0.5 * (2.0 * PI * 480.0 * i as f64 / SAMPLE_RATE_HZ as f64).sin()) as f32)
        .collect()
}

fn published() -> (FeatureRegistry, FeatureContractId) {
    let mut registry = FeatureRegistry::new();
    let id = registry
        .define(FeatureContractDefinition::features_1())
        .unwrap();
    registry.publish(id).unwrap();
    (registry, id)
}

/// The module documentation's band energy: `10 log10(mean 10^(E / 10))` over the frames whose
/// centre lies in the window.
fn power_mean(frames: std::ops::RangeInclusive<usize>, band: usize) -> f64 {
    let count = frames.clone().count() as f64;
    let sum: f64 = frames.map(|i| 10f64.powf(energy(i, band) / 10.0)).sum();
    10.0 * (sum / count).log10()
}

/// Window membership, the power mean, the slot of every band and every spectral scalar, pinned
/// to the literal frame sets of [`NOTE`]. Catches: a centroid or rolloff read from the wrong
/// field, an arithmetic mean of dB, a window that takes the frame centred on its end, a window
/// or band swap.
#[test]
fn values_come_from_the_frames_the_documented_windows_hold() {
    let samples = loud(FRAMES);
    let (registry, id) = published();
    let values = registry
        .extract(id, &hand_built(FRAMES), &samples, NOTE)
        .unwrap()
        .values;
    assert_eq!(values.len(), 32);
    for band in 0..8 {
        for (w, frames) in [0..=4, 5..=21, 22..=31].into_iter().enumerate() {
            let expected = power_mean(frames, band);
            assert!(
                (values[w * 8 + band] - expected).abs() < 1e-9,
                "window {w} band {band}: {} against {expected}",
                values[w * 8 + band]
            );
        }
    }
    // attack band 0: frames 0..=4 at 0, -2, -4, -6, -8 dB; the power mean, not -4 dB
    assert!((values[0] - (-3.118_3)).abs() < 1e-3, "{}", values[0]);
    // gate frames 0..=21: centroid 100 (i + 1), rolloff 5000 + 10 i, flatness 0.001 i
    assert!(
        (values[26] - 1_150.0).abs() < 1e-9,
        "centroid {}",
        values[26]
    );
    assert!(
        (values[27] - 5_105.0).abs() < 1e-9,
        "rolloff {}",
        values[27]
    );
    assert!(
        (values[28] - 0.010_5).abs() < 1e-12,
        "flatness {}",
        values[28]
    );
    assert_eq!(values[31], 69.0 / 127.0);
    // RMS and peak of a 0.5 sine in dBFS, zero-crossing rate of 480 Hz
    assert!((values[24] - 20.0 * (0.5 / 2f64.sqrt()).log10()).abs() < 0.05);
    assert!((values[25] - 20.0 * 0.5f64.log10()).abs() < 0.05);
    assert!((values[29] - 2.0 * 480.0 / SAMPLE_RATE_HZ as f64).abs() < 1e-3);
    assert!(values[30] > 0.0);
}

/// A release window just above [`SILENCE_THRESHOLD_DBFS`] (-59.9 dBFS) is computed; one just
/// below (-60.1 dBFS) is refused as silent at index 16. Catches a threshold other than -60 dB.
#[test]
fn the_release_window_is_silent_exactly_below_the_threshold() {
    let (registry, id) = published();
    let analysis = hand_built(FRAMES);
    let release = NOTE.window(BandWindow::Release, FRAMES);
    assert_eq!(release, 23_552..33_152);
    for (dbfs, admitted) in [
        (SILENCE_THRESHOLD_DBFS + 0.1, true),
        (SILENCE_THRESHOLD_DBFS - 0.1, false),
    ] {
        let level = 10f64.powf(dbfs / 20.0) as f32;
        let mut samples = loud(FRAMES);
        for (k, s) in samples[release.clone()].iter_mut().enumerate() {
            *s = if k % 2 == 0 { level } else { -level };
        }
        for s in &mut samples[release.end..] {
            *s = 0.0;
        }
        let result = registry.extract(id, &analysis, &samples, NOTE);
        if admitted {
            assert_eq!(result.map(|v| v.values.len()), Ok(32), "{dbfs} dBFS");
        } else {
            match result {
                Err(FeatureError::FeatureNotComputable {
                    feature:
                        FeatureSlot::Band {
                            window: BandWindow::Release,
                            band: 0,
                        },
                    index: 16,
                    reason:
                        NotComputableReason::SilentWindow {
                            start_sample: 23_552,
                            end_sample: 33_152,
                            rms_dbfs,
                        },
                }) => assert!((rms_dbfs - dbfs).abs() < 0.01, "{rms_dbfs}"),
                other => panic!("{dbfs} dBFS: {other:?}"),
            }
        }
    }
}

/// A silent attack window is refused at index 0, a silent body window at index 8, each naming
/// its window.
#[test]
fn a_silent_attack_or_body_window_is_refused_at_its_first_band() {
    let (registry, id) = published();
    let analysis = hand_built(FRAMES);
    for (window, index, range) in [
        (BandWindow::Attack, 0, 1_024..5_824),
        (BandWindow::Body, 8, 5_824..23_552),
    ] {
        assert_eq!(NOTE.window(window, FRAMES), range);
        let mut samples = loud(FRAMES);
        for s in &mut samples[range.clone()] {
            *s = 0.0;
        }
        assert_eq!(
            registry.extract(id, &analysis, &samples, NOTE),
            Err(FeatureError::FeatureNotComputable {
                feature: FeatureSlot::Band { window, band: 0 },
                index,
                reason: NotComputableReason::SilentWindow {
                    start_sample: range.start,
                    end_sample: range.end,
                    rms_dbfs: LOG_FLOOR_DB,
                },
            })
        );
    }
}

/// A note ending on the clip's last sample has an empty release window, refused at index 16; a
/// one-sample note holds no frame centre, refused at index 0.
#[test]
fn a_note_at_the_clip_end_or_of_one_sample_holds_no_frame() {
    let (registry, id) = published();
    let analysis = hand_built(FRAMES);
    let samples = loud(FRAMES);
    let to_the_end = NoteSpan {
        start_sample: 1_024,
        end_sample: FRAMES,
    };
    assert_eq!(
        registry.extract(id, &analysis, &samples, to_the_end),
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Band {
                window: BandWindow::Release,
                band: 0
            },
            index: 16,
            reason: NotComputableReason::WindowTooShort {
                start_sample: FRAMES,
                end_sample: FRAMES
            },
        })
    );
    let one = NoteSpan {
        start_sample: 1_000,
        end_sample: 1_001,
    };
    assert_eq!(
        registry.extract(id, &analysis, &samples, one),
        Err(FeatureError::FeatureNotComputable {
            feature: FeatureSlot::Band {
                window: BandWindow::Attack,
                band: 0
            },
            index: 0,
            reason: NotComputableReason::WindowTooShort {
                start_sample: 1_000,
                end_sample: 1_001
            },
        })
    );
}

fn features_1_with(version: &str, value_count: usize) -> FeatureContractDefinition {
    let mut definition = FeatureContractDefinition::features_1();
    definition.version = version.to_string();
    definition.value_count = value_count;
    definition
}

/// `DefineFeatureContract` in the model's outcome order: `wrong-count` fires only for
/// `features/1` with a count other than 32, then `version-taken`. A taken `features/1` defined
/// with 31 values is a count mismatch (both outcomes apply, `wrong-count` first).
#[test]
fn a_taken_features_1_with_a_wrong_count_is_a_count_mismatch() {
    let mut registry = FeatureRegistry::new();
    registry.define(features_1_with("features/1", 32)).unwrap();
    assert_eq!(
        registry.define(features_1_with("features/1", 31)),
        Err(FeatureError::FeatureCountMismatch {
            version: "features/1".to_string(),
            value_count: 31
        })
    );
}

/// The model's `wrong-count` outcome has `when: {all: [version == "features/1", value_count !=
/// 32]}`; for any other version it does not fire, and a definition of a taken version is
/// `version-taken` whatever its count.
#[test]
fn a_taken_version_other_than_features_1_is_refused_as_taken() {
    let mut registry = FeatureRegistry::new();
    registry.define(features_1_with("features/2", 32)).unwrap();
    assert_eq!(
        registry.define(features_1_with("features/2", 33)),
        Err(FeatureError::FeatureVersionTaken {
            version: "features/2".to_string()
        })
    );
}

/// feature-contract-extract arranges its analysis with `fft_lengths: [512, 2048, 8192]` and
/// `hop_length: 512`, then extracts under the published `features/1`: outcome `extracted`.
#[test]
fn the_extract_scenario_analysis_at_hop_512_is_extracted() {
    let samples: Vec<f32> = (0..FRAMES)
        .map(|i| (0.5 * (2.0 * PI * 440.0 * i as f64 / SAMPLE_RATE_HZ as f64).sin()) as f32)
        .collect();
    let config = AnalysisConfig {
        stfts: [512, 2_048, 8_192]
            .into_iter()
            .map(|fft_length| StftConfig {
                fft_length,
                hop_length: 512,
            })
            .collect(),
    };
    let analysis = analyze(&samples, SAMPLE_RATE_HZ, &config).unwrap();
    let (registry, id) = published();
    let note = NoteSpan {
        start_sample: 0,
        end_sample: 24_000,
    };
    let vector = registry.extract(id, &analysis, &samples, note);
    assert_eq!(vector.map(|v| v.values.len()), Ok(32));
}
