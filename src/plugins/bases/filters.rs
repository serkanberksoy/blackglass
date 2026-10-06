//! A base's filters as rows (BA-51), for the filter pane: groups ("all /
//! any / none of these") of conditions, each a property, an operator and a
//! value, as the original's filter menu shows them. Read from the YAML
//! (`filters:` of the base or of a view) and written back as the same
//! YAML: a condition is an expression (`status == "Doing"`,
//! `file.hasTag("garden")`); one this doesn't know stays as written (a
//! formula row).

use yaml_rust2::Yaml;
use yaml_rust2::yaml::Hash;

use super::expr::{self, Expr, Value};

/// How a group's conditions combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conj {
    All,
    Any,
    None,
}

impl Conj {
    /// Its YAML key.
    fn key(self) -> &'static str {
        match self {
            Conj::All => "and",
            Conj::Any => "or",
            Conj::None => "not",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Conj::All => "All of these",
            Conj::Any => "Any of these",
            Conj::None => "None of these",
        }
    }

    pub fn next(self) -> Conj {
        match self {
            Conj::All => Conj::Any,
            Conj::Any => Conj::None,
            Conj::None => Conj::All,
        }
    }
}

/// An operator, as the filter menu names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Is,
    IsNot,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    IsEmpty,
    NotEmpty,
    Gt,
    Ge,
    Lt,
    Le,
    HasTag,
    InFolder,
}

impl Op {
    pub const ALL: [Op; 14] = [
        Op::Is,
        Op::IsNot,
        Op::Contains,
        Op::NotContains,
        Op::StartsWith,
        Op::EndsWith,
        Op::IsEmpty,
        Op::NotEmpty,
        Op::Gt,
        Op::Ge,
        Op::Lt,
        Op::Le,
        Op::HasTag,
        Op::InFolder,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Op::Is => "is",
            Op::IsNot => "is not",
            Op::Contains => "contains",
            Op::NotContains => "does not contain",
            Op::StartsWith => "starts with",
            Op::EndsWith => "ends with",
            Op::IsEmpty => "is empty",
            Op::NotEmpty => "is not empty",
            Op::Gt => ">",
            Op::Ge => "≥",
            Op::Lt => "<",
            Op::Le => "≤",
            Op::HasTag => "has tag",
            Op::InFolder => "is in folder",
        }
    }

    /// Whether it takes a value.
    pub fn has_value(self) -> bool {
        !matches!(self, Op::IsEmpty | Op::NotEmpty)
    }

    /// The operator after (or before) it.
    pub fn step(self, forward: bool) -> Op {
        let i = Op::ALL.iter().position(|&o| o == self).expect("in ALL");
        let n = Op::ALL.len();
        Op::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

/// A condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    /// Property, operator, value (as typed).
    Row(String, Op, String),
    /// An expression this doesn't make rows of, as written.
    Formula(String),
}

/// A filter: a condition or a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Cond(Cond),
    Group(Conj, Vec<Node>),
}

/// A property as written (`status`, `file.tags`, `formula.x`).
fn property(e: &Expr) -> Option<String> {
    match e {
        Expr::Name(n) => Some(n.clone()),
        Expr::Member(m, n) => match m.as_ref() {
            Expr::Name(r) if matches!(r.as_str(), "note" | "file" | "formula") => {
                Some(format!("{r}.{n}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// A literal value, as typed.
fn literal(e: &Expr) -> Option<String> {
    match e {
        Expr::Literal(Value::Text(t)) => Some(t.clone()),
        Expr::Literal(Value::Number(n)) => Some(expr::number(*n)),
        Expr::Literal(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

/// The condition an expression is (or the expression as written).
pub fn cond(text: &str) -> Cond {
    let row = || -> Option<(String, Op, String)> {
        let e = expr::parse(text).ok()?;
        let (negated, e) = match &e {
            Expr::Unary(op, inner) if *op == '!' => (true, inner.as_ref()),
            other => (false, other),
        };
        match e {
            Expr::Binary(op, a, b) if !negated => {
                let op = match op.as_str() {
                    "==" => Op::Is,
                    "!=" => Op::IsNot,
                    ">" => Op::Gt,
                    ">=" => Op::Ge,
                    "<" => Op::Lt,
                    "<=" => Op::Le,
                    _ => return None,
                };
                Some((property(a)?, op, literal(b)?))
            }
            Expr::Method(target, m, args) => {
                if let Expr::Name(f) = target.as_ref()
                    && f == "file"
                    && !negated
                {
                    let op = match m.as_str() {
                        "hasTag" => Op::HasTag,
                        "inFolder" => Op::InFolder,
                        _ => return None,
                    };
                    let p = if op == Op::HasTag {
                        "file.tags"
                    } else {
                        "file.folder"
                    };
                    return Some((p.into(), op, literal(args.first()?)?));
                }
                let p = property(target)?;
                let op = match (m.as_str(), negated) {
                    ("contains", false) => Op::Contains,
                    ("contains", true) => Op::NotContains,
                    ("startsWith", false) => Op::StartsWith,
                    ("endsWith", false) => Op::EndsWith,
                    ("isEmpty", false) => Op::IsEmpty,
                    ("isEmpty", true) => Op::NotEmpty,
                    _ => return None,
                };
                let value = match args.first() {
                    Some(a) => literal(a)?,
                    None if !op.has_value() => String::new(),
                    None => return None,
                };
                Some((p, op, value))
            }
            _ => None,
        }
    };
    match row() {
        Some((p, op, v)) => Cond::Row(p, op, v),
        None => Cond::Formula(text.trim().to_string()),
    }
}

/// A value as an expression: a number or `true` / `false` as it is,
/// anything else quoted.
fn quoted(value: &str) -> String {
    let v = value.trim();
    if v.parse::<f64>().is_ok() || v == "true" || v == "false" {
        return v.to_string();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

impl Cond {
    /// The expression it is.
    pub fn text(&self) -> String {
        let (p, op, v) = match self {
            Cond::Formula(t) => return t.clone(),
            Cond::Row(p, op, v) => (p.trim(), *op, quoted(v)),
        };
        let p = if p.is_empty() { "file.name" } else { p };
        match op {
            Op::Is => format!("{p} == {v}"),
            Op::IsNot => format!("{p} != {v}"),
            Op::Gt => format!("{p} > {v}"),
            Op::Ge => format!("{p} >= {v}"),
            Op::Lt => format!("{p} < {v}"),
            Op::Le => format!("{p} <= {v}"),
            Op::Contains => format!("{p}.contains({v})"),
            Op::NotContains => format!("!{p}.contains({v})"),
            Op::StartsWith => format!("{p}.startsWith({v})"),
            Op::EndsWith => format!("{p}.endsWith({v})"),
            Op::IsEmpty => format!("{p}.isEmpty()"),
            Op::NotEmpty => format!("!{p}.isEmpty()"),
            Op::HasTag => format!("file.hasTag({v})"),
            Op::InFolder => format!("file.inFolder({v})"),
        }
    }
}

/// The filters in `y` (a `filters:` value) as a group: the top one
/// (all of them, when it's a single condition).
pub fn read(y: Option<&Yaml>) -> Node {
    fn node(y: &Yaml) -> Node {
        if let Yaml::Hash(h) = y {
            for conj in [Conj::All, Conj::Any, Conj::None] {
                if let Some(v) = h.get(&Yaml::String(conj.key().into())) {
                    let items = match v {
                        Yaml::Array(items) => items.iter().map(node).collect(),
                        other => vec![node(other)],
                    };
                    return Node::Group(conj, items);
                }
            }
        }
        let text = match y {
            Yaml::String(s) | Yaml::Real(s) => s.clone(),
            Yaml::Integer(i) => i.to_string(),
            Yaml::Boolean(b) => b.to_string(),
            _ => String::new(),
        };
        Node::Cond(cond(&text))
    }
    match y.map(node) {
        None => Node::Group(Conj::All, Vec::new()),
        Some(Node::Cond(c)) => Node::Group(Conj::All, vec![Node::Cond(c)]),
        Some(group) => group,
    }
}

/// The YAML for `node` (`None`: no filters).
pub fn write(node: &Node) -> Option<Yaml> {
    fn yaml(node: &Node) -> Yaml {
        match node {
            Node::Cond(c) => Yaml::String(c.text()),
            Node::Group(conj, items) => {
                let mut h = Hash::new();
                h.insert(
                    Yaml::String(conj.key().into()),
                    Yaml::Array(items.iter().map(yaml).collect()),
                );
                Yaml::Hash(h)
            }
        }
    }
    match node {
        Node::Group(Conj::All, items) if items.is_empty() => None,
        node => Some(yaml(node)),
    }
}

/// The nodes in order as the pane lists them: (path of indices from the
/// top group, depth); the top group is `[]`.
pub fn flatten(top: &Node) -> Vec<(Vec<usize>, usize)> {
    fn walk(node: &Node, path: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, usize)>) {
        out.push((path.clone(), path.len()));
        if let Node::Group(_, items) = node {
            for (i, item) in items.iter().enumerate() {
                path.push(i);
                walk(item, path, out);
                path.pop();
            }
        }
    }
    let mut out = Vec::new();
    walk(top, &mut Vec::new(), &mut out);
    out
}

impl Node {
    pub fn at(&self, path: &[usize]) -> &Node {
        path.iter().fold(self, |n, &i| match n {
            Node::Group(_, items) => &items[i],
            Node::Cond(_) => panic!("a path goes through groups"),
        })
    }

    pub fn at_mut(&mut self, path: &[usize]) -> &mut Node {
        path.iter().fold(self, |n, &i| match n {
            Node::Group(_, items) => &mut items[i],
            Node::Cond(_) => panic!("a path goes through groups"),
        })
    }

    /// Its conditions as one line (the minimized pane).
    pub fn summary(&self) -> String {
        match self {
            Node::Cond(Cond::Formula(t)) => t.clone(),
            Node::Cond(Cond::Row(p, op, v)) if op.has_value() => {
                format!("{p} {} {v}", op.label())
            }
            Node::Cond(Cond::Row(p, op, _)) => format!("{p} {}", op.label()),
            Node::Group(conj, items) => {
                let sep = match conj {
                    Conj::All => " and ",
                    Conj::Any => " or ",
                    Conj::None => ", ",
                };
                let inner: Vec<String> = items.iter().map(Node::summary).collect();
                let inner = inner.join(sep);
                match conj {
                    Conj::None => format!("none of ({inner})"),
                    _ if items.len() > 1 => format!("({inner})"),
                    _ => inner,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust2::YamlLoader;

    fn yaml(text: &str) -> Yaml {
        YamlLoader::load_from_str(text).unwrap().remove(0)
    }

    #[test]
    fn expressions_are_rows() {
        for (text, p, op, v) in [
            ("status == \"Doing\"", "status", Op::Is, "Doing"),
            (
                "note.priority != \"low\"",
                "note.priority",
                Op::IsNot,
                "low",
            ),
            ("rating >= 4", "rating", Op::Ge, "4"),
            ("title.contains(\"x\")", "title", Op::Contains, "x"),
            ("!title.contains(\"x\")", "title", Op::NotContains, "x"),
            (
                "file.name.startsWith(\"A\")",
                "file.name",
                Op::StartsWith,
                "A",
            ),
            ("due.isEmpty()", "due", Op::IsEmpty, ""),
            ("!due.isEmpty()", "due", Op::NotEmpty, ""),
            ("file.hasTag(\"garden\")", "file.tags", Op::HasTag, "garden"),
            (
                "file.inFolder(\"Projects\")",
                "file.folder",
                Op::InFolder,
                "Projects",
            ),
        ] {
            let c = cond(text);
            assert_eq!(c, Cond::Row(p.into(), op, v.into()), "{text}");
            assert_eq!(cond(&c.text()), c, "{text} round trip: {}", c.text());
        }
        assert_eq!(
            cond("price * 2 > pages"),
            Cond::Formula("price * 2 > pages".into())
        );
        assert_eq!(
            Cond::Row("title".into(), Op::Is, "say \"hi\"".into()).text(),
            "title == \"say \\\"hi\\\"\""
        );
    }

    #[test]
    fn groups_read_and_written() {
        let y = yaml(
            "and:\n  - status == \"Doing\"\n  - or:\n      - file.hasTag(\"a\")\n      - rating > 3\n",
        );
        let top = read(Some(&y));
        let Node::Group(Conj::All, items) = &top else {
            panic!("a group: {top:?}");
        };
        assert!(matches!(&items[1], Node::Group(Conj::Any, inner) if inner.len() == 2));
        assert_eq!(read(write(&top).as_ref()), top, "round trip");
        assert_eq!(
            top.summary(),
            "(status is Doing and (file.tags has tag a or rating > 3))"
        );
        assert_eq!(
            flatten(&top),
            [
                (vec![], 0),
                (vec![0], 1),
                (vec![1], 1),
                (vec![1, 0], 2),
                (vec![1, 1], 2)
            ]
        );
        // One condition is a group of one; none is no filters at all.
        let one = read(Some(&yaml("status == \"x\"")));
        assert!(matches!(&one, Node::Group(Conj::All, items) if items.len() == 1));
        assert_eq!(write(&Node::Group(Conj::All, Vec::new())), None);
        assert_eq!(read(None), Node::Group(Conj::All, Vec::new()));
    }
}
