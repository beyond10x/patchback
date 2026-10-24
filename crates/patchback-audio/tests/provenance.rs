//! story:external-audio-provenance (DESIGN.md § 6.1, § 12.3, the AGENTS.md fixture rule): an
//! external recording is imported only with its provenance and licence, a clip synthesized by
//! Patchback carries its origin, and the two guards sit where `ImportWav`
//! (`spec/domains/audio.yaml`) puts them among the reader's refusals.

mod support;

use std::path::{Path, PathBuf};

use patchback_audio::{
    AudioOrigin, Corruption, IMPORT_WAV_PRECEDENCE, MAX_FRAMES, ReadError, SAMPLE_RATE_HZ,
    Truncation, WavError, WavImport, WavSampleFormat, decode, read_wav,
};
use support::*;

const PROVENANCE: &str = "recorded by the project from a hardware monosynth, 2026-10-20";
const LICENCE: &str = "CC-BY-4.0";
const RENDER: &str = "0b6f3c1e-5a7d-4e8b-9c2f-1d3e5f7a9b0c";

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-audio-provenance");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn write(name: &str, bytes: &[u8]) -> PathBuf {
    let path = scratch(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// The scenarios' file: 72 000 frames of 48 kHz mono PCM16 (1.5 s), a synthesized ramp.
fn note_wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..72_000)
        .map(|i| i16::try_from(i % 2_000 - 1_000).unwrap())
        .collect();
    pcm16_wav(1, &samples)
}

fn external(provenance: Option<&str>, licence: Option<&str>) -> WavImport {
    WavImport {
        origin: AudioOrigin::ExternalRecording,
        provenance: provenance.map(str::to_owned),
        licence: licence.map(str::to_owned),
        render_id: None,
    }
}

fn synthesized(render_id: Option<&str>) -> WavImport {
    WavImport {
        origin: AudioOrigin::PatchbackSynthesized,
        provenance: None,
        licence: None,
        render_id: render_id.map(str::to_owned),
    }
}

fn read_refusal(import: &WavImport, path: &Path) -> WavError {
    match import.read(path) {
        Ok(clip) => panic!("imported {} frames, expected a refusal", clip.frame_count()),
        Err(ReadError::Wav(error)) => error,
        Err(ReadError::Io(error)) => panic!("the file could not be read: {error}"),
    }
}

/// Reads `bytes` with `import` through a pipe, which the reader walks front to back.
fn import_through_a_pipe(import: &WavImport, bytes: Vec<u8>) -> Result<(), WavError> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    let (reader, mut writer) = std::io::pipe().unwrap();
    let path = format!("/proc/self/fd/{}", reader.as_raw_fd());
    let feeder = std::thread::spawn(move || {
        let _ = writer.write_all(&bytes);
    });
    let result = import.read(Path::new(&path));
    drop(reader);
    feeder.join().unwrap();
    match result {
        Ok(_) => Ok(()),
        Err(ReadError::Wav(error)) => Err(error),
        Err(ReadError::Io(error)) => panic!("the pipe failed: {error}"),
    }
}

// The three acceptance scenarios, one test each.

/// spec/scenarios/external-audio-provenance-accepted.yaml
#[test]
fn external_recording_with_provenance_and_licence_imports_and_keeps_both() {
    let path = write("external-note.wav", &note_wav());
    let clip = external(Some(PROVENANCE), Some(LICENCE))
        .read(&path)
        .unwrap();
    assert_eq!(clip.origin(), Some(AudioOrigin::ExternalRecording));
    assert_eq!(clip.provenance(), Some(PROVENANCE));
    assert_eq!(clip.licence(), Some(LICENCE));
    assert_eq!(clip.render_id(), None);
    assert_eq!(clip.frame_count(), 72_000);
    assert_eq!(clip.sample_rate_hz(), SAMPLE_RATE_HZ);
    assert_eq!(clip.source_channel_count(), 1);
    assert!(!clip.downmixed());
    assert_eq!(clip.source_format(), Some(WavSampleFormat::Pcm16));
    assert_eq!(clip.samples(), read_wav(&path).unwrap().samples());
}

/// spec/scenarios/external-audio-provenance-missing.yaml
#[test]
fn external_recording_without_provenance_is_refused_and_no_clip_exists() {
    let path = write("unknown-source.wav", &note_wav());
    let import = external(None, Some(LICENCE));
    assert_eq!(read_refusal(&import, &path), WavError::ProvenanceMissing);
    assert_eq!(
        WavError::ProvenanceMissing.domain_error(),
        "patchback.audio.ProvenanceMissing"
    );
    assert_eq!(import.decode(&note_wav()), Err(WavError::ProvenanceMissing));
    assert_eq!(import.check(), Err(WavError::ProvenanceMissing));
}

/// spec/scenarios/external-audio-provenance-licence-missing.yaml
#[test]
fn external_recording_with_provenance_and_no_licence_is_refused_and_no_clip_exists() {
    let path = write("unlicensed.wav", &note_wav());
    let import = external(Some("downloaded from a sample pack, author unknown"), None);
    assert_eq!(read_refusal(&import, &path), WavError::LicenceMissing);
    assert_eq!(
        WavError::LicenceMissing.domain_error(),
        "patchback.audio.LicenceMissing"
    );
    assert_eq!(import.decode(&note_wav()), Err(WavError::LicenceMissing));
    assert_eq!(import.check(), Err(WavError::LicenceMissing));
}

// Beyond the scenarios.

#[test]
fn an_external_recording_with_neither_is_refused_for_provenance_first() {
    assert_eq!(
        external(None, None).decode(&note_wav()),
        Err(WavError::ProvenanceMissing)
    );
}

#[test]
fn a_synthesized_import_needs_neither_and_keeps_the_render_it_names() {
    let bytes = pcm16_wav(2, &[1_000, -3_000, i16::MAX, i16::MIN]);
    let clip = synthesized(Some(RENDER)).decode(&bytes).unwrap();
    assert_eq!(clip.origin(), Some(AudioOrigin::PatchbackSynthesized));
    assert_eq!(clip.render_id(), Some(RENDER));
    assert_eq!(clip.provenance(), None);
    assert_eq!(clip.licence(), None);
    assert!(clip.downmixed());
    assert_eq!(clip.samples(), decode(&bytes).unwrap().samples());

    let clip = synthesized(None).decode(&bytes).unwrap();
    assert_eq!(clip.origin(), Some(AudioOrigin::PatchbackSynthesized));
    assert_eq!(clip.render_id(), None);
}

#[test]
fn an_import_keeps_whatever_it_was_given() {
    let import = WavImport {
        origin: AudioOrigin::PatchbackSynthesized,
        provenance: Some(PROVENANCE.to_owned()),
        licence: Some(LICENCE.to_owned()),
        render_id: Some(RENDER.to_owned()),
    };
    let clip = import.decode(&note_wav()).unwrap();
    assert_eq!(clip.provenance(), Some(PROVENANCE));
    assert_eq!(clip.licence(), Some(LICENCE));
    assert_eq!(clip.render_id(), Some(RENDER));

    let mut import = external(Some(PROVENANCE), Some(LICENCE));
    import.render_id = Some(RENDER.to_owned());
    let clip = import.decode(&note_wav()).unwrap();
    assert_eq!(clip.origin(), Some(AudioOrigin::ExternalRecording));
    assert_eq!(clip.render_id(), Some(RENDER));
}

// Precedence: the guards are input refusals and answer before every other refusal, the
// `before_row` truncated and corrupt-chunk outcomes included (`IMPORT_WAV_PRECEDENCE`).

#[test]
fn a_truncated_external_file_is_refused_by_either_guard_before_its_truncation() {
    let whole = note_wav();
    let cut = &whole[..whole.len() - 1];
    let truncated = WavError::Truncated(Truncation::RiffSize {
        declared: u32::try_from(whole.len() - 8).unwrap(),
        available: u64::try_from(cut.len() - 8).unwrap(),
    });
    assert_eq!(decode(cut), Err(truncated));
    let path = write("truncated-external.wav", cut);
    for (import, expected) in [
        (external(None, None), WavError::ProvenanceMissing),
        (external(Some(PROVENANCE), None), WavError::LicenceMissing),
        (external(Some(PROVENANCE), Some(LICENCE)), truncated),
        (synthesized(None), truncated),
    ] {
        assert_eq!(import.decode(cut), Err(expected));
        assert_eq!(read_refusal(&import, &path), expected);
        assert_eq!(import_through_a_pipe(&import, cut.to_vec()), Err(expected));
    }
}

/// The guards need no byte of the file: a request that fails one is refused before the file is
/// opened, so a missing path or one that cannot be read as a file gives the guard, not
/// [`ReadError::Io`].
#[test]
fn an_external_import_missing_a_guard_is_refused_before_the_file_is_opened() {
    let missing = scratch("no-such-file.wav");
    let _ = std::fs::remove_file(&missing);
    let directory = scratch("a-directory.wav");
    std::fs::create_dir_all(&directory).unwrap();
    for path in [&missing, &directory] {
        for (import, expected) in [
            (external(None, None), WavError::ProvenanceMissing),
            (external(None, Some(LICENCE)), WavError::ProvenanceMissing),
            (external(Some(PROVENANCE), None), WavError::LicenceMissing),
        ] {
            match import.read(path) {
                Err(ReadError::Wav(error)) => assert_eq!(error, expected, "{}", path.display()),
                other => panic!("{}: expected {expected:?}, got {other:?}", path.display()),
            }
        }
        for import in [external(Some(PROVENANCE), Some(LICENCE)), synthesized(None)] {
            assert!(
                matches!(import.read(path), Err(ReadError::Io(_))),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn a_corrupt_external_file_is_refused_by_either_guard_before_its_corruption() {
    let mut block_align = fmt_body(1, 1, 48_000, 16);
    block_align[12..14].copy_from_slice(&4u16.to_le_bytes());
    let cases = [
        (b"RIFX".to_vec(), Corruption::NotRiff),
        (
            riff(&[chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16))]),
            Corruption::MissingData,
        ),
        (
            riff(&[
                chunk(b"fmt ", &block_align),
                chunk(b"data", &pcm16_data(&[0, 0])),
            ]),
            Corruption::BlockAlign { declared: 4 },
        ),
        (
            riff(&[
                chunk(b"fmt ", &fmt_body(1, 2, 48_000, 16)),
                chunk(b"data", &pcm16_data(&[0, 0, 0])),
            ]),
            Corruption::PartialFrame {
                data_size: 6,
                block_align: 4,
            },
        ),
    ];
    for (bytes, how) in cases {
        let corrupt = WavError::CorruptChunk(how);
        assert_eq!(decode(&bytes), Err(corrupt));
        let path = write("corrupt-external.wav", &bytes);
        for (import, expected) in [
            (external(None, Some(LICENCE)), WavError::ProvenanceMissing),
            (external(Some(PROVENANCE), None), WavError::LicenceMissing),
            (external(Some(PROVENANCE), Some(LICENCE)), corrupt),
            (synthesized(None), corrupt),
        ] {
            assert_eq!(import.decode(&bytes), Err(expected), "{how:?}");
            assert_eq!(read_refusal(&import, &path), expected, "{how:?}");
            assert_eq!(
                import_through_a_pipe(&import, bytes.clone()),
                Err(expected),
                "{how:?}"
            );
        }
    }
}

#[test]
fn a_44_1_khz_external_file_is_refused_by_the_guards_before_its_rate() {
    let mut samples = vec![0i16; 44_100];
    samples[1] = 100;
    let bytes = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 44_100, 16)),
        chunk(b"data", &pcm16_data(&samples)),
    ]);
    let rate = WavError::UnsupportedSampleRate {
        sample_rate_hz: 44_100,
    };
    assert_eq!(decode(&bytes), Err(rate));
    let path = write("external-44100.wav", &bytes);
    let cases = [
        (external(None, Some(LICENCE)), WavError::ProvenanceMissing),
        (external(Some(PROVENANCE), None), WavError::LicenceMissing),
        (external(Some(PROVENANCE), Some(LICENCE)), rate),
        (synthesized(None), rate),
    ];
    for (import, expected) in cases {
        assert_eq!(import.decode(&bytes), Err(expected));
        assert_eq!(read_refusal(&import, &path), expected);
        assert_eq!(import_through_a_pipe(&import, bytes.clone()), Err(expected));
    }
}

#[test]
fn the_guards_precede_the_format_channel_empty_non_finite_and_length_refusals() {
    let format = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 24)),
        chunk(b"data", &[0; 6]),
    ]);
    let channels = riff(&[
        chunk(b"fmt ", &fmt_body(1, 3, 48_000, 16)),
        chunk(b"data", &pcm16_data(&[0, 0, 0])),
    ]);
    let empty = pcm16_wav(1, &[]);
    let non_finite = f32_wav(1, &[0.0, f32::NAN]);
    let too_long = riff(&[
        chunk(b"fmt ", &fmt_body(1, 1, 48_000, 16)),
        chunk(b"data", &vec![0; (MAX_FRAMES + 1) * 2]),
    ]);
    for bytes in [format, channels, empty, non_finite, too_long] {
        let plain = decode(&bytes).expect_err("the reader refuses it");
        assert!(
            !matches!(plain, WavError::Truncated(_) | WavError::CorruptChunk(_)),
            "{plain:?}"
        );
        assert_eq!(
            external(None, Some(LICENCE)).decode(&bytes),
            Err(WavError::ProvenanceMissing),
            "{plain:?}"
        );
        assert_eq!(
            external(Some(PROVENANCE), None).decode(&bytes),
            Err(WavError::LicenceMissing),
            "{plain:?}"
        );
        assert_eq!(
            external(Some(PROVENANCE), Some(LICENCE)).decode(&bytes),
            Err(plain)
        );
    }
}

#[test]
fn import_wav_precedence_is_the_order_ess_evaluates_the_outcomes_of_the_spec() {
    let domain = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/audio.yaml"
    ))
    .expect("spec/domains/audio.yaml");
    // Each outcome of ImportWav, in declaration order.
    #[derive(Debug, Default)]
    struct Outcome {
        error: String,
        when: bool,
        when_related: bool,
        external: bool,
        before_row: bool,
    }
    let mut outcomes: Vec<Outcome> = Vec::new();
    let command = domain
        .lines()
        .skip_while(|l| l.trim() != "- name: patchback.audio.ImportWav")
        .skip(1)
        .take_while(|l| !l.starts_with("  - name: "));
    for line in command.skip_while(|l| l.trim() != "outcomes:").skip(1) {
        let trimmed = line.trim();
        if line.starts_with("      - name: ") {
            outcomes.push(Outcome::default());
        } else if let Some(outcome) = outcomes.last_mut() {
            if let Some(error) = trimmed.strip_prefix("error: ") {
                outcome.error = error.to_owned();
            } else if trimmed.starts_with("when:") {
                outcome.when = true;
            } else if trimmed.starts_with("when_related:") {
                outcome.when_related = true;
            } else if trimmed.starts_with("external: ") {
                outcome.external = true;
            } else if trimmed == "before_row: true" {
                outcome.before_row = true;
            }
        }
    }
    let mut refusals: Vec<&Outcome> = outcomes.iter().filter(|o| !o.error.is_empty()).collect();
    assert!(refusals.len() >= 10, "{refusals:?}");
    // The order ESS answers a command's outcomes in, documented in the `ess` repository at
    // website/docs/guides/specify/guards-and-predicates.md § "Which branch answers" and
    // website/docs/reference/diagnostics.md (ESS-AUTHOR-041): the input refusals (a `when:`
    // with an `error:` and no subject) in declaration order; `before_row: true` externals, taken
    // before the row is read, so before the existence check; the existence refusal
    // (`when_related`); then the other externals in declaration order. ImportWav has no
    // held-state branch. A stable sort keeps declaration order within each step.
    let step = |o: &Outcome| match (o.when, o.when_related, o.external, o.before_row) {
        (true, false, false, false) => 0,
        (false, false, true, true) => 1,
        (false, true, false, false) => 2,
        (false, false, true, false) => 3,
        _ => panic!("an ImportWav refusal of no known step: {o:?}"),
    };
    refusals.sort_by_key(|o| step(o));
    let order: Vec<&str> = refusals.iter().map(|o| o.error.as_str()).collect();
    assert_eq!(
        order,
        [
            "patchback.audio.ProvenanceMissing",
            "patchback.audio.LicenceMissing",
            "patchback.audio.UnsupportedSampleRate",
            "patchback.audio.UnsupportedSampleFormat",
            "patchback.audio.UnsupportedChannelCount",
            "patchback.audio.EmptyAudio",
            "patchback.audio.TruncatedRiff",
            "patchback.audio.CorruptRiffChunk",
            "patchback.audio.RenderNotFound",
            "patchback.audio.InputTooLong",
            "patchback.audio.NonFiniteSamples",
        ]
    );
    let mut emitted: Vec<&str> = Vec::new();
    let mut not_emitted: Vec<&str> = Vec::new();
    for error in order {
        let list = if WavError::DOMAIN_ERRORS.contains(&error) {
            &mut emitted
        } else {
            &mut not_emitted
        };
        if !list.contains(&error) {
            list.push(error);
        }
    }
    // The crate holds no renders, so it cannot tell whether a named render exists.
    assert_eq!(not_emitted, ["patchback.audio.RenderNotFound"]);
    assert_eq!(emitted, IMPORT_WAV_PRECEDENCE);
    for error in IMPORT_WAV_PRECEDENCE {
        assert!(WavError::DOMAIN_ERRORS.contains(&error), "{error}");
    }
}

#[test]
fn every_import_refusal_ranks_at_its_place_in_the_precedence() {
    for error in WavError::examples() {
        let rank = error.precedence();
        if error.domain_error() == "patchback.audio.SampleOutOfRange" {
            assert_eq!(rank, IMPORT_WAV_PRECEDENCE.len(), "export only");
        } else {
            assert_eq!(IMPORT_WAV_PRECEDENCE[rank], error.domain_error());
        }
    }
    // Ranked one below the other, in the ESS order of the precedence test above.
    let ranked = [
        WavError::ProvenanceMissing,
        WavError::LicenceMissing,
        WavError::UnsupportedSampleRate {
            sample_rate_hz: 44_100,
        },
        WavError::UnsupportedSampleFormat {
            format_tag: 1,
            bits_per_sample: 24,
        },
        WavError::UnsupportedChannelCount { channel_count: 3 },
        WavError::EmptyAudio,
        WavError::Truncated(Truncation::Header),
        WavError::CorruptChunk(Corruption::MissingFmt),
        WavError::InputTooLong { frame_count: 0 },
        WavError::NonFiniteSample {
            frame: 0,
            channel: 0,
        },
    ];
    let ranks: Vec<usize> = ranked.iter().map(WavError::precedence).collect();
    assert_eq!(ranks, (0..ranked.len()).collect::<Vec<_>>(), "{ranked:?}");
}

// The existing reader is unchanged.

#[test]
fn a_clip_read_without_an_import_request_records_no_origin() {
    let bytes = note_wav();
    let path = write("plain.wav", &bytes);
    for clip in [decode(&bytes).unwrap(), read_wav(&path).unwrap()] {
        assert_eq!(clip.origin(), None);
        assert_eq!(clip.provenance(), None);
        assert_eq!(clip.licence(), None);
        assert_eq!(clip.render_id(), None);
        assert_eq!(clip.frame_count(), 72_000);
        assert_eq!(clip.source_format(), Some(WavSampleFormat::Pcm16));
    }
    let imported = external(Some(PROVENANCE), Some(LICENCE))
        .decode(&bytes)
        .unwrap();
    let plain = decode(&bytes).unwrap();
    assert_eq!(imported.samples(), plain.samples());
    assert_eq!(
        imported.source_channel_count(),
        plain.source_channel_count()
    );
    assert_eq!(imported.downmixed(), plain.downmixed());
    assert_eq!(imported.source_format(), plain.source_format());
    assert_ne!(imported, plain, "the import record is part of the clip");
}

#[test]
fn read_wav_refuses_exactly_as_before() {
    let mut fmt = fmt_body(1, 1, 48_000, 16);
    fmt[8..12].copy_from_slice(&1u32.to_le_bytes());
    let cases = [
        (
            riff(&[
                chunk(b"fmt ", &fmt_body(1, 1, 44_100, 16)),
                chunk(b"data", &pcm16_data(&[0])),
            ]),
            WavError::UnsupportedSampleRate {
                sample_rate_hz: 44_100,
            },
        ),
        (pcm16_wav(1, &[]), WavError::EmptyAudio),
        (
            riff(&[chunk(b"fmt ", &fmt), chunk(b"data", &pcm16_data(&[0]))]),
            WavError::CorruptChunk(Corruption::ByteRate { declared: 1 }),
        ),
        (
            f32_wav(1, &[0.0, f32::INFINITY]),
            WavError::NonFiniteSample {
                frame: 1,
                channel: 0,
            },
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(decode(&bytes), Err(expected));
        let path = write("plain-refused.wav", &bytes);
        match read_wav(&path) {
            Err(ReadError::Wav(error)) => assert_eq!(error, expected),
            other => panic!("{other:?}"),
        }
    }
}
