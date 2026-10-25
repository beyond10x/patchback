//! Adversary pass 2 on story:weight-export (wave a9): the name grammar of correction 1 (Da9-19),
//! driven through `WeightArtifact::export`.
//!
//! Da9-19: the name of an `init` or `param` line is `@<ident>.<ident>` (an `<ident>` one or more
//! ASCII letters, digits or `_`); a line whose name is not of that form is
//! `UnrecognizedTrainingOutput`; the names step compares the first `param` name with
//! `@PatchRegressor.w1` and the second with `@PatchRegressor.w2` in full; an `init` name is checked
//! for form only. The run comes from a stand-in `modelspec` script, as in
//! `tests/adversary_weight_export.rs` (copied here; that file is not edited).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use patchback_dataset::{
    DatasetSplitId, DatasetStore, DenseFileKind, GenerateDataset, Pipeline, PrngAlgorithm,
    SplitRole, WriteDenseFile,
};
use patchback_model::{
    Backend, DEFAULT_TRAINING_TIMEOUT, ExportError, ModelSource, ModelSpec, ModelSpecPin,
    RECOMMENDED_FILE_NAME, TrainingRun, TrainingRunState, W1_ELEMENTS, W1_NAME, W1_SHAPE,
    W2_ELEMENTS, W2_NAME, W2_SHAPE, WeightArtifact,
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
        line("param", W1_NAME, W1_SHAPE, &tokens(&values(W1_ELEMENTS, 1))),
        line("param", W2_NAME, W2_SHAPE, &tokens(&values(W2_ELEMENTS, 2))),
    ]
}

fn join(lines: &[String]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// An edit of one line of `documented()`.
type Edit = Box<dyn Fn(&str) -> String>;

/// `documented()` with each `(index, edit)` applied to line `index` (0-based); every edit must
/// change its line.
fn edited(edits: Vec<(usize, Edit)>) -> String {
    let mut lines = documented();
    for (index, edit) in edits {
        let new = edit(&lines[index]);
        assert_ne!(
            new, lines[index],
            "the edit of line {index} changed nothing"
        );
        lines[index] = new;
    }
    join(&lines)
}

/// Line `index` with its name (`W1_NAME` on lines 0 and 4, `W2_NAME` on 1 and 5) replaced.
fn renamed(index: usize, name: &str) -> String {
    let current = if index.is_multiple_of(2) {
        W1_NAME
    } else {
        W2_NAME
    };
    let name = name.to_owned();
    edited(vec![(
        index,
        Box::new(move |line: &str| line.replacen(current, &name, 1)),
    )])
}

fn standins() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("patchback-model-adversary-export-2");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let version = pin().version().to_owned();
        let path = dir.join("exits-0");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"modelspec {version}\"; exit 0; fi\nif [ \"$1\" = check ]; then exit 0; fi\neval last=\\${{$#}}\ncat \"{}/out-$last.txt\"\nexit 0\n",
                dir.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        dir
    })
}

fn next_steps() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(2000);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

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

/// A `Completed` run whose stand-in printed `stdout`.
fn completed(stdout: &str) -> TrainingRun {
    let dir = standins();
    let steps = next_steps();
    fs::write(dir.join(format!("out-{steps}.txt")), stdout).unwrap();
    let pin = pin();
    let modelspec = ModelSpec::open(&dir.join("exits-0"), &pin).unwrap();
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
    run.finish().unwrap();
    assert_eq!(run.state(), TrainingRunState::Completed);
    run
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Exported,
    Unrecognized(Option<usize>),
    Parameter(String, String),
    Other(String),
}

fn parameter(w1: &str, w2: &str) -> Verdict {
    Verdict::Parameter(w1.to_owned(), w2.to_owned())
}

/// The verdict of exporting a Completed run that printed `stdout`; a refusal leaves the
/// directory empty, and an `UnrecognizedParameter` carries its declared error and both names in
/// its message.
fn probe(stdout: &str) -> Verdict {
    let dir = standins().join(format!("probe-{}", next_steps()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(RECOMMENDED_FILE_NAME);
    let result = WeightArtifact::export(&completed(stdout), "features/1", &path);
    let written = fs::read_dir(&dir).unwrap().count();
    match &result {
        Ok(_) => {
            assert_eq!(written, 1);
            Verdict::Exported
        }
        Err(error) => {
            assert_eq!(written, 0, "the refusal {error:?} wrote a file");
            match error {
                ExportError::UnrecognizedTrainingOutput { line, .. } => {
                    Verdict::Unrecognized(*line)
                }
                ExportError::UnrecognizedParameter { w1_name, w2_name } => {
                    assert_eq!(
                        error.domain_error(),
                        Some("patchback.model.UnrecognizedParameter")
                    );
                    let message = error.to_string();
                    assert!(message.contains(w1_name.as_str()), "{message}");
                    assert!(message.contains(w2_name.as_str()), "{message}");
                    Verdict::Parameter(w1_name.clone(), w2_name.clone())
                }
                other => Verdict::Other(format!("{other:?}").chars().take(200).collect()),
            }
        }
    }
}

fn check(cases: Vec<(&str, String, Verdict)>) {
    let mut wrong = Vec::new();
    for (label, stdout, expected) in cases {
        let found = probe(&stdout);
        if found != expected {
            let show = |verdict: &Verdict| {
                let text = format!("{verdict:?}");
                if text.len() > 160 {
                    format!("{}… ({} bytes)", &text[..160], text.len())
                } else {
                    text
                }
            };
            wrong.push(format!(
                "{label}: expected {}, found {}",
                show(&expected),
                show(&found)
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// The brief's names on the first `param` line (line 5): a name not of the form `@<ident>.<ident>`
/// is unrecognized output at line 5; a well-formed name other than `@PatchRegressor.w1`, in full,
/// is `UnrecognizedParameter` with the name as printed. Catches a suffix (`ends_with(".w1")`) or
/// prefix (`starts_with`) comparison, a case-insensitive comparison, and an identifier test that
/// takes non-ASCII letters.
#[test]
fn the_name_grammar_on_the_first_param_line() {
    let long = format!("@{}.w1", "A".repeat(10_000));
    let w1_line = documented()[4].clone();
    let whole_line = |from: &str, to: &str| {
        let mut lines = documented();
        lines[4] = w1_line.replacen(from, to, 1);
        assert_ne!(lines[4], w1_line);
        join(&lines)
    };
    let u5 = || Verdict::Unrecognized(Some(5));
    let cases: Vec<(&str, String, Verdict)> = vec![
        ("@.w1", renamed(4, "@.w1"), u5()),
        ("@A.", renamed(4, "@A."), u5()),
        ("@A.B.C", renamed(4, "@A.B.C"), u5()),
        ("@A..w1", renamed(4, "@A..w1"), u5()),
        ("@A-B.w1", renamed(4, "@A-B.w1"), u5()),
        ("@A B.w1", renamed(4, "@A B.w1"), u5()),
        ("@A.w1 (trailing space)", renamed(4, "@A.w1 "), u5()),
        ("@Ünicode.w1", renamed(4, "@Ünicode.w1"), u5()),
        ("@PatchRegressor.wé", renamed(4, "@PatchRegressor.wé"), u5()),
        ("empty name", renamed(4, ""), u5()),
        ("@A:B.w1", renamed(4, "@A:B.w1"), u5()),
        (
            "@PatchRegressor.w1:x",
            renamed(4, "@PatchRegressor.w1:x"),
            u5(),
        ),
        (
            "@@PatchRegressor.w1",
            renamed(4, "@@PatchRegressor.w1"),
            u5(),
        ),
        (
            "param@PatchRegressor.w1",
            whole_line("param @", "param@"),
            u5(),
        ),
        (
            "param  @PatchRegressor.w1",
            whole_line("param @", "param  @"),
            u5(),
        ),
        (
            "@PatchRegressor.W1",
            renamed(4, "@PatchRegressor.W1"),
            parameter("@PatchRegressor.W1", W2_NAME),
        ),
        (
            "@patchregressor.w1",
            renamed(4, "@patchregressor.w1"),
            parameter("@patchregressor.w1", W2_NAME),
        ),
        ("@0A.w1", renamed(4, "@0A.w1"), parameter("@0A.w1", W2_NAME)),
        ("@A.0", renamed(4, "@A.0"), parameter("@A.0", W2_NAME)),
        ("@_.w1", renamed(4, "@_.w1"), parameter("@_.w1", W2_NAME)),
        (
            "@Other.w1",
            renamed(4, "@Other.w1"),
            parameter("@Other.w1", W2_NAME),
        ),
        (
            "@XPatchRegressor.w1",
            renamed(4, "@XPatchRegressor.w1"),
            parameter("@XPatchRegressor.w1", W2_NAME),
        ),
        (
            "@PatchRegressor.w1x",
            renamed(4, "@PatchRegressor.w1x"),
            parameter("@PatchRegressor.w1x", W2_NAME),
        ),
        (
            "@PatchRegressor.w2 first",
            renamed(4, "@PatchRegressor.w2"),
            parameter(W2_NAME, W2_NAME),
        ),
        (
            "10,000-byte model identifier",
            renamed(4, &long),
            parameter(&long, W2_NAME),
        ),
    ];
    check(cases);
}

/// The same grammar on the second `param` line (line 6), and the full comparison with
/// `@PatchRegressor.w2`.
#[test]
fn the_name_grammar_on_the_second_param_line() {
    let u6 = || Verdict::Unrecognized(Some(6));
    let cases: Vec<(&str, String, Verdict)> = vec![
        ("@Ünicode.w2", renamed(5, "@Ünicode.w2"), u6()),
        ("@A.B.w2", renamed(5, "@A.B.w2"), u6()),
        ("@.w2", renamed(5, "@.w2"), u6()),
        (
            "@Other.w1 second",
            renamed(5, "@Other.w1"),
            parameter(W1_NAME, "@Other.w1"),
        ),
        (
            "@Other.w2",
            renamed(5, "@Other.w2"),
            parameter(W1_NAME, "@Other.w2"),
        ),
        (
            "@PatchRegressor.W2",
            renamed(5, "@PatchRegressor.W2"),
            parameter(W1_NAME, "@PatchRegressor.W2"),
        ),
        (
            "@PatchRegressor.w2_",
            renamed(5, "@PatchRegressor.w2_"),
            parameter(W1_NAME, "@PatchRegressor.w2_"),
        ),
    ];
    check(cases);
}

/// Da9-19 and Da9-6: an `init` line's name is checked for its form (`@<ident>.<ident>`) and not
/// compared; its shape is not checked. The pass-1 row `init @Other.w1` flipped from a grammar
/// error to `Exported` in correction 1, so these rows are what still observes the `init` name's
/// form: a mutant that skips the name test on `init` lines must fail here.
#[test]
fn an_init_name_is_checked_for_form_only() {
    let init_line = |index: usize, name: &str, shape: [usize; 2], count: usize| {
        let name = name.to_owned();
        edited(vec![(
            index,
            Box::new(move |_: &str| line("init", &name, shape, &tokens(&values(count, 9)))),
        )])
    };
    let u1 = || Verdict::Unrecognized(Some(1));
    let cases: Vec<(&str, String, Verdict)> = vec![
        (
            "init @Other.w1 [64, 32], 5 values",
            init_line(0, "@Other.w1", [64, 32], 5),
            Verdict::Exported,
        ),
        (
            "init @PatchRegressor.W1",
            renamed(0, "@PatchRegressor.W1"),
            Verdict::Exported,
        ),
        ("init @0A.b1", renamed(0, "@0A.b1"), Verdict::Exported),
        (
            "init @Other.w2 second",
            renamed(1, "@Other.w2"),
            Verdict::Exported,
        ),
        ("init @.w1", renamed(0, "@.w1"), u1()),
        ("init @A.", renamed(0, "@A."), u1()),
        (
            "init PatchRegressor.w1",
            renamed(0, "PatchRegressor.w1"),
            u1(),
        ),
        ("init @Ünicode.w1", renamed(0, "@Ünicode.w1"), u1()),
        ("init @A.B.C", renamed(0, "@A.B.C"), u1()),
        ("init @A-B.w1", renamed(0, "@A-B.w1"), u1()),
        ("init @A.w1 (trailing space)", renamed(0, "@A.w1 "), u1()),
        ("init empty name", renamed(0, ""), u1()),
        (
            "init @A..w2 second",
            renamed(1, "@A..w2"),
            Verdict::Unrecognized(Some(2)),
        ),
        (
            "init value abc",
            edited(vec![(
                0,
                Box::new(|line: &str| line.replacen("= [", "= [abc, ", 1)),
            )]),
            u1(),
        ),
    ];
    check(cases);
}

/// Da9-7 with the split of Da9-19: a well-formed other name answers before the shapes, the
/// counts and finiteness; a later grammar error answers before the name; a malformed name with a
/// wrong shape is the grammar error.
#[test]
fn a_param_of_another_model_answers_in_the_order_of_da9_7() {
    let w1_other = |edit: Edit| -> Vec<(usize, Edit)> {
        vec![
            (
                4,
                Box::new(|line: &str| line.replacen(W1_NAME, "@Other.w1", 1)),
            ),
            (4, edit),
        ]
    };
    let other = || parameter("@Other.w1", W2_NAME);
    let mut with_garbage = documented();
    with_garbage[4] = with_garbage[4].replacen(W1_NAME, "@Other.w1", 1);
    with_garbage.push("checkpoint written".to_owned());
    let cases: Vec<(&str, String, Verdict)> = vec![
        (
            "@Other.w1 transposed",
            edited(w1_other(Box::new(|line: &str| {
                line.replacen("[32, 64]", "[64, 32]", 1)
            }))),
            other(),
        ),
        (
            "@Other.w1 with 2,047 values",
            edited(w1_other(Box::new(|line: &str| {
                let (head, _) = line.split_once("= [").unwrap();
                format!(
                    "{head}= [{}]",
                    tokens(&values(W1_ELEMENTS - 1, 1)).join(", ")
                )
            }))),
            other(),
        ),
        (
            "@Other.w1 with NaN",
            edited(w1_other(Box::new(|line: &str| {
                line.replacen("= [", "= [NaN, ", 1)
            }))),
            other(),
        ),
        (
            "w2 @Other.w1 with w2 transposed",
            edited(vec![(
                5,
                Box::new(|line: &str| {
                    line.replacen(W2_NAME, "@Other.w1", 1)
                        .replacen("[64, 8]", "[8, 64]", 1)
                }),
            )]),
            parameter(W1_NAME, "@Other.w1"),
        ),
        (
            "@Other.w1 then an unrecognized line 7",
            join(&with_garbage),
            Verdict::Unrecognized(Some(7)),
        ),
        (
            "@Ünicode.w1 transposed",
            edited(vec![(
                4,
                Box::new(|line: &str| {
                    line.replacen(W1_NAME, "@Ünicode.w1", 1)
                        .replacen("[32, 64]", "[64, 32]", 1)
                }),
            )]),
            Verdict::Unrecognized(Some(5)),
        ),
    ];
    check(cases);
}

/// The unit's documented stdout, unedited, exports: the copied helpers build what the unit's
/// own tests build, so a red row above is the parser's and not the fixture's.
#[test]
fn the_documented_stdout_exports() {
    assert_eq!(probe(&join(&documented())), Verdict::Exported);
}
