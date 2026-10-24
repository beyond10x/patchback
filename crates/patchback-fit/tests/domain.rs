//! Every refusal of the comparison names an error the specification declares
//! (`spec/domains/fit.yaml`, or `spec/domains/analysis.yaml` for the analysis refusals it passes
//! on), and every error `CompareAudio` declares is emitted: by this crate, or, for
//! `ClipNotFound`, by `patchback compare` (`crates/patchback-cli/tests/compare.rs`), which knows
//! the clips by path.
//!
//! The same holds for the fit (story:bounded-search-fit): every [`FitError`] names a declared
//! error, and every error the commands of a fit run declare (`StartFit`, `EvaluateCandidate`,
//! `RejectCandidate`, `CompleteFit`, `FailFit`) is emitted by this crate, by `patchback fit`, or
//! is listed below with the reason no a6 path reaches it.

use patchback_fit::{CompareError, FitError};

fn read(domain: &str) -> String {
    let path = format!(
        "{}/../../spec/domains/{domain}.yaml",
        env!("CARGO_MANIFEST_DIR")
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

fn declared_errors(text: &str) -> Vec<&str> {
    section(text, "\nerrors:\n", "\ncommands:\n")
        .lines()
        .filter_map(|line| line.strip_prefix("  - name: "))
        .collect()
}

#[test]
fn every_refusal_names_a_declared_error() {
    let fit = read("fit");
    let analysis = read("analysis");
    let mut declared = declared_errors(&fit);
    declared.extend(declared_errors(&analysis));
    for name in CompareError::DOMAIN_ERRORS {
        assert!(declared.contains(name), "{name} is not declared in spec/");
    }
}

#[test]
fn every_error_of_compare_audio_is_emitted() {
    let fit = read("fit");
    let command = section(
        &fit,
        "  - name: patchback.fit.CompareAudio\n",
        "\n  - name: patchback.fit.StartFit\n",
    );
    let errors: Vec<&str> = command
        .lines()
        .filter_map(|line| line.trim().strip_prefix("error: "))
        .collect();
    assert_eq!(
        errors,
        [
            "patchback.fit.ClipNotFound",
            "patchback.fit.ClipNotFound",
            "patchback.fit.SampleRateMismatch",
            "patchback.fit.LengthOutsideAlignment",
        ]
    );
    for name in errors {
        if name != "patchback.fit.ClipNotFound" {
            assert!(
                CompareError::DOMAIN_ERRORS.contains(&name),
                "CompareAudio declares {name}, which the crate never emits"
            );
        }
    }
}

#[test]
fn every_variant_maps_into_the_declared_list() {
    let mut seen: Vec<&str> = CompareError::examples()
        .iter()
        .map(CompareError::domain_error)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    let mut all = CompareError::DOMAIN_ERRORS.to_vec();
    all.sort_unstable();
    assert_eq!(seen, all);
}

#[test]
fn every_fit_refusal_names_a_declared_error() {
    let fit = read("fit");
    let analysis = read("analysis");
    let mut declared = declared_errors(&fit);
    declared.extend(declared_errors(&analysis));
    for name in FitError::DOMAIN_ERRORS {
        assert!(declared.contains(name), "{name} is not declared in spec/");
    }
}

#[test]
fn every_fit_variant_maps_into_the_declared_list() {
    let mut seen: Vec<&str> = FitError::examples()
        .iter()
        .map(FitError::domain_error)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    let mut all = FitError::DOMAIN_ERRORS.to_vec();
    all.sort_unstable();
    assert_eq!(seen, all);
}

/// The errors of the fit-run commands no a6 code emits, each with its reason.
const NOT_EMITTED_BY_THE_CRATE: &[(&str, &str)] = &[
    (
        "patchback.fit.ClipNotFound",
        "patchback fit, which knows the reference by path (crates/patchback-cli/tests/fit.rs)",
    ),
    (
        "patchback.fit.NoteSequenceNotFound",
        "patchback fit, which knows the note sequence by path (crates/patchback-cli/tests/fit.rs)",
    ),
    (
        "patchback.fit.FitRunNotFound",
        "a run is a value; no command can name a run that does not exist",
    ),
    (
        "patchback.fit.NoteConfidenceMissing",
        "estimated note sequences are Milestone C; a6 fits known notes only (D-a6-1)",
    ),
    (
        "patchback.fit.AnalysisRequired",
        "regressor-seeded fits are Milestone B; a6 starts from the default patch",
    ),
    (
        "patchback.fit.AnalysisNotFound",
        "regressor-seeded fits are Milestone B; a6 names no analysis",
    ),
    (
        "patchback.fit.FeatureContractMismatch",
        "regressor-seeded fits are Milestone B; a6 names no weight artifact",
    ),
];

#[test]
fn every_error_of_the_fit_run_commands_is_emitted_or_accounted_for() {
    let fit = read("fit");
    let commands = section(
        &fit,
        "  - name: patchback.fit.StartFit\n",
        "\n  - name: patchback.fit.WriteFitResults\n",
    );
    let mut errors: Vec<&str> = commands
        .lines()
        .filter_map(|line| line.trim().strip_prefix("error: "))
        .collect();
    errors.sort_unstable();
    errors.dedup();
    assert_eq!(errors.len(), 14, "{errors:?}");
    for name in errors {
        let emitted = FitError::DOMAIN_ERRORS.contains(&name);
        let accounted = NOT_EMITTED_BY_THE_CRATE.iter().any(|(n, _)| *n == name);
        assert!(
            emitted != accounted,
            "{name}: emitted {emitted}, listed as not emitted {accounted}"
        );
    }
}
