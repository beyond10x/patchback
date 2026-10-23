//! [`analyze`] against the acceptance scenarios of story:analysis-core
//! (`spec/scenarios/analysis-core-*.yaml`), one test per scenario step, plus the order in which
//! refusals are checked (the order of the `AnalyzeAudio` outcomes in `spec/domains/analysis.yaml`).

use std::f64::consts::PI;

use patchback_analysis::{
    ANALYSIS_FFT_LENGTHS, AnalysisConfig, AnalysisError, F0_MAX_HZ, F0_MIN_HZ, MAX_DURATION_FRAMES,
    MAX_FFT_LENGTH, MAX_STFT_VALUES, SAMPLE_RATE_HZ, StftConfig, analyze,
};

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            (amplitude * (2.0 * PI * frequency_hz * i as f64 / f64::from(SAMPLE_RATE_HZ)).sin())
                as f32
        })
        .collect()
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

fn with(stfts: &[(usize, usize)]) -> AnalysisConfig {
    AnalysisConfig {
        stfts: stfts
            .iter()
            .map(|&(fft_length, hop_length)| StftConfig {
                fft_length,
                hop_length,
            })
            .collect(),
    }
}

/// analysis-core-analyzed: a one-second 48 kHz clip is analysed at 512, 2048 and 8192 with a
/// half-frame hop, and every value is finite.
#[test]
fn analysis_core_analyzed() {
    let config = AnalysisConfig::default();
    assert_eq!(
        config.stfts,
        ANALYSIS_FFT_LENGTHS
            .iter()
            .map(|&n| StftConfig {
                fft_length: n,
                hop_length: n / 2
            })
            .collect::<Vec<_>>()
    );
    let analysis = analyze(&sine(440.0, 0.5, 48_000), SAMPLE_RATE_HZ, &config).unwrap();
    assert_eq!(analysis.sample_rate_hz, 48_000);
    assert_eq!(analysis.frame_count, 48_000);
    let shape: Vec<(usize, usize, usize)> = analysis
        .stfts
        .iter()
        .map(|s| (s.fft_length, s.hop_length, s.frames.len()))
        .collect();
    assert_eq!(
        shape,
        [(512, 256, 188), (2_048, 1_024, 47), (8_192, 4_096, 12)]
    );
    assert!((analysis.f0.f0_hz - 440.0).abs() < 1.0, "{:?}", analysis.f0);
    let rms_dbfs = 20.0 * (0.5 / 2f64.sqrt()).log10();
    assert!(
        (analysis.rms_dbfs - rms_dbfs).abs() < 0.01,
        "{}",
        analysis.rms_dbfs
    );
    assert!((analysis.peak - 0.5).abs() < 1e-3);
    for stft in &analysis.stfts {
        for frame in &stft.frames {
            let values = [
                frame.rms,
                frame.peak,
                frame.centroid_hz,
                frame.rolloff_hz,
                frame.flatness,
                frame.flux,
            ];
            assert!(
                values
                    .iter()
                    .chain(&frame.band_energies_db)
                    .all(|v| v.is_finite()),
                "{frame:?}"
            );
        }
        // the sine's energy is in the 375-750 Hz band of every full frame
        let first = &stft.frames[0];
        let loudest = first
            .band_energies_db
            .iter()
            .enumerate()
            .fold(
                (0, f64::MIN),
                |b, (i, e)| if *e > b.1 { (i, *e) } else { b },
            );
        assert_eq!(loudest.0, 2, "{} {first:?}", stft.fft_length);
    }
}

/// analysis-core-refused-input, step 1: empty audio.
#[test]
fn analysis_core_refused_input_empty() {
    assert_eq!(
        analyze(&[], SAMPLE_RATE_HZ, &AnalysisConfig::default()),
        Err(AnalysisError::EmptyAudio)
    );
}

/// analysis-core-refused-input, step 2: a 44.1 kHz clip; nothing is converted.
#[test]
fn analysis_core_refused_input_sample_rate() {
    assert_eq!(
        analyze(
            &sine(440.0, 0.5, 44_100),
            44_100,
            &AnalysisConfig::default()
        ),
        Err(AnalysisError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
}

/// analysis-core-refused-input, step 3: a NaN or infinite sample.
#[test]
fn analysis_core_refused_input_non_finite() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut samples = sine(440.0, 0.5, 48_000);
        samples[30_000] = bad;
        assert_eq!(
            analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()),
            Err(AnalysisError::NonFiniteSamples { index: 30_000 })
        );
    }
}

/// analysis-core-refused-bounds, step 1: a hop of zero.
#[test]
fn analysis_core_refused_bounds_hop() {
    assert_eq!(
        analyze(
            &sine(440.0, 0.5, 48_000),
            SAMPLE_RATE_HZ,
            &with(&[(512, 256), (2_048, 0)])
        ),
        Err(AnalysisError::InvalidHopLength { hop_length: 0 })
    );
}

/// analysis-core-refused-bounds, step 2: an FFT length of 500.
#[test]
fn analysis_core_refused_bounds_not_power_of_two() {
    assert_eq!(
        analyze(
            &sine(440.0, 0.5, 48_000),
            SAMPLE_RATE_HZ,
            &with(&[(500, 250)])
        ),
        Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length: 500 })
    );
}

/// analysis-core-refused-bounds, step 3: an FFT length of 1048576, over the bound.
#[test]
fn analysis_core_refused_bounds_fft_over_bound() {
    const { assert!(1_048_576 > MAX_FFT_LENGTH) };
    assert_eq!(
        analyze(
            &sine(440.0, 0.5, 48_000),
            SAMPLE_RATE_HZ,
            &with(&[(1_048_576, 512)])
        ),
        Err(AnalysisError::FftLengthOverBound {
            fft_length: 1_048_576
        })
    );
}

/// analysis-core-refused-bounds, step 4: a clip one sample over the duration bound.
#[test]
fn analysis_core_refused_bounds_duration() {
    let samples = vec![0.25f32; MAX_DURATION_FRAMES + 1];
    assert_eq!(
        analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()),
        Err(AnalysisError::DurationOverBound {
            frame_count: MAX_DURATION_FRAMES + 1
        })
    );
}

/// analysis-core-silence-and-pitch, step 1: digital silence and a -70 dBFS sine are silent.
#[test]
fn analysis_core_silence_and_pitch_silent() {
    match analyze(
        &vec![0.0; 48_000],
        SAMPLE_RATE_HZ,
        &AnalysisConfig::default(),
    ) {
        Err(AnalysisError::SilentInput { rms_dbfs }) => assert!(rms_dbfs.is_finite()),
        other => panic!("{other:?}"),
    }
    let quiet = 10f64.powf(-70.0 / 20.0) * 2f64.sqrt();
    match analyze(
        &sine(440.0, quiet, 48_000),
        SAMPLE_RATE_HZ,
        &AnalysisConfig::default(),
    ) {
        Err(AnalysisError::SilentInput { rms_dbfs }) => {
            assert!((rms_dbfs + 70.0).abs() < 0.1, "{rms_dbfs}")
        }
        other => panic!("{other:?}"),
    }
}

/// analysis-core-silence-and-pitch, step 2: noise has no pitch, so nothing is analysed.
#[test]
fn analysis_core_silence_and_pitch_ambiguous() {
    match analyze(&noise(48_000), SAMPLE_RATE_HZ, &AnalysisConfig::default()) {
        Err(AnalysisError::PitchAmbiguous { confidence }) => {
            assert!(confidence.is_finite() && confidence < 0.85)
        }
        other => panic!("{other:?}"),
    }
}

/// Refusals come in the order of the AnalyzeAudio outcomes: empty, sample rate, hop, power of
/// two, FFT bound, duration, non-finite, silence, pitch.
#[test]
fn refusals_follow_the_order_of_the_domain_outcomes() {
    let bad_config = with(&[(500, 0)]);
    assert_eq!(
        analyze(&[], 44_100, &bad_config),
        Err(AnalysisError::EmptyAudio)
    );
    assert_eq!(
        analyze(&[f32::NAN], 44_100, &bad_config),
        Err(AnalysisError::UnsupportedSampleRate {
            sample_rate_hz: 44_100
        })
    );
    assert_eq!(
        analyze(&[f32::NAN], SAMPLE_RATE_HZ, &bad_config),
        Err(AnalysisError::InvalidHopLength { hop_length: 0 })
    );
    assert_eq!(
        analyze(
            &[f32::NAN],
            SAMPLE_RATE_HZ,
            &with(&[(1_048_576, 1), (500, 1)])
        ),
        Err(AnalysisError::FftLengthNotPowerOfTwo { fft_length: 500 })
    );
    let mut long = vec![0.0f32; MAX_DURATION_FRAMES + 1];
    long[0] = f32::NAN;
    assert_eq!(
        analyze(&long, SAMPLE_RATE_HZ, &with(&[(1_048_576, 1)])),
        Err(AnalysisError::FftLengthOverBound {
            fft_length: 1_048_576
        })
    );
    assert_eq!(
        analyze(&long, SAMPLE_RATE_HZ, &AnalysisConfig::default()),
        Err(AnalysisError::DurationOverBound {
            frame_count: MAX_DURATION_FRAMES + 1
        })
    );
    assert_eq!(
        analyze(
            &[0.0, f32::INFINITY],
            SAMPLE_RATE_HZ,
            &AnalysisConfig::default()
        ),
        Err(AnalysisError::NonFiniteSamples { index: 1 })
    );
}

/// The STFT value bound comes after the duration bound and before the sample checks, and is
/// checked before anything is allocated: a hop of 1 at an FFT of 65 536 over 1024 samples.
#[test]
fn an_stft_over_the_value_bound_is_refused_after_the_duration_bound() {
    let mut clip = sine(440.0, 0.5, 1_024);
    clip[3] = f32::NAN;
    assert_eq!(
        analyze(&clip, SAMPLE_RATE_HZ, &with(&[(512, 256), (65_536, 1)])),
        Err(AnalysisError::FrameCountOverBound {
            fft_length: 65_536,
            hop_length: 1,
            frame_count: 1_024
        })
    );
}

/// The STFT value bound holds over the whole configuration: two STFTs of 512 frames at 65 536
/// (32 769 bins) are each under [`MAX_STFT_VALUES`] and together over it, so the second is
/// refused before either is computed; one of them alone passes the bound.
#[test]
fn the_stft_value_bound_holds_over_the_whole_configuration() {
    const { assert!(512 * 32_769 <= MAX_STFT_VALUES && 2 * 512 * 32_769 > MAX_STFT_VALUES) };
    let clip = sine(440.0, 0.5, 1_024);
    assert_eq!(
        analyze(&clip, SAMPLE_RATE_HZ, &with(&[(65_536, 2), (65_536, 2)])),
        Err(AnalysisError::FrameCountOverBound {
            fft_length: 65_536,
            hop_length: 2,
            frame_count: 512
        })
    );
    assert_eq!(
        analyze(&clip, SAMPLE_RATE_HZ, &with(&[(65_536, 2)])),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: 1_024,
            required_frame_count: 2_074
        })
    );
}

/// Pitch refusals through `analyze`: a clip too short for the f0 window, and a pitch above or
/// below the search range, each with its own error; nothing else is returned.
#[test]
fn pitch_refusals_name_their_cause() {
    assert_eq!(
        analyze(
            &sine(440.0, 0.5, 2_000),
            SAMPLE_RATE_HZ,
            &AnalysisConfig::default()
        ),
        Err(AnalysisError::ClipTooShortForPitch {
            frame_count: 2_000,
            required_frame_count: 2_074
        })
    );
    assert_eq!(
        analyze(
            &sine(3_000.0, 0.5, 48_000),
            SAMPLE_RATE_HZ,
            &AnalysisConfig::default()
        ),
        Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MAX_HZ
        })
    );
    assert_eq!(
        analyze(
            &sine(48.0, 0.5, 48_000),
            SAMPLE_RATE_HZ,
            &AnalysisConfig::default()
        ),
        Err(AnalysisError::PitchOutOfRange {
            bound_hz: F0_MIN_HZ
        })
    );
}

/// A clip shorter than every frame still yields one zero-padded frame per FFT length.
#[test]
fn a_clip_shorter_than_the_frame_is_zero_padded() {
    let analysis = analyze(
        &sine(440.0, 0.5, 4_000),
        SAMPLE_RATE_HZ,
        &AnalysisConfig::default(),
    )
    .unwrap();
    let counts: Vec<usize> = analysis.stfts.iter().map(|s| s.frames.len()).collect();
    assert_eq!(counts, [16, 4, 1]);
}
