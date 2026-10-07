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
//! first; "Next / Previous periodic note" (and "Jump forwards to closest
//! daily note" … for each period) jump to the closest existing note.
//! Templates may use Obsidian's `{{date}}`, `{{title}}` … variables (for
//! the note's date, with offsets: `{{date+1d:FMT}}`, `{{month-1M:FMT}}`)
//! and Templater commands. Weeks start on the day the settings say;
//! today's note can open at startup. Calendar sets are other sets of the
//! same settings (`[work/daily]` …), one of them in use. Its panel is a
//! calendar ([`calendar`]).

pub mod calendar;

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{Datelike, Duration, Local, Months, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;

use super::moment;
use super::settings::{Kind, Setting, Values};
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

    /// The first day of the period `day` is in (weeks start on
    /// `week_start`).
    pub fn start(self, day: NaiveDate, week_start: Weekday) -> NaiveDate {
        match self {
            Period::Daily => day,
            Period::Weekly => moment::Week::starting(week_start).start_of(day),
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

/// The settings for every period (of the default calendar set), from the
/// settings file's text.
pub fn parse_settings(text: &str) -> Vec<Settings> {
    parse_sets(text).swap_remove(0).1
}

/// The settings for every period of each calendar set: the default one
/// (`""`: `[daily]` …) first, then the others (`[work/daily]` …) in the
/// file's order.
pub fn parse_sets(text: &str) -> Vec<(String, Vec<Settings>)> {
    let mut names = vec![String::new()];
    for line in text.lines().map(str::trim) {
        if let Some((set, _)) = line
            .strip_prefix('[')
            .and_then(|l| l.strip_suffix(']'))
            .and_then(|l| l.rsplit_once('/'))
        {
            let set = set.trim().to_string();
            if !names.contains(&set) {
                names.push(set);
            }
        }
    }
    names
        .into_iter()
        .map(|name| {
            let settings = parse_set(text, &name);
            (name, settings)
        })
        .collect()
}

/// The settings that apply to every calendar set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct General {
    pub week_start: Weekday,
    pub open_at_startup: bool,
    /// The calendar set in use (`""`: the default one).
    pub set: String,
}

/// The settings file's `[general]` section (or its top).
pub fn parse_general(text: &str) -> General {
    let values = Values::parse(text);
    let get = |key: &str| {
        values
            .get("general", key)
            .or_else(|| values.get("", key))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    General {
        week_start: match get("week_start").as_str() {
            "sunday" => Weekday::Sun,
            "saturday" => Weekday::Sat,
            _ => Weekday::Mon,
        },
        open_at_startup: get("open_at_startup") == "true",
        set: match get("calendar_set").as_str() {
            "default" => String::new(),
            set => set.to_string(),
        },
    }
}

/// One calendar set's settings (`set`: its name, `""` for the default).
fn parse_set(text: &str, set: &str) -> Vec<Settings> {
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
            let (owner, period) = name.trim().rsplit_once('/').unwrap_or(("", name.trim()));
            section = Period::ALL
                .iter()
                .position(|p| p.name() == period.trim())
                .filter(|_| owner.trim() == set);
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

/// `when` moved by an offset (`+1d`, `-1M`, `+2h`: moment's units `y`,
/// `Q`, `M`, `w`, `d`, `h`, `m`, `s`).
fn offset(when: NaiveDateTime, by: &str) -> Option<NaiveDateTime> {
    let (sign, rest) = match by.chars().next()? {
        '+' => (1, &by[1..]),
        '-' => (-1, &by[1..]),
        _ => return None,
    };
    let unit = rest.chars().next_back()?;
    let n: i64 = sign * rest[..rest.len() - unit.len_utf8()].parse::<i64>().ok()?;
    let months = |m: i64| {
        let span = Months::new(u32::try_from(m.unsigned_abs()).ok()?);
        if m >= 0 {
            when.checked_add_months(span)
        } else {
            when.checked_sub_months(span)
        }
    };
    match unit {
        'y' => months(n * 12),
        'Q' => months(n * 3),
        'M' => months(n),
        'w' => when.checked_add_signed(Duration::weeks(n)),
        'd' => when.checked_add_signed(Duration::days(n)),
        'h' => when.checked_add_signed(Duration::hours(n)),
        'm' => when.checked_add_signed(Duration::minutes(n)),
        's' => when.checked_add_signed(Duration::seconds(n)),
        _ => None,
    }
}

/// Fills in Obsidian's core template variables for a note of `date`:
/// `{{title}}`, `{{date}}` / `{{date:FORMAT}}`, `{{time}}` /
/// `{{time:FORMAT}}` (now), `{{yesterday}}`, `{{tomorrow}}`, and
/// `{{monday:FORMAT}}` … `{{sunday:FORMAT}}` (that day of the note's week,
/// which starts on `week_start`), `{{month:FORMAT}}`, `{{quarter:FORMAT}}`,
/// `{{year:FORMAT}}` (their first day); `date`, `time`, `month`,
/// `quarter` and `year` take an offset (`{{date+1d:FORMAT}}`,
/// `{{month-1M:FORMAT}}`). Other `{{…}}` stay as they are.
pub fn core_variables(
    template: &str,
    title: &str,
    date: NaiveDate,
    now: NaiveDateTime,
    week_start: Weekday,
) -> String {
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
        let (head, format) = match inner.split_once(':') {
            Some((n, f)) => (n.trim(), Some(f.trim())),
            None => (inner, None),
        };
        // `date+1d`: the name and its offset.
        let cut = head.find(['+', '-']).unwrap_or(head.len());
        let (name, by) = (head[..cut].to_lowercase(), &head[cut..]);
        let day_format = format.unwrap_or("YYYY-MM-DD");
        let week = moment::Week::starting(week_start);
        let show = |when: NaiveDateTime, default: &str| {
            let when = if by.is_empty() {
                Some(when)
            } else {
                offset(when, by)
            };
            when.map(|w| moment::format_in(&w, format.unwrap_or(default), week))
        };
        let first_of =
            |month: u32| at(NaiveDate::from_ymd_opt(date.year(), month, 1).expect("a month's 1st"));
        let value = match name.as_str() {
            "title" if by.is_empty() => Some(title.to_string()),
            "date" => show(date.and_time(now.time()), "YYYY-MM-DD"),
            "time" => show(now, "HH:mm"),
            "month" => show(first_of(date.month()), "YYYY-MM"),
            "quarter" => show(first_of((date.month() - 1) / 3 * 3 + 1), "YYYY-[Q]Q"),
            "year" => show(first_of(1), "YYYY"),
            "yesterday" if by.is_empty() => {
                Some(moment::format(&at(date - Duration::days(1)), day_format))
            }
            "tomorrow" if by.is_empty() => {
                Some(moment::format(&at(date + Duration::days(1)), day_format))
            }
            day if by.is_empty() => WEEKDAYS.iter().position(|w| *w == day).map(|i| {
                let monday = Period::Weekly.start(date, Weekday::Mon);
                let day = monday + Duration::days(i as i64);
                // The day of the note's week (as the settings start it).
                let first = Period::Weekly.start(date, week_start);
                let day = if day < first {
                    day + Duration::weeks(1)
                } else if day >= first + Duration::weeks(1) {
                    day - Duration::weeks(1)
                } else {
                    day
                };
                moment::format(&at(day), day_format)
            }),
            _ => None,
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
    /// The calendar set in use's settings.
    settings: Vec<Settings>,
    /// Every calendar set (the default one, `""`, first), and which is in
    /// use.
    sets: Vec<(String, Vec<Settings>)>,
    set: usize,
    general: General,
    /// Whether startup is over (today's note opened if the settings say).
    started: bool,
    /// The note the calendar asked about deleting.
    deleting: Option<PathBuf>,
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
            sets: parse_sets(""),
            set: 0,
            general: parse_general(""),
            started: false,
            deleting: None,
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

    fn week_start(&self) -> Weekday {
        self.general.week_start
    }

    /// The first day of `period` that `day` is in.
    fn start(&self, period: Period, day: NaiveDate) -> NaiveDate {
        period.start(day, self.week_start())
    }

    /// The note's name (relative to the period's folder) for a date.
    fn name(&self, period: Period, date: NaiveDate) -> String {
        moment::format_in(
            &date.and_time(NaiveTime::MIN),
            &self.settings(period).format,
            moment::Week::starting(self.week_start()),
        )
    }

    /// A calendar set's name as shown (`default` for the default one).
    fn set_name(name: &str) -> &str {
        if name.is_empty() { "default" } else { name }
    }

    /// Saves `key = value` in the settings file (`section` `""`: its top),
    /// then reads it again.
    fn save(&mut self, vault: &Vault, section: &str, entries: &[(&str, &str)]) -> Effect {
        let file = super::settings_file(vault, "periodic-notes");
        let mut values = Values::parse(&std::fs::read_to_string(&file).unwrap_or_default());
        for (key, value) in entries {
            values.set(section, key, value);
        }
        let dir = file.parent().expect("a settings file is in a folder");
        let written = std::fs::create_dir_all(dir).and_then(|()| {
            mdedit::files::write_atomic(&file, &values.to_text(&Plugin::settings(self)))
        });
        match written {
            Ok(()) => {
                self.on_vault_changed(vault);
                Effect::None
            }
            Err(e) => Effect::Message(format!("Periodic Notes: cannot save the settings: {e}")),
        }
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
        let date = self.start(period, day);
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
        let date = self.start(period, day);
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
                    Ok(text) => core_variables(&text, &title, date, now, self.week_start()),
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
            let parsed = moment::parse(name, &s.format).map(|d| self.start(period, d.date()));
            if let Some(d) = parsed.filter(|&d| self.name(period, d) == name) {
                return Some((period, d));
            }
            let start = self.start(period, today);
            (0..=period.reach())
                .flat_map(|k| [k, -k])
                .filter_map(|k| period.step(start, k))
                .find(|&d| self.name(period, d) == name)
                .map(|d| (period, d))
        })
    }

    /// Opens the closest existing note after (or before) the active one
    /// (which must be a note of `only`, if given).
    fn jump(&self, forward: bool, only: Option<Period>, ctx: &Context) -> Effect {
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
        if let Some(only) = only.filter(|&p| p != period) {
            return Effect::Message(format!("Periodic Notes: open a {} note first", only.name()));
        }
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
        self.sets = parse_sets(&text);
        self.general = parse_general(&text);
        self.set = self
            .sets
            .iter()
            .position(|(n, _)| *n == self.general.set)
            .unwrap_or(0);
        self.settings = self.sets[self.set].1.clone();
        self.calendar.week_start = self.general.week_start;
        self.calendar.set = match self.sets.len() {
            1 => String::new(),
            _ => Self::set_name(&self.sets[self.set].0).to_string(),
        };
    }

    fn settings(&self) -> Vec<Setting> {
        let names: Vec<String> = self
            .sets
            .iter()
            .map(|(n, _)| Self::set_name(n).to_string())
            .collect();
        let mut all = Vec::new();
        let periods = self
            .sets
            .iter()
            .flat_map(|(set, _)| Period::ALL.iter().map(move |p| (set.clone(), *p)));
        all.extend(periods.flat_map(|(set, p)| {
            let s = if set.is_empty() {
                p.name().to_string()
            } else {
                format!("{set}/{}", p.name())
            };
            let s = s.as_str();
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
        }));
        all.extend([
            Setting::new(
                "general",
                "week_start",
                "Week starts on",
                "The first day of weekly notes, the calendar and {{monday}} … (Monday: ISO weeks)",
                Kind::Choice(vec!["monday".into(), "sunday".into(), "saturday".into()]),
                "monday",
            ),
            Setting::new(
                "general",
                "open_at_startup",
                "Open today's daily note at startup",
                "When the vault opens, today's daily note opens (made if new)",
                Kind::Toggle,
                "false",
            ),
            Setting::new(
                "general",
                "calendar_set",
                "Calendar set",
                "Which set of these settings is in use (Add a calendar set makes another)",
                Kind::Choice(names),
                "default",
            ),
        ]);
        all.push(Setting::new(
            "sets",
            "add",
            "Add a calendar set",
            "Another set of these settings (a work journal beside a personal one)",
            Kind::Action("add-calendar-set"),
            "",
        ));
        all
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
        const JUMPS: [(&str, &str, &str, &str); 5] = [
            (
                "jump-forward-daily",
                "Jump forwards to closest daily note",
                "jump-backward-daily",
                "Jump backwards to closest daily note",
            ),
            (
                "jump-forward-weekly",
                "Jump forwards to closest weekly note",
                "jump-backward-weekly",
                "Jump backwards to closest weekly note",
            ),
            (
                "jump-forward-monthly",
                "Jump forwards to closest monthly note",
                "jump-backward-monthly",
                "Jump backwards to closest monthly note",
            ),
            (
                "jump-forward-quarterly",
                "Jump forwards to closest quarterly note",
                "jump-backward-quarterly",
                "Jump backwards to closest quarterly note",
            ),
            (
                "jump-forward-yearly",
                "Jump forwards to closest yearly note",
                "jump-backward-yearly",
                "Jump backwards to closest yearly note",
            ),
        ];
        for (p, (fid, fname, bid, bname)) in Period::ALL.iter().zip(JUMPS) {
            if self.settings(*p).enabled {
                commands.push(PluginCommand::new(fid, fname));
                commands.push(PluginCommand::new(bid, bname));
            }
        }
        commands.push(PluginCommand::new(
            "switch-calendar-set",
            "Switch calendar set",
        ));
        commands.push(PluginCommand::new("add-calendar-set", "Add a calendar set"));
        commands
    }

    fn run(&mut self, id: &str, ctx: &Context) -> Effect {
        self.pending = None;
        self.fetching = None;
        // A question closed with Esc is never answered.
        self.confirm = None;
        self.deleting = None;
        if let Some(rest) = id
            .strip_prefix("jump-forward-")
            .map(|p| (true, p))
            .or_else(|| id.strip_prefix("jump-backward-").map(|p| (false, p)))
        {
            let (forward, name) = rest;
            let period = Period::ALL.into_iter().find(|p| p.name() == name);
            return self.jump(forward, period, ctx);
        }
        match id {
            "next" | "previous" => self.jump(id == "next", None, ctx),
            "switch-calendar-set" => {
                if self.sets.len() < 2 {
                    return Effect::Message(
                        "Periodic Notes: one calendar set; Add a calendar set makes another".into(),
                    );
                }
                Effect::Ask(vec![Question::Choose {
                    prompt: "Calendar set".into(),
                    items: self
                        .sets
                        .iter()
                        .map(|(n, _)| Self::set_name(n).to_string())
                        .collect(),
                }])
            }
            "add-calendar-set" => Effect::Ask(vec![Question::Text {
                prompt: "The new calendar set's name (its notes go in a folder of that name)"
                    .into(),
                default: String::new(),
            }]),
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
        if !self.started {
            self.started = true;
            if self.general.open_at_startup && self.pending.is_none() {
                return self.open_date(Period::Daily, Local::now().date_naive(), ctx);
            }
        }
        let Some(got) = self.fetching.as_mut().and_then(|f| f.poll()) else {
            return Effect::None;
        };
        self.fetching = None;
        if let Some(p) = &mut self.pending {
            p.results.extend(got);
        }
        self.create(&[], ctx)
    }

    fn row_action(&mut self, payload: &str, ctx: &Context) -> Effect {
        // `day:2026-03-05` (a Dataview calendar's day without notes): its
        // daily note, asked about first if it isn't there; `open-day:`
        // makes it without asking (a task added to it).
        let day = |p: &str| NaiveDate::parse_from_str(p, "%Y-%m-%d").ok();
        if let Some(d) = payload.strip_prefix("open-day:").and_then(day) {
            return self.open_date(Period::Daily, d, ctx);
        }
        match payload.strip_prefix("day:").and_then(day) {
            Some(d) => self.calendar_open(Period::Daily, d, ctx),
            None => Effect::None,
        }
    }

    fn daily_note(&self, day: NaiveDate) -> Option<String> {
        self.settings(Period::Daily)
            .enabled
            .then(|| self.rel(Period::Daily, self.start(Period::Daily, day)))
    }

    fn answer(&mut self, id: &str, answers: &[Answer], ctx: &Context) -> Effect {
        match (id, answers) {
            ("switch-calendar-set", [Answer::Choice(i)]) => {
                let Some((name, _)) = self.sets.get(*i) else {
                    return Effect::None;
                };
                let name = Self::set_name(name).to_string();
                return match self.save(ctx.vault, "general", &[("calendar_set", &name)]) {
                    Effect::None => {
                        Effect::Message(format!("Periodic Notes: the {name} calendar set"))
                    }
                    other => other,
                };
            }
            ("add-calendar-set", [Answer::Text(name)]) => {
                let name = name.trim().replace(['/', '[', ']', '"'], "");
                if name.is_empty() || self.sets.iter().any(|(n, _)| Self::set_name(n) == name) {
                    return Effect::Message(format!("Periodic Notes: no new set named {name}"));
                }
                let section = format!("{name}/daily");
                return match self.save(
                    ctx.vault,
                    &section,
                    &[("enabled", "true"), ("folder", &name)],
                ) {
                    Effect::None => Effect::Message(format!(
                        "Periodic Notes: calendar set {name} added (Switch calendar set uses it)"
                    )),
                    other => other,
                };
            }
            _ => {}
        }
        if let Some(path) = self.deleting.take() {
            return match answers {
                [Answer::Choice(0)] => Effect::DeleteNote(path),
                _ => Effect::None,
            };
        }
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
            self.settings(period).enabled
                && notes.contains(&self.rel(period, self.start(period, day)))
        };
        let weekly = self.settings(Period::Weekly).enabled;
        Some(
            self.calendar
                .lines(Local::now().date_naive(), width, weekly, has_note),
        )
    }

    fn panel_key(&mut self, key: KeyEvent, ctx: &Context) -> Effect {
        self.confirm = None;
        self.deleting = None;
        match self.calendar.key(key, Local::now().date_naive()) {
            Some(calendar::Action::Open(period, day)) => self.calendar_open(period, day, ctx),
            Some(calendar::Action::Delete(period, day)) => {
                let rel = self.rel(period, self.start(period, day));
                let path = ctx.vault.root.join(format!("{rel}.md"));
                if !path.is_file() {
                    return Effect::Message(format!("Periodic Notes: there's no {rel}.md"));
                }
                self.deleting = Some(path);
                Effect::Ask(vec![Question::Choose {
                    prompt: format!("Delete {rel}?"),
                    items: vec![format!("Delete {rel}.md"), "Keep it".into()],
                }])
            }
            None => Effect::None,
        }
    }

    fn panel_click(&mut self, row: u16, col: u16, ctx: &Context) -> Effect {
        self.confirm = None;
        self.deleting = None;
        match self.calendar.click(row, col, Local::now().date_naive()) {
            Some(calendar::Action::Open(period, day)) => self.calendar_open(period, day, ctx),
            _ => Effect::None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn template_offsets_months_quarters_and_years() {
        let date = day(2026, 10, 6);
        let now = date.and_hms_opt(14, 30, 0).unwrap();
        assert_eq!(
            core_variables(
                "{{date+1d:YYYY-MM-DD}} {{date-1w:DD}} {{time+2h:HH:mm}} {{date:HH:mm}} \
                 {{month:MMMM YYYY}} {{month-1M:YYYY-MM}} {{year:YYYY}} {{year-1y:YYYY}} \
                 {{quarter:YYYY-[Q]Q}} {{quarter+1Q:YYYY-[Q]Q}} {{date+1x:DD}}",
                "t",
                date,
                now,
                Weekday::Mon
            ),
            "2026-10-07 29 16:30 14:30 October 2026 2026-09 2026 2025 2026-Q4 2027-Q1 {{date+1x:DD}}"
        );
        // The week's days follow its first day.
        let days = |start| core_variables("{{sunday:DD}} {{monday:DD}}", "t", date, now, start);
        assert_eq!(days(Weekday::Mon), "11 05");
        assert_eq!(days(Weekday::Sun), "04 05");
        assert_eq!(Period::Weekly.start(date, Weekday::Sun), day(2026, 10, 4));
        assert_eq!(Period::Weekly.start(date, Weekday::Mon), day(2026, 10, 5));
    }

    #[test]
    fn calendar_sets_and_general_settings() {
        let text = "[general]\nweek_start = \"sunday\"\nopen_at_startup = \"true\"\ncalendar_set = \"work\"\n\n\
                    [daily]\nfolder = \"Journal\"\n\n[work/daily]\nfolder = \"Work\"\nformat = \"YYYYMMDD\"\n\n\
                    [work/weekly]\nenabled = false\n";
        let g = parse_general(text);
        assert_eq!(
            (g.week_start, g.open_at_startup, g.set.as_str()),
            (Weekday::Sun, true, "work")
        );
        let sets = parse_sets(text);
        let names: Vec<&str> = sets.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["", "work"]);
        assert_eq!(sets[0].1[0].folder, "Journal");
        assert_eq!(
            (sets[1].1[0].folder.as_str(), sets[1].1[0].format.as_str()),
            ("Work", "YYYYMMDD")
        );
        assert!(!sets[1].1[1].enabled && sets[1].1[2].enabled);
        assert_eq!(parse_settings(text), sets[0].1, "the default set");
    }

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
        assert_eq!(Period::Weekly.start(d, Weekday::Mon), day(2026, 8, 3));
        assert_eq!(Period::Monthly.start(d, Weekday::Mon), day(2026, 8, 1));
        assert_eq!(Period::Quarterly.start(d, Weekday::Mon), day(2026, 7, 1));
        assert_eq!(Period::Yearly.start(d, Weekday::Mon), day(2026, 1, 1));
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
            Weekday::Mon,
        );
        assert_eq!(
            t,
            "2026-08-09 2026-08-09 Sunday 08:05 2026-08-08 10 08-03 9 {{other}} {{ unclosed"
        );
    }
}
