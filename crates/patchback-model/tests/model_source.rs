//! The story's numeric obligation on `models/patch_regressor.mspec` (DESIGN.md § 8.1): only the
//! documented M0 operations (§ 1.1) and the static shapes `[16, 32]`, `[32, 64]`, `[64, 8]` and
//! `[16, 8]`.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model() -> String {
    fs::read_to_string(repository().join("models/patch_regressor.mspec")).unwrap()
}

/// The operations DESIGN.md § 1.1 lists for ModelSpec M0.
const M0_OPERATIONS: [&str; 9] = [
    "matmul",
    "add",
    "sub",
    "mul",
    "relu",
    "transpose",
    "sum",
    "mean",
    "mse",
];

/// What § 8.1 calls that is not a tensor operation: the parameter initializer and the optimizer.
const NOT_OPERATIONS: [&str; 2] = ["xavier_uniform", "sgd"];

/// Every `name(` in `source` that is a call: not a `fn name(` definition and not a method call
/// `.name(` on the model.
fn calls(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes = source.as_bytes();
    for (index, _) in source.match_indices('(') {
        let start = source[..index]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(0, |i| i + 1);
        let name = &source[start..index];
        if name.is_empty() {
            continue;
        }
        let before = source[..start].trim_end();
        if before.ends_with("fn") || (start > 0 && bytes[start - 1] == b'.') {
            continue;
        }
        found.insert(name.to_owned());
    }
    found
}

/// Every `Tensor<f32, [..]>` shape in `source`, whitespace removed.
fn shapes(source: &str) -> BTreeSet<String> {
    source
        .split("Tensor<")
        .skip(1)
        .map(|rest| {
            let shape = &rest[rest.find('[').unwrap()..=rest.find(']').unwrap()];
            assert!(rest[..rest.find('[').unwrap()].trim() == "f32,", "{rest}");
            shape.split_whitespace().collect()
        })
        .collect()
}

#[test]
fn the_model_uses_only_m0_operations() {
    let source = model();
    let calls = calls(&source);
    let unknown: Vec<_> = calls
        .iter()
        .filter(|c| !M0_OPERATIONS.contains(&c.as_str()) && !NOT_OPERATIONS.contains(&c.as_str()))
        .collect();
    assert!(unknown.is_empty(), "not M0 operations: {unknown:?}");
    for used in ["matmul", "relu", "mse"] {
        assert!(calls.contains(used), "{used} not called");
    }
}

#[test]
fn the_model_has_exactly_the_static_shapes_of_section_8_1() {
    let expected: BTreeSet<String> = ["[16,32]", "[32,64]", "[64,8]", "[16,8]"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(shapes(&model()), expected);
}

#[test]
fn the_operation_scan_sees_a_call_outside_m0() {
    let calls = calls("let h = softmax(matmul(x, w1)); fn forward(x) { model.forward(x) }");
    assert_eq!(
        calls,
        ["matmul", "softmax"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn the_model_is_the_design_section_8_1_source_verbatim() {
    let design = fs::read_to_string(repository().join("DESIGN.md")).unwrap();
    let section = &design[design.find("### 8.1").unwrap()..];
    let start = section.find("```mspec\n").unwrap() + "```mspec\n".len();
    let length = section[start..].find("```").unwrap();
    assert_eq!(model(), section[start..start + length]);
}
