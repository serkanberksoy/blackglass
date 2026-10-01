//! Project consistency checks: the version in `Cargo.toml` must match the
//! latest `VERSION.md` entry and the README; every done feature must name
//! a test that exists; `--help` must list the README's keys; the product
//! never names another note app.

use std::fs;
use std::path::Path;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn read(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {file}: {e}"))
}

/// Version entries in VERSION.md: `## X.Y.Z (date)` headings, newest first.
fn version_headings(history: &str) -> Vec<&str> {
    history
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .filter(|h| h.starts_with(|c: char| c.is_ascii_digit()))
        .map(|h| h.split_whitespace().next().unwrap_or_default())
        .collect()
}

#[test]
fn version_md_latest_entry_matches_cargo_version() {
    let history = read("VERSION.md");
    let latest = *version_headings(&history)
        .first()
        .expect("VERSION.md has no `## X.Y.Z` entry");
    assert_eq!(
        latest, VERSION,
        "the newest VERSION.md entry must be the Cargo.toml version"
    );
    let expected = format!("Current version: **{VERSION}**");
    assert!(
        history.contains(&expected),
        "VERSION.md must say `{expected}`"
    );
}

#[test]
fn readme_shows_cargo_version() {
    let expected = format!("**Version:** {VERSION}");
    assert!(
        read("README.md").contains(&expected),
        "README.md must say `{expected}`"
    );
}

#[test]
fn version_entries_are_newest_first_and_unique() {
    let history = read("VERSION.md");
    let versions: Vec<Vec<u64>> = version_headings(&history)
        .into_iter()
        .map(|v| {
            v.split('.')
                .map(|n| n.parse().expect("version parts are numbers"))
                .collect()
        })
        .collect();
    assert!(
        versions.windows(2).all(|w| w[0] > w[1]),
        "VERSION.md entries must be strictly newest first: {versions:?}"
    );
}

/// The text of every `.rs` file under `dir`.
fn sources(dir: &Path, out: &mut String) {
    for entry in fs::read_dir(dir).expect("readable source folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push_str(&fs::read_to_string(&path).expect("readable source"));
        }
    }
}

#[test]
fn every_done_feature_names_a_test_that_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut code = String::new();
    sources(&root.join("src"), &mut code);
    sources(&root.join("tests"), &mut code);
    let features = read("requirements/features.md");
    let mut checked = 0;
    for row in features.lines().filter(|l| l.starts_with("| W-")) {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        let (id, now, test) = (cells[1], cells[3], cells[5]);
        if !matches!(now, "✅" | "🟡") {
            continue;
        }
        let name = test
            .trim_matches('`')
            .rsplit("::")
            .next()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| panic!("{id} is {now} but names no test"));
        assert!(
            code.contains(&format!("fn {name}(")),
            "{id}: test {name} doesn't exist"
        );
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} rows checked");
}

/// `blackglass --help`, from the real binary.
fn help() -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blackglass"))
        .arg("--help")
        .output()
        .expect("blackglass --help runs");
    assert!(out.status.success());
    String::from_utf8(out.stdout).expect("UTF-8 help")
}

#[test]
fn help_lists_every_key_in_the_readme() {
    let readme = read("README.md");
    let start = readme
        .find("| Key | Action |")
        .expect("README has a key table");
    let table = &readme[start..];
    let table = &table[..table.find("\n\n").unwrap_or(table.len())];
    let help = help();
    let mut keys = 0;
    for row in table.lines().skip(2) {
        let cell = row.split('|').nth(1).expect("a key cell").trim();
        for key in cell.split(" / ").flat_map(|k| k.split(", ")) {
            let key = key.trim().trim_matches('`');
            assert!(help.contains(key), "key {key:?} missing from --help");
            keys += 1;
        }
    }
    assert!(keys > 15, "found {keys} keys");
}

#[test]
fn version_flag_prints_the_version() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_blackglass"))
        .arg("--version")
        .output()
        .expect("blackglass --version runs");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("blackglass {VERSION}")
    );
}

/// Text a user of blackglass sees: `--help`, the README, the built-in help,
/// the example vault, the package description, and the strings in the code
/// (outside comments and tests).
fn product_texts() -> Vec<(String, String)> {
    fn walk(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).expect("a folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, ext, out);
            } else if path.extension().is_some_and(|e| e == ext) {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut texts = vec![
        ("--help".to_string(), help()),
        ("README.md".to_string(), read("README.md")),
        ("src/help.md".to_string(), read("src/help.md")),
        (
            "Cargo.toml description".to_string(),
            env!("CARGO_PKG_DESCRIPTION").to_string(),
        ),
    ];
    let mut files = Vec::new();
    walk(&root.join("example_vault"), "md", &mut files);
    let mut sources = Vec::new();
    walk(&root.join("src"), "rs", &mut sources);
    for path in files {
        let text = fs::read_to_string(&path).expect("a note");
        texts.push((path.display().to_string(), text));
    }
    for path in sources {
        let text = fs::read_to_string(&path).expect("a source file");
        let code = text.split("#[cfg(test)]").next().unwrap_or_default();
        let strings: String = code
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        texts.push((path.display().to_string(), strings));
    }
    texts
}

#[test]
fn the_product_never_names_another_note_app() {
    for (name, text) in product_texts() {
        // Names, not mentions: the `.obsidian/` folder marks a vault, and
        // `tp.obsidian` is a Templater module a template may use.
        let text = text
            .replace("\".obsidian\"", "")
            .replace("\"obsidian\"", "");
        assert!(
            !text.to_lowercase().contains("obsidian"),
            "{name} mentions Obsidian"
        );
    }
}
