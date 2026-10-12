//! A ```` ```tracker ```` block's parameters (YAML, the original's
//! names): what to search, where, which dates, how values are taken, and
//! the outputs (`line`, `bar`, `summary`, `month`, `pie`, `bullet`) in the
//! order written.

use std::collections::HashMap;

use yaml_rust2::{Yaml, YamlLoader};

/// What a target is searched as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    /// `#tag` (constant) or `#tag:value`, and frontmatter tags.
    Tag,
    /// A frontmatter property's value.
    Frontmatter,
    /// A frontmatter property there (not empty): the constant.
    FrontmatterExists,
    /// Wiki links to a note.
    Wiki,
    /// A regular expression (a `value` group gives the value), or a word
    /// counted.
    Text,
    /// A Dataview inline field (`name:: value`).
    DvField,
    /// A column of a table in a note.
    Table,
    /// The file's data (`cDate`, `mDate`, `size`, `numWords`, `numChars`,
    /// `numSentences`).
    FileMeta,
    /// Tasks: all, done or not done (their text containing the target).
    Task(TaskKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    All,
    Done,
    NotDone,
}

impl SearchType {
    fn parse(text: &str) -> Option<SearchType> {
        Some(match text.trim().to_lowercase().as_str() {
            "tag" => SearchType::Tag,
            "frontmatter" => SearchType::Frontmatter,
            "frontmatter.exists" => SearchType::FrontmatterExists,
            "wiki" => SearchType::Wiki,
            "text" => SearchType::Text,
            "dvfield" => SearchType::DvField,
            "table" => SearchType::Table,
            "filemeta" => SearchType::FileMeta,
            "task" | "task.all" => SearchType::Task(TaskKind::All),
            "task.done" => SearchType::Task(TaskKind::Done),
            "task.notdone" => SearchType::Task(TaskKind::NotDone),
            _ => return None,
        })
    }
}

/// An output's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Line,
    Bar,
    Summary,
    Month,
    Pie,
    Bullet,
}

/// An output and its parameters (by name, as written).
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub kind: Kind,
    pub opts: Opts,
}

/// Parameters by name: one value, or a list (`a, b` or a YAML list).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opts(HashMap<String, Vec<String>>);

impl Opts {
    /// The first value of `key`.
    pub fn str(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.first().map(String::as_str)
    }

    /// `key`'s values (a list).
    pub fn list(&self, key: &str) -> Vec<String> {
        self.0.get(key).cloned().unwrap_or_default()
    }

    /// `key`'s value as written, its list joined back (for templates and
    /// expressions, which have commas of their own).
    pub fn text(&self, key: &str) -> Option<String> {
        self.0.get(key).map(|v| v.join(", "))
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        self.str(key)?.trim().parse().ok()
    }

    pub fn nums(&self, key: &str) -> Vec<f64> {
        self.list(key)
            .iter()
            .filter_map(|v| v.trim().parse().ok())
            .collect()
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.str(key)?.trim().to_lowercase().as_str() {
            "true" | "yes" | "on" => Some(true),
            "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    /// Value `i` of a list given per dataset: the one value for all, else
    /// the `i`th.
    pub fn nth(&self, key: &str, i: usize) -> Option<&str> {
        let values = self.0.get(key)?;
        if values.len() == 1 {
            values.first()
        } else {
            values.get(i)
        }
        .map(String::as_str)
    }
}

/// A block's parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// The targets, and how each is searched.
    pub targets: Vec<String>,
    pub types: Vec<SearchType>,
    /// Where the notes are (relative to the vault; empty: all of it).
    pub folder: String,
    /// Notes named outright, and whether only they are read.
    pub files: Vec<String>,
    pub specified_only: bool,
    /// How a note's name holds its date: the format (moment's, or
    /// `iso-8601`), with text (regular expressions) before and after.
    pub date_format: String,
    pub date_prefix: String,
    pub date_suffix: String,
    /// The first and last days (dates, or relative to today: `-7d`).
    pub start: Option<String>,
    pub end: Option<String>,
    /// Everything else at the top (per dataset, as lists): `datasetName`,
    /// `separator`, `xDataset`, `constValue`, `penalty` …
    pub root: Opts,
    /// Text values as numbers (`textValueMap`): exact text or a regular
    /// expression.
    pub text_values: Vec<(String, f64)>,
    pub outputs: Vec<Output>,
}

/// The outputs' names, as the original's.
const OUTPUTS: [(&str, Kind); 6] = [
    ("line", Kind::Line),
    ("bar", Kind::Bar),
    ("summary", Kind::Summary),
    ("month", Kind::Month),
    ("pie", Kind::Pie),
    ("bullet", Kind::Bullet),
];

/// A YAML value as text (numbers and booleans as written).
fn scalar(value: &Yaml) -> Option<String> {
    Some(match value {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(n) => n.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Null => String::new(),
        _ => return None,
    })
}

/// A YAML value as a list: a YAML list, or text split at commas outside
/// quotes, brackets and `{{ }}`.
fn values(value: &Yaml) -> Vec<String> {
    match value {
        Yaml::Array(items) => items.iter().filter_map(scalar).collect(),
        Yaml::String(s) if s.trim().is_empty() => vec![String::new()],
        Yaml::String(s) => split(s),
        other => scalar(other).into_iter().collect(),
    }
}

/// `text` split at its top-level commas, each part trimmed.
pub fn split(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut part = String::new();
    for c in text.chars() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '{' | '[' | '(') => depth += 1,
            (None, '}' | ']' | ')') => depth -= 1,
            (None, ',') if depth <= 0 => {
                parts.push(part.trim().to_string());
                part.clear();
                continue;
            }
            _ => {}
        }
        part.push(c);
    }
    if !part.trim().is_empty() || !parts.is_empty() {
        parts.push(part.trim().to_string());
    }
    parts
}

/// A target or value without the quotes around it.
fn unquote(text: &str) -> String {
    let t = text.trim();
    for q in ['\'', '"'] {
        if let Some(inner) = t.strip_prefix(q).and_then(|t| t.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    t.to_string()
}

fn opts(hash: &yaml_rust2::yaml::Hash) -> Opts {
    let mut all = HashMap::new();
    for (key, value) in hash {
        if let Some(key) = scalar(key) {
            all.insert(key, values(value));
        }
    }
    Opts(all)
}

impl Params {
    /// Reads a block's source; `Err` says what's wrong, as the original
    /// does.
    pub fn parse(source: &str) -> Result<Params, String> {
        let docs =
            YamlLoader::load_from_str(source).map_err(|e| format!("Error parsing YAML: {e}"))?;
        let Some(Yaml::Hash(hash)) = docs.into_iter().next() else {
            return Err("Error parsing YAML".into());
        };
        let root = opts(&hash);
        let targets: Vec<String> = root
            .list("searchTarget")
            .iter()
            .map(|t| unquote(t))
            .collect();
        if targets.is_empty() {
            return Err("Parameter 'searchTarget' not found or empty".into());
        }
        let type_names = root.list("searchType");
        if type_names.is_empty() {
            return Err("Parameter 'searchType' not found or empty".into());
        }
        let mut types = Vec::new();
        for i in 0..targets.len() {
            let name = if type_names.len() == 1 {
                &type_names[0]
            } else {
                type_names
                    .get(i)
                    .ok_or("The number of searchTypes must be one or the number of searchTargets")?
            };
            types.push(SearchType::parse(name).ok_or(format!("Invalid searchType '{name}'"))?);
        }
        // Only a task search goes without a target (every task).
        if targets
            .iter()
            .zip(&types)
            .any(|(t, ty)| t.is_empty() && !matches!(ty, SearchType::Task(_)))
        {
            return Err("Parameter 'searchTarget' not found or empty".into());
        }
        let text = |key: &str| root.str(key).map(unquote).filter(|s| !s.is_empty());
        let mut outputs = Vec::new();
        let mut text_values = Vec::new();
        for (key, value) in &hash {
            let Some(key) = scalar(key) else { continue };
            if key == "textValueMap"
                && let Yaml::Hash(map) = value
            {
                for (k, v) in map {
                    let n = scalar(v).and_then(|v| v.trim().parse::<f64>().ok());
                    if let (Some(k), Some(n)) = (scalar(k), n) {
                        text_values.push((k, n));
                    }
                }
            }
            if let Some(&(_, kind)) = OUTPUTS.iter().find(|(name, _)| *name == key) {
                let opts = match value {
                    Yaml::Hash(h) => opts(h),
                    _ => Opts::default(),
                };
                outputs.push(Output { kind, opts });
            }
        }
        if outputs.is_empty() {
            return Err("No output parameter provided, please place line, bar, pie, month, bullet, or summary.".into());
        }
        Ok(Params {
            types,
            folder: text("folder")
                .unwrap_or_default()
                .trim_matches('/')
                .to_string(),
            files: root.list("file").iter().map(|f| unquote(f)).collect(),
            specified_only: root.bool("specifiedFilesOnly").unwrap_or(false),
            date_format: text("dateFormat").unwrap_or_else(|| "YYYY-MM-DD".into()),
            date_prefix: text("dateFormatPrefix").unwrap_or_default(),
            date_suffix: text("dateFormatSuffix").unwrap_or_default(),
            start: text("startDate"),
            end: text("endDate"),
            targets,
            root,
            text_values,
            outputs,
        })
    }

    /// Dataset `i`'s name (`datasetName`, else `untitled`).
    pub fn name(&self, i: usize) -> String {
        self.root
            .nth("datasetName", i)
            .map(unquote)
            .unwrap_or_else(|| "untitled".into())
    }

    /// The index of the target used for dates (`xDataset`), if any.
    pub fn x_dataset(&self) -> Option<usize> {
        self.root
            .list("xDataset")
            .iter()
            .filter_map(|x| x.trim().parse::<i64>().ok())
            .find(|&x| x >= 0)
            .map(|x| x as usize)
            .filter(|&x| x < self.targets.len())
    }

    /// A per-dataset number (`constValue`, `penalty` …).
    pub fn num(&self, key: &str, i: usize) -> Option<f64> {
        self.root.nth(key, i)?.trim().parse().ok()
    }

    /// A per-dataset switch (`accum`, `ignoreZeroValue` …).
    pub fn flag(&self, key: &str, i: usize) -> bool {
        self.root
            .nth(key, i)
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
    }

    /// The separator of a target's values (`/`, `,` for frontmatter lists).
    pub fn separator(&self, i: usize) -> String {
        self.root
            .nth("separator", i)
            .map(unquote)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_types_and_outputs_are_read() {
        let p = Params::parse(
            "searchType: tag, dvField\nsearchTarget: exercise, weight\nfolder: /Daily/\n\
             startDate: -7d\ndatasetName: Run, Weight\naccum: true\n\
             textValueMap:\n  \"😀\": 5\n  bad: 1\n\
             line:\n  title: W\n  lineColor: red, blue\nsummary:\n  template: \"{{sum()}}, {{max()}}\"\n",
        )
        .unwrap();
        assert_eq!(p.targets, ["exercise", "weight"]);
        assert_eq!(p.types, [SearchType::Tag, SearchType::DvField]);
        assert_eq!(p.folder, "Daily");
        assert_eq!(p.start.as_deref(), Some("-7d"));
        assert_eq!((p.name(0), p.name(1)), ("Run".into(), "Weight".into()));
        assert!(p.flag("accum", 0) && p.flag("accum", 1), "one for all");
        assert_eq!(p.text_values, [("😀".into(), 5.0), ("bad".into(), 1.0)]);
        assert_eq!(p.outputs.len(), 2);
        assert_eq!(p.outputs[0].kind, Kind::Line);
        assert_eq!(p.outputs[0].opts.list("lineColor"), ["red", "blue"]);
        assert_eq!(
            p.outputs[1].opts.text("template").as_deref(),
            Some("{{sum()}}, {{max()}}"),
            "commas inside {{ }} stay"
        );
    }

    #[test]
    fn what_is_missing_is_said() {
        let err = |s: &str| Params::parse(s).unwrap_err();
        assert!(err("searchType: tag\nline:\n").contains("searchTarget"));
        assert!(err("searchTarget: a\nline:\n").contains("searchType"));
        assert!(err("searchType: tag\nsearchTarget: ''\nline:\n").contains("searchTarget"));
        assert!(
            Params::parse("searchType: task\nsearchTarget: ''\nline:\n").is_ok(),
            "every task"
        );
        assert!(err("searchType: tags\nsearchTarget: a\nline:\n").contains("Invalid searchType"));
        assert!(err("searchType: tag\nsearchTarget: a\n").contains("No output"));
        let p =
            Params::parse("searchType: task.done\nsearchTarget: '(?<value>\\d+)'\nbar:\n").unwrap();
        assert_eq!(p.targets, ["(?<value>\\d+)"]);
        assert_eq!(p.types, [SearchType::Task(TaskKind::Done)]);
    }
}
