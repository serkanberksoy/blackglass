//! Periodic Notes, blackglass's third plugin: an implementation of Liam
//! Cain's Obsidian plugin. Daily, weekly, monthly, quarterly and yearly
//! notes, each with a name format (moment.js), a folder and a template, set
//! in the plugin's folder, `.blackglass/plugins/periodic-notes/settings.toml`:
//!
//! ```toml
//! [daily]
//! format = "YYYY-MM-DD"
//! folder = "Journal"
//! template = "Templates/Daily"
//!
//! [yearly]
//! enabled = false
//! ```
//!
//! "Open daily note" opens today's note, creating it from its template
//! first; "Next / Previous periodic note" jump to the closest existing note
//! of the active note's period. Templates may use Obsidian's `{{date}}`,
//! `{{title}}` … variables (for the note's date) and Templater commands.
//! Its panel is a calendar ([`calendar`]).

pub mod calendar;

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, Months, NaiveDate, NaiveDateTime, NaiveTime};

use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;

use super::moment;
use super::settings::{Kind, Setting};
use super::templater::{self, engine, new_note_target};
use super::{Answer, Context, Effect, Manifest, Plugin, PluginCommand, Question};
use crate::vault::Vault;

/// A kind of periodic note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Period {
    pub const ALL: [Period; 5] = [
        Period::Daily,
        Period::Weekly,
        Period::Monthly,
        Period::Quarterly,
        Period::Yearly,
    ];

    /// Its name in settings, commands and messages.
    pub fn name(self) -> &'static str {
        match self {
            Period::Daily => "daily",
            Period::Weekly => "weekly",
            Period::Monthly => "monthly",
            Period::Quarterly => "quarterly",
            Period::Yearly => "yearly",
        }
    }

    /// The original plugin's default name formats.
    pub fn default_format(self) -> &'static str {
        match self {
            Period::Daily => "YYYY-MM-DD",
            Period::Weekly => "gggg-[W]ww",
            Period::Monthly => "YYYY-MM",
            Period::Quarterly => "YYYY-[Q]Q",
            Period::Yearly => "YYYY",
        }
    }

    /// The first day of the period `day` is in (weeks start on Monday).
    pub fn start(self, day: NaiveDate) -> NaiveDate {
        match self {
            Period::Daily => day,
            Period::Weekly => day - Duration::days(i64::from(day.weekday().num_days_from_monday())),
            Period::Monthly => day.with_day(1).expect("every month has a 1st"),
            Period::Quarterly => {
                let month = (day.month() - 1) / 3 * 3 + 1;
                NaiveDate::from_ymd_opt(day.year(), month, 1).expect("a quarter's first month")
            }
            Period::Yearly => NaiveDate::from_ymd_opt(day.year(), 1, 1).expect("January 1st"),
        }
    }

    /// `start` moved by `n` periods.
    pub fn step(self, start: NaiveDate, n: i64) -> Option<NaiveDate> {
        let months = |m: i64| {
            let span = Months::new(u32::try_from(m.unsigned_abs()).ok()?);
            if m >= 0 {
                start.checked_add_months(span)
            } else {
                start.checked_sub_months(span)
            }
        };
        match self {
            Period::Daily => start.checked_add_signed(Duration::days(n)),
            Period::Weekly => start.checked_add_signed(Duration::weeks(n)),
            Period::Monthly => months(n),
            Period::Quarterly => months(3 * n),
            Period::Yearly => months(12 * n),
        }
    }

    /// How many periods "next" / "previous" look through (about ten years).
    fn reach(self) -> i64 {
        match self {
            Period::Daily => 3660,
            Period::Weekly => 530,
            Period::Monthly => 122,
            Period::Quarterly => 42,
            Period::Yearly => 12,
        }
    }
}

/// One period's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub format: String,
    /// Relative to the vault (`""`: the top folder).
    pub folder: String,
    /// The template note, relative to the vault (`.md` optional).
    pub template: Option<String>,
}

/// The settings for every period, from the settings file's text.
pub fn parse_settings(text: &str) -> Vec<Settings> {
    let mut all: Vec<Settings> = Period::ALL
        .iter()
        .map(|p| Settings {
            enabled: true,
            format: p.default_format().into(),
            folder: String::new(),
            template: None,
        })
        .collect();
    let mut section: Option<usize> = None;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = Period::ALL.iter().position(|p| p.name() == name.trim());
            continue;
        }
        let (Some(i), Some((key, value))) = (section, line.split_once('=')) else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        let s = &mut all[i];
        match key.trim() {
            "enabled" => s.enabled = value != "false",
            "format" if !value.is_empty() => s.format = value,
            "folder" => s.folder = value.trim_matches('/').to_string(),
            "template" => s.template = Some(value).filter(|v| !v.is_empty()),
            _ => {}
        }
    }
    all
}

/// Fills in Obsidian's core template variables for a note of `date`:
/// `{{title}}`, `{{date}}` / `{{date:FORMAT}}`, `{{time}}` /
/// `{{time:FORMAT}}` (now), `{{yesterday}}`, `{{tomorrow}}`, and
/// `{{monday:FORMAT}}` … `{{sunday:FORMAT}}` (that day of the note's week).
/// Other `{{…}}` stay as they are.
pub fn core_variables(template: &str, title: &str, date: NaiveDate, now: NaiveDateTime) -> String {
    const WEEKDAYS: [&str; 7] = [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ];
    let at = |d: NaiveDate| d.and_time(NaiveTime::MIN);
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = after[..close].trim();
        let (name, format) = match inner.split_once(':') {
            Some((n, f)) => (n.trim().to_lowercase(), Some(f.trim())),
            None => (inner.to_lowercase(), None),
        };
        let day_format = format.unwrap_or("YYYY-MM-DD");
        let value = match name.as_str() {
            "title" => Some(title.to_string()),
            "date" => Some(moment::format(&at(date), day_format)),
            "time" => Some(moment::format(&now, format.unwrap_or("HH:mm"))),
            "yesterday" => Some(moment::format(&at(date - Duration::days(1)), day_format)),
            "tomorrow" => Some(moment::format(&at(date + Duration::days(1)), day_format)),
            day => WEEKDAYS.iter().position(|w| *w == day).map(|i| {
                let monday = Period::Weekly.start(date);
                moment::format(&at(monday + Duration::days(i as i64)), day_format)
            }),
        };
        match value {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[open..open + 2 + close + 2]),
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

/// A note being created while its template's questions are answered.
struct Pending {
    folder: PathBuf,
    name: String,
    /// The template with the core variables filled in.
    template: String,
    /// The template's path in the vault, if it has one.
    template_file: Option<String>,
    /// The answers to its questions so far.
    answers: Vec<Answer>,
    /// What it got from outside (`tp.web`, user commands …), and how many
    /// times it went for more.
    results: engine::Results,
    rounds: usize,
}

pub struct PeriodicNotes {
    settings: Vec<Settings>,
    pending: Option<Pending>,
    /// … while what its template needs from outside is fetched.
    fetching: Option<templater::outside::Fetch>,
    /// The calendar's day whose missing note waits for "Create it?".
    confirm: Option<(Period, NaiveDate)>,
    calendar: calendar::Calendar,
}

impl Default for PeriodicNotes {
    fn default() -> Self {
        Self::new()
    }
}

impl PeriodicNotes {
    pub fn new() -> Self {
        PeriodicNotes {
            settings: parse_settings(""),
            pending: None,
            fetching: None,
            confirm: None,
            calendar: calendar::Calendar::default(),
        }
    }

    fn settings(&self, period: Period) -> &Settings {
        let i = Period::ALL
            .iter()
            .position(|&p| p == period)
            .expect("every period has settings");
        &self.settings[i]
    }

    /// The note's name (relative to the period's folder) for a date.
    fn name(&self, period: Period, date: NaiveDate) -> String {
        moment::format(
            &date.and_time(NaiveTime::MIN),
            &self.settings(period).format,
        )
    }

    /// The note's path relative to the vault, without `.md`.
    fn rel(&self, period: Period, date: NaiveDate) -> String {
        let folder = &self.settings(period).folder;
        let name = self.name(period, date);
        if folder.is_empty() {
            name
        } else {
            format!("{folder}/{name}")
        }
    }

    /// The calendar opens a period's note; one that doesn't exist yet is
    /// made only after asking.
    fn calendar_open(&mut self, period: Period, day: NaiveDate, ctx: &Context) -> Effect {
        let date = period.start(day);
        let rel = self.rel(period, date);
        let exists = ctx.vault.root.join(format!("{rel}.md")).is_file();
        if exists || !self.settings(period).enabled {
            return self.open_date(period, day, ctx);
        }
        self.pending = None;
        self.fetching = None;
        self.confirm = Some((period, day));
        let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
        Effect::Ask(vec![Question::Choose {
            prompt: format!("Create {name}?"),
            items: vec![format!("Create {rel}.md"), "Don't create it".into()],
        }])
    }

    /// Opens the period's note for `day`, creating it first.
    fn open_date(&mut self, period: Period, day: NaiveDate, ctx: &Context) -> Effect {
        if !self.settings(period).enabled {
            return Effect::Message(format!(
                "Periodic Notes: {} notes are off in the settings",
                period.name()
            ));
        }
        let date = period.start(day);
        let settings = self.settings(period).clone();
        let path = ctx
            .vault
            .root
            .join(format!("{}.md", self.rel(period, date)));
        if path.is_file() {
            return Effect::Open { path };
        }
        let name = self.name(period, date);
        let title = name.rsplit('/').next().unwrap_or(&name).to_string();
        let now = Local::now().naive_local();
        let template_file = settings.template.as_ref().map(|t| {
            if t.ends_with(".md") {
                t.clone()
            } else {
                format!("{t}.md")
            }
        });
        let template = match &settings.template {
            None => String::new(),
            Some(t) => {
                let file = ctx.vault.root.join(if t.ends_with(".md") {
                    t.clone()
                } else {
                    format!("{t}.md")
                });
                match std::fs::read_to_string(&file) {
                    Ok(text) => core_variables(&text, &title, date, now),
                    Err(_) => {
                        return Effect::Message(format!(
                            "Periodic Notes: the {} template {t} doesn't exist",
                            period.name()
                        ));
                    }
                }
            }
        };
        self.pending = Some(Pending {
            folder: ctx.vault.root.join(&settings.folder),
            name,
            template,
            template_file,
            answers: Vec::new(),
            results: engine::Results::new(),
            rounds: 0,
        });
        self.fetching = None;
        self.create(&[], ctx)
    }

    /// Runs the pending note's template (Templater commands too, with
    /// Templater's settings) and creates the note, or asks the template's
    /// questions first, or fetches what it needs from outside (then it
    /// goes on at a tick).
    fn create(&mut self, answers: &[Answer], ctx: &Context) -> Effect {
        let Some(mut p) = self.pending.take() else {
            return Effect::None;
        };
        p.answers.extend_from_slice(answers);
        let (setup, timeout) = templater::setup(ctx.vault);
        loop {
            let target = new_note_target(ctx.vault, &p.folder, &p.name);
            let more = engine::More {
                results: p.results.clone(),
                run_mode: engine::RunMode::CreateNewFromTemplate,
                template_file: p.template_file.clone(),
                active_file: ctx.note.and_then(|n| n.path).map(|path| {
                    let rel = path.strip_prefix(&ctx.vault.root).unwrap_or(path);
                    rel.to_string_lossy().replace('\\', "/")
                }),
                new_folder: String::new(),
                setup: setup.clone(),
            };
            let env = engine::Env {
                vault: ctx.vault,
                target: &target,
                now: Local::now().naive_local(),
                answers: &p.answers,
                more: &more,
            };
            let output = match engine::render(&p.template, &env) {
                Ok(o) => o,
                Err(e) => return Effect::Message(format!("Periodic Notes: {e}")),
            };
            if output.questions.len() > p.answers.len() {
                let questions = output.questions[p.answers.len()..].to_vec();
                self.pending = Some(p);
                return Effect::Ask(questions);
            }
            let (created, needs) =
                match templater::created_notes(&output, &p.answers, &more, ctx.vault) {
                    templater::Next::Done(created, needs) => (created, needs),
                    templater::Next::Ask(questions) => {
                        self.pending = Some(p);
                        return Effect::Ask(questions);
                    }
                    templater::Next::Failed(e) => {
                        return Effect::Message(format!("Periodic Notes: {e}"));
                    }
                };
            let needs: Vec<String> = needs
                .into_iter()
                .filter(|k| !p.results.contains_key(k))
                .collect();
            if needs.is_empty() {
                let main = Effect::CreateNote {
                    folder: p.folder,
                    name: p.name,
                    text: output.text,
                    cursor: output.cursor,
                };
                return templater::file_effects(
                    main,
                    &target,
                    output.actions,
                    created,
                    &ctx.vault.root,
                );
            }
            p.rounds += 1;
            if p.rounds > 10 {
                return Effect::Message(format!("Periodic Notes: {}", templater::TOO_MANY_ROUNDS));
            }
            let slow = templater::sort_needs(needs, &mut p.results, &setup, &ctx.vault.root);
            if !slow.is_empty() {
                self.fetching = Some(templater::outside::start(slow, &ctx.vault.root, timeout));
                self.pending = Some(p);
                return Effect::Message(
                    "Periodic Notes: getting what the template needs (web, commands)…".into(),
                );
            }
        }
    }

    /// The period and date of the note at `rel` (relative to the vault,
    /// without `.md`), if it's a periodic note.
    fn which(&self, rel: &str) -> Option<(Period, NaiveDate)> {
        let today = Local::now().date_naive();
        Period::ALL.into_iter().find_map(|period| {
            let s = self.settings(period);
            if !s.enabled {
                return None;
            }
            let name = if s.folder.is_empty() {
                rel
            } else {
                rel.strip_prefix(&format!("{}/", s.folder))?
            };
            // Read the name as a date; else look for the date that has it.
            let parsed = moment::parse(name, &s.format).map(|d| period.start(d.date()));
            if let Some(d) = parsed.filter(|&d| self.name(period, d) == name) {
                return Some((period, d));
            }
            let start = period.start(today);
            (0..=period.reach())
                .flat_map(|k| [k, -k])
                .filter_map(|k| period.step(start, k))
                .find(|&d| self.name(period, d) == name)
                .map(|d| (period, d))
        })
    }

    /// Opens the closest existing note after (or before) the active one.
    fn jump(&self, forward: bool, ctx: &Context) -> Effect {
        let Some(path) = ctx.note.and_then(|n| n.path) else {
            return Effect::Message("Periodic Notes: open a periodic note first".into());
        };
        let rel = ctx
            .vault
            .rel(path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let rel = rel.strip_suffix(".md").unwrap_or(&rel);
        let Some((period, date)) = self.which(rel) else {
            let title = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Effect::Message(format!("Periodic Notes: {title} isn't a periodic note"));
        };
        let notes: HashSet<String> = ctx
            .vault
            .notes
            .iter()
            .map(|n| n.rel_name().replace('\\', "/"))
            .collect();
        let sign = if forward { 1 } else { -1 };
        (1..=period.reach())
            .filter_map(|k| period.step(date, sign * k))
            .map(|d| self.rel(period, d))
            .find(|r| notes.contains(r))
            .map_or_else(
                || {
                    let which = if forward { "later" } else { "earlier" };
                    Effect::Message(format!("Periodic Notes: no {which} {} note", period.name()))
                },
                |r| Effect::Open {
                    path: ctx.vault.root.join(format!("{r}.md")),
                },
            )
    }
}

impl Plugin for PeriodicNotes {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "periodic-notes",
            name: "Periodic Notes",
            version: "0.1.0",
            author: "blackglass, after Liam Cain's Periodic Notes",
            description: "Daily, weekly, monthly, quarterly and yearly notes: open today's (created from its \
                          template, with {{date}} variables and Templater commands) and jump to the next or \
                          previous one. Settings: .blackglass/plugins/periodic-notes/settings.toml.",
        }
    }

    fn on_load(&mut self, vault: &Vault) {
        self.on_vault_changed(vault);
    }

    fn on_vault_changed(&mut self, vault: &Vault) {
        let text = std::fs::read_to_string(super::settings_file(vault, "periodic-notes"))
            .unwrap_or_default();
        self.settings = parse_settings(&text);
    }

    fn settings(&self) -> Vec<Setting> {
        Period::ALL
            .iter()
            .flat_map(|p| {
                let s = p.name();
                [
                    Setting::new(
                        s,
                        "enabled",
                        "Enabled",
                        &format!("Commands for {s} notes"),
                        Kind::Toggle,
                        "true",
                    ),
                    Setting::new(
                        s,
                        "format",
                        "Name format",
                        "The note's name, in moment.js tokens; / makes folders",
                        Kind::Text,
                        p.default_format(),
                    ),
                    Setting::new(
                        s,
                        "folder",
                        "Folder",
                        "Where the notes go (empty: the vault's top folder)",
                        Kind::Text,
                        "",
                    ),
                    Setting::new(
                        s,
                        "template",
                        "Template",
                        "The note new notes start from (e.g. Templates/Daily; empty: none)",
                        Kind::Text,
                        "",
                    ),
                ]
            })
            .collect()
    }

    fn commands(&self) -> Vec<PluginCommand> {
        const OPEN: [(&str, &str); 5] = [
            ("open-daily", "Open daily note"),
            ("open-weekly", "Open weekly note"),
            ("open-monthly", "Open monthly note"),
            ("open-quarterly", "Open quarterly note"),
            ("open-yearly", "Open yearly note"),
        ];
        let mut commands: Vec<PluginCommand> = Period::ALL
            .iter()
            .zip(OPEN)
            .filter(|(p, _)| self.settings(**p).enabled)
            .map(|(_, (id, name))| match id {
                // Today's note has a key.
                "open-daily" => PluginCommand::new(id, name).keys("Alt+D"),
                _ => PluginCommand::new(id, name),
            })
            .collect();
        commands.push(PluginCommand::new("next", "Next periodic note"));
        commands.push(PluginCommand::new("previous", "Previous periodic note"));
        commands
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.pending = None;
        self.fetching = None;
        // A question closed with Esc is never answered.
        self.confirm = None;
        match id {
            "next" | "previous" => self.jump(id == "next", ctx),
            _ => match Period::ALL
                .iter()
                .find(|p| id == format!("open-{}", p.name()))
            {
                Some(&period) => self.open_date(period, Local::now().date_naive(), ctx),
                None => Effect::None,
            },
        }
    }

    fn tick(&mut self, ctx: &Context) -> Effect {
        let Some(got) = self.fetching.as_mut().and_then(|f| f.poll()) else {
            return Effect::None;
        };
        self.fetching = None;
        if let Some(p) = &mut self.pending {
            p.results.extend(got);
        }
        self.create(&[], ctx)
    }

    fn answer(&mut self, _id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        if let Some((period, day)) = self.confirm.take() {
            return match answers {
                [Answer::Choice(0)] => self.open_date(period, day, ctx),
                _ => Effect::None,
            };
        }
        self.create(answers, ctx)
    }

    fn panel(&self, ctx: &Context, width: u16) -> Option<Vec<Line<'static>>> {
        let notes: HashSet<String> = ctx
            .vault
            .notes
            .iter()
            .map(|n| n.rel_name().replace('\\', "/"))
            .collect();
        let has_note = |period: Period, day: NaiveDate| {
            self.settings(period).enabled && notes.contains(&self.rel(period, period.start(day)))
        };
        let weekly = self.settings(Period::Weekly).enabled;
        Some(
            self.calendar
                .lines(Local::now().date_naive(), width, weekly, has_note),
        )
    }

    fn panel_key(&mut self, key: KeyEvent, ctx: &Context) -> Effect {
        self.confirm = None;
        match self.calendar.key(key, Local::now().date_naive()) {
            Some(calendar::Action::Open(period, day)) => self.calendar_open(period, day, ctx),
            None => Effect::None,
        }
    }

    fn panel_click(&mut self, row: u16, col: u16, ctx: &Context) -> Effect {
        self.confirm = None;
        match self.calendar.click(row, col, Local::now().date_naive()) {
            Some(calendar::Action::Open(period, day)) => self.calendar_open(period, day, ctx),
            None => Effect::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The path of a period's note for a date, relative to the vault.
    fn note_path(settings: &Settings, date: NaiveDate) -> PathBuf {
        let name = moment::format(&date.and_time(NaiveTime::MIN), &settings.format);
        Path::new(&settings.folder).join(format!("{name}.md"))
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn settings_have_the_original_defaults() {
        let s = parse_settings(
            "[daily]\nfolder = \"/Journal/\"\ntemplate = \"Templates/Daily\"\n[weekly]\nenabled = false\nformat = \"YYYY-[W]WW\"\n[nope]\nformat = \"x\"",
        );
        assert_eq!(s[0].folder, "Journal");
        assert_eq!(s[0].format, "YYYY-MM-DD");
        assert_eq!(s[0].template.as_deref(), Some("Templates/Daily"));
        assert!(!s[1].enabled);
        assert_eq!(s[1].format, "YYYY-[W]WW");
        assert_eq!(s[2].format, "YYYY-MM");
        assert!(s[4].enabled && s[4].template.is_none());
    }

    #[test]
    fn periods_start_and_step() {
        let d = day(2026, 8, 9); // a Sunday
        assert_eq!(Period::Weekly.start(d), day(2026, 8, 3));
        assert_eq!(Period::Monthly.start(d), day(2026, 8, 1));
        assert_eq!(Period::Quarterly.start(d), day(2026, 7, 1));
        assert_eq!(Period::Yearly.start(d), day(2026, 1, 1));
        assert_eq!(
            Period::Quarterly.step(day(2026, 7, 1), -1),
            Some(day(2026, 4, 1))
        );
        assert_eq!(
            Period::Monthly.step(day(2026, 1, 1), 13),
            Some(day(2027, 2, 1))
        );
    }

    #[test]
    fn names_follow_the_formats() {
        let s = parse_settings("[monthly]\nfolder = \"Months\"\nformat = \"YYYY/MM\"");
        assert_eq!(note_path(&s[1], day(2026, 8, 3)), Path::new("2026-W32.md"));
        assert_eq!(
            note_path(&s[2], day(2026, 8, 1)),
            Path::new("Months/2026/08.md")
        );
        assert_eq!(note_path(&s[3], day(2026, 7, 1)), Path::new("2026-Q3.md"));
    }

    #[test]
    fn core_template_variables() {
        let now = day(2026, 9, 29).and_hms_opt(8, 5, 0).unwrap();
        let t = core_variables(
            "{{title}} {{date}} {{date:dddd}} {{time}} {{yesterday}} {{tomorrow:D}} {{monday:MM-DD}} {{sunday:D}} {{other}} {{ unclosed",
            "2026-08-09",
            day(2026, 8, 9),
            now,
        );
        assert_eq!(
            t,
            "2026-08-09 2026-08-09 Sunday 08:05 2026-08-08 10 08-03 9 {{other}} {{ unclosed"
        );
    }
}
