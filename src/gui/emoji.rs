//! Color emoji in the window: egui draws one-color glyphs only, so an
//! emoji is shaped (swash: joined emoji and flags are one glyph) and
//! drawn (vello_cpu: COLR and bitmap color fonts) from the system's color
//! emoji font, once per size, into a texture painted in its cells. Without
//! such a font, emoji stay text.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use vello_cpu::peniko::{Blob, FontData};

/// Color emoji fonts by file name, best first: Linux, macOS, Windows.
const FONTS: [&str; 4] = [
    "Noto-COLRv1.ttf",
    "NotoColorEmoji.ttf",
    "Apple Color Emoji.ttc",
    "seguiemj.ttf",
];

/// The drawing's size, in em: room for an emoji's width and its
/// ascent and descent, its baseline at [`BASELINE`].
const WIDTH: f32 = 1.3;
const HEIGHT: f32 = 1.25;
const BASELINE: f32 = 0.95;

/// Whether `text` (a wide character) is an emoji.
pub fn is_emoji(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c as u32,
            0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0xFE0F | 0x200D
        )
    })
}

/// The emoji drawn, by text and size.
pub struct Emoji {
    path: Option<PathBuf>,
    /// The font, read the first time it's needed (`None` inside: unreadable).
    font: Option<Option<(Arc<Vec<u8>>, FontData)>>,
    shaper: swash::shape::ShapeContext,
    resources: vello_cpu::Resources,
    textures: HashMap<(String, u32), Option<egui::TextureHandle>>,
    drawn: HashSet<(String, u32)>,
}

impl Emoji {
    /// The emoji drawn from the first color emoji font in `found`.
    pub fn new(found: &[PathBuf]) -> Self {
        let path = FONTS.iter().find_map(|name| {
            found
                .iter()
                .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy() == *name))
                .cloned()
        });
        Emoji {
            path,
            font: None,
            shaper: swash::shape::ShapeContext::new(),
            resources: vello_cpu::Resources::new(),
            textures: HashMap::new(),
            drawn: HashSet::new(),
        }
    }

    /// `text` drawn `px` pixels high (an em): RGBA pixels, `None` without a
    /// color emoji font or its glyph.
    pub fn draw(&mut self, text: &str, px: u32) -> Option<egui::ColorImage> {
        let path = self.path.clone()?;
        let font = self.font.get_or_insert_with(|| {
            let bytes = Arc::new(std::fs::read(&path).ok()?);
            let data = FontData::new(Blob::new(bytes.clone()), 0);
            Some((bytes, data))
        });
        let (bytes, data) = font.as_ref()?;
        let size = px as f32;
        let font_ref = swash::FontRef::from_index(bytes, 0)?;
        let mut shaper = self.shaper.builder(font_ref).size(size).build();
        shaper.add_str(text);
        let mut glyphs = Vec::new();
        let mut x = 0.0;
        shaper.shape_with(|cluster| {
            for g in cluster.glyphs {
                glyphs.push(vello_cpu::Glyph {
                    id: u32::from(g.id),
                    x: x + g.x,
                    y: size * BASELINE - g.y,
                });
                x += g.advance;
            }
        });
        // A glyph the font hasn't got (0) isn't an emoji it can draw.
        if glyphs.is_empty() || glyphs.iter().any(|g| g.id == 0) {
            return None;
        }
        let (w, h) = ((size * WIDTH).ceil() as u16, (size * HEIGHT).ceil() as u16);
        let mut ctx = vello_cpu::RenderContext::new(w, h);
        ctx.glyph_run(&mut self.resources, data)
            .font_size(size)
            .fill_glyphs(glyphs.into_iter())
            .ok()?;
        ctx.flush();
        let mut pixmap = vello_cpu::Pixmap::new(w, h);
        ctx.render(&mut pixmap, &mut self.resources);
        let rgba = pixmap.take_rgba8(vello_cpu::peniko::ImageAlphaType::Alpha);
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [w as usize, h as usize],
            &rgba,
        ))
    }

    /// `text`'s texture at `px` (drawn the first time); `None`: as text.
    pub fn get(&mut self, ctx: &egui::Context, text: &str, px: u32) -> Option<egui::TextureId> {
        let key = (text.to_string(), px);
        self.drawn.insert(key.clone());
        if !self.textures.contains_key(&key) {
            let texture = self.draw(text, px).map(|image| {
                ctx.load_texture(
                    format!("emoji-{text}-{px}"),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            });
            self.textures.insert(key.clone(), texture);
        }
        self.textures[&key].as_ref().map(egui::TextureHandle::id)
    }

    /// The frame is drawn: the emoji it didn't draw are let go.
    pub fn sweep(&mut self) {
        let drawn = std::mem::take(&mut self.drawn);
        self.textures.retain(|k, _| drawn.contains(k));
    }
}

/// The emoji's box in a cell row `height` high: its drawing's shape.
pub fn size(height: f32) -> egui::Vec2 {
    egui::vec2(height * WIDTH / HEIGHT, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_are_told_from_other_wide_characters() {
        for e in ["😀", "👩‍💻", "🇹🇷", "☕", "❤️"] {
            assert!(is_emoji(e), "{e}");
        }
        for t in ["漢", "가", "Ａ"] {
            assert!(!is_emoji(t), "{t}");
        }
    }

    #[test]
    fn an_emoji_is_drawn_in_color_when_the_system_has_the_font() {
        let mut emoji = Emoji::new(&crate::gui::fonts::found());
        if emoji.path.is_none() {
            // No color emoji font here: emoji stay text.
            assert!(emoji.draw("😀", 32).is_none());
            return;
        }
        let image = emoji.draw("😀", 32).expect("drawn");
        assert_eq!(image.size, [42, 40]);
        let colored = image
            .pixels
            .iter()
            .filter(|p| p.a() > 200 && p.r() > 200 && p.g() > 150 && p.b() < 100)
            .count();
        assert!(colored > 100, "yellow: {colored}");
        assert!(emoji.draw("x", 32).is_none(), "not in the emoji font");
    }
}
