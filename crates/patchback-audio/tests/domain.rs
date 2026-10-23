//! Every refusal the crate can emit names an error of `spec/domains/audio.yaml`, so a reader of
//! the specification can reach each one.

use patchback_audio::WavError;

fn domain_errors() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/domains/audio.yaml");
    let text = std::fs::read_to_string(path).unwrap();
    let errors = text
        .split_once("\nerrors:\n")
        .expect("audio.yaml has an errors section")
        .1;
    let errors = errors
        .split_once("\ncommands:\n")
        .expect("audio.yaml has a commands section after errors")
        .0;
    errors
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_refusal_names_an_error_of_the_audio_domain() {
    let declared = domain_errors();
    assert!(!WavError::DOMAIN_ERRORS.is_empty());
    for name in WavError::DOMAIN_ERRORS {
        assert!(
            declared.iter().any(|d| d == name),
            "{name} is not an error of spec/domains/audio.yaml"
        );
    }
}

#[test]
fn every_variant_maps_into_the_declared_list() {
    for error in WavError::examples() {
        assert!(
            WavError::DOMAIN_ERRORS.contains(&error.domain_error()),
            "{error:?} maps to {}",
            error.domain_error()
        );
    }
}
