//! The Encrypt plugin (W-134), after Meld Encrypt's in-place encryption:
//! "Encrypt selection" asks for a password (twice), a hint and whether
//! the marker shows when reading, and puts the encrypted text in the
//! selection's place, in the original's format ([`crypto`]) so notes stay
//! readable by it; "Decrypt" (the cursor on encrypted text) asks for the
//! password, shows the hint, and shows the text: copy it, or put it back
//! decrypted. Passwords can be remembered while blackglass runs (for the
//! note, its folder or the vault, for some minutes); they're never saved.
//! Encrypted text shows as 🔐 and its hint where the cursor isn't.

pub mod crypto;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::settings::{Kind, Setting, Values};
use super::{
    ActiveNote, Answer, Context, Effect, FormField, Manifest, Plugin, PluginCommand, Question,
};
use crate::vault::Vault;

const ID: &str = "encrypt";

/// Where a remembered password applies.
const LEVELS: [&str; 4] = ["off", "note", "folder", "vault"];

/// "When reading" choices: the marker shown, or hidden in a comment.
const READING: [&str; 2] = ["Show 🔐", "Hide (a %% comment)"];

const BUTTONS: [&str; 3] = ["Copy", "Decrypt in place", "Close"];

/// Text in the note to put something in place of: the lines' range (from
/// (line, char column) to (line, char column)) and what's there now.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Place {
    path: Option<PathBuf>,
    from: (usize, usize),
    to: (usize, usize),
    text: String,
}

/// What the plugin is in the middle of.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    None,
    /// The encrypt window is open: for this place (an empty one: the text
    /// is typed in the window).
    Encrypt(Place),
    /// The password is asked for this encrypted text.
    Password(Place, crypto::Secret),
    /// The decrypted text is shown.
    Shown(Place, String),
}

pub struct Encrypt {
    confirm: bool,
    expand_lines: bool,
    show_marker: bool,
    /// "off", "note", "folder" or "vault".
    remember: String,
    /// How long a password is kept (`None`: until blackglass quits).
    keep: Option<Duration>,
    /// Remembered passwords and hints, by where they apply.
    passwords: HashMap<String, (String, String, Instant)>,
    pending: Pending,
    root: PathBuf,
}

impl Encrypt {
    pub fn new() -> Self {
        Encrypt {
            confirm: true,
            expand_lines: false,
            show_marker: true,
            remember: "vault".into(),
            keep: Some(Duration::from_secs(30 * 60)),
            passwords: HashMap::new(),
            pending: Pending::None,
            root: PathBuf::new(),
        }
    }

    /// Where a password for the note at `path` is remembered (`None`: not
    /// remembered).
    fn slot(&self, path: Option<&Path>) -> Option<String> {
        let path = path.unwrap_or(&self.root);
        match self.remember.as_str() {
            "note" => Some(path.to_string_lossy().into_owned()),
            "folder" => Some(
                path.parent()
                    .unwrap_or(&self.root)
                    .to_string_lossy()
                    .into_owned(),
            ),
            "vault" => Some(String::new()),
            _ => None,
        }
    }

    /// The password and hint remembered for `path`, if still kept.
    fn remembered(&mut self, path: Option<&Path>) -> Option<(String, String)> {
        let slot = self.slot(path)?;
        let &(_, _, at) = self.passwords.get(&slot)?;
        if self.keep.is_some_and(|keep| at.elapsed() > keep) {
            self.passwords.remove(&slot);
            return None;
        }
        self.passwords
            .get(&slot)
            .map(|(pw, hint, _)| (pw.clone(), hint.clone()))
    }

    fn remember(&mut self, path: Option<&Path>, password: &str, hint: &str) {
        if let Some(slot) = self.slot(path) {
            self.passwords
                .insert(slot, (password.into(), hint.into(), Instant::now()));
        }
    }

    fn encrypt_command(&mut self, note: &ActiveNote) -> Effect {
        if note.selected.is_none() && secret_at(note).is_some() {
            return Effect::Message(
                "Encrypt: the cursor is on encrypted text (Decrypt shows it)".into(),
            );
        }
        let lines: Vec<&str> = note.text.split('\n').collect();
        let line = lines.get(note.row).copied().unwrap_or("");
        let place = match note.selected {
            Some((from, to)) => Place {
                path: note.path.map(Path::to_path_buf),
                from,
                to,
                text: note.selection.unwrap_or_default().to_string(),
            },
            None if self.expand_lines && !line.trim().is_empty() => Place {
                path: note.path.map(Path::to_path_buf),
                from: (note.row, 0),
                to: (note.row, line.chars().count()),
                text: line.to_string(),
            },
            None => Place {
                path: note.path.map(Path::to_path_buf),
                from: (note.row, note.col),
                to: (note.row, note.col),
                text: String::new(),
            },
        };
        if place.text.contains('🔐') {
            return Effect::Message("Encrypt: that holds encrypted text already".into());
        }
        let (password, hint) = self.remembered(note.path).unwrap_or_default();
        let mut fields = Vec::new();
        if place.text.is_empty() {
            fields.push(FormField::text("Text", "What to encrypt", ""));
        }
        fields.push(FormField::secret(
            "Password",
            "The password to encrypt with",
            &password,
        ));
        if self.confirm {
            fields.push(FormField::secret(
                "Confirm",
                "The password again",
                &password,
            ));
        }
        fields.push(FormField::text(
            "Hint",
            "Shown when decrypting (optional; not encrypted)",
            &hint,
        ));
        fields.push(FormField::choice(
            "When reading",
            "Show the 🔐 marker in the note, or hide it in a comment",
            READING.map(String::from).to_vec(),
            usize::from(!self.show_marker),
        ));
        self.pending = Pending::Encrypt(place);
        Effect::Ask(vec![Question::Form {
            title: "Encrypt".into(),
            fields,
        }])
    }

    fn encrypt_answer(&mut self, place: Place, fields: &[Answer], ctx: &Context) -> Effect {
        let text = |i: usize| match fields.get(i) {
            Some(Answer::Text(t)) => t.as_str(),
            _ => "",
        };
        let mut i = 0;
        let typed = if place.text.is_empty() {
            i += 1;
            text(0).to_string()
        } else {
            place.text.clone()
        };
        let password = text(i).to_string();
        i += 1;
        if self.confirm {
            if text(i) != password {
                return Effect::Message("Encrypt: the passwords don't match".into());
            }
            i += 1;
        }
        let hint = text(i).to_string();
        let visible = !matches!(fields.get(i + 1), Some(Answer::Choice(1)));
        if password.is_empty() {
            return Effect::Message("Encrypt: no password given".into());
        }
        if typed.is_empty() {
            return Effect::Message("Encrypt: nothing to encrypt".into());
        }
        let written = crypto::encrypt(&typed, &password, &hint, visible);
        self.remember(place.path.as_deref(), &password, &hint);
        replace(&place, &written, ctx)
    }

    fn decrypt_command(&mut self, note: &ActiveNote) -> Effect {
        let Some(place) = secret_at(note) else {
            return Effect::Message("Decrypt: put the cursor on encrypted text".into());
        };
        let secret = crypto::parse(&place.text).expect("found as a secret");
        if let Some((password, _)) = self.remembered(note.path)
            && let Some(plain) = crypto::decrypt(&secret, &password)
        {
            return self.show(place, plain);
        }
        let help = if secret.hint.is_empty() {
            "The password it was encrypted with".to_string()
        } else {
            format!("Hint: {}", secret.hint)
        };
        self.pending = Pending::Password(place, secret);
        Effect::Ask(vec![Question::Form {
            title: "Decrypt".into(),
            fields: vec![FormField::secret("Password", &help, "")],
        }])
    }

    fn show(&mut self, place: Place, plain: String) -> Effect {
        let question = Question::Show {
            title: "Decrypted".into(),
            text: plain.clone(),
            buttons: BUTTONS.map(String::from).to_vec(),
        };
        self.pending = Pending::Shown(place, plain);
        Effect::Ask(vec![question])
    }

    fn settings_from(&mut self, values: &Values) {
        let schema = self.settings();
        let get = |key: &str| {
            values.value(
                schema
                    .iter()
                    .find(|s| s.key == key)
                    .expect("a declared setting"),
            )
        };
        self.confirm = get("confirm_password") != "false";
        self.expand_lines = get("expand_to_lines") == "true";
        self.show_marker = get("show_marker") != "false";
        self.remember = get("remember_password").to_string();
        let minutes: u64 = get("remember_minutes").trim().parse().unwrap_or(30);
        self.keep = (minutes > 0).then(|| Duration::from_secs(minutes * 60));
    }
}

impl Default for Encrypt {
    fn default() -> Self {
        Self::new()
    }
}

/// Char column `col` of `line` as a byte offset.
fn byte_at(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}

/// The encrypted text at the note's cursor (or selected), if any.
fn secret_at(note: &ActiveNote) -> Option<Place> {
    let line = note.text.split('\n').nth(note.row)?;
    let at = byte_at(line, note.col);
    let (start, end) = crypto::spans(line)
        .into_iter()
        .find(|&(s, e)| s <= at && at <= e)?;
    let col = |b: usize| line[..b].chars().count();
    Some(Place {
        path: note.path.map(Path::to_path_buf),
        from: (note.row, col(start)),
        to: (note.row, col(end)),
        text: line[start..end].to_string(),
    })
}

/// Puts `with` in `place` of the active note, if it's still as it was.
fn replace(place: &Place, with: &str, ctx: &Context) -> Effect {
    let Some(note) = ctx.note.filter(|n| n.path == place.path.as_deref()) else {
        return Effect::Message("Encrypt: the note isn't open any more".into());
    };
    let lines: Vec<&str> = note.text.split('\n').collect();
    let ((r0, c0), (r1, c1)) = (place.from, place.to);
    if r1 >= lines.len() {
        return Effect::Message("Encrypt: the note changed".into());
    }
    let before = &lines[r0][..byte_at(lines[r0], c0)];
    let after = &lines[r1][byte_at(lines[r1], c1)..];
    let now: String = lines[r0..=r1].join("\n");
    let now = &now[before.len()..now.len() - after.len()];
    if now != place.text {
        return Effect::Message("Encrypt: the note changed".into());
    }
    let new = format!("{before}{with}");
    let new_lines: Vec<String> = format!("{new}{after}")
        .split('\n')
        .map(String::from)
        .collect();
    let end = new.split('\n').count() - 1;
    let end_col = new.rsplit('\n').next().unwrap_or("").chars().count();
    Effect::ReplaceLines {
        from: r0,
        to: r1 + 1,
        lines: new_lines,
        cursor: (r0 + end, end_col),
    }
}

impl Plugin for Encrypt {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID,
            name: "Encrypt",
            version: "0.1.0",
            author: "blackglass, after meld-cp's Meld Encrypt",
            description: "Encrypt a selection with a password (and a hint), decrypt it again; \
                          in Meld Encrypt's format.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        self.root = vault.root.clone();
        let text = std::fs::read_to_string(super::settings_file(vault, ID)).unwrap_or_default();
        self.settings_from(&Values::parse(&text));
    }

    fn on_note_changed(&mut self, _path: &Path, _vault: &Vault) {}

    fn settings(&self) -> Vec<Setting> {
        let toggle = |key: &str, label: &str, help: &str, default: &str| {
            Setting::new("", key, label, help, Kind::Toggle, default)
        };
        vec![
            toggle(
                "confirm_password",
                "Confirm the password",
                "Type the password twice when encrypting",
                "true",
            ),
            toggle(
                "show_marker",
                "Show the marker when reading",
                "The default for new encrypted text: 🔐 shown (off: hidden in a %% comment)",
                "true",
            ),
            toggle(
                "expand_to_lines",
                "Encrypt the whole line",
                "With nothing selected, encrypt the cursor's line (off: type the text in the window)",
                "false",
            ),
            Setting::new(
                "",
                "remember_password",
                "Remember passwords",
                "While blackglass runs, for the note, its folder or the whole vault; never saved",
                Kind::Choice(LEVELS.map(String::from).to_vec()),
                "vault",
            ),
            Setting::new(
                "",
                "remember_minutes",
                "Forget after (minutes)",
                "How long a password is remembered; 0: until blackglass quits",
                Kind::Text,
                "30",
            ),
            Setting::new(
                "",
                "forget",
                "Forget passwords",
                "Forget every remembered password now",
                Kind::Action("forget-passwords"),
                "",
            ),
        ]
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand::new("encrypt-selection", "Encrypt selection"),
            PluginCommand::new("decrypt", "Decrypt"),
            PluginCommand::new("forget-passwords", "Forget passwords"),
        ]
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.pending = Pending::None;
        if id == "forget-passwords" {
            self.passwords.clear();
            return Effect::Message("Encrypt: passwords forgotten".into());
        }
        let Some(note) = &ctx.note else {
            return Effect::Message("Encrypt: no note is open".into());
        };
        match id {
            "encrypt-selection" => self.encrypt_command(note),
            "decrypt" => self.decrypt_command(note),
            _ => Effect::None,
        }
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        let pending = std::mem::replace(&mut self.pending, Pending::None);
        match (pending, answers) {
            (Pending::Encrypt(place), [Answer::Fields(fields)]) => {
                self.encrypt_answer(place, fields, ctx)
            }
            (Pending::Password(place, secret), [Answer::Fields(fields)]) => {
                let Some(Answer::Text(password)) = fields.first() else {
                    return Effect::None;
                };
                match crypto::decrypt(&secret, password) {
                    Some(plain) => {
                        self.remember(place.path.as_deref(), password, &secret.hint);
                        self.show(place, plain)
                    }
                    None => Effect::Message("Decrypt: Wrong password".into()),
                }
            }
            (Pending::Shown(place, plain), [Answer::Choice(button)]) => match button {
                0 => Effect::CopyText(plain),
                1 => replace(&place, &plain, ctx),
                _ => Effect::None,
            },
            _ => Effect::None,
        }
    }

    fn rendered_spans(&self) -> Vec<(&'static str, &'static str)> {
        crypto::PREFIXES
            .iter()
            .map(|&(open, _, visible)| (open, if visible { " 🔐" } else { " 🔐%%" }))
            .collect()
    }

    fn render_span(&self, open: &str, inner: &str) -> Option<String> {
        let close = if open.starts_with("%%") {
            " 🔐%%"
        } else {
            " 🔐"
        };
        let secret = crypto::parse(&format!("{open}{inner}{close}"))?;
        let lock = crate::ui::theme::glyph("🔐", "[locked]");
        Some(if secret.hint.is_empty() {
            lock.to_string()
        } else {
            format!("{lock} {}", secret.hint)
        })
    }
}
