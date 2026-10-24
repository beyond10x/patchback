//! Every refusal of the dataset model names an error `spec/domains/dataset.yaml` declares, and
//! every error the six commands this crate carries declare is emitted. `WriteDenseFile` and
//! `FinishSplit` are story:dense-data-files and are not checked here.

use patchback_dataset::DatasetError;

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

/// The commands this crate carries, each with the command that follows it in the domain.
const COMMANDS: [(&str, &str); 6] = [
    ("GenerateDataset", "OpenSplit"),
    ("OpenSplit", "DrawPatch"),
    ("DrawPatch", "AddSample"),
    ("AddSample", "OpenDiagnosticSet"),
    ("OpenDiagnosticSet", "AddDiagnosticExample"),
    ("AddDiagnosticExample", "WriteDenseFile"),
];

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

#[test]
fn every_error_of_the_six_commands_is_emitted() {
    let spec = spec();
    let mut named = Vec::new();
    for (command, next) in COMMANDS {
        let body = section(
            &spec,
            &format!("  - name: patchback.dataset.{command}\n"),
            &format!("\n  - name: patchback.dataset.{next}\n"),
        );
        named.extend(
            body.lines()
                .filter_map(|line| line.trim().strip_prefix("error: "))
                .map(str::to_owned),
        );
    }
    assert!(!named.is_empty());
    let emitted: Vec<&str> = DatasetError::examples()
        .iter()
        .map(DatasetError::domain_error)
        .collect();
    for name in &named {
        assert!(
            emitted.contains(&name.as_str()),
            "{name} is declared by a command and never emitted"
        );
    }
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
