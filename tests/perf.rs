//! Performance: time and memory of the workspace on a large vault (5000
//! notes, the plugins on). Not part of the normal run (slow, and timings
//! vary): `cargo test --release --test perf -- --ignored --nocapture`.
//! Each step prints its time and the process's memory (RSS).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use blackglass::ui;
use blackglass::vault::Vault;
use blackglass::workspace::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const NOTES: usize = 5000;

/// The process's resident memory, in MB.
fn rss() -> f64 {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

fn report(what: &str, took: Duration) {
    println!("PERF {what:<40} {:>10.2?}  rss {:>7.1} MB", took, rss());
}

fn big_vault() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("perf-vault");
    if dir.join("done").exists() {
        return dir.canonicalize().unwrap();
    }
    let _ = fs::remove_dir_all(&dir);
    for i in 0..NOTES {
        let folder = dir.join(format!("Folder {}", i % 50));
        fs::create_dir_all(&folder).unwrap();
        let mut text = format!(
            "---\ntype: {}\nrating: {}\ntags: [t{}, common]\n---\n# Note {i}\n",
            if i % 3 == 0 { "book" } else { "note" },
            i % 5,
            i % 20
        );
        for k in 0..30 {
            text.push_str(&format!(
                "Line {k} of note {i} with [[Note {}]] and #tag{} and some words here.\n",
                (i + k) % NOTES,
                k % 10
            ));
        }
        text.push_str(&format!(
            "- [ ] task {i} 📅 2026-10-{:02}\n- [x] done {i}\n",
            i % 28 + 1
        ));
        text.push_str("```rust\nfn main() { println!(\"hi\"); }\n```\n");
        fs::write(folder.join(format!("Note {i}.md")), text).unwrap();
    }
    let mut big = String::from(
        "# Big\n```tasks\nnot done\ndue before 2026-10-05\nlimit 20\n```\n```dataview\nTABLE rating FROM \"Folder 1\" LIMIT 10\n```\n```base\nfilters: type == \"book\"\nviews:\n  - type: table\n    limit: 10\n    order: [file.name, rating]\n```\n",
    );
    for i in 0..10_000 {
        big.push_str(&format!("line {i} with **bold** and [[Note {i}]]\n"));
    }
    fs::write(dir.join("Big.md"), big).unwrap();
    fs::create_dir_all(dir.join(".blackglass")).unwrap();
    let plugins = "\"dataview\", \"templater\", \"periodic-notes\", \"tasks\", \"bases\", \"tables\", \"recent-files\", \"mermaid\"";
    fs::write(
        dir.join(".blackglass/plugins.toml"),
        format!("installed = [{plugins}]\nenabled = [{plugins}]\n"),
    )
    .unwrap();
    fs::write(dir.join("done"), "").unwrap();
    dir.canonicalize().unwrap()
}

fn draw(app: &mut App, t: &mut Terminal<TestBackend>) {
    t.draw(|f| ui::draw(f, app)).unwrap();
}

fn key(app: &mut App, code: KeyCode, m: KeyModifiers) {
    app.handle_key(KeyEvent::new(code, m));
}

#[test]
#[ignore = "slow: run by hand with --release"]
fn a_large_vault() {
    let dir = big_vault();
    report("start", Duration::ZERO);
    let t0 = Instant::now();
    let vault = Vault::open(&dir).unwrap();
    report("Vault::open (5000 notes)", t0.elapsed());
    let t0 = Instant::now();
    let mut app = App::new(vault);
    report("App::new (plugins index)", t0.elapsed());
    let mut term = Terminal::new(TestBackend::new(160, 50)).unwrap();
    let t0 = Instant::now();
    draw(&mut app, &mut term);
    report("first draw (no note)", t0.elapsed());
    let big = dir.join("Big.md");
    let t0 = Instant::now();
    app.open(&big);
    draw(&mut app, &mut term);
    report("open Big.md + draw", t0.elapsed());
    let t0 = Instant::now();
    for _ in 0..20 {
        draw(&mut app, &mut term);
    }
    report("draw x20 (blocks cached)", t0.elapsed() / 20);
    // Type at the end of the note (below the blocks).
    let t0 = Instant::now();
    for _ in 0..50 {
        key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
        draw(&mut app, &mut term);
    }
    report("key + draw, top of Big (each)", t0.elapsed() / 50);
    let t0 = Instant::now();
    for _ in 0..50 {
        key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
    }
    report("key alone, top of Big (each)", t0.elapsed() / 50);
    key(&mut app, KeyCode::End, KeyModifiers::CONTROL);
    let t0 = Instant::now();
    for _ in 0..50 {
        key(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
        draw(&mut app, &mut term);
    }
    report("key + draw, end of Big (each)", t0.elapsed() / 50);
    let t0 = Instant::now();
    key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    report("save Big.md (plugins re-index)", t0.elapsed());
    let t0 = Instant::now();
    draw(&mut app, &mut term);
    report("draw after save (blocks again)", t0.elapsed());
    let small = dir.join("Folder 1/Note 1.md");
    app.open(&small);
    draw(&mut app, &mut term);
    let t0 = Instant::now();
    for _ in 0..50 {
        key(&mut app, KeyCode::Char('y'), KeyModifiers::NONE);
        draw(&mut app, &mut term);
    }
    report("key + draw, small note (each)", t0.elapsed() / 50);
    let t0 = Instant::now();
    key(&mut app, KeyCode::Char('s'), KeyModifiers::CONTROL);
    report("save small note", t0.elapsed());
    let t0 = Instant::now();
    for _ in 0..20 {
        app.tick();
    }
    report("tick (idle, each)", t0.elapsed() / 20);
    let t0 = Instant::now();
    app.sidebar.search("words here common", &app.vault);
    report("vault search", t0.elapsed());
    let t0 = Instant::now();
    key(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL);
    for c in "note 42".chars() {
        key(&mut app, KeyCode::Char(c), KeyModifiers::NONE);
        draw(&mut app, &mut term);
    }
    report("quick switcher: 7 keys + draws", t0.elapsed());
    key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    let t0 = Instant::now();
    app.rescan();
    report("rescan (F5)", t0.elapsed());
    report("end", Duration::ZERO);
}
