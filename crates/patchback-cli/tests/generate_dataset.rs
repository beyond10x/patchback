//! `patchback generate-dataset --seed <n> --samples <n> --out <dir>` through the binary
//! (story:dataset-generation): the files it writes (`dataset.meta`, and `samples.meta` of each
//! split), what it prints, its refusals with exit 1, and byte-identical output for one seed in two
//! processes (DESIGN.md § 2 invariant 5). Generations here are 48 samples, one batch a split.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

/// A fresh, empty directory for one test under Cargo's per-target scratch directory.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-generate-dataset")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

fn generate(seed: &str, samples: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args([
            "generate-dataset",
            "--seed",
            seed,
            "--samples",
            samples,
            "--out",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("the patchback binary runs")
}

/// The `key value` lines of a text.
fn records(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once(' ').expect("key value");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

const SPLITS: [&str; 3] = ["train", "val", "test"];
const DIAGNOSTICS: [&str; 3] = [
    "diagnostic_silence",
    "diagnostic_noise",
    "diagnostic_clipping",
];

/// Every file a generation writes, by path relative to `out`.
fn files(out: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![(
        "dataset.meta".to_owned(),
        fs::read(out.join("dataset.meta")).unwrap(),
    )];
    for split in SPLITS {
        let name = format!("{split}/samples.meta");
        files.push((name.clone(), fs::read(out.join(&name)).unwrap()));
    }
    files
}

#[test]
fn forty_eight_samples_write_the_dataset_meta_and_one_sample_list_a_split() {
    let dir = scratch("forty-eight");
    let out = dir.join("data");
    let started = Instant::now();
    let output = generate("42", "48", &out);
    let elapsed = started.elapsed();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    eprintln!("generate-dataset --samples 48: {elapsed:?} (this build's profile)");

    let meta = text(&fs::read(out.join("dataset.meta")).unwrap());
    let lines: Vec<&str> = meta.lines().collect();
    assert_eq!(
        lines[..9],
        [
            "patchback-dataset-meta 1",
            "seed 42",
            "prng xoshiro256**",
            "batch_size 16",
            "feature_contract features/1",
            "topology subtractive-mono/1",
            "train 16",
            "validation 16",
            "test 16",
        ]
    );
    assert_eq!(lines.len(), 12, "{meta}");
    assert!(meta.ends_with('\n'));
    let meta_records = records(&meta);
    let keys: Vec<&str> = meta_records[9..].iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, DIAGNOSTICS);
    for (_, count) in &meta_records[9..] {
        count.parse::<u64>().expect("a count");
    }

    let printed = records(&text(&output.stdout));
    let mut expected = vec![("out".to_owned(), format!("{out:?}"))];
    expected.extend(meta_records[6..].iter().cloned());
    assert_eq!(printed, expected);

    let mut seeds = HashSet::new();
    for split in SPLITS {
        let list = text(&fs::read(out.join(split).join("samples.meta")).unwrap());
        let mut lines = list.lines();
        assert_eq!(lines.next(), Some("patchback-dataset-samples 1"), "{split}");
        let rows: Vec<Vec<u64>> = lines
            .map(|line| {
                line.split(' ')
                    .map(|field| field.parse::<u64>().expect("an unsigned integer"))
                    .collect()
            })
            .collect();
        assert_eq!(rows.len(), 16, "{split}");
        for row in rows {
            let [seed, midi, gate] = row[..] else {
                panic!("{split}: {row:?} is not patch_seed midi_note gate_samples");
            };
            assert!((36..=84).contains(&midi), "{split}: {row:?}");
            assert!((12_000..=72_000).contains(&gate), "{split}: {row:?}");
            assert!(seeds.insert(seed), "patch seed {seed} is in two splits");
        }
    }
    assert!(
        !out.join("validation").exists(),
        "the validation split's directory is val/ (DESIGN.md § 7.2)"
    );
}

#[test]
fn the_same_seed_writes_byte_identical_files_in_two_processes() {
    let dir = scratch("twice");
    let first = dir.join("first");
    let second = dir.join("second");
    let a = generate("7", "48", &first);
    let b = generate("7", "48", &second);
    assert_eq!(a.status.code(), Some(0), "{a:?}");
    assert_eq!(b.status.code(), Some(0), "{b:?}");
    assert_eq!(files(&first), files(&second));
    let other = dir.join("other");
    assert_eq!(generate("8", "48", &other).status.code(), Some(0));
    assert_ne!(
        files(&first)[1],
        files(&other)[1],
        "another seed draws other patches"
    );
}

fn assert_refused(output: &Output, prefix: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with(prefix), "{stderr}");
    stderr
}

#[test]
fn a_train_count_that_is_not_a_batch_multiple_is_refused_and_nothing_is_written() {
    let dir = scratch("not-a-multiple");
    for (samples, train) in [("47", "15"), ("20", "-12"), ("0", "-32")] {
        let out = dir.join(format!("data-{samples}"));
        let stderr = assert_refused(
            &generate("42", samples, &out),
            "patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: ",
        );
        assert!(
            stderr.contains(&format!("Train split of {train} samples")),
            "{stderr}"
        );
        assert!(stderr.contains("batch size 16"), "{stderr}");
        assert!(!out.exists(), "nothing is created for a refused count");
    }
}

/// Da6-26: `--samples` admits up to `i64::MAX`, and there the train count is the rule's (D-a6-10,
/// computed here in u128), never a saturated one. `i64::MAX` is odd and the held-out counts are
/// even, so train is odd and refused.
#[test]
fn the_largest_admitted_sample_count_is_refused_with_the_train_count_of_the_rule() {
    let samples = u128::from(i64::MAX as u64);
    let held_out = (samples * 4_992 / 60_000 / 16 * 16).max(16);
    let train = samples - 2 * held_out;
    assert_eq!(train % 2, 1, "train is odd");

    let dir = scratch("largest-admitted");
    let out = dir.join("data");
    let stderr = assert_refused(
        &generate("42", "9223372036854775807", &out),
        "patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: ",
    );
    assert!(
        stderr.contains(&format!("Train split of {train} samples")),
        "the rule's train count {train}: {stderr}"
    );
    assert!(!out.exists(), "nothing is created for a refused count");
}

#[test]
fn an_existing_dataset_meta_is_never_overwritten() {
    let dir = scratch("existing");
    let out = dir.join("data");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("dataset.meta"), "kept\n").unwrap();
    let stderr = assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert!(
        stderr.contains(&format!("{:?}", out.join("dataset.meta"))),
        "{stderr}"
    );
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(
        fs::read_to_string(out.join("dataset.meta")).unwrap(),
        "kept\n"
    );
    for split in SPLITS {
        assert!(!out.join(split).exists(), "{split} was written");
    }
}

#[test]
fn an_existing_sample_list_is_never_overwritten_and_nothing_is_left_behind() {
    let dir = scratch("existing-split");
    let out = dir.join("data");
    fs::create_dir_all(out.join("val")).unwrap();
    fs::write(out.join("val/samples.meta"), "kept\n").unwrap();
    let stderr = assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert!(
        stderr.contains(&format!("{:?}", out.join("val").join("samples.meta"))),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(out.join("val/samples.meta")).unwrap(),
        "kept\n"
    );
    assert!(!out.join("dataset.meta").exists());
    assert!(!out.join("train").exists(), "nothing else is written");
    assert!(!out.join("test").exists());
}

/// A write that fails after the generation (the `test` split's directory is a file) removes the
/// files and directories the run had written.
#[test]
fn a_write_that_fails_removes_what_the_run_wrote() {
    let dir = scratch("failed-write");
    let out = dir.join("data");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("test"), "a file\n").unwrap();
    let stderr = assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert!(
        stderr.contains(&format!("{:?}", out.join("test").join("samples.meta"))),
        "{stderr}"
    );
    assert!(!out.join("train").exists(), "the train list is removed");
    assert!(!out.join("val").exists(), "the val list is removed");
    assert!(!out.join("dataset.meta").exists());
    assert_eq!(fs::read_to_string(out.join("test")).unwrap(), "a file\n");
    assert!(out.is_dir(), "a directory the run did not create stays");
}

#[test]
fn an_out_path_that_is_a_file_is_refused() {
    let dir = scratch("file");
    let out = dir.join("data");
    fs::write(&out, "a file\n").unwrap();
    assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert_eq!(fs::read_to_string(&out).unwrap(), "a file\n");
}
