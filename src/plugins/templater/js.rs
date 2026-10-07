//! Templater's JavaScript commands (`<%* … %>`): a template that has one
//! (or a command only JavaScript runs) runs as a single async JavaScript
//! function, as in Templater. Text is added to `tR`, `<% expr %>` adds the
//! expression's value, and `<%* %>` runs its code, which may change `tR`,
//! declare variables for later commands and `await` Templater's functions.
//! `tp` has all of Templater's modules but the original app's (`tp.app`,
//! `tp.obsidian`): `date`, `file` (with `create_new`, `move`, `rename`,
//! `find_tfile`, `cursor_append`, `include` running the included note's
//! commands), `frontmatter`, `system` (`prompt`, `suggester`,
//! `multi_suggester`, `clipboard`), `web`, `config`, `hooks` and `user`
//! (the vault's scripts and the settings' system commands); `moment()`
//! covers the basics (`format`, `add`, `subtract`).
//!
//! What a template needs from outside (a web page, a command's output, a
//! note's text) it asks for with `__need`: missing, it's listed in
//! `needs` and the value is `undefined` (the plugin gets it and runs the
//! template again, see [`engine::Results`]).

use boa_engine::{Context, JsResult, JsValue};
use serde_json::{Value as Json, json};

use super::engine::{self, Action, CURSOR, Env, Piece, Source};
use crate::plugins::js;
use crate::plugins::{Answer, Question};

/// `tp` and `moment`, in JavaScript; `__data` is set before it.
const PRELUDE: &str = r##"
const __asked = [];
const __cursors = [];
const __needs = [];
const __actions = [];
const __hooks = [];
const __made = [];
const __AsyncFunction = (async function () {}).constructor;
let __depth = 0;
function __tstr(v) {
  if (v === null || v === undefined) return "";
  if (Array.isArray(v)) {
    const plain = v.every(x => x === null || typeof x !== "object" || x.__iso || x.__tfile);
    return plain ? v.map(__tstr).join(",") : JSON.stringify(v);
  }
  if (typeof v === "object" && v.__iso) return v.format();
  if (typeof v === "object" && v.__tfile) return v.path;
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
function __need(request) {
  const key = JSON.stringify(request);
  const r = __data.results[key];
  if (r === undefined) {
    if (!__needs.includes(key)) __needs.push(key);
    return undefined;
  }
  if (r.err !== undefined) throw new Error(r.err);
  return r.ok;
}
function __get(url) {
  if (!__data.web) throw new Error("tp.web is off (Templater's settings: Web access)");
  return __need({ get: String(url) });
}
function __tfile(path) {
  if (!path) return null;
  const name = path.split("/").pop();
  const dot = name.lastIndexOf(".");
  const slash = path.lastIndexOf("/");
  return {
    __tfile: true, path, name,
    basename: dot > 0 ? name.slice(0, dot) : name,
    extension: dot > 0 ? name.slice(dot + 1) : "",
    parent: { path: slash > 0 ? path.slice(0, slash) : "/" },
    toString() { return this.path; },
  };
}
// The file a link's name points to (the nearest match, as links resolve).
function __find(name) {
  const lower = String(name).trim().replace(/^\/+/, "").toLowerCase();
  const files = __data.files;
  const exact = files.find(f => f.toLowerCase() === lower)
    || files.find(f => f.toLowerCase() === lower + ".md");
  if (exact) return exact;
  const base = lower.split("/").pop();
  return files
    .filter(f => { const b = f.toLowerCase().split("/").pop(); return b === base || b === base + ".md"; })
    .sort((a, b) => a.length - b.length)[0] || null;
}
function __note(path) {
  const known = __data.notes[path.replace(/\.md$/, "").toLowerCase()];
  return known !== undefined ? known : __need({ read: path });
}
// Runs a note's (or a command's) Templater commands with the same `tp`.
async function __run(text) {
  if (__depth >= __data.max_depth) throw new Error("tp.file.include nests too deep (does a note include itself?)");
  __depth++;
  try {
    return await (new __AsyncFunction("tp", "moment", "tR", __compile(text) + "\nreturn tR;"))(tp, moment, "");
  } finally {
    __depth--;
  }
}
function __base(reference, referenceFormat) {
  if (!reference) return __data.now;
  const f = referenceFormat || "YYYY-MM-DD";
  const d = __parse(reference, f);
  if (!d) throw new Error(`can't read "${reference}" as ${f}`);
  return d;
}
function __moved(base, offset) {
  if (offset === undefined || offset === null || offset === "") return base;
  const d = __shift(base, String(offset));
  if (!d) throw new Error(`"${offset}" isn't an offset`);
  return d;
}
function __shown(texts, items) {
  return Array.from(typeof texts === "function" ? Array.from(items).map(texts) : texts, String);
}
const tp = {
  date: {
    now(format = "YYYY-MM-DD", offset, reference, referenceFormat) {
      return __fmt(__moved(__base(reference, referenceFormat), offset), format);
    },
    tomorrow(format = "YYYY-MM-DD") { return __fmt(__shift(__data.now, "1"), format); },
    yesterday(format = "YYYY-MM-DD") { return __fmt(__shift(__data.now, "-1"), format); },
    weekday(format = "YYYY-MM-DD", weekday = 0, reference, referenceFormat) {
      const base = __base(reference, referenceFormat);
      const iso = Number(__fmt(base, "E"));
      return __fmt(__shift(base, String(weekday - (iso - 1))), format);
    },
  },
  file: {
    title: __data.title,
    content: __data.content,
    tags: __data.tags,
    folder(absolute = false) { return absolute ? __data.folder : __data.folder.split("/").pop(); },
    path(relative = false) { return relative ? __data.rel_path : __data.abs_path; },
    creation_date(format = "YYYY-MM-DD HH:mm") { return __fmt(__data.created, format); },
    last_modified_date(format = "YYYY-MM-DD HH:mm") { return __fmt(__data.modified, format); },
    exists(path) { return __data.files.includes(String(path)); },
    find_tfile(name) { return __tfile(__find(name)); },
    async include(link) {
      let path, sub, shown;
      if (link && link.__tfile) {
        path = link.path;
        shown = link.basename;
      } else {
        const m = /^\[\[([^\]]*)\]\]$/.exec(String(link).trim());
        if (!m) throw new Error("tp.file.include needs a [[link]] between quotes");
        const inner = m[1].split("|")[0];
        const hash = inner.indexOf("#");
        const name = hash >= 0 ? inner.slice(0, hash) : inner;
        sub = hash >= 0 ? inner.slice(hash + 1) : undefined;
        shown = inner;
        path = __find(name);
        if (!path) throw new Error(`can't include [[${name}]]: no such note`);
      }
      let text = __note(path);
      if (text === undefined) return "";
      if (sub) {
        text = __section(text, sub);
        if (text === null) throw new Error(`can't include [[${shown}]]: no such heading`);
      }
      return await __run(text);
    },
    async create_new(template, filename, open_new = false, folder) {
      const f = folder && typeof folder === "object" ? folder.path : folder;
      const dir = String(f === undefined || f === null ? __data.new_folder : f).replace(/^\/+|\/+$/g, "");
      const base = filename ? String(filename) : "Untitled";
      const path = n => (dir ? dir + "/" : "") + n + ".md";
      let name = base;
      for (let n = 1; __data.files.includes(path(name)) || __made.includes(path(name)); n++) name = `${base} ${n}`;
      __made.push(path(name));
      const source = template && template.__tfile ? { file: template.path } : { text: __tstr(template) };
      __actions.push({ t: "create", ...source, name, open: !!open_new, folder: dir });
      return __tfile(path(name));
    },
    async move(new_path, file_to_move) {
      __actions.push({ t: "move", to: String(new_path), file: file_to_move && file_to_move.__tfile ? file_to_move.path : null });
      return "";
    },
    async rename(new_title) {
      new_title = String(new_title);
      if (/[\\/:]/.test(new_title)) throw new Error("File name cannot contain any of these characters: \\ / :");
      __actions.push({ t: "rename", title: new_title });
      return "";
    },
    cursor(order = 0) { __cursors.push(Number(order) || 0); return ""; },
    cursor_append(content) { __actions.push({ t: "append", text: __tstr(content) }); return ""; },
    selection() { return __data.selection; },
  },
  frontmatter: __data.frontmatter,
  system: {
    prompt(prompt_text = "Enter a value", default_value = "", throw_on_cancel = false, multiline = false) {
      const i = __asked.length;
      const def = String(default_value ?? "");
      __asked.push({ t: multiline ? "lines" : "text", prompt: String(prompt_text), default: def });
      const a = __data.answers[i];
      return a && a.text !== undefined ? a.text : def;
    },
    suggester(text_items, items, throw_on_cancel = false, placeholder = "", limit) {
      items = Array.from(items);
      const shown = __shown(text_items, items).slice(0, limit || items.length);
      const i = __asked.length;
      __asked.push({ t: "choose", prompt: String(placeholder || "Choose"), items: shown });
      const a = __data.answers[i];
      return a && a.choice !== undefined ? items[a.choice] : null;
    },
    multi_suggester(text_items, items, throw_on_cancel = false, title = "", limit) {
      items = Array.from(items);
      const shown = __shown(text_items, items).slice(0, limit || items.length);
      const i = __asked.length;
      __asked.push({ t: "many", prompt: String(title || "Choose any"), items: shown });
      const a = __data.answers[i];
      return a && a.choices !== undefined ? a.choices.map(c => items[c]) : [];
    },
    clipboard() { return __clipboard(); },
  },
  web: {
    async daily_quote(language) {
      const lang = String(language || "en").toLowerCase();
      const body = __get(__data.quotes[lang] || __data.quotes.en);
      if (body === undefined) return "";
      try {
        const quotes = JSON.parse(body);
        const { quote, author } = quotes[Math.floor(Math.random() * quotes.length)];
        return `> [!quote] ${quote}\n> — ${author}`;
      } catch (e) {
        return "Error generating daily quote";
      }
    },
    async random_picture(size, query, include_size = false) {
      const body = __get(`https://templater-unsplash-2.fly.dev/${query ? "?q=" + query : ""}`);
      if (body === undefined) return "";
      try {
        const photo = JSON.parse(body);
        let url = photo.full;
        if (size && !include_size) {
          if (String(size).includes("x")) {
            const [width, height] = String(size).split("x");
            url += `&w=${width}&h=${height}`;
          } else {
            url += `&w=${size}`;
          }
        }
        const alt = `photo by ${photo.photog}(${photo.photogUrl}) on Unsplash`;
        return include_size ? `![${alt}|${size}](${url})` : `![${alt}](${url})`;
      } catch (e) {
        return "Error generating random picture";
      }
    },
    async request(url, path) {
      const body = __get(url);
      if (body === undefined) return "";
      let data;
      try { data = JSON.parse(body); } catch (e) { throw new Error(`${url} didn't answer with JSON`); }
      if (path && data) {
        return String(path).split(".").reduce((o, k) => {
          if (o !== null && typeof o === "object" && Object.prototype.hasOwnProperty.call(o, k)) return o[k];
          throw new Error(`Path ${path} not found in the JSON response`);
        }, data);
      }
      return data;
    },
  },
  config: {
    run_mode: __data.run_mode,
    template_file: __tfile(__data.template_file),
    target_file: __tfile(__data.rel_path),
    active_file: __tfile(__data.active_file) || undefined,
  },
  hooks: {
    on_all_templates_executed(callback) { __hooks.push(callback); },
  },
  // An unknown name says where scripts and commands come from.
  user: new Proxy({}, {
    get(known, name) {
      if (name in known || typeof name === "symbol" || name === "then" || name === "toJSON") return known[name];
      throw new Error(`tp.user.${name} isn't a script or a user function (Templater's settings: Script files folder, System commands)`);
    },
  }),
  get app() { throw new Error("tp.app is the original app's own API (not in blackglass)"); },
  get ["obsidian"]() { throw new Error(`tp.${"obsidian"} is the original app's own API (not in blackglass)`); },
};
for (const [name, cmd] of __data.commands) {
  tp.user[name] = async (args) => {
    const env = {};
    if (args && typeof args === "object") for (const k of Object.keys(args)) env[k] = __tstr(args[k]);
    const line = cmd.includes("<%") ? await __run(cmd) : cmd;
    const out = __need({ run: name, cmd: line, env });
    return out === undefined ? "" : out;
  };
}
"##;

/// A template run as JavaScript.
pub(super) struct Rendered {
    pub text: String,
    /// Each cursor marker's order, in the order they're in the text.
    pub cursors: Vec<i64>,
    pub questions: Vec<Question>,
    pub needs: Vec<String>,
    pub actions: Vec<Action>,
}

/// The JavaScript that adds a template's `pieces` to `tR`.
fn body(pieces: &[Piece]) -> String {
    let mut body = String::new();
    for piece in pieces {
        match piece {
            Piece::Text(text) => body.push_str(&format!("tR += {};\n", js::literal(text))),
            Piece::Command { code, exec: false } if code.trim().is_empty() => {}
            Piece::Command { code, exec: false } => {
                body.push_str(&format!("tR += __tstr(await ({code}));\n"))
            }
            Piece::Command { code, exec: true } => {
                body.push_str(code);
                body.push_str(";\n");
            }
        }
    }
    body
}

/// `__compile(text)`: the JavaScript for a note's commands (an included
/// note's), or a `throw` saying why there's none.
fn compile(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let text = js::arg(args, 0, ctx)?.unwrap_or_default();
    Ok(js::text(&match engine::parse(&text) {
        Ok(pieces) => body(&pieces),
        Err(e) => format!("throw new Error({});", js::literal(&e)),
    }))
}

/// `__section(text, fragment)`: a heading's section or a block of a
/// note's text, or `null`.
fn section(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let text = js::arg(args, 0, ctx)?.unwrap_or_default();
    let fragment = js::arg(args, 1, ctx)?.unwrap_or_default();
    let lines: Vec<String> = text.split('\n').map(String::from).collect();
    Ok(match mdedit::embed::part(&lines, &fragment) {
        Some(part) => js::text(&part.join("\n")),
        None => JsValue::null(),
    })
}

/// Runs the template's `pieces` as JavaScript.
pub(super) fn render(pieces: &[Piece], env: &Env) -> Result<Rendered, String> {
    let t = env.target;
    let more = env.more;
    let iso = |d: Option<chrono::NaiveDateTime>| js::write_iso(&d.unwrap_or(env.now));
    let lower = t.content.to_lowercase();
    // Notes the template may include: those it names (others are read
    // when it asks for them).
    let mut notes = serde_json::Map::new();
    let whole: String = pieces
        .iter()
        .map(|p| match p {
            Piece::Command { code, .. } => code.to_lowercase(),
            Piece::Text(_) => String::new(),
        })
        .collect();
    for note in &env.vault.notes {
        for name in [note.name().to_lowercase(), note.rel_name().to_lowercase()] {
            if whole.contains(&name) || lower.contains(&name) {
                notes.insert(name, json!(note.lines.join("\n")));
            }
        }
    }
    let answers: Vec<Json> = env
        .answers
        .iter()
        .map(|a| match a {
            Answer::Text(t) => json!({ "text": t }),
            Answer::Choice(i) => json!({ "choice": i }),
            Answer::Choices(all) => json!({ "choices": all }),
            // A form isn't a template's question.
            Answer::Fields(_) => Json::Null,
        })
        .collect();
    let results: serde_json::Map<String, Json> = more
        .results
        .iter()
        .map(|(k, r)| {
            let v = match r {
                Ok(text) => json!({ "ok": text }),
                Err(e) => json!({ "err": e }),
            };
            (k.clone(), v)
        })
        .collect();
    let data = json!({
        "title": t.title, "folder": t.folder, "rel_path": t.rel_path,
        "abs_path": t.abs_path.display().to_string(),
        "created": iso(t.created), "modified": iso(t.modified),
        "content": t.content, "selection": t.selection,
        "tags": crate::vault::tags::extract(&crate::vault::split_lines(&t.content))
            .into_iter().map(|tag| format!("#{tag}")).collect::<Vec<_>>(),
        "frontmatter": engine::frontmatter_json(&t.content),
        "now": js::write_iso(&env.now),
        "answers": answers,
        "files": env.vault.files.iter().map(|f| f.to_string_lossy().replace('\\', "/")).collect::<Vec<_>>(),
        "notes": notes,
        "results": results,
        "web": more.setup.web,
        "quotes": { "en": more.setup.quotes, "es": engine::QUOTES_ES },
        "commands": more.setup.commands,
        "run_mode": more.run_mode as u8,
        "template_file": more.template_file,
        "active_file": more.active_file,
        "new_folder": more.new_folder,
        "max_depth": engine::MAX_INCLUDE_DEPTH,
    });
    // The vault's scripts (CommonJS modules) as `tp.user.<file name>`.
    let mut scripts = String::new();
    for (name, code) in &more.setup.scripts {
        scripts.push_str(&format!(
            "try {{ tp.user[{n}] = (() => {{ const module = {{ exports: {{}} }}; const exports = module.exports;\n{code}\n;\nreturn module.exports; }})(); }} \
             catch (e) {{ throw new Error(`script ${{{n}}}: ${{e}}`); }}\n",
            n = js::literal(name),
        ));
    }
    let script = format!(
        "const __data = JSON.parse({data});\n{PRELUDE}\n{moment}\n{scripts}\n(async () => {{\nlet tR = \"\";\n{body}\
         const __text = tR;\n\
         for (const hook of __hooks) await hook();\n\
         globalThis.__result = JSON.stringify({{ text: __text, asked: __asked, cursors: __cursors, needs: __needs, actions: __actions }});\n\
         }})().catch(e => {{\n\
           if (__needs.length) globalThis.__result = JSON.stringify({{ text: \"\", asked: __asked, cursors: [], needs: __needs, actions: [] }});\n\
           else globalThis.__error = String(e);\n\
         }});",
        data = js::literal(&data.to_string()),
        moment = js::MOMENT,
        body = body(pieces),
    );
    let natives: [js::Native; 2] = [("__compile", 1, compile), ("__section", 2, section)];
    let out: Json =
        serde_json::from_str(&js::run(&script, &natives)?).map_err(|e| e.to_string())?;
    let text = out["text"].as_str().unwrap_or_default().to_string();
    let cursors = out["cursors"]
        .as_array()
        .map(|a| a.iter().map(|c| c.as_f64().unwrap_or(0.0) as i64).collect())
        .unwrap_or_default();
    let strings = |v: &Json| -> Vec<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|i| i.as_str().unwrap_or_default().to_string())
            .collect()
    };
    let questions = out["asked"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| {
            let s = |k: &str| q[k].as_str().unwrap_or_default().to_string();
            match q["t"].as_str() {
                Some("choose") => Question::Choose {
                    prompt: s("prompt"),
                    items: strings(&q["items"]),
                },
                Some("many") => Question::Many {
                    prompt: s("prompt"),
                    items: strings(&q["items"]),
                },
                Some("lines") => Question::Lines {
                    prompt: s("prompt"),
                    default: s("default"),
                },
                _ => Question::Text {
                    prompt: s("prompt"),
                    default: s("default"),
                },
            }
        })
        .collect();
    let actions = out["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let s = |k: &str| a[k].as_str().map(String::from);
            Some(match a["t"].as_str()? {
                "create" => Action::Create {
                    template: match s("file") {
                        Some(path) => Source::File(path),
                        None => Source::Text(s("text").unwrap_or_default()),
                    },
                    name: s("name")?,
                    open: a["open"].as_bool().unwrap_or(false),
                    folder: s("folder"),
                },
                "move" => Action::Move {
                    file: s("file"),
                    to: s("to")?,
                },
                "rename" => Action::Rename(s("title")?),
                "append" => Action::Append(s("text")?),
                _ => return None,
            })
        })
        .collect();
    debug_assert!(!text.contains(CURSOR) || text.chars().filter(|&c| c == CURSOR).count() >= 1);
    Ok(Rendered {
        text,
        cursors,
        questions,
        needs: strings(&out["needs"]),
        actions,
    })
}

#[cfg(test)]
mod tests {
    use super::super::engine::{Action, Env, More, Output, QUOTES_EN, RunMode, Source, render};
    use crate::plugins::{Answer, Question};
    use crate::vault::Vault;
    use crate::vault::tests::{scratch, write};
    use chrono::NaiveDate;
    use serde_json::json;

    fn vault() -> &'static Vault {
        static VAULT: std::sync::OnceLock<Vault> = std::sync::OnceLock::new();
        VAULT.get_or_init(|| {
            let dir = scratch("templater-js");
            write(
                &dir,
                &[
                    ("Snippets/Sig.md", "Regards"),
                    ("Snippets/Hello.md", "Hello <% tp.file.title %>"),
                    ("Snippets/Parts.md", "# A\nx\n# B\n<% 1 + 1 %>"),
                ],
            );
            Vault::open(&dir).unwrap()
        })
    }

    fn with(template: &str, answers: &[Answer], more: &More) -> Result<Output, String> {
        let target = Target {
            title: "2026-08-09".into(),
            folder: "Journal".into(),
            rel_path: "Journal/2026-08-09.md".into(),
            content: "---\nmood: good\nlist: [a, b]\n---\ntext".into(),
            ..Target::default()
        };
        let now = NaiveDate::from_ymd_opt(2026, 8, 9)
            .unwrap()
            .and_hms_opt(14, 7, 0)
            .unwrap();
        render(
            template,
            &Env {
                vault: vault(),
                target: &target,
                now,
                answers,
                more,
            },
        )
    }

    use super::super::engine::Target;

    fn out(template: &str, answers: &[Answer]) -> Result<Output, String> {
        with(template, answers, More::none())
    }

    fn text(template: &str) -> String {
        out(template, &[])
            .map(|o| o.text)
            .unwrap_or_else(|e| format!("ERR {e}"))
    }

    /// Renders `template` with `got` for its needs (key → text).
    fn got(template: &str, got: &[(String, &str)]) -> Output {
        let mut more = More::default();
        for (k, v) in got {
            more.results.insert(k.clone(), Ok(v.to_string()));
        }
        with(template, &[], &more).unwrap()
    }

    fn get(url: &str) -> String {
        json!({ "get": url }).to_string()
    }

    #[test]
    fn javascript_commands_share_variables_and_change_tr() {
        assert_eq!(
            text(
                "<%* const names = ['a', 'b']; let n = 0; %>List:\n<%* for (const x of names) { tR += `- ${x}\\n`; n++; } %>n = <% n %>"
            ),
            "List:\n- a\n- b\nn = 2"
        );
        assert_eq!(text("x<%* tR = 'reset' %>!"), "reset!");
        assert_eq!(
            text(
                "<%* if (tp.file.title.startsWith('2026')) { %>this year<%* } else { %>other<%* } %>"
            ),
            "this year"
        );
    }

    #[test]
    fn tp_and_moment_in_javascript() {
        assert_eq!(
            text(
                "<%* const d = tp.date.now('dddd', 1) %><% d %> <% tp.date.weekday('YYYY-MM-DD', 0) %> <% tp.frontmatter.mood %> <% tp.frontmatter.list %>"
            ),
            "Monday 2026-08-03 good a,b"
        );
        assert_eq!(
            text(
                "<%* tR += moment().add(1, 'M').format('YYYY-MM') + ' ' + moment('2026-08-09').subtract(2, 'days').format('D') %>"
            ),
            "2026-09 7"
        );
        assert_eq!(
            text(
                "<%* tR += await tp.file.include('[[Sig]]') + (tp.file.exists('Snippets/Sig.md') ? '!' : '?') %>"
            ),
            "Regards!"
        );
        let o = out("a<%* tR += tp.file.cursor() %>b", &[]).unwrap();
        assert_eq!((o.text.as_str(), o.cursor), ("ab", Some(1)));
        assert_eq!(
            text("<% moment().format('YYYY') %>"),
            "2026",
            "moment() alone runs as JavaScript"
        );
    }

    #[test]
    fn awaited_questions_are_asked_then_answered() {
        let t = "<%* const who = await tp.system.prompt('Who?', 'you'); const c = await tp.system.suggester(x => x.toUpperCase(), ['red', 'blue']); %>Hi <% who %> in <% c %>";
        let first = out(t, &[]).unwrap();
        assert_eq!(
            first.questions,
            [
                Question::Text {
                    prompt: "Who?".into(),
                    default: "you".into()
                },
                Question::Choose {
                    prompt: "Choose".into(),
                    items: vec!["RED".into(), "BLUE".into()]
                },
            ]
        );
        let o = out(t, &[Answer::Text("Ann".into()), Answer::Choice(1)]).unwrap();
        assert_eq!(o.text, "Hi Ann in blue");
    }

    #[test]
    fn multiline_prompts_and_multi_suggesters() {
        let t = "<%* const a = await tp.system.prompt('Notes', 'x', false, true); const b = await tp.system.multi_suggester(['A', 'B', 'C'], ['a', 'b', 'c'], false, 'Pick', 2); %><% a %>|<% b.join('+') %>";
        assert_eq!(
            out(t, &[]).unwrap().questions,
            [
                Question::Lines {
                    prompt: "Notes".into(),
                    default: "x".into()
                },
                Question::Many {
                    prompt: "Pick".into(),
                    items: vec!["A".into(), "B".into()]
                },
            ],
            "limit: two of the three"
        );
        let o = out(
            t,
            &[Answer::Text("l1\nl2".into()), Answer::Choices(vec![1, 0])],
        )
        .unwrap();
        assert_eq!(o.text, "l1\nl2|b+a");
    }

    #[test]
    fn web_functions_ask_for_their_pages_then_use_them() {
        // The line from a daily template: no `await`, no JavaScript tag.
        let quote = "<% tp.web.daily_quote() %>";
        assert_eq!(out(quote, &[]).unwrap().needs, [get(QUOTES_EN)]);
        let o = got(
            quote,
            &[(get(QUOTES_EN), r#"[{"quote":"Q","author":"A"}]"#)],
        );
        assert_eq!(o.text, "> [!quote] Q\n> — A");
        assert!(o.needs.is_empty());
        let todos = "https://x.io/todos";
        let body = r#"[{"title":"T","done":false}]"#;
        assert_eq!(
            got(
                &format!("<% await tp.web.request('{todos}', '0.title') %>"),
                &[(get(todos), body)]
            )
            .text,
            "T"
        );
        assert_eq!(
            got(
                &format!("<% tp.web.request('{todos}') %>"),
                &[(get(todos), body)]
            )
            .text,
            body.replace(' ', ""),
            "JSON shown as JSON"
        );
        let pic = "https://templater-unsplash-2.fly.dev/?q=water";
        assert_eq!(
            got(
                "<% tp.web.random_picture('200x200', 'water') %>",
                &[(
                    get(pic),
                    r#"{"full":"https://img?x=1","photog":"P","photogUrl":"u"}"#
                )]
            )
            .text,
            "![photo by P(u) on Unsplash](https://img?x=1&w=200&h=200)"
        );
        // Code using a page that hasn't come yet doesn't fail.
        let o = out(
            &format!(
                "<%* const t = await tp.web.request('{todos}', '0.title'); tR += t.toUpperCase() %>"
            ),
            &[],
        )
        .unwrap();
        assert_eq!(o.needs, [get(todos)]);
        let mut off = More::default();
        off.setup.web = false;
        let e = with(quote, &[], &off).unwrap_err();
        assert!(e.contains("Web access"), "{e}");
    }

    #[test]
    fn file_actions_and_tfiles() {
        let o = out(
            "<%* const f = tp.file.find_tfile('Sig'); await tp.file.create_new(f, 'New', true, 'Inbox'); await tp.file.create_new('x', 'Sig', false, 'Snippets'); await tp.file.rename('Renamed'); await tp.file.move('Archive/X'); tp.file.cursor_append('!') %><% f.basename %> <% f.path %> <% tp.file.find_tfile('Nope') %>",
            &[],
        )
        .unwrap();
        assert_eq!(o.text, "Sig Snippets/Sig.md ");
        assert_eq!(
            o.actions,
            [
                Action::Create {
                    template: Source::File("Snippets/Sig.md".into()),
                    name: "New".into(),
                    open: true,
                    folder: Some("Inbox".into())
                },
                Action::Create {
                    template: Source::Text("x".into()),
                    name: "Sig 1".into(),
                    open: false,
                    folder: Some("Snippets".into())
                },
                Action::Rename("Renamed".into()),
                Action::Move {
                    file: None,
                    to: "Archive/X".into()
                },
                Action::Append("!".into()),
            ]
        );
        assert!(text("<%* await tp.file.rename('a/b') %>").contains("cannot contain"));
    }

    #[test]
    fn includes_run_their_commands() {
        assert_eq!(
            text("<% tp.file.include('[[Hello]]') %>"),
            "Hello 2026-08-09"
        );
        assert_eq!(text("<% tp.file.include('[[Parts#B]]') %>"), "# B\n2");
        // A note the template doesn't name is read when it asks for it.
        let t = "<%* const n = 'Hel' + 'lo'; tR += await tp.file.include(tp.file.find_tfile(n)) %>";
        let read = json!({ "read": "Snippets/Hello.md" }).to_string();
        assert_eq!(out(t, &[]).unwrap().needs, std::slice::from_ref(&read));
        assert_eq!(
            got(t, &[(read, "Hello <% tp.file.title %>")]).text,
            "Hello 2026-08-09"
        );
    }

    #[test]
    fn config_hooks_and_user_functions() {
        let more = More {
            run_mode: RunMode::AppendActiveFile,
            template_file: Some("Templates/T.md".into()),
            setup: super::super::engine::Setup {
                scripts: vec![
                    (
                        "shout".into(),
                        "module.exports = (s) => s.toUpperCase() + '!';".into(),
                    ),
                    (
                        "fmt".into(),
                        "module.exports = { note: t => `> [!note] ${t}` };".into(),
                    ),
                ],
                commands: vec![("greet".into(), "echo hi".into())],
                ..Default::default()
            },
            ..More::default()
        };
        let o = with(
            "<% tp.config.run_mode %> <% tp.config.template_file.basename %> <% tp.config.target_file.path %> <% tp.user.shout('hey') %> <% tp.user.fmt.note('x') %>",
            &[],
            &more,
        )
        .unwrap();
        assert_eq!(o.text, "1 T Journal/2026-08-09.md HEY! > [!note] x");
        let o = out(
            "<%* tp.hooks.on_all_templates_executed(async () => { await tp.file.rename('Done') }) %>x",
            &[],
        )
        .unwrap();
        assert_eq!(
            (o.text.as_str(), o.actions.as_slice()),
            ("x", [Action::Rename("Done".into())].as_slice())
        );
        let call = "<% tp.user.greet({who: 'Ann'}) %>";
        // The key as JavaScript writes it.
        let key = r#"{"run":"greet","cmd":"echo hi","env":{"who":"Ann"}}"#.to_string();
        assert_eq!(
            with(call, &[], &more).unwrap().needs,
            std::slice::from_ref(&key)
        );
        let mut done = more.clone();
        done.results.insert(key, Ok("hi Ann".into()));
        assert_eq!(with(call, &[], &done).unwrap().text, "hi Ann");
        assert!(text("<% tp.app.vault %>").contains("original app's own API"));
    }

    #[test]
    fn script_errors_are_reported() {
        assert!(
            text("<%* nope() %>").starts_with("ERR "),
            "{}",
            text("<%* nope() %>")
        );
        assert!(text("<%* while (true) {} %>").contains("loop"));
    }
}
