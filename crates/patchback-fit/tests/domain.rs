//! Every refusal of the comparison names an error the specification declares
//! (`spec/domains/fit.yaml`, or `spec/domains/analysis.yaml` for the analysis refusals it passes
//! on), and every error `CompareAudio` declares is emitted: by this crate, or, for
//! `ClipNotFound`, by `patchback compare` (`crates/patchback-cli/tests/compare.rs`), which knows
//! the clips by path.

use patchback_fit::CompareError;

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
