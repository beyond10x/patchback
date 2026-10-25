//! The listening demos: committed voices (`demos/<voice>/voice.patch` and `note.events`, one
//! note each), a committed demo model (`models/demo/`: a `weights/1` artifact, the training
//! split's `normalization.stats` and their `PROVENANCE`), the numbers `demos fit` derives from
//! them (`website/data/demos.json`), the audio `demos render` renders from those numbers when the
//! site is built, and the listen page, `website/docs/listen.mdx`.
//!
//! `demos fit` renders each voice's note (the "recording"), analyses it once, and fits it twice
//! with `FitConfig::default()` but a wall-time bound of [`MAX_WALL_TIME_MS`], so that no fit
//! stops on the clock and the numbers do not depend on the machine: from the default patch
//! (`patchback_fit::fit`) and from the demo model's prediction (`patchback_fit::fit_request`,
//! wired as `patchback fit --weights --normalization` wires it). It records the SHA-256 of every
//! input and of every take's 16-bit samples, so `generate` and `demos render` refuse numbers made
//! from other inputs, takes that no longer render as recorded (a later engine, mapping or
//! export change, or an edited vector) and physical values the page would show beside a take
//! that its vector does not map to. The fits of
//! all voices run on their own threads; each fit is deterministic, so the threads change the
//! wall time and nothing else.
//!
//! `demos render` renders four takes per voice as 16-bit PCM WAV, 48 kHz, mono: the recording,
//! the two fits' recovered patches and the clamped prediction, each through `patch_at` and cut or
//! padded to the recording's length as the fit scores a candidate. A take outside [-1, 1] is
//! refused by the PCM16 export, never normalized.
//!
//! `demos train` retrains the demo model as `PROVENANCE` records it (dataset seed and split
//! sizes, steps, batch) with a ModelSpec binary the caller supplies, and says whether the new
//! artifact's checksum equals the committed one. Nothing here downloads ModelSpec.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use patchback_analysis::{
    AnalysisConfig, FEATURES_1_VERSION, NORMALIZATION_STATS_FILE, analyze_clip, read_stats,
    write_stats,
};
use patchback_audio::{AudioClip, WavSampleFormat, encode};
use patchback_core::digest::{hex, sha256};
use patchback_core::parameter::{ParameterUnit, Topology};
use patchback_core::patch::{Patch, compile, parse_patch, validate};
use patchback_core::score::{NoteSequence, parse_events};
use patchback_dataset::{Generator, SplitCounts, SplitRole, dense, training_stats};
use patchback_dsp::{RenderConfig, render};
use patchback_fit::{
    FitBounds, FitConfig, FitOutcome, FitRequest, PATCHBACK_VERSION, SeedInputs, StopReason,
    check_seed_inputs, fit, fit_request, patch_at,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, MODEL_NAME, ModelSource, ModelSourceState, ModelSpec,
    ModelSpecPin, Predictor, RECOMMENDED_FILE_NAME, TrainingRunState, WeightArtifact,
};
use serde_json::{Value, json};

/// The demo numbers, relative to the repository root; written only by `demos fit`.
pub const DATA: &str = "website/data/demos.json";
/// The listen page's file name under `website/docs`.
pub const PAGE: &str = "listen.mdx";
/// The committed demo voices, one directory each.
pub const VOICES_DIR: &str = "demos";
/// The committed demo model.
pub const MODEL_DIR: &str = "models/demo";
pub const WEIGHTS_FILE: &str = "regressor.weights";
pub const STATS_FILE: &str = "normalization.stats";
pub const PROVENANCE_FILE: &str = "PROVENANCE";

/// One take of a voice: its file stem and its label on the page.
pub struct Take {
    pub file: &'static str,
    pub label: &'static str,
}

pub const TAKES: [Take; 4] = [
    Take {
        file: "recording",
        label: "recording",
    },
    Take {
        file: "default-fit",
        label: "default-start fit",
    },
    Take {
        file: "seeded-fit",
        label: "regressor-seeded fit",
    },
    Take {
        file: "prediction",
        label: "prediction",
    },
];

/// A demo voice: its directory under [`VOICES_DIR`] and one line on its character.
pub struct DemoVoice {
    pub name: &'static str,
    pub character: &'static str,
}

/// The demo voices, in page order. Each must stay under two seconds with its release and inside
/// [-1, 1] in every take (Da12-22, Da12-25); one that clips is replaced, not normalized.
pub const VOICES: &[DemoVoice] = &[
    DemoVoice {
        name: "pluck",
        character: "A short, bright note that decays fast.",
    },
    DemoVoice {
        name: "pad",
        character: "A slow swell that holds and fades out.",
    },
    DemoVoice {
        name: "bass",
        character: "A low note through heavy drive and a resonant filter.",
    },
    DemoVoice {
        name: "lead",
        character: "A bright, resonant note in the middle register.",
    },
];

/// The format of [`DATA`].
pub const FORMAT: &str = "patchback-demos/1";
/// The format line of [`PROVENANCE_FILE`].
pub const PROVENANCE_HEADER: &str = "patchback-demo-provenance 1";
/// The wall-time bound of every demo fit, in milliseconds: far past any demo fit, so none stops
/// on the clock (Da12-22). `FitConfig::default()` has 60,000.
pub const MAX_WALL_TIME_MS: i64 = 600_000;
/// The largest [`DATA`] `demos fit` writes, in bytes.
pub const MAX_DATA_BYTES: usize = 64 * 1024;
/// The sample rate of every take.
const SAMPLE_RATE_HZ: f64 = 48_000.0;
/// The model source `demos train` trains, relative to the repository root.
const MODEL_SOURCE: &str = "models/patch_regressor.mspec";
/// Where a voice's inputs are named in [`DATA`], and the file each is read from.
const INPUTS: [&str; 4] = ["patch", "notes", "weights", "statistics"];
/// The key of a voice's take digests in [`DATA`]: the SHA-256 of each take's 16-bit PCM sample
/// bytes, by take file name.
pub const TAKE_DIGESTS: &str = "pcm16Sha256";

/// One rendered take.
pub struct TakeAudio {
    pub voice: String,
    pub take: &'static str,
    pub samples: Vec<f32>,
    pub wav: Vec<u8>,
}

/// The path, relative to the repository root, of `input` ([`INPUTS`]) of `voice`.
fn input_path(voice: &str, input: &str) -> String {
    match input {
        "patch" => format!("{VOICES_DIR}/{voice}/voice.patch"),
        "notes" => format!("{VOICES_DIR}/{voice}/note.events"),
        "weights" => format!("{MODEL_DIR}/{WEIGHTS_FILE}"),
        _ => format!("{MODEL_DIR}/{STATS_FILE}"),
    }
}

fn read_bytes(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    fs::read(root.join(relative)).map_err(crate::io(relative))
}

fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    fs::read_to_string(root.join(relative)).map_err(crate::io(relative))
}

/// The SHA-256 of every input of `voice`, by [`INPUTS`] key, with its path.
pub fn input_digests(
    root: &Path,
    voice: &str,
) -> Result<Vec<(&'static str, String, String)>, String> {
    INPUTS
        .iter()
        .map(|input| {
            let path = input_path(voice, input);
            let digest = hex(&sha256(&read_bytes(root, &path)?));
            Ok((*input, path, digest))
        })
        .collect()
}

/// [`DATA`] of the repository at `root`, parsed, of format [`FORMAT`].
pub fn read_data(root: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(root.join(DATA))
        .map_err(|e| format!("{DATA}: {e}; run `task docs-demos` to write it"))?;
    let data: Value =
        serde_json::from_str(&text).map_err(|e| format!("{DATA} is not JSON: {e}"))?;
    if data["format"] != FORMAT {
        return Err(format!(
            "{DATA} has format {}, not {FORMAT}; run `task docs-demos`",
            data["format"]
        ));
    }
    Ok(data)
}

/// The voices of `data`.
fn voices(data: &Value) -> Result<&Vec<Value>, String> {
    data["voices"]
        .as_array()
        .ok_or_else(|| format!("{DATA} holds no voices"))
}

/// Refuses `data` unless it holds [`VOICES`] in order, each made from the inputs committed under
/// `root`: every input's path and SHA-256 must be the file's.
pub fn check_inputs(root: &Path, data: &Value) -> Result<(), String> {
    let names: Vec<&str> = voices(data)?
        .iter()
        .map(|voice| voice["name"].as_str().unwrap_or_default())
        .collect();
    let expected: Vec<&str> = VOICES.iter().map(|voice| voice.name).collect();
    if names != expected {
        return Err(format!(
            "{DATA} holds the voices {names:?}, not {expected:?}; run `task docs-demos` to regenerate it"
        ));
    }
    let mut changed = Vec::new();
    for voice in voices(data)? {
        let name = voice["name"].as_str().unwrap_or_default();
        for (input, path, digest) in input_digests(root, name)? {
            let recorded = &voice["inputs"][input];
            if recorded["path"] != path.as_str() {
                changed.push(format!(
                    "{path} ({DATA} names {} instead)",
                    recorded["path"]
                ));
            } else if recorded["sha256"] != digest.as_str() {
                changed.push(format!(
                    "{path} (sha256 {digest}; {DATA} records {})",
                    recorded["sha256"]
                ));
            }
        }
    }
    changed.sort();
    changed.dedup();
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{DATA} was made from other demo inputs than the committed ones: {}; run `task docs-demos` to regenerate it",
            changed.join(", ")
        ))
    }
}

/// A committed voice: its patch and its note sequence, parsed.
struct Voice {
    patch: Patch,
    notes: NoteSequence,
}

fn read_voice(root: &Path, name: &str) -> Result<Voice, String> {
    let path = input_path(name, "patch");
    let patch = parse_patch(&read_text(root, &path)?).map_err(|e| format!("{path}: {e}"))?;
    let path = input_path(name, "notes");
    let notes = parse_events(&read_text(root, &path)?).map_err(|e| format!("{path}: {e}"))?;
    if notes.notes.len() != 1 {
        return Err(format!(
            "{path} holds {} notes; a demo voice plays one",
            notes.notes.len()
        ));
    }
    Ok(Voice { patch, notes })
}

/// Renders `patch` driven by `notes` at its natural length, or cut or padded to `frames`.
fn render_patch(
    patch: &Patch,
    notes: &NoteSequence,
    frames: Option<usize>,
) -> Result<(Vec<f32>, u64), String> {
    let validated = validate(patch).map_err(|e| format!("{}: {e}", e.spec_error()))?;
    let rendered =
        render(&compile(&validated), notes, &RenderConfig::default()).map_err(|e| e.to_string())?;
    let mut samples = rendered.samples;
    if let Some(frames) = frames {
        samples.resize(frames, 0.0);
    }
    Ok((samples, rendered.report.limited_samples))
}

/// The physical value of every parameter of `topology` in `patch`, in contract order.
fn physical(patch: &Patch, topology: &Topology) -> Result<Vec<f64>, String> {
    topology
        .contracts()
        .iter()
        .map(|contract| {
            patch
                .nodes
                .iter()
                .flat_map(|node| &node.parameters)
                .find(|parameter| parameter.name == contract.path)
                .map(|parameter| parameter.value)
                .ok_or_else(|| format!("the patch sets no {}", contract.path))
        })
        .collect()
}

/// The physical values of the normalized point `normalized`, as the fit renders it.
fn physical_at(topology: &Topology, normalized: &[f64]) -> Result<Vec<f64>, String> {
    physical(
        &patch_at(topology, normalized).map_err(|e| e.to_string())?,
        topology,
    )
}

/// A number as the page and the JSON show an `f32` input: its shortest decimal.
fn short(value: f32) -> f64 {
    value.to_string().parse().unwrap_or(f64::from(value))
}

fn unit(unit: ParameterUnit) -> &'static str {
    match unit {
        ParameterUnit::Seconds => "s",
        ParameterUnit::Hertz => "Hz",
        ParameterUnit::Decibels => "dB",
        ParameterUnit::LinearAmplitude | ParameterUnit::Dimensionless => "",
    }
}

/// One fit's numbers.
fn fit_record(outcome: &FitOutcome, topology: &Topology) -> Result<Value, String> {
    let run = &outcome.run;
    Ok(json!({
        "init": outcome.init.record(),
        "initialLoss": run.initial_loss(),
        "bestLoss": run.best_loss(),
        "evaluations": run.evaluations(),
        "restarts": outcome.restarts,
        "stopReason": run.stop_reason().map(StopReason::name),
        "normalized": outcome.normalized,
        "physical": physical(&outcome.patch, topology)?,
    }))
}

/// The demo model, read once.
struct Model {
    predictor: Predictor,
    statistics: patchback_analysis::NormalizationTable,
    statistics_sha256: [u8; 32],
}

fn read_model(root: &Path) -> Result<Model, String> {
    let path = format!("{MODEL_DIR}/{WEIGHTS_FILE}");
    let predictor = Predictor::load(&root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
    let path = format!("{MODEL_DIR}/{STATS_FILE}");
    let bytes = read_bytes(root, &path)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let statistics = read_stats(text).map_err(|e| format!("{path}: {e}"))?;
    Ok(Model {
        predictor,
        statistics,
        statistics_sha256: sha256(&bytes),
    })
}

/// The demo fit configuration: the default, with [`MAX_WALL_TIME_MS`].
fn fit_config() -> FitConfig {
    FitConfig {
        bounds: FitBounds {
            max_wall_time_ms: MAX_WALL_TIME_MS,
            ..FitBounds::default()
        },
        ..FitConfig::default()
    }
}

/// Fits the voice `name` from the default start and from the model's prediction, on two threads,
/// and returns its record in [`DATA`] and the two fits' wall times in milliseconds.
fn fit_voice(root: &Path, name: &str, model: &Model) -> Result<(Value, [u64; 2]), String> {
    let voice = read_voice(root, name)?;
    let topology = Topology::subtractive_mono();
    let (samples, limited) = render_patch(&voice.patch, &voice.notes, None)?;
    let frames = samples.len();
    let reference = AudioClip::from_mono(samples).map_err(|e| format!("{name}: {e}"))?;
    let analysis = analyze_clip(&reference, &AnalysisConfig::default())
        .map_err(|e| format!("{name}: the recording's analysis: {e}"))?;
    let config = fit_config();
    let seed = SeedInputs {
        predictor: model.predictor.clone(),
        normalization: model.statistics.clone(),
        normalization_sha256: model.statistics_sha256,
    };
    check_seed_inputs(&seed).map_err(|e| format!("{name}: {e}"))?;
    let (default, seeded) = std::thread::scope(|scope| {
        let default = scope.spawn(|| fit(&reference, &voice.notes, &topology, &config));
        let seeded = scope.spawn(|| {
            fit_request(FitRequest {
                reference: &reference,
                notes: &voice.notes,
                topology: &topology,
                config: &config,
                analysis: Some(&analysis),
                seed: Some(seed),
            })
        });
        (default.join(), seeded.join())
    });
    let joined = |result: std::thread::Result<Result<FitOutcome, patchback_fit::FitError>>,
                  which: &str| {
        result
            .map_err(|_| format!("{name}: the {which} fit panicked"))?
            .map_err(|e| format!("{name}: the {which} fit: {e}"))
            .and_then(|outcome| {
                if outcome.run.stop_reason() == Some(StopReason::TimeBudgetExhausted) {
                    Err(format!(
                        "{name}: the {which} fit stopped on its {MAX_WALL_TIME_MS} ms wall-time bound"
                    ))
                } else {
                    Ok(outcome)
                }
            })
    };
    let default = joined(default, "default-start")?;
    let seeded = joined(seeded, "regressor-seeded")?;
    let start = seeded
        .seeded
        .as_ref()
        .ok_or_else(|| format!("{name}: the seeded fit recorded no prediction"))?;
    let prediction = start.point();
    let voice_physical = physical(&voice.patch, &topology).map_err(|e| format!("{name}: {e}"))?;
    let voice_normalized = topology
        .contracts()
        .iter()
        .zip(&voice_physical)
        .map(|(contract, value)| contract.to_normalized(*value).map_err(|e| e.to_string()))
        .collect::<Result<Vec<f64>, String>>()?;
    let note = &voice.notes.notes[0];
    let mut inputs = serde_json::Map::new();
    for (input, path, digest) in input_digests(root, name)? {
        inputs.insert(input.to_owned(), json!({"path": path, "sha256": digest}));
    }
    let record = json!({
        "name": name,
        "inputs": inputs,
        "note": {
            "midi": note.midi_note,
            "velocity": short(note.velocity),
            "startSample": note.start_sample,
            "endSample": note.end_sample,
        },
        "recording": {
            "frames": frames,
            "seconds": frames as f64 / SAMPLE_RATE_HZ,
            "limitedSamples": limited,
        },
        "voice": {"normalized": voice_normalized, "physical": voice_physical},
        "defaultFit": fit_record(&default, &topology)?,
        "seededFit": fit_record(&seeded, &topology)?,
        "prediction": {
            "normalized": prediction,
            "physical": physical_at(&topology, &prediction)?,
            "clamped": start.prediction.clamped,
        },
    });
    let wall = |outcome: &FitOutcome| outcome.run.wall_time_ms().unwrap_or_default();
    Ok((record, [wall(&default), wall(&seeded)]))
}

/// The default-start and seeded fits' wall times of each voice, in milliseconds.
type FitTimes = Vec<(String, [u64; 2])>;

/// The numbers of every voice, the fits of all voices at once, and each fit's wall time.
pub fn fit_all(root: &Path) -> Result<(Value, FitTimes), String> {
    let model = read_model(root)?;
    let artifact = model.predictor.artifact();
    let results: Vec<Result<(Value, [u64; 2]), String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = VOICES
            .iter()
            .map(|voice| scope.spawn(|| fit_voice(root, voice.name, &model)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("a voice's fit panicked".to_owned()))
            })
            .collect()
    });
    let mut records = Vec::new();
    let mut times = Vec::new();
    for (voice, result) in VOICES.iter().zip(results) {
        let (record, wall) = result?;
        records.push(record);
        times.push((voice.name.to_owned(), wall));
    }
    let topology = Topology::subtractive_mono();
    let config = fit_config();
    let mut data = json!({
        "format": FORMAT,
        "generator": "patchback-docs demos fit",
        "patchback": PATCHBACK_VERSION,
        "topology": topology.name(),
        "parameters": topology.contracts().iter().map(|c| json!({"name": c.path, "unit": unit(c.unit)})).collect::<Vec<_>>(),
        "fit": {
            "seed": config.seed,
            "maxRenders": config.bounds.max_renders,
            "maxRestarts": config.bounds.max_restarts,
            "maxWallTimeMs": config.bounds.max_wall_time_ms,
            "improvementThreshold": config.bounds.improvement_threshold,
            "blockSizeFrames": config.render.block_size_frames,
        },
        "model": {
            "checksum": artifact.checksum(),
            "modelspec": artifact.modelspec_version(),
            "modelspecCommit": artifact.modelspec_commit(),
            "featureContract": artifact.feature_contract(),
        },
        "voices": records,
    });
    let takes = render_unchecked(root, &data)?;
    for take in &takes {
        let digest = pcm_sha256(&take.wav)?;
        let voice = data["voices"]
            .as_array_mut()
            .and_then(|voices| voices.iter_mut().find(|v| v["name"] == take.voice.as_str()))
            .ok_or_else(|| format!("{}: no record", take.voice))?;
        if !voice[TAKE_DIGESTS].is_object() {
            voice[TAKE_DIGESTS] = json!({});
        }
        voice[TAKE_DIGESTS][take.take] = Value::from(digest);
    }
    Ok((data, times))
}

/// The normalized vector at `value`, every element a number.
fn vector(value: &Value, what: &str) -> Result<Vec<f64>, String> {
    value
        .as_array()
        .and_then(|values| values.iter().map(Value::as_f64).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("{DATA}: {what} is not a vector of numbers"))
}

/// The vectors `demos render` reads back, by voice.
fn rendered_vectors(data: &Value) -> Result<Vec<[Vec<f64>; 3]>, String> {
    voices(data)?
        .iter()
        .map(|voice| {
            Ok([
                vector(&voice["defaultFit"]["normalized"], "defaultFit.normalized")?,
                vector(&voice["seededFit"]["normalized"], "seededFit.normalized")?,
                vector(&voice["prediction"]["normalized"], "prediction.normalized")?,
            ])
        })
        .collect()
}

/// The text of [`DATA`]: pretty JSON, refused past [`MAX_DATA_BYTES`] or when a vector would
/// not read back bit for bit.
pub fn data_text(data: &Value) -> Result<String, String> {
    let text = crate::pretty(data)?;
    if text.len() > MAX_DATA_BYTES {
        return Err(format!(
            "{DATA} would hold {} bytes, more than {MAX_DATA_BYTES}",
            text.len()
        ));
    }
    let read_back: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let bits = |vectors: Vec<[Vec<f64>; 3]>| -> Vec<Vec<u64>> {
        vectors
            .iter()
            .flatten()
            .map(|v| v.iter().map(|x| x.to_bits()).collect())
            .collect()
    };
    if bits(rendered_vectors(data)?) != bits(rendered_vectors(&read_back)?) {
        return Err(format!(
            "{DATA}: a recovered vector does not read back bit for bit"
        ));
    }
    Ok(text)
}

/// The SHA-256 of the sample bytes of `wav`: its `data` chunk, the 16-bit PCM samples.
pub fn pcm_sha256(wav: &[u8]) -> Result<String, String> {
    let mut at = 12;
    while at + 8 <= wav.len() {
        let id = &wav[at..at + 4];
        let size = u32::from_le_bytes([wav[at + 4], wav[at + 5], wav[at + 6], wav[at + 7]]);
        let start = at + 8;
        let end = start + size as usize;
        if end > wav.len() {
            break;
        }
        if id == b"data" {
            return Ok(hex(&sha256(&wav[start..end])));
        }
        at = end + end % 2;
    }
    Err("a rendered take holds no complete data chunk".to_owned())
}

/// Renders the four takes of every voice of `data` from the inputs committed under `root`,
/// refusing `data` made from other inputs or whose takes no longer render to the samples it
/// records ([`TAKE_DIGESTS`]) or whose physical values its vectors and voices do not map to
/// ([`check_physical`]).
pub fn render_takes(root: &Path, data: &Value) -> Result<Vec<TakeAudio>, String> {
    let takes = render_unchecked(root, data)?;
    let mut changed = Vec::new();
    for take in &takes {
        let digest = pcm_sha256(&take.wav)?;
        let recorded = voices(data)?
            .iter()
            .find(|voice| voice["name"] == take.voice.as_str())
            .map(|voice| &voice[TAKE_DIGESTS][take.take]);
        if recorded.and_then(Value::as_str) != Some(digest.as_str()) {
            changed.push(format!(
                "{}/{} (sha256 {digest}; {DATA} records {})",
                take.voice,
                take.take,
                recorded.unwrap_or(&Value::Null)
            ));
        }
    }
    if changed.is_empty() {
        check_physical(root, data)?;
        Ok(takes)
    } else {
        Err(format!(
            "the demo takes no longer render to the samples {DATA} records: {}; run `task docs-demos` to regenerate it",
            changed.join(", ")
        ))
    }
}

/// The fields of a voice in [`DATA`] whose `physical` values the page shows, with the normalized
/// vector each maps from (`None`: the voice's own patch).
const PHYSICAL_FIELDS: [(&str, Option<usize>); 4] = [
    ("voice", None),
    ("defaultFit", Some(0)),
    ("seededFit", Some(1)),
    ("prediction", Some(2)),
];

/// Refuses `data` whose recorded `physical` values are not, bit for bit, the ones `demos fit`
/// derives: `voice.patch`'s parameters for `voice`, and `patch_at` of the recorded vector for
/// the two fits and the prediction. Names the voice, the field and the parameters.
fn check_physical(root: &Path, data: &Value) -> Result<(), String> {
    let topology = Topology::subtractive_mono();
    let contracts = topology.contracts();
    let mut changed = Vec::new();
    for (record, vectors) in voices(data)?.iter().zip(rendered_vectors(data)?) {
        let name = record["name"].as_str().unwrap_or_default();
        let voice = read_voice(root, name)?;
        for (field, vector) in PHYSICAL_FIELDS {
            let expected = match vector {
                None => physical(&voice.patch, &topology),
                Some(index) => physical_at(&topology, &vectors[index]),
            }
            .map_err(|e| format!("{name} {field}: {e}"))?;
            let recorded = record[field]["physical"].as_array();
            let mut differs: Vec<String> = contracts
                .iter()
                .zip(&expected)
                .enumerate()
                .filter_map(|(index, (contract, value))| {
                    let stored = recorded.and_then(|r| r.get(index)).and_then(Value::as_f64);
                    (stored.map(f64::to_bits) != Some(value.to_bits())).then(|| {
                        format!(
                            "{} maps to {value}, recorded {}",
                            contract.path,
                            stored.map_or_else(|| "nothing".to_owned(), |s| s.to_string())
                        )
                    })
                })
                .collect();
            let length = recorded.map_or(0, Vec::len);
            if length != expected.len() {
                differs.push(format!(
                    "{length} values recorded, {} parameters",
                    expected.len()
                ));
            }
            if !differs.is_empty() {
                changed.push(format!("{name} {field}.physical ({})", differs.join("; ")));
            }
        }
    }
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{DATA} records physical values its vectors and voices do not map to: {}; run `task docs-demos` to regenerate it",
            changed.join(", ")
        ))
    }
}

/// [`render_takes`] without the take digests: what `demos fit` records them from.
fn render_unchecked(root: &Path, data: &Value) -> Result<Vec<TakeAudio>, String> {
    check_inputs(root, data)?;
    let topology = Topology::subtractive_mono();
    let mut takes = Vec::new();
    for (record, vectors) in voices(data)?.iter().zip(rendered_vectors(data)?) {
        let name = record["name"].as_str().unwrap_or_default();
        let voice = read_voice(root, name)?;
        let (recording, _) = render_patch(&voice.patch, &voice.notes, None)?;
        let frames = recording.len();
        if record["recording"]["frames"].as_u64() != Some(frames as u64) {
            return Err(format!(
                "{name}: the recording renders {frames} frames; {DATA} records {}; run `task docs-demos` to regenerate it",
                record["recording"]["frames"]
            ));
        }
        let mut samples = vec![recording];
        for normalized in &vectors {
            let patch = patch_at(&topology, normalized).map_err(|e| format!("{name}: {e}"))?;
            samples.push(render_patch(&patch, &voice.notes, Some(frames))?.0);
        }
        for (take, samples) in TAKES.iter().zip(samples) {
            let clip = AudioClip::from_mono(samples.clone())
                .map_err(|e| format!("{name}/{}: {e}", take.file))?;
            let wav = encode(&clip, WavSampleFormat::Pcm16).map_err(|e| {
                format!(
                    "{name}/{}: {e}; a demo voice that clips is replaced, not normalized",
                    take.file
                )
            })?;
            takes.push(TakeAudio {
                voice: name.to_owned(),
                take: take.file,
                samples,
                wav,
            });
        }
    }
    Ok(takes)
}

/// Writes every take to `<out>/<voice>/<take>.wav`, replacing what is there.
pub fn write_audio(out: &Path, takes: &[TakeAudio]) -> Result<(), String> {
    for take in takes {
        let dir = out.join(&take.voice);
        fs::create_dir_all(&dir).map_err(crate::io(dir.display()))?;
        let path = dir.join(format!("{}.wav", take.take));
        fs::write(&path, &take.wav).map_err(crate::io(path.display()))?;
    }
    Ok(())
}

/// A physical value with its unit, as the page shows it.
fn quantity(value: f64, unit: &str) -> String {
    match unit {
        "s" => format!("{value:.3} s"),
        "Hz" => format!("{value:.0} Hz"),
        "dB" => format!("{value:.1} dB"),
        _ => format!("{value:.2}"),
    }
}

/// The name of MIDI note `midi`, C4 = 60.
fn note_name(midi: u64) -> String {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!("{}{}", NAMES[(midi % 12) as usize], midi as i64 / 12 - 1)
}

/// `n` with a comma between groups of three digits: 2,000.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn loss(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "—".to_owned(), |v| format!("{v:.4}"))
}

/// The listen page of `data`.
pub fn page(data: &Value) -> Result<String, String> {
    let parameters = data["parameters"]
        .as_array()
        .ok_or_else(|| format!("{DATA} names no parameters"))?;
    let fit = &data["fit"];
    let mut out = format!(
        "---\n{header}\ntitle: Listen\nsidebar_position: 5\ndescription: \"Four synth voices, what two fits recovered from each, and the regressor's prediction, to play in the browser.\"\ncustom_edit_url: null\n---\n\n\
         {mdx}\n\n\
         # Listen\n\n\
         For each voice you hear the engine's render of a patch (the recording), the patches two \
         fits recovered from that recording alone, one started from the default patch and one \
         from the regressor's prediction, and that prediction itself before any search; the audio \
         is rendered when the site is built, from the committed voices and the recovered \
         parameters below.\n\n\
         Each fit ran with `patchback fit`'s defaults (seed {seed}, at most {renders} renders and \
         {restarts} restarts) but a wall-time bound of {wall} s, which none reached. The regressor \
         is a demo model trained with ModelSpec {modelspec} on a generated dataset; its \
         prediction is clamped to the parameter ranges, and the label says how many of its \
         outputs were. A loss of 0 is identical audio. [Fit a recording](guides/fit-a-recording.md) \
         shows the same fit on your own file.\n",
        header = crate::FRONT_MATTER_HEADER,
        mdx = crate::reference::MDX_HEADER,
        seed = fit["seed"],
        renders = grouped(fit["maxRenders"].as_u64().unwrap_or_default()),
        restarts = fit["maxRestarts"],
        wall = grouped(fit["maxWallTimeMs"].as_u64().unwrap_or_default() / 1000),
        modelspec = data["model"]["modelspec"].as_str().unwrap_or_default(),
    );
    for (voice, record) in VOICES.iter().zip(voices(data)?) {
        let name = voice.name;
        let note = &record["note"];
        let midi = note["midi"].as_u64().unwrap_or_default();
        let start = note["startSample"].as_f64().unwrap_or_default();
        let end = note["endSample"].as_f64().unwrap_or_default();
        let mut heading = name.to_owned();
        heading[..1].make_ascii_uppercase();
        out.push_str(&format!(
            "\n## {heading}\n\n{} MIDI {midi} ({}), velocity {}, a {:.2} s note; {:.2} s with its release.\n",
            voice.character,
            note_name(midi),
            note["velocity"],
            (end - start) / SAMPLE_RATE_HZ,
            record["recording"]["seconds"].as_f64().unwrap_or_default(),
        ));
        let clamped = record["prediction"]["clamped"].as_u64().unwrap_or_default();
        for take in &TAKES {
            let label = if take.file == "prediction" {
                format!(
                    "**{}**, {clamped} of {} outputs clamped",
                    take.label,
                    parameters.len()
                )
            } else {
                format!("**{}**", take.label)
            };
            out.push_str(&format!(
                "\n{label}\n\n<audio controls preload=\"none\" src={{require('@site/demo-audio/{name}/{}.wav').default}} />\n",
                take.file
            ));
        }
        out.push_str(
            "\n| Parameter | Voice | Default-start fit | Regressor-seeded fit | Prediction |\n|---|---|---|---|---|\n",
        );
        for (index, parameter) in parameters.iter().enumerate() {
            let unit = parameter["unit"].as_str().unwrap_or_default();
            let cell = |path: &str| {
                record[path]["physical"][index]
                    .as_f64()
                    .map_or_else(|| "—".to_owned(), |v| quantity(v, unit))
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                parameter["name"].as_str().unwrap_or_default(),
                cell("voice"),
                cell("defaultFit"),
                cell("seededFit"),
                cell("prediction"),
            ));
        }
        out.push_str("\n| Fit | Start loss | Best loss | Evaluations |\n|---|---|---|---|\n");
        for (label, key) in [
            ("Default start", "defaultFit"),
            ("Regressor-seeded", "seededFit"),
        ] {
            let fit = &record[key];
            out.push_str(&format!(
                "| {label} | {} | {} | {} |\n",
                loss(&fit["initialLoss"]),
                loss(&fit["bestLoss"]),
                fit["evaluations"]
            ));
        }
    }
    out.push_str(
        "\n## The takes' digests\n\n\
         The site build renders every take again from the numbers above, and the documentation \
         check refuses a take whose samples differ from the ones recorded here: the SHA-256 of \
         each take's 16-bit samples, the WAV file's `data` chunk.\n\n\
         | Voice | Take | SHA-256 of the samples |\n|---|---|---|\n",
    );
    for (voice, record) in VOICES.iter().zip(voices(data)?) {
        for take in &TAKES {
            out.push_str(&format!(
                "| {} | {} | `{}` |\n",
                voice.name,
                take.label,
                record[TAKE_DIGESTS][take.file].as_str().unwrap_or("—")
            ));
        }
    }
    Ok(out)
}

/// The `key value` lines of a [`PROVENANCE_FILE`] after its [`PROVENANCE_HEADER`]; a key given
/// twice is refused.
pub fn read_provenance(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut lines = text.lines();
    if lines.next() != Some(PROVENANCE_HEADER) {
        return Err(format!(
            "{PROVENANCE_FILE} does not open with `{PROVENANCE_HEADER}`"
        ));
    }
    let mut values = BTreeMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (key, value) = line
            .split_once(' ')
            .ok_or_else(|| format!("{PROVENANCE_FILE}: `{line}` is not `key value`"))?;
        if values.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("{PROVENANCE_FILE}: {key} is given twice"));
        }
    }
    Ok(values)
}

/// The listen page of the repository at `root`, refused while [`DATA`] was made from other
/// inputs than the committed ones or its takes no longer render to the samples it records.
pub fn output(root: &Path) -> Result<crate::Output, String> {
    let data = read_data(root)?;
    render_takes(root, &data)?;
    Ok(crate::Output {
        dir: crate::DOCS,
        files: crate::Pages::from([(PAGE.to_owned(), page(&data)?)]),
        exclusive: false,
    })
}

/// `demos fit`: writes [`DATA`] to `out`.
pub fn run_fit(root: &Path, out: &Path) -> Result<(), String> {
    let started = Instant::now();
    let (data, times) = fit_all(root)?;
    let text = data_text(&data)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(crate::io(parent.display()))?;
    }
    fs::write(out, &text).map_err(crate::io(out.display()))?;
    for (record, (name, [default_ms, seeded_ms])) in voices(&data)?.iter().zip(&times) {
        let summary = |key: &str| {
            let fit = &record[key];
            format!(
                "{} -> {} in {} evaluations",
                loss(&fit["initialLoss"]),
                loss(&fit["bestLoss"]),
                fit["evaluations"]
            )
        };
        println!(
            "{name}: default {} ({default_ms} ms); seeded {} ({seeded_ms} ms); {} of 8 predictions clamped",
            summary("defaultFit"),
            summary("seededFit"),
            record["prediction"]["clamped"]
        );
    }
    println!(
        "{}: wrote {} bytes; demos fit took {} ms",
        out.display(),
        text.len(),
        started.elapsed().as_millis()
    );
    Ok(())
}

/// `demos render`: writes every take under `out`.
pub fn run_render(root: &Path, out: &Path) -> Result<(), String> {
    let started = Instant::now();
    let data = read_data(root)?;
    let takes = render_takes(root, &data)?;
    write_audio(out, &takes)?;
    for take in &takes {
        let peak = take.samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
        println!(
            "{}/{}/{}.wav: {} frames, {:.3} s, peak {peak:.4}",
            out.display(),
            take.voice,
            take.take,
            take.samples.len(),
            take.samples.len() as f64 / SAMPLE_RATE_HZ
        );
    }
    println!(
        "{}: rendered {} takes in {} ms",
        out.display(),
        takes.len(),
        started.elapsed().as_millis()
    );
    Ok(())
}

/// A `PROVENANCE` value as a number.
fn provenance_number(provenance: &BTreeMap<String, String>, key: &str) -> Result<u64, String> {
    provenance
        .get(key)
        .ok_or_else(|| format!("{PROVENANCE_FILE} has no {key}"))?
        .parse()
        .map_err(|e| format!("{PROVENANCE_FILE}: {key}: {e}"))
}

/// `demos train`: retrains the demo model as its provenance records it and writes the new
/// artifact to `<out>/regressor.weights`. The dataset is generated into `dataset` when that is
/// missing or empty; otherwise it is generated again in memory (the dataset store cannot be read
/// back from its files) and every file must equal the one on disk byte for byte.
pub fn run_train(root: &Path, binary: &Path, dataset: &Path, out: &Path) -> Result<(), String> {
    let provenance = read_provenance(&read_text(root, &format!("{MODEL_DIR}/{PROVENANCE_FILE}"))?)?;
    let number = |key: &str| provenance_number(&provenance, key);
    let seed = number("dataset_seed")?;
    let as_count = |key: &str| -> Result<i64, String> {
        i64::try_from(number(key)?).map_err(|e| format!("{PROVENANCE_FILE}: {key}: {e}"))
    };
    let counts = SplitCounts {
        train: as_count("dataset_train")?,
        validation: as_count("dataset_validation")?,
        test: as_count("dataset_test")?,
    };
    let steps = u32::try_from(number("training_steps")?).map_err(|e| e.to_string())?;
    let batch = usize::try_from(number("batch_size")?).map_err(|e| e.to_string())?;
    let committed = provenance
        .get("checksum")
        .ok_or_else(|| format!("{PROVENANCE_FILE} has no checksum"))?;

    let pin = ModelSpecPin::repository().map_err(|e| e.to_string())?;
    if provenance.get("modelspec").map(String::as_str) != Some(pin.version()) {
        println!(
            "note: {PROVENANCE_FILE} names ModelSpec {}; the pin is {}",
            provenance.get("modelspec").map_or("none", String::as_str),
            pin.version()
        );
    }
    let modelspec = ModelSpec::open(binary, &pin).map_err(|e| e.to_string())?;
    let mut source = ModelSource::register(root.join(MODEL_SOURCE), MODEL_NAME, batch, &pin);
    match source.check(&modelspec).map_err(|e| e.to_string())? {
        ModelSourceState::Checked => {}
        state => {
            return Err(format!(
                "{MODEL_SOURCE} is {state:?}: {}",
                source.diagnostic().unwrap_or_default()
            ));
        }
    }

    let reuse = dataset.exists()
        && fs::read_dir(dataset)
            .map_err(crate::io(dataset.display()))?
            .next()
            .is_some();
    fs::create_dir_all(dataset).map_err(crate::io(dataset.display()))?;
    let put = |path: &Path, bytes: &[u8]| -> io::Result<()> {
        if reuse {
            if fs::read(path)? == bytes {
                Ok(())
            } else {
                Err(io::Error::other(
                    "differs from the dataset the provenance names",
                ))
            }
        } else {
            dense::create_new(path, bytes)
        }
    };
    let started = Instant::now();
    let mut generated = Generator::new(seed, counts, dataset.to_path_buf())
        .run()
        .map_err(|e| e.to_string())?;
    let stats = training_stats(&generated).map_err(|e| e.to_string())?;
    let path = dataset.join(NORMALIZATION_STATS_FILE);
    put(&path, write_stats(stats.table()).as_bytes()).map_err(crate::io(path.display()))?;
    for role in SplitRole::ALL {
        let dir = dataset.join(role.dir());
        fs::create_dir_all(&dir).map_err(crate::io(dir.display()))?;
        let split = generated.split_id(role);
        dense::write_split(&mut generated.store, split, &dir, &stats, |path, bytes| {
            put(path, bytes)
        })
        .map_err(|e| e.to_string())?;
    }
    println!(
        "dataset seed {seed}, {}/{}/{}: {} s",
        counts.train,
        counts.validation,
        counts.test,
        started.elapsed().as_secs()
    );

    let mut run =
        patchback_model::TrainingRun::plan(Backend::Cpu, &pin).map_err(|e| e.to_string())?;
    run.bind(&source.trainable().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    run.start(
        &generated.store,
        generated.split_id(SplitRole::Train),
        Some(generated.split_id(SplitRole::Validation)),
        &modelspec,
        steps,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .map_err(|e| e.to_string())?;
    let state = run.finish().map_err(|e| e.to_string())?;
    if state != TrainingRunState::Completed {
        return Err(format!(
            "the training run is {state:?}: {:?}: {}",
            run.failure(),
            run.stderr()
        ));
    }
    println!(
        "training, {steps} steps: {} s; eval loss {:?}",
        started.elapsed().as_secs(),
        run.eval_loss()
    );
    fs::create_dir_all(out).map_err(crate::io(out.display()))?;
    let path: PathBuf = out.join(RECOMMENDED_FILE_NAME);
    if path.exists() {
        return Err(format!(
            "{} already exists; it is never overwritten",
            path.display()
        ));
    }
    let artifact =
        WeightArtifact::export(&run, FEATURES_1_VERSION, &path).map_err(|e| e.to_string())?;
    println!(
        "{}: checksum {}; the committed checksum is {committed}: {}",
        path.display(),
        artifact.checksum(),
        if artifact.checksum() == committed {
            "equal"
        } else {
            "different"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    use patchback_analysis::{FEATURES_1_VERSION, read_stats};
    use patchback_core::digest::{hex, sha256};
    use patchback_model::WeightArtifact;

    /// A committed input and how a test changes it.
    type Change = (&'static str, fn(&mut Vec<u8>));
    /// A take of the demos and how a test changes `demos.json` so that it no longer renders.
    type DataChange = (&'static str, fn(&mut Value));

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchback-docs-demos-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Every file under `dir`, recursively, sorted.
    fn files_under(dir: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for entry in crate::sorted_entries(dir).unwrap() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(files_under(&path));
            } else {
                found.push(path);
            }
        }
        found
    }

    /// Copies the committed demo inputs and numbers of the repository into `root`.
    fn copy_demo_data(root: &Path) {
        let repository = repository();
        let mut files = files_under(&repository.join(VOICES_DIR));
        files.extend(files_under(&repository.join(MODEL_DIR)));
        files.push(repository.join(DATA));
        for file in files {
            let relative = file.strip_prefix(&repository).unwrap();
            fs::create_dir_all(root.join(relative).parent().unwrap()).unwrap();
            fs::copy(&file, root.join(relative)).unwrap();
        }
    }

    #[test]
    fn check_fails_when_a_demo_input_changes_without_regeneration() {
        let root = scratch("inputs");
        copy_demo_data(&root);
        let current = output(&root).unwrap();
        assert_eq!(
            current.files[PAGE],
            output(&repository()).unwrap().files[PAGE]
        );
        let changes: [Change; 4] = [
            ("demos/pluck/voice.patch", |bytes| {
                let text = String::from_utf8(bytes.clone()).unwrap();
                *bytes = text
                    .replace("filter_cutoff 5000", "filter_cutoff 5001")
                    .into_bytes();
            }),
            ("demos/bass/note.events", |bytes| {
                let text = String::from_utf8(bytes.clone()).unwrap();
                *bytes = text.replace(" 0.7\n", " 0.6\n").into_bytes();
            }),
            ("models/demo/regressor.weights", |bytes| {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }),
            ("models/demo/normalization.stats", |bytes| bytes.push(b'\n')),
        ];
        for (path, change) in changes {
            let original = fs::read(root.join(path)).unwrap();
            let mut changed = original.clone();
            change(&mut changed);
            assert_ne!(changed, original, "{path}");
            fs::write(root.join(path), &changed).unwrap();
            let error = output(&root).map(drop).unwrap_err();
            assert_eq!(error.matches(path).count(), 1, "{path}: {error}");
            assert!(error.contains("task docs-demos"), "{path}: {error}");
            let data = read_data(&root).unwrap();
            let refused = render_takes(&root, &data).map(drop).unwrap_err();
            assert!(refused.contains(path), "{path}: {refused}");
            fs::write(root.join(path), &original).unwrap();
            output(&root).unwrap();
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn demo_audio_renders_bit_identical_twice() {
        let root = repository();
        let data = read_data(&root).unwrap();
        let first = render_takes(&root, &data).unwrap();
        let second = render_takes(&root, &data).unwrap();
        assert_eq!(first.len(), VOICES.len() * TAKES.len());
        let out = scratch("render");
        write_audio(&out, &first).unwrap();
        for (a, b) in first.iter().zip(&second) {
            assert_eq!((&a.voice, a.take), (&b.voice, b.take));
            assert!(
                a.wav == b.wav,
                "{}/{} differs between renders",
                a.voice,
                a.take
            );
            let written = fs::read(out.join(&a.voice).join(format!("{}.wav", a.take))).unwrap();
            assert!(written == a.wav, "{}/{} differs on disk", a.voice, a.take);
        }
        let _ = fs::remove_dir_all(&out);
    }

    #[test]
    fn a_take_that_no_longer_renders_as_recorded_is_refused() {
        let root = scratch("take-digests");
        copy_demo_data(&root);
        let data = read_data(&root).unwrap();
        let takes = render_takes(&root, &data).unwrap();
        for take in &takes {
            let recorded = voices(&data)
                .unwrap()
                .iter()
                .find(|v| v["name"] == take.voice.as_str())
                .unwrap()[TAKE_DIGESTS][take.take]
                .clone();
            assert_eq!(
                recorded,
                hex(&sha256(&take.wav[44..])),
                "{}/{}",
                take.voice,
                take.take
            );
        }
        let original = fs::read_to_string(root.join(DATA)).unwrap();
        let changes: [DataChange; 3] = [
            ("pluck/seeded-fit", |data| {
                let cutoff = &mut data["voices"][0]["seededFit"]["normalized"][4];
                let value = cutoff.as_f64().unwrap();
                *cutoff = Value::from(if value > 0.5 {
                    value - 0.25
                } else {
                    value + 0.25
                });
            }),
            ("lead/prediction", |data| {
                data["voices"][3][TAKE_DIGESTS]["prediction"] = Value::from("0".repeat(64));
            }),
            ("bass/recording", |data| {
                data["voices"][2][TAKE_DIGESTS]
                    .as_object_mut()
                    .unwrap()
                    .remove("recording");
            }),
        ];
        for (take, change) in changes {
            let mut changed: Value = serde_json::from_str(&original).unwrap();
            change(&mut changed);
            fs::write(root.join(DATA), crate::pretty(&changed).unwrap()).unwrap();
            let error = output(&root).map(drop).unwrap_err();
            assert_eq!(error.matches(take).count(), 1, "{take}: {error}");
            assert!(error.contains("task docs-demos"), "{take}: {error}");
            let refused = render_takes(&root, &changed).map(drop).unwrap_err();
            assert!(refused.contains(take), "{take}: {refused}");
        }
        fs::write(root.join(DATA), &original).unwrap();
        output(&root).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recorded_physical_value_its_vector_does_not_map_to_is_refused() {
        let root = scratch("physical");
        copy_demo_data(&root);
        check_physical(&root, &read_data(&root).unwrap()).unwrap();
        let original = fs::read_to_string(root.join(DATA)).unwrap();
        let changes: [DataChange; 4] = [
            ("pluck seededFit.physical", |data| {
                let cutoff = &mut data["voices"][0]["seededFit"]["physical"][4];
                *cutoff = Value::from(cutoff.as_f64().unwrap() + 1000.0);
            }),
            ("bass voice.physical", |data| {
                let drive = &mut data["voices"][2]["voice"]["physical"][0];
                *drive = Value::from(f64::from_bits(drive.as_f64().unwrap().to_bits() + 1));
            }),
            ("pad defaultFit.physical", |data| {
                data["voices"][1]["defaultFit"]["physical"][7] = Value::Null;
            }),
            ("lead prediction.physical", |data| {
                data["voices"][3]["prediction"]["physical"]
                    .as_array_mut()
                    .unwrap()
                    .push(Value::from(1.0));
            }),
        ];
        for (field, change) in changes {
            let mut changed: Value = serde_json::from_str(&original).unwrap();
            change(&mut changed);
            fs::write(root.join(DATA), crate::pretty(&changed).unwrap()).unwrap();
            let error = output(&root).map(drop).unwrap_err();
            assert_eq!(error.matches(field).count(), 1, "{field}: {error}");
            assert!(error.contains("task docs-demos"), "{field}: {error}");
            assert_eq!(error.matches(".physical (").count(), 1, "{field}: {error}");
            let refused = render_takes(&root, &changed).map(drop).unwrap_err();
            assert!(refused.contains(field), "{field}: {refused}");
        }
        fs::write(root.join(DATA), &original).unwrap();
        output(&root).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    #[test]
    fn demo_audio_is_pcm16_48k_mono_under_two_seconds() {
        let root = repository();
        let data = read_data(&root).unwrap();
        let takes = render_takes(&root, &data).unwrap();
        assert!(VOICES.len() == 4 || VOICES.len() == 3, "Da12-22");
        for voice in VOICES {
            let names: Vec<_> = takes
                .iter()
                .filter(|t| t.voice == voice.name)
                .map(|t| t.take)
                .collect();
            assert_eq!(names, TAKES.map(|t| t.file), "{}", voice.name);
        }
        for take in &takes {
            let name = format!("{}/{}", take.voice, take.take);
            let wav = &take.wav;
            assert_eq!(&wav[0..4], b"RIFF", "{name}");
            assert_eq!(&wav[8..12], b"WAVE", "{name}");
            assert_eq!(&wav[12..16], b"fmt ", "{name}");
            assert_eq!(u16_at(wav, 20), 1, "{name}: PCM");
            assert_eq!(u16_at(wav, 22), 1, "{name}: mono");
            assert_eq!(u32_at(wav, 24), 48_000, "{name}: rate");
            assert_eq!(u16_at(wav, 34), 16, "{name}: bits");
            assert_eq!(&wav[36..40], b"data", "{name}");
            let frames = u32_at(wav, 40) as usize / 2;
            assert_eq!(frames, take.samples.len(), "{name}");
            assert_eq!(wav.len(), 44 + 2 * frames, "{name}");
            assert!(frames > 0 && frames < 2 * 48_000, "{name}: {frames} frames");
            let peak = take.samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
            assert!(peak <= 1.0 && peak > 0.0, "{name}: peak {peak}");
            if take.take == "recording" {
                // The limiter holds every render inside [-1, 1] by setting a sample past its
                // threshold to the threshold: a recording that reaches it clips (Da12-25).
                assert!(peak < 1.0, "{name}: the voice clips, peak {peak}");
            }
        }
        for record in voices(&data).unwrap() {
            assert_eq!(
                record["recording"]["limitedSamples"], 0,
                "{}: the limiter changed the recording",
                record["name"]
            );
        }
    }

    #[test]
    fn committed_demo_data_is_text_under_64_kib() {
        let root = repository();
        let mut files = files_under(&root.join(VOICES_DIR));
        assert_eq!(files.len(), 2 * VOICES.len(), "{files:?}");
        files.push(root.join(MODEL_DIR).join(STATS_FILE));
        files.push(root.join(MODEL_DIR).join(PROVENANCE_FILE));
        files.push(root.join(DATA));
        let mut total = 0;
        for file in &files {
            let bytes = fs::read(file).unwrap();
            let text = String::from_utf8(bytes.clone())
                .unwrap_or_else(|_| panic!("{} is not UTF-8", file.display()));
            assert!(
                text.chars().all(|c| c == '\n' || !c.is_control()),
                "{} holds a control character",
                file.display()
            );
            assert!(text.ends_with('\n'), "{}", file.display());
            total += bytes.len();
        }
        assert!(total < 64 * 1024, "{total} bytes");
        let data = fs::read_to_string(root.join(DATA)).unwrap();
        assert!(
            !data.contains("\"log\""),
            "demos.json holds no evaluation log"
        );
        let walk = files_under(&root.join(MODEL_DIR));
        assert_eq!(walk.len(), 3, "{walk:?}");
    }

    #[test]
    fn committed_weights_match_their_provenance() {
        let model = repository().join(MODEL_DIR);
        let provenance =
            read_provenance(&fs::read_to_string(model.join(PROVENANCE_FILE)).unwrap()).unwrap();
        let weights = fs::read(model.join(WEIGHTS_FILE)).unwrap();
        assert_eq!(weights.len().to_string(), provenance["artifact_bytes"]);
        assert_eq!(weights.len(), 10_438);
        let artifact = WeightArtifact::read(&model.join(WEIGHTS_FILE)).unwrap();
        assert_eq!(artifact.checksum(), provenance["checksum"]);
        assert_eq!(artifact.modelspec_version(), provenance["modelspec"]);
        assert_eq!(artifact.modelspec_commit(), provenance["modelspec_commit"]);
        assert_eq!(artifact.feature_contract(), provenance["feature_contract"]);
        assert_eq!(provenance["artifact_format"], "weights/1");
        let stats = fs::read(model.join(STATS_FILE)).unwrap();
        assert_eq!(hex(&sha256(&stats)), provenance["statistics_sha256"]);
        let table = read_stats(std::str::from_utf8(&stats).unwrap()).unwrap();
        assert_eq!(table.feature_contract_version(), FEATURES_1_VERSION);
        assert_eq!(provenance["retrain_reproduces_checksum"], "not-verified");
        assert_eq!(
            provenance["checksum"],
            "a17d810459ccfa9ce41c171661a25d5313d1f9e575125062a17b4e51bf9be926"
        );
        assert_eq!(
            provenance["statistics_sha256"],
            "6a1772acb04b1a4b992e29fe7804185072677fa82f1534f1e04c6e110a0810c7"
        );
        for (key, value) in [
            ("dataset_seed", 42),
            ("dataset_train", 50_016),
            ("dataset_validation", 4_992),
            ("dataset_test", 4_992),
            ("training_steps", 3_126),
            ("batch_size", 16),
        ] {
            assert_eq!(
                provenance_number(&provenance, key),
                Ok(value),
                "{key} in {PROVENANCE_FILE}"
            );
        }
        provenance["dataset_generation_s"].parse::<u64>().unwrap();
        provenance["eval_loss"].parse::<f64>().unwrap();
        assert!(read_provenance("patchback-demo-provenance 2\n").is_err());
        assert!(read_provenance("patchback-demo-provenance 1\nchecksum a\nchecksum b\n").is_err());
    }

    #[test]
    fn the_listen_page_offers_four_takes_per_voice() {
        let root = repository();
        let data = read_data(&root).unwrap();
        let page = page(&data).unwrap();
        assert!(!VOICES.is_empty());
        for voice in VOICES {
            for take in &TAKES {
                let tag = format!(
                    "<audio controls preload=\"none\" src={{require('@site/demo-audio/{}/{}.wav').default}} />",
                    voice.name, take.file
                );
                assert_eq!(page.matches(&tag).count(), 1, "{tag}\n{page}");
            }
        }
        assert_eq!(page.matches("<audio ").count(), VOICES.len() * TAKES.len());
        for take in &TAKES {
            assert_eq!(
                page.matches(&format!("**{}**", take.label)).count(),
                VOICES.len(),
                "{}",
                take.label
            );
        }
        assert_eq!(page.matches(" of 8 outputs clamped").count(), VOICES.len());
        assert_eq!(
            page.matches(
                "| Parameter | Voice | Default-start fit | Regressor-seeded fit | Prediction |"
            )
            .count(),
            VOICES.len()
        );
        assert_eq!(
            page.matches("| Fit | Start loss | Best loss | Evaluations |")
                .count(),
            VOICES.len()
        );
        assert!(page.contains("rendered when the site is built"));
        for private in crate::PRIVATE {
            assert!(!page.contains(private), "{private}");
        }
    }
}
