//! blackglass: an Obsidian-style note vault in the terminal.
//!
//! A vault is a folder of Markdown notes. blackglass shows it the way
//! Obsidian does: a sidebar on the left with the file explorer, vault
//! search and tags, and the notes in tabs on the right. Each tab is an
//! mdedit editor ([`mdedit::view::EditorView`]) with live preview; links
//! are found anywhere in the vault ([`resolver::VaultResolver`]).
//!
//! - [`vault`]: the scanned folder: tree, note texts, tags, search.
//! - [`sidebar`]: the three panels and their keys.
//! - [`workspace`]: tabs, focus, prompts; keys and mouse events.
//! - [`ui`](mod@ui): drawing.

pub mod backlinks;
pub mod cli;
pub mod commands;
pub mod config;
pub mod keymap;
pub mod link_suggest;
pub mod note_ids;
pub mod pdf;
pub mod plugins;
pub mod properties;
pub mod query_embed;
pub mod resolver;
pub mod session;
pub mod settings_window;
pub mod sidebar;
pub mod switcher;
pub mod tag_suggest;
pub mod ui;
pub mod vault;
pub mod vault_picker;
pub mod watch;
pub mod words;
pub mod workspace;

/// The static Linux build's allocator: musl's own is slow with the vault's
/// parallel indexing (see Cargo.toml).
#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
