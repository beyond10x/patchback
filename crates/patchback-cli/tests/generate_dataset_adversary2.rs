//! Adversary cases for `patchback generate-dataset` (story:dataset-generation, wave a6, pass 2):
//! the exact edge of the `--samples` range, a count in the range where `samples · 4992` passes
//! `i64::MAX`, and the exact set of paths and stdout keys of a 48-sample generation after the
//! `val/` directory change (Da6-19, Da6-27). Re-pinned to Da7-10 and the dense files of
//! story:dense-data-files: `--samples` admits `0..=u32::MAX` (replacing Da6-26's `i64::MAX`), and
//! each split directory holds `x.f32`, `y.f32` and `order.u32` beside `samples.meta`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-generate-dataset-adversary2")
        .join(test);
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn generate(seed: &str, samples: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["generate-dataset", "--seed", seed, "--samples", samples])
        .arg("--out")
        .arg(out)
        .output()
        .expect("the patchback binary runs")
}

/// `generate`, for a count that must be refused at once: a run still going after 30 s is killed
/// and the case fails, instead of the case waiting on a generation of ~2^63 samples.
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
            panic!(
                "--samples {samples} was not refused: the generation was still running after 30 s"
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn names(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

/// Da7-10 at its edge: `u32::MAX + 1` is the first value clap refuses, with its range message,
/// exit 2, nothing on stdout and nothing created; so is Da6-26's first refused value.
#[test]
fn adversary_one_past_max_samples_is_a_usage_error() {
    let samples = (u64::from(u32::MAX) + 1).to_string();
    assert_eq!(samples, "4294967296");
    for samples in [samples.as_str(), "9223372036854775808"] {
        let dir = scratch(&format!("one-past-max-{samples}"));
        let out = dir.join("data");
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

/// `--samples 2^62 + 8`: `samples · 4992` passes `i64::MAX`. Under Da6-26 it was refused with the
/// rule's train count; since Da7-10 it is past `u32::MAX` and a usage error (exit 2, clap's range
/// message), never a generation and never a SplitCountNotBatchMultiple refusal.
#[test]
fn adversary_a_count_past_i64_times_4992_is_a_usage_error() {
    let samples: u64 = (1 << 62) + 8;
    assert!(i64::try_from(samples).unwrap().checked_mul(4_992).is_none());

    let dir = scratch("past-i64-product");
    let out = dir.join("data");
    let output = generate_refused(&samples.to_string(), &out);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains(&format!("{samples} is not in 0..=4294967295")),
        "{stderr}"
    );
    assert!(!stderr.contains("SplitCountNotBatchMultiple"), "{stderr}");
    assert!(!out.exists());
}

/// Da6-19, Da6-27 on disk and on stdout: `<out>` holds exactly `dataset.meta`,
/// `normalization.stats` (story:normalization-stats), `train/`, `val/` and `test/`, each split
/// directory exactly its `samples.meta`, `x.f32`, `y.f32` and `order.u32`
/// (story:dense-data-files); the stdout keys are, in order, `out`, `train`, `validation`, `test`
/// and the three diagnostic counts.
#[test]
fn adversary_a_generation_writes_exactly_the_val_layout_and_prints_the_validation_key() {
    let dir = scratch("layout");
    let out = dir.join("data");
    let output = generate("42", "48", &out);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        names(&out),
        [
            "dataset.meta",
            "normalization.stats",
            "test",
            "train",
            "val"
        ]
        .map(str::to_owned)
        .into(),
    );
    for split in ["train", "val", "test"] {
        assert_eq!(
            names(&out.join(split)),
            ["order.u32", "samples.meta", "x.f32", "y.f32"]
                .map(str::to_owned)
                .into(),
            "{split}"
        );
    }
    let stdout = text(&output.stdout);
    let keys: Vec<&str> = stdout
        .lines()
        .map(|line| line.split_once(' ').unwrap().0)
        .collect();
    assert_eq!(
        keys,
        [
            "out",
            "train",
            "validation",
            "test",
            "diagnostic_silence",
            "diagnostic_noise",
            "diagnostic_clipping"
        ]
    );
    let meta = text(&fs::read(out.join("dataset.meta")).unwrap());
    assert!(meta.contains("\nvalidation 16\n"), "{meta}");
    assert!(!meta.contains("\nval 16\n"), "{meta}");
}
