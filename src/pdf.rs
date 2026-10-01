//! A PDF's text, for "PDF to note" (W-105): the text of every page, in
//! reading order as the PDF stores it, tidied into paragraphs.

use std::path::Path;

/// The text of the PDF at `path`, its lines trimmed and runs of blank
/// lines made one.
pub fn text(path: &Path) -> Result<String, String> {
    let name = path.display().to_string();
    // A damaged PDF can make the reader panic: that's an error here.
    let read = std::panic::catch_unwind(|| pdf_extract::extract_text(path));
    let raw = match read {
        Ok(Ok(raw)) => raw,
        Ok(Err(e)) => return Err(format!("Cannot read {name}: {e}")),
        Err(_) => return Err(format!("Cannot read {name}: it looks damaged")),
    };
    let mut out = String::new();
    let mut blank = 0;
    for line in raw.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        blank = 0;
        out.push_str(line.trim_start());
    }
    Ok(out)
}

/// A small one-page PDF showing `lines` (Helvetica), with a correct
/// cross-reference table: for tests and examples.
pub fn sample(lines: &[&str]) -> Vec<u8> {
    let mut content = String::from("BT /F1 12 Tf 72 720 Td 14 TL\n");
    for line in lines {
        content.push_str(&format!("({line}) Tj T*\n"));
    }
    content.push_str("ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pdfs_text_in_paragraphs() {
        let dir = crate::vault::tests::scratch("pdf-text");
        let file = dir.join("paper.pdf");
        std::fs::write(&file, sample(&["Hello PDF", "Second line"])).unwrap();
        let got = text(&file).unwrap();
        assert!(got.contains("Hello PDF"), "{got:?}");
        assert!(got.contains("Second line"), "{got:?}");
        assert!(!got.contains("\n\n\n"), "tidied: {got:?}");
        let bad = dir.join("bad.pdf");
        std::fs::write(&bad, "not a pdf").unwrap();
        assert!(text(&bad).is_err());
    }
}
