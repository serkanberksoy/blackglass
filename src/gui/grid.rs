//! The window's screen: a ratatui backend that keeps the cells drawn
//! ([`Grid`]), and their pieces to paint ([`runs`]): a run of cells with
//! the same colors and style is painted as one text, a wide character
//! (an emoji, CJK) on its own, so every piece starts at its own column
//! and the grid stays aligned whatever the font's widths.

use std::convert::Infallible;

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

/// The cells drawn, as a terminal would show them.
#[derive(Debug, Clone)]
pub struct Grid {
    pub buffer: Buffer,
    pub cursor: Position,
    pub cursor_shown: bool,
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self {
        Grid {
            buffer: Buffer::empty(Rect::new(0, 0, width, height)),
            cursor: Position::ORIGIN,
            cursor_shown: false,
        }
    }

    /// The window's size changed: `width` × `height` cells.
    pub fn resize(&mut self, width: u16, height: u16) {
        if self.buffer.area.width != width || self.buffer.area.height != height {
            self.buffer.resize(Rect::new(0, 0, width, height));
            self.buffer.reset();
        }
    }
}

impl Backend for Grid {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Infallible>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let area = self.buffer.area;
        for (x, y, cell) in content {
            if x < area.width && y < area.height {
                self.buffer[(x, y)] = cell.clone();
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Infallible> {
        self.cursor_shown = false;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Infallible> {
        self.cursor_shown = true;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Infallible> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Infallible> {
        self.cursor = position.into();
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Infallible> {
        self.buffer.reset();
        Ok(())
    }

    fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Infallible> {
        self.buffer.reset();
        Ok(())
    }

    fn size(&self) -> Result<Size, Infallible> {
        Ok(self.buffer.area.as_size())
    }

    fn window_size(&mut self) -> Result<WindowSize, Infallible> {
        Ok(WindowSize {
            columns_rows: self.buffer.area.as_size(),
            pixels: Size::default(),
        })
    }

    fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

/// A piece of a row to paint: its first column, how many columns it
/// covers, its text and colors and style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub x: u16,
    pub width: u16,
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub modifier: Modifier,
    /// An image (an iTerm2 image in `text`; its area's other cells are
    /// under it).
    pub image: bool,
}

/// Whether a cell's symbol is an iTerm2 image.
pub fn is_image(symbol: &str) -> bool {
    symbol.contains("\x1b]1337;File=")
}

/// Row `y`'s pieces, left to right: cells with the same colors and style
/// together; a wide character alone, the cells it covers skipped.
pub fn runs(buffer: &Buffer, y: u16) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    let mut x = 0;
    let width = buffer.area.width;
    while x < width {
        let cell = &buffer[(x, y)];
        let symbol = cell.symbol();
        let image = is_image(symbol);
        let w = if image {
            1
        } else {
            (symbol.width() as u16).clamp(1, width - x)
        };
        let same = |r: &Run| {
            r.fg == cell.fg
                && r.bg == cell.bg
                && r.modifier == cell.modifier
                && r.x + r.width == x
                && r.text.chars().count() == r.width as usize
                && r.text.width() == r.width as usize
        };
        match out.last_mut() {
            Some(run) if w == 1 && !image && !run.image && same(run) => {
                run.text.push_str(symbol);
                run.width += 1;
            }
            _ => out.push(Run {
                x,
                width: w,
                text: symbol.to_string(),
                fg: cell.fg,
                bg: cell.bg,
                modifier: cell.modifier,
                image,
            }),
        }
        x += w;
    }
    out
}

/// A terminal color as RGB: named colors as a dark terminal shows them,
/// the 256 indexed ones as xterm's, `Reset` as `default`.
pub fn rgb(color: Color, default: [u8; 3]) -> [u8; 3] {
    const NAMED: [[u8; 3]; 16] = [
        [0x1e, 0x1e, 0x1e],
        [0xcd, 0x31, 0x31],
        [0x0d, 0xbc, 0x79],
        [0xe5, 0xe5, 0x10],
        [0x24, 0x72, 0xc8],
        [0xbc, 0x3f, 0xbc],
        [0x11, 0xa8, 0xcd],
        [0xcc, 0xcc, 0xcc],
        [0x66, 0x66, 0x66],
        [0xf1, 0x4c, 0x4c],
        [0x23, 0xd1, 0x8b],
        [0xf5, 0xf5, 0x43],
        [0x3b, 0x8e, 0xea],
        [0xd6, 0x70, 0xd6],
        [0x29, 0xb8, 0xdb],
        [0xf2, 0xf2, 0xf2],
    ];
    let indexed = |i: u8| match i {
        0..=15 => NAMED[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [level(i / 36), level(i / 6 % 6), level(i % 6)]
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            [v, v, v]
        }
    };
    match color {
        Color::Reset => default,
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(i) => indexed(i),
        Color::Black => NAMED[0],
        Color::Red => NAMED[1],
        Color::Green => NAMED[2],
        Color::Yellow => NAMED[3],
        Color::Blue => NAMED[4],
        Color::Magenta => NAMED[5],
        Color::Cyan => NAMED[6],
        Color::Gray => NAMED[7],
        Color::DarkGray => NAMED[8],
        Color::LightRed => NAMED[9],
        Color::LightGreen => NAMED[10],
        Color::LightYellow => NAMED[11],
        Color::LightBlue => NAMED[12],
        Color::LightMagenta => NAMED[13],
        Color::LightCyan => NAMED[14],
        Color::White => NAMED[15],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::style::Style;
    use ratatui::text::Line;

    #[test]
    fn the_grid_is_a_backend_that_keeps_the_cells() {
        let mut terminal = Terminal::new(Grid::new(20, 2)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(Line::from("hello"), Rect::new(0, 0, 20, 1));
                f.set_cursor_position((3, 1));
            })
            .unwrap();
        let grid = terminal.backend();
        assert_eq!(grid.buffer[(1, 0)].symbol(), "e");
        assert_eq!(grid.cursor, Position::new(3, 1));
        assert!(grid.cursor_shown);
        terminal.backend_mut().resize(30, 3);
        assert_eq!(terminal.size().unwrap(), Size::new(30, 3));
    }

    #[test]
    fn rows_are_runs_of_one_style_and_wide_characters_alone() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 1));
        buffer.set_string(0, 0, "ab", Style::new());
        buffer.set_string(2, 0, "cd", Style::new().fg(Color::Red));
        buffer.set_string(4, 0, "😀x", Style::new().fg(Color::Red));
        let row = runs(&buffer, 0);
        let pieces: Vec<(u16, u16, &str)> = row
            .iter()
            .map(|r| (r.x, r.width, r.text.as_str()))
            .collect();
        assert_eq!(
            pieces,
            [
                (0, 2, "ab"),
                (2, 2, "cd"),
                (4, 2, "😀"),
                (6, 1, "x"),
                (7, 5, "     ")
            ]
        );
        assert_eq!(row[1].fg, Color::Red);
    }

    #[test]
    fn an_image_is_a_run_of_its_own() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
        buffer.set_string(0, 0, "ab", Style::new());
        buffer[(2, 0)].set_symbol("\x1b]1337;File=inline=1;width=20px;height=10px:QUJD\x07");
        buffer.set_string(3, 0, "cd", Style::new());
        let row = runs(&buffer, 0);
        assert_eq!(row.len(), 3);
        assert!(row[1].image && !row[0].image && !row[2].image);
        assert_eq!((row[1].x, row[1].width, row[2].x), (2, 1, 3));
        assert_eq!(row[2].text, "cd     ");
    }

    #[test]
    fn colors_as_rgb() {
        assert_eq!(rgb(Color::Rgb(1, 2, 3), [0; 3]), [1, 2, 3]);
        assert_eq!(rgb(Color::Reset, [9; 3]), [9; 3]);
        assert_eq!(rgb(Color::Indexed(196), [0; 3]), [255, 0, 0]);
        assert_eq!(rgb(Color::Indexed(232), [0; 3]), [8, 8, 8]);
        assert_eq!(
            rgb(Color::LightBlue, [0; 3]),
            rgb(Color::Indexed(12), [0; 3])
        );
    }
}
