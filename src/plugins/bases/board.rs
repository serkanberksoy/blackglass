//! The kanban board's columns (BA-46, BA-47), as Obsidian 1.14 makes them:
//! one per value of the view's `groupBy` property, notes without a value
//! in "None"; `groupOrder` (when the view has it) is the columns shown, in
//! order (a group in it that no note has is an empty column; `null` is
//! "None"; a group left out is hidden). blackglass's `groupColors` give
//! columns a color (Obsidian ignores the key).

use ratatui::style::Color;

use super::expr::Value;
use super::syntax::View;
use super::views::Results;
use crate::plugins::dataview::index::Page;

/// The column of notes without a value.
pub const NONE: &str = "None";

/// A column of the board.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Its group (`None`: notes without a value).
    pub group: Option<String>,
    /// The result rows in it, in the view's order.
    pub cards: Vec<usize>,
    pub color: Option<Color>,
}

impl Column {
    /// Its heading.
    pub fn label(&self) -> &str {
        self.group.as_deref().unwrap_or(NONE)
    }
}

/// A color name (the highlight colors' names, and a few more) as a named
/// terminal color, so themes change it.
pub fn color(name: &str) -> Option<Color> {
    Some(match name.trim().to_lowercase().as_str() {
        "red" => Color::Red,
        "orange" => Color::LightRed,
        "yellow" => Color::Yellow,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "purple" | "violet" => Color::Magenta,
        "pink" => Color::LightMagenta,
        "cyan" | "teal" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        _ => return None,
    })
}

/// A group's name: its value as shown (`None` for no value).
pub fn group_of(value: Option<&Value>, pages: &[Page]) -> Option<String> {
    value.map(|v| v.display(pages)).filter(|s| !s.is_empty())
}

/// The board's columns for the view's results.
pub fn columns(view: &View, results: &Results, pages: &[Page]) -> Vec<Column> {
    // The groups in the results' order (they're sorted by group).
    let mut found: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    for (r, g) in results.groups.iter().enumerate() {
        let group = group_of(g.as_ref(), pages);
        match found.iter_mut().find(|(k, _)| *k == group) {
            Some((_, rows)) => rows.push(r),
            None => found.push((group, vec![r])),
        }
    }
    // Notes without a value last, as Obsidian does.
    if let Some(i) = found.iter().position(|(k, _)| k.is_none()) {
        let none = found.remove(i);
        found.push(none);
    }
    let mut out: Vec<(Option<String>, Vec<usize>)> = match &view.group_order {
        Some(order) => order
            .iter()
            .map(|g| {
                let rows = found
                    .iter()
                    .find(|(k, _)| k == g)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_default();
                (g.clone(), rows)
            })
            .collect(),
        None => found,
    };
    if view.option("hideEmptyColumns") == Some("true") {
        out.retain(|(_, rows)| !rows.is_empty());
    }
    out.into_iter()
        .map(|(group, cards)| {
            let color = view
                .group_colors
                .iter()
                .find(|(g, _)| Some(g.as_str()) == group.as_deref())
                .and_then(|(_, c)| color(c));
            Column {
                group,
                cards,
                color,
            }
        })
        .collect()
}

/// The groups in order, as `groupOrder` writes them (every column shown).
pub fn order(columns: &[Column]) -> Vec<Option<String>> {
    columns.iter().map(|c| c.group.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::bases::syntax;

    fn results(groups: &[Option<&str>]) -> Results {
        Results {
            rows: (0..groups.len()).collect(),
            columns: vec![("file.name".into(), "name".into())],
            cells: (0..groups.len())
                .map(|i| vec![Value::Text(format!("n{i}"))])
                .collect(),
            groups: groups
                .iter()
                .map(|g| Some(g.map_or(Value::Null, |t| Value::Text(t.into()))))
                .collect(),
        }
    }

    fn view(yaml: &str) -> View {
        syntax::parse(&format!(
            "views:\n  - type: kanban\n    groupBy: status\n{yaml}"
        ))
        .unwrap()
        .views
        .remove(0)
    }

    fn shown(cols: &[Column]) -> Vec<(String, usize)> {
        cols.iter()
            .map(|c| (c.label().to_string(), c.cards.len()))
            .collect()
    }

    #[test]
    fn columns_follow_the_group_order() {
        let r = results(&[None, Some("Doing"), Some("Doing"), Some("Done")]);
        let auto = columns(&view(""), &r, &[]);
        assert_eq!(
            shown(&auto),
            [("Doing".into(), 2), ("Done".into(), 1), ("None".into(), 1)],
            "notes without a value last"
        );
        let ordered = columns(
            &view("    groupOrder:\n      - Planned\n      - Done\n      - null\n"),
            &r,
            &[],
        );
        assert_eq!(
            shown(&ordered),
            [
                ("Planned".into(), 0),
                ("Done".into(), 1),
                ("None".into(), 1)
            ],
            "an added group is empty; one left out is hidden"
        );
        assert_eq!(
            order(&ordered),
            [Some("Planned".into()), Some("Done".into()), None]
        );
        let hidden = columns(
            &view("    hideEmptyColumns: true\n    groupOrder: [Planned, Done]\n"),
            &r,
            &[],
        );
        assert_eq!(shown(&hidden), [("Done".into(), 1)]);
        let colored = columns(&view("    groupColors:\n      Done: green\n"), &r, &[]);
        assert_eq!(colored[1].color, Some(Color::Green));
        assert_eq!(colored[0].color, None);
        assert_eq!(color("Orange"), Some(Color::LightRed));
        assert_eq!(color("mauve"), None);
    }
}
