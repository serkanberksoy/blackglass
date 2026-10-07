//! Embeds the example vault (`example_vault/`) in the program, for
//! `blackglass --example`: the files git keeps (so a checkout's own notes,
//! session and trash stay out), or every file but those when there's no
//! git.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Files a running blackglass writes into a vault, never embedded.
fn personal(rel: &str) -> bool {
    rel.contains(".trash/")
        || rel.ends_with(".blackglass/workspace.json")
        || rel.contains("plugins/recent-files/")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let vault = root.join("example_vault");
    println!("cargo:rerun-if-changed=example_vault");
    let tracked = std::process::Command::new("git")
        .args(["ls-files", "-z", "example_vault"])
        .current_dir(&root)
        .output()
        .ok()
        .filter(|o| o.status.success() && !o.stdout.is_empty())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(|p| root.join(p))
                .collect::<Vec<_>>()
        });
    let mut files = tracked.unwrap_or_else(|| {
        let mut all = Vec::new();
        walk(&vault, &mut all);
        all
    });
    files.retain(|p| p.is_file());
    files.sort();
    let mut code = String::from("/// The example vault's files: (path in the vault, contents).\n");
    code.push_str("pub static FILES: &[(&str, &[u8])] = &[\n");
    for path in files {
        let rel = path
            .strip_prefix(&vault)
            .expect("inside the example vault")
            .to_string_lossy()
            .replace('\\', "/");
        if personal(&rel) {
            continue;
        }
        let _ = writeln!(
            code,
            "    ({rel:?}, include_bytes!({:?})),",
            path.display().to_string()
        );
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    std::fs::write(out.join("example_vault.rs"), code).expect("OUT_DIR is writable");
}
