//! The editor's images in the window: the editor draws them as iTerm2
//! images (a cell holding the PNG, base64, and its size in pixels; the
//! window gives it an iTerm2 picker sized to its cells), and the window
//! decodes each once into a texture ([`Textures`]) and paints it over the
//! cells it covers.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use base64::Engine;
use eframe::egui;

/// An image cell's PNG and its size in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The image in an iTerm2 image's symbol (`ESC ] 1337 ; File=…:BASE64 BEL`).
pub fn parse(symbol: &str) -> Option<Payload> {
    let start = symbol.find("]1337;File=")? + "]1337;File=".len();
    let rest = &symbol[start..];
    let colon = rest.find(':')?;
    let (params, data) = (&rest[..colon], &rest[colon + 1..]);
    let end = data.find(['\x07', '\x1b']).unwrap_or(data.len());
    let png = base64::engine::general_purpose::STANDARD
        .decode(data[..end].trim())
        .ok()?;
    let px = |key: &str| {
        params.split(';').find_map(|p| {
            p.strip_prefix(key)?
                .strip_prefix('=')?
                .trim_end_matches("px")
                .parse::<u32>()
                .ok()
        })
    };
    Some(Payload {
        png,
        width: px("width")?,
        height: px("height")?,
    })
}

/// The images decoded, by their cell's symbol; dropped once not drawn.
#[derive(Default)]
pub struct Textures {
    textures: HashMap<u64, Option<(egui::TextureHandle, [u32; 2])>>,
    drawn: HashSet<u64>,
}

impl Textures {
    /// The texture of the image in `symbol` and its size in pixels
    /// (decoded the first time); `None` if it can't be read.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        symbol: &str,
    ) -> Option<(egui::TextureId, [u32; 2])> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        symbol.hash(&mut hasher);
        let key = hasher.finish();
        self.drawn.insert(key);
        let entry = self.textures.entry(key).or_insert_with(|| {
            let payload = parse(symbol)?;
            let decoded =
                image::load_from_memory_with_format(&payload.png, image::ImageFormat::Png)
                    .ok()?
                    .to_rgba8();
            let size = [decoded.width() as usize, decoded.height() as usize];
            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
            let texture =
                ctx.load_texture(format!("image-{key}"), pixels, egui::TextureOptions::LINEAR);
            Some((texture, [payload.width, payload.height]))
        });
        entry.as_ref().map(|(t, size)| (t.id(), *size))
    }

    /// The frame is drawn: the images it didn't draw are let go.
    pub fn sweep(&mut self) {
        let drawn = std::mem::take(&mut self.drawn);
        self.textures.retain(|k, _| drawn.contains(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_iterm2_image_is_read() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let data = base64::engine::general_purpose::STANDARD.encode(&png);
        let symbol = format!(
            "\x1b[Kclear\x1b]1337;File=inline=1;size={};width=20px;height=10px;doNotMoveCursor=1:{data}\x07",
            png.len()
        );
        let payload = parse(&symbol).expect("an image");
        assert_eq!((payload.width, payload.height), (20, 10));
        assert_eq!(payload.png, png);
        assert!(parse("plain text").is_none());
    }
}
