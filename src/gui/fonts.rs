//! The window's fonts: the font family set in the Window settings (else a
//! system default: DejaVu Sans Mono, Noto Sans Mono, Adwaita Mono,
//! Liberation Mono, Menlo, Consolas; else egui's Hack),
//! with its bold, italic and bold italic faces where they're there; then
//! egui's own fonts and emoji, then the system's fonts for the symbols
//! the others haven't got (☐ ✎ ⧉ ∑ …). Fonts are looked for in the usual
//! font folders of Linux, macOS and Windows.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;

/// Fallback fonts by file name, best first.
const WANTED: [&str; 12] = [
    "DejaVuSansMono.ttf",
    "DejaVuSans.ttf",
    "NotoSansMath-Regular.ttf",
    "NotoSansSymbols2-Regular.ttf",
    "NotoSansSymbols-Regular.ttf",
    "Symbola.ttf",
    "Menlo.ttc",
    "Apple Symbols.ttf",
    "STIXTwoMath.otf",
    "consola.ttf",
    "seguisym.ttf",
    "cambria.ttc",
];

/// The folders fonts are kept in.
fn folders() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/System/Library/Fonts",
        "/Library/Fonts",
        "C:\\Windows\\Fonts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    // Windows: fonts installed for the user only.
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Microsoft\\Windows\\Fonts"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.extend([
            home.join(".local/share/fonts"),
            home.join(".fonts"),
            home.join("Library/Fonts"),
        ]);
    }
    dirs
}

/// The files under `dir` (a few levels deep).
fn files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                files(&path, depth - 1, out);
            }
        } else {
            out.push(path);
        }
    }
}

/// Of `found`, the fallbacks to use: for each wanted name, its first file.
pub fn choose(found: &[PathBuf]) -> Vec<PathBuf> {
    WANTED
        .iter()
        .filter_map(|name| {
            found
                .iter()
                .find(|p| {
                    p.file_name()
                        .is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name))
                })
                .cloned()
        })
        .collect()
}

/// A font family's faces: regular, bold, italic, bold italic (a file, and
/// its index in a collection).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Family {
    pub faces: [Option<(PathBuf, u32)>; 4],
}

/// The system families tried when none is set, in order.
const DEFAULTS: [&str; 6] = [
    "DejaVuSansMono",
    "NotoSansMono",
    "AdwaitaMono",
    "LiberationMono",
    "Menlo",
    "consola",
];

/// Family `name`'s faces among `found`: `Name-Regular` (or `Name`),
/// `-Bold`, `-Italic` / `-Oblique`, `-BoldItalic` / `-BoldOblique`;
/// Windows' Consolas (`consola`, `consolab`, `consolai`, `consolaz`) and
/// a collection holding all four (`Menlo.ttc`) too.
pub fn family(name: &str, found: &[PathBuf]) -> Family {
    let file = |stems: &[String]| {
        found.iter().find_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().to_lowercase();
            let ext = p.extension()?.to_string_lossy().to_lowercase();
            (matches!(ext.as_str(), "ttf" | "otf" | "ttc")
                && stems.iter().any(|s| s.eq_ignore_ascii_case(&stem)))
            .then(|| p.clone())
        })
    };
    let n = name.trim();
    let suffixed =
        |ends: &[&str]| -> Vec<String> { ends.iter().map(|e| format!("{n}{e}")).collect() };
    let mut faces: [Option<(PathBuf, u32)>; 4] = [
        file(&suffixed(&["-regular", "", "regular"])).map(|p| (p, 0)),
        file(&suffixed(&["-bold", "bold", "b"])).map(|p| (p, 0)),
        file(&suffixed(&["-italic", "-oblique", "italic", "i"])).map(|p| (p, 0)),
        file(&suffixed(&[
            "-bolditalic",
            "-boldoblique",
            "bolditalic",
            "z",
        ]))
        .map(|p| (p, 0)),
    ];
    // A collection: its faces in that order.
    if let Some((path, _)) = &faces[0]
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ttc"))
    {
        let path = path.clone();
        for (i, face) in faces.iter_mut().enumerate().skip(1) {
            face.get_or_insert((path.clone(), i as u32));
        }
    }
    Family { faces }
}

/// The first default family `found` has (none: egui's Hack is used).
pub fn default_family(found: &[PathBuf]) -> Family {
    DEFAULTS
        .iter()
        .map(|n| family(n, found))
        .find(|f| f.faces[0].is_some())
        .unwrap_or_default()
}

/// The fonts installed: which styles have faces of their own, and the
/// family asked for if it wasn't found (the default is used).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Styles {
    pub bold: bool,
    pub italic: bool,
    pub missing: String,
}

/// The family names of the styles' fonts.
pub const BOLD: &str = "bg-bold";
pub const ITALIC: &str = "bg-italic";
pub const BOLD_ITALIC: &str = "bg-bold-italic";

/// The fonts in the system's font folders.
pub fn found() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in folders() {
        files(&dir, 3, &mut found);
    }
    found
}

/// Sets egui's fonts: `name`'s family (empty: the first default found)
/// first, then egui's, then the fallbacks; what styles it has faces for.
pub fn install(ctx: &egui::Context, name: &str, found: &[PathBuf]) -> Styles {
    let chosen = if name.trim().is_empty() {
        default_family(found)
    } else {
        family(name, found)
    };
    let mut fonts = egui::FontDefinitions::default();
    let base: Vec<String> = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let mut add = |path: &Path, index: u32| -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let key = format!("{}#{index}", path.display());
        let mut data = egui::FontData::from_owned(bytes);
        data.index = index;
        fonts.font_data.insert(key.clone(), Arc::new(data));
        Some(key)
    };
    let mut faces: [Option<String>; 4] = Default::default();
    for (i, face) in chosen.faces.iter().enumerate() {
        if let Some((path, index)) = face {
            faces[i] = add(path, *index);
        }
    }
    let fallbacks: Vec<String> = choose(found).iter().filter_map(|p| add(p, 0)).collect();
    // Each style: its face, the regular one, egui's, the fallbacks.
    let list = |own: &Option<String>| -> Vec<String> {
        own.iter()
            .chain(faces[0].iter())
            .cloned()
            .chain(base.iter().cloned())
            .chain(fallbacks.iter().cloned())
            .collect()
    };
    fonts
        .families
        .insert(egui::FontFamily::Monospace, list(&None));
    for (name, face) in [(BOLD, 1), (ITALIC, 2), (BOLD_ITALIC, 3)] {
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), list(&faces[face]));
    }
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.extend(fallbacks.iter().cloned());
    }
    ctx.set_fonts(fonts);
    Styles {
        bold: faces[1].is_some(),
        italic: faces[2].is_some(),
        missing: if name.trim().is_empty() || chosen.faces[0].is_some() {
            String::new()
        } else {
            name.trim().to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_familys_faces() {
        let found: Vec<PathBuf> = [
            "/f/JetBrainsMono-Regular.ttf",
            "/f/JetBrainsMono-Bold.ttf",
            "/f/JetBrainsMono-Italic.ttf",
            "/f/JetBrainsMono-BoldItalic.ttf",
            "/f/DejaVuSansMono.ttf",
            "/f/DejaVuSansMono-Bold.ttf",
            "/f/DejaVuSansMono-Oblique.ttf",
            "/w/consola.ttf",
            "/w/consolab.ttf",
            "/m/Menlo.ttc",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let names = |f: Family| -> Vec<Option<(String, u32)>> {
            f.faces
                .iter()
                .map(|x| {
                    x.as_ref()
                        .map(|(p, i)| (p.file_name().unwrap().to_string_lossy().into_owned(), *i))
                })
                .collect()
        };
        let some = |s: &str, i: u32| Some((s.to_string(), i));
        assert_eq!(
            names(family("jetbrainsmono", &found)),
            [
                some("JetBrainsMono-Regular.ttf", 0),
                some("JetBrainsMono-Bold.ttf", 0),
                some("JetBrainsMono-Italic.ttf", 0),
                some("JetBrainsMono-BoldItalic.ttf", 0)
            ]
        );
        assert_eq!(
            names(family("DejaVuSansMono", &found)),
            [
                some("DejaVuSansMono.ttf", 0),
                some("DejaVuSansMono-Bold.ttf", 0),
                some("DejaVuSansMono-Oblique.ttf", 0),
                None
            ]
        );
        assert_eq!(
            names(family("consola", &found))[..2],
            [some("consola.ttf", 0), some("consolab.ttf", 0)]
        );
        assert_eq!(
            names(family("Menlo", &found)),
            [
                some("Menlo.ttc", 0),
                some("Menlo.ttc", 1),
                some("Menlo.ttc", 2),
                some("Menlo.ttc", 3)
            ]
        );
        assert!(family("Nope", &found).faces.iter().all(Option::is_none));
    }

    #[test]
    fn the_default_is_the_first_common_family_there() {
        let found: Vec<PathBuf> = [
            "/f/LiberationMono-Regular.ttf",
            "/f/LiberationMono-Bold.ttf",
            "/f/NotoSansMono-Regular.ttf",
            "/f/NotoSansMono-Bold.ttf",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let chosen = default_family(&found);
        assert_eq!(
            chosen.faces[1].as_ref().map(|(p, _)| p.clone()),
            Some(PathBuf::from("/f/NotoSansMono-Bold.ttf")),
            "Noto Sans Mono before Liberation Mono"
        );
        assert_eq!(default_family(&[]), Family::default());
    }

    #[test]
    fn a_font_that_isnt_there_is_said() {
        let ctx = egui::Context::default();
        assert!(!install(&ctx, "NoSuchFontFamily", &[]).missing.is_empty());
        assert!(
            install(&ctx, "", &[]).missing.is_empty(),
            "the default asked for nothing"
        );
    }

    #[test]
    fn fallbacks_in_their_order_each_once() {
        let found: Vec<PathBuf> = [
            "/usr/share/fonts/noto/NotoSansMath-Regular.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/other/DejaVuSans.ttf",
            "/usr/share/fonts/x/Random.ttf",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            choose(&found),
            [
                PathBuf::from("/usr/share/fonts/dejavu/DejaVuSans.ttf"),
                PathBuf::from("/usr/share/fonts/noto/NotoSansMath-Regular.ttf"),
            ]
        );
    }
}
