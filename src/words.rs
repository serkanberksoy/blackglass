//! Word count for the status bar: the note's words and characters (its
//! properties left out), or the selection's.

/// The words (runs of non-space with a letter or digit; `-` and `|` alone
/// aren't words) and characters (line breaks left out) of `text`, without
/// its properties (the frontmatter at the top).
pub fn count(text: &str) -> (usize, usize) {
    let body = without_frontmatter(text);
    let words = body
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count();
    let chars = body.chars().filter(|&c| c != '\n' && c != '\r').count();
    (words, chars)
}

/// [`count`] of the note's lines, without joining them into one text
/// (the status bar counts on every frame).
pub fn count_lines(lines: &[String]) -> (usize, usize) {
    let body = match lines.first().map(|l| l.trim_end()) {
        Some("---") => match lines[1..].iter().position(|l| l.trim_end() == "---") {
            Some(end) => &lines[end + 2..],
            None => lines,
        },
        _ => lines,
    };
    body.iter().fold((0, 0), |(words, chars), line| {
        let w = line
            .split_whitespace()
            .filter(|w| w.chars().any(char::is_alphanumeric))
            .count();
        let c = line.chars().filter(|&c| c != '\r').count();
        (words + w, chars + c)
    })
}

/// `text` after a closed `---` frontmatter block at its top.
fn without_frontmatter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return &rest[at + line.len()..];
        }
        at += line.len();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_count_like_the_text() {
        for text in [
            "---\ntags: [a]\n---\n# A title\n- item one\n\nIt's 2 words? | x |",
            "no frontmatter\nhere",
            "---\nnot closed\nstill",
            "",
        ] {
            let lines: Vec<String> = text.lines().map(String::from).collect();
            assert_eq!(count_lines(&lines), count(text), "{text:?}");
        }
    }

    #[test]
    fn words_and_characters_without_the_properties() {
        let text = "---\ntags: [a]\n---\n# A title\n- item one\n\nIt's 2 words? | x |";
        assert_eq!(
            count(text),
            (8, 38),
            "words have a letter or digit; chars without line breaks"
        );
        assert_eq!(count(""), (0, 0));
        assert_eq!(count("---\nopen frontmatter"), (2, 19), "not closed: text");
    }
}
