//! Reading a bibliography: BibTeX (`.bib`, as Zotero's Better BibTeX
//! exports it) or CSL JSON (`.json`): each reference's key, title,
//! authors, year, abstract and web address.

use serde_json::Value;

/// A reference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reference {
    /// Its citation key (`doe2020`).
    pub key: String,
    pub title: String,
    /// Its authors as written: "Last, First" or "First Last".
    pub authors: Vec<String>,
    pub year: String,
    pub abstract_: String,
    pub url: String,
}

/// "Last" of an author written "Last, First" or "First Last".
fn last_name(author: &str) -> String {
    match author.split_once(',') {
        Some((last, _)) => last.trim().to_string(),
        None => author
            .split_whitespace()
            .last()
            .unwrap_or(author)
            .to_string(),
    }
}

/// "First Last" of an author written either way.
fn full_name(author: &str) -> String {
    match author.split_once(',') {
        Some((last, first)) => format!("{} {}", first.trim(), last.trim()),
        None => author.trim().to_string(),
    }
}

impl Reference {
    /// "Doe", "Doe & Smith", "Doe et al." (the first author's last name
    /// for three or more).
    pub fn author_short(&self) -> String {
        match self.authors.as_slice() {
            [] => String::new(),
            [one] => last_name(one),
            [a, b] => format!("{} & {}", last_name(a), last_name(b)),
            [a, ..] => format!("{} et al.", last_name(a)),
        }
    }

    /// "Doe 2020": how a citation is shown.
    pub fn short(&self) -> String {
        let who = self.author_short();
        match (who.is_empty(), self.year.is_empty()) {
            (false, false) => format!("{who} {}", self.year),
            (false, true) => who,
            (true, false) => format!("{} {}", self.key, self.year),
            (true, true) => self.key.clone(),
        }
    }

    /// The authors' full names, joined.
    pub fn authors_full(&self) -> String {
        self.authors
            .iter()
            .map(|a| full_name(a))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The references in a bibliography's `text`: CSL JSON if it's JSON,
/// else BibTeX.
pub fn parse(text: &str) -> Vec<Reference> {
    if text.trim_start().starts_with('[') {
        return csl_json(text);
    }
    bibtex(text)
}

/// CSL JSON: a list of `{id, title, author: [{family, given} | {literal}],
/// issued: {date-parts: [[year]]}, URL, abstract}`.
fn csl_json(text: &str) -> Vec<Reference> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let s = |k: &str| item[k].as_str().unwrap_or_default().to_string();
            let key = s("id");
            if key.is_empty() {
                return None;
            }
            let authors = item["author"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| match (a["family"].as_str(), a["given"].as_str()) {
                    (Some(f), Some(g)) => Some(format!("{f}, {g}")),
                    (Some(f), None) => Some(f.to_string()),
                    _ => a["literal"].as_str().map(String::from),
                })
                .collect();
            let year = item["issued"]["date-parts"][0][0]
                .as_i64()
                .map(|y| y.to_string())
                .or_else(|| {
                    item["issued"]["date-parts"][0][0]
                        .as_str()
                        .map(String::from)
                })
                .unwrap_or_default();
            Some(Reference {
                key,
                title: s("title"),
                authors,
                year,
                abstract_: s("abstract"),
                url: s("URL"),
            })
        })
        .collect()
}

/// BibTeX: `@type{key, field = {value} | "value" | number, …}`.
fn bibtex(text: &str) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        rest = &rest[at + 1..];
        let Some(open) = rest.find(['{', '(']) else {
            break;
        };
        let kind = rest[..open].trim().to_lowercase();
        let body_start = open + 1;
        // The entry ends where its braces balance.
        let mut depth = 1;
        let mut end = None;
        for (i, c) in rest[body_start..].char_indices() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(body_start + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            break;
        };
        let body = &rest[body_start..end];
        rest = &rest[end + 1..];
        if matches!(kind.as_str(), "comment" | "string" | "preamble") {
            continue;
        }
        let Some((key, fields)) = body.split_once(',') else {
            continue;
        };
        let mut r = Reference {
            key: key.trim().to_string(),
            ..Reference::default()
        };
        for (name, value) in fields_of(fields) {
            match name.as_str() {
                "title" => r.title = value,
                "author" => {
                    r.authors = value
                        .split(" and ")
                        .map(|a| a.trim().to_string())
                        .filter(|a| !a.is_empty())
                        .collect();
                }
                "year" => r.year = value,
                "date" if r.year.is_empty() => r.year = value.chars().take(4).collect(),
                "abstract" => r.abstract_ = value,
                "url" => r.url = value,
                _ => {}
            }
        }
        if !r.key.is_empty() {
            out.push(r);
        }
    }
    out
}

/// An entry's `name = value` fields (names lowercase, values without
/// their braces or quotes).
fn fields_of(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let Some(eq) = chars[i..].iter().position(|&c| c == '=').map(|p| i + p) else {
            break;
        };
        let name: String = chars[i..eq].iter().collect();
        let name = name.trim().trim_start_matches(',').trim().to_lowercase();
        let mut j = eq + 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        let mut value = String::new();
        match chars.get(j) {
            Some('{') => {
                let mut depth = 0;
                while j < chars.len() {
                    match chars[j] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                j += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    value.push(chars[j]);
                    j += 1;
                }
            }
            Some('"') => {
                j += 1;
                while j < chars.len() && chars[j] != '"' {
                    value.push(chars[j]);
                    j += 1;
                }
                j += 1;
            }
            _ => {
                while j < chars.len() && chars[j] != ',' {
                    value.push(chars[j]);
                    j += 1;
                }
            }
        }
        // Inner braces (`{Spice}`, kept capitals) aren't shown.
        let value: String = value.chars().filter(|&c| c != '{' && c != '}').collect();
        let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if !name.is_empty() {
            out.push((name, value));
        }
        while j < chars.len() && chars[j] != ',' {
            j += 1;
        }
        i = j + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibtex_and_csl_json_are_read() {
        let bib = "@comment{x}\n@book{herbert1965,\n title = {Dune},\n author = {Herbert, Frank},\n year = 1965,\n abstract = {A {desert} planet.}\n}\n@article{doe2020, title = \"On {Spice}\", author = {Doe, Jane and Ann Smith}, date = {2020-05-01}}";
        let refs = parse(bib);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].key, "herbert1965");
        assert_eq!(refs[0].abstract_, "A desert planet.");
        assert_eq!(refs[0].short(), "Herbert 1965");
        assert_eq!(refs[1].title, "On Spice");
        assert_eq!(refs[1].short(), "Doe & Smith 2020");
        assert_eq!(refs[1].authors_full(), "Jane Doe, Ann Smith");
        let json = r#"[{"id": "roe2001", "title": "T", "author": [{"family": "Roe", "given": "R"}, {"family": "A"}, {"literal": "Org"}], "issued": {"date-parts": [[2001, 2]]}, "URL": "https://x"}]"#;
        let refs = parse(json);
        assert_eq!(refs[0].short(), "Roe et al. 2001");
        assert_eq!(refs[0].url, "https://x");
    }
}
