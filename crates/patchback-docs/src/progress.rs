//! The progress page, `website/docs/progress.mdx`: every milestone epic of the planning store
//! with its stories, each story's status and the release that delivered it, read from the
//! store's files and `CHANGELOG.md` as text (`aep` is not installed where the site is built).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The page's file name under `website/docs`.
pub const PAGE: &str = "progress.mdx";

/// A story of the planning store, as the page needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Story {
    pub slug: String,
    pub title: String,
    pub status: String,
    /// The epic it decomposes, by slug.
    pub epic: Option<String>,
    /// The `at` of its last transition to `implemented`.
    pub implemented_at: Option<String>,
}

/// An epic of the planning store.
#[derive(Clone, Debug, PartialEq)]
pub struct Epic {
    pub slug: String,
    pub title: String,
}

/// One `## [x.y.z] - YYYY-MM-DD` section of the CHANGELOG, or `## [Unreleased]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    /// The heading's date as written.
    pub date: Option<String>,
    /// The heading's date read as numbers.
    pub day: Option<Day>,
    /// The story slugs the section names.
    pub names: BTreeSet<String>,
}

/// A calendar day as (year, month, day), ordered as dates are.
pub type Day = (u32, u32, u32);

/// `YYYY-MM-DD` read as a [`Day`]: a four-digit year, a month 1 to 12 and a day 1 to 31, each
/// of one or two digits (`2026-9-5` reads as 2026-09-05); anything else is `None`.
pub fn read_day(text: &str) -> Option<Day> {
    let number = |part: &str, digits: std::ops::RangeInclusive<usize>| {
        (digits.contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    let mut parts = text.split('-');
    let day = (
        number(parts.next()?, 4..=4)?,
        number(parts.next()?, 1..=2)?,
        number(parts.next()?, 1..=2)?,
    );
    (parts.next().is_none() && (1..=12).contains(&day.1) && (1..=31).contains(&day.2))
        .then_some(day)
}

/// The planning store's directory, relative to the repository root.
const STORE: &str = ".engineering/planning";
/// The CHANGELOG, relative to the repository root.
const CHANGELOG: &str = "CHANGELOG.md";
/// The one CHANGELOG heading without a date.
const UNRELEASED: &str = "Unreleased";
/// Where a release's notes are published.
const RELEASES_URL: &str = "https://github.com/beyond10x/patchback/releases/tag";
/// The status of a story that is done.
const IMPLEMENTED: &str = "implemented";
/// What a story with no release shows.
const NONE: &str = "—";
/// The statuses in the order the page counts and sorts them; any other comes after, by name.
const STATUS_ORDER: [&str; 4] = [IMPLEMENTED, "active", "proposed", "draft"];

/// The front matter of a planning file: its lines between the opening and the closing `---`.
fn front_matter(text: &str) -> Vec<&str> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Vec::new();
    }
    lines.take_while(|line| *line != "---").collect()
}

/// A YAML scalar as the store writes it: plain, single-quoted (`''` for a quote) or
/// double-quoted (`\"` and `\\`).
fn scalar(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        inner.replace("''", "'")
    } else if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        value.to_owned()
    }
}

/// The top-level `key:` of a front matter.
fn field(lines: &[&str], key: &str) -> Option<String> {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .map(scalar)
}

/// The value of `key: "<value>"` inside a flow mapping such as a `transitions:` line.
fn flow_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = &line[line.find(&format!("{key}: \""))? + key.len() + 3..];
    rest.split('"').next()
}

/// The `.md` files of the store's `kind` directory, by slug.
fn planning_files(root: &Path, kind: &str) -> Result<Vec<(String, String)>, String> {
    let dir = root.join(STORE).join(kind);
    let mut files = Vec::new();
    for entry in crate::sorted_entries(&dir)? {
        let path = entry.path();
        let Some(slug) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".md"))
        else {
            continue;
        };
        let text = fs::read_to_string(&path).map_err(crate::io(path.display()))?;
        files.push((slug.to_owned(), text));
    }
    Ok(files)
}

/// Every story of the planning store at `root`.
pub fn read_stories(root: &Path) -> Result<Vec<Story>, String> {
    let mut stories = Vec::new();
    for (slug, text) in planning_files(root, "story")? {
        let lines = front_matter(&text);
        let missing = |key: &str| format!("the story {slug} has no {key} in its front matter");
        let title = field(&lines, "title").ok_or_else(|| missing("title"))?;
        let status = field(&lines, "status").ok_or_else(|| missing("status"))?;
        let epic = lines.iter().find_map(|line| {
            line.trim()
                .strip_prefix("- decomposes:")
                .map(|target| target.trim())
                .and_then(|target| target.strip_prefix("epic:"))
                .map(str::to_owned)
        });
        let implemented_at = lines
            .iter()
            .filter(|line| line.starts_with("- {"))
            .filter(|line| flow_value(line, "to") == Some(IMPLEMENTED))
            .filter_map(|line| flow_value(line, "at"))
            .next_back()
            .map(str::to_owned);
        stories.push(Story {
            slug,
            title,
            status,
            epic,
            implemented_at,
        });
    }
    Ok(stories)
}

/// Every epic of the planning store at `root`.
pub fn read_epics(root: &Path) -> Result<Vec<Epic>, String> {
    planning_files(root, "epic")?
        .into_iter()
        .map(|(slug, text)| {
            let title = field(&front_matter(&text), "title")
                .ok_or_else(|| format!("the epic {slug} has no title in its front matter"))?;
            Ok(Epic { slug, title })
        })
        .collect()
}

/// The story slugs `section` names: `(story <slug>)`, `(stories … <slug> …)` and
/// `story:<slug>`, with the section's lines joined so a mention wrapped across lines counts.
fn named_stories(section: &str) -> BTreeSet<String> {
    let joined = section.split_whitespace().collect::<Vec<_>>().join(" ");
    let is_slug = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    let mut names = BTreeSet::new();
    for opener in ["(story ", "(stories "] {
        for (at, _) in joined.match_indices(opener) {
            let inside = &joined[at + opener.len()..];
            let inside = &inside[..inside.find(')').unwrap_or(inside.len())];
            names.extend(
                inside
                    .split(|c: char| !is_slug(c))
                    .filter(|token| !token.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    for (at, _) in joined.match_indices("story:") {
        let rest = &joined[at + "story:".len()..];
        let slug: String = rest.chars().take_while(|c| is_slug(*c)).collect();
        if !slug.is_empty() {
            names.insert(slug);
        }
    }
    names
}

/// The CHANGELOG's sections in file order, newest first: `## [Unreleased]` (no date) and every
/// `## [x.y.z] - YYYY-MM-DD`, each with the story slugs it names.
pub fn read_releases(changelog: &str) -> Result<Vec<Release>, String> {
    let mut releases = Vec::new();
    let mut current: Option<(String, Option<String>, Option<Day>, String)> = None;
    let finish = |current: Option<(String, Option<String>, Option<Day>, String)>,
                  releases: &mut Vec<Release>| {
        if let Some((version, date, day, body)) = current {
            releases.push(Release {
                version,
                date,
                day,
                names: named_stories(&body),
            });
        }
    };
    for line in changelog.lines() {
        if let Some((version, after)) = line
            .strip_prefix("## [")
            .and_then(|rest| rest.split_once(']'))
        {
            finish(current.take(), &mut releases);
            let (date, day) = if version == UNRELEASED {
                (None, None)
            } else {
                let text = after.strip_prefix(" - ").unwrap_or(after).trim();
                let day = read_day(text).ok_or_else(|| {
                    format!(
                        "{CHANGELOG}: the heading of {version} is dated `{text}`, which does not read as YYYY-MM-DD; only [{UNRELEASED}] is undated"
                    )
                })?;
                (Some(text.to_owned()), Some(day))
            };
            current = Some((version.to_owned(), date, day, String::new()));
        } else if line.starts_with("# ") || line.starts_with("[") && line.contains("]: ") {
            finish(current.take(), &mut releases);
        } else if let Some((_, _, _, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    finish(current, &mut releases);
    Ok(releases)
}

/// A release as the page shows it: a link to its notes, or `Unreleased`.
fn release_cell(release: &Release) -> String {
    if release.date.is_none() {
        "Unreleased".to_owned()
    } else {
        format!("[{0}]({RELEASES_URL}/{0})", release.version)
    }
}

/// The release cell of `story`: for an implemented story the newest release naming it, else
/// the first release dated on or after the day of its `implemented` transition, marked
/// "by date", else `Unreleased`; for any other story [`NONE`].
pub fn release_of(story: &Story, releases: &[Release]) -> String {
    if story.status != IMPLEMENTED {
        return NONE.to_owned();
    }
    if let Some(release) = releases.iter().find(|r| r.names.contains(&story.slug)) {
        return release_cell(release);
    }
    let Some(day) = story
        .implemented_at
        .as_deref()
        .and_then(|at| read_day(at.split_once('T').map_or(at, |(day, _)| day)))
    else {
        return NONE.to_owned();
    };
    releases
        .iter()
        .rev()
        .find(|r| r.day.is_some_and(|released| released >= day))
        .map_or_else(
            || "Unreleased".to_owned(),
            |release| format!("{} (by date)", release_cell(release)),
        )
}

/// Where `status` sorts: [`STATUS_ORDER`], then any other.
fn status_rank(status: &str) -> usize {
    STATUS_ORDER
        .iter()
        .position(|s| *s == status)
        .unwrap_or(STATUS_ORDER.len())
}

/// "3 stories (2 implemented, 1 draft)".
fn counts(stories: &[&Story]) -> String {
    let mut by_status: Vec<(usize, &str, usize)> = Vec::new();
    for story in stories {
        match by_status.iter_mut().find(|(_, s, _)| *s == story.status) {
            Some((_, _, n)) => *n += 1,
            None => by_status.push((status_rank(&story.status), &story.status, 1)),
        }
    }
    by_status.sort();
    let noun = if stories.len() == 1 {
        "story"
    } else {
        "stories"
    };
    let parts: Vec<String> = by_status
        .iter()
        .map(|(_, status, n)| format!("{n} {status}"))
        .collect();
    format!("{} {noun} ({})", stories.len(), parts.join(", "))
}

/// Text for a Markdown table cell or heading in MDX: no pipe, brace or tag can break it.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "&lt;")
}

/// One section: a heading, the counts and the table of `stories`.
fn section(out: &mut String, heading: &str, stories: &mut [&Story], releases: &[Release]) {
    out.push_str(&format!("\n## {}\n\n", escape(heading)));
    if stories.is_empty() {
        out.push_str("No stories yet.\n");
        return;
    }
    stories.sort_by(|a, b| {
        (
            status_rank(&a.status),
            &a.status,
            &a.implemented_at,
            &a.title,
        )
            .cmp(&(
                status_rank(&b.status),
                &b.status,
                &b.implemented_at,
                &b.title,
            ))
    });
    out.push_str(&format!("{}.\n\n", counts(stories)));
    out.push_str("| Story | Status | Release |\n|---|---|---|\n");
    for story in stories.iter() {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            escape(&story.title),
            escape(&story.status),
            release_of(story, releases)
        ));
    }
}

/// The progress page: the latest release, then one section per epic (the milestones by title,
/// then the other epics by title) and "Outside the milestones" for stories of no epic.
pub fn page(epics: &[Epic], stories: &[Story], releases: &[Release]) -> String {
    let latest = releases.iter().find(|r| r.date.is_some()).map_or_else(
        || "No release yet.".to_owned(),
        |r| {
            format!(
                "Latest release: {}, {}.",
                release_cell(r),
                r.date.as_deref().unwrap_or_default()
            )
        },
    );
    let all: Vec<&Story> = stories.iter().collect();
    let mut out = format!(
        "---\n{header}\ntitle: Progress\nsidebar_position: 4\ndescription: \"What has been built so far, milestone by milestone, and the release that delivered each part.\"\ncustom_edit_url: null\n---\n\n\
         {mdx}\n\n\
         # Progress\n\n\
         {latest} {total}.\n\n\
         Patchback is built in the milestones of \
         [DESIGN.md § 13](https://github.com/beyond10x/patchback/blob/main/DESIGN.md#13-delivery-plan-and-decisions). \
         Each milestone is a set of stories, and each story is one change with its own acceptance \
         tests. This page is generated from the project's plan and its \
         [CHANGELOG](https://github.com/beyond10x/patchback/blob/main/CHANGELOG.md) whenever the \
         documentation is regenerated.\n\n\
         **Status:** `draft` is planned, `active` is being built, `implemented` is finished and on \
         `main`. **Release:** the newest release whose CHANGELOG entry names the story; \"by date\" \
         marks a story the CHANGELOG does not name, shown with the first release dated on or after \
         the day it was finished.\n",
        header = crate::FRONT_MATTER_HEADER,
        mdx = crate::reference::MDX_HEADER,
        total = counts(&all),
    );
    let mut ordered: Vec<&Epic> = epics.iter().collect();
    ordered.sort_by_key(|epic| (!epic.title.starts_with("Milestone "), &epic.title));
    for epic in ordered {
        let mut members: Vec<&Story> = stories
            .iter()
            .filter(|s| s.epic.as_deref() == Some(epic.slug.as_str()))
            .collect();
        section(&mut out, &epic.title, &mut members, releases);
    }
    let mut outside: Vec<&Story> = stories.iter().filter(|s| s.epic.is_none()).collect();
    if !outside.is_empty() {
        section(&mut out, "Outside the milestones", &mut outside, releases);
    }
    out
}

/// The progress page of the repository at `root`. A story that decomposes an epic the store
/// does not hold is refused.
pub fn output(root: &Path) -> Result<crate::Output, String> {
    let epics = read_epics(root)?;
    let stories = read_stories(root)?;
    for story in &stories {
        if let Some(epic) = &story.epic
            && !epics.iter().any(|e| &e.slug == epic)
        {
            return Err(format!(
                "the story {} decomposes the epic {epic}, which the planning store does not hold",
                story.slug
            ));
        }
    }
    let path = root.join(CHANGELOG);
    let changelog = fs::read_to_string(&path).map_err(crate::io(path.display()))?;
    let releases = read_releases(&changelog)?;
    Ok(crate::Output {
        dir: crate::DOCS,
        files: crate::Pages::from([(PAGE.to_owned(), page(&epics, &stories, &releases))]),
        exclusive: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n\
        ## [0.3.0] - 2026-10-25\n\n### Added\n\n- The fit (story\n  fit-core): it fits.\n\n\
        ## [0.2.0] - 2026-10-24\n\n### Added\n\n- A first fit (story fit-core) and the\n  analysis (story:analysis).\n\n\
        ## [0.1.1] - 2026-10-23\n\n### Fixed\n\n- Nothing named.\n\n\
        ## [0.1.0] - 2026-10-23\n\n### Added\n\n- The scaffold, unnamed.\n";

    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "patchback-docs-progress-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".engineering/planning/epic")).unwrap();
        fs::create_dir_all(dir.join(".engineering/planning/story")).unwrap();
        fs::write(dir.join("CHANGELOG.md"), CHANGELOG).unwrap();
        epic(&dir, "later", "'Milestone B: later work'");
        epic(&dir, "engine", "'Milestone A: the engine'");
        epic(&dir, "studio", "'Patch studio: a page'");
        story(
            &dir,
            "fit-core",
            "Core fit",
            "implemented",
            Some("engine"),
            Some("2026-10-24T22:51:31Z"),
        );
        story(
            &dir,
            "analysis",
            "'Analysis: FFT'",
            "implemented",
            Some("engine"),
            Some("2026-10-24T01:00:00Z"),
        );
        story(
            &dir,
            "scaffold",
            "Workspace scaffold",
            "implemented",
            Some("engine"),
            Some("2026-10-23T08:58:29Z"),
        );
        story(&dir, "phrases", "Phrase fit", "draft", Some("later"), None);
        story(
            &dir,
            "dataset",
            "Seeded dataset",
            "active",
            Some("later"),
            None,
        );
        story(&dir, "replay", "Fit replay", "draft", Some("studio"), None);
        story(&dir, "site", "Progress on the site", "active", None, None);
        dir
    }

    fn epic(root: &Path, slug: &str, title: &str) {
        fs::write(
            root.join(format!(".engineering/planning/epic/{slug}.md")),
            format!("---\nformat: aep.planning-md/3\nid: epic:{slug}\nkind: epic\nstatus: draft\ntitle: {title}\nrelations:\n- serves: vision:x\nrevision: 1\n---\n\n# Epic\n"),
        )
        .unwrap();
    }

    fn story(
        root: &Path,
        slug: &str,
        title: &str,
        status: &str,
        epic: Option<&str>,
        implemented: Option<&str>,
    ) {
        let decomposes = epic.map_or(String::new(), |e| format!("- decomposes: epic:{e}\n"));
        let transitions = implemented.map_or(String::new(), |at| {
            format!(
                "transitions:\n- {{from: \"draft\", to: \"active\", at: \"2026-10-20T00:00:00Z\", actor: \"human:timo\", revision: 2}}\n\
                 - {{from: \"active\", to: \"implemented\", at: \"{at}\", actor: \"human:timo\", revision: 3}}\n"
            )
        });
        fs::write(
            root.join(format!(".engineering/planning/story/{slug}.md")),
            format!(
                "---\nformat: aep.planning-md/3\nid: story:{slug}\nkind: story\nstatus: {status}\ntitle: {title}\nrelations:\n{decomposes}- serves: vision:x\nscope:\n- confidence: cited\n  path: x\nrevision: 3\n{transitions}---\n\n# Body\n\nstatus: rejected\ntitle: Not the title\n"
            ),
        )
        .unwrap();
    }

    fn render(root: &Path) -> String {
        let releases =
            read_releases(&fs::read_to_string(root.join("CHANGELOG.md")).unwrap()).unwrap();
        page(
            &read_epics(root).unwrap(),
            &read_stories(root).unwrap(),
            &releases,
        )
    }

    /// The table row of the story titled `title`.
    fn row<'a>(page: &'a str, title: &str) -> &'a str {
        page.lines()
            .find(|line| line.starts_with(&format!("| {title} |")))
            .unwrap_or_else(|| panic!("no row for {title} in\n{page}"))
    }

    #[test]
    fn lists_every_milestone_with_its_stories_status_and_counts() {
        let root = repo("milestones");
        let page = render(&root);
        let at = |heading: &str| {
            page.find(&format!("\n## {heading}\n"))
                .unwrap_or_else(|| panic!("no heading {heading} in\n{page}"))
        };
        let a = at("Milestone A: the engine");
        let b = at("Milestone B: later work");
        let studio = at("Patch studio: a page");
        let outside = at("Outside the milestones");
        assert!(a < b && b < studio && studio < outside, "{page}");
        assert!(page[a..b].contains("3 stories (3 implemented)."), "{page}");
        assert!(
            page[b..studio].contains("2 stories (1 active, 1 draft)."),
            "{page}"
        );
        assert!(
            page[studio..outside].contains("1 story (1 draft)."),
            "{page}"
        );
        assert!(page[outside..].contains("1 story (1 active)."), "{page}");
        for (title, section) in [
            ("Core fit", &page[a..b]),
            ("Analysis: FFT", &page[a..b]),
            ("Workspace scaffold", &page[a..b]),
            ("Phrase fit", &page[b..studio]),
            ("Seeded dataset", &page[b..studio]),
            ("Fit replay", &page[studio..outside]),
            ("Progress on the site", &page[outside..]),
        ] {
            assert!(section.contains(&format!("| {title} |")), "{title}: {page}");
        }
        assert!(row(&page, "Phrase fit").contains("| draft |"));
        assert!(row(&page, "Seeded dataset").contains("| active | — |"));
        assert!(!page.contains("Not the title"));
        assert!(page.contains("custom_edit_url: null"));
        assert!(page.contains(crate::reference::MDX_HEADER));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_implemented_story_shows_the_newest_release_naming_it() {
        let root = repo("newest");
        let releases = read_releases(CHANGELOG).unwrap();
        assert_eq!(
            releases
                .iter()
                .map(|r| r.version.as_str())
                .collect::<Vec<_>>(),
            ["Unreleased", "0.3.0", "0.2.0", "0.1.1", "0.1.0"]
        );
        assert!(releases[1].names.contains("fit-core"), "{releases:?}");
        assert!(releases[2].names.contains("fit-core"));
        assert!(releases[2].names.contains("analysis"));
        let page = render(&root);
        assert!(
            row(&page, "Core fit").contains("| implemented | [0.3.0]("),
            "{page}"
        );
        assert!(
            row(&page, "Analysis: FFT").contains("| implemented | [0.2.0]("),
            "{page}"
        );
        assert!(!row(&page, "Analysis: FFT").contains("by date"));
        // A story not implemented shows no release, whatever the CHANGELOG says.
        story(
            &root,
            "fit-core",
            "Core fit",
            "active",
            Some("engine"),
            None,
        );
        let page = render(&root);
        assert!(row(&page, "Core fit").ends_with("| active | — |"), "{page}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_story_the_changelog_never_names_shows_the_first_release_dated_after_its_implemented_transition()
     {
        let root = repo("by-date");
        let page = render(&root);
        // implemented 2026-10-23: 0.1.0 and 0.1.1 are both dated that day; the first is 0.1.0.
        let scaffold = row(&page, "Workspace scaffold");
        assert!(scaffold.contains("[0.1.0]("), "{scaffold}");
        assert!(scaffold.contains("(by date)"), "{scaffold}");
        // implemented after the last release: no release yet.
        story(
            &root,
            "scaffold",
            "Workspace scaffold",
            "implemented",
            Some("engine"),
            Some("2026-10-26T00:00:00Z"),
        );
        let page = render(&root);
        assert!(
            row(&page, "Workspace scaffold").ends_with("| implemented | Unreleased |"),
            "{page}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_latest_release_is_the_first_dated_heading() {
        let root = repo("latest");
        let page = render(&root);
        assert!(
            page.contains("Latest release: [0.3.0](https://github.com/beyond10x/patchback/releases/tag/0.3.0), 2026-10-25."),
            "{page}"
        );
        let releases = read_releases(CHANGELOG).unwrap();
        assert_eq!(releases[0].date, None);
        assert_eq!(releases[1].date.as_deref(), Some("2026-10-25"));
        assert_eq!(releases[1].day, Some((2026, 10, 25)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn release_dates_compare_as_numbers_and_an_unreadable_date_is_refused() {
        assert_eq!(read_day("2026-9-5"), Some((2026, 9, 5)));
        assert!(read_day("2026-9-5") < read_day("2026-10-25"));
        for bad in [
            "2026-10",
            "2026-10-25-1",
            "2026-13-01",
            "2026-10-00",
            "2026-1o-25",
            "",
            "-10-25",
            "26-10-30",
            "02026-10-30",
            "2026-010-30",
            "2026-10-030",
            "2026-10-30x",
        ] {
            assert_eq!(read_day(bad), None, "{bad}");
        }
        assert_eq!(read_day("2026-09-05"), Some((2026, 9, 5)));
        for heading in ["## [0.2.0] - ", "## [0.2.0]", "## [0.2.0] - 26-10-30"] {
            let error =
                read_releases(&format!("## [Unreleased]\n\n{heading}\n\n- x\n")).unwrap_err();
            assert!(error.contains("0.2.0"), "{heading}: {error}");
        }
        let undated = read_releases("## [Unreleased]\n\n- pending\n").unwrap();
        assert_eq!((undated[0].date.as_deref(), undated[0].day), (None, None));
        let releases = read_releases("## [0.2.0] - 2026-10-30\n\n## [0.1.0] - 2026-9-5\n").unwrap();
        let story = Story {
            slug: "dated".to_owned(),
            title: "Dated".to_owned(),
            status: IMPLEMENTED.to_owned(),
            epic: None,
            implemented_at: Some("2026-10-25T10:00:00Z".to_owned()),
        };
        assert!(release_of(&story, &releases).starts_with("[0.2.0]("));
        let error = read_releases("## [Unreleased]\n\n## [0.1.0] - 25.10.2026\n").unwrap_err();
        assert!(error.contains("0.1.0"), "{error}");
        assert!(error.contains("25.10.2026"), "{error}");
        assert!(error.contains("CHANGELOG.md"), "{error}");
    }

    #[test]
    fn check_fails_when_a_story_status_changes_without_regeneration() {
        let root = repo("status-drift");
        let current = output(&root).unwrap();
        crate::write(&root, &current).unwrap();
        crate::apply(&root, &[output(&root).unwrap()], true).unwrap();
        story(
            &root,
            "phrases",
            "Phrase fit",
            "active",
            Some("later"),
            None,
        );
        let error = crate::apply(&root, &[output(&root).unwrap()], true).unwrap_err();
        assert!(error.contains("stale or edited progress.mdx"), "{error}");
        assert!(error.contains("task docs-generate"), "{error}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn check_fails_when_a_release_names_a_story_without_regeneration() {
        let root = repo("release-drift");
        crate::write(&root, &output(&root).unwrap()).unwrap();
        let changelog = CHANGELOG.replace(
            "## [Unreleased]\n\n",
            "## [Unreleased]\n\n## [0.4.0] - 2026-10-26\n\n### Added\n\n- More (stories\n  analysis and fit-core).\n\n",
        );
        fs::write(root.join("CHANGELOG.md"), changelog).unwrap();
        let error = crate::apply(&root, &[output(&root).unwrap()], true).unwrap_err();
        assert!(error.contains("stale or edited progress.mdx"), "{error}");
        let page = render(&root);
        assert!(row(&page, "Analysis: FFT").contains("[0.4.0]("), "{page}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_page_names_no_planning_id() {
        let root = repo("private");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for page in [
            render(&root),
            output(&repository).unwrap().files[PAGE].clone(),
        ] {
            assert!(page.contains("## Milestone A"), "{page}");
            for private in crate::PRIVATE {
                assert!(!page.contains(private), "{private} in\n{page}");
            }
        }
        let _ = fs::remove_dir_all(&root);
    }
}
