//! Every refusal of a training run and of a weight export names an error
//! `spec/domains/model.yaml` declares, and every error the four training-run commands
//! (`PlanTrainingRun`, `BindModelSource`, `StartTrainingRun`, `FinishTrainingRun`) and
//! `ExportWeights` declare is emitted by driving that command through [`TrainingRun`] or
//! [`WeightArtifact::export`], or is listed in [`NOT_EMITTED_BY_THE_CRATE`] with the reason no
//! path of this crate reaches it. `CorruptWeightArtifact`, which `Predict` declares, is emitted
//! by [`WeightArtifact::read`], the reader the predictor will call (Da9-10, Da9-18).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use patchback_dataset::{
    DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset, Pipeline, PrngAlgorithm,
    SplitRole, WriteDenseFile,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ExportError, ModelSource, ModelSpec, ModelSpecError,
    ModelSpecPin, ReadError, TrainingError, TrainingRun, TrainingRunState, W1_NAME, W1_SHAPE,
    W2_NAME, W2_SHAPE, WeightArtifact,
};

fn spec() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/domains/model.yaml");
    std::fs::read_to_string(path).unwrap()
}

fn section<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    text.split_once(start)
        .unwrap_or_else(|| panic!("the domain has {start:?}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("the domain has {end:?} after {start:?}"))
        .0
}

/// The errors a command declares (`error:` lines between its name and `next`).
fn declared_by(spec: &str, command: &str, next: &str) -> BTreeSet<String> {
    section(
        spec,
        &format!("  - name: patchback.model.{command}\n"),
        next,
    )
    .lines()
    .filter_map(|line| line.trim().strip_prefix("error: "))
    .map(str::to_owned)
    .collect()
}

/// The training-run commands and `ExportWeights`, each with what follows it in the domain.
const COMMANDS: [(&str, &str); 5] = [
    (
        "PlanTrainingRun",
        "\n  - name: patchback.model.BindModelSource\n",
    ),
    (
        "BindModelSource",
        "\n  - name: patchback.model.StartTrainingRun\n",
    ),
    (
        "StartTrainingRun",
        "\n  - name: patchback.model.FinishTrainingRun\n",
    ),
    (
        "FinishTrainingRun",
        "\n  - name: patchback.model.ExportWeights\n",
    ),
    ("ExportWeights", "\n  - name: patchback.model.Predict\n"),
];

/// The errors the five commands declare that no path of this crate emits, each with its reason.
const NOT_EMITTED_BY_THE_CRATE: [(&str, &str); 4] = [
    (
        "patchback.model.UnknownPin",
        "a ModelSpecPin is a value handed to TrainingRun::plan, not a record looked up by identity (Da8-21)",
    ),
    (
        "patchback.model.GpuEquivalenceNotRecorded",
        "every Vulkan plan is refused BackendNotShipped first, since the pin file records no shipped Vulkan backend (Da8-22); the outcome after it is unreachable",
    ),
    (
        "patchback.model.UnknownTrainingRun",
        "a TrainingRun is a value its methods are called on, and WeightArtifact::export takes the run itself; there is no identity to miss (Da8-21, Da9-7)",
    ),
    (
        "patchback.model.UnknownModelSource",
        "a ModelSource is a value; bind takes the source itself, not an identity (Da8-21, Da8-39)",
    ),
];

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().unwrap()
}

/// A pin of another release than the repository's.
fn other_pin() -> ModelSpecPin {
    ModelSpecPin::parse(&format!(
        "version 0.4.0\ncommit {}\nsha256-x86_64-unknown-linux-gnu {}\n",
        "a".repeat(40),
        "b".repeat(64)
    ))
    .unwrap()
}

/// A `param` line of `count` copies of `token`.
fn param(name: &str, shape: [usize; 2], count: usize, token: &str) -> String {
    format!(
        "param {name}: Tensor<f32, [{}, {}]> = [{}]",
        shape[0],
        shape[1],
        vec![token; count].join(", ")
    )
}

/// The stdout of each export stand-in: the two documented `param` lines with one defect, each
/// leading `ExportWeights` to one refusal.
fn export_outputs() -> [(&'static str, Vec<String>); 5] {
    let w1 = || param(W1_NAME, W1_SHAPE, 2048, "0.5");
    let w2 = || param(W2_NAME, W2_SHAPE, 512, "0.5");
    [
        ("extra-line", vec!["noise".to_owned(), w1(), w2()]),
        (
            "bias",
            vec![param("@PatchRegressor.b1", W1_SHAPE, 2048, "0.5"), w2()],
        ),
        (
            "transposed",
            vec![param(W1_NAME, [64, 32], 2048, "0.5"), w2()],
        ),
        ("short", vec![w1(), param(W2_NAME, W2_SHAPE, 511, "0.5")]),
        ("nan", vec![w1(), param(W2_NAME, W2_SHAPE, 512, "NaN")]),
    ]
}

/// Stand-ins accepting every source: one reporting the repository's version, one 0.4.0, and one
/// an export defect printing its stdout on `train` and exiting 0. Every script is written before
/// anything is spawned (ETXTBSY).
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-domain-standins");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let script = |name: &str, version: &str, train: &str| {
            let path = dir.join(name);
            fs::write(
                &path,
                format!("#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = train ]; then {train}; fi\nexit 0\n"),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        };
        let version = pin().version().to_owned();
        script("checks", &version, ":");
        script("other", "0.4.0", ":");
        for (name, lines) in export_outputs() {
            let stdout = dir.join(format!("{name}.stdout"));
            let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
            fs::write(&stdout, text).unwrap();
            script(name, &version, &format!("cat {}", stdout.display()));
        }
        dir
    })
}

fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

/// A source registered under `pin` and checked by a stand-in of that pin.
fn checked(pin: &ModelSpecPin, standin: &str) -> ModelSource {
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, pin);
    source
        .check(&ModelSpec::open(&standins().join(standin), pin).unwrap())
        .unwrap();
    source
}

/// A split of `count` rows in a dataset of `batch_size`, Written through `WriteDenseFile` and
/// `FinishSplit` when `written`.
fn split(store: &mut DatasetStore, batch_size: u64, count: i64, written: bool) -> DatasetSplitId {
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 1,
        prng_algorithm: PrngAlgorithm("xoshiro256**".to_owned()),
        batch_size,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let split = store.open_split(dataset, SplitRole::Train, count).unwrap();
    if written {
        for (kind, columns) in [(DenseFileKind::X, 32), (DenseFileKind::Y, 8)] {
            let content: Vec<u8> = (0..count * columns)
                .flat_map(|_| 0.5f32.to_le_bytes())
                .collect();
            store
                .write_dense_file(
                    WriteDenseFile {
                        dataset_split_id: split,
                        kind,
                        path: PathBuf::from("data/train").join(kind.file_name()),
                        rows: count,
                        columns,
                        byte_count: count * columns * 4,
                    },
                    &content,
                )
                .unwrap();
        }
        store.finish_split(split).unwrap();
    }
    split
}

/// One refusal: the command, the declared error it names (`None` for one that is not the
/// model's) and its debug text.
type Refusal = (&'static str, Option<&'static str>, String);

/// Each command driven to each refusal this crate can return for it. A call that was not refused
/// contributes nothing, so the test below finds it missing.
fn driven() -> Vec<Refusal> {
    let mut refusals: Vec<Refusal> = Vec::new();
    let mut refused = |command: &'static str, result: Result<(), TrainingError>| {
        if let Err(error) = result {
            refusals.push((command, error.domain_error(), format!("{error:?}")));
        }
    };
    let pin = pin();
    let modelspec = ModelSpec::open(&standins().join("checks"), &pin).unwrap();
    let mut store = DatasetStore::new();
    let open = split(&mut store, 16, 16, false);
    let written = split(&mut store, 16, 16, true);
    let off_batch = split(&mut store, 8, 24, true);
    let unknown = DatasetSplitId(0xff);

    refused(
        "PlanTrainingRun",
        TrainingRun::plan(Backend::Vulkan, &pin).map(drop),
    );

    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    let unchecked = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    refused("BindModelSource", run.bind(&unchecked));
    let elsewhere = checked(&other_pin(), "other");
    refused("BindModelSource", run.bind(&elsewhere.trainable().unwrap()));

    let mut start = |run: &mut TrainingRun, data, eval| {
        refused(
            "StartTrainingRun",
            run.start(&store, data, eval, &modelspec, 1, DEFAULT_TRAINING_TIMEOUT),
        );
    };
    start(&mut run, written, None);
    let source = checked(&pin, "checks");
    run.bind(&source.trainable().unwrap()).unwrap();
    start(&mut run, unknown, None);
    start(&mut run, open, None);
    start(&mut run, written, Some(unknown));
    start(&mut run, written, Some(off_batch));

    refused("BindModelSource", run.bind(&source.trainable().unwrap()));
    refused("FinishTrainingRun", run.finish().map(drop));

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-domain-export");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("regressor.weights");
    let mut exported = |run: &TrainingRun| {
        if let Err(error) = WeightArtifact::export(run, "features/1", &path) {
            refusals.push(("ExportWeights", error.domain_error(), format!("{error:?}")));
        }
        assert!(!path.exists());
    };
    exported(&TrainingRun::plan(Backend::Cpu, &pin).unwrap());
    for (standin, _) in export_outputs() {
        let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
        run.bind(&source.trainable().unwrap()).unwrap();
        let binary = ModelSpec::open(&standins().join(standin), &pin).unwrap();
        run.start(&store, written, None, &binary, 1, DEFAULT_TRAINING_TIMEOUT)
            .unwrap();
        assert_eq!(run.finish().unwrap(), TrainingRunState::Completed);
        exported(&run);
    }
    refusals
}

#[test]
fn every_refusal_names_a_declared_error() {
    let spec = spec();
    let declared: Vec<&str> = section(&spec, "\nerrors:\n", "\ncommands:\n")
        .lines()
        .filter_map(|line| line.strip_prefix("  - name: "))
        .collect();
    let lists = [
        TrainingError::DOMAIN_ERRORS,
        ExportError::DOMAIN_ERRORS,
        ReadError::DOMAIN_ERRORS,
    ];
    for name in lists.iter().copied().flatten() {
        assert!(declared.contains(name), "{name} is not declared in spec/");
    }
    let named = |examples: Vec<(Option<&'static str>, String)>| -> Vec<&'static str> {
        examples
            .into_iter()
            .map(|(error, message)| {
                assert!(
                    !message.is_empty() && !message.contains('\n'),
                    "{message:?}"
                );
                error.unwrap_or_else(|| panic!("{message} names no domain error"))
            })
            .collect()
    };
    assert_eq!(
        named(
            TrainingError::examples()
                .iter()
                .map(|e| (e.domain_error(), e.to_string()))
                .collect()
        ),
        TrainingError::DOMAIN_ERRORS,
        "one example an error"
    );
    assert_eq!(
        named(
            ExportError::examples()
                .iter()
                .map(|e| (e.domain_error(), e.to_string()))
                .collect()
        ),
        ExportError::DOMAIN_ERRORS,
        "one example an error"
    );
    assert_eq!(
        named(
            ReadError::examples()
                .iter()
                .map(|e| (e.domain_error(), e.to_string()))
                .collect()
        ),
        ReadError::DOMAIN_ERRORS,
        "one example an error"
    );
    let not_a_verdict = TrainingError::ModelSpec(ModelSpecError::NotFound {
        searched: Vec::new(),
    });
    assert_eq!(not_a_verdict.domain_error(), None);
}

/// Each command's refusals, observed by driving it, and the errors listed as not emitted, are
/// together exactly the errors the domain declares for that command; the observed ones are
/// [`TrainingError::DOMAIN_ERRORS`] and [`ExportError::DOMAIN_ERRORS`].
#[test]
fn every_error_of_the_five_commands_is_emitted_or_accounted_for() {
    let spec = spec();
    let mut declared: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (command, next) in COMMANDS {
        declared.insert(command, declared_by(&spec, command, next));
    }
    let not_emitted: BTreeSet<&str> = NOT_EMITTED_BY_THE_CRATE.iter().map(|(n, _)| *n).collect();
    let mut observed: BTreeMap<&str, BTreeSet<String>> = COMMANDS
        .iter()
        .map(|(command, _)| (*command, BTreeSet::new()))
        .collect();
    for (command, name, error) in driven() {
        let name = name
            .unwrap_or_else(|| panic!("{command} refused with {error}, which is no domain error"));
        observed.get_mut(command).unwrap().insert(name.to_owned());
    }
    for (command, errors) in &declared {
        let mut accounted: BTreeSet<String> = observed[command].clone();
        for name in errors {
            if not_emitted.contains(name.as_str()) {
                assert!(
                    accounted.insert(name.clone()),
                    "{name} is both emitted by {command} and listed as not emitted"
                );
            }
        }
        assert_eq!(&accounted, errors, "{command}");
    }

    let all_declared: BTreeSet<&str> = declared.values().flatten().map(String::as_str).collect();
    assert_eq!(all_declared.len(), 20, "{all_declared:?}");
    let all_observed: BTreeSet<&str> = observed.values().flatten().map(String::as_str).collect();
    let listed: BTreeSet<&str> = TrainingError::DOMAIN_ERRORS
        .iter()
        .chain(ExportError::DOMAIN_ERRORS)
        .copied()
        .collect();
    assert_eq!(all_observed, listed);
    assert_eq!(all_observed.len(), 16);
    assert!(all_observed.is_disjoint(&not_emitted));
    let export: BTreeSet<&str> = ExportError::DOMAIN_ERRORS.iter().copied().collect();
    assert_eq!(
        observed["ExportWeights"]
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        export
    );
}

/// `Predict` declares `CorruptWeightArtifact`; [`WeightArtifact::read`], the reader `Predict`
/// will call, emits it for every artifact it refuses, and it is the reader's only domain error.
/// `Predict`'s other errors are the native predictor's (story:native-predictor).
#[test]
fn corrupt_weight_artifact_is_emitted_by_the_reader_for_predict() {
    let spec = spec();
    let predict = declared_by(&spec, "Predict", "\nevents:\n");
    assert!(
        predict.contains("patchback.model.CorruptWeightArtifact"),
        "{predict:?}"
    );
    assert_eq!(
        ReadError::DOMAIN_ERRORS,
        ["patchback.model.CorruptWeightArtifact"]
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-domain-read");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in [
        ("empty", b"".as_slice()),
        ("weights-2", b"weights/2\n\n".as_slice()),
        ("text", b"param @PatchRegressor.w1\n".as_slice()),
    ] {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        match WeightArtifact::read(&path) {
            Err(error) => assert_eq!(
                error.domain_error(),
                Some("patchback.model.CorruptWeightArtifact"),
                "{name}: {error:?}"
            ),
            Ok(artifact) => panic!("{name}: {artifact:?}"),
        }
    }
    match WeightArtifact::read(&dir.join("missing")) {
        Err(error @ ReadError::Io { .. }) => assert_eq!(error.domain_error(), None),
        other => panic!("{other:?}"),
    }
}
