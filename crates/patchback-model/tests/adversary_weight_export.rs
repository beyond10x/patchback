//! Adversary pass 1 on story:weight-export (wave a9): `WeightArtifact::export` and `read`
//! driven through the public API at the boundaries the unit's own tests leave open.
//!
//! The run comes from one stand-in `modelspec` script that prints the file `out-<steps>.txt`
//! (the `--steps` argument picks the stdout, so every case writes its own stdout file and no
//! script is written after the first spawn) and exits 0 or 1. The data split is an in-memory
//! `DatasetStore` split as `tests/domain.rs` builds it; the stand-in reads no data.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use patchback_core::digest;
use patchback_dataset::{
    DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset, Pipeline, PrngAlgorithm,
    SplitRole, WriteDenseFile,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ExportError, MAX_HEADER_BYTES, ModelSource, ModelSpec,
    ModelSpecPin, OutputError, PAYLOAD_BYTES, RECOMMENDED_FILE_NAME, ReadError, TrainingRun,
    TrainingRunState, W1_ELEMENTS, W1_NAME, W1_SHAPE, W2_ELEMENTS, W2_NAME, W2_SHAPE,
    WeightArtifact,
};

fn pin() -> ModelSpecPin {
    ModelSpecPin::repository().unwrap()
}

fn design_model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/patch_regressor.mspec")
}

fn values(count: usize, salt: usize) -> Vec<f32> {
    (0..count)
        .map(|index| ((index * 7919 + salt * 104_729) % 4001) as f32 / 3001.0 - 0.6667)
        .collect()
}

fn w1() -> Vec<f32> {
    values(W1_ELEMENTS, 1)
}

fn w2() -> Vec<f32> {
    values(W2_ELEMENTS, 2)
}

fn tokens(values: &[f32]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn line(kind: &str, name: &str, shape: [usize; 2], tokens: &[String]) -> String {
    format!(
        "{kind} {name}: Tensor<f32, [{}, {}]> = [{}]",
        shape[0],
        shape[1],
        tokens.join(", ")
    )
}

/// The documented stdout of a 4-step run: `init` w1 (line 1) and w2 (2), `step` (3, 4), `param`
/// w1 (5) and w2 (6).
fn documented() -> Vec<String> {
    vec![
        line("init", W1_NAME, W1_SHAPE, &tokens(&values(W1_ELEMENTS, 3))),
        line("init", W2_NAME, W2_SHAPE, &tokens(&values(W2_ELEMENTS, 4))),
        "step 0 loss 0.25".to_owned(),
        "step 4 loss 0.125".to_owned(),
        line("param", W1_NAME, W1_SHAPE, &tokens(&w1())),
        line("param", W2_NAME, W2_SHAPE, &tokens(&w2())),
    ]
}

fn join(lines: &[String]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// `documented()` with line `index` (0-based) replaced.
fn replaced(index: usize, with: String) -> String {
    let mut lines = documented();
    lines[index] = with;
    join(&lines)
}

/// `documented()` with `extra` lines appended.
fn appended(extra: &[&str]) -> String {
    let mut lines = documented();
    lines.extend(extra.iter().map(|line| (*line).to_owned()));
    join(&lines)
}

/// The documented stdout with w1's (`in_w1`) or w2's value tokens edited.
fn with_tokens(in_w1: bool, edit: impl FnOnce(&mut Vec<String>)) -> String {
    let (index, name, shape, mut list) = if in_w1 {
        (4, W1_NAME, W1_SHAPE, tokens(&w1()))
    } else {
        (5, W2_NAME, W2_SHAPE, tokens(&w2()))
    };
    edit(&mut list);
    replaced(index, line("param", name, shape, &list))
}

/// The documented stdout with the text of `param` line w1 (`in_w1`) or w2 edited.
fn with_param_text(in_w1: bool, edit: impl FnOnce(&str) -> String) -> String {
    let index = if in_w1 { 4 } else { 5 };
    let lines = documented();
    let edited = edit(&lines[index]);
    assert_ne!(edited, lines[index], "the edit changed nothing");
    replaced(index, edited)
}

/// The stand-ins: `exits-0` and `exits-1`. Both answer `--version` with the pinned version and
/// every `check` with 0; on `train` they print `out-<last argument>.txt` (the `--steps` value).
fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-export");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let version = pin().version().to_owned();
        for (name, status) in [("exits-0", 0), ("exits-1", 1)] {
            let path = dir.join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = check ]; then exit 0; fi\neval last=\\${{$#}}\ncat \"{}/out-$last.txt\"\nexit {status}\n",
                    dir.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    })
}

/// A step number no other case uses, so its stdout file is its own.
fn next_steps() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1000);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// A Written train split of 16 rows in a dataset of batch size 16 (`tests/domain.rs`'s `split`).
fn written_split(store: &mut DatasetStore) -> DatasetSplitId {
    let dataset = store.generate_dataset(GenerateDataset {
        seed: 1,
        prng_algorithm: PrngAlgorithm("xoshiro256**".to_owned()),
        batch_size: 16,
        feature_contract_id: Pipeline::new().feature_contract_id(),
        topology_id: "subtractive-mono/1".to_owned(),
        out_dir: PathBuf::from("data/"),
    });
    let count = 16;
    let split = store.open_split(dataset, SplitRole::Train, count).unwrap();
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
    split
}

/// A run started with a stand-in printing `stdout` and exiting `status`; finished when `finish`.
fn run_printing(stdout: &str, status: i32, finish: bool) -> TrainingRun {
    let dir = standins();
    let steps = next_steps();
    fs::write(dir.join(format!("out-{steps}.txt")), stdout).unwrap();
    let pin = pin();
    let modelspec = ModelSpec::open(&dir.join(format!("exits-{status}")), &pin).unwrap();
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    source.check(&modelspec).unwrap();
    let mut store = DatasetStore::new();
    let split = written_split(&mut store);
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run.start(
        &store,
        split,
        None,
        &modelspec,
        steps,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(run.stdout(), stdout, "the stand-in printed its file");
    if finish {
        run.finish().unwrap();
    }
    run
}

fn completed(stdout: &str) -> TrainingRun {
    let run = run_printing(stdout, 0, true);
    assert_eq!(run.state(), TrainingRunState::Completed);
    run
}

/// A fresh directory for one case, holding one sentinel file so its listing is not empty.
fn fresh(case: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-model-adversary-export-cases")
        .join(case);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("sentinel"), b"sentinel").unwrap();
    dir
}

fn listing(dir: &Path) -> Vec<(String, u64)> {
    let mut entries: Vec<(String, u64)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.metadata().unwrap().len(),
            )
        })
        .collect();
    entries.sort();
    entries
}

/// What an export answered, without the 2,560 values of an exported artifact.
#[derive(Debug, PartialEq)]
enum Verdict {
    Exported,
    NotCompleted(TrainingRunState),
    Unrecognized(Option<usize>),
    Parameter(String, String),
    Shape([usize; 4]),
    Count(usize, usize),
    NonFinite(&'static str, usize),
    Unwritable,
    Io(io::ErrorKind),
}

fn verdict(result: &Result<WeightArtifact, ExportError>) -> Verdict {
    match result {
        Ok(_) => Verdict::Exported,
        Err(ExportError::TrainingNotCompleted { state }) => Verdict::NotCompleted(*state),
        Err(ExportError::UnrecognizedTrainingOutput { line, .. }) => Verdict::Unrecognized(*line),
        Err(ExportError::UnrecognizedParameter { w1_name, w2_name }) => {
            Verdict::Parameter(w1_name.clone(), w2_name.clone())
        }
        Err(ExportError::WrongWeightShape {
            w1_rows,
            w1_columns,
            w2_rows,
            w2_columns,
        }) => Verdict::Shape([*w1_rows, *w1_columns, *w2_rows, *w2_columns]),
        Err(ExportError::WrongElementCount {
            w1_elements,
            w2_elements,
        }) => Verdict::Count(*w1_elements, *w2_elements),
        Err(ExportError::NonFiniteWeight { name, index, value }) => {
            assert!(!value.is_finite(), "{name} {index} {value}");
            Verdict::NonFinite(name, *index)
        }
        Err(ExportError::Unwritable { .. }) => Verdict::Unwritable,
        Err(ExportError::Io { error, .. }) => Verdict::Io(error.kind()),
    }
}

/// Exports `run` for `contract` into `dir`, and asserts a refusal left `dir`'s listing as it was.
fn export_into(
    run: &TrainingRun,
    contract: &str,
    dir: &Path,
) -> (Verdict, Result<WeightArtifact, ExportError>) {
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let before = listing(dir);
    let result = WeightArtifact::export(run, contract, &path);
    let after = listing(dir);
    let found = verdict(&result);
    if result.is_err() {
        assert_eq!(
            after,
            before,
            "the refusal {found:?} changed {}",
            dir.display()
        );
    } else {
        let _ = fs::remove_file(&path);
    }
    (found, result)
}

/// The verdict of exporting a Completed run that printed `stdout`.
fn probe(stdout: &str) -> Verdict {
    let dir = standins().join(format!("probe-{}", next_steps()));
    fs::create_dir_all(&dir).unwrap();
    export_into(&completed(stdout), "features/1", &dir).0
}

/// Da9-7 (state, grammar, names, shapes, counts, finiteness, write) with two defects at once:
/// the earlier check answers, and the parent directory is as it was.
#[test]
fn two_defects_answer_in_the_order_of_da9_7() {
    let dir = fresh("two-defects");
    let transposed_and_nan = {
        let mut lines = documented();
        lines[4] = line("param", W1_NAME, [64, 32], &tokens(&w1()));
        let mut nan = tokens(&w2());
        nan[9] = "NaN".to_owned();
        lines[5] = line("param", W2_NAME, W2_SHAPE, &nan);
        join(&lines)
    };
    let run = completed(&transposed_and_nan);
    assert_eq!(
        export_into(&run, "features/1", &dir).0,
        Verdict::Shape([64, 32, 64, 8])
    );
    // decision 5: the header check comes after Da9-7's checks.
    let long = "f".repeat(1000);
    assert_eq!(
        export_into(&run, &long, &dir).0,
        Verdict::Shape([64, 32, 64, 8])
    );

    let bias_and_short = {
        let mut lines = documented();
        lines[4] = line("param", "@PatchRegressor.b1", W1_SHAPE, &tokens(&w1()));
        lines[5] = line("param", W2_NAME, W2_SHAPE, &tokens(&w2()[..511]));
        join(&lines)
    };
    assert_eq!(
        export_into(&completed(&bias_and_short), "features/1", &dir).0,
        Verdict::Parameter("@PatchRegressor.b1".to_owned(), W2_NAME.to_owned())
    );

    let running = run_printing("garbage\n", 0, false);
    assert_eq!(running.state(), TrainingRunState::Running);
    assert_eq!(
        export_into(&running, "features/1", &dir).0,
        Verdict::NotCompleted(TrainingRunState::Running)
    );
    let failed = run_printing("garbage\n", 1, true);
    assert_eq!(failed.state(), TrainingRunState::Failed);
    assert_eq!(
        export_into(&failed, "", &dir).0,
        Verdict::NotCompleted(TrainingRunState::Failed)
    );

    // Da9-8: a Completed run whose output_error is Some fails the grammar, not the state.
    let mut lines = documented();
    lines.insert(3, "step 2 loss abc".to_owned());
    let malformed = completed(&join(&lines));
    assert!(matches!(
        malformed.output_error(),
        Some(OutputError::Malformed { line: 4, .. })
    ));
    assert_eq!(
        export_into(&malformed, "features/1", &dir).0,
        Verdict::Unrecognized(Some(4))
    );
    // Da9-8: a NaN loss with finite weights exports.
    let nan_loss = completed(&replaced(3, "step 4 loss NaN".to_owned()));
    assert_eq!(
        export_into(&nan_loss, "features/1", &dir).0,
        Verdict::Exported
    );

    // finiteness before the header's checks (decision 5).
    let nan_w1 = completed(&with_tokens(true, |list| list[0] = "NaN".to_owned()));
    for contract in ["features 1", "", "é", long.as_str()] {
        assert_eq!(
            export_into(&nan_w1, contract, &dir).0,
            Verdict::NonFinite(W1_NAME, 0),
            "{contract:?}"
        );
    }
    let good = completed(&join(&documented()));
    for contract in ["features 1", "", "é", long.as_str(), "features/1\n"] {
        assert_eq!(
            export_into(&good, contract, &dir).0,
            Verdict::Unwritable,
            "{contract:?}"
        );
    }
    assert_eq!(listing(&dir), vec![("sentinel".to_owned(), 8)]);
}

/// Da9-5 and Da9-6 on the lines the brief names: what each answers through `export`.
#[test]
fn the_grammar_on_the_named_lines() {
    let lines = documented();
    let joined = join(&lines);
    let mut unterminated = joined.clone();
    unterminated.pop();
    let crlf = format!("{}\r\n", lines.join("\r\n"));
    let w1_init_8x8 = line("init", W1_NAME, [8, 8], &tokens(&values(64, 7)));
    let w1_init_transposed = line("init", W1_NAME, [64, 32], &tokens(&values(W1_ELEMENTS, 3)));
    let mut w1_twice = lines.clone();
    w1_twice[5] = lines[4].clone();
    let mut swapped = lines.clone();
    swapped.swap(4, 5);
    let mut third = lines.clone();
    third.push(line(
        "param",
        "@PatchRegressor.b1",
        [8, 1],
        &tokens(&values(8, 5)),
    ));
    let one: Vec<String> = lines[..5].to_vec();
    let none: Vec<String> = lines[..4].to_vec();
    let cases: Vec<(&str, String, Verdict)> = vec![
        ("no final newline", unterminated, Verdict::Exported),
        (
            "two final newlines",
            format!("{joined}\n"),
            Verdict::Unrecognized(Some(7)),
        ),
        (
            "trailing space",
            replaced(2, "step 0 loss 0.25 ".to_owned()),
            Verdict::Unrecognized(Some(3)),
        ),
        (
            "tab",
            replaced(2, "step\t0 loss 0.25".to_owned()),
            Verdict::Unrecognized(Some(3)),
        ),
        ("CRLF", crlf, Verdict::Unrecognized(Some(1))),
        (
            "loss 1e-3",
            replaced(3, "step 4 loss 1e-3".to_owned()),
            Verdict::Exported,
        ),
        (
            "loss -0.5",
            replaced(3, "step 4 loss -0.5".to_owned()),
            Verdict::Exported,
        ),
        (
            "loss nan",
            replaced(3, "step 4 loss nan".to_owned()),
            Verdict::Exported,
        ),
        (
            "loss inf",
            replaced(3, "step 4 loss inf".to_owned()),
            Verdict::Exported,
        ),
        (
            "eval nan",
            appended(&["eval loss=nan accuracy=0"]),
            Verdict::Exported,
        ),
        (
            "two evals",
            appended(&["eval loss=0.5 accuracy=0", "eval loss=0.5 accuracy=0"]),
            Verdict::Unrecognized(Some(8)),
        ),
        (
            "step after param",
            appended(&["step 8 loss 0.1"]),
            Verdict::Exported,
        ),
        ("init [8, 8]", replaced(0, w1_init_8x8), Verdict::Exported),
        (
            "init [64, 32]",
            replaced(0, w1_init_transposed),
            Verdict::Exported,
        ),
        (
            "init b1",
            replaced(0, lines[0].replacen(W1_NAME, "@PatchRegressor.b1", 1)),
            Verdict::Exported,
        ),
        (
            "init @Other.w1",
            replaced(0, lines[0].replacen(W1_NAME, "@Other.w1", 1)),
            Verdict::Exported,
        ),
        (
            "w1 twice",
            join(&w1_twice),
            Verdict::Parameter(W1_NAME.to_owned(), W1_NAME.to_owned()),
        ),
        (
            "w2 first",
            join(&swapped),
            Verdict::Parameter(W2_NAME.to_owned(), W1_NAME.to_owned()),
        ),
        ("three params", join(&third), Verdict::Unrecognized(Some(7))),
        ("one param", join(&one), Verdict::Unrecognized(None)),
        ("no param", join(&none), Verdict::Unrecognized(None)),
        (
            "param @Other.w1",
            with_param_text(true, |text| text.replacen(W1_NAME, "@Other.w1", 1)),
            Verdict::Parameter("@Other.w1".to_owned(), W2_NAME.to_owned()),
        ),
        (
            "param b1",
            with_param_text(true, |text| text.replacen(W1_NAME, "@PatchRegressor.b1", 1)),
            Verdict::Parameter("@PatchRegressor.b1".to_owned(), W2_NAME.to_owned()),
        ),
    ];
    for (name, stdout, expected) in cases {
        assert_eq!(probe(&stdout), expected, "{name}");
    }
}

/// Da9-4 on value tokens and the shape text: what each answers through `export`.
#[test]
fn value_tokens_and_shape_text() {
    let w1_shape_text = |from: &'static str, to: &'static str| {
        with_param_text(true, move |text| text.replacen(from, to, 1))
    };
    let cases: Vec<(&str, String, Verdict)> = vec![
        (
            "3.5e38",
            with_tokens(true, |list| list[5] = "3.5e38".to_owned()),
            Verdict::NonFinite(W1_NAME, 5),
        ),
        (
            "NaN",
            with_tokens(false, |list| list[0] = "NaN".to_owned()),
            Verdict::NonFinite(W2_NAME, 0),
        ),
        (
            "nan",
            with_tokens(false, |list| list[1] = "nan".to_owned()),
            Verdict::NonFinite(W2_NAME, 1),
        ),
        (
            "-nan",
            with_tokens(true, |list| list[2] = "-nan".to_owned()),
            Verdict::NonFinite(W1_NAME, 2),
        ),
        (
            "infinity",
            with_tokens(true, |list| list[3] = "infinity".to_owned()),
            Verdict::NonFinite(W1_NAME, 3),
        ),
        (
            "-inf",
            with_tokens(false, |list| list[511] = "-inf".to_owned()),
            Verdict::NonFinite(W2_NAME, 511),
        ),
        (
            "1_000.0",
            with_tokens(true, |list| list[7] = "1_000.0".to_owned()),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "0x1p0",
            with_tokens(true, |list| list[7] = "0x1p0".to_owned()),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "1.0f32",
            with_tokens(true, |list| list[7] = "1.0f32".to_owned()),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "empty token",
            with_tokens(true, |list| list[7] = String::new()),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "trailing comma",
            with_param_text(true, |text| format!("{},]", &text[..text.len() - 1])),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "no space after a comma",
            with_param_text(false, |text| {
                let (head, values) = text.split_once("= [").unwrap();
                format!("{head}= [{}", values.replacen(", ", ",", 1))
            }),
            Verdict::Unrecognized(Some(6)),
        ),
        (
            "two spaces after a comma",
            with_param_text(false, |text| {
                let (head, values) = text.split_once("= [").unwrap();
                format!("{head}= [{}", values.replacen(", ", ",  ", 1))
            }),
            Verdict::Unrecognized(Some(6)),
        ),
        (
            "no closing ]",
            with_param_text(true, |text| text[..text.len() - 1].to_owned()),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "text after ]",
            with_param_text(true, |text| format!("{text} x")),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[ ]",
            replaced(5, format!("param {W2_NAME}: Tensor<f32, [64, 8]> = [ ]")),
            Verdict::Unrecognized(Some(6)),
        ),
        (
            "[]",
            replaced(5, format!("param {W2_NAME}: Tensor<f32, [64, 8]> = []")),
            Verdict::Count(2048, 0),
        ),
        (
            "2,047",
            with_tokens(true, |list| {
                list.pop();
            }),
            Verdict::Count(2047, 512),
        ),
        (
            "2,049",
            with_tokens(true, |list| list.push("0.5".to_owned())),
            Verdict::Count(2049, 512),
        ),
        (
            "511",
            with_tokens(false, |list| {
                list.pop();
            }),
            Verdict::Count(2048, 511),
        ),
        (
            "513",
            with_tokens(false, |list| list.push("0.5".to_owned())),
            Verdict::Count(2048, 513),
        ),
        (
            "[32,64]",
            w1_shape_text("[32, 64]", "[32,64]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[32, 64, 1]",
            w1_shape_text("[32, 64]", "[32, 64, 1]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[32]",
            w1_shape_text("[32, 64]", "[32]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[-32, 64]",
            w1_shape_text("[32, 64]", "[-32, 64]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[32, 64 ]",
            w1_shape_text("[32, 64]", "[32, 64 ]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[032, 64]",
            w1_shape_text("[32, 64]", "[032, 64]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "[99999999999999999999, 64]",
            w1_shape_text("[32, 64]", "[99999999999999999999, 64]"),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "Tensor<f64",
            w1_shape_text("Tensor<f32, ", "Tensor<f64, "),
            Verdict::Unrecognized(Some(5)),
        ),
        (
            "Tensor<f32,[",
            w1_shape_text("Tensor<f32, [", "Tensor<f32,["),
            Verdict::Unrecognized(Some(5)),
        ),
    ];
    for (name, stdout, expected) in cases {
        assert_eq!(probe(&stdout), expected, "{name}");
    }
}

/// Da9-4 and Da9-3: tokens `f32::from_str` takes keep their exact bits through write and read,
/// the sign of zero and a subnormal included, at the payload's first and last positions.
#[test]
fn edge_values_keep_their_bits_through_write_and_read() {
    let dir = fresh("edge-values");
    let mut w1_tokens = tokens(&w1());
    let edge = [
        ("-0", (-0.0f32).to_bits()),
        ("1e-45", 1),
        ("3.4028235e38", f32::MAX.to_bits()),
        ("+1.0", 1.0f32.to_bits()),
        ("1.", 1.0f32.to_bits()),
        (".5", 0.5f32.to_bits()),
    ];
    for (index, (token, _)) in edge.iter().enumerate() {
        w1_tokens[index] = (*token).to_owned();
    }
    let mut w2_tokens = tokens(&w2());
    w2_tokens[511] = "-0.0".to_owned();
    let mut lines = documented();
    lines[4] = line("param", W1_NAME, W1_SHAPE, &w1_tokens);
    lines[5] = line("param", W2_NAME, W2_SHAPE, &w2_tokens);
    let run = completed(&join(&lines));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact = WeightArtifact::export(&run, "features/1", &path).unwrap();
    let read = WeightArtifact::read(&path).unwrap();
    assert_eq!(read, artifact);
    for (index, (token, bits)) in edge.iter().enumerate() {
        assert_eq!(read.w1()[index].to_bits(), *bits, "{token}");
    }
    assert_eq!(read.w2()[511].to_bits(), (-0.0f32).to_bits());
    let bytes = fs::read(&path).unwrap();
    let payload = &bytes[bytes.len() - PAYLOAD_BYTES..];
    assert_eq!(payload[..4], (-0.0f32).to_le_bytes());
    assert_eq!(payload[PAYLOAD_BYTES - 4..], (-0.0f32).to_le_bytes());
}

/// Da9-3 and Da9-2 read off the bytes: seven header lines, one empty line, 10,240 payload bytes
/// in print order, the `sha256` line the SHA-256 of exactly those bytes; two exports of one run
/// are byte-identical.
#[test]
fn the_written_bytes() {
    let dir = fresh("written-bytes");
    let run = completed(&join(&documented()));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    WeightArtifact::export(&run, "features/1", &path).unwrap();
    let bytes = fs::read(&path).unwrap();
    let end = bytes.windows(2).position(|pair| pair == b"\n\n").unwrap() + 2;
    let header = std::str::from_utf8(&bytes[..end]).unwrap();
    let lines: Vec<&str> = header.split_terminator('\n').collect();
    assert_eq!(lines.len(), 8, "{header}");
    assert_eq!(lines[7], "");
    let keys: Vec<&str> = lines[..7]
        .iter()
        .map(|line| line.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "weights/1",
            "model",
            "modelspec",
            "features",
            "w1",
            "w2",
            "sha256"
        ]
    );
    let payload = &bytes[end..];
    assert_eq!(payload.len(), PAYLOAD_BYTES);
    assert_eq!(
        lines[6],
        format!("sha256 {}", digest::hex(&digest::sha256(payload)))
    );
    assert_eq!(payload[..4], w1()[0].to_le_bytes());
    assert_eq!(
        payload[4 * W1_ELEMENTS..4 * W1_ELEMENTS + 4],
        w2()[0].to_le_bytes()
    );
    assert_eq!(
        payload[PAYLOAD_BYTES - 4..],
        w2()[W2_ELEMENTS - 1].to_le_bytes()
    );
    let again = dir.join("again.weights");
    WeightArtifact::export(&run, "features/1", &again).unwrap();
    assert_eq!(fs::read(&again).unwrap(), bytes);
}

/// The feature contract that makes the header exactly `MAX_HEADER_BYTES` long.
fn contract_for_full_header() -> String {
    let pin = pin();
    let base = format!(
        "weights/1\nmodel PatchRegressor\nmodelspec {} {}\nfeatures \nw1 32 64\nw2 64 8\nsha256 {}\n\n",
        pin.version(),
        pin.commit(),
        "0".repeat(64)
    )
    .len();
    "f".repeat(MAX_HEADER_BYTES - base)
}

/// Da9-11 and decision 5 at the bound: a header of exactly 1,024 bytes is written and read back;
/// one byte more is `Unwritable` and writes nothing. (A `>=` in the export's bound check is not
/// caught by the unit's tests, which only export a 1,024-byte contract.)
#[test]
fn export_writes_a_header_of_exactly_max_header_bytes_and_refuses_one_more() {
    let dir = fresh("header-bound");
    let run = completed(&join(&documented()));
    let contract = contract_for_full_header();
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact = WeightArtifact::export(&run, &contract, &path)
        .unwrap_or_else(|error| panic!("a {MAX_HEADER_BYTES}-byte header: {error}"));
    assert_eq!(
        fs::metadata(&path).unwrap().len() as usize,
        MAX_HEADER_BYTES + PAYLOAD_BYTES
    );
    assert_eq!(WeightArtifact::read(&path).unwrap(), artifact);
    fs::remove_file(&path).unwrap();
    let (found, _) = export_into(&run, &format!("{contract}f"), &dir);
    assert_eq!(found, Verdict::Unwritable);
}

/// Decision 6: `read` takes at most 1,024 + 10,240 + 1 bytes, and that last byte is the one that
/// tells an artifact with a full header from the same artifact with one byte appended. The file
/// of 11,265 bytes is corrupt; its first 11,264 bytes (what a reader bounded one byte lower
/// would see) are a valid artifact, so the `+ 1` is load-bearing and nothing else tests it.
#[test]
fn read_refuses_a_full_header_artifact_with_one_byte_appended() {
    let dir = fresh("read-bound");
    let run = completed(&join(&documented()));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact = WeightArtifact::export(&run, &contract_for_full_header(), &path).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), MAX_HEADER_BYTES + PAYLOAD_BYTES);
    bytes.push(0);
    let longer = dir.join("longer.weights");
    fs::write(&longer, &bytes).unwrap();
    assert!(
        matches!(
            WeightArtifact::read(&longer),
            Err(ReadError::Corrupt { .. })
        ),
        "an 11,265-byte file read as an artifact"
    );
    let cut = dir.join("cut.weights");
    fs::write(&cut, &bytes[..MAX_HEADER_BYTES + PAYLOAD_BYTES]).unwrap();
    assert_eq!(WeightArtifact::read(&cut).unwrap(), artifact);
}

/// Decision 6: `read` is bounded; a file that never ends is refused, not read to its end.
/// (Removing the bound is caught by nothing in the unit's tests.)
#[test]
fn read_is_bounded_on_a_file_without_end() {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(WeightArtifact::read(Path::new("/dev/zero")));
    });
    match receiver.recv_timeout(Duration::from_secs(60)) {
        Ok(Err(ReadError::Corrupt { .. })) => {}
        Ok(other) => panic!("{other:?}"),
        Err(_) => panic!("read of /dev/zero did not return within 60 s"),
    }
}

/// Da9-11: every failure to create the file is `Io` naming the path, and leaves nothing.
#[test]
fn a_path_that_cannot_be_created_is_io_and_leaves_nothing() {
    let dir = fresh("write-failures");
    let run = completed(&join(&documented()));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let check = |path: &Path, kind: Option<io::ErrorKind>| match WeightArtifact::export(
        &run,
        "features/1",
        path,
    ) {
        Err(ExportError::Io { path: at, error }) => {
            assert_eq!(at, path);
            if let Some(kind) = kind {
                assert_eq!(error.kind(), kind, "{}", path.display());
            }
        }
        other => panic!("{}: {:?}", path.display(), other.map(drop)),
    };

    fs::write(&path, b"keep").unwrap();
    check(&path, Some(io::ErrorKind::AlreadyExists));
    assert_eq!(fs::read(&path).unwrap(), b"keep");
    fs::remove_file(&path).unwrap();

    let as_dir = dir.join("a-directory.weights");
    fs::create_dir(&as_dir).unwrap();
    check(&as_dir, None);
    assert!(as_dir.is_dir());
    assert_eq!(fs::read_dir(&as_dir).unwrap().count(), 0);

    check(
        &dir.join("missing").join(RECOMMENDED_FILE_NAME),
        Some(io::ErrorKind::NotFound),
    );
    assert!(!dir.join("missing").exists());

    let locked = dir.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
    let probe_write = fs::write(locked.join("probe"), b"x").is_ok();
    if !probe_write {
        check(
            &locked.join(RECOMMENDED_FILE_NAME),
            Some(io::ErrorKind::PermissionDenied),
        );
        assert_eq!(fs::read_dir(&locked).unwrap().count(), 0);
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    check(Path::new("/proc/regressor.weights"), None);
    let names: Vec<String> = listing(&dir).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["a-directory.weights", "locked", "sentinel"]);
}

/// The reader on the edits the brief names: each is `Corrupt`; a directory and an unreadable
/// file are `Io`.
#[test]
fn the_reader_on_edited_bytes() {
    let dir = fresh("reader");
    let run = completed(&join(&documented()));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    WeightArtifact::export(&run, "features/1", &path).unwrap();
    let good = fs::read(&path).unwrap();
    let end = good.windows(2).position(|pair| pair == b"\n\n").unwrap() + 2;
    let header = std::str::from_utf8(&good[..end]).unwrap().to_owned();
    let payload = good[end..].to_vec();
    let with_header = |text: &str| [text.as_bytes(), &payload].concat();
    let header_lines: Vec<&str> = header.split_inclusive('\n').collect();
    let reordered = {
        let mut lines = header_lines.clone();
        lines.swap(2, 3);
        lines.concat()
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("payload short by 1", good[..good.len() - 1].to_vec()),
        ("payload short by 4", good[..good.len() - 4].to_vec()),
        ("payload short by 5,000", good[..good.len() - 5000].to_vec()),
        ("one byte appended", [good.as_slice(), b"\0"].concat()),
        ("NaN payload", {
            let nan: Vec<u8> = (0..PAYLOAD_BYTES / 4)
                .flat_map(|_| f32::NAN.to_le_bytes())
                .collect();
            let checksum = digest::hex(&digest::sha256(&nan));
            let text = header.replacen(&header_lines[6][7..71], &checksum, 1);
            [text.as_bytes(), &nan].concat()
        }),
        (
            "uppercase sha256",
            with_header(
                &header.replacen(
                    header_lines[6],
                    &header_lines[6]
                        .to_uppercase()
                        .replacen("SHA256", "sha256", 1),
                    1,
                ),
            ),
        ),
        (
            "weights/2",
            with_header(&header.replacen("weights/1", "weights/2", 1)),
        ),
        (
            "weights/1 ",
            with_header(&header.replacen("weights/1\n", "weights/1 \n", 1)),
        ),
        ("reordered", with_header(&reordered)),
        ("no empty line", with_header(&header[..header.len() - 1])),
        ("two empty lines", with_header(&format!("{header}\n"))),
        (
            "w1 32  64",
            with_header(&header.replacen("w1 32 64", "w1 32  64", 1)),
        ),
        (
            "w2 64 8 extra",
            with_header(&header.replacen("w2 64 8", "w2 64 8 extra", 1)),
        ),
        (
            "model Other",
            with_header(&header.replacen("model PatchRegressor", "model Other", 1)),
        ),
        ("empty file", Vec::new()),
        ("header only", header.as_bytes().to_vec()),
    ];
    for (name, bytes) in cases {
        let file = dir.join("edited.weights");
        fs::write(&file, &bytes).unwrap();
        match WeightArtifact::read(&file) {
            Err(error @ ReadError::Corrupt { .. }) => assert_eq!(
                error.domain_error(),
                Some("patchback.model.CorruptWeightArtifact")
            ),
            other => panic!("{name}: {:?}", other.map(drop)),
        }
        fs::remove_file(&file).unwrap();
    }
    assert!(matches!(
        WeightArtifact::read(&dir),
        Err(ReadError::Io { .. })
    ));
    let unreadable = dir.join("unreadable.weights");
    fs::write(&unreadable, &good).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let blocked = fs::read(&unreadable).is_err();
    let result = WeightArtifact::read(&unreadable);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).unwrap();
    if blocked {
        assert!(matches!(result, Err(ReadError::Io { .. })), "{result:?}");
    }
}

/// Two exports of one run to one path at once: exactly one writes, the other is `Io`
/// `AlreadyExists`, and the file is the writer's bytes.
#[test]
fn two_exports_racing_for_one_path() {
    let dir = fresh("race");
    let run = completed(&join(&documented()));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let results: Vec<Result<WeightArtifact, ExportError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| scope.spawn(|| WeightArtifact::export(&run, "features/1", &path)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    let verdicts: Vec<Verdict> = results.iter().map(verdict).collect();
    assert_eq!(
        verdicts.iter().filter(|v| **v == Verdict::Exported).count(),
        1,
        "{verdicts:?}"
    );
    assert!(
        verdicts.contains(&Verdict::Io(io::ErrorKind::AlreadyExists)),
        "{verdicts:?}"
    );
    let written = results.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(WeightArtifact::read(&path).unwrap(), written);
}

/// The pinned lane (skipped without a binary unless `PATCHBACK_REQUIRE_MODELSPEC` is set): a real
/// run without `--eval`, `--steps 1` on 16 rows, exports, its stdout holds no `eval` line, and
/// two exports of it are byte-identical. `pinned_export.rs` covers only a run with `--eval`.
#[test]
fn a_pinned_run_without_eval_exports() {
    use patchback_dataset::{Generator, SplitCounts, dense, training_stats};
    let pin = pin();
    let binary = match patchback_model::locate(None) {
        Ok(binary) => binary,
        Err(patchback_model::ModelSpecError::NotFound { .. }) => {
            assert!(
                std::env::var_os("PATCHBACK_REQUIRE_MODELSPEC").is_none(),
                "PATCHBACK_REQUIRE_MODELSPEC is set and no modelspec binary was found"
            );
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "SKIPPED a_pinned_run_without_eval_exports: no modelspec binary"
            );
            return;
        }
        Err(other) => panic!("{other}"),
    };
    let modelspec = ModelSpec::open(&binary, &pin).unwrap_or_else(|e| panic!("{e}"));
    let mut source = ModelSource::register(design_model(), "PatchRegressor", 16, &pin);
    source.check(&modelspec).unwrap();
    let dir = fresh("pinned-no-eval");
    let counts = SplitCounts {
        train: 16,
        validation: 16,
        test: 16,
    };
    let mut generated = Generator::new(42, counts, dir.clone()).run().unwrap();
    let split_dir = dir.join(SplitRole::Train.dir());
    fs::create_dir_all(&split_dir).unwrap();
    let train = generated.split_id(SplitRole::Train);
    let stats = training_stats(&generated).unwrap();
    dense::write_split(
        &mut generated.store,
        train,
        &split_dir,
        &stats,
        dense::create_new,
    )
    .unwrap();
    let mut run = TrainingRun::plan(Backend::Cpu, &pin).unwrap();
    run.bind(&source.trainable().unwrap()).unwrap();
    run.start(
        &generated.store,
        train,
        None,
        &modelspec,
        1,
        DEFAULT_TRAINING_TIMEOUT,
    )
    .unwrap();
    assert_eq!(
        run.finish().unwrap(),
        TrainingRunState::Completed,
        "{:?} {}",
        run.failure(),
        run.stderr()
    );
    assert!(!run.stdout().lines().any(|line| line.starts_with("eval ")));
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let artifact =
        WeightArtifact::export(&run, "features/1", &path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(WeightArtifact::read(&path).unwrap(), artifact);
    let again = dir.join("again.weights");
    WeightArtifact::export(&run, "features/1", &again).unwrap();
    assert_eq!(fs::read(&again).unwrap(), fs::read(&path).unwrap());
}
