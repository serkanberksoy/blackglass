//! The example vault's examples keep working: every `dataview` and
//! `dataviewjs` block and every inline query (`` `= …` ``, `` `$= …` ``)
//! renders without an error, and every template renders (with the default
//! answers to its questions).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use blackglass::plugins::templater::engine::{Env, More, render};
use blackglass::plugins::templater::new_note_target;
use blackglass::plugins::{Answer, Blocks, Plugins, Question};
use blackglass::vault::Vault;
use mdedit::processor::CodeBlockProcessor;

fn example_vault() -> Vault {
    Vault::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("example_vault"))
        .expect("the example vault")
}

/// The fenced blocks in `lines` whose language is `dataview` or
/// `dataviewjs`: (language, source).
fn blocks(lines: &[String]) -> Vec<(String, Vec<String>)> {
    let mut found = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in lines {
        match &mut open {
            Some((lang, body)) if line.trim_start().starts_with("```") => {
                found.push((lang.clone(), std::mem::take(body)));
                open = None;
            }
            Some((_, body)) => body.push(line.clone()),
            None => {
                if let Some(lang) = line.trim_start().strip_prefix("```")
                    && lang.starts_with("dataview")
                {
                    open = Some((lang.trim().to_string(), Vec::new()));
                }
            }
        }
    }
    found
}

#[test]
fn every_dataview_block_in_the_example_vault_renders() {
    let vault = example_vault();
    let blocks_processor = Blocks(Rc::new(RefCell::new(Plugins::for_vault(&vault))));
    let mut checked = 0;
    for note in vault
        .notes
        .iter()
        .filter(|n| !n.rel_name().starts_with("Templates"))
    {
        for (lang, source) in blocks(&note.lines) {
            assert!(
                blocks_processor.handles(&lang),
                "{lang} in {}",
                note.rel_name()
            );
            let lines = blocks_processor.render(&lang, &source, Some(&note.path), 80);
            let text: Vec<String> = lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
                .collect();
            assert!(
                !text.iter().any(|l| l.starts_with("Dataview:")),
                "{} ({lang}): {text:#?}\n{}",
                note.rel_name(),
                source.join("\n")
            );
            checked += 1;
        }
    }
    assert!(checked >= 15, "only {checked} blocks");
}

/// The inline queries in `lines`, outside code blocks: (opening, code).
fn inline_queries(lines: &[String]) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    let mut fence = false;
    for line in lines {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        for (i, part) in line.split('`').enumerate() {
            // Odd parts are inside backticks.
            if i % 2 == 0 {
                continue;
            }
            if let Some(code) = part.strip_prefix("$=") {
                found.push(("`$=", code.to_string()));
            } else if let Some(code) = part.strip_prefix('=').filter(|c| !c.starts_with('=')) {
                found.push(("`=", code.to_string()));
            }
        }
    }
    found
}

#[test]
fn every_inline_query_in_the_example_vault_renders() {
    let vault = example_vault();
    let mut plugins = Plugins::for_vault(&vault);
    let mut checked = 0;
    for note in vault
        .notes
        .iter()
        .filter(|n| !n.rel_name().starts_with("Templates"))
    {
        plugins.note_opened(&note.path, &vault);
        for (open, code) in inline_queries(&note.lines) {
            let shown = plugins
                .render_span(open, &code)
                .unwrap_or_else(|| panic!("{}: {open}{code}` shows nothing", note.rel_name()));
            assert!(
                !shown.starts_with('⚠'),
                "{}: {open}{code}` → {shown}",
                note.rel_name()
            );
            checked += 1;
        }
    }
    assert!(checked >= 12, "only {checked} inline queries");
}

#[test]
fn every_template_in_the_example_vault_renders() {
    let vault = example_vault();
    let now = chrono::NaiveDate::from_ymd_opt(2026, 8, 9)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    let mut checked = 0;
    for note in vault
        .notes
        .iter()
        .filter(|n| n.rel_name().starts_with("Templates/"))
    {
        let target = new_note_target(&vault, &vault.root.join("Journal"), "2026-08-09");
        let template = note.lines.join("\n");
        // Answer every question with its default (or the first choice).
        let mut answers = Vec::new();
        // As Templater runs it: the vault's settings (its scripts), the
        // template's file (nothing fetched from the web).
        let more = More {
            template_file: Some(note.rel_name().replace('\\', "/") + ".md"),
            setup: blackglass::plugins::templater::setup(&vault).0,
            ..More::default()
        };
        let output = loop {
            let env = Env {
                vault: &vault,
                target: &target,
                now,
                answers: &answers,
                more: &more,
            };
            let output =
                render(&template, &env).unwrap_or_else(|e| panic!("{}: {e}", note.rel_name()));
            if output.questions.len() <= answers.len() {
                break output;
            }
            answers.extend(output.questions[answers.len()..].iter().map(|q| match q {
                Question::Text { default, .. } | Question::Lines { default, .. } => {
                    Answer::Text(default.clone())
                }
                Question::Choose { .. } => Answer::Choice(0),
                Question::Many { .. } => Answer::Choices(Vec::new()),
                Question::Form { .. } => Answer::Fields(Vec::new()),
            }));
        };
        assert!(
            !output.text.contains("<%"),
            "{}: {}",
            note.rel_name(),
            output.text
        );
        checked += 1;
    }
    assert!(checked >= 4, "only {checked} templates");
}
