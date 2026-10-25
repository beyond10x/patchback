//! Adversary cases for story docs-progress-and-demos (wave a12, pass 2, after correction 1).
//! Every case runs the `patchback-docs` binary, on the repository itself when it only reads, or
//! on a scratch copy of the repository under `target/tmp/` when it changes an input.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patchback_core::digest::{hex, sha256};
use serde_json::Value;

const VOICES: [&str; 4] = ["pluck", "pad", "bass", "lead"];
const TAKES: [&str; 4] = ["recording", "default-fit", "seeded-fit", "prediction"];
const LABELS: [&str; 4] = [
    "recording",
    "default-start fit",
    "regressor-seeded fit",
    "prediction",
];
const DATA: &str = "website/data/demos.json";
/// What a scratch copy leaves out, relative to the repository root.
const NOT_COPIED: [&str; 7] = [
    ".git",
    "target",
    "studio",
    "website/node_modules",
    "website/build",
    "website/.docusaurus",
    "website/demo-audio",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// A fresh directory `target/tmp/adv2-docs-<name>-<pid>` with `tmp/` in it.
fn scratch(name: &str) -> PathBuf {
    let dir = repository()
        .join("target/tmp")
        .join(format!("adv2-docs-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("tmp")).unwrap();
    dir
}

fn copy_tree(from: &Path, to: &Path, root: &Path) {
    fs::create_dir_all(to).unwrap();
    let mut entries: Vec<_> = fs::read_dir(from).unwrap().map(|e| e.unwrap()).collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if NOT_COPIED.contains(&relative.as_str()) {
            continue;
        }
        let target = to.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_tree(&path, &target, root);
        } else if kind.is_file() {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// A scratch copy of the repository: `<scratch>/repo`.
fn copy_repository(name: &str) -> PathBuf {
    let dir = scratch(name);
    let source = repository();
    copy_tree(&source, &dir.join("repo"), &source);
    dir
}

/// Runs the binary in `cwd` with `args`; `TMPDIR` is `tmp`.
fn run_in(cwd: &Path, tmp: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patchback-docs"))
        .args(args)
        .current_dir(cwd)
        .env("TMPDIR", tmp)
        .output()
        .unwrap()
}

/// Runs the binary in the copy `<dir>/repo`.
fn run(dir: &Path, args: &[&str]) -> Output {
    run_in(&dir.join("repo"), &dir.join("tmp"), args)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn describe(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        stderr(output)
    )
}

/// Every file under `dir`, recursively, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    if !dir.exists() {
        return found;
    }
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(snapshot(&path));
        } else {
            found.insert(path.clone(), fs::read(&path).unwrap());
        }
    }
    found
}

fn generated(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut all = snapshot(&root.join("website/docs"));
    all.extend(snapshot(&root.join("website/data")));
    all
}

/// The `data` chunk of a RIFF/WAVE file, found by walking the chunk list.
fn data_chunk(wav: &[u8]) -> &[u8] {
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    let mut at = 12;
    while at + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[at + 4..at + 8].try_into().unwrap()) as usize;
        if &wav[at..at + 4] == b"data" {
            return &wav[at + 8..at + 8 + size];
        }
        at += 8 + size + size % 2;
    }
    panic!("no data chunk");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_string_pretty(value).unwrap() + "\n").unwrap();
}

fn story_file(slug: &str, title: &str, implemented: &str) -> String {
    format!(
        "---\nformat: aep.planning-md/3\nid: story:{slug}\nkind: story\nstatus: implemented\ntitle: {title}\nrelations:\n- serves: vision:patchback\nrevision: 3\n\
         transitions:\n- {{from: \"draft\", to: \"active\", at: \"2026-08-01T00:00:00Z\", actor: \"human:timo\", revision: 2}}\n\
         - {{from: \"active\", to: \"implemented\", at: \"{implemented}\", actor: \"human:timo\", revision: 3}}\n---\n\n# Body\n"
    )
}

fn add_story(root: &Path, slug: &str, title: &str, implemented: &str) {
    fs::write(
        root.join(format!(".engineering/planning/story/{slug}.md")),
        story_file(slug, title, implemented),
    )
    .unwrap();
}

/// The table row of the story titled `title` on the progress page.
fn row(root: &Path, title: &str) -> String {
    let page = fs::read_to_string(root.join("website/docs/progress.mdx")).unwrap();
    page.lines()
        .find(|line| line.starts_with(&format!("| {title} |")))
        .unwrap_or_else(|| panic!("no row for {title} in\n{page}"))
        .to_owned()
}

// ---------------------------------------------------------------------------------------------
// The take digests

/// The 16 recorded digests are the SHA-256 of the `data` chunk (the PCM16 sample bytes) of the
/// 16 files a fresh `demos render` writes, not of the whole WAV; each voice records exactly the
/// four takes; the listen page shows the same 16; two renders are byte-identical.
#[test]
fn adv2_take_digests_are_the_data_chunks_of_the_files_render_writes() {
    let dir = scratch("take-digests");
    let repository = repository();
    let tmp = dir.join("tmp");
    let data = read_json(&repository.join(DATA));
    let page = fs::read_to_string(repository.join("website/docs/listen.mdx")).unwrap();
    let mut renders = Vec::new();
    for n in 0..2 {
        let audio = dir.join(format!("audio-{n}"));
        let output = run_in(
            &repository,
            &tmp,
            &["demos", "render", "--out", audio.to_str().unwrap()],
        );
        assert!(output.status.success(), "{}", describe(&output));
        renders.push(snapshot(&audio));
    }
    assert_eq!(renders[0].len(), 16);
    let first: Vec<_> = renders[0].values().collect();
    let second: Vec<_> = renders[1].values().collect();
    assert!(first == second, "two renders differ");
    for (index, voice) in VOICES.iter().enumerate() {
        let record = &data["voices"][index];
        assert_eq!(record["name"], *voice);
        let digests = record["pcm16Sha256"].as_object().unwrap();
        let mut keys: Vec<&str> = digests.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = TAKES.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "{voice}");
        for (take, label) in TAKES.iter().zip(LABELS) {
            let recorded = digests[*take].as_str().unwrap();
            assert_eq!(recorded.len(), 64, "{voice}/{take}");
            assert!(
                recorded
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{voice}/{take}: {recorded}"
            );
            let wav = &renders[0][&dir.join(format!("audio-0/{voice}/{take}.wav"))];
            assert_eq!(
                hex(&sha256(data_chunk(wav))),
                recorded,
                "{voice}/{take}: the data chunk"
            );
            assert_ne!(
                hex(&sha256(wav)),
                recorded,
                "{voice}/{take}: the whole file"
            );
            let line = format!("| {voice} | {label} | `{recorded}` |");
            assert_eq!(page.matches(&line).count(), 1, "{line}");
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

/// One take digest changed in several ways: `demos render`, `generate --check` and `generate`
/// each exit 1, name that voice and take and no other, say `task docs-demos`, and write nothing.
#[test]
fn adv2_a_changed_take_digest_is_refused_naming_the_take_and_writing_nothing() {
    let dir = copy_repository("digest-refusals");
    let root = dir.join("repo");
    let path = root.join(DATA);
    let original = fs::read_to_string(&path).unwrap();
    type Variant = (&'static str, usize, &'static str, fn(&mut Value));
    let variants: [Variant; 5] = [
        ("one hex digit changed", 1, "default-fit", |d| {
            let mut s = d.as_str().unwrap().to_owned();
            let last = if s.ends_with('0') { "1" } else { "0" };
            s.replace_range(63..64, last);
            *d = Value::from(s);
        }),
        ("63 characters", 2, "seeded-fit", |d| {
            let s = d.as_str().unwrap()[..63].to_owned();
            *d = Value::from(s);
        }),
        ("not hex", 3, "recording", |d| {
            *d = Value::from("z".repeat(64))
        }),
        ("missing", 1, "recording", |d| *d = Value::Null),
        ("a number", 0, "prediction", |d| *d = Value::from(0)),
    ];
    for (what, voice_index, take, change) in variants {
        let voice = VOICES[voice_index];
        let mut data: Value = serde_json::from_str(&original).unwrap();
        let digests = &mut data["voices"][voice_index]["pcm16Sha256"];
        if what == "missing" {
            digests.as_object_mut().unwrap().remove(take);
        } else {
            change(&mut digests[take]);
        }
        write_json(&path, &data);
        let before = generated(&root);
        let audio = dir.join("audio");
        let named = format!("{voice}/{take}");
        let runs = [
            (
                "render",
                run(&dir, &["demos", "render", "--out", audio.to_str().unwrap()]),
            ),
            ("check", run(&dir, &["generate", "--check"])),
            ("generate", run(&dir, &["generate"])),
        ];
        for (verb, output) in &runs {
            let context = format!("{what}, {verb}: {}", describe(output));
            assert_eq!(output.status.code(), Some(1), "{context}");
            let error = stderr(output);
            assert!(error.contains(&named), "{context}");
            assert!(error.contains("task docs-demos"), "{context}");
            for other_voice in VOICES {
                for other_take in TAKES {
                    let other = format!("{other_voice}/{other_take}");
                    if other != named {
                        assert!(!error.contains(&other), "{context}");
                    }
                }
            }
        }
        assert!(
            !audio.exists(),
            "{what}: render created {}",
            audio.display()
        );
        assert!(
            before == generated(&root),
            "{what}: a generated file changed"
        );
    }
    fs::write(&path, &original).unwrap();
    let _ = fs::remove_dir_all(&dir);
}

/// The listen page shows each fit's physical values beside the take rendered from its
/// normalized vector. `seededFit.physical` edited with the vector, the inputs and the take
/// digests untouched: `generate` publishes a cutoff the take was not rendered with, and
/// `generate --check` and `demos render` then pass. `physical` is `patch_at(normalized)`
/// (`patchback-fit/src/search.rs:342`), so it can be recomputed and compared.
#[test]
fn adv2_a_displayed_physical_value_that_the_take_does_not_render_is_refused() {
    let dir = copy_repository("physical");
    let root = dir.join("repo");
    let path = root.join(DATA);
    let mut data = read_json(&path);
    let cutoff = &mut data["voices"][0]["seededFit"]["physical"][4];
    let recorded = cutoff.as_f64().unwrap();
    *cutoff = Value::from(recorded + 1000.0);
    write_json(&path, &data);
    let generate = run(&dir, &["generate"]);
    let check = run(&dir, &["generate", "--check"]);
    let audio = dir.join("audio");
    let render = run(&dir, &["demos", "render", "--out", audio.to_str().unwrap()]);
    let page = fs::read_to_string(root.join("website/docs/listen.mdx")).unwrap();
    let shown = format!("{:.0} Hz", recorded + 1000.0);
    assert!(
        !generate.status.success() || !check.status.success() || !render.status.success(),
        "pluck's seeded fit: the page shows filter_cutoff {shown} (on the page: {}), the take renders {recorded:.0} Hz; \
         generate, generate --check and demos render all pass:\ngenerate: {}\ncheck: {}\nrender: {}",
        page.contains(&shown),
        describe(&generate),
        describe(&check),
        describe(&render)
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// CHANGELOG dates

/// A versioned heading with no date (`## [0.2.0] - `, which `task release-check`'s prefix test
/// accepts) does not read as YYYY-MM-DD: refused by name, not shown as Unreleased.
#[test]
fn adv2_a_versioned_heading_without_a_date_is_refused_by_name() {
    let dir = copy_repository("undated");
    let root = dir.join("repo");
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - \n\n- The thing (story adv-undated).\n\n## [0.1.0] - 2026-10-10\n\n- The scaffold.\n",
    )
    .unwrap();
    add_story(&root, "adv-undated", "Adv undated", "2026-10-20T10:00:00Z");
    let output = run(&dir, &["generate"]);
    assert!(
        output.status.code() == Some(1) && stderr(&output).contains("0.2.0"),
        "`## [0.2.0] - ` is accepted; the story it names shows: {}\n{}",
        if output.status.success() {
            row(&root, "Adv undated")
        } else {
            String::new()
        },
        describe(&output)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `26-10-30` is not YYYY-MM-DD. Read as the year 26 it sorts before every story, so a release
/// on the story's own week is skipped: refused by name, or read as 2026.
#[test]
fn adv2_a_two_digit_year_is_refused_or_read_as_the_century_it_means() {
    let dir = copy_repository("two-digit-year");
    let root = dir.join("repo");
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 26-10-30\n\n- Later.\n\n## [0.1.0] - 2026-10-10\n\n- The scaffold.\n",
    )
    .unwrap();
    add_story(&root, "adv-year", "Adv year", "2026-10-25T10:00:00Z");
    let output = run(&dir, &["generate"]);
    let refused = output.status.code() == Some(1) && stderr(&output).contains("0.2.0");
    let shown = if output.status.success() {
        row(&root, "Adv year")
    } else {
        String::new()
    };
    assert!(
        refused || shown.contains("[0.2.0]("),
        "`## [0.2.0] - 26-10-30` is read as the year 26; the story implemented 2026-10-25 shows: {shown}\n{}",
        describe(&output)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Two releases on one day, a story implemented that day, one after every release and one
/// before every release but the first.
#[test]
fn adv2_same_day_releases_and_releases_before_a_story() {
    let dir = copy_repository("same-day");
    let root = dir.join("repo");
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.3.0] - 2026-10-25\n\n- B.\n\n## [0.2.0] - 2026-10-25\n\n- A.\n\n## [0.1.0] - 2026-10-01\n\n- The scaffold.\n",
    )
    .unwrap();
    add_story(&root, "adv-same", "Adv same", "2026-10-25T23:59:59Z");
    add_story(&root, "adv-after", "Adv after", "2026-10-26T00:00:00Z");
    add_story(&root, "adv-early", "Adv early", "2026-09-30T00:00:00Z");
    let output = run(&dir, &["generate"]);
    assert!(output.status.success(), "{}", describe(&output));
    let tag = "https://github.com/beyond10x/patchback/releases/tag";
    assert!(
        row(&root, "Adv same").contains(&format!("[0.2.0]({tag}/0.2.0) (by date)")),
        "{}",
        row(&root, "Adv same")
    );
    assert!(
        row(&root, "Adv after").contains("| Unreleased |"),
        "{}",
        row(&root, "Adv after")
    );
    assert!(
        row(&root, "Adv early").contains(&format!("[0.1.0]({tag}/0.1.0) (by date)")),
        "{}",
        row(&root, "Adv early")
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// --out and --root

/// `demos render --root <copy> --out <relative>` run from another directory writes under the
/// copy, creating missing directories; an absolute `--out` stays absolute. The correction's unit
/// test calls `under_root` itself, so only a run of the binary covers `main`'s dispatch.
#[test]
fn adv2_render_out_resolves_against_root_through_the_binary() {
    let dir = copy_repository("out-root");
    let root = dir.join("repo");
    let elsewhere = dir.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let tmp = dir.join("tmp");
    let output = run_in(
        &elsewhere,
        &tmp,
        &[
            "demos",
            "render",
            "--root",
            root.to_str().unwrap(),
            "--out",
            "rel/nested/audio",
        ],
    );
    assert!(output.status.success(), "{}", describe(&output));
    assert_eq!(snapshot(&root.join("rel/nested/audio")).len(), 16);
    assert!(snapshot(&elsewhere).is_empty(), "wrote under the cwd");
    let absolute = dir.join("absolute/audio");
    let output = run_in(
        &elsewhere,
        &tmp,
        &[
            "demos",
            "render",
            "--root",
            root.to_str().unwrap(),
            "--out",
            absolute.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{}", describe(&output));
    assert_eq!(snapshot(&absolute).len(), 16);
    assert!(snapshot(&elsewhere).is_empty(), "wrote under the cwd");
    let _ = fs::remove_dir_all(&dir);
}
