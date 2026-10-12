//! Find orphaned files and broken links, a blackglass plugin after the
//! community plugin of that name: a report in a note at the top of the
//! vault (opened when it's written, brought up to date when it runs
//! again) of the files no note links to, the links to files that don't
//! exist, the empty files (nothing but frontmatter, or no bytes) and the
//! empty folders (no files in them, hidden ones aside). Folders, tags and
//! file types can be left out. See `requirements/orphaned_requirements.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::settings::{Kind, Setting, Values};
use super::{Context, Effect, Manifest, Plugin, PluginCommand};
use crate::vault::{Vault, slash};

const ID: &str = "orphaned-files";

/// What a report has in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sections {
    pub orphans: bool,
    pub broken: bool,
    pub empty_files: bool,
    pub empty_folders: bool,
}

impl Sections {
    const ALL: Sections = Sections {
        orphans: true,
        broken: true,
        empty_files: true,
        empty_folders: true,
    };
    const NONE: Sections = Sections {
        orphans: false,
        broken: false,
        empty_files: false,
        empty_folders: false,
    };
}

/// The settings, from the plugin's `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The report's file, in the vault's top folder.
    pub output: String,
    /// Folders (and what's in them), tags and file types left out.
    pub folders: Vec<String>,
    pub tags: Vec<String>,
    pub types: Vec<String>,
    /// Files that link to these aren't orphans.
    pub linking: Vec<String>,
    /// By size (largest first) instead of by name.
    pub by_size: bool,
}

fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.trim().trim_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl Settings {
    fn schema() -> Vec<Setting> {
        let text = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Text, default)
        };
        vec![
            text(
                "output",
                "Report file",
                "The note the report is written to, in the vault's top folder",
                "Orphaned files and broken links.md",
            ),
            text(
                "ignore_folders",
                "Folders left out",
                "Folders (and their subfolders) not reported, by path: Templates, Archive/2025",
                "",
            ),
            text(
                "ignore_tags",
                "Tags left out",
                "Notes with any of these tags aren't reported: draft, template",
                "",
            ),
            text(
                "ignore_types",
                "File types left out",
                "Files with these extensions aren't reported: pdf, png",
                "",
            ),
            text(
                "ignore_linking",
                "Notes linking to",
                "Files that link to any of these notes aren't orphans (a hub or index note)",
                "",
            ),
            Setting::new(
                "",
                "sort",
                "Sort by",
                "The orphaned files by name, or by size (largest first)",
                Kind::Choice(vec!["name".into(), "size".into()]),
                "name",
            ),
        ]
    }

    pub fn load(vault: &Vault) -> Settings {
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        let values = Values::parse(&text);
        let schema = Settings::schema();
        let get = |key: &str| {
            values.value(
                schema
                    .iter()
                    .find(|s| s.key == key)
                    .expect("a declared setting"),
            )
        };
        let output = get("output").trim().trim_matches('/').to_string();
        Settings {
            output: if output.is_empty() {
                "Orphaned files and broken links.md".into()
            } else if output.ends_with(".md") {
                output
            } else {
                format!("{output}.md")
            },
            folders: list(&get("ignore_folders")),
            tags: list(&get("ignore_tags"))
                .into_iter()
                .map(|t| t.trim_start_matches('#').to_lowercase())
                .collect(),
            types: list(&get("ignore_types"))
                .into_iter()
                .map(|t| t.trim_start_matches('.').to_lowercase())
                .collect(),
            linking: list(&get("ignore_linking")),
            by_size: get("sort") == "size",
        }
    }
}

/// What was found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Files no note links to, relative to the vault.
    pub orphans: Vec<PathBuf>,
    /// (target as written, the note it's in).
    pub broken: Vec<(String, PathBuf)>,
    pub empty_files: Vec<PathBuf>,
    /// Folders without files (the outermost of nested ones), relative.
    pub empty_folders: Vec<PathBuf>,
}

/// Whether a relative path is hidden (a part starts with `.`).
fn hidden(rel: &Path) -> bool {
    rel.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

/// Searches `vault` (the report itself left out).
pub fn find(vault: &Vault, settings: &Settings) -> Found {
    let report = PathBuf::from(&settings.output);
    let in_folder = |rel: &Path| {
        let rel = slash(rel).to_lowercase();
        settings
            .folders
            .iter()
            .any(|f| rel.starts_with(&format!("{}/", f.to_lowercase())))
    };
    let tagged = |rel: &Path| {
        !settings.tags.is_empty()
            && vault.note(&vault.root.join(rel)).is_some_and(|n| {
                n.tags.iter().any(|t| {
                    let t = t.to_lowercase();
                    settings
                        .tags
                        .iter()
                        .any(|i| t == *i || t.starts_with(&format!("{i}/")))
                })
            })
    };
    let typed = |rel: &Path| {
        rel.extension()
            .is_some_and(|e| settings.types.contains(&e.to_string_lossy().to_lowercase()))
    };
    let left_out =
        |rel: &Path| *rel == report || hidden(rel) || in_folder(rel) || tagged(rel) || typed(rel);

    // Every link from every note but the report.
    let mut linked: HashSet<PathBuf> = HashSet::new();
    let mut found = Found::default();
    let mut linking_to: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
    for note in vault.notes.iter().filter(|n| n.rel != report) {
        let links = crate::backlinks::outgoing(&note.lines, &note.path, vault);
        let mut targets = Vec::new();
        for link in links {
            match link.path {
                Some(path) => {
                    let rel = path
                        .strip_prefix(&vault.root)
                        .unwrap_or(&path)
                        .to_path_buf();
                    if rel != note.rel {
                        linked.insert(rel.clone());
                    }
                    targets.push(rel);
                }
                None if !left_out(&note.rel) => found.broken.push((link.name, note.rel.clone())),
                None => {}
            }
        }
        linking_to.push((note.rel.clone(), targets));
    }
    // Files that link to a hub note aren't orphans.
    let hubs: Vec<&crate::vault::Note> = vault
        .notes
        .iter()
        .filter(|n| {
            settings.linking.iter().any(|l| {
                let l = l.strip_suffix(".md").unwrap_or(l);
                n.rel_name().eq_ignore_ascii_case(l) || n.name().eq_ignore_ascii_case(l)
            })
        })
        .collect();
    let links_to_hub = |rel: &Path| {
        linking_to
            .iter()
            .find(|(r, _)| r == rel)
            .is_some_and(|(_, targets)| hubs.iter().any(|h| targets.contains(&h.rel)))
    };
    let size = |rel: &Path| std::fs::metadata(vault.root.join(rel)).map_or(0, |m| m.len());
    found.orphans = vault
        .files
        .iter()
        .filter(|rel| !left_out(rel) && !linked.contains(*rel) && !links_to_hub(rel))
        .cloned()
        .collect();
    if settings.by_size {
        found
            .orphans
            .sort_by_key(|rel| std::cmp::Reverse(size(rel)));
    } else {
        found.orphans.sort_by_key(|rel| slash(rel).to_lowercase());
    }
    found
        .broken
        .sort_by_key(|(name, from)| (slash(from).to_lowercase(), name.to_lowercase()));
    // Empty files: notes with nothing but frontmatter and blank lines;
    // other files without a byte.
    found.empty_files = vault
        .files
        .iter()
        .filter(|rel| !left_out(rel))
        .filter(|rel| match vault.note(&vault.root.join(rel)) {
            Some(note) => {
                let body =
                    crate::vault::properties::frontmatter_end(&note.lines).map_or(0, |e| e + 1);
                note.lines[body.min(note.lines.len())..]
                    .iter()
                    .all(|l| l.trim().is_empty())
            }
            None => size(rel) == 0,
        })
        .cloned()
        .collect();
    found
        .empty_files
        .sort_by_key(|rel| slash(rel).to_lowercase());
    // Empty folders, the outermost of nested ones: a folder reports its
    // empty subfolders when it isn't empty itself.
    fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>, skip: &dyn Fn(&Path) -> bool) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        let mut empty = true;
        let mut empties = Vec::new();
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
                empty = false;
                continue;
            }
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if skip(&rel) {
                empty = false;
            } else if walk(&path, root, out, skip) {
                empties.push(rel);
            } else {
                empty = false;
            }
        }
        if !empty {
            out.extend(empties);
        }
        empty
    }
    let skip = |rel: &Path| {
        let rel = slash(rel).to_lowercase();
        settings
            .folders
            .iter()
            .any(|f| rel == f.to_lowercase() || rel.starts_with(&format!("{}/", f.to_lowercase())))
    };
    let mut folders = Vec::new();
    walk(&vault.root, &vault.root, &mut folders, &skip);
    folders.sort_by_key(|rel| slash(rel).to_lowercase());
    found.empty_folders = folders;
    found
}

/// A link to a file: a note without `.md`, any other file with its
/// extension.
fn link(rel: &Path) -> String {
    let rel = slash(rel);
    format!("[[{}]]", rel.strip_suffix(".md").unwrap_or(&rel))
}

/// The report's text.
pub fn report(found: &Found, sections: Sections, when: &str) -> String {
    let mut out = format!(
        "# Orphaned files and broken links\n\nFound {when} by **Find orphaned files and broken links**; run it again to bring this note up to date.\n"
    );
    let mut section = |title: &str, about: &str, items: Vec<String>| {
        out.push_str(&format!("\n## {title} ({})\n\n{about}\n\n", items.len()));
        if items.is_empty() {
            out.push_str("None.\n");
        }
        for item in items {
            out.push_str(&format!("- {item}\n"));
        }
    };
    if sections.orphans {
        section(
            "Orphaned files",
            "Files no note links to.",
            found.orphans.iter().map(|r| link(r)).collect(),
        );
    }
    if sections.broken {
        section(
            "Broken links",
            "Links to files that don't exist (a click on one makes the note).",
            found
                .broken
                .iter()
                .map(|(name, from)| format!("[[{name}]] in {}", link(from)))
                .collect(),
        );
    }
    if sections.empty_files {
        section(
            "Empty files",
            "Files with nothing in them (or nothing but properties).",
            found.empty_files.iter().map(|r| link(r)).collect(),
        );
    }
    if sections.empty_folders {
        section(
            "Empty folders",
            "Folders with no files in them.",
            found
                .empty_folders
                .iter()
                .map(|r| format!("`{}/`", slash(r)))
                .collect(),
        );
    }
    out
}

#[derive(Default)]
pub struct OrphanedFiles;

impl OrphanedFiles {
    pub fn new() -> Self {
        OrphanedFiles
    }
}

impl Plugin for OrphanedFiles {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Orphaned files",
            version: "0.1.0",
            author: "blackglass, after Vinzent03's Find orphaned files and broken links",
            description: "A report at the top of the vault: files no note links to, links to files \
                          that don't exist, empty files and empty folders.",
        }
    }

    fn settings(&self) -> Vec<Setting> {
        Settings::schema()
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("find", "Find orphaned files and broken links"),
            PluginCommand::new("find-orphans", "Find orphaned files"),
            PluginCommand::new("find-broken-links", "Find broken links"),
            PluginCommand::new("find-empty-files", "Find empty files"),
            PluginCommand::new("find-empty-folders", "Find empty folders"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        let sections = match id {
            "find" => Sections::ALL,
            "find-orphans" => Sections {
                orphans: true,
                ..Sections::NONE
            },
            "find-broken-links" => Sections {
                broken: true,
                ..Sections::NONE
            },
            "find-empty-files" => Sections {
                empty_files: true,
                ..Sections::NONE
            },
            "find-empty-folders" => Sections {
                empty_folders: true,
                ..Sections::NONE
            },
            _ => return Effect::None,
        };
        let vault = ctx.vault;
        let settings = Settings::load(vault);
        let found = find(vault, &settings);
        let when = chrono::Local::now()
            .format("on %Y-%m-%d at %H:%M")
            .to_string();
        let text = report(&found, sections, &when);
        let path = vault.root.join(&settings.output);
        // A report there already is replaced (as the vault has it).
        let write = match vault.note(&path) {
            Some(note) => Effect::EditNote {
                path: path.clone(),
                from: 0,
                to: note.lines.len(),
                lines: text.lines().map(String::from).collect(),
                expect: note.lines.clone(),
            },
            None if path.exists() => {
                return Effect::Message(format!(
                    "{} isn't a note the report can go in",
                    settings.output
                ));
            }
            None => Effect::WriteFile {
                path: path.clone(),
                text,
            },
        };
        let counts = format!(
            "{} orphaned, {} broken links, {} empty files, {} empty folders",
            found.orphans.len(),
            found.broken.len(),
            found.empty_files.len(),
            found.empty_folders.len()
        );
        Effect::Many(vec![write, Effect::Open { path }, Effect::Message(counts)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    fn settings() -> Settings {
        Settings {
            output: "Report.md".into(),
            folders: Vec::new(),
            tags: Vec::new(),
            types: Vec::new(),
            linking: Vec::new(),
            by_size: false,
        }
    }

    #[test]
    fn orphans_broken_links_and_empty_things() {
        let dir = scratch("orphans-find");
        write(
            &dir,
            &[
                (
                    "Hub.md",
                    "[[A]] [[Sub/B|bee]] ![[img.png]] [[Nope#Heading]] [[Hub]]",
                ),
                ("A.md", "#draft\n"),
                ("Sub/B.md", "[x](../A.md) [web](https://example.com) [[]]"),
                ("C.md", "[[Hub]]"),
                ("Big.md", "lots of words in here, more than the others"),
                ("img.png", "png"),
                ("zero.txt", ""),
                ("Report.md", "[[Ghost]] [[C]]"),
                ("Deep/Er/.keep", ""),
            ],
        );
        std::fs::create_dir_all(dir.join("Deep/Too")).unwrap();
        let vault = Vault::open(&dir).unwrap();
        let found = find(&vault, &settings());
        let rels = |v: &[PathBuf]| v.iter().map(|p| slash(p)).collect::<Vec<_>>();
        assert_eq!(
            rels(&found.orphans),
            ["Big.md", "C.md", "zero.txt"],
            "Hub's link to itself doesn't count, nor the report's to C"
        );
        assert_eq!(
            found.broken,
            [("Nope".to_string(), PathBuf::from("Hub.md"))]
        );
        assert_eq!(rels(&found.empty_files), ["zero.txt"], "A has a tag in it");
        assert_eq!(rels(&found.empty_folders), ["Deep"], "the outermost");
        // Left out by folder, tag, type; by size; files linking to a hub.
        let s = Settings {
            folders: vec!["Sub".into()],
            tags: vec!["draft".into()],
            types: vec!["txt".into()],
            linking: vec!["Hub".into()],
            by_size: true,
            ..settings()
        };
        let found = find(&vault, &s);
        assert_eq!(rels(&found.orphans), ["Big.md"]);
        let text = report(&found, Sections::ALL, "now");
        assert!(
            text.contains("## Orphaned files (1)\n\nFiles no note links to.\n\n- [[Big]]\n"),
            "{text}"
        );
        assert!(text.contains("- [[Nope]] in [[Hub]]"), "{text}");
        assert!(
            text.contains("## Empty folders (1)") && text.contains("- `Deep/`"),
            "{text}"
        );
        let only = report(
            &found,
            Sections {
                broken: true,
                ..Sections::NONE
            },
            "now",
        );
        assert!(
            only.contains("## Broken links") && !only.contains("## Orphaned"),
            "{only}"
        );
    }
}
