//! Mermaid, a blackglass plugin: ```` ```mermaid ```` blocks drawn as text
//! in the note (flowcharts as a tree of their links, sequence diagrams
//! with lifelines); other diagram types are shown as their source, with a
//! line saying so. See `requirements/mermaid_requirements.md`.

pub mod charts;
pub mod flowchart;
pub mod sequence;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use ratatui::style::{Color, Style};
use ratatui::text::Line;

use super::{Manifest, Plugin};

#[derive(Default)]
pub struct Mermaid {
    /// Drawn blocks by (source, width): drawing runs every frame.
    cache: RefCell<HashMap<(String, usize), Vec<Line<'static>>>>,
}

impl Mermaid {
    pub fn new() -> Self {
        Mermaid::default()
    }
}

/// The diagram in `source` as text lines.
pub fn draw(source: &[String], width: usize) -> Vec<String> {
    let kind = source
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .unwrap_or_default();
    let word = kind.split([' ', ';']).next().unwrap_or_default();
    let body: Vec<String> = source
        .iter()
        .skip_while(|l| l.trim().is_empty() || l.trim().starts_with("%%"))
        .cloned()
        .collect();
    match word {
        "graph" | "flowchart" => flowchart::render(&body),
        "sequenceDiagram" => sequence::render(&body, width),
        "pie" => charts::pie(&body, width),
        "gantt" => charts::gantt(&body, width),
        other => {
            let mut lines: Vec<String> = source.to_vec();
            lines.push(format!(
                "(a Mermaid {} is shown as its source: blackglass draws flowcharts, sequence diagrams, pie and Gantt charts)",
                if other.is_empty() { "block" } else { other }
            ));
            lines
        }
    }
}

impl Plugin for Mermaid {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "mermaid",
            name: "Mermaid",
            version: "0.1.0",
            author: "blackglass",
            description: "```mermaid blocks drawn as text: flowcharts (graph / flowchart) as a tree of \
                          their links, sequence diagrams with lifelines and arrows. Other diagram types \
                          are shown as their source.",
        }
    }

    fn on_vault_changed(&mut self, _vault: &crate::vault::Vault) {
        self.cache.borrow_mut().clear();
    }

    fn block_languages(&self) -> &[&'static str] {
        &["mermaid"]
    }

    fn render_block(
        &self,
        _lang: &str,
        source: &[String],
        _from: Option<&Path>,
        width: usize,
    ) -> Vec<Line<'static>> {
        let key = (source.join("\n"), width);
        if let Some(lines) = self.cache.borrow().get(&key) {
            return lines.clone();
        }
        let style = Style::default().fg(Color::Cyan);
        let lines: Vec<Line<'static>> = draw(source, width)
            .into_iter()
            .map(|l| Line::styled(l, style))
            .collect();
        self.cache.borrow_mut().insert(key, lines.clone());
        lines
    }
}
