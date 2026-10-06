//! A base's YAML (BA-10 … BA-15): `filters` (a statement, or `and` / `or`
//! / `not` of lists, nested), `formulas`, `properties` (display names),
//! `summaries` (custom ones) and `views`.

use yaml_rust2::{Yaml, YamlLoader};

use super::expr::{self, Expr};

/// A filter tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Expr(Expr),
    And(Vec<Filter>),
    Or(Vec<Filter>),
    /// None of them.
    Not(Vec<Filter>),
}

/// A view's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Table,
    List,
    Cards,
    Kanban,
}

/// A view.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub kind: Kind,
    pub name: String,
    pub limit: Option<usize>,
    /// The property, and whether it's descending.
    pub group_by: Option<(String, bool)>,
    pub filters: Option<Filter>,
    /// The properties shown, in order.
    pub order: Vec<String>,
    pub sort: Vec<(String, bool)>,
    /// Column → summary name.
    pub summaries: Vec<(String, String)>,
    /// Other settings (`markers`, `separator`, `hideEmptyColumns` …).
    pub options: Vec<(String, String)>,
    /// The groups shown, in order (`groupOrder`; `None` in it: the group
    /// of notes without a value); `None`: every group, sorted.
    pub group_order: Option<Vec<Option<String>>>,
    /// A color per group (`groupColors`, blackglass's: Obsidian ignores
    /// it): group → color name.
    pub group_colors: Vec<(String, String)>,
}

impl View {
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// A base.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Base {
    pub filters: Option<Filter>,
    pub formulas: Vec<(String, Expr)>,
    /// Property → display name.
    pub display: Vec<(String, String)>,
    /// Custom summaries: name → formula over `values`.
    pub summaries: Vec<(String, Expr)>,
    pub views: Vec<View>,
    /// Only this view (an embed of `File.base#View`: a first line
    /// `# view: View`).
    pub only: Option<String>,
}

impl Base {
    /// The column header for `property`.
    pub fn display_name(&self, property: &str) -> String {
        if let Some((_, name)) = self.display.iter().find(|(p, _)| p == property) {
            return name.clone();
        }
        property
            .strip_prefix("note.")
            .or_else(|| property.strip_prefix("formula."))
            .or_else(|| property.strip_prefix("file."))
            .unwrap_or(property)
            .to_string()
    }
}

fn scalar(y: &Yaml) -> Option<String> {
    Some(match y {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        _ => return None,
    })
}

fn key<'a>(y: &'a Yaml, k: &str) -> Option<&'a Yaml> {
    match &y[k] {
        Yaml::BadValue | Yaml::Null => None,
        v => Some(v),
    }
}

fn filter(y: &Yaml) -> Result<Filter, String> {
    if let Some(s) = scalar(y) {
        return expr::parse(&s)
            .map(Filter::Expr)
            .map_err(|e| format!("filter {s:?}: {e}"));
    }
    let list = |y: &Yaml| -> Result<Vec<Filter>, String> {
        match y {
            Yaml::Array(items) => items.iter().map(filter).collect(),
            other => Ok(vec![filter(other)?]),
        }
    };
    if let Some(v) = key(y, "and") {
        return Ok(Filter::And(list(v)?));
    }
    if let Some(v) = key(y, "or") {
        return Ok(Filter::Or(list(v)?));
    }
    if let Some(v) = key(y, "not") {
        return Ok(Filter::Not(list(v)?));
    }
    Err("a filter is a statement, or and / or / not with a list".into())
}

/// Reads a base.
pub fn parse(text: &str) -> Result<Base, String> {
    if text.trim().is_empty() {
        return Ok(Base {
            views: vec![default_view()],
            ..Base::default()
        });
    }
    let docs = YamlLoader::load_from_str(text).map_err(|e| format!("YAML: {e}"))?;
    let Some(doc) = docs.first() else {
        return Ok(Base::default());
    };
    let mut base = Base {
        only: text
            .lines()
            .find(|l| !l.trim().is_empty())
            .and_then(|l| l.trim().strip_prefix("# view:"))
            .map(|v| v.trim().to_string()),
        ..Base::default()
    };
    if let Some(f) = key(doc, "filters") {
        base.filters = Some(filter(f)?);
    }
    if let Some(Yaml::Hash(h)) = key(doc, "formulas") {
        for (k, v) in h {
            let (Some(name), Some(text)) = (scalar(k), scalar(v)) else {
                return Err("a formula is name: expression".into());
            };
            let e = expr::parse(&text).map_err(|e| format!("formula {name}: {e}"))?;
            base.formulas.push((name, e));
        }
    }
    if let Some(Yaml::Hash(h)) = key(doc, "properties") {
        for (k, v) in h {
            if let (Some(p), Some(name)) = (scalar(k), key(v, "displayName").and_then(scalar)) {
                base.display.push((p, name));
            }
        }
    }
    if let Some(Yaml::Hash(h)) = key(doc, "summaries") {
        for (k, v) in h {
            let (Some(name), Some(text)) = (scalar(k), scalar(v)) else {
                continue;
            };
            let e = expr::parse(&text).map_err(|e| format!("summary {name}: {e}"))?;
            base.summaries.push((name, e));
        }
    }
    match key(doc, "views") {
        Some(Yaml::Array(views)) => {
            for v in views {
                base.views.push(view(v)?);
            }
        }
        Some(_) => return Err("views is a list".into()),
        None => {}
    }
    if base.views.is_empty() {
        base.views.push(default_view());
    }
    Ok(base)
}

fn default_view() -> View {
    View {
        kind: Kind::Table,
        name: "Table".into(),
        limit: None,
        group_by: None,
        filters: None,
        order: vec!["file.name".into()],
        sort: Vec::new(),
        summaries: Vec::new(),
        options: Vec::new(),
        group_order: None,
        group_colors: Vec::new(),
    }
}

fn direction(y: &Yaml) -> bool {
    key(y, "direction")
        .and_then(scalar)
        .is_some_and(|d| d.eq_ignore_ascii_case("desc"))
}

fn view(y: &Yaml) -> Result<View, String> {
    let kind = match key(y, "type").and_then(scalar).as_deref() {
        None | Some("table") => Kind::Table,
        Some("list") => Kind::List,
        Some("cards") => Kind::Cards,
        Some("kanban") | Some("board") => Kind::Kanban,
        Some(other) => return Err(format!("no {other} view")),
    };
    let mut v = default_view();
    v.kind = kind;
    v.name = key(y, "name")
        .and_then(scalar)
        .unwrap_or_else(|| match kind {
            Kind::Table => "Table".into(),
            Kind::List => "List".into(),
            Kind::Cards => "Cards".into(),
            Kind::Kanban => "Board".into(),
        });
    v.limit = key(y, "limit")
        .and_then(scalar)
        .and_then(|l| l.parse().ok());
    if let Some(g) = key(y, "groupBy") {
        let property = scalar(g)
            .or_else(|| key(g, "property").and_then(scalar))
            .ok_or("groupBy needs a property")?;
        v.group_by = Some((property, direction(g)));
    }
    if let Some(f) = key(y, "filters") {
        v.filters = Some(filter(f)?);
    }
    if let Some(Yaml::Array(order)) = key(y, "order") {
        v.order = order.iter().filter_map(scalar).collect();
    }
    if let Some(Yaml::Array(sort)) = key(y, "sort") {
        for s in sort {
            let property = scalar(s)
                .or_else(|| key(s, "property").and_then(scalar))
                .ok_or("a sort needs a property")?;
            v.sort.push((property, direction(s)));
        }
    }
    if let Some(Yaml::Hash(h)) = key(y, "summaries") {
        v.summaries = h
            .iter()
            .filter_map(|(k, s)| Some((scalar(k)?, scalar(s)?)))
            .collect();
    }
    match &y["groupOrder"] {
        Yaml::Array(groups) => {
            v.group_order = Some(
                groups
                    .iter()
                    .map(|g| match g {
                        Yaml::Null => None,
                        g => scalar(g),
                    })
                    .collect(),
            );
        }
        Yaml::BadValue | Yaml::Null => {}
        _ => return Err("groupOrder is a list of groups".into()),
    }
    if let Some(Yaml::Hash(h)) = key(y, "groupColors") {
        v.group_colors = h
            .iter()
            .filter_map(|(k, c)| Some((scalar(k)?, scalar(c)?)))
            .collect();
    }
    if let Yaml::Hash(h) = y {
        for (k, val) in h {
            let (Some(k), Some(val)) = (scalar(k), scalar(val)) else {
                continue;
            };
            if !["type", "name", "limit"].contains(&k.as_str()) {
                v.options.push((k, val));
            }
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_is_read_from_yaml() {
        let base = parse(
            "filters:\n  and:\n    - file.hasTag(\"book\")\n    - not:\n        - status == \"dropped\"\nformulas:\n  ppu: price / pages\nproperties:\n  price:\n    displayName: Price\nsummaries:\n  double: values.sum() * 2\nviews:\n  - type: table\n    name: All\n    limit: 5\n    groupBy:\n      property: status\n      direction: DESC\n    order: [file.name, price, formula.ppu]\n    sort:\n      - property: price\n        direction: ASC\n    summaries:\n      price: Average\n  - type: kanban\n    groupBy: status\n    hideEmptyColumns: true\n",
        )
        .unwrap();
        assert!(matches!(&base.filters, Some(Filter::And(f)) if f.len() == 2));
        assert_eq!(base.formulas[0].0, "ppu");
        assert_eq!(base.display_name("price"), "Price");
        assert_eq!(base.display_name("formula.ppu"), "ppu");
        assert_eq!(base.summaries[0].0, "double");
        let v = &base.views[0];
        assert_eq!(
            (v.kind, v.name.as_str(), v.limit),
            (Kind::Table, "All", Some(5))
        );
        assert_eq!(v.group_by, Some(("status".into(), true)));
        assert_eq!(v.order, ["file.name", "price", "formula.ppu"]);
        assert_eq!(v.sort, [("price".into(), false)]);
        assert_eq!(v.summaries, [("price".into(), "Average".into())]);
        let k = &base.views[1];
        assert_eq!((k.kind, k.name.as_str()), (Kind::Kanban, "Board"));
        assert_eq!(k.option("hideEmptyColumns"), Some("true"));
        assert!(parse("filters: rating >").unwrap_err().contains("rating"));
        assert!(
            parse("views:\n  - type: map\n")
                .unwrap_err()
                .contains("map")
        );
        assert!(parse("a: [").unwrap_err().contains("YAML"));
        assert_eq!(parse("").unwrap().views.len(), 1, "every note, in a table");
    }

    #[test]
    fn group_order_and_colors() {
        let base = parse(
            "views:\n  - type: kanban\n    groupBy:\n      property: note.status\n      direction: ASC\n    groupOrder:\n      - Planned\n      - null\n      - Done\n    groupColors:\n      Planned: blue\n      Done: green\n    cardColor: formula.color\n",
        )
        .unwrap();
        let v = &base.views[0];
        assert_eq!(
            v.group_order,
            Some(vec![Some("Planned".into()), None, Some("Done".into())])
        );
        assert_eq!(
            v.group_colors,
            [
                ("Planned".into(), "blue".into()),
                ("Done".into(), "green".into())
            ]
        );
        assert_eq!(v.option("cardColor"), Some("formula.color"));
        assert_eq!(
            parse("views:\n  - type: kanban\n").unwrap().views[0].group_order,
            None
        );
        assert!(parse("views:\n  - groupOrder: x\n").is_err());
    }
}
