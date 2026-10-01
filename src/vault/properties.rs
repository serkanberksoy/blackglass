//! A note's properties: the `key: value` lines of its frontmatter (a list
//! as `- item` lines or `[a, b]` is joined with `, `), and its `aliases`.

/// The frontmatter's top-level `key: value`s, in order: values unquoted,
/// lists (`[a, b]` or `- item` lines) joined with `, `; a key with nested
/// keys under it has an empty value.
pub fn extract(lines: &[String]) -> Vec<(String, String)> {
    let Some(end) = frontmatter_end(lines) else {
        return Vec::new();
    };
    let mut props: Vec<(String, String)> = Vec::new();
    for line in &lines[1..end] {
        let indented = line.starts_with([' ', '\t']);
        let trimmed = line.trim();
        if let Some(item) = trimmed
            .strip_prefix("- ")
            .filter(|_| indented || trimmed.starts_with('-'))
        {
            if let Some((_, value)) = props.last_mut() {
                if !value.is_empty() {
                    value.push_str(", ");
                }
                value.push_str(unquote(item));
            }
            continue;
        }
        if indented {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let value = match value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            Some(list) => list
                .split(',')
                .map(|item| unquote(item.trim()))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
            None => unquote(value).to_string(),
        };
        props.push((key.trim().to_string(), value));
    }
    props
}

/// The note's other names: `aliases` (or `alias`), one per list item.
pub fn aliases(props: &[(String, String)]) -> Vec<String> {
    props
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "aliases" | "alias"))
        .flat_map(|(_, value)| value.split(", "))
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(String::from)
        .collect()
}

fn unquote(text: &str) -> &str {
    text.trim_matches(|c| c == '"' || c == '\'')
}

/// The line of the frontmatter's closing `---` (or `...`).
pub fn frontmatter_end(lines: &[String]) -> Option<usize> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return None;
    }
    (1..lines.len()).find(|&i| matches!(lines[i].trim_end(), "---" | "..."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn properties_and_aliases_from_the_frontmatter() {
        let note = lines(
            "---\ntitle: \"Dune\"\naliases: [Arrakis book, 'Spice']\ntags:\n  - a\n  - b\nnested:\n  key: x\n---\nbody: no",
        );
        let props = extract(&note);
        assert_eq!(
            props,
            [
                ("title".into(), "Dune".into()),
                ("aliases".into(), "Arrakis book, Spice".into()),
                ("tags".into(), "a, b".into()),
                ("nested".into(), String::new()),
            ]
        );
        assert_eq!(aliases(&props), ["Arrakis book", "Spice"]);
        assert!(extract(&lines("no: frontmatter")).is_empty());
    }
}
