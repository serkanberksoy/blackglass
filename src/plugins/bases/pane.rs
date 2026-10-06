//! The filter pane (BA-51): a `.base` page's filters as rows above its
//! results, edited with keys, the results following at every key (each
//! change is written to the file at once). Alt+F shows or hides it,
//! Alt+M makes it half, nearly all or one line of the note's side.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;
use yaml_rust2::{Yaml, YamlLoader};

use super::filters::{self, Cond, Conj, Node, Op};
use super::{Bases, base_at_cursor, edit, syntax};
use crate::plugins::{Context, Effect, Pane, PaneSize};

/// Columns for a row's property and operator.
const PROPERTY: usize = 20;
const OPERATOR: usize = 18;

/// The rows being edited, and what they were read from.
#[derive(Debug, Clone)]
struct Work {
    file: String,
    this_view: bool,
    view: usize,
    node: Node,
    /// The filters' YAML as last read or written: the file changed
    /// elsewhere when it isn't this any more.
    written: Option<Yaml>,
}

/// The pane's state.
#[derive(Debug, Clone)]
pub struct FilterPane {
    pub shown: bool,
    pub size: PaneSize,
    /// The view's own filters (else the base's: every view's).
    this_view: bool,
    /// The row chosen: an index into the rows (one more: the line to add
    /// one).
    row: usize,
    /// The cell: 0 the property, 1 the operator, 2 the value.
    cell: usize,
    /// The text typed into the cell, and what it was before.
    typing: Option<(String, String)>,
    work: Option<Work>,
}

impl Default for FilterPane {
    fn default() -> Self {
        FilterPane {
            shown: false,
            size: PaneSize::Half,
            this_view: false,
            row: 0,
            cell: 2,
            typing: None,
            work: None,
        }
    }
}

impl FilterPane {
    /// Shown, as for a new base: half, the base's filters, the focus on it.
    pub fn open(&mut self) {
        *self = FilterPane {
            shown: true,
            ..FilterPane::default()
        };
    }
}

/// Text cut or padded to `width` columns.
fn cell(text: &str, width: usize) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + c.to_string().width() > width.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(c);
    }
    format!("{out}{}", " ".repeat(width.saturating_sub(out.width())))
}

/// Drops conditions with no property yet (being made): they aren't
/// written.
fn written(node: &Node) -> Node {
    match node {
        Node::Group(conj, items) => Node::Group(
            *conj,
            items
                .iter()
                .filter(|n| !matches!(n, Node::Cond(Cond::Row(p, _, _)) if p.trim().is_empty()))
                .map(written)
                .collect(),
        ),
        other => other.clone(),
    }
}

impl Bases {
    /// The rows shown for the `.base` page that's open: the working copy,
    /// or (when there's none, or the file changed) read from its YAML.
    fn filter_rows(&self, ctx: &Context) -> Option<Work> {
        let found = base_at_cursor(ctx)?;
        let file = found.file.clone()?;
        let base = syntax::parse(&found.source).ok()?;
        let view = self.view_of(&base, &found.source);
        let doc = YamlLoader::load_from_str(&found.source)
            .ok()
            .and_then(|d| d.into_iter().next())
            .unwrap_or(Yaml::Null);
        let y = if self.pane.this_view {
            &doc["views"][view]["filters"]
        } else {
            &doc["filters"]
        };
        let now = match y {
            Yaml::BadValue | Yaml::Null => None,
            y => Some(y.clone()),
        };
        if let Some(w) = &self.pane.work
            && w.file == file
            && w.this_view == self.pane.this_view
            && w.view == view
            && w.written == now
        {
            return Some(w.clone());
        }
        Some(Work {
            file,
            this_view: self.pane.this_view,
            view,
            node: filters::read(now.as_ref()),
            written: now,
        })
    }

    /// The view's name, for the scope line.
    fn view_name(&self, ctx: &Context) -> String {
        base_at_cursor(ctx)
            .and_then(|f| {
                let base = syntax::parse(&f.source).ok()?;
                let v = self.view_of(&base, &f.source);
                Some(base.views[v].name.clone())
            })
            .unwrap_or_default()
    }

    pub(super) fn filter_pane(&self, ctx: &Context, height: u16) -> Option<Pane> {
        if !self.pane.shown {
            return None;
        }
        let work = self.filter_rows(ctx)?;
        let title = "Filters   Alt+M size · Alt+F hide".to_string();
        if self.pane.size == PaneSize::Minimized {
            let summary = match written(&work.node).summary() {
                s if s.is_empty() => "none".to_string(),
                s => s,
            };
            let line = Line::from(vec![
                Span::styled("filters: ", Style::new().add_modifier(Modifier::BOLD)),
                Span::raw(summary),
            ]);
            return Some(Pane {
                title,
                size: PaneSize::Minimized,
                lines: vec![line],
                cursor: None,
            });
        }
        let selected = Style::new()
            .bg(crate::ui::theme::paint(crate::ui::theme::BG_SELECTED))
            .add_modifier(Modifier::BOLD);
        let dim = Style::new().fg(crate::ui::theme::paint(crate::ui::theme::MUTED));
        let accent = Style::new()
            .fg(crate::ui::theme::paint(crate::ui::theme::ACCENT))
            .add_modifier(Modifier::BOLD);
        let scope = |on: bool, text: String| {
            if on {
                Span::styled(format!("[{text}]"), accent)
            } else {
                Span::styled(text, dim)
            }
        };
        let mut lines = vec![Line::from(vec![
            scope(!self.pane.this_view, "All views".into()),
            Span::styled("  ·  ", dim),
            scope(
                self.pane.this_view,
                format!("This view: {}", self.view_name(ctx)),
            ),
            Span::styled("   Tab switches", dim),
        ])];
        let rows = filters::flatten(&work.node);
        let row = self.pane.row.min(rows.len());
        let mut cursor = None;
        for (i, (path, depth)) in rows.iter().enumerate() {
            let indent = "  ".repeat(*depth);
            let here = i == row;
            let typed = self
                .pane
                .typing
                .as_ref()
                .filter(|_| here)
                .map(|(t, _)| t.clone());
            let mut spans = vec![Span::raw(indent.clone())];
            match work.node.at(path) {
                Node::Group(conj, items) => {
                    let text = format!("{}  ({})", conj.label(), items.len());
                    spans.push(Span::styled(text, if here { selected } else { accent }));
                }
                Node::Cond(c) => {
                    let (p, op, v) = match c {
                        Cond::Row(p, op, v) => (p.clone(), op.label().to_string(), v.clone()),
                        Cond::Formula(t) => ("formula".into(), String::new(), t.clone()),
                    };
                    let mut cells = [p, op, v];
                    if let Some(t) = &typed {
                        let at = if matches!(c, Cond::Formula(_)) {
                            2
                        } else {
                            self.pane.cell
                        };
                        cells[at] = t.clone();
                        let before =
                            indent.width() + [0, PROPERTY, PROPERTY + OPERATOR][at] + t.width();
                        cursor = Some((before as u16, i as u16 + 1));
                    }
                    for (k, (text, width)) in cells.iter().zip([PROPERTY, OPERATOR, 60]).enumerate()
                    {
                        let style = if here && k == self.pane.cell {
                            selected
                        } else {
                            Style::new()
                        };
                        spans.push(Span::styled(cell(text, width), style));
                    }
                }
            }
            lines.push(Line::from(spans));
        }
        let add = Span::styled(
            "+ add filter (a) · add group (g) · d delete · o operator · c all/any/none",
            if row == rows.len() { selected } else { dim },
        );
        lines.push(Line::from(add));
        // Scrolled so the chosen row shows.
        let room = usize::from(self.pane.size.rows(height).saturating_sub(2));
        let skip = (row + 2).saturating_sub(room);
        if skip > 0 {
            lines.drain(1..=skip.min(lines.len() - 2));
            cursor = cursor.map(|(x, y)| (x, y.saturating_sub(skip as u16)));
        }
        Some(Pane {
            title,
            size: self.pane.size,
            lines,
            cursor,
        })
    }

    /// Writes `work`'s rows to the file (results follow) and keeps them.
    fn write_rows(&mut self, mut work: Work, ctx: &Context) -> Effect {
        let yaml = filters::write(&written(&work.node));
        let change = if work.this_view {
            edit::Change::ViewFilters(yaml.clone())
        } else {
            edit::Change::BaseFilters(yaml.clone())
        };
        let effect = self.rewrite(ctx, &change);
        if !matches!(effect, Effect::Message(_)) {
            work.written = yaml;
        }
        self.pane.work = Some(work);
        effect
    }

    /// "Show or hide filters" (Alt+F).
    pub(super) fn toggle_filters(&mut self, ctx: &Context) -> Effect {
        if base_at_cursor(ctx).and_then(|f| f.file).is_none() {
            return Effect::Message("Bases: open a .base file first".into());
        }
        self.pane.shown = !self.pane.shown;
        self.pane.typing = None;
        Effect::FocusPane(self.pane.shown)
    }

    /// "Change filters size" (Alt+M): half, nearly all, one line.
    pub(super) fn filters_size(&mut self, ctx: &Context) -> Effect {
        if base_at_cursor(ctx).and_then(|f| f.file).is_none() {
            return Effect::Message("Bases: open a .base file first".into());
        }
        if self.pane.shown {
            self.pane.size = self.pane.size.next();
            Effect::Redraw
        } else {
            self.pane.shown = true;
            self.pane.size = PaneSize::Half;
            Effect::FocusPane(true)
        }
    }

    pub(super) fn filter_key(&mut self, key: KeyEvent, ctx: &Context) -> Effect {
        let Some(mut work) = self.filter_rows(ctx) else {
            return Effect::FocusPane(false);
        };
        let rows = filters::flatten(&work.node);
        let row = self.pane.row.min(rows.len());
        self.pane.row = row;
        let path = rows.get(row).map(|(p, _)| p.clone());
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // Typing into a cell: each key is written.
        if let Some((mut text, before)) = self.pane.typing.take() {
            let Some(path) = path else {
                return Effect::Redraw;
            };
            let done = match key.code {
                KeyCode::Char(c) if !ctrl => {
                    text.push(c);
                    false
                }
                KeyCode::Backspace => {
                    text.pop();
                    false
                }
                KeyCode::Esc => {
                    text = before.clone();
                    true
                }
                KeyCode::Enter | KeyCode::Tab => true,
                _ => {
                    self.pane.typing = Some((text, before));
                    return Effect::None;
                }
            };
            let property = self.pane.cell == 0;
            if let Node::Cond(c) = work.node.at_mut(&path) {
                match c {
                    Cond::Row(p, _, _) if property => *p = text.clone(),
                    Cond::Row(_, _, v) => *v = text.clone(),
                    Cond::Formula(t) => *t = text.clone(),
                }
            }
            let next_value = matches!(
                work.node.at(&path),
                Node::Cond(Cond::Row(_, op, v)) if op.has_value() && v.is_empty()
            );
            if !done {
                self.pane.typing = Some((text, before));
            } else if property && key.code == KeyCode::Enter && next_value {
                // The property typed: on to the value.
                self.pane.cell = 2;
                self.pane.typing = Some((String::new(), String::new()));
            }
            return self.write_rows(work, ctx);
        }
        let cond_at = |w: &Work| match path.as_ref().map(|p| w.node.at(p)) {
            Some(Node::Cond(_)) => path.clone(),
            _ => None,
        };
        match key.code {
            KeyCode::Esc => Effect::FocusPane(false),
            KeyCode::Up => {
                self.pane.row = row.saturating_sub(1);
                Effect::Redraw
            }
            KeyCode::Down => {
                self.pane.row = (row + 1).min(rows.len());
                Effect::Redraw
            }
            KeyCode::Left => {
                self.pane.cell = self.pane.cell.saturating_sub(1);
                Effect::Redraw
            }
            KeyCode::Right => {
                self.pane.cell = (self.pane.cell + 1).min(2);
                Effect::Redraw
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.pane.this_view = !self.pane.this_view;
                self.pane.row = 0;
                self.pane.work = None;
                Effect::Redraw
            }
            KeyCode::Char('o' | 'O') | KeyCode::Enter
                if cond_at(&work).is_some()
                    && (key.code != KeyCode::Enter || self.pane.cell == 1) =>
            {
                let p = cond_at(&work).expect("checked");
                if let Node::Cond(Cond::Row(_, op, _)) = work.node.at_mut(&p) {
                    *op = op.step(key.code != KeyCode::Char('O'));
                }
                self.write_rows(work, ctx)
            }
            KeyCode::Enter | KeyCode::Char('e') if cond_at(&work).is_some() => {
                let p = cond_at(&work).expect("checked");
                let now = match work.node.at(&p) {
                    Node::Cond(Cond::Row(prop, _, _)) if self.pane.cell == 0 => prop.clone(),
                    Node::Cond(Cond::Row(_, _, v)) => {
                        self.pane.cell = 2;
                        v.clone()
                    }
                    Node::Cond(Cond::Formula(t)) => t.clone(),
                    Node::Group(..) => unreachable!("a condition"),
                };
                self.pane.typing = Some((now.clone(), now));
                self.pane.work = Some(work);
                Effect::Redraw
            }
            KeyCode::Enter | KeyCode::Char('c') if row < rows.len() => {
                // A group's (or the row's group's) all / any / none.
                let mut p = path.clone().expect("a row");
                if matches!(work.node.at(&p), Node::Cond(_)) {
                    p.pop();
                }
                if let Node::Group(conj, _) = work.node.at_mut(&p) {
                    *conj = conj.next();
                }
                self.write_rows(work, ctx)
            }
            KeyCode::Char('a') | KeyCode::Char('g') | KeyCode::Enter => {
                let new = match key.code {
                    KeyCode::Char('g') => Node::Group(
                        Conj::Any,
                        vec![Node::Cond(Cond::Row(String::new(), Op::Is, String::new()))],
                    ),
                    _ => Node::Cond(Cond::Row(String::new(), Op::Is, String::new())),
                };
                let group = key.code == KeyCode::Char('g');
                // In the chosen group, or after the chosen condition.
                let (parent, at) = match &path {
                    Some(p) if matches!(work.node.at(p), Node::Group(..)) => {
                        let len = match work.node.at(p) {
                            Node::Group(_, items) => items.len(),
                            Node::Cond(_) => 0,
                        };
                        (p.clone(), len)
                    }
                    Some(p) => {
                        let mut parent = p.clone();
                        let i = parent.pop().expect("not the top");
                        (parent, i + 1)
                    }
                    None => {
                        let len = match &work.node {
                            Node::Group(_, items) => items.len(),
                            Node::Cond(_) => 0,
                        };
                        (Vec::new(), len)
                    }
                };
                if let Node::Group(_, items) = work.node.at_mut(&parent) {
                    items.insert(at, new);
                }
                let mut target = parent;
                target.push(at);
                if group {
                    target.push(0);
                }
                self.pane.row = filters::flatten(&work.node)
                    .iter()
                    .position(|(p, _)| *p == target)
                    .unwrap_or(0);
                self.pane.cell = 0;
                self.pane.typing = Some((String::new(), String::new()));
                self.pane.work = Some(work);
                Effect::Redraw
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                let Some(mut p) = path.filter(|p| !p.is_empty()) else {
                    return Effect::None;
                };
                let i = p.pop().expect("not the top");
                if let Node::Group(_, items) = work.node.at_mut(&p) {
                    items.remove(i);
                }
                self.write_rows(work, ctx)
            }
            _ => Effect::None,
        }
    }
}
