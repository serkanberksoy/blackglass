# Charts with DataviewJS

These charts are drawn by the JavaScript in each block, from the daily
notes' `waistline`, `sleep` and `mood` fields. Move the cursor into a
block to read or change its code; the chart comes back when you leave it.

## Waistline over time

```dataviewjs
// A line chart in braille dots: each character holds 2 × 4 dots.
function plot(values, width, height) {
  const W = width * 2, H = height * 4;
  const min = Math.min(...values), max = Math.max(...values), span = (max - min) || 1;
  const grid = Array.from({ length: height }, () => new Array(width).fill(0));
  const bits = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
  const dot = (x, y) => { grid[Math.floor(y / 4)][Math.floor(x / 2)] |= bits[y % 4][x % 2]; };
  const px = i => Math.round(i * (W - 1) / (values.length - 1));
  const py = v => Math.round((max - v) * (H - 1) / span);
  for (let i = 0; i + 1 < values.length; i++) {
    const [x0, y0, x1, y1] = [px(i), py(values[i]), px(i + 1), py(values[i + 1])];
    const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0), 1);
    for (let s = 0; s <= steps; s++) {
      dot(Math.round(x0 + (x1 - x0) * s / steps), Math.round(y0 + (y1 - y0) * s / steps));
    }
  }
  return { rows: grid.map(r => r.map(b => String.fromCharCode(0x2800 + b)).join("")), min, max };
}

const days = dv.pages('"Journal"').where(p => p.waistline).sort(p => p.file.name);
const values = days.map(p => p.waistline);
const chart = plot(values, 48, 8);
const label = v => v.toFixed(1).padStart(6);
chart.rows.forEach((row, i) => {
  const end = i === 0 || i === chart.rows.length - 1;
  const axis = i === 0 ? label(chart.max) : end ? label(chart.min) : "      ";
  dv.paragraph(`${axis} ${end ? "┤" : "│"}${row}`, { color: "#a88bfa" });
});
const first = days.first().file.name, last = days.last().file.name;
dv.paragraph(`        └${"─".repeat(48)}`, { color: "#5c5c5c" });
dv.paragraph(`         ${first}${last.padStart(48 - first.length)}`, { color: "#999999" });
const change = values.last() - values.first();
dv.paragraph(`${days.length} days, ${change > 0 ? "+" : ""}${change.toFixed(1)} since ${first}`, { bold: true });
```

## The last three weeks at a glance

```dataviewjs
const ticks = "▁▂▃▄▅▆▇█";
function sparkline(values) {
  const min = Math.min(...values), max = Math.max(...values), span = (max - min) || 1;
  return values.map(v => ticks[Math.round((v - min) / span * (ticks.length - 1))]).join("");
}
const days = dv.pages('"Journal"').where(p => p.sleep).sort(p => p.file.name).slice(-21);
dv.paragraph("sleep     " + sparkline(days.map(p => p.sleep)), { color: "#6cb6eb" });
dv.paragraph("waistline " + sparkline(days.map(p => p.waistline)), { color: "#e5c07b" });
```

## Hours of sleep, per day

```dataviewjs
const days = dv.pages('"Journal"').where(p => p.sleep).sort(p => p.file.name).slice(-10);
for (const p of days) {
  const bar = "█".repeat(Math.round(p.sleep * 4)) + (p.sleep * 4 % 1 ? "▌" : "");
  const color = p.sleep >= 7 ? "#6fc28b" : p.sleep >= 6 ? "#e5c07b" : "#e06c75";
  dv.paragraph(`${p.file.name.slice(5)}  ${bar} ${p.sleep}h`, { color });
}
```

## Moods

```dataviewjs
const order = ["great", "good", "ok", "tired"];
const colors = { great: "#6fc28b", good: "#4ec9b0", ok: "#e5c07b", tired: "#e06c75" };
const groups = dv.pages('"Journal"').where(p => p.mood).groupBy(p => p.mood);
const total = groups.map(g => g.rows.length).sum();
for (const mood of order) {
  const g = groups.find(g => g.key === mood);
  const n = g ? g.rows.length : 0;
  const pct = Math.round(n / total * 100);
  dv.paragraph(`${mood.padEnd(6)} ${"■".repeat(n * 2).padEnd(30)} ${String(pct).padStart(3)}%`, { color: colors[mood] });
}
```

## Journal activity (a contribution graph)

```dataviewjs
// One column per week, one row per weekday: ■ a day with a daily note.
const days = new Set(dv.pages('"Journal"').map(p => p.file.name));
const weeks = 8;
const end = new Date("2026-08-23"); // a Sunday
const day = n => new Date(end.getTime() - n * 86400000).toISOString().slice(0, 10);
const names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
for (let d = 0; d < 7; d++) {
  let row = names[d] + " ";
  for (let w = 0; w < weeks; w++) {
    const back = (weeks - 1 - w) * 7 + (6 - d);
    row += days.has(day(back)) ? "■ " : "· ";
  }
  dv.paragraph(row, { color: d < 5 ? "#6fc28b" : "#4ec9b0" });
}
const first = day(weeks * 7 - 1), last = day(0);
dv.paragraph(`   ${dv.func.dateformat(first, "D MMM")} … ${dv.func.dateformat(last, "D MMM")}`, { color: "#999999" });
```

## A link graph around Dune

```dataviewjs
// Notes linking to Dune on the left, Dune's own links on the right.
const dune = dv.page("Dune");
const inbound = dune.file.inlinks.sort();
const outbound = dune.file.outlinks.map(l => dv.page(l)?.file.name ?? l);
const width = Math.max(...inbound.map(n => n.length));
const rows = Math.max(inbound.length, outbound.length, 1);
const middle = Math.floor((inbound.length - 1) / 2);
for (let i = 0; i < rows; i++) {
  const left = i < inbound.length
    ? inbound[i].padStart(width) + (inbound.length === 1 ? " ──" : i === 0 ? " ─┐" : i === inbound.length - 1 ? " ─┘" : " ─┤")
    : " ".repeat(width + 3);
  const hub = i === middle ? "──▶ Dune ──▶ " : " ".repeat(13);
  const right = i === middle ? outbound.join(", ") : "";
  dv.paragraph(left + hub + right, { color: i === middle ? "#a88bfa" : "#dcddde" });
}
dv.paragraph(`${inbound.length} notes link to Dune; Dune links to ${outbound.length}.`, { color: "#999999" });
```
