//! `patchback generate-dataset --seed <n> --samples <n> --out <dir>` through the binary
//! (story:dataset-generation, story:dense-data-files): the files it writes (`dataset.meta`, and
//! `samples.meta`, `x.f32`, `y.f32` and `order.u32` of each split), what it prints, its refusals
//! with exit 1, the `--samples` ceiling, and byte-identical output for one seed in two processes
//! (DESIGN.md § 2 invariant 5). Generations here are 48 samples, one batch a split.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

/// The files of a split's directory, in the order they are written (story:dense-data-files adds
/// the three dense files after `samples.meta`).
const SPLIT_FILES: [&str; 4] = ["samples.meta", "x.f32", "y.f32", "order.u32"];

/// Every file a generation writes, by path relative to `out`.
fn files(out: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = vec![(
        "dataset.meta".to_owned(),
        fs::read(out.join("dataset.meta")).unwrap(),
    )];
    for split in SPLITS {
        for file in SPLIT_FILES {
            let name = format!("{split}/{file}");
            files.push((name.clone(), fs::read(out.join(&name)).unwrap()));
        }
    }
    files
}

/// Every path under `dir`, relative to it, files and directories, sorted.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, paths: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            paths.push(relative);
            if path.is_dir() && !path.is_symlink() {
                walk(root, &path, paths);
            }
        }
    }
    let mut paths = Vec::new();
    walk(dir, dir, &mut paths);
    paths.sort();
    paths
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

    // story:dense-data-files: 16 rows a split, x 16 × 32 × 4, y 16 × 8 × 4, order 16 × 4 bytes.
    for split in SPLITS {
        let sizes: Vec<u64> = ["x.f32", "y.f32", "order.u32"]
            .iter()
            .map(|file| fs::metadata(out.join(split).join(file)).unwrap().len())
            .collect();
        assert_eq!(sizes, [2_048, 512, 64], "{split}");
        let order: Vec<u32> = fs::read(out.join(split).join("order.u32"))
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect();
        assert_eq!(order, (0..16).collect::<Vec<u32>>(), "{split}");
    }
    let mut expected_tree = vec!["dataset.meta".to_owned()];
    for split in SPLITS {
        expected_tree.push(split.to_owned());
        expected_tree.extend(SPLIT_FILES.iter().map(|file| format!("{split}/{file}")));
    }
    expected_tree.sort();
    assert_eq!(tree(&out), expected_tree);
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

/// Da7-10 (superseding Da6-26's `i64::MAX`): `--samples` admits up to `u32::MAX`, the most rows
/// `order.u32` indexes. `4294967295` is accepted by the parser and refused by the count rule:
/// it is odd and the held-out counts are even, so train (D-a6-10, computed here in u128) is odd.
#[test]
fn the_largest_admitted_sample_count_is_refused_with_the_train_count_of_the_rule() {
    let samples = u128::from(u32::MAX);
    let held_out = (samples * 4_992 / 60_000 / 16 * 16).max(16);
    let train = samples - 2 * held_out;
    assert_eq!(train % 2, 1, "train is odd");

    let dir = scratch("largest-admitted");
    let out = dir.join("data");
    let stderr = assert_refused(
        &generate("42", "4294967295", &out),
        "patchback: generate-dataset refused: patchback.dataset.SplitCountNotBatchMultiple: ",
    );
    assert!(
        stderr.contains(&format!("Train split of {train} samples")),
        "the rule's train count {train}: {stderr}"
    );
    assert!(!out.exists(), "nothing is created for a refused count");
}

/// `generate` for a count that must be refused at once: a run still going after 30 s is killed
/// and the case fails, instead of waiting on a generation of 2^32 samples (the red run of this
/// case, before the ceiling moved, generated for ten minutes until it was killed by hand).
fn generate_refused(samples: &str, out: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["generate-dataset", "--seed", "42", "--samples", samples])
        .arg("--out")
        .arg(out)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the patchback binary runs");
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("--samples {samples} was not refused: still running after 30 s");
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

/// Da7-10: one past `u32::MAX`, `4294967296`, and the old ceiling `i64::MAX` are usage errors
/// (clap's range message, exit 2), with nothing on stdout and nothing created.
#[test]
fn a_sample_count_past_u32_max_is_a_usage_error() {
    let dir = scratch("past-u32");
    for samples in ["4294967296", "9223372036854775807"] {
        let out = dir.join(format!("data-{samples}"));
        let output = generate_refused(samples, &out);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains(&format!("{samples} is not in 0..=4294967295")),
            "{stderr}"
        );
        assert!(!out.exists());
    }
}

/// An existing dense file is refused before the generation, like `samples.meta`: the file keeps
/// its bytes and nothing is written.
#[test]
fn an_existing_dense_file_is_never_overwritten_and_nothing_is_written() {
    let dir = scratch("existing-dense");
    let out = dir.join("data");
    fs::create_dir_all(out.join("train")).unwrap();
    fs::write(out.join("train/x.f32"), "kept\n").unwrap();
    let stderr = assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert!(
        stderr.contains(&format!("{:?}", out.join("train").join("x.f32"))),
        "{stderr}"
    );
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(
        fs::read_to_string(out.join("train/x.f32")).unwrap(),
        "kept\n"
    );
    assert_eq!(tree(&out), ["train", "train/x.f32"]);
}

/// A dense write that fails after the generation (the last file of the last split,
/// `test/order.u32`, is a dangling symbolic link, which the pre-check does not see and the
/// exclusive create refuses) removes every file and directory the run created, the other eleven
/// dense and sample files among them; what the run did not create stays.
#[cfg(unix)]
#[test]
fn a_failed_dense_write_removes_everything_the_run_created() {
    let dir = scratch("failed-dense-write");
    let out = dir.join("data");
    fs::create_dir_all(out.join("test")).unwrap();
    std::os::unix::fs::symlink(dir.join("nowhere"), out.join("test/order.u32")).unwrap();
    let stderr = assert_refused(
        &generate("42", "48", &out),
        "patchback: generate-dataset refused: cannot write ",
    );
    assert!(
        stderr.contains(&format!("{:?}", out.join("test").join("order.u32"))),
        "{stderr}"
    );
    assert_eq!(tree(&out), ["test", "test/order.u32"]);
    assert!(!dir.join("nowhere").exists());
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
