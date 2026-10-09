//! The real window (`--gui`), on a display: it opens a vault, stays up
//! and closes cleanly. Ignored by default (it needs a display); run with
//! `cargo test --test window -- --ignored`.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
#[ignore = "needs a display (Wayland or X11)"]
fn the_window_opens_a_vault_and_stays_up() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("window-smoke");
    let _ = std::fs::remove_dir_all(&root);
    let vault = root.join("vault");
    let config = root.join("config");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        vault.join("Hello.md"),
        "# Hello 🎉\n\nSome **bold** text.\n",
    )
    .unwrap();
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_blackglass"))
        .arg("--gui")
        .arg(vault.join("Hello.md"))
        .env("XDG_CONFIG_HOME", &config)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("blackglass runs");
    // Up for three seconds: it opened, and nothing panicked.
    while started.elapsed() < Duration::from_secs(3) {
        if let Some(status) = child.try_wait().unwrap() {
            let mut err = String::new();
            std::io::Read::read_to_string(child.stderr.as_mut().unwrap(), &mut err).unwrap();
            panic!("it ended ({status}): {err}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    child.kill().unwrap();
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("panicked"), "{err}");
    // It saved its session in the vault (its own folder there).
    assert!(vault.join(".blackglass").is_dir());
}
