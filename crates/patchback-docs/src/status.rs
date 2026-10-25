//! The status data, `website/data/status.json` (`b10x-status/1`), and the status page,
//! `website/docs/status.mdx`, from [`CAPABILITIES`].
//!
//! `shipped` means code on `main`, and a shipped capability names the test that holds it as
//! `path::function`; generation fails when that test does not exist, so a claim cannot outlive
//! what it rests on. A capability that is not shipped names the milestone epic of the planning
//! store that carries it, which must exist under `.engineering/planning/epic/`; generation warns
//! about one whose epic is already `implemented`, because the item is then due to move to
//! `shipped`. The page names the milestone, never the planning id.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

/// Where the status data lands, relative to the repository root.
pub const DIR: &str = "website/data";
/// The status data's file name under [`DIR`].
pub const FILE: &str = "status.json";
/// The status page's file name under `website/docs`.
pub const PAGE: &str = "status.mdx";

/// The date [`CAPABILITIES`] was last checked against `main` and the planning store.
pub const AS_OF: &str = "2026-10-25";

/// A milestone of DESIGN.md § 13 and the epic that carries it.
#[derive(Clone, Copy)]
pub struct Milestone {
    pub name: &'static str,
    pub epic: &'static str,
}

/// Every Milestone A capability is shipped, so only the tests name it until a new one is planned.
#[cfg_attr(not(test), allow(dead_code))]
const A: Milestone = Milestone {
    name: "Milestone A",
    epic: "sound-engine",
};
const B: Milestone = Milestone {
    name: "Milestone B",
    epic: "modelspec-regressor",
};
const C: Milestone = Milestone {
    name: "Milestone C",
    epic: "phrases",
};
const D: Milestone = Milestone {
    name: "Milestone D",
    epic: "module-choice",
};
const E: Milestone = Milestone {
    name: "Milestone E",
    epic: "larger-scope",
};
const STUDIO: Milestone = Milestone {
    name: "Patch studio",
    epic: "patch-studio",
};

/// One capability on the status page.
pub struct Capability {
    pub area: &'static str,
    pub label: &'static str,
    pub detail: &'static str,
    /// `shipped`, `decided` or `planned`.
    pub status: &'static str,
    /// For a shipped capability, the test that holds it, as `path::function`.
    pub evidence: Option<&'static str>,
    /// For a capability that is not shipped, the milestone that carries it.
    pub milestone: Option<Milestone>,
    pub href: &'static str,
}

const fn planned(
    milestone: Milestone,
    area: &'static str,
    label: &'static str,
    detail: &'static str,
    href: &'static str,
) -> Capability {
    Capability {
        area,
        label,
        detail,
        status: "planned",
        evidence: None,
        milestone: Some(milestone),
        href,
    }
}

/// Every capability the site claims, in the order the status table lists them.
pub const CAPABILITIES: &[Capability] = &[
    Capability {
        area: "Audio",
        label: "WAV input and output without an audio crate",
        detail: "Mono, or stereo downmixed to mono by the per-frame mean and recorded; PCM16 or IEEE float32 at 48 kHz, written back byte-exact. Truncated chunks, size overflow, non-finite samples, unsupported formats and files past the frame bound are refused.",
        status: "shipped",
        evidence: Some("crates/patchback-audio/tests/wav.rs::pcm16_roundtrip_is_byte_exact"),
        milestone: None,
        href: "/docs/reference/files#reference-audio",
    },
    Capability {
        area: "Patch",
        label: "A typed patch graph, validated before it runs",
        detail: "Identifiers, port rates, parameter contracts, the ports of each kind, cycles and provisional resource caps are checked, in the order the specification declares; a refusal names its error and the node, edge or line. Only a validated patch compiles, into a deterministic execution plan (ready nodes by lowest identifier, one buffer per output port). patchback patch validate runs the checks and the compilation on a .patch file.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-core/tests/patch_graph_validation.rs::accepted_the_default_voice_validates_and_compiles",
        ),
        milestone: None,
        href: "/docs/reference/cli#patchback-patch-validate",
    },
    Capability {
        area: "Patch",
        label: "The line-oriented .patch file",
        detail: "A versioned, human-readable patch file in physical units, parsed into a loaded, unvalidated patch and written in canonical form; write then parse is bit-exact. Unknown versions, duplicate keys or IDs, malformed numbers, unsupported rates, lines outside the grammar and lengths past the provisional bounds are refused with the line, and nothing is loaded. patchback patch validate reads it.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-core/tests/patch_file_format.rs::default_voice_write_then_parse_is_identity",
        ),
        milestone: None,
        href: "/docs/reference/files#patch-patch",
    },
    Capability {
        area: "Patch",
        label: "Parameter contracts for the subtractive voice",
        detail: "Eight parameters, each with an index, a stable path, a normalized-to-physical mapping and its inverse, a physical range and a default; the defaults are placeholders until the design sets them. An out-of-range normalized value is clamped and the clamp recorded; a non-finite value and an invalid contract are refused.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-core/tests/parameter_contracts.rs::subtractive_mono_declares_the_eight_contracts_in_design_order",
        ),
        milestone: None,
        href: "/docs/reference/parameters",
    },
    Capability {
        area: "Score",
        label: "Note events and the notes.events file",
        detail: "A versioned, line-oriented list of sample-timestamped note events, parsed into a note sequence and written in canonical form; write then parse is bit-exact and sample offsets are exact. Notes that do not end after they start, overlap in a monophonic sequence or fall outside the MIDI or velocity range, unknown versions, duplicate keys, malformed numbers, lines outside the grammar and lengths past the provisional bounds are refused with the line, and nothing is loaded. Notes only; patchback render refuses a sequence whose rate is not the rendering rate.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-core/tests/note_events.rs::write_then_parse_is_identity_over_generated_sequences",
        ),
        milestone: None,
        href: "/docs/reference/files#events-events",
    },
    Capability {
        area: "Sound engine",
        label: "The fixed subtractive voice",
        detail: "One monophonic voice at 48 kHz: a polyBLEP saw oscillator, a bounded tanh drive, a TPT state-variable lowpass with cutoff clamped below Nyquist and resonance to its contract range, a linear ADSR with sample-accurate note-on, retrigger and note-off and safe zero times, a smoothed VCA and output gain, and a protective limiter whose engagement is counted and reported. Golden waveform, envelope and filter tests hold the units; high-note aliasing against a naive saw is measured.",
        status: "shipped",
        evidence: Some("crates/patchback-dsp/tests/units.rs::the_adsr_matches_its_golden_segments"),
        milestone: None,
        href: "/docs/concepts/patch#the-fixed-subtractive-voice",
    },
    Capability {
        area: "Sound engine",
        label: "Deterministic offline rendering",
        detail: "patchback render --patch --notes --out [--block-size]: a compiled patch plays a note sequence into a mono 48 kHz float32 WAV lasting to the last note's end plus the release and a provisional 100 ms tail. The same inputs give the same bytes at every block size; bounded controls give finite samples. Whether the limiter engaged and how many samples it changed are printed. An unsupported rate, a block size of zero or less, a patch or note sequence that does not load, an uncompiled patch, a render past the length bound and a non-finite sample are refused and nothing is written.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/render.rs::render_writes_a_float32_wav_with_the_default_block_size_and_with_512",
        ),
        milestone: None,
        href: "/docs/guides/render-a-patch",
    },
    Capability {
        area: "Analysis",
        label: "Native FFT, STFT and audio descriptors",
        detail: "patchback analyze --input --out: radix-2 FFT, Hann-windowed STFT at 512, 2048 and 8192 samples, RMS and peak envelopes, spectral centroid, rolloff, flatness, log-band energies, spectral flux and f0 estimation, written to one versioned analysis file. Silent input, an ambiguous or out-of-range pitch, a clip too short for the f0 search, non-finite samples and sizes past the provisional bounds are refused and nothing is written.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/analyze.rs::analyze_writes_one_versioned_analysis_file",
        ),
        milestone: None,
        href: "/docs/concepts/listening",
    },
    Capability {
        area: "Fit",
        label: "A multi-resolution audio comparison",
        detail: "patchback compare --reference --candidate: the candidate is aligned to the reference by up to 100 ms, then log-spectral (512, 2048 and 8192 samples), envelope, transient and band-energy terms, each in [0, 1] and 0 for identical clips, are printed with the design's weights, their weighted sum and the lag. Clips at different rates and lengths alignment cannot absorb are refused. The tolerance, spectral floor and term definitions are provisional.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/compare.rs::compare_prints_the_loss_and_its_components",
        ),
        milestone: None,
        href: "/docs/reference/cli#patchback-compare",
    },
    Capability {
        area: "Fit",
        label: "Bounded derivative-free search",
        detail: "patchback fit --input --notes --out: from the default patch, then from seeded restart points, coordinate search over the eight normalized parameters (step 0.25, halving) renders and scores each candidate against the reference and keeps only a strictly lower valid loss. It stops at the render budget (--max-renders), the wall-time bound (--max-wall-time-ms), the restart count (--max-restarts) or when a restart improves less than --improvement-threshold, and writes the best patch as patch.patch with the run's other files. The same seed and inputs give the same patch. Invalid bounds, a missing reference or note sequence, a silent or unpitched reference and a run without a valid candidate are refused and nothing is written. The defaults and the smallest step are provisional.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/fit.rs::fit_writes_the_best_patch_and_reports_the_run",
        ),
        milestone: None,
        href: "/docs/reference/cli#patchback-fit",
    },
    Capability {
        area: "Fit",
        label: "The fit run's six files",
        detail: "patchback fit writes patch.patch, notes.events, reconstructed.wav, report.txt, run.meta and trace.events into the output directory. reconstructed.wav is the clip the best patch was scored on, so patchback compare of the reference and it prints the best loss again; report.txt holds the losses, counts, stop reason, wall time and warnings; run.meta the seed, the input digests, the ModelSpec pin, the feature contract and the bounds; trace.events one line per evaluation. Any of the six already present is refused before the search, and a failed write removes everything the run wrote. The same seed and inputs write the same files, report.txt's wall time aside, unless the wall-time bound stops the run.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/fit.rs::the_same_seed_and_inputs_write_the_same_patch_in_two_processes",
        ),
        milestone: None,
        href: "/docs/reference/files#a-fit-run",
    },
    Capability {
        area: "Fit",
        label: "A baseline against the default patch",
        detail: "patchback benchmark synthesizes a seeded holdout of reference clips with known notes, scores the default patch on each and fits each from it, and records the lower median loss of both, the reduction, whether the 20 % go/no-go target was met, the median render evaluations, the wall time and three listening examples in benchmark.txt, clips.txt and examples/. Both verdicts complete the benchmark. The same flags write the same files, benchmark.txt's wall time aside.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/benchmark.rs::a_small_benchmark_completes_and_its_files_agree",
        ),
        milestone: None,
        href: "/docs/reference/cli#patchback-benchmark",
    },
    Capability {
        area: "Audio",
        label: "Provenance for every fixture",
        detail: "An external recording is imported only with its provenance and licence recorded beside the clip; one missing either is refused and no clip exists. A clip imported as synthesized by Patchback needs neither and records its origin and, when given, its render. The two checks come before the file is read: a recording missing either is refused even when the file is missing, truncated or corrupt. The patchback commands take no origin, provenance or licence yet.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-audio/tests/provenance.rs::external_recording_with_provenance_and_licence_imports_and_keeps_both",
        ),
        milestone: None,
        href: "/docs/concepts/guarantees#fixtures",
    },
    Capability {
        area: "Dependencies",
        label: "A dependency gate",
        detail: "An audit of the resolved Cargo.lock passes a workspace with no third-party application crate but clap, the one reviewed CLI parser, and its reviewed closure; it refuses a forbidden or unreviewed crate and any ModelSpec crate, and admits at most one approved audio I/O backend with its reviewed closure, only behind an optional feature.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-cli/tests/dependencies.rs::the_application_crates_take_no_third_party_crate_but_clap_and_its_closure",
        ),
        milestone: None,
        href: "/docs/concepts/guarantees#dependencies",
    },
    Capability {
        area: "Model",
        label: "A pinned ModelSpec that checks the model",
        detail: "ModelSpec 0.5.0 is pinned in models/modelspec.pin by version, commit and binary checksum; CI installs that binary and runs modelspec check on models/patch_regressor.mspec. A binary reporting another version is refused, and a model the pinned binary rejects is recorded as rejected and cannot be trained.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-model/tests/adapter.rs::a_rejected_source_cannot_be_trained",
        ),
        milestone: None,
        href: "/docs/guides/train-the-regressor#check-and-train",
    },
    Capability {
        area: "Analysis",
        label: "The frozen feature contract features/1",
        detail: "32 values per note: eight band energies over attack, body and release windows, then RMS, peak, spectral centroid, rolloff, flatness, zero-crossing rate, attack-envelope slope and normalized MIDI pitch. A contract is defined once per version, amended only as a draft and frozen by publishing; extraction runs only under a published contract, and a value that cannot be computed reliably or is not finite is refused rather than estimated.",
        status: "shipped",
        evidence: Some("crates/patchback-analysis/tests/features.rs::feature_contract_extract"),
        milestone: None,
        href: "/docs/concepts/listening#features1",
    },
    Capability {
        area: "Analysis",
        label: "Training-split normalization statistics",
        detail: "patchback generate-dataset writes normalization.stats: the mean and population standard deviation of each features/1 value over the training split's rows, computed in f64 under the dataset's published contract. Statistics from the validation or test split, under an unknown or draft contract, or under a version the contract does not carry are refused. The same statistics normalize features at training and inference, as (x − mean) / std, with divisor 1 for a feature whose training values are all equal; features of another contract version and non-finite values are refused. The x.f32 of every split, validation and test included, holds the features z-scored under the training statistics.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-dataset/tests/normalization_scenarios.rs::normalization_stats_train_only",
        ),
        milestone: None,
        href: "/docs/reference/files#normalizationstats",
    },
    Capability {
        area: "Dataset",
        label: "A deterministic paired dataset",
        detail: "patchback generate-dataset draws patch seeds from SplitMix64 of --seed. From each seed's own xoshiro256** stream it draws eight normalized parameters, a MIDI note from 36 to 84 and a gate from 0.25 to 1.5 s, log-uniform. It renders each draw with the same DSP engine and extracts features/1, then fills train, validation and test to exact multiples of the batch size 16. Splits are isolated by patch seed. Clipped, silent and noisy renders go to a separate diagnostic set and never into a split. Each split is written as ModelSpec dense data: x.f32 (N × 32) of features z-scored under the training split's statistics and y.f32 (N × 8), row-major little-endian f32, and order.u32 (the row indices). Each file is read back and checked: its byte count, finite values in x.f32 and y.f32, and each row index once in order.u32. The same seed writes byte-identical dataset.meta, samples.meta and dense files.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-dataset/tests/generate.rs::a_real_generation_fills_each_split_with_valid_isolated_samples",
        ),
        milestone: None,
        href: "/docs/reference/cli#patchback-generate-dataset",
    },
    Capability {
        area: "Model",
        label: "A parameter regressor trained with ModelSpec",
        detail: "A fixed-shape two-layer MLP, 32 features to 8 normalized parameters, checked and trained by modelspec on its CPU backend. patchback_model::TrainingRun runs modelspec train --backend cpu on a written training split, with an optional evaluation split, under a timeout; exit status 0 completes the run, and its stdout gives the loss curve and the evaluation loss. On generated data the pinned ModelSpec 0.5.0 lowers the loss from 1.0717294 to 0.8882302 in 4 steps on 64 rows. The run is a library call with no patchback command yet; patchback_model::WeightArtifact::export writes its checked weights to a checksummed weights/1 file.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-model/tests/pinned_training.rs::the_pinned_modelspec_trains_on_real_generated_data",
        ),
        milestone: None,
        href: "/docs/guides/train-the-regressor#check-and-train",
    },
    Capability {
        area: "Model",
        label: "Checked weight export",
        detail: "patchback_model::WeightArtifact::export reads the stdout of a completed run of the pinned ModelSpec and refuses any line outside its documented init, step, eval and param lines, a parameter other than @PatchRegressor.w1 and .w2, shapes other than [32, 64] and [64, 8], counts other than 2,048 and 512, and non-finite values. It writes a new weights/1 file: a header with the model, the ModelSpec version and commit, the feature contract and the payload's SHA-256, then 10,240 bytes of little-endian f32. WeightArtifact::read verifies the header, the length and the checksum. On the pinned ModelSpec 0.5.0 all 2,560 values read back as printed. A library call with no patchback command yet.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-model/tests/pinned_export.rs::the_weights_of_a_pinned_run_are_exported_and_read_back",
        ),
        milestone: None,
        href: "/docs/reference/files#exported-weights",
    },
    Capability {
        area: "Model",
        label: "A native predictor",
        detail: "patchback_model::Predictor loads a weights/1 file and runs its two dense layers and ReLU in f32 on one normalized features/1 vector. It refuses a missing or corrupt artifact, a vector of another length than 32, another feature contract and non-finite values. Outputs outside [0, 1] are clamped and counted. Golden tests and an f64 reference hold the arithmetic; on the pinned ModelSpec 0.5.0 its error on the evaluation rows is ModelSpec's eval loss. A library call with no patchback command yet.",
        status: "shipped",
        evidence: Some(
            "crates/patchback-model/tests/predictor.rs::sixty_four_seeded_inputs_match_an_f64_reference",
        ),
        milestone: None,
        href: "/docs/guides/train-the-regressor#predict-with-the-weights",
    },
    planned(
        B,
        "Fit",
        "A regressor-seeded fit",
        "The search starts from the regressor's prediction instead of the default patch, benchmarked against the default patch and the raw prediction.",
        "/docs/concepts/closed-loop#the-search",
    ),
    planned(
        C,
        "Score",
        "Monophonic note estimation with confidence",
        "Onsets, pitches, gate lengths and tails of external audio, each with a confidence; an ambiguous estimate is refused with a diagnostic.",
        "/docs/concepts/events",
    ),
    planned(
        C,
        "Score",
        "Short phrases",
        "One editable patch and its event sequence for an isolated bass or lead phrase.",
        "/docs/concepts/events",
    ),
    planned(
        D,
        "Patch",
        "Candidate topologies",
        "Saw, square and sine oscillators, filter families, LFO and delay; the topology is chosen by rendering and ranking candidates, with a simplicity penalty.",
        "/docs/concepts/patch#beyond-the-fixed-voice",
    ),
    planned(
        D,
        "Patch",
        "Control automation and delayed feedback",
        "Time-varying control events, and cycles only through an explicit delay of at least one sample.",
        "/docs/concepts/patch#beyond-the-fixed-voice",
    ),
    planned(
        E,
        "Scope",
        "Stems, drums and arrangement",
        "Several monophonic or polyphonic stems, drum and effect modules, repeated-pattern analysis and arrangement events.",
        "/docs/concepts/events#longer-music",
    ),
    planned(
        E,
        "Model",
        "GPU training through ModelSpec",
        "Supported training moves to ModelSpec's GPU backend once it ships and passes CPU/GPU equivalence tests.",
        "/docs/concepts/regressor",
    ),
    planned(
        STUDIO,
        "Studio",
        "Module catalogue view",
        "One card per module kind with its ports (direction, rate, meaning) and parameter contracts (index, path, mapping, physical range, default). The prototype renders illustrative fixture data.",
        "/docs/concepts/studio#module-catalogue",
    ),
    planned(
        STUDIO,
        "Studio",
        "Patch graph view",
        "A validated patch drawn as nodes by rank and edges coloured by port rate, with delayed feedback routed as a z⁻¹ return path through a Delay node.",
        "/docs/concepts/studio#patch-graph",
    ),
    planned(
        STUDIO,
        "Studio",
        "Fit evolution replay",
        "A finished fit run replayed evaluation by evaluation: loss curve, retained or candidate patch, parameter trajectories and topology changes. Replays a fixture trace until a fit run writes one.",
        "/docs/concepts/studio#fit-replay",
    ),
    planned(
        STUDIO,
        "Studio",
        "Fit evolution live",
        "The same view following a running fit, frames streamed over server-sent events from a std-only Rust server on 127.0.0.1.",
        "/docs/concepts/studio#live-fit",
    ),
    planned(
        STUDIO,
        "Studio",
        "End result view",
        "The retained patch, its parameters against the reference, and reference and render spectra at an FFT length of 512, 2048 or 8192.",
        "/docs/concepts/studio#end-result",
    ),
];

/// Whether `source` defines a function called `name`.
fn defines(source: &str, name: &str) -> bool {
    source.contains(&format!("fn {name}("))
}

fn check_evidence(root: &Path, label: &str, reference: &str) -> Result<String, String> {
    let (file, function) = reference
        .rsplit_once("::")
        .ok_or_else(|| format!("{label}: evidence {reference} is not path::function"))?;
    let source = fs::read_to_string(root.join(file))
        .map_err(|e| format!("{label}: evidence file {file}: {e}"))?;
    if !defines(&source, function) {
        return Err(format!("{label}: {file} defines no fn {function}"));
    }
    Ok(format!("Held by test {function} in {file}."))
}

/// The `status:` of a milestone's epic, read from its front matter.
fn epic_status(root: &Path, label: &str, epic: &str) -> Result<String, String> {
    let path = root.join(format!(".engineering/planning/epic/{epic}.md"));
    let text = fs::read_to_string(&path).map_err(|e| {
        format!("{label}: the milestone epic {epic} is not in the planning store: {e}")
    })?;
    text.lines()
        .skip(1)
        .take_while(|line| *line != "---")
        .find_map(|line| line.strip_prefix("status:"))
        .map(|status| status.trim().to_owned())
        .ok_or_else(|| format!("{label}: epic {epic} has no status"))
}

/// The status document for `capabilities` in the repository at `root`, and any warnings.
pub fn document_for(
    root: &Path,
    capabilities: &[Capability],
    as_of: &str,
) -> Result<(String, Vec<String>), String> {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for capability in capabilities {
        let label = capability.label;
        let detail = match (capability.status, capability.evidence, capability.milestone) {
            ("shipped", Some(evidence), None) => {
                format!(
                    "{} {}",
                    capability.detail,
                    check_evidence(root, label, evidence)?
                )
            }
            ("shipped", _, _) => {
                return Err(format!(
                    "{label}: a shipped capability names its test and no milestone"
                ));
            }
            ("decided" | "planned", Some(_), _) => {
                return Err(format!("{label}: only a shipped capability names a test"));
            }
            ("decided" | "planned", None, Some(milestone)) => {
                if epic_status(root, label, milestone.epic)? == "implemented" {
                    warnings.push(format!(
                        "{label}: {} is implemented; move the item to shipped",
                        milestone.name
                    ));
                }
                format!("{} {}.", capability.detail, milestone.name)
            }
            ("decided" | "planned", None, None) => {
                return Err(format!(
                    "{label}: a {} capability names its milestone",
                    capability.status
                ));
            }
            (other, _, _) => {
                return Err(format!(
                    "{label}: status {other} is not shipped, decided or planned"
                ));
            }
        };
        let mut item = json!({
            "area": capability.area,
            "label": label,
            "detail": detail,
            "status": capability.status,
        });
        if !capability.href.is_empty() {
            item["href"] = Value::from(capability.href);
        }
        items.push(item);
    }
    let document = json!({
        "format": "b10x-status/1",
        "asOf": as_of,
        "source": "generated by patchback-docs from its capability list, checked against main and the planning store",
        "items": items,
    });
    let text = serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?;
    Ok((text + "\n", warnings))
}

/// The status document of this repository.
pub fn document(root: &Path) -> Result<(String, Vec<String>), String> {
    document_for(root, CAPABILITIES, AS_OF)
}

/// How many of `capabilities` carry `status`.
fn count(capabilities: &[Capability], status: &str) -> usize {
    capabilities.iter().filter(|c| c.status == status).count()
}

/// The status page, which renders the status data with Docs System's `StatusTable`.
pub fn page(capabilities: &[Capability], as_of: &str) -> String {
    let shipped = count(capabilities, "shipped");
    let planned = count(capabilities, "planned");
    let decided = count(capabilities, "decided");
    format!(
        "---\n{header}\ntitle: Status\nsidebar_position: 3\ndescription: \"What Patchback ships today and what is planned, capability by capability.\"\ncustom_edit_url: null\n---\n\n\
         {mdx}\n\n\
         import status from '@site/data/status.json';\n\n\
         # Status\n\n\
         As of {as_of}: {shipped} shipped, {decided} decided, {planned} planned. The \
         documentation describes Patchback as [DESIGN.md](https://github.com/beyond10x/patchback/blob/main/DESIGN.md) \
         specifies it; this page says which of it exists. The \
         [ESS specification](/docs/reference/ess) in `spec/` is what the repository holds today.\n\n\
         **Shipped** means code on `main`, held by a test the row names. **Planned** means a \
         milestone of the design and nothing more; each planned row names its milestone.\n\n\
         | Milestone | Delivers |\n|---|---|\n\
         | A | Sound engine without ML: WAV I/O, the fixed subtractive voice, analysis, audio loss, bounded search |\n\
         | B | Native dataset, ModelSpec M0 regressor, weight export, native predictor |\n\
         | C | Monophonic phrases: onset, pitch and gate estimation with confidence |\n\
         | D | Candidate topologies, modulation, delayed feedback |\n\
         | E | Stems, drums, arrangement; GPU training once ModelSpec ships it |\n\
         | Patch studio | A local page: module catalogue, patch graph, fit replay and live fit, end result |\n\n\
         ## Every capability\n\n\
         <StatusTable data={{status}} label=\"Every capability, shipped and planned\" />\n\n\
         The table is generated by `patchback-docs` from its capability list. Generation fails \
         when a shipped row's test does not exist or a planned row's milestone is missing from \
         the planning store, so a row cannot outlive what it rests on.\n",
        header = crate::FRONT_MATTER_HEADER,
        mdx = crate::reference::MDX_HEADER,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchback-docs-status-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("crates/x/tests")).unwrap();
        fs::create_dir_all(dir.join(".engineering/planning/epic")).unwrap();
        fs::write(
            dir.join("crates/x/tests/render.rs"),
            "#[test]\nfn renders_repeatably() {}\n",
        )
        .unwrap();
        fs::write(
            dir.join(".engineering/planning/epic/sound-engine.md"),
            "---\nid: epic:sound-engine\nkind: epic\nstatus: draft\n---\n",
        )
        .unwrap();
        fs::write(
            dir.join(".engineering/planning/epic/done.md"),
            "---\nid: epic:done\nkind: epic\nstatus: implemented\n---\n",
        )
        .unwrap();
        dir
    }

    fn item(
        status: &'static str,
        evidence: Option<&'static str>,
        milestone: Option<Milestone>,
    ) -> Capability {
        Capability {
            area: "Area",
            label: "Label",
            detail: "Detail.",
            status,
            evidence,
            milestone,
            href: "/docs/x",
        }
    }

    #[test]
    fn shipped_items_need_a_test_that_exists() {
        let root = repo("shipped");
        let test = "crates/x/tests/render.rs::renders_repeatably";
        let (text, _) = document_for(&root, &[item("shipped", Some(test), None)], AS_OF).unwrap();
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            document["items"][0]["detail"],
            "Detail. Held by test renders_repeatably in crates/x/tests/render.rs."
        );
        for missing in [
            "crates/x/tests/render.rs::gone",
            "crates/x/tests/nowhere.rs::renders_repeatably",
            "crates/x/tests/render.rs",
        ] {
            assert!(document_for(&root, &[item("shipped", Some(missing), None)], AS_OF).is_err());
        }
        assert!(document_for(&root, &[item("shipped", None, None)], AS_OF).is_err());
        assert!(document_for(&root, &[item("shipped", Some(test), Some(A))], AS_OF).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn planned_items_name_their_milestone_not_the_planning_id() {
        let root = repo("planned");
        let (text, warnings) =
            document_for(&root, &[item("planned", None, Some(A))], AS_OF).unwrap();
        assert!(warnings.is_empty());
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["format"], "b10x-status/1");
        assert_eq!(document["items"][0]["detail"], "Detail. Milestone A.");
        assert!(!text.contains("epic:"));
        let done = Milestone {
            name: "Milestone Z",
            epic: "done",
        };
        let (_, warnings) =
            document_for(&root, &[item("planned", None, Some(done))], AS_OF).unwrap();
        assert_eq!(warnings.len(), 1);
        let gone = Milestone {
            name: "Milestone Y",
            epic: "gone",
        };
        for refused in [
            item("planned", None, None),
            item("planned", None, Some(gone)),
            item(
                "planned",
                Some("crates/x/tests/render.rs::renders_repeatably"),
                Some(A),
            ),
            item("someday", None, Some(A)),
        ] {
            assert!(document_for(&root, &[refused], AS_OF).is_err());
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn only_shipped_capabilities_name_a_test() {
        assert!(count(CAPABILITIES, "shipped") > 0);
        for capability in CAPABILITIES {
            assert_eq!(
                capability.evidence.is_some(),
                capability.status == "shipped",
                "{}",
                capability.label
            );
            assert_eq!(
                capability.milestone.is_none(),
                capability.status == "shipped",
                "{}",
                capability.label
            );
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(document(&root).is_ok());
    }

    #[test]
    fn the_page_counts_the_capabilities_and_renders_the_table() {
        let page = page(CAPABILITIES, AS_OF);
        let shipped = count(CAPABILITIES, "shipped");
        let decided = count(CAPABILITIES, "decided");
        assert_eq!(
            shipped + decided + count(CAPABILITIES, "planned"),
            CAPABILITIES.len()
        );
        assert!(page.contains(&format!(
            "{shipped} shipped, {decided} decided, {} planned",
            CAPABILITIES.len() - shipped - decided
        )));
        assert!(page.contains("import status from '@site/data/status.json';"));
        assert!(page.contains("<StatusTable data={status}"));
        assert!(page.contains("custom_edit_url: null"));
    }
}
