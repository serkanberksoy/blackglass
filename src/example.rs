//! The example vault, built into the program (`blackglass --example`): a
//! guided tour of every feature, written out to a folder to try.

use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/example_vault.rs"));

/// Where `--example` puts the vault without a folder given.
pub fn default_folder() -> PathBuf {
    crate::config::home().join("blackglass-example")
}

/// Writes the example vault into `folder` (made if needed) and returns its
/// start page. A copy already there is kept as it is (its changes too);
/// another folder that isn't empty is refused.
pub fn install(folder: &Path) -> Result<PathBuf, String> {
    let welcome = folder.join("Welcome.md");
    if welcome.is_file() && folder.join(".blackglass").is_dir() {
        return Ok(welcome);
    }
    let busy = std::fs::read_dir(folder).is_ok_and(|mut d| d.next().is_some());
    if busy {
        return Err(format!(
            "{} isn't empty: give --example a new or empty folder",
            folder.display()
        ));
    }
    for (rel, bytes) in FILES {
        let path = folder.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(welcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::scratch;

    #[test]
    fn the_example_vault_is_written_once() {
        assert!(FILES.iter().any(|(p, _)| *p == "Welcome.md"));
        assert!(FILES.iter().any(|(p, _)| *p == ".blackglass/plugins.toml"));
        assert!(
            !FILES
                .iter()
                .any(|(p, _)| p.contains(".trash/") || p.ends_with("workspace.json")),
            "nothing personal"
        );
        let dir = scratch("example-install").join("tour");
        let welcome = install(&dir).unwrap();
        assert_eq!(welcome, dir.join("Welcome.md"));
        assert!(dir.join("Plugins/Dataview/Time estimates.md").is_file());
        assert!(dir.join("assets/sunset.png").is_file(), "images too");
        // Again: the copy there is kept, changes and all.
        std::fs::write(&welcome, "mine").unwrap();
        assert_eq!(install(&dir).unwrap(), welcome);
        assert_eq!(std::fs::read_to_string(&welcome).unwrap(), "mine");
        // Another folder with things in it: refused.
        let other = scratch("example-busy");
        std::fs::write(other.join("notes.md"), "x").unwrap();
        assert!(install(&other).unwrap_err().contains("isn't empty"));
    }
}
