//! The property editor's model (W-118): a note's frontmatter as a list of
//! properties with types, and back to text. What it can't edit (nested
//! keys) is kept as it is.

/// A property's value, by its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Checkbox(bool),
    /// As written (`42`, `3.5`).
    Number(String),
    /// As written (`2026-09-30`, with a time too).
    Date(String),
    /// The items, and whether they're written inline (`[a, b]`) or as
    /// `- item` lines.
    List(Vec<String>, bool),
    /// Lines under the key kept as they are (nested keys): not editable
    /// here.
    Nested(Vec<String>),
}

/// A property: its key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub value: Value,
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}

fn is_date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..10]
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

impl Value {
    /// The value a typed text stands for: `true` / `false` a checkbox, a
    /// number, a date, `[a, b]` a list, else text.
    pub fn guess(text: &str) -> Value {
        let text = text.trim();
        match text {
            "true" => return Value::Checkbox(true),
            "false" => return Value::Checkbox(false),
            _ => {}
        }
        if let Some(inner) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
            let items = inner
                .split(',')
                .map(|i| unquote(i.trim()).to_string())
                .filter(|i| !i.is_empty())
                .collect();
            return Value::List(items, true);
        }
        if !text.is_empty() && text.parse::<f64>().is_ok() {
            return Value::Number(text.into());
        }
        if is_date(text) {
            return Value::Date(text.into());
        }
        Value::Text(unquote(text).to_string())
    }

    /// Its type's name.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Text(_) => "text",
            Value::Checkbox(_) => "checkbox",
            Value::Number(_) => "number",
            Value::Date(_) => "date",
            Value::List(..) => "list",
            Value::Nested(_) => "nested",
        }
    }

    /// As edited (and shown): a list as `a, b`.
    pub fn edit_text(&self) -> String {
        match self {
            Value::Text(t) | Value::Number(t) | Value::Date(t) => t.clone(),
            Value::Checkbox(b) => b.to_string(),
            Value::List(items, _) => items.join(", "),
            Value::Nested(lines) => format!("({} nested lines)", lines.len()),
        }
    }

    /// The value after `text` was typed for it: a list stays a list (split
    /// at commas, in its style); anything else is guessed from the text.
    pub fn edited(&self, text: &str) -> Value {
        match self {
            Value::List(_, inline) => Value::List(
                text.split(',')
                    .map(|i| i.trim().to_string())
                    .filter(|i| !i.is_empty())
                    .collect(),
                *inline,
            ),
            Value::Nested(_) => self.clone(),
            _ => Value::guess(text),
        }
    }
}

/// Whether text has to be quoted to stay text in YAML.
fn needs_quotes(text: &str) -> bool {
    text.is_empty()
        || text.contains(": ")
        || text.contains(" #")
        || text.starts_with([
            '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`', '[', '{', '-',
        ])
        || !matches!(Value::guess(text), Value::Text(_))
}

/// The note's properties (from its frontmatter) and the text after it.
pub fn parse(text: &str) -> (Vec<Property>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (Vec::new(), text);
    };
    let mut at = 0;
    let mut end = None;
    for line in rest.split_inclusive('\n') {
        if matches!(line.trim_end(), "---" | "...") {
            end = Some((at, at + line.len()));
            break;
        }
        at += line.len();
    }
    let Some((inside, after)) = end else {
        return (Vec::new(), text);
    };
    let lines: Vec<&str> = rest[..inside].lines().collect();
    let mut props: Vec<Property> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        let Some((key, value)) = line
            .split_once(':')
            .filter(|_| !line.starts_with([' ', '\t']))
        else {
            continue;
        };
        let value = value.trim();
        let value = if value.is_empty() {
            let under: Vec<&str> = lines[i..]
                .iter()
                .take_while(|l| l.starts_with([' ', '\t']) || l.trim().is_empty())
                .copied()
                .collect();
            i += under.len();
            if under.iter().all(|l| l.trim_start().starts_with("- ")) && !under.is_empty() {
                let items = under
                    .iter()
                    .map(|l| unquote(l.trim_start()[2..].trim()).to_string())
                    .collect();
                Value::List(items, false)
            } else if under.is_empty() {
                Value::Text(String::new())
            } else {
                Value::Nested(under.iter().map(|l| l.to_string()).collect())
            }
        } else {
            Value::guess(value)
        };
        props.push(Property {
            key: key.trim().to_string(),
            value,
        });
    }
    (props, &rest[after..])
}

/// The note's text with `props` as its frontmatter (none: no frontmatter)
/// before `body`.
pub fn write(props: &[Property], body: &str) -> String {
    if props.is_empty() {
        return body.to_string();
    }
    let mut out = String::from("---\n");
    for p in props {
        let key = &p.key;
        match &p.value {
            Value::Text(t) if t.is_empty() => out.push_str(&format!("{key}:\n")),
            Value::Text(t) if needs_quotes(t) => {
                out.push_str(&format!("{key}: \"{}\"\n", t.replace('"', "\\\"")))
            }
            Value::Text(t) | Value::Number(t) | Value::Date(t) => {
                out.push_str(&format!("{key}: {t}\n"))
            }
            Value::Checkbox(b) => out.push_str(&format!("{key}: {b}\n")),
            Value::List(items, true) => out.push_str(&format!("{key}: [{}]\n", items.join(", "))),
            Value::List(items, false) => {
                out.push_str(&format!("{key}:\n"));
                for item in items {
                    out.push_str(&format!("  - {item}\n"));
                }
            }
            Value::Nested(lines) => {
                out.push_str(&format!("{key}:\n"));
                for line in lines {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_is_read_and_written_back() {
        let text = "---\ntitle: \"Dune: 1\"\ndone: false\nn: 42\nday: 2026-09-30\ntags: [a, b]\nlist:\n  - one\n  - two\nnested:\n  key: x\n---\nBody\n";
        let (props, body) = parse(text);
        assert_eq!(body, "Body\n");
        let kinds: Vec<&str> = props.iter().map(|p| p.value.kind()).collect();
        assert_eq!(
            kinds,
            [
                "text", "checkbox", "number", "date", "list", "list", "nested"
            ]
        );
        assert_eq!(props[0].value, Value::Text("Dune: 1".into()));
        assert_eq!(write(&props, body), text, "unchanged: the same text");
        let (none, body) = parse("Body");
        assert!(none.is_empty() && body == "Body");
        assert_eq!(
            write(&none, "Body"),
            "Body",
            "no properties: no frontmatter"
        );
        assert_eq!(Value::guess("true"), Value::Checkbox(true));
        assert_eq!(
            Value::guess("[a, b]"),
            Value::List(vec!["a".into(), "b".into()], true)
        );
        assert_eq!(Value::guess("a: b"), Value::Text("a: b".into()));
    }
}
