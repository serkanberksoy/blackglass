//! A task checked from a query's results stays where it was, shown with
//! its new state, for a few seconds (a setting, [`DEFAULT`]) before the results are
//! drawn anew (where a `not done` query leaves it out): time to see the
//! click took, and to click again to undo it. Tasks' and Dataview's
//! blocks keep their drawn rows here while that lasts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use mdedit::processor::CellRow;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// How long checked results stay as they are, unless set.
pub const DEFAULT: &str = "3.5";

/// The longest a setting may keep them.
const MOST: f64 = 10.0;

/// A setting's seconds, held to 0 … 10 (not a number: the default's).
pub fn seconds(text: &str) -> Duration {
    let n = text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .unwrap_or_else(|| DEFAULT.parse().expect("the default is a number"));
    Duration::from_secs_f64(n.clamp(0.0, MOST))
}

/// Blocks' rows held as drawn, by cache key, until a moment.
pub struct Frozen<K> {
    held: RefCell<HashMap<K, (Vec<CellRow>, Instant)>>,
}

impl<K> Default for Frozen<K> {
    fn default() -> Self {
        Frozen {
            held: RefCell::new(HashMap::new()),
        }
    }
}

/// A task state's glyph (`x` ☑, ` ` ☐, `-` ☒, `/` ◐, others `[c]`).
fn glyph(status: char) -> String {
    match status {
        ' ' => "☐".into(),
        'x' | 'X' => "☑".into(),
        '-' => "☒".into(),
        '/' => "◐".into(),
        c => format!("[{c}]"),
    }
}

/// The row's task shown in `status`: its glyph (in the first span, after
/// the indent) and its text (struck out when done or cancelled).
fn restyle(row: &mut CellRow, status: char) {
    let spans = &mut row.0.spans;
    let Some(first) = spans.first_mut() else {
        return;
    };
    let indent: String = first.content.chars().take_while(|c| *c == ' ').collect();
    *first = Span::styled(format!("{indent}{} ", glyph(status)), first.style);
    if let Some(text) = spans.get_mut(1) {
        let done = matches!(status, 'x' | 'X' | '-');
        text.style = if done {
            Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::CROSSED_OUT)
        } else {
            Style::new()
        };
    }
}

impl<K: Hash + Eq + Clone> Frozen<K> {
    /// The rows held for `key`, while they're held.
    pub fn get(&self, key: &K) -> Option<Vec<CellRow>> {
        let held = self.held.borrow();
        let (rows, until) = held.get(key)?;
        (Instant::now() < *until).then(|| rows.clone())
    }

    /// The task whose row has `action` (its toggle) is now in `status`:
    /// every block showing it (as held, else as `drawn`) is held with it
    /// so, for `grace` from now (none: nothing held).
    pub fn hold(
        &self,
        drawn: &HashMap<K, Vec<CellRow>>,
        action: &str,
        status: char,
        grace: Duration,
    ) {
        if grace.is_zero() {
            self.held.borrow_mut().clear();
            return;
        }
        let until = Instant::now() + grace;
        let mut held = self.held.borrow_mut();
        let mut keys: Vec<K> = held.keys().cloned().collect();
        keys.extend(drawn.keys().filter(|k| !held.contains_key(k)).cloned());
        for key in keys {
            let mut rows = match held.get(&key) {
                Some((rows, _)) => rows.clone(),
                None => drawn[&key].clone(),
            };
            let mut found = false;
            for row in &mut rows {
                if row.1.iter().any(|(.., a)| a == action) {
                    restyle(row, status);
                    found = true;
                }
            }
            if found {
                held.insert(key, (rows, until));
            } else if let Some(entry) = held.get_mut(&key) {
                // Another task held in it: the block stays held as long.
                entry.1 = until.max(entry.1);
            }
        }
    }

    /// Lets go of what's held past its moment; whether anything was (the
    /// blocks are to be drawn anew).
    pub fn expire(&self) -> bool {
        let now = Instant::now();
        let mut held = self.held.borrow_mut();
        let before = held.len();
        held.retain(|_, (_, until)| now < *until);
        held.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delay_is_0_to_10_seconds() {
        assert_eq!(seconds("2"), Duration::from_secs(2));
        assert_eq!(seconds("0"), Duration::ZERO);
        assert_eq!(seconds("10"), Duration::from_secs(10));
        assert_eq!(seconds("11"), Duration::from_secs(10), "at most 10");
        assert_eq!(seconds("-1"), Duration::ZERO, "at least 0");
        assert_eq!(seconds("soon"), Duration::from_millis(3500));
    }
}
