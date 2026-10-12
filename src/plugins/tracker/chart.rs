//! Tracker's charts as text: a line chart in braille (two by four dots a
//! cell), bars of eighth blocks, a pie of half blocks (two round pixels a
//! cell) with its legend, and a bullet chart's bands, value and marker.
//! Axes have their values on the left (and the right for datasets there)
//! and the dates under them. A click on a chart's column says its day's
//! values (`plugin:tracker:say:…`), as hovering does in the original.

use chrono::NaiveDate;
use mdedit::processor::CellRow;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::collect::{Data, add_months, format_date};
use super::expr;
use super::params::Opts;
use crate::ui::theme::{self, ACCENT, FAINT, MUTED, TEXT};

/// A chart's rows, with actions on parts of them.
pub type Rows = Vec<CellRow>;

/// Rows without actions.
pub fn plain(lines: Vec<Line<'static>>) -> Rows {
    lines.into_iter().map(|l| (l, Vec::new())).collect()
}

/// A color written in a block: a name (`red`, `steelblue` …) or `#rrggbb`.
pub fn color(text: &str) -> Option<Color> {
    let t = text.trim().trim_matches(['\'', '"']).to_lowercase();
    if let Some(hex) = t.strip_prefix('#') {
        let hex = if hex.len() == 3 {
            hex.chars().flat_map(|c| [c, c]).collect()
        } else {
            hex.to_string()
        };
        let n = u32::from_str_radix(&hex, 16)
            .ok()
            .filter(|_| hex.len() == 6)?;
        return Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
    }
    Some(match t.as_str() {
        "black" => Color::Black,
        "white" | "snow" | "ivory" => Color::White,
        "red" | "crimson" | "firebrick" | "tomato" => Color::Red,
        "green" | "lime" | "forestgreen" | "seagreen" | "olive" => Color::Green,
        "yellow" | "gold" | "orange" | "khaki" => Color::Yellow,
        "blue" | "navy" | "royalblue" | "steelblue" | "dodgerblue" => Color::Blue,
        "magenta" | "fuchsia" | "purple" | "violet" | "orchid" | "pink" => Color::Magenta,
        "cyan" | "aqua" | "teal" | "turquoise" | "skyblue" => Color::Cyan,
        "gray" | "grey" | "silver" | "darkgray" | "darkgrey" => Color::Gray,
        _ => return None,
    })
}

/// Dataset `i`'s color: `key`'s (one for all, or one each), else one of
/// the theme's accent and the named colors in turn.
pub fn dataset_color(opts: &Opts, key: &str, i: usize) -> Color {
    opts.nth(key, i)
        .and_then(color)
        .unwrap_or_else(|| default_color(i))
}

pub fn default_color(i: usize) -> Color {
    const OTHERS: [Color; 6] = [
        Color::Green,
        Color::Yellow,
        Color::Magenta,
        Color::Cyan,
        Color::Red,
        Color::Blue,
    ];
    match i {
        0 => theme::paint(ACCENT),
        i => OTHERS[(i - 1) % OTHERS.len()],
    }
}

/// A summary's `style` (CSS): `color`, `background-color`,
/// `font-weight: bold`, `font-style: italic`, `text-decoration`
/// (`underline`, `line-through`); the rest is for a web page.
pub fn css(style: &str) -> Style {
    let mut out = Style::default().fg(theme::paint(TEXT));
    for rule in style.split(';') {
        let Some((key, value)) = rule.split_once(':') else {
            continue;
        };
        let value = value.trim().to_lowercase();
        match key.trim().to_lowercase().as_str() {
            "color" => {
                if let Some(c) = color(&value) {
                    out = out.fg(c);
                }
            }
            "background-color" | "background" => {
                if let Some(c) = color(&value) {
                    out = out.bg(c);
                }
            }
            "font-weight" if value == "bold" || value.parse::<u32>().is_ok_and(|w| w >= 600) => {
                out = out.add_modifier(Modifier::BOLD);
            }
            "font-style" if value == "italic" || value == "oblique" => {
                out = out.add_modifier(Modifier::ITALIC);
            }
            "text-decoration" | "text-decoration-line" => {
                if value.contains("underline") {
                    out = out.add_modifier(Modifier::UNDERLINED);
                }
                if value.contains("line-through") {
                    out = out.add_modifier(Modifier::CROSSED_OUT);
                }
            }
            _ => {}
        }
    }
    out
}

fn text_style() -> Style {
    Style::default().fg(theme::paint(TEXT))
}

fn muted() -> Style {
    Style::default().fg(theme::paint(MUTED))
}

fn width_of(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn line_width(line: &Line) -> usize {
    line.spans.iter().map(|s| width_of(&s.content)).sum()
}

/// A title line, bold and centered in `width`.
pub fn title(text: &str, width: usize) -> Line<'static> {
    let pad = width.saturating_sub(width_of(text)) / 2;
    Line::from(vec![
        Span::raw(" ".repeat(pad)),
        Span::styled(text.to_string(), text_style().add_modifier(Modifier::BOLD)),
    ])
}

/// How many rows a chart's plot has, for `width` columns.
fn plot_rows(opts: &Opts, root: &Opts, width: usize) -> usize {
    let scale = root.num("fixedScale").unwrap_or(1.0).clamp(0.25, 4.0);
    let ratio = root
        .str("aspectRatio")
        .and_then(|r| {
            let (w, h) = r.split_once(':')?;
            Some(w.trim().parse::<f64>().ok()? / h.trim().parse::<f64>().ok()?)
        })
        .filter(|r| *r > 0.0)
        .unwrap_or(1.0);
    let base = opts
        .num("height")
        .unwrap_or((width as f64 / 7.0).clamp(6.0, 14.0) / ratio.sqrt());
    (base * scale).round().clamp(3.0, 40.0) as usize
}

/// The `i`th of a per-axis list (`yMin: 0, 10`): left 0, right 1.
fn per_side(opts: &Opts, key: &str, side: usize) -> Option<String> {
    let list = opts.list(key);
    list.get(side)
        .or(list.first().filter(|_| side == 0))
        .cloned()
        .filter(|v| !v.trim().is_empty())
}

/// A y axis: its range and how its values read.
struct Axis {
    lo: f64,
    hi: f64,
    reverse: bool,
    unit: String,
    /// `yAxisTickInterval`, `yAxisTickLabelFormat`, `yAxisColor`.
    interval: Option<f64>,
    format: Option<String>,
    color: Option<Color>,
}

impl Axis {
    fn new(values: &[f64], opts: &Opts, side: usize, from_zero: bool) -> Axis {
        let pick = |key: &str| per_side(opts, key, side).and_then(|v| v.trim().parse::<f64>().ok());
        let mut lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let mut hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if from_zero {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        if let Some(v) = pick("yMin") {
            lo = v;
        }
        if let Some(v) = pick("yMax") {
            hi = v;
        }
        if !lo.is_finite() || !hi.is_finite() {
            (lo, hi) = (0.0, 1.0);
        }
        if hi <= lo {
            (lo, hi) = (lo - 1.0, lo + 1.0);
        }
        Axis {
            lo,
            hi,
            reverse: per_side(opts, "reverseYAxis", side).is_some_and(|v| v.trim() == "true"),
            unit: opts
                .list("yAxisUnit")
                .get(side)
                .cloned()
                .unwrap_or_default(),
            interval: pick("yAxisTickInterval").filter(|i| *i > 0.0),
            format: per_side(opts, "yAxisTickLabelFormat", side),
            color: per_side(opts, "yAxisColor", side).and_then(|c| color(&c)),
        }
    }

    /// `v`'s place from the bottom, 0 … 1.
    fn frac(&self, v: f64) -> f64 {
        let f = ((v - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0);
        if self.reverse { 1.0 - f } else { f }
    }

    /// The value at `frac` from the bottom.
    fn at(&self, frac: f64) -> f64 {
        let frac = if self.reverse { 1.0 - frac } else { frac };
        self.lo + frac * (self.hi - self.lo)
    }

    fn label(&self, v: f64) -> String {
        let text = match &self.format {
            Some(f) => expr::printf(v, f),
            None => expr::plain(v),
        };
        format!("{text}{}", self.unit)
    }

    fn style(&self) -> Style {
        self.color.map_or_else(muted, |c| Style::default().fg(c))
    }

    /// The labels of `rows` rows: with a tick interval, its multiples;
    /// else the top and bottom values and round values between them (on
    /// their rows, not next to another label).
    fn labels(&self, rows: usize) -> Vec<Option<String>> {
        let mut out = vec![None; rows];
        let row_of = |v: f64| ((1.0 - self.frac(v)) * (rows - 1) as f64).round() as usize;
        if let Some(step) = self.interval {
            let first = (self.lo / step - 1e-9).ceil() as i64;
            let last = (self.hi / step + 1e-9).floor() as i64;
            for k in first..=last.min(first + 200) {
                let v = k as f64 * step;
                let row = row_of(v);
                if row < rows && out[row].is_none() {
                    out[row] = Some(self.label(v));
                }
            }
            return out;
        }
        out[0] = Some(self.label(self.at(1.0)));
        if rows > 1 {
            out[rows - 1] = Some(self.label(self.at(0.0)));
        }
        if rows < 5 {
            return out;
        }
        let step = nice((self.hi - self.lo) / (rows / 3).max(1) as f64);
        let first = (self.lo / step).ceil() as i64;
        let last = (self.hi / step).floor() as i64;
        for k in first..=last {
            let v = k as f64 * step;
            let row = row_of(v);
            let free = |r: usize| out.get(r).is_none_or(Option::is_none);
            if row > 0 && row + 1 < rows && free(row) && free(row - 1) && free(row + 1) {
                out[row] = Some(self.label(v));
            }
        }
        out
    }

    fn label_width(&self, rows: usize) -> usize {
        self.labels(rows)
            .iter()
            .flatten()
            .map(|l| width_of(l))
            .max()
            .unwrap_or(0)
    }
}

/// A round step near `x`: 1, 2 or 5 times a power of ten.
fn nice(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return 1.0;
    }
    let power = 10f64.powf(x.log10().floor());
    let f = x / power;
    let m = if f < 1.5 {
        1.0
    } else if f < 3.5 {
        2.0
    } else if f < 7.5 {
        5.0
    } else {
        10.0
    };
    m * power
}

/// Which datasets go on the right axis (`yAxisLocation: right`).
fn on_right(opts: &Opts, i: usize) -> bool {
    opts.nth("yAxisLocation", i)
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("right"))
}

/// A plot's cells (char and color), drawn by the line or bar chart.
type Cells = Vec<Vec<(char, Option<Color>)>>;

/// The days the x axis is labelled at: every `xAxisTickInterval` (`7d`,
/// `1w`, `1M`, `1y`, or days) from the first; else the first, the middle
/// and the last.
fn x_ticks(data: &Data, opts: &Opts) -> Vec<usize> {
    let Some(step) = opts
        .str("xAxisTickInterval")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        let mut ticks = vec![0];
        if data.days > 2 {
            ticks.push(data.days / 2);
        }
        if data.days > 1 {
            ticks.push(data.days - 1);
        }
        return ticks;
    };
    let digits: String = step.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().unwrap_or(1).max(1);
    let unit = step[digits.len()..].trim().to_lowercase();
    let next = |day: NaiveDate| -> Option<NaiveDate> {
        match unit.chars().next() {
            Some('w') => Some(day + chrono::Duration::weeks(n)),
            Some('m') => add_months(day, n),
            Some('y') => add_months(day, n * 12),
            _ => Some(day + chrono::Duration::days(n)),
        }
    };
    let mut ticks = Vec::new();
    let mut day = data.start;
    while day <= data.end() && ticks.len() < 500 {
        ticks.push((day - data.start).num_days() as usize);
        match next(day) {
            Some(d) => day = d,
            None => break,
        }
    }
    ticks
}

/// The plot column of day `d`.
fn column(d: usize, days: usize, plot: usize) -> usize {
    if days <= 1 {
        plot / 2
    } else {
        (d as f64 * (plot.saturating_sub(1)) as f64 / (days - 1) as f64).round() as usize
    }
}

/// What a click on a column says: its day(s) and each dataset's value.
fn inspect(data: &Data, days: std::ops::Range<usize>) -> String {
    let (first, last) = (days.start, days.end.saturating_sub(1).max(days.start));
    let when = if first == last {
        data.format(data.day(first))
    } else {
        format!(
            "{} … {}",
            data.format(data.day(first)),
            data.format(data.day(last))
        )
    };
    let values: Vec<String> = data
        .shown()
        .map(|(_, set)| {
            let vs: Vec<f64> = (first..=last)
                .filter_map(|d| set.values.get(d).copied().flatten())
                .collect();
            let v = if vs.is_empty() {
                "–".to_string()
            } else {
                expr::plain(vs.iter().sum())
            };
            format!("{} {v}", set.name)
        })
        .collect();
    format!("{when}: {}", values.join(", "))
}

/// The actions of a plot's columns, a part for each run of columns that
/// say the same.
fn parts(says: &[String], offset: usize) -> Vec<mdedit::processor::Part> {
    let mut out: Vec<mdedit::processor::Part> = Vec::new();
    for (c, say) in says.iter().enumerate() {
        let action = format!("plugin:tracker:say:{say}");
        match out.last_mut() {
            Some((_, end, a)) if *a == action && *end == offset + c => *end += 1,
            _ => out.push((offset + c, offset + c + 1, action)),
        }
    }
    out
}

/// The plot with its axes, dates and labels around it; `says`: what a
/// click on each column says (none: `allowInspectData: false`).
fn frame(
    data: &Data,
    opts: &Opts,
    width: usize,
    cells: Cells,
    axes: [Option<&Axis>; 2],
    says: Option<Vec<String>>,
) -> Rows {
    let rows = cells.len();
    let left: Vec<Option<String>> = axes[0].map_or(vec![None; rows], |a| a.labels(rows));
    let right: Vec<Option<String>> = axes[1].map_or(vec![None; rows], |a| a.labels(rows));
    let lw = axes[0].map_or(0, |a| a.label_width(rows));
    let plot = cells.first().map_or(0, Vec::len);
    let left_style = axes[0].map_or_else(muted, Axis::style);
    let right_style = axes[1].map_or_else(muted, Axis::style);
    let x_style = opts
        .str("xAxisColor")
        .and_then(color)
        .map_or_else(muted, |c| Style::default().fg(c));
    let label_style = |key: &str, side: usize| {
        per_side(opts, key, side)
            .and_then(|c| color(&c))
            .map_or_else(muted, |c| Style::default().fg(c))
    };
    let mut out: Rows = Vec::new();
    if let Some(t) = opts.str("title").filter(|t| !t.is_empty()) {
        out.push((title(t, width), Vec::new()));
    }
    let ylabels: Vec<String> = opts.list("yAxisLabel");
    if let Some(label) = ylabels.first().filter(|l| !l.is_empty()) {
        let mut spans = vec![Span::styled(
            label.clone(),
            label_style("yAxisLabelColor", 0),
        )];
        if let Some(r) = ylabels.get(1).filter(|_| axes[1].is_some()) {
            let gap = (lw + 1 + plot + 1).saturating_sub(width_of(label));
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(r.clone(), label_style("yAxisLabelColor", 1)));
        }
        out.push((Line::from(spans), Vec::new()));
    }
    let row_parts = says.map(|s| parts(&s, lw + 1)).unwrap_or_default();
    for (r, row) in cells.into_iter().enumerate() {
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("{:>lw$}", left[r].clone().unwrap_or_default()),
            left_style,
        ));
        spans.push(Span::styled(
            if left[r].is_some() {
                theme::glyph("┤", "+")
            } else {
                theme::glyph("│", "|")
            },
            left_style,
        ));
        for (c, color) in row {
            spans.push(match color {
                Some(color) => Span::styled(c.to_string(), Style::default().fg(color)),
                None => Span::raw(c.to_string()),
            });
        }
        if axes[1].is_some() {
            spans.push(Span::styled(
                if right[r].is_some() {
                    theme::glyph("├", "+")
                } else {
                    theme::glyph("│", "|")
                },
                right_style,
            ));
            spans.push(Span::styled(
                right[r].clone().unwrap_or_default(),
                right_style,
            ));
        }
        out.push((Line::from(spans), row_parts.clone()));
    }
    let corner = theme::glyph("└", "+");
    let rule = theme::glyph("─", "-").repeat(plot);
    let end = if axes[1].is_some() {
        theme::glyph("┘", "+")
    } else {
        ""
    };
    out.push((
        Line::styled(format!("{}{corner}{rule}{end}", " ".repeat(lw)), x_style),
        Vec::new(),
    ));
    // The dates at the ticks, where they fit.
    let format = opts
        .str("xAxisTickLabelFormat")
        .map(str::to_string)
        .unwrap_or_else(|| data.date_format.clone());
    let mut dates = String::new();
    let mut used = 0usize;
    let ticks = x_ticks(data, opts);
    for (k, &d) in ticks.iter().enumerate() {
        let text = format_date(data.day(d), &format);
        let w = width_of(&text);
        let at = lw + 1 + column(d, data.days, plot);
        // The first starts at its day, the last ends at its day.
        let start = if k == 0 && ticks.len() > 1 {
            at
        } else if k + 1 == ticks.len() && ticks.len() > 1 {
            (at + 1).saturating_sub(w)
        } else {
            at.saturating_sub(w / 2)
        }
        .clamp(lw + 1, (lw + 1 + plot).saturating_sub(w).max(lw + 1));
        if start < used + 2 && used > 0 {
            continue;
        }
        dates.push_str(&" ".repeat(start - width_of(&dates)));
        dates.push_str(&text);
        used = start + w;
    }
    out.push((Line::styled(dates, x_style), Vec::new()));
    if let Some(label) = opts.str("xAxisLabel").filter(|l| !l.is_empty()) {
        let pad = (lw + 1 + plot / 2).saturating_sub(width_of(label) / 2);
        out.push((
            Line::styled(
                format!("{}{label}", " ".repeat(pad)),
                label_style("xAxisLabelColor", 0),
            ),
            Vec::new(),
        ));
    }
    out
}

/// The legend's lines (`showLegend`): each name with its color's mark,
/// in a row or one a line (`legendOrientation`), on a background
/// (`legendBgColor`) and in a box (`legendBorderColor`).
fn legend(
    items: &[(String, Color)],
    opts: &Opts,
    mark: &'static str,
    side: bool,
) -> Vec<Line<'static>> {
    let vertical = match opts
        .str("legendOrientation")
        .map(|o| o.trim().to_lowercase())
    {
        Some(o) => o == "vertical",
        None => side,
    };
    let bg = opts.str("legendBgColor").and_then(color);
    let with_bg = |s: Style| match bg {
        Some(bg) => s.bg(bg),
        None => s,
    };
    let entry = |(name, color): &(String, Color)| {
        vec![
            Span::styled(format!("{mark} "), with_bg(Style::default().fg(*color))),
            Span::styled(name.clone(), with_bg(text_style())),
        ]
    };
    let mut lines: Vec<Line<'static>> = if vertical {
        items.iter().map(|i| Line::from(entry(i))).collect()
    } else {
        let mut spans = Vec::new();
        for (k, item) in items.iter().enumerate() {
            if k > 0 {
                spans.push(Span::styled("   ", with_bg(Style::default())));
            }
            spans.extend(entry(item));
        }
        vec![Line::from(spans)]
    };
    let w = lines.iter().map(line_width).max().unwrap_or(0);
    for l in &mut lines {
        let pad = w - line_width(l);
        if pad > 0 {
            l.spans
                .push(Span::styled(" ".repeat(pad), with_bg(Style::default())));
        }
    }
    if let Some(border) = opts.str("legendBorderColor").and_then(color) {
        let b = Style::default().fg(border);
        let edge = |l: &str, r: &str| {
            Line::styled(format!("{l}{}{r}", theme::glyph("─", "-").repeat(w + 2)), b)
        };
        let mut boxed = vec![edge(theme::glyph("┌", "+"), theme::glyph("┐", "+"))];
        for l in lines {
            let mut spans = vec![
                Span::styled(theme::glyph("│", "|"), b),
                Span::styled(" ", with_bg(Style::default())),
            ];
            spans.extend(l.spans);
            spans.push(Span::styled(" ", with_bg(Style::default())));
            spans.push(Span::styled(theme::glyph("│", "|"), b));
            boxed.push(Line::from(spans));
        }
        boxed.push(edge(theme::glyph("└", "+"), theme::glyph("┘", "+")));
        lines = boxed;
    }
    lines
}

/// Where the legend goes: `top`, `bottom`, `left` or `right`.
fn legend_position(opts: &Opts, default: &str) -> String {
    opts.str("legendPosition")
        .map(|p| p.trim().to_lowercase())
        .filter(|p| matches!(p.as_str(), "top" | "bottom" | "left" | "right"))
        .unwrap_or_else(|| default.to_string())
}

/// `main` with `legend` at `position`: above, below, or beside it (the
/// legend's rows from the top; actions moved along on the left).
fn place(main: Rows, legend: Vec<Line<'static>>, position: &str) -> Rows {
    if legend.is_empty() {
        return main;
    }
    match position {
        "top" => plain(legend).into_iter().chain(main).collect(),
        "left" | "right" => {
            let lw = legend.iter().map(line_width).max().unwrap_or(0);
            let mw = main.iter().map(|(l, _)| line_width(l)).max().unwrap_or(0);
            let rows = main.len().max(legend.len());
            let mut main = main.into_iter();
            let mut legend = legend.into_iter();
            (0..rows)
                .map(|_| {
                    let (m, parts) = main.next().unwrap_or_default();
                    let l = legend.next().unwrap_or_default();
                    if position == "left" {
                        let pad = lw + 2 - line_width(&l);
                        let mut spans = l.spans;
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.extend(m.spans);
                        let parts = parts
                            .into_iter()
                            .map(|(a, b, act)| (a + lw + 2, b.saturating_add(lw + 2), act))
                            .collect();
                        (Line::from(spans), parts)
                    } else {
                        let pad = mw + 2 - line_width(&m);
                        let mut spans = m.spans;
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.extend(l.spans);
                        (Line::from(spans), parts)
                    }
                })
                .collect()
        }
        _ => main.into_iter().chain(plain(legend)).collect(),
    }
}

/// The width a chart has beside its legend, and the legend.
fn with_legend(
    opts: &Opts,
    items: &[(String, Color)],
    mark: &'static str,
    default: &str,
    width: usize,
) -> (usize, Vec<Line<'static>>, String) {
    if !opts.bool("showLegend").unwrap_or(false) || items.is_empty() {
        return (width, Vec::new(), String::new());
    }
    let position = legend_position(opts, default);
    let side = matches!(position.as_str(), "left" | "right");
    let lines = legend(items, opts, mark, side);
    let lw = lines.iter().map(line_width).max().unwrap_or(0);
    let room = if side {
        width.saturating_sub(lw + 2).max(12)
    } else {
        width
    };
    (room, lines, position)
}

/// The plot's width in cells, for the axes' labels around it.
fn plot_width(width: usize, rows: usize, axes: &[Option<Axis>; 2]) -> usize {
    let lw = axes[0].as_ref().map_or(0, |a| a.label_width(rows));
    let rw = axes[1].as_ref().map_or(0, |a| 1 + a.label_width(rows));
    width.saturating_sub(lw + 1 + rw).max(4)
}

/// The two axes for the datasets drawn (left, right).
fn axes(data: &Data, opts: &Opts, from_zero: bool, stacked: bool) -> [Option<Axis>; 2] {
    let mut sides: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for (i, set) in data.shown() {
        sides[usize::from(on_right(opts, i))].extend(set.values.iter().flatten());
    }
    if stacked {
        // The totals of each day.
        let totals: Vec<f64> = (0..data.days)
            .map(|d| data.shown().filter_map(|(_, s)| s.values[d]).sum())
            .collect();
        sides[0].extend(totals);
    }
    [0, 1].map(|side| {
        (!sides[side].is_empty() || side == 0)
            .then(|| Axis::new(&sides[side], opts, side, from_zero))
    })
}

/// Braille dots on a plot of `w` × `h` cells.
struct Canvas {
    w: usize,
    h: usize,
    dots: Vec<u8>,
    colors: Vec<Option<Color>>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            dots: vec![0; w * h],
            colors: vec![None; w * h],
        }
    }

    fn set(&mut self, x: i64, y: i64, color: Color) {
        if x < 0 || y < 0 || x >= (self.w * 2) as i64 || y >= (self.h * 4) as i64 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let bit = match (x % 2, y % 4) {
            (0, 0) => 0x01,
            (0, 1) => 0x02,
            (0, 2) => 0x04,
            (1, 0) => 0x08,
            (1, 1) => 0x10,
            (1, 2) => 0x20,
            (0, _) => 0x40,
            _ => 0x80,
        };
        let cell = (y / 4) * self.w + x / 2;
        self.dots[cell] |= bit;
        self.colors[cell] = Some(color);
    }

    fn line(&mut self, (x0, y0): (i64, i64), (x1, y1): (i64, i64), color: Color) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn cells(self) -> Cells {
        let ascii = theme::glyph("x", "*") == "*";
        (0..self.h)
            .map(|r| {
                (0..self.w)
                    .map(|c| {
                        let i = r * self.w + c;
                        match self.dots[i] {
                            0 => (' ', None),
                            _ if ascii => ('*', self.colors[i]),
                            d => (
                                char::from_u32(0x2800 + d as u32).expect("braille is a char"),
                                self.colors[i],
                            ),
                        }
                    })
                    .collect()
            })
            .collect()
    }
}

/// Whether a click on a column says its values (`allowInspectData`).
fn inspecting(opts: &Opts) -> bool {
    opts.bool("allowInspectData").unwrap_or(true)
}

/// A line chart: each dataset's days as points joined by lines (a gap
/// where a day has no value, unless `fillGap`).
pub fn line(data: &Data, opts: &Opts, root: &Opts, width: usize) -> Rows {
    let items: Vec<(String, Color)> = data
        .shown()
        .map(|(i, s)| (s.name.clone(), dataset_color(opts, "lineColor", i)))
        .collect();
    let (width, legend, position) =
        with_legend(opts, &items, theme::glyph("━━", "--"), "bottom", width);
    let axes = axes(data, opts, false, false);
    let rows = plot_rows(opts, root, width);
    let pw = plot_width(width, rows, &axes);
    let mut canvas = Canvas::new(pw, rows);
    let (dots_w, dots_h) = ((pw * 2) as i64, (rows * 4) as i64);
    let x = |d: usize| {
        if data.days <= 1 {
            dots_w / 2
        } else {
            (d as f64 * (dots_w - 1) as f64 / (data.days - 1) as f64).round() as i64
        }
    };
    for (i, set) in data.shown() {
        let axis = axes[usize::from(on_right(opts, i))]
            .as_ref()
            .or(axes[0].as_ref())
            .expect("the left axis is always there");
        let line_color = dataset_color(opts, "lineColor", i);
        let y = |v: f64| ((1.0 - axis.frac(v)) * (dots_h - 1) as f64).round() as i64;
        let show_line = opts.nth("showLine", i).is_none_or(|v| v.trim() != "false");
        let show_point = opts.nth("showPoint", i).is_none_or(|v| v.trim() != "false");
        let fill_gap = opts.nth("fillGap", i).is_some_and(|v| v.trim() == "true");
        let point_color = opts
            .nth("pointColor", i)
            .and_then(color)
            .unwrap_or(line_color);
        let mut prev: Option<(i64, i64)> = None;
        for (d, v) in set.values.iter().enumerate() {
            match v {
                Some(v) => {
                    let p = (x(d), y(*v));
                    if show_line && let Some(q) = prev {
                        canvas.line(q, p, line_color);
                    }
                    if show_point || !show_line {
                        canvas.set(p.0, p.1, point_color);
                    }
                    prev = Some(p);
                }
                None if !fill_gap => prev = None,
                None => {}
            }
        }
    }
    // A column says the day nearest to it.
    let says = inspecting(opts).then(|| {
        (0..pw)
            .map(|c| {
                let d = if data.days <= 1 || pw <= 1 {
                    0
                } else {
                    (c as f64 * (data.days - 1) as f64 / (pw - 1) as f64).round() as usize
                };
                inspect(data, d..d + 1)
            })
            .collect()
    });
    let [l, r] = &axes;
    let main = frame(
        data,
        opts,
        width,
        canvas.cells(),
        [l.as_ref(), r.as_ref()],
        says,
    );
    place(main, legend, &position)
}

/// The eighth blocks, from none to full.
const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A bar chart: a bar a day (side by side for several datasets, or
/// stacked); days put together when there are more than columns.
pub fn bar(data: &Data, opts: &Opts, root: &Opts, width: usize) -> Rows {
    let shown: Vec<(usize, &super::collect::Dataset)> = data.shown().collect();
    let items: Vec<(String, Color)> = shown
        .iter()
        .map(|(i, s)| (s.name.clone(), dataset_color(opts, "barColor", *i)))
        .collect();
    let (width, legend, position) =
        with_legend(opts, &items, theme::glyph("██", "##"), "bottom", width);
    let stacked = root.bool("stack").unwrap_or(false);
    let axes = axes(data, opts, true, stacked);
    let rows = plot_rows(opts, root, width);
    let pw = plot_width(width, rows, &axes);
    // Days into columns: one or more days a slot.
    let slots = data.days.min(pw).max(1);
    let slot_days = |s: usize| (s * data.days / slots)..((s + 1) * data.days / slots);
    let slot_w = (pw / slots).max(1);
    let mut cells: Cells = vec![vec![(' ', None); pw]; rows];
    let ascii = theme::glyph("x", "#") == "#";
    let h8 = (rows * 8) as f64;
    for s in 0..slots {
        let value = |set: &super::collect::Dataset| {
            let vs: Vec<f64> = slot_days(s).filter_map(|d| set.values[d]).collect();
            (!vs.is_empty()).then(|| vs.iter().sum::<f64>())
        };
        // Bars of this slot: (columns, from, to in eighths, color).
        let mut bars: Vec<(std::ops::Range<usize>, f64, f64, Color)> = Vec::new();
        let x0 = s * slot_w;
        let gap = usize::from(slot_w > 2);
        let room = slot_w - gap;
        if stacked {
            let axis = axes[0].as_ref().expect("the left axis is always there");
            let mut base = axis.frac(0.0) * h8;
            let mut total = 0.0;
            for (i, set) in &shown {
                if let Some(v) = value(set) {
                    total += v;
                    let top = axis.frac(total) * h8;
                    bars.push((
                        x0..x0 + room.max(1),
                        base,
                        top,
                        dataset_color(opts, "barColor", *i),
                    ));
                    base = top;
                }
            }
        } else {
            let each = (room / shown.len().max(1)).max(1);
            for (k, (i, set)) in shown.iter().enumerate() {
                let axis = axes[usize::from(on_right(opts, *i))]
                    .as_ref()
                    .or(axes[0].as_ref())
                    .expect("the left axis is always there");
                if let Some(v) = value(set) {
                    let from = (x0 + k * each).min(pw - 1);
                    let zero = axis.frac(0.0) * h8;
                    let top = axis.frac(v) * h8;
                    bars.push((
                        from..(from + each).min(pw),
                        zero.min(top),
                        zero.max(top),
                        dataset_color(opts, "barColor", *i),
                    ));
                }
            }
        }
        for (cols, from, to, color) in bars {
            for r in 0..rows {
                // Row r from the bottom covers eighths [r*8, r*8 + 8).
                let (lo, hi) = ((r * 8) as f64, (r * 8 + 8) as f64);
                let covered = (to.min(hi) - from.max(lo)).max(0.0);
                if covered <= 0.0 {
                    continue;
                }
                let ch = if from > lo + 0.5 {
                    // Starts inside the row (a stacked part): a full block
                    // if most of it is covered.
                    if covered >= 4.0 { '█' } else { continue }
                } else {
                    EIGHTHS[(covered.round() as usize).clamp(1, 8)]
                };
                let ch = if ascii { '#' } else { ch };
                for c in cols.clone() {
                    cells[rows - 1 - r][c] = (ch, Some(color));
                }
            }
        }
    }
    // A column says its slot's days.
    let says = inspecting(opts).then(|| {
        (0..pw)
            .map(|c| inspect(data, slot_days((c / slot_w).min(slots - 1))))
            .collect()
    });
    let [l, r] = &axes;
    let main = frame(data, opts, width, cells, [l.as_ref(), r.as_ref()], says);
    place(main, legend, &position)
}

/// The values of a list given by expressions (a pie's `data`).
fn expressions(opts: &Opts, key: &str, data: &Data) -> Result<Vec<f64>, String> {
    opts.text(key)
        .map(|t| super::params::split(&t))
        .unwrap_or_default()
        .iter()
        .map(|v| expr::number(v.trim_matches(['\'', '"']), data))
        .collect()
}

/// Texts with expressions in them, one an item (`label`, `extLabel`).
fn texts(opts: &Opts, key: &str, data: &Data) -> Vec<String> {
    opts.text(key)
        .map(|t| super::params::split(&t))
        .unwrap_or_default()
        .iter()
        .map(|l| {
            let l = l.trim_matches(['\'', '"']);
            expr::fill(l, data).unwrap_or_else(|_| l.to_string())
        })
        .collect()
}

/// A pie (or donut, `ratioInnerRadius`): its sectors in half blocks,
/// `data` by expression; a legend beside it (`legendPosition`) with each
/// one's name, value and share, and its `label` (not under
/// `hideLabelLessThan`) and `extLabel`.
pub fn pie(data: &Data, opts: &Opts, width: usize) -> Result<Rows, String> {
    let values = expressions(opts, "data", data)?;
    if values.is_empty() {
        return Err("Parameter 'data' for pie not found".into());
    }
    let total: f64 = values.iter().filter(|v| **v > 0.0).sum();
    if total <= 0.0 {
        return Err("The pie's data add up to nothing".into());
    }
    let names = opts.list("dataName");
    let labels = texts(opts, "label", data);
    let ext = texts(opts, "extLabel", data);
    let hide_below = opts.num("hideLabelLessThan").unwrap_or(0.03);
    let ext_only_if_no_label = opts.bool("showExtLabelOnlyIfNoLabel").unwrap_or(false);
    let colors: Vec<Color> = (0..values.len())
        .map(|i| dataset_color(opts, "dataColor", i))
        .collect();
    let rows = ((width / 8).clamp(4, 8)) as i64;
    let cols = rows * 2;
    let (cx, cy, radius) = (cols as f64 / 2.0, rows as f64, rows as f64);
    let inner = opts.num("ratioInnerRadius").unwrap_or(0.0).clamp(0.0, 0.95) * radius;
    // The sectors' ends, clockwise from the top.
    let mut ends = Vec::new();
    let mut acc = 0.0;
    for v in &values {
        acc += v.max(0.0) / total;
        ends.push(acc * std::f64::consts::TAU);
    }
    let pixel = |x: i64, y: i64| -> Option<Color> {
        let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
        let r = (dx * dx + dy * dy).sqrt();
        if r > radius || r < inner {
            return None;
        }
        let angle = dx.atan2(-dy).rem_euclid(std::f64::consts::TAU);
        let i = ends
            .iter()
            .position(|&e| angle <= e)
            .unwrap_or(ends.len() - 1);
        Some(colors[i])
    };
    let ascii = theme::glyph("x", "#") == "#";
    // The legend: name, value, share, label, outside label.
    let shown_names: Vec<String> = (0..values.len())
        .map(|i| {
            names
                .get(i)
                .cloned()
                .or_else(|| labels.get(i).cloned())
                .unwrap_or_else(|| format!("#{}", i + 1))
        })
        .collect();
    let name_w = shown_names.iter().map(|n| width_of(n)).max().unwrap_or(0);
    let shown: Vec<String> = values.iter().map(|v| expr::plain(*v)).collect();
    let value_w = shown.iter().map(|v| width_of(v)).max().unwrap_or(0);
    let legend: Vec<Line<'static>> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let fraction = v.max(0.0) / total;
            let share = (fraction * 100.0).round();
            let mut text = format!(
                "{:<name_w$}  {:>value_w$}  {share}%",
                shown_names[i], shown[i]
            );
            let label = labels
                .get(i)
                .filter(|l| !l.is_empty() && names.get(i).is_some() && fraction >= hide_below);
            if let Some(l) = label {
                text.push_str(&format!("  {l}"));
            }
            if let Some(e) = ext.get(i).filter(|e| !e.is_empty())
                && !(ext_only_if_no_label && label.is_some())
            {
                text.push_str(&format!("  ({e})"));
            }
            Line::from(vec![
                Span::styled(theme::glyph("■ ", "# "), Style::default().fg(colors[i])),
                Span::styled(text, text_style()),
            ])
        })
        .collect();
    let mut disk: Vec<Line<'static>> = Vec::new();
    for r in 0..rows {
        let mut spans = vec![Span::raw(" ")];
        for c in 0..cols {
            let (up, down) = (pixel(c, r * 2), pixel(c, r * 2 + 1));
            spans.push(match (up, down) {
                (None, None) => Span::raw(" "),
                _ if ascii => Span::styled("#", Style::default().fg(up.or(down).expect("one is"))),
                (Some(u), None) => Span::styled("▀", Style::default().fg(u)),
                (None, Some(d)) => Span::styled("▄", Style::default().fg(d)),
                (Some(u), Some(d)) => Span::styled("▀", Style::default().fg(u).bg(d)),
            });
        }
        disk.push(Line::from(spans));
    }
    let position = legend_position(opts, "right");
    let mut out = Vec::new();
    if let Some(t) = opts.str("title").filter(|t| !t.is_empty()) {
        out.push((title(t, cols as usize + 2), Vec::new()));
    }
    // Beside the pie, the legend in its middle.
    let legend = if matches!(position.as_str(), "left" | "right") {
        let top = (rows as usize).saturating_sub(legend.len()) / 2;
        let mut padded = vec![Line::default(); top];
        padded.extend(legend);
        padded
    } else {
        legend
    };
    out.extend(place(plain(disk), legend, &position));
    Ok(out)
}

/// A bullet chart: its ranges as shaded bands, the value as a bar over
/// them, the marker as a line; the value and the scale's end under it.
/// `orientation: vertical` stands it up, the scale on its left.
pub fn bullet(data: &Data, opts: &Opts, width: usize) -> Result<Rows, String> {
    let value = match opts.text("value").filter(|v| !v.trim().is_empty()) {
        Some(v) => expr::number(v.trim_matches(['\'', '"']), data)?,
        None => return Err("Parameter 'value' for bullet not found".into()),
    };
    let ranges = opts.nums("range");
    if ranges.is_empty() {
        return Err("Parameter 'range' for bullet not found".into());
    }
    let max = ranges
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(value);
    let marker = opts
        .num("markerValue")
        .filter(|_| opts.bool("showMarker").unwrap_or(true));
    let bands = opts.list("rangeColor");
    let shades = [FAINT, MUTED, TEXT];
    let band_color = |v: f64| {
        let k = ranges
            .iter()
            .position(|r| v <= *r)
            .unwrap_or(ranges.len() - 1);
        bands
            .get(k)
            .and_then(|c| color(c))
            .unwrap_or_else(|| theme::paint(shades[k.min(shades.len() - 1)]))
    };
    let value_color = opts
        .str("valueColor")
        .and_then(color)
        .unwrap_or_else(|| theme::paint(ACCENT));
    let marker_color = opts
        .str("markerColor")
        .and_then(color)
        .unwrap_or_else(|| theme::paint(TEXT));
    let unit = opts.str("valueUnit").unwrap_or_default().to_string();
    let ascii = theme::glyph("x", "#") == "#";
    let mut out = Vec::new();
    if let Some(t) = opts.str("title").filter(|t| !t.is_empty()) {
        out.push(Line::styled(
            t.to_string(),
            text_style().add_modifier(Modifier::BOLD),
        ));
    }
    let band = |bg: Color| {
        if ascii {
            Span::styled(".", Style::default().fg(bg))
        } else {
            Span::styled(" ", Style::default().bg(bg))
        }
    };
    let vertical = opts
        .str("orientation")
        .is_some_and(|o| o.trim().eq_ignore_ascii_case("vertical"));
    if vertical {
        let h = 10usize;
        let at = |v: f64| ((v / max) * h as f64).round().clamp(0.0, h as f64) as usize;
        let lw = width_of(&expr::plain(max));
        for r in 0..h {
            // Row r from the top covers values above `h - r - 1` tenths.
            let from_bottom = h - r;
            let v = (from_bottom as f64 - 0.5) / h as f64 * max;
            let bg = band_color(v);
            let label = match r {
                0 => expr::plain(max),
                _ if r + 1 == h => "0".into(),
                _ => String::new(),
            };
            let mut spans = vec![Span::styled(format!("{label:>lw$} "), muted()), band(bg)];
            let marked =
                marker.is_some_and(|m| at(m) == from_bottom || (at(m) == 0 && from_bottom == 1));
            for _ in 0..2 {
                spans.push(if marked {
                    Span::styled(
                        theme::glyph("━", "-"),
                        Style::default().fg(marker_color).bg(bg),
                    )
                } else if at(value) >= from_bottom {
                    Span::styled(
                        theme::glyph("█", "="),
                        Style::default().fg(value_color).bg(bg),
                    )
                } else {
                    band(bg)
                });
            }
            spans.push(band(bg));
            out.push(Line::from(spans));
        }
    } else {
        let w = width.saturating_sub(2).max(4);
        let at = |v: f64| ((v / max) * w as f64).round().clamp(0.0, w as f64) as usize;
        let row = |middle: bool| {
            let mut spans = vec![Span::raw(" ")];
            for c in 0..w {
                let bg = band_color((c as f64 + 0.5) / w as f64 * max);
                let span = if marker.is_some_and(|m| at(m) == c || (at(m) == w && c == w - 1)) {
                    Span::styled(
                        theme::glyph("┃", "|"),
                        Style::default().fg(marker_color).bg(bg),
                    )
                } else if middle && c < at(value) {
                    Span::styled(
                        theme::glyph("━", "="),
                        Style::default()
                            .fg(value_color)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    band(bg)
                };
                spans.push(span);
            }
            Line::from(spans)
        };
        out.push(row(false));
        out.push(row(true));
        out.push(row(false));
    }
    let text = format!(
        " {} / {}{}",
        expr::plain(value),
        expr::plain(max),
        if unit.is_empty() {
            String::new()
        } else {
            format!(" {unit}")
        }
    );
    out.push(Line::styled(text, muted()));
    Ok(plain(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::tracker::collect::Dataset;
    use crate::plugins::tracker::params::Params;

    fn data(values: Vec<Option<f64>>) -> Data {
        Data {
            start: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
            days: values.len(),
            sets: vec![Dataset {
                name: "w".into(),
                found: values.len(),
                values,
                is_x: false,
            }],
            notes: Default::default(),
            date_format: "YYYY-MM-DD".into(),
        }
    }

    fn text(rows: &Rows) -> Vec<String> {
        rows.iter()
            .map(|(l, _)| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn output(block: &str) -> (Opts, Opts) {
        let p = Params::parse(&format!("searchType: tag\nsearchTarget: a\n{block}")).unwrap();
        (p.outputs[0].opts.clone(), p.root)
    }

    #[test]
    fn colors_by_name_or_hex_and_styles_by_css() {
        assert_eq!(color("#69b3a2"), Some(Color::Rgb(0x69, 0xb3, 0xa2)));
        assert_eq!(color("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(color("steelblue"), Some(Color::Blue));
        assert_eq!(color("'Red'"), Some(Color::Red));
        assert_eq!(color("nope"), None);
        let s = css(
            "color: red; font-weight: bold; font-style: italic; text-decoration: underline; font-size: 2em",
        );
        assert_eq!(s.fg, Some(Color::Red));
        assert!(
            s.add_modifier
                .contains(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED)
        );
    }

    #[test]
    fn a_line_chart_has_axes_dates_and_braille() {
        let d = data(vec![Some(1.0), Some(3.0), None, Some(2.0)]);
        let (opts, root) =
            output("line:\n  title: T\n  yAxisLabel: kg\n  yAxisUnit: kg\n  showLegend: true\n");
        let rows = text(&line(&d, &opts, &root, 60));
        assert!(rows[0].trim() == "T", "{rows:#?}");
        assert_eq!(rows[1], "kg");
        assert!(
            rows[2].trim_start().starts_with("3kg┤"),
            "the top value: {rows:#?}"
        );
        assert!(
            rows.iter().any(|r| r.trim_start().starts_with("1kg┤")),
            "{rows:#?}"
        );
        assert!(
            rows.iter().any(|r| r.trim_start().starts_with("2kg┤")),
            "a round value between: {rows:#?}"
        );
        assert!(rows.iter().any(|r| r.contains('└')));
        let dates = rows.iter().find(|r| r.contains("2026-10-01")).unwrap();
        assert!(dates.trim_end().ends_with("2026-10-04"), "{dates}");
        assert!(
            rows.iter()
                .any(|r| r.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)))
        );
        assert!(
            rows.last().unwrap().contains("━━ w"),
            "the legend: {rows:#?}"
        );
    }

    #[test]
    fn a_click_on_a_column_says_its_day() {
        let d = data(vec![Some(1.0), Some(3.0), None, Some(2.0)]);
        let (opts, root) = output("line:\n  height: 3\n");
        let rows = line(&d, &opts, &root, 30);
        let parts = &rows[0].1;
        assert_eq!(
            parts.first().unwrap().2,
            "plugin:tracker:say:2026-10-01: w 1"
        );
        assert!(
            parts
                .iter()
                .any(|p| p.2 == "plugin:tracker:say:2026-10-03: w –"),
            "{parts:?}"
        );
        assert_eq!(
            parts.last().unwrap().2,
            "plugin:tracker:say:2026-10-04: w 2"
        );
        // Bars: each bar's days.
        let (opts, root) = output("bar:\n  height: 3\n");
        let rows = bar(&d, &opts, &root, 30);
        assert!(rows[0].1.iter().any(|p| p.2.ends_with("2026-10-02: w 3")));
        let (opts, root) = output("bar:\n  allowInspectData: false\n");
        assert!(bar(&d, &opts, &root, 30).iter().all(|r| r.1.is_empty()));
    }

    #[test]
    fn ticks_formats_and_legends_where_asked() {
        let d = data((0..15).map(|i| Some(i as f64)).collect());
        let (opts, root) = output(
            "line:\n  height: 6\n  yAxisTickInterval: 5\n  yAxisTickLabelFormat: '%.1f'\n  xAxisTickInterval: 1w\n  \
             xAxisTickLabelFormat: D MMM\n  showLegend: true\n  legendPosition: right\n  legendBorderColor: red\n",
        );
        let rows = text(&line(&d, &opts, &root, 70));
        let labels: Vec<&str> = rows
            .iter()
            .filter_map(|r| r.split(['┤']).next().filter(|_| r.contains('┤')))
            .map(str::trim)
            .collect();
        assert_eq!(
            labels,
            ["10.0", "5.0", "0.0"],
            "every 5, formatted: {rows:#?}"
        );
        let dates = rows.iter().find(|r| r.contains("1 Oct")).unwrap();
        assert!(
            dates.contains("8 Oct") && dates.contains("15 Oct"),
            "weekly: {dates}"
        );
        assert!(
            rows[0].contains("┌") && rows[1].contains("━━ w"),
            "a boxed legend on the right: {rows:#?}"
        );
        let (opts, root) = output(
            "bar:\n  showLegend: true\n  legendPosition: left\n  legendOrientation: horizontal\n",
        );
        let rows = bar(&d, &opts, &root, 60);
        assert!(text(&rows)[0].starts_with("██ w"), "{:?}", text(&rows));
        let shifted = rows.iter().find(|r| !r.1.is_empty()).unwrap();
        assert!(
            shifted.1[0].0 > 4,
            "the clicks moved along: {:?}",
            shifted.1[0]
        );
    }

    #[test]
    fn bars_grow_from_zero_in_eighths() {
        let d = data(vec![Some(8.0), Some(4.0), Some(1.0)]);
        let (opts, root) = output("bar:\n  height: 3\n");
        let rows = text(&bar(&d, &opts, &root, 20));
        // Three rows of 24 eighths: 8 → full, 4 → a row and a half, 1 → ▃.
        assert!(rows[0].starts_with("8┤█"), "{rows:#?}");
        assert!(rows[1].contains('▄') && !rows[1].contains('▃'), "{rows:#?}");
        assert!(rows[2].starts_with("0┤"), "from zero: {rows:#?}");
        assert!(rows[2].contains('▃'), "{rows:#?}");
    }

    #[test]
    fn a_pie_and_a_bullet_chart() {
        let d = data(vec![Some(30.0), Some(10.0)]);
        let (opts, _) = output("pie:\n  data: '{{sum()}}, 60'\n  dataName: Done, Left\n");
        let rows = text(&pie(&d, &opts, 40).unwrap());
        assert!(
            rows.iter().any(|r| r.contains("Done  40  40%")),
            "{rows:#?}"
        );
        assert!(
            rows.iter().any(|r| r.contains("Left  60  60%")),
            "{rows:#?}"
        );
        assert!(rows.iter().any(|r| r.contains('▀')), "{rows:#?}");
        let (opts, _) = output("pie:\n  data: 0\n");
        assert!(pie(&d, &opts, 40).is_err());
        let (opts, _) = output(
            "bullet:\n  title: Run\n  value: '{{sum()}}'\n  range: 20, 50\n  valueUnit: km\n  markerValue: 45\n",
        );
        let rows = text(&bullet(&d, &opts, 22).unwrap());
        assert_eq!(rows[0], "Run");
        assert_eq!(
            rows[2].matches('━').count(),
            16,
            "40 of 50 over 20 columns: {rows:#?}"
        );
        assert!(rows[1].contains('┃'), "the marker: {rows:#?}");
        assert_eq!(rows[4], " 40 / 50 km");
    }

    #[test]
    fn pie_labels_legends_and_a_standing_bullet() {
        let d = data(vec![Some(30.0), Some(10.0)]);
        let (opts, _) = output(
            "pie:\n  data: '1, 99'\n  dataName: A, B\n  label: '{{sum()}}, big'\n  extLabel: x, y\n  \
             hideLabelLessThan: 0.05\n  legendPosition: bottom\n",
        );
        let rows = text(&pie(&d, &opts, 40).unwrap());
        let a = rows.iter().find(|r| r.contains("A  ")).unwrap();
        assert!(
            !a.contains("40") && a.contains("(x)"),
            "a small share's label hidden: {a}"
        );
        let b = rows.iter().find(|r| r.contains("B  ")).unwrap();
        assert!(b.contains("big") && b.contains("(y)"), "{b}");
        assert!(
            rows.iter().position(|r| r.contains('▀')) < rows.iter().position(|r| r.contains("A  ")),
            "below"
        );
        let (opts, _) = output(
            "bullet:\n  value: 40\n  range: 20, 50\n  orientation: vertical\n  markerValue: 25\n",
        );
        let rows = text(&bullet(&d, &opts, 22).unwrap());
        assert!(rows[0].starts_with("50 "), "{rows:#?}");
        assert_eq!(
            rows.iter().filter(|r| r.contains("██")).count(),
            7,
            "40 of 50 over 10 rows, the marker over one: {rows:#?}"
        );
        assert!(rows.iter().any(|r| r.contains("━━")), "{rows:#?}");
    }
}
