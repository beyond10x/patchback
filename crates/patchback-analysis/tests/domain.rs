//! Every refusal the crate can emit names an error of `spec/domains/analysis.yaml`, so a reader
//! of the specification can reach each one, and every error the `AnalyzeAudio` command declares
//! is one the crate can emit.

use patchback_analysis::AnalysisError;

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
