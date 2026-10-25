//! Every refusal of the dataset model names an error `spec/domains/dataset.yaml` declares, and
//! every error the eight commands of the domain declare (the six of story:dataset-generation and
//! `WriteDenseFile` and `FinishSplit` of story:dense-data-files) is emitted by driving that
//! command through [`DatasetStore`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use patchback_analysis::FeatureContractId;
use patchback_core::prng::NAME;
use patchback_dataset::{
    DatasetError, DatasetId, DatasetSplitId, DatasetStore, DenseFileKind, DiagnosticCase,
    DiagnosticCause, DiagnosticSetId, GenerateDataset, PrngAlgorithm, SampledPatchId, SplitRole,
    WriteDenseFile,
};

fn spec() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/dataset.yaml"
    );
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

/// The commands this crate carries, each with what follows it in the domain: the next command's
/// heading, or for the last command the `events:` section.
const COMMANDS: [(&str, &str); 8] = [
    (
        "GenerateDataset",
        "\n  - name: patchback.dataset.OpenSplit\n",
    ),
    ("OpenSplit", "\n  - name: patchback.dataset.DrawPatch\n"),
    ("DrawPatch", "\n  - name: patchback.dataset.AddSample\n"),
    (
        "AddSample",
        "\n  - name: patchback.dataset.OpenDiagnosticSet\n",
    ),
    (
        "OpenDiagnosticSet",
        "\n  - name: patchback.dataset.AddDiagnosticExample\n",
    ),
    (
        "AddDiagnosticExample",
        "\n  - name: patchback.dataset.WriteDenseFile\n",
    ),
    (
        "WriteDenseFile",
        "\n  - name: patchback.dataset.FinishSplit\n",
    ),
    ("FinishSplit", "\nevents:\n"),
];

/// The commands of the domain, in order: the eight above, and no other.
#[test]
fn the_domain_declares_exactly_these_commands() {
    let spec = spec();
    let declared: Vec<&str> = section(&spec, "\ncommands:\n", "\nevents:\n")
        .lines()
        .filter_map(|line| line.strip_prefix("  - name: patchback.dataset."))
        .collect();
    let carried: Vec<&str> = COMMANDS.iter().map(|(command, _)| *command).collect();
    assert_eq!(declared, carried);
}

#[test]
fn every_refusal_names_a_declared_error() {
    let spec = spec();
    let declared: Vec<&str> = section(&spec, "\nerrors:\n", "\ncommands:\n")
        .lines()
        .filter_map(|line| line.strip_prefix("  - name: "))
        .collect();
    for name in DatasetError::DOMAIN_ERRORS {
        assert!(declared.contains(name), "{name} is not declared in spec/");
    }
    for example in DatasetError::examples() {
        assert!(
            DatasetError::DOMAIN_ERRORS.contains(&example.domain_error()),
            "{example:?} names {} outside DOMAIN_ERRORS",
            example.domain_error()
        );
    }
}

/// `GenerateDataset` as every scenario gives it (`tests/scenarios.rs`): seed 42, batch size 16,
/// `data/`.
fn generate(store: &mut DatasetStore) -> DatasetId {
    store.generate_dataset(GenerateDataset {
        seed: 42,
        prng_algorithm: PrngAlgorithm(NAME.to_owned()),
        batch_size: 16,
        feature_contract_id: FeatureContractId(1),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    })
}

/// A `WriteDenseFile` of a validation split (`tests/dense_scenarios.rs`).
fn input(
    split: DatasetSplitId,
    kind: DenseFileKind,
    rows: i64,
    columns: i64,
    byte_count: i64,
) -> WriteDenseFile {
    WriteDenseFile {
        dataset_split_id: split,
        kind,
        path: PathBuf::from("data/val").join(kind.file_name()),
        rows,
        columns,
        byte_count,
    }
}

/// `count` little-endian `f32` values, `value(i)` each (`tests/dense_scenarios.rs`).
fn f32_content(count: usize, value: impl Fn(usize) -> f32) -> Vec<u8> {
    (0..count).flat_map(|i| value(i).to_le_bytes()).collect()
}

/// Each command of the domain driven, through [`DatasetStore`], to each refusal the domain
/// declares for it: the command's name and the refusal it returned. A command that was not
/// refused contributes nothing, so the test below finds the refusal missing.
fn driven() -> Vec<(&'static str, DatasetError)> {
    let mut refusals = Vec::new();
    let mut refused = |command: &'static str, result: Result<(), DatasetError>| {
        if let Err(error) = result {
            refusals.push((command, error));
        }
    };
    let mut store = DatasetStore::new();
    let dataset = generate(&mut store);
    let unknown_dataset = DatasetId(dataset.0 + 100);

    refused(
        "OpenSplit",
        store
            .open_split(unknown_dataset, SplitRole::Train, 16)
            .map(drop),
    );
    refused(
        "OpenSplit",
        store.open_split(dataset, SplitRole::Train, 17).map(drop),
    );
    let split = store
        .open_split(dataset, SplitRole::Validation, 16)
        .unwrap();
    let unknown_split = DatasetSplitId(split.0 + 100);

    let patch = store.draw_patch(dataset, split, 7).unwrap();
    refused("DrawPatch", store.draw_patch(dataset, split, 7).map(drop));

    refused(
        "AddSample",
        store
            .add_sample(
                SampledPatchId(patch.0 + 100),
                60,
                24_000,
                [0.25; 32],
                [0.5; 8],
            )
            .map(drop),
    );

    refused(
        "OpenDiagnosticSet",
        store.open_diagnostic_set(unknown_dataset).map(drop),
    );
    let set = store.open_diagnostic_set(dataset).unwrap();
    let case = DiagnosticCase {
        patch_seed: 9,
        midi_note: 60,
        gate_samples: 24_000,
        cause: DiagnosticCause::limited(12),
    };
    refused(
        "AddDiagnosticExample",
        store
            .add_diagnostic_example(DiagnosticSetId(set.0 + 100), case)
            .map(drop),
    );

    let x = f32_content(16 * 32, |_| 0.5);
    let y = f32_content(16 * 8, |_| 0.5);
    let nan_y = f32_content(16 * 8, |i| if i == 5 { f32::NAN } else { 0.5 });
    for (command, split, kind, rows, columns, byte_count, content) in [
        ("WriteDenseFile", split, DenseFileKind::X, 16, 31, 1_984, &x),
        (
            "WriteDenseFile",
            unknown_split,
            DenseFileKind::X,
            16,
            32,
            2_048,
            &x,
        ),
        ("WriteDenseFile", split, DenseFileKind::X, 15, 32, 1_920, &x),
        ("WriteDenseFile", split, DenseFileKind::X, 16, 32, 2_044, &x),
        (
            "WriteDenseFile",
            split,
            DenseFileKind::Y,
            16,
            8,
            512,
            &nan_y,
        ),
    ] {
        refused(
            command,
            store
                .write_dense_file(input(split, kind, rows, columns, byte_count), content)
                .map(drop),
        );
    }

    refused("FinishSplit", store.finish_split(unknown_split));
    refused("FinishSplit", store.finish_split(split));
    store
        .write_dense_file(input(split, DenseFileKind::X, 16, 32, 2_048), &x)
        .unwrap();
    store
        .write_dense_file(input(split, DenseFileKind::Y, 16, 8, 512), &y)
        .unwrap();
    store.finish_split(split).unwrap();
    refused("FinishSplit", store.finish_split(split));
    refusals
}

/// Each command's refusals, observed by driving the command (not read from a list the crate
/// builds), name exactly the errors the domain declares for that command; together they are the
/// twelve the domain declares and [`DatasetError::DOMAIN_ERRORS`] lists.
#[test]
fn every_error_of_the_eight_commands_is_emitted() {
    let spec = spec();
    let mut declared: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (command, next) in COMMANDS {
        let body = section(
            &spec,
            &format!("  - name: patchback.dataset.{command}\n"),
            next,
        );
        declared.insert(
            command,
            body.lines()
                .filter_map(|line| line.trim().strip_prefix("error: "))
                .map(str::to_owned)
                .collect(),
        );
    }
    let mut observed: BTreeMap<&str, BTreeSet<String>> = COMMANDS
        .iter()
        .map(|(command, _)| (*command, BTreeSet::new()))
        .collect();
    for (command, error) in driven() {
        observed
            .get_mut(command)
            .unwrap_or_else(|| panic!("{command} is not a command of the domain"))
            .insert(error.domain_error().to_owned());
    }
    assert_eq!(observed, declared, "the refusals each command returned");

    let all_declared: BTreeSet<&str> = declared.values().flatten().map(String::as_str).collect();
    let all_observed: BTreeSet<&str> = observed.values().flatten().map(String::as_str).collect();
    assert_eq!(all_observed, all_declared);
    let listed: BTreeSet<&str> = DatasetError::DOMAIN_ERRORS.iter().copied().collect();
    assert_eq!(all_observed, listed);
    assert_eq!(all_observed.len(), 12);
}

#[test]
fn every_example_is_one_line_naming_its_values() {
    for example in DatasetError::examples() {
        let message = example.to_string();
        assert!(
            !message.is_empty() && !message.contains('\n'),
            "{message:?}"
        );
    }
}
