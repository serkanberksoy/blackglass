//! Tracker, a blackglass plugin: ```` ```tracker ```` blocks that gather
//! what notes track (tags with values, properties, inline fields, tasks,
//! text, tables, file data), a value a day by the dates in the notes'
//! names, and show it as a summary, a line or bar chart, a month view, a
//! pie or a bullet chart, the original's parameters as written. See
//! `requirements/tracker_requirements.md`.

pub mod chart;
pub mod collect;
pub mod expr;
pub mod month;
pub mod params;

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::rc::Weak;

use mdedit::processor::CellRow;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::{Context, Effect, Manifest, Plugin};
use crate::vault::Vault;
use params::{Kind, Params};

#[derive(Default)]
pub struct Tracker {
    vault: Weak<Vault>,
    /// Drawn blocks by (source, width): drawing runs every frame.
    cache: RefCell<HashMap<(String, usize), Vec<CellRow>>>,
    /// What the user changed in each block's month view, by its key.
    months: HashMap<u64, month::State>,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker::default()
    }
}

/// A block's key in its actions: its source's hash.
fn block_key(source: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

/// A message in place of the block (what the original says).
fn error(text: &str) -> Vec<CellRow> {
    vec![(
        Line::styled(format!("Tracker: {text}"), Style::default().fg(Color::Red)),
        Vec::new(),
    )]
}

/// `margin` (pixels in the original: top, right, bottom, left; one or two
/// values for all or for top / bottom and left / right) as rows above and
/// below and columns on the left.
fn margin(root: &params::Opts) -> (usize, usize, usize) {
    let px = root.nums("margin");
    let (top, left, bottom) = match px.as_slice() {
        [all] => (*all, *all, *all),
        [vertical, horizontal] => (*vertical, *horizontal, *vertical),
        [top, _, bottom, left, ..] => (*top, *left, *bottom),
        [top, horizontal, bottom] => (*top, *horizontal, *bottom),
        _ => (0.0, 0.0, 0.0),
    };
    let rows = |v: f64| (v.max(0.0) / 20.0).round() as usize;
    let cols = |v: f64| (v.max(0.0) / 10.0).round() as usize;
    (rows(top), cols(left), rows(bottom))
}

/// A block's rows: each output in turn, a blank row between them.
pub fn render(source: &str, vault: &Vault, width: usize, today: chrono::NaiveDate) -> Vec<CellRow> {
    render_with(source, vault, width, today, &month::State::default())
}

/// [`render`] with what the user changed in its month view.
fn render_with(
    source: &str,
    vault: &Vault,
    width: usize,
    today: chrono::NaiveDate,
    state: &month::State,
) -> Vec<CellRow> {
    let params = match Params::parse(source) {
        Ok(p) => p,
        Err(e) => return error(&e),
    };
    let data = match collect::collect(vault, &params, today) {
        Ok(d) => d,
        Err(e) => return error(&e),
    };
    let (top, left, bottom) = margin(&params.root);
    let width = width.saturating_sub(left).max(8);
    let key = block_key(source);
    let mut out: Vec<CellRow> = Vec::new();
    for (k, output) in params.outputs.iter().enumerate() {
        if k > 0 {
            out.push((Line::default(), Vec::new()));
        }
        let opts = &output.opts;
        let rows = match output.kind {
            Kind::Summary => {
                let template = opts.text("template").unwrap_or_default();
                let style = chart::css(opts.str("style").unwrap_or_default());
                match expr::fill(&template.replace("\\n", "\n"), &data) {
                    Ok(text) => chart::plain(
                        text.lines()
                            .map(|l| Line::styled(l.to_string(), style))
                            .collect(),
                    ),
                    Err(e) => error(&e),
                }
            }
            Kind::Line => chart::line(&data, opts, &params.root, width),
            Kind::Bar => chart::bar(&data, opts, &params.root, width),
            Kind::Pie => chart::pie(&data, opts, width).unwrap_or_else(|e| error(&e)),
            Kind::Bullet => chart::bullet(&data, opts, width).unwrap_or_else(|e| error(&e)),
            Kind::Month => month::month(&data, opts, today, state, key),
        };
        out.extend(std::iter::repeat_n((Line::default(), Vec::new()), top));
        out.extend(rows.into_iter().map(|(line, parts)| {
            if left == 0 {
                return (line, parts);
            }
            let mut spans = vec![Span::raw(" ".repeat(left))];
            spans.extend(line.spans);
            let parts = parts
                .into_iter()
                .map(|(a, b, act)| (a + left, b.saturating_add(left), act))
                .collect();
            (Line::from(spans).style(line.style), parts)
        }));
        out.extend(std::iter::repeat_n((Line::default(), Vec::new()), bottom));
    }
    out
}

impl Plugin for Tracker {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "tracker",
            name: "Tracker",
            version: "0.1.0",
            author: "blackglass",
            description: "```tracker blocks: what your daily notes track (tags with values, properties, \
                          inline fields, tasks, text, tables) as a summary, line or bar chart, month \
                          view, pie or bullet chart.",
        }
    }

    fn set_vault(&mut self, vault: Weak<Vault>) {
        self.vault = vault;
        self.cache.borrow_mut().clear();
    }

    fn on_vault_changed(&mut self, _vault: &Vault) {
        self.cache.borrow_mut().clear();
    }

    fn on_theme_changed(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn on_unload(&mut self) {
        self.cache.borrow_mut().clear();
    }

    fn block_languages(&self) -> &[&'static str] {
        &["tracker"]
    }

    /// A chart's column says its values; the month view's buttons change
    /// its month, dataset and selected day (a second click opens it).
    fn row_action(&mut self, payload: &str, ctx: &Context) -> Effect {
        if let Some(say) = payload.strip_prefix("say:") {
            return Effect::Message(say.to_string());
        }
        let Some(rest) = payload.strip_prefix("m:") else {
            return Effect::None;
        };
        let Some((key, what)) = rest.split_once(':') else {
            return Effect::None;
        };
        let Ok(key) = key.parse::<u64>() else {
            return Effect::None;
        };
        let state = self.months.entry(key).or_default();
        match what {
            "prev" => state.offset -= 1,
            "next" => state.offset += 1,
            "reset" => state.offset = 0,
            "dataset" => state.dataset += 1,
            _ => {
                let Some((day, rel)) = what.strip_prefix("day:").and_then(|d| d.split_once(':'))
                else {
                    return Effect::None;
                };
                let Ok(day) = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d") else {
                    return Effect::None;
                };
                if state.selected == Some(day) && !rel.is_empty() {
                    return Effect::Open {
                        path: ctx.vault.root.join(rel),
                    };
                }
                state.selected = Some(day);
            }
        }
        self.cache.borrow_mut().clear();
        Effect::Redraw
    }

    fn render_block(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<Line<'static>> {
        self.render_block_cells(lang, source, from, width)
            .into_iter()
            .map(|(l, _)| l)
            .collect()
    }

    fn render_block_rows(
        &self,
        lang: &str,
        source: &[String],
        from: Option<&Path>,
        width: usize,
    ) -> Vec<(Line<'static>, Option<String>)> {
        self.render_block_cells(lang, source, from, width)
            .into_iter()
            .map(|(l, _)| (l, None))
            .collect()
    }

    fn render_block_cells(
        &self,
        _lang: &str,
        source: &[String],
        _from: Option<&Path>,
        width: usize,
    ) -> Vec<CellRow> {
        let key = (source.join("\n"), width);
        if let Some(rows) = self.cache.borrow().get(&key) {
            return rows.clone();
        }
        let Some(vault) = self.vault.upgrade() else {
            return error("the vault isn't open");
        };
        let today = chrono::Local::now().date_naive();
        let state = self
            .months
            .get(&block_key(&key.0))
            .cloned()
            .unwrap_or_default();
        let rows = render_with(&key.0, &vault, width, today, &state);
        let mut cache = self.cache.borrow_mut();
        // Every edit of a block is a new source: keep the cache small.
        if cache.len() > 64 {
            cache.clear();
        }
        cache.insert(key, rows.clone());
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};
    use ratatui::style::Modifier;

    #[test]
    fn margins_and_a_styled_summary() {
        let dir = scratch("tracker-margin");
        write(
            &dir,
            &[("2026-10-01.md", "#a:2"), ("2026-10-02.md", "#a:3")],
        );
        let vault = Vault::open(&dir).unwrap();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        let rows = render(
            "searchType: tag\nsearchTarget: a\nmargin: 20, 10, 20, 30\n\
             summary:\n  template: 'Sum {{sum()}}\\nOld {{sum(0)}}'\n  style: 'color: red; font-weight: bold'\n",
            &vault,
            40,
            today,
        );
        let text: Vec<String> = rows
            .iter()
            .map(|(l, _)| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(
            text,
            ["", "   Sum 5", "   Old 5", ""],
            "a row above and below, three columns on the left"
        );
        let style = rows[1].0.style;
        assert_eq!(style.fg, Some(Color::Red));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
