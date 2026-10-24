//! Every refusal the crate can emit names an error of `spec/domains/analysis.yaml`, so a reader
//! of the specification can reach each one, and every error the `AnalyzeAudio` command declares
//! is one the crate can emit. The same holds for [`FeatureError`] and the five feature contract
//! commands (story:feature-contract).

use patchback_analysis::{AnalysisError, FeatureError};

fn section<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    text.split_once(start)
        .unwrap_or_else(|| panic!("analysis.yaml has {start:?}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("analysis.yaml has {end:?} after {start:?}"))
        .0
}

fn domain() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/domains/analysis.yaml"
    );
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn every_refusal_names_an_error_of_the_analysis_domain() {
    let text = domain();
    let declared: Vec<&str> = section(&text, "\nerrors:\n", "\ncommands:\n")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .collect();
    for name in AnalysisError::DOMAIN_ERRORS {
        assert!(
            declared.contains(name),
            "{name} is not an error of spec/domains/analysis.yaml"
        );
    }
}

#[test]
fn every_error_of_analyze_audio_is_emitted() {
    let text = domain();
    let command = section(
        &text,
        "  - name: patchback.analysis.AnalyzeAudio\n",
        "\n  - name: patchback.analysis.DefineFeatureContract\n",
    );
    let errors: Vec<&str> = command
        .lines()
        .filter_map(|line| line.trim().strip_prefix("error: "))
        .collect();
    assert_eq!(errors.len(), 12, "{errors:?}");
    for name in errors {
        assert!(
            AnalysisError::DOMAIN_ERRORS.contains(&name),
            "AnalyzeAudio declares {name}, which the crate never emits"
        );
    }
}

#[test]
fn every_variant_maps_into_the_declared_list() {
    let mut seen = Vec::new();
    for error in AnalysisError::examples() {
        let name = error.domain_error();
        assert!(
            AnalysisError::DOMAIN_ERRORS.contains(&name),
            "{error:?} maps to {name}"
        );
        seen.push(name);
    }
    seen.sort_unstable();
    let mut all = AnalysisError::DOMAIN_ERRORS.to_vec();
    all.sort_unstable();
    assert_eq!(seen, all);
}

/// The fields of an error in the domain, `audio_clip_id` aside (the crate takes samples, not a
/// clip, so it never knows the id), are the fields of its Rust variant, by name and in order.
#[test]
fn error_fields_agree_with_the_rust_variants() {
    let text = domain();
    let errors = section(&text, "\nerrors:\n", "\ncommands:\n");
    for (name, rust_fields) in [
        (
            "patchback.analysis.FrameCountOverBound",
            &["fft_length", "hop_length", "frame_count"][..],
        ),
        ("patchback.analysis.PitchOutOfRange", &["bound_hz"][..]),
        (
            "patchback.analysis.ClipTooShortForPitch",
            &["frame_count", "required_frame_count"][..],
        ),
    ] {
        let start = format!("  - name: {name}\n");
        let body = errors
            .split_once(start.as_str())
            .unwrap_or_else(|| panic!("analysis.yaml declares {name}"))
            .1;
        let body = body.split_once("\n  - name: ").map_or(body, |(b, _)| b);
        let fields: Vec<&str> = body
            .lines()
            .filter_map(|line| line.strip_prefix("      - name: "))
            .filter(|field| *field != "audio_clip_id")
            .collect();
        assert_eq!(fields, rust_fields, "{name}");
    }
}

/// The feature contract commands, from `DefineFeatureContract` to the command after
/// `CheckFeatureVector`.
fn feature_commands(text: &str) -> &str {
    section(
        text,
        "  - name: patchback.analysis.DefineFeatureContract\n",
        "\n  - name: patchback.analysis.ComputeNormalizationStats\n",
    )
}

#[test]
fn every_feature_refusal_names_an_error_of_the_analysis_domain() {
    let text = domain();
    let declared: Vec<&str> = section(&text, "\nerrors:\n", "\ncommands:\n")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .collect();
    for name in FeatureError::DOMAIN_ERRORS {
        assert!(
            declared.contains(name),
            "{name} is not an error of spec/domains/analysis.yaml"
        );
    }
}

/// Every error the five feature contract commands declare is one the crate emits, and every
/// error [`FeatureError`] names is declared by one of them.
#[test]
fn every_error_of_the_feature_commands_is_emitted() {
    let text = domain();
    let commands = feature_commands(&text);
    // the section opens with DefineFeatureContract's own name line; the other four follow it
    for command in [
        "AmendFeatureContract",
        "PublishFeatureContract",
        "ExtractFeatures",
        "CheckFeatureVector",
    ] {
        assert!(
            commands.contains(&format!("name: patchback.analysis.{command}\n")),
            "{command} lies between DefineFeatureContract and ComputeNormalizationStats"
        );
    }
    let errors: Vec<&str> = commands
        .lines()
        .filter_map(|line| line.trim().strip_prefix("error: "))
        .collect();
    assert_eq!(errors.len(), 14, "{errors:?}");
    for name in &errors {
        assert!(
            FeatureError::DOMAIN_ERRORS.contains(name),
            "a feature contract command declares {name}, which the crate never emits"
        );
    }
    for name in FeatureError::DOMAIN_ERRORS {
        assert!(
            errors.contains(name),
            "FeatureError names {name}, which no feature contract command declares"
        );
    }
}

#[test]
fn every_feature_variant_maps_into_the_declared_list() {
    let mut seen = Vec::new();
    for error in FeatureError::examples() {
        let name = error.domain_error();
        assert!(
            FeatureError::DOMAIN_ERRORS.contains(&name),
            "{error:?} maps to {name}"
        );
        assert!(!error.to_string().is_empty(), "{error:?}");
        seen.push(name);
    }
    seen.sort_unstable();
    let mut all = FeatureError::DOMAIN_ERRORS.to_vec();
    all.sort_unstable();
    assert_eq!(seen, all);
}

/// The fields of each feature error in the domain, `analysis_id` aside (an [`Analysis`] carries
/// no identity, so the crate never knows it), lead the fields of its Rust variant, by name and
/// in order. The Rust variant may carry diagnostics after them: which value and why.
///
/// [`Analysis`]: patchback_analysis::Analysis
#[test]
fn feature_error_fields_lead_the_rust_variants() {
    let text = domain();
    let errors = section(&text, "\nerrors:\n", "\ncommands:\n");
    for (name, rust_fields) in [
        ("patchback.analysis.FeatureVersionTaken", &["version"][..]),
        (
            "patchback.analysis.FeatureCountMismatch",
            &["version", "value_count"][..],
        ),
        (
            "patchback.analysis.FeatureContractFrozen",
            &["feature_contract_id"][..],
        ),
        (
            "patchback.analysis.FeatureContractNotFound",
            &["feature_contract_id"][..],
        ),
        (
            "patchback.analysis.FeatureContractNotPublished",
            &["feature_contract_id"][..],
        ),
        (
            "patchback.analysis.FeatureNotComputable",
            &["feature", "index", "reason"][..],
        ),
        (
            "patchback.analysis.NonFiniteFeature",
            &["feature_contract_id", "index"][..],
        ),
        (
            "patchback.analysis.FeatureVersionMismatch",
            &["version"][..],
        ),
        (
            "patchback.analysis.FeatureLengthMismatch",
            &["value_count"][..],
        ),
    ] {
        assert!(FeatureError::DOMAIN_ERRORS.contains(&name), "{name}");
        let start = format!("  - name: {name}\n");
        let body = errors
            .split_once(start.as_str())
            .unwrap_or_else(|| panic!("analysis.yaml declares {name}"))
            .1;
        let body = body.split_once("\n  - name: ").map_or(body, |(b, _)| b);
        let fields: Vec<&str> = body
            .lines()
            .filter_map(|line| line.strip_prefix("      - name: "))
            .filter(|field| *field != "analysis_id")
            .collect();
        assert!(
            rust_fields.starts_with(&fields),
            "{name}: domain {fields:?}, Rust {rust_fields:?}"
        );
    }
}
