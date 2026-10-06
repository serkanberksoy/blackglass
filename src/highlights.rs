//! Highlight colors (W-130), as in the original's 1.14: a color emoji at
//! a highlight's start colors it (`==🔴text==`; mdedit draws it, T-07a).
//! Typing `==` suggests the colors; commands highlight the selection in a
//! color, or change (or remove) the color of the highlight at the cursor.

use mdedit::markdown::HIGHLIGHT_COLORS;

use crate::plugins::Suggestions;

/// Where `line`'s `==` marks are (char columns, outside code spans).
fn marks(line: &str) -> Vec<usize> {
    let chars: Vec<char> = line.chars().collect();
    let mut markers = Vec::new();
    let mut code = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            code = !code;
        } else if !code
            && chars[i] == '='
            && chars.get(i + 1) == Some(&'=')
            && chars.get(i + 2) != Some(&'=')
            && (i == 0 || chars[i - 1] != '=')
        {
            markers.push(i);
            i += 2;
            continue;
        }
        i += 1;
    }
    markers
}

/// The `==` pairs of `line`, as char ranges from the opener's first `=`
/// to the closer's end.
fn pairs(line: &str) -> Vec<(usize, usize)> {
    marks(line)
        .chunks_exact(2)
        .map(|p| (p[0], p[1] + 2))
        .collect()
}

/// The colors suggested when `==` opening a highlight was just typed
/// before char `col` of `line` (replacing it with `==🔴` …).
pub fn suggestions(line: &str, col: usize) -> Option<Suggestions> {
    let before: Vec<char> = line.chars().take(col).collect();
    let n = before.len();
    if n < 2 || before[n - 2..] != ['=', '='] || (n > 2 && before[n - 3] == '=') {
        return None;
    }
    // An opener (an even number of marks before it), not in code.
    let ahead: String = before[..n - 2].iter().collect();
    let opener =
        ahead.matches('`').count().is_multiple_of(2) && marks(&ahead).len().is_multiple_of(2);
    opener.then(|| Suggestions {
        start: n - 2,
        items: HIGHLIGHT_COLORS
            .iter()
            .map(|(emoji, _, name)| (format!("{emoji}  {name}"), format!("=={emoji}")))
            .collect(),
        ..Suggestions::default()
    })
}

/// `line` with the highlight at char `col` in the color `emoji` (`None`:
/// no color); `None` if the cursor isn't in a highlight.
pub fn recolor(line: &str, col: usize, emoji: Option<&str>) -> Option<String> {
    let (start, end) = pairs(line)
        .into_iter()
        .find(|&(s, e)| (s..=e).contains(&col))?;
    let chars: Vec<char> = line.chars().collect();
    let inner: String = chars[start + 2..end - 2].iter().collect();
    let text = HIGHLIGHT_COLORS
        .iter()
        .find_map(|(e, _, _)| inner.strip_prefix(e))
        .unwrap_or(&inner);
    let head: String = chars[..start].iter().collect();
    let tail: String = chars[end..].iter().collect();
    Some(format!(
        "{head}=={}{text}=={tail}",
        emoji.unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_suggested_after_an_opening_mark() {
        let s = suggestions("a ==", 4).expect("suggested");
        assert_eq!(s.start, 2);
        assert_eq!(s.items.len(), 6);
        assert_eq!(s.items[0], ("🔴  red".into(), "==🔴".into()));
        for (line, col) in [
            ("a ==x==", 7),
            ("a", 1),
            ("a ===", 5),
            ("`==", 3),
            ("==x ==", 6),
        ] {
            assert!(suggestions(line, col).is_none(), "{line}");
        }
        assert!(suggestions("==x== ==", 8).is_some(), "a second one");
    }

    #[test]
    fn a_highlight_changes_color() {
        let line = "a ==🔴hot== b ==cold==";
        assert_eq!(
            recolor(line, 5, Some("🟢")).unwrap(),
            "a ==🟢hot== b ==cold=="
        );
        assert_eq!(recolor(line, 5, None).unwrap(), "a ==hot== b ==cold==");
        assert_eq!(
            recolor(line, 17, Some("🔵")).unwrap(),
            "a ==🔴hot== b ==🔵cold=="
        );
        assert_eq!(recolor(line, 0, Some("🔵")), None);
    }
}
