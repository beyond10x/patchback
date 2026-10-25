//! Adversary cases for `patchback generate-dataset` (story:dense-data-files, wave a7, pass 1):
//! every dense path of `targets(out)` is refused before the generation, and a dense write that
//! fails in the middle split removes what the run created and nothing it did not.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-generate-dataset-dense-adversary")
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

fn generate(samples: &str, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(["generate-dataset", "--seed", "42", "--samples", samples])
        .arg("--out")
        .arg(out)
        .output()
        .expect("the patchback binary runs")
}

/// Every path under `dir`, relative to it, sorted; a symbolic link is not followed.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, paths: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            paths.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
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

/// `test/order.u32` and `test/y.f32`, the last dense paths written, are each refused by the
/// pre-check: exit 1, the path named, nothing on stdout, the file kept, nothing created, and well
/// before a 48-sample generation could have finished (about 11 s in a debug build).
#[test]
fn adversary_the_last_dense_paths_are_refused_before_the_generation() {
    for name in ["order.u32", "y.f32"] {
        let dir = scratch(&format!("existing-test-{name}"));
        let out = dir.join("data");
        fs::create_dir_all(out.join("test")).unwrap();
        fs::write(out.join("test").join(name), "kept\n").unwrap();
        let started = Instant::now();
        let output = generate("48", &out);
        let elapsed = started.elapsed();
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
        let stderr = text(&output.stderr);
        assert!(
            stderr.starts_with("patchback: generate-dataset refused: cannot write ")
                && stderr.contains(&format!("{:?}", out.join("test").join(name)))
                && stderr.contains("already exists"),
            "{stderr}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "{name}: refused after {elapsed:?}, so not by the pre-check"
        );
        assert_eq!(
            fs::read_to_string(out.join("test").join(name)).unwrap(),
            "kept\n"
        );
        assert_eq!(tree(&out), ["test".to_owned(), format!("test/{name}")]);
    }
}

/// A dense write that fails in the middle split (`val/y.f32` a dangling symbolic link, which the
/// pre-check does not see): the train split's four files, `val/samples.meta` and `val/x.f32` are
/// removed with the `train` directory the run created; an unrelated file in `<out>` and one in
/// the existing `val/` stay.
#[cfg(unix)]
#[test]
fn adversary_a_failed_dense_write_in_the_middle_split_keeps_what_the_run_did_not_create() {
    let dir = scratch("middle-split");
    let out = dir.join("data");
    fs::create_dir_all(out.join("val")).unwrap();
    fs::write(out.join("notes.txt"), "mine\n").unwrap();
    fs::write(out.join("val/keep.bin"), "mine too\n").unwrap();
    std::os::unix::fs::symlink(dir.join("nowhere"), out.join("val/y.f32")).unwrap();
    let output = generate("48", &out);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with("patchback: generate-dataset refused: cannot write ")
            && stderr.contains(&format!("{:?}", out.join("val").join("y.f32"))),
        "{stderr}"
    );
    assert_eq!(
        tree(&out),
        ["notes.txt", "val", "val/keep.bin", "val/y.f32"]
    );
    assert_eq!(fs::read_to_string(out.join("notes.txt")).unwrap(), "mine\n");
    assert!(!dir.join("nowhere").exists());
}
