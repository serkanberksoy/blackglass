//! Link autocomplete (W-23): typing `[[` in a note opens a list of notes
//! under the cursor: the most recently changed ones, then, as you type,
//! the best fuzzy matches (as in the quick switcher). ↑/↓ choose, Enter or
//! Tab inserts the link, Esc closes the list.

use crate::switcher::Switcher;

/// Notes listed before anything is typed (the most recently changed).
pub const RECENT: usize = 5;
/// Notes listed for a search.
pub const MATCHES: usize = 8;

/// A plugin's suggestion list (Tasks' fields and dates): the line, the char
/// column the replaced text starts at, (label, text) pairs, the
/// highlighted one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSuggest {
    /// The plugin's id (`None`: blackglass's own tags and properties).
    pub plugin: Option<&'static str>,
    pub row: usize,
    pub start: usize,
    pub items: Vec<(String, String)>,
    /// What choosing an item does besides (item, effect).
    pub effects: Vec<(usize, crate::plugins::Effect)>,
    /// What Shift+Enter puts in instead (item, text).
    pub alts: Vec<(usize, String)>,
    pub selected: usize,
}

/// The open suggestion list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggest {
    /// The line and char column where the link's name starts.
    pub row: usize,
    pub start: usize,
    /// The name typed so far, the ranked notes and the highlighted one.
    pub switcher: Switcher,
}

/// The link being typed at char column `col` of `line`: the char column
/// where its name starts (after `[[`) and the name typed so far. `None`
/// outside a link, after `|` (an alias) or `#` (a heading), and for `^`.
pub fn context(line: &str, col: usize) -> Option<(usize, String)> {
    let before: Vec<char> = line.chars().take(col).collect();
    let start = (1..before.len())
        .rev()
        .find(|&i| before[i - 1] == '[' && before[i] == '[')?
        + 1;
    let name: String = before[start..].iter().collect();
    // `[[^^` (a block anywhere) is for plugins, as `#` is.
    (!name.contains(['[', ']', '|', '#']) && !name.starts_with('^')).then_some((start, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str) -> Option<(usize, String)> {
        // `¦` marks the cursor.
        let col = line.chars().position(|c| c == '¦').expect("a cursor");
        context(&line.replace('¦', ""), col)
    }

    #[test]
    fn the_name_typed_after_double_brackets() {
        assert_eq!(at("see [[¦]]"), Some((6, String::new())));
        assert_eq!(at("see [[Du¦]]"), Some((6, "Du".into())));
        assert_eq!(
            at("[[Du¦"),
            Some((2, "Du".into())),
            "no closing brackets yet"
        );
        assert_eq!(at("![[pic¦]]"), Some((3, "pic".into())), "an embed");
        assert_eq!(
            at("[[a]] and [[b c¦]]"),
            Some((12, "b c".into())),
            "the last link"
        );
    }

    #[test]
    fn not_outside_a_link_or_after_an_alias_or_heading() {
        assert_eq!(at("plain ¦text"), None);
        assert_eq!(at("[[Dune]] after¦"), None, "the link is closed");
        assert_eq!(at("[[Dune|the book¦]]"), None);
        assert_eq!(at("[[Dune#Part¦]]"), None);
        assert_eq!(at("[[^^spi¦]]"), None, "blocks anywhere: a plugin's");
        assert_eq!(at("[[##Par¦]]"), None, "headings anywhere: a plugin's");
        assert_eq!(at("[single¦]"), None);
        assert_eq!(at("[[a [b¦]]"), None, "a bracket in the name");
    }
}
