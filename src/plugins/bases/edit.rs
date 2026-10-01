//! Changing a base without writing its YAML (BA-14, BA-51, BA-52): a view's
//! sort, grouping, limit, properties and filters, a new formula or view.
//! The YAML is read, changed and written again (comments go, as when
//! Obsidian rewrites a base); a change that doesn't make a valid base is
//! refused, with the reason.

use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

use super::expr;
use super::syntax;

/// One change to a base's view `view` (or to the base).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Sort by the property, descending or not (replacing the sort).
    Sort(String, bool),
    /// Group by the property, or not at all.
    Group(Option<String>),
    /// At most this many rows, or every row.
    Limit(Option<usize>),
    /// The properties shown, in order.
    Order(Vec<String>),
    /// One more filter (all of the view's filters must hold).
    Filter(String),
    /// A formula: its name and expression.
    Formula(String, String),
    /// A new view: its type and name.
    View(String, String),
}

fn key(k: &str) -> Yaml {
    Yaml::String(k.to_string())
}

fn text(s: &str) -> Yaml {
    Yaml::String(s.to_string())
}

/// The base `source` with `change` made to view `view`: the new YAML and
/// the view to show after it.
pub fn apply(source: &str, view: usize, change: &Change) -> Result<(String, usize), String> {
    match change {
        Change::Filter(e) | Change::Formula(_, e) => {
            expr::parse(e).map_err(|err| format!("{e:?}: {err}"))?;
        }
        _ => {}
    }
    let docs = YamlLoader::load_from_str(source).map_err(|e| format!("YAML: {e}"))?;
    let mut doc = docs.into_iter().next().unwrap_or(Yaml::Hash(Hash::new()));
    let Yaml::Hash(base) = &mut doc else {
        return Err("a base is a set of keys (filters, formulas, views)".into());
    };
    if !matches!(base.get(&key("views")), Some(Yaml::Array(v)) if !v.is_empty()) {
        let mut table = Hash::new();
        table.insert(key("type"), text("table"));
        table.insert(key("name"), text("Table"));
        table.insert(key("order"), Yaml::Array(vec![text("file.name")]));
        base.insert(key("views"), Yaml::Array(vec![Yaml::Hash(table)]));
    }
    let mut shown = view;
    match change {
        Change::Formula(name, e) => {
            let formulas = base
                .entry(key("formulas"))
                .or_insert_with(|| Yaml::Hash(Hash::new()));
            let Yaml::Hash(formulas) = formulas else {
                return Err("formulas is a set of name: expression".into());
            };
            formulas.insert(key(name), text(e));
        }
        Change::View(kind, name) => {
            let Some(Yaml::Array(views)) = base.get_mut(&key("views")) else {
                unreachable!("views was made above");
            };
            let mut v = Hash::new();
            v.insert(key("type"), text(kind));
            v.insert(key("name"), text(name));
            v.insert(key("order"), Yaml::Array(vec![text("file.name")]));
            views.push(Yaml::Hash(v));
            shown = views.len() - 1;
        }
        change => {
            let Some(Yaml::Array(views)) = base.get_mut(&key("views")) else {
                unreachable!("views was made above");
            };
            shown = view.min(views.len() - 1);
            let Yaml::Hash(v) = &mut views[shown] else {
                return Err("a view is a set of keys".into());
            };
            change_view(v, change);
        }
    }
    let mut out = String::new();
    YamlEmitter::new(&mut out)
        .dump(&doc)
        .map_err(|e| format!("YAML: {e}"))?;
    let out = out.strip_prefix("---\n").unwrap_or(&out).to_string();
    // Still a base?
    syntax::parse(&out)?;
    Ok((out, shown))
}

fn change_view(v: &mut Hash, change: &Change) {
    let direction = |desc: bool| text(if desc { "DESC" } else { "ASC" });
    match change {
        Change::Sort(p, desc) => {
            let mut s = Hash::new();
            s.insert(key("property"), text(p));
            s.insert(key("direction"), direction(*desc));
            v.insert(key("sort"), Yaml::Array(vec![Yaml::Hash(s)]));
        }
        Change::Group(None) => {
            v.remove(&key("groupBy"));
        }
        Change::Group(Some(p)) => {
            let mut g = Hash::new();
            g.insert(key("property"), text(p));
            g.insert(key("direction"), direction(false));
            v.insert(key("groupBy"), Yaml::Hash(g));
        }
        Change::Limit(None) => {
            v.remove(&key("limit"));
        }
        Change::Limit(Some(n)) => {
            v.insert(key("limit"), Yaml::Integer(*n as i64));
        }
        Change::Order(props) => {
            v.insert(
                key("order"),
                Yaml::Array(props.iter().map(|p| text(p)).collect()),
            );
        }
        Change::Filter(e) => {
            let filters = match v.remove(&key("filters")) {
                None => text(e),
                // Already "all of these": one more.
                Some(Yaml::Hash(mut h)) if matches!(h.get(&key("and")), Some(Yaml::Array(_))) => {
                    if let Some(Yaml::Array(all)) = h.get_mut(&key("and")) {
                        all.push(text(e));
                    }
                    Yaml::Hash(h)
                }
                Some(old) => {
                    let mut h = Hash::new();
                    h.insert(key("and"), Yaml::Array(vec![old, text(e)]));
                    Yaml::Hash(h)
                }
            };
            v.insert(key("filters"), filters);
        }
        Change::Formula(..) | Change::View(..) => unreachable!("not a view's own change"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "filters: type == \"book\"\nviews:\n  - type: table\n    name: All\n    order: [file.name]\n";

    fn changed(change: Change) -> syntax::Base {
        let (yaml, _) = apply(BASE, 0, &change).unwrap();
        syntax::parse(&yaml).unwrap()
    }

    #[test]
    fn a_views_settings_change() {
        assert_eq!(
            changed(Change::Sort("rating".into(), true)).views[0].sort,
            [("rating".into(), true)]
        );
        let g = changed(Change::Group(Some("status".into())));
        assert_eq!(g.views[0].group_by, Some(("status".into(), false)));
        assert_eq!(changed(Change::Limit(Some(3))).views[0].limit, Some(3));
        assert_eq!(
            changed(Change::Order(vec!["file.name".into(), "rating".into()])).views[0].order,
            ["file.name", "rating"]
        );
        let f = changed(Change::Filter("rating > 3".into()));
        assert!(matches!(&f.views[0].filters, Some(syntax::Filter::Expr(_))));
        let (yaml, _) = apply(BASE, 0, &Change::Filter("a > 1".into())).unwrap();
        let (yaml, _) = apply(&yaml, 0, &Change::Filter("b > 1".into())).unwrap();
        let two = syntax::parse(&yaml).unwrap();
        assert!(matches!(&two.views[0].filters, Some(syntax::Filter::And(all)) if all.len() == 2));
        assert_eq!(
            two.filters,
            syntax::parse(BASE).unwrap().filters,
            "the base's own kept"
        );
    }

    #[test]
    fn formulas_and_views_are_added_checked() {
        let f = changed(Change::Formula("double".into(), "rating * 2".into()));
        assert_eq!(f.formulas[0].0, "double");
        assert!(apply(BASE, 0, &Change::Formula("bad".into(), "rating *".into())).is_err());
        let (yaml, shown) = apply(BASE, 0, &Change::View("list".into(), "Titles".into())).unwrap();
        assert_eq!(shown, 1);
        assert_eq!(syntax::parse(&yaml).unwrap().views[1].name, "Titles");
        let (yaml, _) = apply("", 0, &Change::Limit(Some(2))).unwrap();
        assert_eq!(
            syntax::parse(&yaml).unwrap().views[0].limit,
            Some(2),
            "an empty base"
        );
    }
}
