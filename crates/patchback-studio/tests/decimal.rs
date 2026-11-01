//! Every decimal the export writes is an ESS `Decimal` (`studio/src/generated/source.schema.json`:
//! `^-?(0|[1-9][0-9]*)(\.[0-9]+)?$`), never an exponent form, and reads back to the same `f64`
//! (wave c1, correction 2 item 6).

mod common;

use common::{demo_patch, fit_of, json, run_dir};
use patchback_studio::decimal;
use patchback_studio::export::export_run;
use serde_json::Value;

/// The ESS `Decimal` pattern, checked by hand (no regex crate).
fn is_ess_decimal(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (unsigned, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(whole) && (whole == "0" || !whole.starts_with('0')) && fraction.is_none_or(digits)
}

#[test]
fn a_decimal_is_plain_and_reads_back_to_the_same_f64() {
    let values: [f64; 14] = [
        0.0,
        -0.0,
        1.0,
        -8.0,
        0.5,
        1e-3,
        0.0030000000000000005,
        5000.000000000002,
        1e-7,
        1.2345678901234567e-308,
        5e-324,
        1e21,
        -1.7976931348623157e308,
        0.1 + 0.2,
    ];
    for value in values {
        let text = decimal(value);
        assert!(is_ess_decimal(&text), "{value:e}: {text}");
        let back: f64 = text.parse().expect("reads back");
        assert_eq!(back.to_bits(), value.to_bits(), "{text}");
    }
    assert_eq!(decimal(1e-3), "0.001");
    assert_eq!(decimal(0.25), "0.25");
    assert_eq!(decimal(16000.0), "16000");
}

/// Walks a document and returns every string whose key names a decimal member of the schema.
fn decimals(value: &Value, key: &str, out: &mut Vec<String>) {
    const DECIMAL_KEYS: [&str; 10] = [
        "physical_at_zero",
        "physical_at_one",
        "default_normalized",
        "total_loss",
        "best_loss",
        "log_spectral",
        "envelope",
        "transient",
        "band_energy",
        "normalized_value",
    ];
    match value {
        Value::String(s) if DECIMAL_KEYS.contains(&key) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| decimals(v, key, out)),
        Value::Object(map) => map.iter().for_each(|(k, v)| decimals(v, k, out)),
        _ => {}
    }
}

#[test]
fn every_decimal_of_an_export_is_an_ess_decimal() {
    let dir = run_dir("decimals", &fit_of(&demo_patch()), None, None);
    let document = json(&export_run(&dir).expect("exports"));
    let mut found = Vec::new();
    decimals(&document, "", &mut found);
    assert!(found.len() > 20, "{}", found.len());
    let bad: Vec<&String> = found.iter().filter(|d| !is_ess_decimal(d)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}
