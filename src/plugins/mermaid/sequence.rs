//! Mermaid sequence diagrams as text: the participants side by side, a
//! lifeline under each, and every message as an arrow between them (solid
//! `──▶`, dotted `┄┄▶`) with its text above it; notes and `loop` / `alt`
//! blocks as rows of their own.

/// A participant: its id and the name shown.
struct Party {
    id: String,
    label: String,
}

/// What a row of the diagram is.
enum Step {
    /// From, to (indexes), dotted, text.
    Message(usize, usize, bool, String),
    Note(usize, String),
    /// `loop …`, `alt …`, `end` …
    Block(String),
}

/// The diagram's rows, at most `width` wide (else as a list of messages).
pub fn render(lines: &[String], width: usize) -> Vec<String> {
    let mut parties: Vec<Party> = Vec::new();
    let mut steps = Vec::new();
    // `autonumber`: messages numbered from 1.
    let (mut numbered, mut count) = (false, 0);
    let index = |parties: &mut Vec<Party>, id: &str| -> usize {
        let id = id.trim();
        if let Some(i) = parties.iter().position(|p| p.id == id) {
            return i;
        }
        parties.push(Party {
            id: id.to_string(),
            label: id.to_string(),
        });
        parties.len() - 1
    };
    for line in lines.iter().skip(1) {
        let line = line.trim();
        let word = line.split_whitespace().next().unwrap_or_default();
        match word {
            "autonumber" => numbered = true,
            "" | "activate" | "deactivate" => {}
            _ if line.starts_with("%%") => {}
            "participant" | "actor" => {
                let rest = line[word.len()..].trim();
                let (id, label) = rest.split_once(" as ").unwrap_or((rest, rest));
                let i = index(&mut parties, id);
                parties[i].label = label.trim().to_string();
            }
            "loop" | "alt" | "else" | "opt" | "par" | "and" | "critical" | "break" | "rect"
            | "end" => {
                steps.push(Step::Block(line.to_string()));
            }
            _ if word.eq_ignore_ascii_case("note") => {
                let (head, text) = line.split_once(':').unwrap_or((line, ""));
                let who = head
                    .split_whitespace()
                    .last()
                    .unwrap_or_default()
                    .split(',')
                    .next()
                    .unwrap_or_default();
                let i = index(&mut parties, who);
                steps.push(Step::Note(i, text.trim().to_string()));
            }
            _ => {
                let Some((from, arrow, rest)) = split_arrow(line) else {
                    continue;
                };
                let (to, text) = rest.split_once(':').unwrap_or((rest, ""));
                let to = to.trim_start_matches(['+', '-']);
                let (a, b) = (index(&mut parties, from), index(&mut parties, to));
                let text = if numbered {
                    count += 1;
                    format!("{count}. {}", text.trim())
                } else {
                    text.trim().to_string()
                };
                steps.push(Step::Message(a, b, arrow.starts_with("--"), text));
            }
        }
    }
    if parties.is_empty() {
        return Vec::new();
    }
    // Where each lifeline is: room for the labels and the messages between
    // neighbours.
    let mut centers: Vec<usize> = Vec::new();
    for (i, p) in parties.iter().enumerate() {
        let w = p.label.chars().count();
        let center = if i == 0 {
            w / 2
        } else {
            let prev = centers[i - 1];
            let prev_end = prev - parties[i - 1].label.chars().count() / 2
                + parties[i - 1].label.chars().count();
            let text = steps
                .iter()
                .filter_map(|s| match s {
                    Step::Message(a, b, _, t) if a.min(b) + 1 == i && a.max(b) == &i => {
                        Some(t.chars().count())
                    }
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            (prev + text + 3).max(prev_end + 2 + w / 2)
        };
        centers.push(center);
    }
    let last = parties.len() - 1;
    let full = centers[last] + parties[last].label.chars().count()
        - parties[last].label.chars().count() / 2;
    if full > width {
        return steps
            .iter()
            .map(|s| match s {
                Step::Message(a, b, dotted, t) => format!(
                    "{} {} {}: {t}",
                    parties[*a].label,
                    if *dotted { "┄┄▶" } else { "──▶" },
                    parties[*b].label
                ),
                Step::Note(a, t) => format!("Note ({}): {t}", parties[*a].label),
                Step::Block(t) => format!("┄ {t}"),
            })
            .collect();
    }
    let blank = |upto: usize| -> Vec<char> {
        let mut row = vec![' '; full.max(1)];
        for &c in centers.iter().filter(|&&c| c <= upto) {
            row[c] = '│';
        }
        row
    };
    let text = |row: Vec<char>| row.into_iter().collect::<String>().trim_end().to_string();
    let put = |row: &mut Vec<char>, at: usize, s: &str| {
        for (i, c) in s.chars().enumerate() {
            if at + i < row.len() {
                row[at + i] = c;
            } else {
                row.push(c);
            }
        }
    };
    let mut out = Vec::new();
    let mut header = vec![' '; full.max(1)];
    for (p, &c) in parties.iter().zip(&centers) {
        put(&mut header, c - p.label.chars().count() / 2, &p.label);
    }
    out.push(text(header));
    for step in &steps {
        match step {
            Step::Block(t) => out.push(format!("┄ {t}")),
            Step::Note(a, t) => {
                let mut row = blank(centers[*a]);
                row.truncate(centers[*a] + 1);
                put(&mut row, centers[*a] + 1, &format!(" Note: {t}"));
                out.push(text(row));
            }
            Step::Message(a, b, dotted, t) if a == b => {
                let c = centers[*a];
                let mut row = blank(usize::MAX);
                row.truncate(c + 1);
                put(&mut row, c + 1, &format!(" {t}"));
                out.push(text(row));
                let mut row = blank(usize::MAX);
                row.truncate(c + 1);
                row[c] = '├';
                put(&mut row, c + 1, if *dotted { "┄↺" } else { "─↺" });
                out.push(text(row));
            }
            Step::Message(a, b, dotted, t) => {
                let (l, r) = (centers[*a.min(b)], centers[*a.max(b)]);
                let mut row = blank(usize::MAX);
                put(&mut row, l + 2, t);
                out.push(text(row));
                let mut row = blank(usize::MAX);
                let fill = if *dotted { '┄' } else { '─' };
                for cell in &mut row[l + 1..r] {
                    *cell = fill;
                }
                if a < b {
                    row[l] = '├';
                    row[r - 1] = '▶';
                } else {
                    row[l + 1] = '◀';
                    row[r] = '┤';
                }
                out.push(text(row));
            }
        }
    }
    out
}

/// A message line's sender, arrow and the rest (`Alice->>Bob: Hi`).
fn split_arrow(line: &str) -> Option<(&str, &str, &str)> {
    let start = line.find('-')?;
    let arrow_len = line[start..]
        .chars()
        .take_while(|c| matches!(c, '-' | '>' | 'x' | ')' | '<'))
        .count();
    let arrow = &line[start..start + arrow_len];
    if !arrow.contains('>') && !arrow.ends_with('x') && !arrow.ends_with(')') {
        return None;
    }
    Some((
        line[..start].trim(),
        arrow,
        line[start + arrow_len..].trim(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(source: &str, width: usize) -> Vec<String> {
        let lines: Vec<String> = source.lines().map(String::from).collect();
        render(&lines, width)
    }

    #[test]
    fn messages_are_arrows_between_lifelines() {
        let chart = draw(
            "sequenceDiagram\n  participant A as Alice\n  A->>Bob: Hello\n  Bob-->>A: Hi!\n  loop Every day\n  Bob->>Bob: Think\n  end\n  Note over A: Done",
            80,
        );
        assert_eq!(
            chart,
            [
                "Alice    Bob",
                "  │ Hello │",
                "  ├──────▶│",
                "  │ Hi!   │",
                "  │◀┄┄┄┄┄┄┤",
                "┄ loop Every day",
                "  │       │ Think",
                "  │       ├─↺",
                "┄ end",
                "  │ Note: Done",
            ]
        );
    }

    #[test]
    fn autonumber_numbers_the_messages() {
        let chart = draw("sequenceDiagram\nautonumber\nA->>B: one\nB-->>A: two", 80);
        assert_eq!(chart[1], "│ 1. one │", "{chart:#?}");
        assert_eq!(chart[3], "│ 2. two │", "{chart:#?}");
    }

    #[test]
    fn too_wide_becomes_a_list() {
        let chart = draw("sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: Hi", 8);
        assert_eq!(chart, ["Alice ──▶ Bob: Hello", "Bob ┄┄▶ Alice: Hi"]);
    }
}
