//! Adversarial cases against story:analysis-core (wave a3). Each case names what it attacks.

use std::f64::consts::PI;

use patchback_analysis::{
    AnalysisConfig, AnalysisError, BAND_COUNT, BAND_EDGES_HZ, Complex, LOG_FLOOR_DB,
    MAX_DURATION_FRAMES, MAX_FFT_LENGTH, SAMPLE_RATE_HZ, analyze, estimate_f0, fft, hann, ifft,
    log_band_energies, peak, rms, spectral_centroid, spectral_flatness, spectral_flux,
    spectral_rolloff, stft,
};

const RATE: f64 = 48_000.0;

fn sine(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (amplitude * (2.0 * PI * frequency_hz * i as f64 / RATE).sin()) as f32)
        .collect()
}

fn saw(frequency_hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| {
            let phase = (frequency_hz * i as f64 / RATE).fract();
            (amplitude * (2.0 * phase - 1.0)) as f32
        })
        .collect()
}

fn cents(estimate: f64, truth: f64) -> f64 {
    1_200.0 * (estimate / truth).log2()
}

/// The band power of one magnitude spectrum in dB, computed directly in f64 from the documented
/// definition (power `sum m^2` per band, `10 log10`, floored at `LOG_FLOOR_DB`).
fn reference_bands(magnitudes: &[f64], bin_hz: f64) -> [f64; BAND_COUNT] {
    let mut power = [0.0f64; BAND_COUNT];
    for (k, m) in magnitudes.iter().enumerate() {
        let frequency = k as f64 * bin_hz;
        let band = (0..BAND_COUNT)
            .find(|&b| frequency < BAND_EDGES_HZ[b + 1])
            .unwrap_or(BAND_COUNT - 1);
        power[band] += m * m;
    }
    power.map(|p| {
        if p > 0.0 {
            (10.0 * p.log10()).max(LOG_FLOOR_DB)
        } else {
            LOG_FLOOR_DB
        }
    })
}

/// f0 is either refused or right: `None` when acceptable, `Some(reason)` otherwise.
fn f0_wrong(samples: &[f32], truth: f64, tolerance_cents: f64) -> Option<String> {
    match estimate_f0(samples, SAMPLE_RATE_HZ) {
        Ok(estimate) => {
            let error = cents(estimate.f0_hz, truth);
            (error.abs() > tolerance_cents).then(|| {
                format!(
                    "{truth} Hz reported as {} Hz ({error:.1} cents) with confidence {:.3}",
                    estimate.f0_hz, estimate.confidence
                )
            })
        }
        Err(AnalysisError::PitchAmbiguous { .. } | AnalysisError::PitchOutOfRange { .. }) => None,
        Err(other) => Some(format!("{truth} Hz: unexpected {other:?}")),
    }
}

// --- Band energies below the log floor -------------------------------------------------------

/// `log_band_energies` floors the spectrum's scale at `LOG_FLOOR_DB` before adding the band's
/// relative power, so a quiet spectrum's band energy is raised above its true value. Every bin of
/// an 8192-point spectrum at magnitude 1e-7 (-140 dB): band 7 holds 2049 bins, power 2.049e-11,
/// -106.9 dB.
#[test]
fn adversary_band_energy_of_a_quiet_spectrum_is_its_true_power() {
    let bin_hz = RATE / 8_192.0;
    let magnitudes = vec![1e-7; 4_097];
    let bands = log_band_energies(&magnitudes, bin_hz).unwrap();
    let expected = reference_bands(&magnitudes, bin_hz);
    assert!(
        (expected[7] - 10.0 * (2_049.0f64 * 1e-14).log10()).abs() < 1e-9,
        "reference {expected:?}"
    );
    for (b, (actual, expected)) in bands.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() < 1e-6,
            "band {b}: {actual} dB, true power is {expected} dB; all bands {bands:?}"
        );
    }
}

/// The same defect through `analyze`: a clip of `11 * 4096 + 2` samples ends in an 8192-point
/// frame holding two clip samples, the first under `w[0] = 0` and the second under
/// `w[1] ~ 1.5e-7`, so its spectrum is flat near -160 dB per bin and every band's true energy is
/// at the floor. Every frame of every STFT is compared with the band power of the crate's own
/// `stft` magnitudes.
#[test]
fn adversary_analyze_band_energies_match_the_stft_power() {
    let samples = sine(440.0, 0.5, 11 * 4_096 + 2);
    let analysis = analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()).unwrap();
    let mut wrong = Vec::new();
    for described in &analysis.stfts {
        let spectra = stft(&samples, described.fft_length, described.hop_length).unwrap();
        let bin_hz = RATE / described.fft_length as f64;
        for (index, (frame, magnitudes)) in
            described.frames.iter().zip(&spectra.magnitudes).enumerate()
        {
            let expected = reference_bands(magnitudes, bin_hz);
            for (b, (actual, expected)) in frame.band_energies_db.iter().zip(expected).enumerate() {
                if (actual - expected).abs() > 1e-6 {
                    wrong.push(format!(
                        "fft {} frame {index} band {b}: {actual} dB vs {expected} dB",
                        described.fft_length
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

// --- f0 outside the search range -------------------------------------------------------------

/// A sine above `F0_MAX_HZ` is reported an octave (or more) down with full confidence instead
/// of refused: C7 is 2093 Hz. DESIGN.md § 6.2: an unidentifiable pitch "must be surfaced rather
/// than guessed away".
#[test]
fn adversary_f0_above_the_search_range_is_refused_not_reported_an_octave_down() {
    let wrong: Vec<String> = [2_093.0, 3_000.0, 4_000.0]
        .into_iter()
        .filter_map(|f| f0_wrong(&sine(f, 0.5, 48_000), f, 5.0))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A sine just below `F0_MIN_HZ` is reported at the edge of the range with high confidence
/// instead of refused: G1 is 49.0 Hz.
#[test]
fn adversary_f0_just_below_the_search_range_is_refused_not_pinned_to_the_edge() {
    let wrong: Vec<String> = [48.0, 49.0]
        .into_iter()
        .filter_map(|f| f0_wrong(&sine(f, 0.5, 48_000), f, 5.0))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The stated 5-cent tolerance across the whole search range, not four chosen frequencies:
/// 49 sines log-spaced from 50 to 2000 Hz.
#[test]
fn adversary_f0_of_sines_across_the_range_is_within_5_cents() {
    let wrong: Vec<String> = (0..=48)
        .map(|i| 50.0 * 40f64.powf(f64::from(i) / 48.0))
        .filter_map(|f| f0_wrong(&sine(f, 0.5, 48_000), f, 5.0))
        .collect();
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The same for naive saws, where octave errors live.
#[test]
fn adversary_f0_of_saws_across_the_range_is_within_5_cents() {
    let wrong: Vec<String> = (0..=48)
        .map(|i| 50.0 * 40f64.powf(f64::from(i) / 48.0))
        .filter_map(|f| f0_wrong(&saw(f, 0.5, 48_000), f, 5.0))
        .collect();
    assert!(wrong.is_empty(), "{} wrong: {wrong:#?}", wrong.len());
}

/// The edges themselves, 50 Hz and 2000 Hz, sine and saw.
#[test]
fn adversary_f0_at_the_range_edges_is_within_5_cents() {
    for f in [50.0, 2_000.0] {
        for (shape, samples) in [("sine", sine(f, 0.5, 48_000)), ("saw", saw(f, 0.5, 48_000))] {
            let estimate = estimate_f0(&samples, SAMPLE_RATE_HZ)
                .unwrap_or_else(|e| panic!("{shape} {f} Hz: {e:?}"));
            let error = cents(estimate.f0_hz, f);
            assert!(error.abs() <= 5.0, "{shape} {f} Hz: {estimate:?}");
        }
    }
}

/// A clip shorter than `F0_WINDOW + max_lag + 1` (3009 samples) is refused as an ambiguous
/// pitch with confidence 0, even when it is 62 periods of a clean 1 kHz sine: the refusal names
/// the wrong cause.
#[test]
fn adversary_a_short_clean_sine_is_not_called_ambiguous() {
    let samples = sine(1_000.0, 0.5, 3_000);
    match estimate_f0(&samples, SAMPLE_RATE_HZ) {
        Ok(estimate) => assert!(cents(estimate.f0_hz, 1_000.0).abs() <= 5.0, "{estimate:?}"),
        Err(error) => panic!("62 periods of a 1 kHz sine: {error:?} ({error})"),
    }
}

// --- Finite output ---------------------------------------------------------------------------

/// "Every function ... returns finite values or a typed AnalysisError": a finite bin width and
/// finite magnitudes give an infinite centroid and rolloff.
#[test]
fn adversary_centroid_and_rolloff_are_finite_or_refused() {
    let magnitudes = [0.0, 0.0, 1.0];
    for (what, result) in [
        ("centroid", spectral_centroid(&magnitudes, f64::MAX)),
        ("rolloff", spectral_rolloff(&magnitudes, f64::MAX)),
    ] {
        if let Ok(value) = result {
            assert!(value.is_finite(), "{what} = {value}");
        }
    }
}

// --- Mutants the suite misses ----------------------------------------------------------------

/// No existing test reads an STFT frame after the first: a mutant that reads every frame from
/// sample 0 passes. 8 samples, length 4, hop 4, an impulse at sample 6: frame 0 is silent and
/// frame 1 holds the impulse at `w[2] = 1`, a flat spectrum of 1.
#[test]
fn adversary_stft_frame_reads_from_its_hop_offset() {
    let spectra = stft(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0], 4, 4).unwrap();
    assert_eq!(spectra.magnitudes.len(), 2);
    for (index, expected) in [[0.0; 3], [1.0; 3]].iter().enumerate() {
        for (k, (a, e)) in spectra.magnitudes[index].iter().zip(expected).enumerate() {
            assert!((a - e).abs() < 1e-12, "frame {index} bin {k}: {a} vs {e}");
        }
    }
}

/// No existing test reads a frame descriptor of `analyze` other than the first frame's loudest
/// band: mutants of the wiring in `describe` (flux against no spectrum, the whole clip's RMS for
/// each frame, centroid and rolloff swapped) pass. Every descriptor of every frame is checked
/// against the public functions on that frame's own samples and spectrum.
#[test]
fn adversary_frame_descriptors_are_those_of_their_own_frame() {
    let mut samples = sine(440.0, 0.5, 24_000);
    samples.extend(sine(660.0, 0.2, 24_000));
    let analysis = analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()).unwrap();
    for described in &analysis.stfts {
        let n = described.fft_length;
        let hop = described.hop_length;
        let bin_hz = RATE / n as f64;
        let spectra = stft(&samples, n, hop).unwrap();
        assert_eq!(described.frames.len(), spectra.magnitudes.len());
        let mut previous: &[f64] = &[];
        for (index, (frame, magnitudes)) in
            described.frames.iter().zip(&spectra.magnitudes).enumerate()
        {
            let start = index * hop;
            let slice = &samples[start..(start + n).min(samples.len())];
            let at = format!("fft {n} frame {index}");
            assert_eq!(frame.rms, rms(slice).unwrap(), "{at} rms");
            assert_eq!(frame.peak, peak(slice).unwrap(), "{at} peak");
            assert_eq!(
                frame.centroid_hz,
                spectral_centroid(magnitudes, bin_hz).unwrap(),
                "{at} centroid"
            );
            assert_eq!(
                frame.rolloff_hz,
                spectral_rolloff(magnitudes, bin_hz).unwrap(),
                "{at} rolloff"
            );
            assert_eq!(
                frame.flatness,
                spectral_flatness(magnitudes).unwrap(),
                "{at} flatness"
            );
            assert_eq!(
                frame.flux,
                spectral_flux(previous, magnitudes).unwrap(),
                "{at} flux"
            );
            previous = magnitudes;
        }
        // the clip changes at sample 24000, so frames differ: a constant descriptor is a mutant
        let first = &described.frames[1];
        let late = &described.frames[described.frames.len() - 3];
        assert_ne!(first.rms, late.rms, "fft {n}");
        assert_ne!(first.centroid_hz, late.centroid_hz, "fft {n}");
    }
}

/// The duration bound admits exactly `MAX_DURATION_FRAMES`; only `+ 1` is tested, so a `>=`
/// mutant passes.
#[test]
fn adversary_duration_bound_admits_the_bound_itself() {
    let samples = sine(440.0, 0.5, MAX_DURATION_FRAMES);
    let analysis = analyze(&samples, SAMPLE_RATE_HZ, &AnalysisConfig::default()).unwrap();
    assert_eq!(analysis.frame_count, MAX_DURATION_FRAMES);
}

/// The FFT bound admits exactly `MAX_FFT_LENGTH`; only `2 * MAX` is tested, so a `>=` mutant
/// passes. At the bound: an impulse at index 1 transforms to `e^(-2 pi i k / N)`, and the
/// roundtrip holds to 1e-12.
#[test]
fn adversary_fft_admits_the_length_bound_and_stays_accurate_there() {
    let n = MAX_FFT_LENGTH;
    let mut buffer = vec![Complex::ZERO; n];
    buffer[1] = Complex::new(1.0, 0.0);
    fft(&mut buffer).unwrap();
    for (k, z) in buffer.iter().enumerate() {
        let e = Complex::from_angle(-2.0 * PI * k as f64 / n as f64);
        assert!(
            (z.re - e.re).abs() < 1e-12 && (z.im - e.im).abs() < 1e-12,
            "bin {k}: {z:?} vs {e:?}"
        );
    }
    ifft(&mut buffer).unwrap();
    for (i, z) in buffer.iter().enumerate() {
        let e = if i == 1 { 1.0 } else { 0.0 };
        assert!((z.re - e).abs() < 1e-12 && z.im.abs() < 1e-12, "{i}: {z:?}");
    }
    assert_eq!(hann(n).unwrap().len(), n);
}

/// A sine 1 dB above the provisional silence threshold is analysed, not refused.
#[test]
fn adversary_near_silence_above_the_threshold_is_analysed() {
    let amplitude = 10f64.powf(-59.0 / 20.0) * 2f64.sqrt();
    let analysis = analyze(
        &sine(440.0, amplitude, 48_000),
        SAMPLE_RATE_HZ,
        &AnalysisConfig::default(),
    )
    .unwrap();
    assert!(
        (analysis.rms_dbfs + 59.0).abs() < 0.05,
        "{}",
        analysis.rms_dbfs
    );
    assert!(cents(analysis.f0.f0_hz, 440.0).abs() <= 5.0);
}

/// A hop near `usize::MAX` does not overflow the frame start.
#[test]
fn adversary_stft_with_a_huge_hop_has_one_frame() {
    for hop in [usize::MAX, usize::MAX - 1, usize::MAX / 2 + 1] {
        let spectra = stft(&[0.5; 8], 4, hop).unwrap();
        assert_eq!(spectra.magnitudes.len(), 1, "hop {hop}");
    }
}
