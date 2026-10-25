//! Adversary cases for story:fit-run-artifacts (wave a7, pass 1) through the binary: the block
//! size changes no file but its own record and the wall time, a failed write leaves an
//! unrelated file in `--out`, the reference digest is of the file's bytes on disk, and `--out`
//! as a dangling symbolic link is refused before the search.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_core::digest::{hex, sha256};

const RESULT_FILES: [&str; 6] = [
    "patch.patch",
    "notes.events",
    "reconstructed.wav",
    "report.txt",
    "run.meta",
    "trace.events",
];

fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("patchback-cli-fit-artifacts-adversary")
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

fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn reference_patch(dir: &Path) -> PathBuf {
    write(
        dir,
        "reference.patch",
        "patchback-patch 1\nsample_rate_hz 48000\ntopology_id subtractive-mono/1\n\
         node 1 NoteInput\nnode 2 SawOscillator\nnode 3 Drive\nparam 3 drive 12\n\
         node 4 LowpassFilter\nparam 4 filter_cutoff 178.88543819998318\nparam 4 resonance_q 6.25\n\
         node 5 AdsrEnvelope\nparam 5 attack 0.044721359549995794\nparam 5 decay 0.1\n\
         param 5 sustain 0.5\nparam 5 release 0.1\nnode 6 Vca\nnode 7 Output\n\
         param 7 output_gain -12\n\
         edge 1 pitch Control 2 pitch Control\nedge 1 gate Event 5 gate Event\n\
         edge 2 out Audio 3 in Audio\nedge 3 out Audio 4 in Audio\nedge 4 out Audio 6 in Audio\n\
         edge 5 env Control 6 gain Control\nedge 6 out Audio 7 in Audio\n",
    )
}

fn notes(dir: &Path) -> PathBuf {
    write(
        dir,
        "notes.events",
        "patchback-notes 1\nsample_rate_hz 48000\npatch_id 7f0c0000-0000-4000-8000-000000000911\n\
         note 0 12000 45 0.8\n",
    )
}

fn patchback(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback"))
        .args(args)
        .output()
        .expect("the patchback binary runs")
}

fn reference(dir: &Path) -> PathBuf {
    let patch = reference_patch(dir);
    let notes = notes(dir);
    let out = dir.join("reference.wav");
    let rendered = patchback(&[
        "render",
        "--patch",
        patch.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(
        rendered.status.code(),
        Some(0),
        "{}",
        text(&rendered.stderr)
    );
    out
}

fn fit_args<'a>(
    input: &'a Path,
    notes: &'a Path,
    out: &'a Path,
    extra: &[&'a str],
) -> Vec<&'a str> {
    let mut args = vec![
        "fit",
        "--input",
        input.to_str().unwrap(),
        "--notes",
        notes.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    args
}

fn without(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with(&format!("{key} ")))
        .map(str::to_owned)
        .collect()
}

/// § 2 invariant 5 with the render's block size varied: the samples do not depend on it, so the
/// six files are the same but for `report.txt`'s `wall_time_ms` and `run.meta`'s `block_size`.
#[test]
fn adversary_the_block_size_changes_no_file_but_its_record_and_the_wall_time() {
    let dir = scratch("block-size");
    let input = reference(&dir);
    let notes = notes(&dir);
    let (a, b) = (dir.join("a"), dir.join("b"));
    let small = ["--max-renders", "12", "--max-restarts", "1", "--seed", "5"];
    let mut first = small.to_vec();
    first.extend(["--block-size", "64"]);
    let mut second = small.to_vec();
    second.extend(["--block-size", "4096"]);
    let x = patchback(&fit_args(&input, &notes, &a, &first));
    let y = patchback(&fit_args(&input, &notes, &b, &second));
    assert_eq!(x.status.code(), Some(0), "{}", text(&x.stderr));
    assert_eq!(y.status.code(), Some(0), "{}", text(&y.stderr));
    for name in RESULT_FILES {
        let (p, q) = (
            fs::read(a.join(name)).unwrap(),
            fs::read(b.join(name)).unwrap(),
        );
        match name {
            "report.txt" => assert_eq!(
                without(&text(&p), "wall_time_ms"),
                without(&text(&q), "wall_time_ms")
            ),
            "run.meta" => {
                assert_eq!(
                    without(&text(&p), "block_size"),
                    without(&text(&q), "block_size")
                );
                assert!(text(&p).ends_with("\nblock_size 64\n"));
                assert!(text(&q).ends_with("\nblock_size 4096\n"));
            }
            _ => assert_eq!(p, q, "{name}"),
        }
    }
}

/// A write that fails after the search (`reconstructed.wav` past a 16-block file size limit)
/// into an existing `--out` holding an unrelated file removes the files the run wrote, the
/// partial WAV included, and leaves the directory and that file as they were. A control run
/// without the limit writes all six, so the refusal is the limit's.
#[cfg(unix)]
#[test]
fn adversary_a_failed_write_keeps_an_unrelated_file_and_removes_the_partial_wav() {
    let dir = scratch("failed-write-unrelated");
    let input = reference(&dir);
    let notes = notes(&dir);
    let small = ["--max-renders", "6", "--max-restarts", "1"];
    let control = dir.join("control");
    let ok = patchback(&fit_args(&input, &notes, &control, &small));
    assert_eq!(ok.status.code(), Some(0), "{}", text(&ok.stderr));
    assert!(
        fs::metadata(control.join("reconstructed.wav"))
            .unwrap()
            .len()
            > 64 * 1024
    );

    let out = dir.join("results");
    fs::create_dir_all(&out).unwrap();
    write(&out, "unrelated.txt", "keep\n");
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("ulimit -f 16 && trap '' XFSZ && exec \"$0\" \"$@\"")
        .arg(env!("CARGO_BIN_EXE_patchback"))
        .args(fit_args(&input, &notes, &out, &small))
        .output()
        .expect("sh runs");
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.starts_with(&format!(
            "patchback: fit refused: cannot write {:?}: ",
            out.join("reconstructed.wav")
        )),
        "{stderr}"
    );
    let mut names: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["unrelated.txt"]);
    assert_eq!(
        fs::read_to_string(out.join("unrelated.txt")).unwrap(),
        "keep\n"
    );
}

/// A reference WAV with bytes after its RIFF body is imported, and `run.meta` records the
/// SHA-256 of the file's bytes on disk, the trailing bytes included.
#[test]
fn adversary_the_reference_digest_is_of_every_byte_on_disk() {
    let dir = scratch("trailing");
    let input = reference(&dir);
    let mut bytes = fs::read(&input).unwrap();
    bytes.extend_from_slice(b"trailing junk");
    fs::write(&input, &bytes).unwrap();
    let notes = notes(&dir);
    let out = dir.join("results");
    let output = patchback(&fit_args(&input, &notes, &out, &["--max-renders", "3"]));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let meta = fs::read_to_string(out.join("run.meta")).unwrap();
    assert!(
        meta.contains(&format!("\nreference sha256 {}\n", hex(&sha256(&bytes)))),
        "{meta}"
    );
}

/// `--out` a dangling symbolic link, or a link to a file, is refused before the search and
/// nothing is created at the link's target.
#[cfg(unix)]
#[test]
fn adversary_an_out_that_is_a_dangling_link_or_a_link_to_a_file_is_refused() {
    let dir = scratch("out-link");
    let input = reference(&dir);
    let notes = notes(&dir);
    let dangling = dir.join("dangling");
    let target = dir.join("nowhere");
    std::os::unix::fs::symlink(&target, &dangling).unwrap();
    let output = patchback(&fit_args(
        &input,
        &notes,
        &dangling,
        &["--max-renders", "3"],
    ));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stdout));
    assert_eq!(
        text(&output.stderr),
        format!("patchback: fit refused: cannot write {dangling:?}: is not a directory\n")
    );
    assert!(fs::symlink_metadata(&target).is_err());

    let file = write(&dir, "a-file", "keep\n");
    let to_file = dir.join("to-file");
    std::os::unix::fs::symlink(&file, &to_file).unwrap();
    let output = patchback(&fit_args(&input, &notes, &to_file, &["--max-renders", "3"]));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stdout));
    assert_eq!(fs::read_to_string(&file).unwrap(), "keep\n");
}
