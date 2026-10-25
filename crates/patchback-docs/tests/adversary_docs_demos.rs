//! Adversary cases for story docs-progress-and-demos (wave a12, pass 1). Every case runs the
//! `patchback-docs` binary, on the repository itself when it only reads, or on a scratch copy of
//! the repository under `target/tmp/` when it changes an input.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use patchback_core::digest::{hex, sha256};
use patchback_model::WeightArtifact;
use serde_json::Value;

const CHECKSUM: &str = "a17d810459ccfa9ce41c171661a25d5313d1f9e575125062a17b4e51bf9be926";
const STATS_SHA256: &str = "6a1772acb04b1a4b992e29fe7804185072677fa82f1534f1e04c6e110a0810c7";
const MODELSPEC_COMMIT: &str = "7c45c1adaf6e0cc0ba05f18878b31bfdc57cc529";
/// The parameters of `subtractive-mono/1` and their units, from `patchback-core/src/parameter.rs`.
const PARAMETERS: [(&str, &str); 8] = [
    ("attack", "s"),
    ("decay", "s"),
    ("sustain", ""),
    ("release", "s"),
    ("filter_cutoff", "Hz"),
    ("resonance_q", ""),
    ("drive", "dB"),
    ("output_gain", "dB"),
];
const VOICES: [&str; 4] = ["pluck", "pad", "bass", "lead"];
const TAKES: [&str; 4] = ["recording", "default-fit", "seeded-fit", "prediction"];
/// A committed input and how a case changes one of its bytes.
type Change = (&'static str, fn(&mut Vec<u8>));
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

/// A fresh directory `target/tmp/adv-docs-<name>-<pid>` with `repo/` and `tmp/` in it.
fn scratch(name: &str) -> PathBuf {
    let dir = repository()
        .join("target/tmp")
        .join(format!("adv-docs-{name}-{}", std::process::id()));
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

/// Runs the binary in `root` with `args`; `TMPDIR` is the scratch's `tmp/`.
fn run(root: &Path, args: &[&str]) -> Output {
    let tmp = root.parent().unwrap().join("tmp");
    Command::new(env!("CARGO_BIN_EXE_patchback-docs"))
        .args(args)
        .current_dir(root)
        .env("TMPDIR", &tmp)
        .output()
        .unwrap()
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

/// The section of `page` under `## <heading>`, up to the next `## ` heading.
fn section<'a>(page: &'a str, heading: &str) -> &'a str {
    let marker = format!("\n## {heading}\n");
    let start = page
        .find(&marker)
        .unwrap_or_else(|| panic!("no section {heading} in\n{page}"))
        + marker.len();
    let rest = &page[start..];
    &rest[..rest.find("\n## ").unwrap_or(rest.len())]
}

/// The table row of the story titled `title` (as the page writes it).
fn row<'a>(page: &'a str, title: &str) -> &'a str {
    page.lines()
        .find(|line| line.starts_with(&format!("| {title} |")))
        .unwrap_or_else(|| panic!("no row for {title} in\n{page}"))
}

fn story_file(
    slug: &str,
    title: &str,
    status: &str,
    relations: &str,
    implemented: Option<&str>,
) -> String {
    let transitions = implemented.map_or(String::new(), |at| {
        format!(
            "transitions:\n- {{from: \"draft\", to: \"active\", at: \"2026-10-01T00:00:00Z\", actor: \"human:timo\", revision: 2}}\n\
             - {{from: \"active\", to: \"implemented\", at: \"{at}\", actor: \"human:timo\", revision: 3}}\n"
        )
    });
    format!(
        "---\nformat: aep.planning-md/3\nid: story:{slug}\nkind: story\nstatus: {status}\ntitle: {title}\nrelations:\n{relations}- serves: vision:patchback\nrevision: 3\n{transitions}---\n\n# Body\n"
    )
}

fn add_story(
    root: &Path,
    slug: &str,
    title: &str,
    status: &str,
    relations: &str,
    implemented: Option<&str>,
) {
    fs::write(
        root.join(format!(".engineering/planning/story/{slug}.md")),
        story_file(slug, title, status, relations, implemented),
    )
    .unwrap();
}

fn progress(root: &Path) -> String {
    fs::read_to_string(root.join("website/docs/progress.mdx")).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Drift and idempotence

#[test]
fn adv_pristine_copy_checks_clean_and_generate_is_idempotent() {
    let dir = copy_repository("idempotent");
    let root = dir.join("repo");
    let started = Instant::now();
    let check = run(&root, &["generate", "--check"]);
    let check_ms = started.elapsed().as_millis();
    assert!(check.status.success(), "{}", describe(&check));
    assert!(check_ms < 30_000, "generate --check took {check_ms} ms");
    let before = generated(&root);
    for _ in 0..2 {
        let output = run(&root, &["generate"]);
        assert!(output.status.success(), "{}", describe(&output));
        assert!(
            before == generated(&root),
            "generate changed a generated file"
        );
    }
    let check = run(&root, &["generate", "--check"]);
    assert!(check.status.success(), "{}", describe(&check));
    let _ = fs::remove_dir_all(&dir);
}

/// One changed byte of each committed demo input: `generate --check`, `generate` and
/// `demos render` refuse, name the file, and write nothing.
#[test]
fn adv_one_byte_of_each_demo_input_is_refused_by_check_generate_and_render() {
    let dir = copy_repository("input-bytes");
    let root = dir.join("repo");
    let changes: [Change; 6] = [
        ("demos/pluck/voice.patch", |b| {
            let at = b.len() - 2;
            b[at] ^= 1;
        }),
        ("demos/lead/note.events", |b| {
            let at = b.len() - 2;
            b[at] ^= 1;
        }),
        ("models/demo/regressor.weights", |b| {
            let at = b.len() - 1;
            b[at] ^= 1;
        }),
        ("models/demo/regressor.weights", |b| b[3] ^= 1),
        ("models/demo/normalization.stats", |b| {
            let at = b.len() / 2;
            b[at] = if b[at] == b'1' { b'2' } else { b'1' };
        }),
        ("demos/pad/voice.patch", |b| b.push(b'\n')),
    ];
    let before = generated(&root);
    for (path, change) in changes {
        let original = fs::read(root.join(path)).unwrap();
        let mut changed = original.clone();
        change(&mut changed);
        assert_ne!(changed, original);
        fs::write(root.join(path), &changed).unwrap();

        let check = run(&root, &["generate", "--check"]);
        assert_eq!(check.status.code(), Some(1), "{path}: {}", describe(&check));
        assert!(
            stderr(&check).contains(path),
            "{path}: {}",
            describe(&check)
        );

        let plain = run(&root, &["generate"]);
        assert_eq!(plain.status.code(), Some(1), "{path}: {}", describe(&plain));
        assert!(
            stderr(&plain).contains(path),
            "{path}: {}",
            describe(&plain)
        );
        assert!(before == generated(&root), "{path}: generate wrote a file");

        let audio = dir.join("audio");
        let render = run(
            &root,
            &["demos", "render", "--out", audio.to_str().unwrap()],
        );
        assert_eq!(
            render.status.code(),
            Some(1),
            "{path}: {}",
            describe(&render)
        );
        assert!(
            stderr(&render).contains(path),
            "{path}: {}",
            describe(&render)
        );
        assert!(!audio.exists(), "{path}: render wrote audio");

        fs::write(root.join(path), &original).unwrap();
    }
    let check = run(&root, &["generate", "--check"]);
    assert!(check.status.success(), "{}", describe(&check));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adv_store_status_move_and_new_release_heading_fail_check_naming_progress() {
    let dir = copy_repository("store-drift");
    let root = dir.join("repo");
    let story = root.join(".engineering/planning/story/phrase-fit.md");
    let original = fs::read_to_string(&story).unwrap();
    assert!(original.contains("\nstatus: draft\n"));
    fs::write(
        &story,
        original.replace("\nstatus: draft\n", "\nstatus: active\n"),
    )
    .unwrap();
    let check = run(&root, &["generate", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", describe(&check));
    assert!(
        stderr(&check).contains("progress.mdx"),
        "{}",
        describe(&check)
    );
    fs::write(&story, &original).unwrap();

    let changelog = root.join("CHANGELOG.md");
    let text = fs::read_to_string(&changelog).unwrap();
    fs::write(
        &changelog,
        text.replacen(
            "## [Unreleased]\n",
            "## [Unreleased]\n\n## [0.99.0] - 2026-12-01\n\n- Native predictor again (story native-predictor).\n",
            1,
        ),
    )
    .unwrap();
    let check = run(&root, &["generate", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", describe(&check));
    assert!(
        stderr(&check).contains("progress.mdx"),
        "{}",
        describe(&check)
    );
    let plain = run(&root, &["generate"]);
    assert!(plain.status.success(), "{}", describe(&plain));
    let page = progress(&root);
    assert!(
        row(&page, "Native matmul/relu predictor").contains("[0.99.0]("),
        "{page}"
    );
    assert!(page.contains("Latest release: [0.99.0]("), "{page}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adv_a_displayed_loss_digit_in_demos_json_fails_check_naming_listen() {
    let dir = copy_repository("loss-digit");
    let root = dir.join("repo");
    let path = root.join("website/data/demos.json");
    let text = fs::read_to_string(&path).unwrap();
    let mut data: Value = serde_json::from_str(&text).unwrap();
    let best = data["voices"][0]["defaultFit"]["bestLoss"]
        .as_f64()
        .unwrap();
    data["voices"][0]["defaultFit"]["bestLoss"] = Value::from(best + 0.001);
    fs::write(&path, serde_json::to_string_pretty(&data).unwrap() + "\n").unwrap();
    let check = run(&root, &["generate", "--check"]);
    assert_eq!(check.status.code(), Some(1), "{}", describe(&check));
    assert!(
        stderr(&check).contains("listen.mdx"),
        "{}",
        describe(&check)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `demos.json` whose recovered vector no longer maps to the physical values it records (what a
/// change to a parameter contract's mapping does to a committed `demos.json`): the audio is
/// rendered from the vector, the page's table from the recorded physical values. Nothing may
/// publish a take that disagrees with the table beside it.
#[test]
fn adv_render_refuses_a_vector_that_disagrees_with_its_recorded_physical_values() {
    let dir = copy_repository("vector-physical");
    let root = dir.join("repo");
    let path = root.join("website/data/demos.json");
    let mut data: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let cutoff = &mut data["voices"][0]["seededFit"]["normalized"][4];
    let value = cutoff.as_f64().unwrap();
    *cutoff = Value::from(if value > 0.5 {
        value - 0.25
    } else {
        value + 0.25
    });
    fs::write(&path, serde_json::to_string_pretty(&data).unwrap() + "\n").unwrap();
    let audio = dir.join("audio");
    let render = run(
        &root,
        &["demos", "render", "--out", audio.to_str().unwrap()],
    );
    let check = run(&root, &["generate", "--check"]);
    assert!(
        !render.status.success() || !check.status.success(),
        "a seededFit.normalized that disagrees with seededFit.physical passes both:\nrender: {}\ncheck: {}",
        describe(&render),
        describe(&check)
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// The release rule and the store reader

const SYNTHETIC_CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n\
    - Pending (story adv-only-unreleased).\n\n\
    ## [0.3.0] - 2026-10-25\n\n### Added\n\n\
    - The seeded fit (story adv-seeded-fit), the export story:adv-wav-io-export and the wrapped\n  one (story\n  adv-wrapped).\n\
    - Three at once (stories adv-a, adv-b and\n  adv-c).\n\
    - At the line end story:adv-line-end\n  and more.\n\
    - Again (story adv-twice); a proposed one (story adv-proposed).\n\n\
    ## [0.2.0] - 2026-10-20\n\n- First (story adv-twice).\n\n\
    ## [0.1.0] - 2026-10-10\n\n- The scaffold.\n\n\
    [Unreleased]: https://github.com/beyond10x/patchback/compare/0.3.0...HEAD\n";

#[test]
fn adv_release_rule_on_a_synthetic_changelog() {
    let dir = copy_repository("release-rule");
    let root = dir.join("repo");
    fs::write(root.join("CHANGELOG.md"), SYNTHETIC_CHANGELOG).unwrap();
    let implemented = |slug: &str, title: &str, at: &str| {
        add_story(&root, slug, title, "implemented", "", Some(at))
    };
    implemented("adv-fit", "Adv fit", "2026-10-26T01:00:00Z");
    implemented("adv-seeded-fit", "Adv seeded fit", "2026-10-24T01:00:00Z");
    implemented("adv-wav-io", "Adv wav io", "2026-10-27T01:00:00Z");
    implemented(
        "adv-wav-io-export",
        "Adv wav io export",
        "2026-10-24T01:00:00Z",
    );
    implemented("adv-wrapped", "Adv wrapped", "2026-10-24T01:00:00Z");
    implemented("adv-a", "'Adv a: quoted ''x'''", "2026-10-24T01:00:00Z");
    implemented("adv-b", "\"Adv b \\\"dq\\\"\"", "2026-10-24T01:00:00Z");
    implemented(
        "adv-c",
        "Adv c | pipe {brace} <tag>",
        "2026-10-24T01:00:00Z",
    );
    implemented("adv-line-end", "Adv line end", "2026-10-24T01:00:00Z");
    implemented("adv-twice", "Adv twice", "2026-10-19T01:00:00Z");
    implemented(
        "adv-only-unreleased",
        "Adv only unreleased",
        "2026-10-01T01:00:00Z",
    );
    implemented("adv-same-day", "Adv same day", "2026-10-20T23:59:59Z");
    add_story(&root, "adv-proposed", "Adv proposed", "proposed", "", None);
    add_story(&root, "adv-active", "Adv active", "active", "", None);
    add_story(&root, "adv-odd", "Adv odd", "parked", "", None);
    add_story(
        &root,
        "adv-studio",
        "Adv studio",
        "draft",
        "- decomposes: epic:patch-studio\n",
        None,
    );

    let output = run(&root, &["generate"]);
    assert!(output.status.success(), "{}", describe(&output));
    let page = progress(&root);
    let release = |tag: &str| {
        format!(
            "| implemented | [{tag}](https://github.com/beyond10x/patchback/releases/tag/{tag}) |"
        )
    };
    assert!(
        row(&page, "Adv fit").ends_with("| implemented | Unreleased |"),
        "{page}"
    );
    assert!(row(&page, "Adv seeded fit").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv wav io").ends_with("| implemented | Unreleased |"));
    assert!(row(&page, "Adv wav io export").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv wrapped").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv a: quoted 'x'").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv b \"dq\"").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv c \\| pipe \\{brace\\} &lt;tag>").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv line end").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv twice").ends_with(&release("0.3.0")));
    assert!(row(&page, "Adv only unreleased").ends_with("| implemented | Unreleased |"));
    assert!(row(&page, "Adv same day").ends_with(
        "| implemented | [0.2.0](https://github.com/beyond10x/patchback/releases/tag/0.2.0) (by date) |"
    ));
    assert!(row(&page, "Adv proposed").ends_with("| proposed | — |"));
    assert!(row(&page, "Adv active").ends_with("| active | — |"));
    assert!(row(&page, "Adv odd").ends_with("| parked | — |"));
    assert!(page.contains("Latest release: [0.3.0](https://github.com/beyond10x/patchback/releases/tag/0.3.0), 2026-10-25."));
    let studio = section(
        &page,
        "Patch studio: see modules, patch graphs and how a fit evolves",
    );
    assert!(studio.contains("| Adv studio | draft | — |"), "{studio}");
    let outside = section(&page, "Outside the milestones");
    assert!(outside.contains("| Adv odd |"), "{outside}");
    assert!(outside.contains("1 parked"), "{outside}");
    let _ = fs::remove_dir_all(&dir);
}

/// "the first release dated on or after the day": dates compared as dates. A heading dated
/// `2026-9-5` is before 2026-10-25, so a story implemented on 2026-10-25 belongs to 0.2.0.
#[test]
fn adv_release_dates_compare_as_dates_not_strings() {
    let dir = copy_repository("dates");
    let root = dir.join("repo");
    fs::write(
        root.join("CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 2026-10-30\n\n- Later.\n\n## [0.1.0] - 2026-9-5\n\n- Earlier.\n",
    )
    .unwrap();
    add_story(
        &root,
        "adv-dated",
        "Adv dated",
        "implemented",
        "",
        Some("2026-10-25T10:00:00Z"),
    );
    let output = run(&root, &["generate"]);
    assert!(output.status.success(), "{}", describe(&output));
    let page = progress(&root);
    assert!(
        row(&page, "Adv dated").contains("[0.2.0]("),
        "released 2026-09-05, before the story was implemented on 2026-10-25: {}",
        row(&page, "Adv dated")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adv_store_reader_refusals_name_the_story() {
    let dir = copy_repository("refusals");
    let root = dir.join("repo");
    let cases: [(&str, String, &[&str]); 4] = [
        (
            "adv-ghost",
            story_file(
                "adv-ghost",
                "Adv ghost",
                "draft",
                "- decomposes: epic:no-such-epic\n",
                None,
            ),
            &["adv-ghost", "no-such-epic"],
        ),
        (
            "adv-bare",
            "# A story without front matter\n".to_owned(),
            &["adv-bare", "title"],
        ),
        (
            "adv-leak-epic",
            story_file(
                "adv-leak-epic",
                "Adv leak epic:sound-engine",
                "draft",
                "",
                None,
            ),
            &["progress.mdx", "planning ids"],
        ),
        (
            "adv-leak-story",
            story_file(
                "adv-leak-story",
                "'Adv leak story:phrase-fit'",
                "draft",
                "",
                None,
            ),
            &["progress.mdx", "planning ids"],
        ),
    ];
    for (slug, text, expected) in cases {
        let path = root.join(format!(".engineering/planning/story/{slug}.md"));
        fs::write(&path, text).unwrap();
        let output = run(&root, &["generate"]);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{slug}: {}",
            describe(&output)
        );
        for word in expected {
            assert!(
                stderr(&output).contains(word),
                "{slug}: {word}: {}",
                describe(&output)
            );
        }
        fs::remove_file(&path).unwrap();
    }
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// The committed pages against their sources

/// A YAML scalar as the store writes it.
fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        value[1..value.len() - 1].replace("''", "'")
    } else {
        value.to_owned()
    }
}

#[test]
fn adv_progress_counts_rows_and_order_equal_the_store() {
    let root = repository();
    let page = progress(&root);
    let mut stories = BTreeMap::new();
    for entry in fs::read_dir(root.join(".engineering/planning/story")).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        let front: Vec<&str> = text.lines().skip(1).take_while(|l| *l != "---").collect();
        let get = |key: &str| front.iter().find_map(|l| l.strip_prefix(key)).map(unquote);
        let title = get("title:").unwrap();
        let status = get("status:").unwrap();
        let at = front
            .iter()
            .filter(|l| l.contains("to: \"implemented\""))
            .filter_map(|l| l.split("at: \"").nth(1))
            .filter_map(|l| l.split('"').next())
            .next_back()
            .map(str::to_owned);
        stories.insert(title, (status, at));
    }
    let mut total = 0;
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let rank = |s: &str| {
        ["implemented", "active", "proposed", "draft"]
            .iter()
            .position(|r| *r == s)
            .unwrap_or(4)
    };
    for heading in page.lines().filter_map(|l| l.strip_prefix("## ")) {
        let body = section(&page, heading);
        let rows: Vec<&str> = body
            .lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Story |"))
            .collect();
        let count = body.lines().find(|l| l.contains(" stor")).unwrap();
        let n: usize = count.split(' ').next().unwrap().parse().unwrap();
        assert_eq!(n, rows.len(), "{heading}: {count}");
        let mut keys = Vec::new();
        for line in &rows {
            let cells: Vec<&str> = line.trim_matches('|').split(" | ").map(str::trim).collect();
            let (status, at) = &stories[cells[0]];
            assert_eq!(cells[1], status, "{line}");
            *tally.entry(status.clone()).or_default() += 1;
            keys.push((rank(status), at.clone()));
        }
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            keys, sorted,
            "{heading}: rows not ordered by status, then implementation date"
        );
        let mut per: BTreeMap<&str, usize> = BTreeMap::new();
        for line in &rows {
            *per.entry(line.split(" | ").nth(1).unwrap()).or_default() += 1;
        }
        for (status, k) in per {
            assert!(
                count.contains(&format!("{k} {status}")),
                "{heading}: {count}"
            );
        }
        total += rows.len();
    }
    assert_eq!(total, stories.len(), "every story file is one row");
    let top = format!(
        "{} stories ({} implemented, {} active, {} draft).",
        stories.len(),
        tally["implemented"],
        tally["active"],
        tally["draft"]
    );
    assert!(page.contains(&top), "{top}\n{page}");
    for title in stories.keys() {
        assert_eq!(page.matches(&format!("| {title} |")).count(), 1, "{title}");
    }
    for page in ["progress.mdx", "listen.mdx"] {
        let text = fs::read_to_string(root.join("website/docs").join(page)).unwrap();
        for id in ["story:", "epic:", "task:", "initiative:", "/home/"] {
            assert!(!text.contains(id), "{page}: {id}");
        }
    }
}

fn quantity(value: f64, unit: &str) -> String {
    match unit {
        "s" => format!("{value:.3} s"),
        "Hz" => format!("{value:.0} Hz"),
        "dB" => format!("{value:.1} dB"),
        _ => format!("{value:.2}"),
    }
}

#[test]
fn adv_listen_page_equals_demos_json_and_the_committed_inputs() {
    let root = repository();
    let text = fs::read_to_string(root.join("website/data/demos.json")).unwrap();
    assert!(text.len() < 64 * 1024, "{}", text.len());
    let data: Value = serde_json::from_str(&text).unwrap();
    let page = fs::read_to_string(root.join("website/docs/listen.mdx")).unwrap();
    let names: Vec<(&str, &str)> = data["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap(), p["unit"].as_str().unwrap()))
        .collect();
    assert_eq!(names, PARAMETERS);
    assert_eq!(data["model"]["checksum"], CHECKSUM);
    assert_eq!(data["model"]["modelspec"], "0.5.0");
    assert_eq!(data["model"]["modelspecCommit"], MODELSPEC_COMMIT);
    assert_eq!(data["fit"]["maxWallTimeMs"], 600_000);
    for (index, voice) in VOICES.iter().enumerate() {
        let record = &data["voices"][index];
        assert_eq!(record["name"], *voice);
        assert_eq!(record["inputs"]["statistics"]["sha256"], STATS_SHA256);
        let mut heading = voice.to_string();
        heading[..1].make_ascii_uppercase();
        let body = section(&page, &heading);
        let patch = fs::read_to_string(root.join(format!("demos/{voice}/voice.patch"))).unwrap();
        let notes = fs::read_to_string(root.join(format!("demos/{voice}/note.events"))).unwrap();
        let note: Vec<&str> = notes
            .lines()
            .find(|l| l.starts_with("note "))
            .unwrap()
            .split(' ')
            .collect();
        let (start, end): (f64, f64) = (note[1].parse().unwrap(), note[2].parse().unwrap());
        let expected = format!(
            "MIDI {}, velocity {}, a {:.2} s note; {:.2} s with its release.",
            note[3],
            note[4],
            (end - start) / 48_000.0,
            record["recording"]["seconds"].as_f64().unwrap()
        );
        assert!(
            body.contains(&expected.replacen(
                ", velocity",
                &format!(" ({}), velocity", midi_name(note[3])),
                1
            )),
            "{voice}: {expected}\n{body}"
        );
        assert_eq!(record["recording"]["limitedSamples"], 0, "{voice}");
        assert_eq!(
            body.matches(&format!(
                "**prediction**, {} of 8 outputs clamped",
                record["prediction"]["clamped"]
            ))
            .count(),
            1,
            "{voice}"
        );
        for (k, (name, unit)) in PARAMETERS.iter().enumerate() {
            let value: f64 = patch
                .lines()
                .find_map(|l| {
                    let fields: Vec<&str> = l.split(' ').collect();
                    (fields.len() == 4 && fields[0] == "param" && fields[2] == *name)
                        .then(|| fields[3].parse().unwrap())
                })
                .unwrap_or_else(|| panic!("{voice}: {name}"));
            let cell = |key: &str| quantity(record[key]["physical"][k].as_f64().unwrap(), unit);
            let line = format!(
                "| {name} | {} | {} | {} | {} |",
                quantity(value, unit),
                cell("defaultFit"),
                cell("seededFit"),
                cell("prediction")
            );
            assert!(body.contains(&line), "{voice}: {line}\n{body}");
            let p = record["prediction"]["normalized"][k].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&p), "{voice}: prediction {name} {p}");
        }
        for (label, key) in [
            ("Default start", "defaultFit"),
            ("Regressor-seeded", "seededFit"),
        ] {
            let fit = &record[key];
            let line = format!(
                "| {label} | {:.4} | {:.4} | {} |",
                fit["initialLoss"].as_f64().unwrap(),
                fit["bestLoss"].as_f64().unwrap(),
                fit["evaluations"]
            );
            assert!(body.contains(&line), "{voice}: {line}");
            assert_eq!(
                fit["stopReason"], "ImprovementBelowThreshold",
                "{voice} {key}"
            );
        }
    }
}

fn midi_name(midi: &str) -> String {
    const NAMES: [&str; 12] = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    let n: i64 = midi.parse().unwrap();
    format!("{}{}", NAMES[(n % 12) as usize], n / 12 - 1)
}

#[test]
fn adv_provenance_and_model_files_equal_the_recorded_run() {
    let model = repository().join("models/demo");
    let provenance = fs::read_to_string(model.join("PROVENANCE")).unwrap();
    for line in [
        "patchback-demo-provenance 1",
        "dataset_seed 42",
        "dataset_train 50016",
        "dataset_validation 4992",
        "dataset_test 4992",
        "training_steps 3126",
        "batch_size 16",
        "eval_loss 0.0600",
        "modelspec 0.5.0",
        "retrain_reproduces_checksum not-verified",
        "dataset_generation_s 1310",
    ] {
        assert!(provenance.lines().any(|l| l == line), "{line}");
    }
    assert!(provenance.starts_with("patchback-demo-provenance 1\n"));
    assert!(
        provenance
            .lines()
            .any(|l| l == format!("checksum {CHECKSUM}"))
    );
    assert!(
        provenance
            .lines()
            .any(|l| l == format!("statistics_sha256 {STATS_SHA256}"))
    );
    assert!(
        provenance
            .lines()
            .any(|l| l == format!("modelspec_commit {MODELSPEC_COMMIT}"))
    );
    let weights = model.join("regressor.weights");
    assert_eq!(fs::read(&weights).unwrap().len(), 10_438);
    let artifact = WeightArtifact::read(&weights).unwrap();
    assert_eq!(artifact.checksum(), CHECKSUM);
    assert_eq!(
        hex(&sha256(
            &fs::read(model.join("normalization.stats")).unwrap()
        )),
        STATS_SHA256
    );
}

// ---------------------------------------------------------------------------------------------
// Audio

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[test]
fn adv_render_twice_through_the_binary_gives_the_sixteen_files_the_page_requires() {
    let root = repository();
    let dir = scratch("render");
    let (a, b) = (dir.join("a"), dir.join("b"));
    // A stale tree: a wrong byte in one take and a file no take writes.
    fs::create_dir_all(b.join("pluck")).unwrap();
    fs::write(b.join("pluck/recording.wav"), b"stale").unwrap();
    fs::write(b.join("pluck/extra.wav"), b"stale").unwrap();
    for out in [&a, &b] {
        let output = Command::new(env!("CARGO_BIN_EXE_patchback-docs"))
            .args([
                "demos",
                "render",
                "--root",
                root.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", describe(&output));
    }
    let first = snapshot(&a);
    let mut second = snapshot(&b);
    assert_eq!(
        second.remove(&b.join("pluck/extra.wav")).as_deref(),
        Some(&b"stale"[..])
    );
    assert_eq!(first.len(), 16);
    for (path, bytes) in &first {
        let other = &second[&b.join(path.strip_prefix(&a).unwrap())];
        assert!(bytes == other, "{} differs between renders", path.display());
    }
    let page = fs::read_to_string(root.join("website/docs/listen.mdx")).unwrap();
    let required: Vec<String> = page
        .match_indices("require('@site/demo-audio/")
        .map(|(at, m)| {
            let rest = &page[at + m.len()..];
            rest[..rest.find('\'').unwrap()].to_owned()
        })
        .collect();
    let mut rendered: Vec<String> = first
        .keys()
        .map(|p| p.strip_prefix(&a).unwrap().to_string_lossy().into_owned())
        .collect();
    let mut sorted = required.clone();
    sorted.sort();
    rendered.sort();
    assert_eq!(sorted, rendered);
    let expected: Vec<String> = VOICES
        .iter()
        .flat_map(|v| TAKES.iter().map(move |t| format!("{v}/{t}.wav")))
        .collect();
    assert_eq!(required, expected);
    let data: Value =
        serde_json::from_str(&fs::read_to_string(root.join("website/data/demos.json")).unwrap())
            .unwrap();
    for (path, wav) in &first {
        let name = path.display();
        assert_eq!(&wav[0..4], b"RIFF", "{name}");
        assert_eq!(&wav[8..12], b"WAVE", "{name}");
        assert_eq!(u16_at(wav, 20), 1, "{name}");
        assert_eq!(u16_at(wav, 22), 1, "{name}");
        assert_eq!(u32_at(wav, 24), 48_000, "{name}");
        assert_eq!(u16_at(wav, 34), 16, "{name}");
        assert_eq!(&wav[36..40], b"data", "{name}");
        let frames = u32_at(wav, 40) as usize / 2;
        assert_eq!(wav.len(), 44 + 2 * frames, "{name}");
        assert!(frames > 0 && frames < 96_000, "{name}: {frames}");
        let peak = wav[44..]
            .chunks(2)
            .map(|c| i32::from(i16::from_le_bytes([c[0], c[1]])).abs())
            .max()
            .unwrap();
        assert!(peak > 0 && peak < 32_767, "{name}: peak {peak}");
        let relative = path.strip_prefix(&a).unwrap();
        let voice = relative.parent().unwrap().to_str().unwrap();
        let index = VOICES.iter().position(|v| *v == voice).unwrap();
        assert_eq!(
            data["voices"][index]["recording"]["frames"], frames,
            "{name}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}
