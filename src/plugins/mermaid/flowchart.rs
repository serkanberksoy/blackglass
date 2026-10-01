//! Mermaid flowcharts (`graph` / `flowchart`) as text: every node with its
//! shape (`[box]`, `(round)`, `{decision}`, `((circle))` …, as written),
//! its links under it as a tree (`├─yes─▶ [OK]`), a node already shown
//! as `↺ name`.

/// A link between two nodes.
struct Edge {
    from: usize,
    to: usize,
    label: String,
    /// `─`, `┄` or `═`.
    line: char,
    arrow: bool,
}

/// Node shapes: how they open and close.
const SHAPES: [(&str, &str); 11] = [
    ("((", "))"),
    ("([", "])"),
    ("[(", ")]"),
    ("[[", "]]"),
    ("{{", "}}"),
    ("[/", "/]"),
    ("[\\", "\\]"),
    ("[", "]"),
    ("(", ")"),
    ("{", "}"),
    (">", "]"),
];

#[derive(Default)]
struct Chart {
    /// (id, shown with its shape, its text).
    nodes: Vec<(String, String, String)>,
    edges: Vec<Edge>,
    /// Subgraphs: their titles and nodes.
    groups: Vec<(String, Vec<usize>)>,
}

impl Chart {
    /// The node `id` (added if new); a shape given sets how it's shown.
    fn node(&mut self, id: &str, shown: Shape) -> usize {
        let i = match self.nodes.iter().position(|n| n.0 == id) {
            Some(i) => i,
            None => {
                self.nodes
                    .push((id.to_string(), id.to_string(), id.to_string()));
                self.nodes.len() - 1
            }
        };
        if let Some((shown, text)) = shown
            && self.nodes[i].1 == self.nodes[i].0
        {
            self.nodes[i].1 = shown;
            self.nodes[i].2 = text;
        }
        i
    }
}

/// A node's shape: how it's shown (with its brackets) and its text.
type Shape = Option<(String, String)>;

/// Reads a node at the start of `s`: its id, its shape and the rest.
fn read_node(s: &str) -> Option<(&str, Shape, &str)> {
    let s = s.trim_start();
    let id_len = s
        .char_indices()
        .find(|&(_, c)| !(c.is_alphanumeric() || c == '_'))
        .map_or(s.len(), |(i, _)| i);
    if id_len == 0 {
        return None;
    }
    let (id, rest) = s.split_at(id_len);
    for (open, close) in SHAPES {
        if let Some(inner) = rest.strip_prefix(open)
            && let Some(end) = inner.find(close)
        {
            let text = inner[..end].trim().trim_matches('"').to_string();
            let shown = format!("{open}{text}{close}");
            return Some((id, Some((shown, text)), &inner[end + close.len()..]));
        }
    }
    Some((id, None, rest))
}

/// Reads a link at the start of `s`: (label, line, arrow) and the rest.
fn read_link(s: &str) -> Option<(String, char, bool, &str)> {
    let s = s.trim_start();
    let len = s
        .chars()
        .take_while(|c| matches!(c, '-' | '=' | '.' | '>' | '<'))
        .count();
    if len < 2 {
        return None;
    }
    let (mut token, mut rest) = s.split_at(len);
    let mut label = String::new();
    // `A -- text --> B`: the text between the two halves.
    if !token.ends_with('>') && matches!(token, "--" | "==" | "-.") {
        for end in ["-->", "---", "==>", "===", ".->", "-.-"] {
            if let Some(at) = rest.find(end) {
                label = rest[..at].trim().to_string();
                let after = &rest[at..];
                let more = after
                    .chars()
                    .take_while(|c| matches!(c, '-' | '=' | '.' | '>'))
                    .count();
                token = &after[..more];
                rest = &after[more..];
                break;
            }
        }
    }
    // `-->|text| B`
    let trimmed = rest.trim_start();
    if let Some(inner) = trimmed.strip_prefix('|')
        && let Some(end) = inner.find('|')
    {
        label = inner[..end].trim().to_string();
        rest = &inner[end + 1..];
    }
    let line = if token.contains('=') {
        '═'
    } else if token.contains('.') {
        '┄'
    } else {
        '─'
    };
    Some((label, line, token.ends_with('>'), rest))
}

/// Reads the statements: nodes and chains of links.
fn parse(lines: &[String]) -> Chart {
    let mut chart = Chart::default();
    // The subgraphs open now: their titles and nodes.
    let mut open: Vec<(String, Vec<usize>)> = Vec::new();
    let statements = lines
        .iter()
        .flat_map(|l| l.split(';'))
        .map(str::trim)
        .skip(1); // `graph TD` / `flowchart LR`
    for statement in statements {
        let first = statement.split_whitespace().next().unwrap_or_default();
        if first == "subgraph" {
            let rest = statement["subgraph".len()..].trim();
            let title = match (rest.find('['), rest.rfind(']')) {
                (Some(a), Some(b)) if a < b => rest[a + 1..b].trim(),
                _ => rest,
            };
            open.push((title.trim_matches('"').to_string(), Vec::new()));
            continue;
        }
        if first == "end" {
            if let Some(group) = open.pop() {
                chart.groups.push(group);
            }
            continue;
        }
        if statement.is_empty()
            || statement.starts_with("%%")
            || matches!(
                first,
                "style" | "classDef" | "class" | "click" | "linkStyle" | "direction"
            )
        {
            continue;
        }
        let Some((mut from, mut rest)) = read_nodes(&mut chart, statement) else {
            continue;
        };
        let mut all = from.clone();
        while let Some((label, line, arrow, after)) = read_link(rest) {
            let Some((to, after)) = read_nodes(&mut chart, after) else {
                break;
            };
            for &a in &from {
                for &b in &to {
                    chart.edges.push(Edge {
                        from: a,
                        to: b,
                        label: label.clone(),
                        line,
                        arrow,
                    });
                }
            }
            all.extend(&to);
            from = to;
            rest = after;
        }
        if let Some((_, nodes)) = open.last_mut() {
            for n in all {
                if !nodes.contains(&n) {
                    nodes.push(n);
                }
            }
        }
    }
    chart.groups.extend(open);
    chart
}

/// Nodes joined with `&` (`A & B`), added to the chart; and the rest.
fn read_nodes<'a>(chart: &mut Chart, s: &'a str) -> Option<(Vec<usize>, &'a str)> {
    let (id, shape, mut rest) = read_node(s)?;
    let mut nodes = vec![chart.node(id, shape)];
    while let Some(after) = rest.trim_start().strip_prefix('&') {
        let Some((id, shape, after)) = read_node(after) else {
            break;
        };
        nodes.push(chart.node(id, shape));
        rest = after;
    }
    Some((nodes, rest))
}

/// The flowchart as rows of text.
pub fn render(lines: &[String]) -> Vec<String> {
    let chart = parse(lines);
    let mut shown = vec![false; chart.nodes.len()];
    let mut out = Vec::new();
    let has_incoming = |i: usize| chart.edges.iter().any(|e| e.to == i);
    let roots: Vec<usize> = (0..chart.nodes.len())
        .filter(|&i| !has_incoming(i))
        .collect();
    let mut order = roots;
    order.extend(0..chart.nodes.len());
    for root in order {
        if shown[root] {
            continue;
        }
        if !out.is_empty() {
            out.push(String::new());
        }
        tree(&chart, root, "", &mut shown, &mut out);
    }
    for (title, nodes) in &chart.groups {
        let names: Vec<&str> = nodes.iter().map(|&n| chart.nodes[n].2.as_str()).collect();
        out.push(String::new());
        out.push(format!("▣ {title}: {}", names.join(", ")));
    }
    out
}

/// Node `i`'s line (at `prefix`) and its links below it.
fn tree(chart: &Chart, i: usize, prefix: &str, shown: &mut [bool], out: &mut Vec<String>) {
    // A root writes its own line; the others' is their link's.
    if prefix.is_empty() {
        out.push(chart.nodes[i].1.clone());
    }
    shown[i] = true;
    let edges: Vec<&Edge> = chart.edges.iter().filter(|e| e.from == i).collect();
    for (n, edge) in edges.iter().enumerate() {
        let last = n + 1 == edges.len();
        let branch = if last { '└' } else { '├' };
        let head = if edge.arrow { '▶' } else { edge.line };
        let link = if edge.label.is_empty() {
            format!("{branch}{}{head}", edge.line)
        } else {
            format!("{branch}{}{}{}{head}", edge.line, edge.label, edge.line)
        };
        let target = &chart.nodes[edge.to];
        if shown[edge.to] {
            out.push(format!("{prefix}{link} ↺ {}", target.2));
            continue;
        }
        out.push(format!("{prefix}{link} {}", target.1));
        let deeper = format!("{prefix}{}", if last { "    " } else { "│   " });
        tree(chart, edge.to, &deeper, shown, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(source: &str) -> Vec<String> {
        let lines: Vec<String> = source.lines().map(String::from).collect();
        render(&lines)
    }

    #[test]
    fn a_flowchart_is_a_tree_of_its_links() {
        let chart = draw(
            "flowchart TD\n  A[Start] --> B{Is it?}\n  B -->|yes| C[OK]\n  B -- no --> D(Retry)\n  D -.-> A\n  %% a comment\n  style A fill:#f9f",
        );
        assert_eq!(
            chart,
            [
                "[Start]",
                "└─▶ {Is it?}",
                "    ├─yes─▶ [OK]",
                "    └─no─▶ (Retry)",
                "        └┄▶ ↺ Start",
            ]
        );
    }

    #[test]
    fn chains_shapes_and_several_roots() {
        let chart = draw("graph LR; A((One)) --> B([Two]) --- C; X>Flag] ==> C");
        assert_eq!(
            chart,
            [
                "((One))",
                "└─▶ ([Two])",
                "    └── C",
                "",
                ">Flag]",
                "└═▶ ↺ C"
            ]
        );
    }

    #[test]
    fn ampersands_and_subgraphs() {
        let chart = draw("graph TD\nsubgraph one [First group]\n  A & B --> C\nend\nC --> D");
        assert_eq!(
            chart,
            [
                "A",
                "└─▶ C",
                "    └─▶ D",
                "",
                "B",
                "└─▶ ↺ C",
                "",
                "▣ First group: A, B, C",
            ]
        );
    }

    #[test]
    fn nodes_without_links_are_listed() {
        assert_eq!(draw("graph TD\nA[Alone]\nB"), ["[Alone]", "", "B"]);
    }
}
