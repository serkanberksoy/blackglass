//! What a template needs from outside it ([`engine::Output::needs`]): a
//! web page (`tp.web`, through `curl`), a user system command's output
//! (`tp.user.<name>()`, run by the shell with the arguments as environment
//! variables), or a note's text (`tp.file.include` of a note it didn't
//! name). Reading a note is quick and done at once ([`read`]); the others
//! run on a thread ([`start`]) so the editor never waits on them, and the
//! plugin polls ([`Fetch::poll`]) from its tick.
//!
//! [`engine::Output::needs`]: super::engine::Output::needs

use std::io::Read as _;
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use serde_json::Value as Json;

use super::engine::Setup;

/// One need's answer: its key and the text (or why there's none).
pub type Got = (String, Result<String, String>);

/// A need, read from its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    /// A web page (or any URL `curl` reads).
    Get(String),
    /// User function `name`'s command line, with these environment
    /// variables.
    Run {
        name: String,
        cmd: String,
        env: Vec<(String, String)>,
    },
    /// A note's text, by its path in the vault.
    Read(String),
}

impl Need {
    /// The need a key (`{"get": url}` …) asks for.
    pub fn parse(key: &str) -> Option<Need> {
        let v: Json = serde_json::from_str(key).ok()?;
        let s = |k: &str| v[k].as_str().map(String::from);
        if let Some(url) = s("get") {
            return Some(Need::Get(url));
        }
        if let Some(path) = s("read") {
            return Some(Need::Read(path));
        }
        let env = v["env"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
            .collect();
        Some(Need::Run {
            name: s("run")?,
            cmd: s("cmd")?,
            env,
        })
    }
}

/// A note's text for a `read` need (`None`: not a read). The path must
/// stay in the vault.
pub fn read(root: &Path, key: &str) -> Option<Result<String, String>> {
    let Some(Need::Read(path)) = Need::parse(key) else {
        return None;
    };
    let rel = Path::new(&path);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Some(Err(format!("{path} is outside the vault")));
    }
    Some(std::fs::read_to_string(root.join(rel)).map_err(|e| format!("can't read {path}: {e}")))
}

/// Whether `url` is on the web (http or https).
fn on_the_web(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Whether the settings allow a need: the web (http, https; or the quotes
/// address the settings name) while web access is on, a command only if
/// it's one of the user functions (system commands on).
pub fn allowed(need: &Need, setup: &Setup) -> Result<(), String> {
    match need {
        Need::Get(_) if !setup.web => {
            Err("tp.web is off (Templater's settings: Web access)".into())
        }
        Need::Get(url) if !on_the_web(url) && *url != setup.quotes => Err(format!(
            "tp.web only fetches web pages (http, https), not {url}"
        )),
        Need::Run { name, cmd, .. } => {
            let Some((_, line)) = setup.commands.iter().find(|(n, _)| n == name) else {
                return Err(format!(
                    "tp.user.{name} isn't a user function (Templater's settings: System commands)"
                ));
            };
            // A command with Templater commands in it was run through them.
            if !line.contains("<%") && line != cmd {
                return Err(format!("tp.user.{name} ran another command"));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Needs being got on threads.
pub struct Fetch {
    rx: Receiver<Got>,
    left: usize,
    got: Vec<Got>,
}

impl Fetch {
    /// Everything, once every need has its answer.
    pub fn poll(&mut self) -> Option<Vec<Got>> {
        loop {
            match self.rx.try_recv() {
                Ok(got) => {
                    self.got.push(got);
                    self.left -= 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.left = 0;
                    break;
                }
            }
        }
        (self.left == 0).then(|| std::mem::take(&mut self.got))
    }

    /// Waits for every answer (tests).
    pub fn wait(&mut self) -> Vec<Got> {
        loop {
            if let Some(all) = self.poll() {
                return all;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Gets `needs` (web pages, commands) on a thread each, each within
/// `timeout`, from the vault at `root`.
pub fn start(needs: Vec<String>, root: &Path, timeout: Duration) -> Fetch {
    let (tx, rx) = channel();
    let left = needs.len();
    for key in needs {
        let tx = tx.clone();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let result = match Need::parse(&key) {
                Some(Need::Get(url)) => get(&url, timeout),
                Some(Need::Run { cmd, env, .. }) => run(&cmd, &env, &root, timeout),
                Some(Need::Read(_)) => read(&root, &key).expect("a read need"),
                None => Err(format!("can't tell what {key} needs")),
            };
            let _ = tx.send((key, result));
        });
    }
    Fetch {
        rx,
        left,
        got: Vec::new(),
    }
}

/// A URL's body, through `curl`.
fn get(url: &str, timeout: Duration) -> Result<String, String> {
    let secs = timeout.as_secs().max(1).to_string();
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", "--max-time", &secs]);
    if on_the_web(url) {
        // Redirects stay on the web too.
        cmd.args(["--proto", "=http,https", "--proto-redir", "=http,https"]);
    }
    cmd.args(["--", url]);
    let out = finished(cmd, timeout + Duration::from_secs(1))
        .map_err(|e| format!("can't fetch {url}: {e}"))?;
    if !out.0 {
        return Err(format!("can't fetch {url}: {}", out.2.trim()));
    }
    Ok(out.1)
}

/// A command line's output (trailing whitespace dropped), run by the shell
/// in the vault's folder with `env` set.
fn run(
    line: &str,
    env: &[(String, String)],
    root: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", line]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", line]);
        c
    };
    cmd.current_dir(root);
    cmd.envs(env.iter().map(|(k, v)| (k, v)));
    let (ok, out, err) = finished(cmd, timeout).map_err(|e| format!("{line}: {e}"))?;
    if !ok {
        let err = err.trim();
        return Err(if err.is_empty() {
            format!("{line} failed")
        } else {
            format!("{line}: {err}")
        });
    }
    Ok(out.trim_end().to_string())
}

/// Runs `cmd` to its end or `timeout` (then it's killed): whether it
/// succeeded, its output and its errors.
fn finished(mut cmd: Command, timeout: Duration) -> Result<(bool, String, String), String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Read while it runs, so a big output can't fill the pipe and stall it.
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("took longer than {} s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    Ok((status.success(), out, err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{scratch, write};

    #[test]
    fn needs_are_read_from_their_keys() {
        assert_eq!(
            Need::parse(r#"{"get":"https://x.io"}"#),
            Some(Need::Get("https://x.io".into()))
        );
        assert_eq!(
            Need::parse(r#"{"run":"hi","cmd":"echo $a","env":{"a":"1"}}"#),
            Some(Need::Run {
                name: "hi".into(),
                cmd: "echo $a".into(),
                env: vec![("a".into(), "1".into())]
            })
        );
        let dir = scratch("templater-outside");
        write(&dir, &[("Notes/A.md", "alpha")]);
        assert_eq!(
            read(&dir, r#"{"read":"Notes/A.md"}"#),
            Some(Ok("alpha".into()))
        );
        assert!(matches!(read(&dir, r#"{"read":"../x.md"}"#), Some(Err(_))));
        assert_eq!(read(&dir, r#"{"get":"x"}"#), None);
    }

    #[test]
    fn only_the_settings_commands_run() {
        let setup = Setup {
            commands: vec![("greet".into(), "echo hi".into())],
            web: false,
            ..Setup::default()
        };
        let run = |name: &str, cmd: &str| Need::Run {
            name: name.into(),
            cmd: cmd.into(),
            env: Vec::new(),
        };
        assert!(allowed(&run("greet", "echo hi"), &setup).is_ok());
        assert!(allowed(&run("greet", "rm -rf x"), &setup).is_err());
        assert!(allowed(&run("other", "echo hi"), &setup).is_err());
        assert!(
            allowed(&Need::Get("https://x.io".into()), &setup).is_err(),
            "web off"
        );
        let web = Setup {
            quotes: "file:///vault/quotes.json".into(),
            ..Setup::default()
        };
        assert!(allowed(&Need::Get("https://x.io/a".into()), &web).is_ok());
        assert!(allowed(&Need::Get("HTTP://x.io".into()), &web).is_ok());
        assert!(
            allowed(&Need::Get("file:///etc/passwd".into()), &web).is_err(),
            "the web only"
        );
        assert!(allowed(&Need::Get("ftp://x.io/f".into()), &web).is_err());
        assert!(
            allowed(&Need::Get("file:///vault/quotes.json".into()), &web).is_ok(),
            "the quotes the settings name"
        );
    }

    #[cfg(unix)]
    #[test]
    fn commands_and_files_are_got_on_threads() {
        let dir = scratch("templater-outside-run");
        write(&dir, &[("data.json", "{\"a\": 1}")]);
        let url = format!("file://{}", dir.join("data.json").display());
        let keys = vec![
            r#"{"run":"x","cmd":"echo \"$name!\"","env":{"name":"Ann"}}"#.to_string(),
            r#"{"run":"x","cmd":"exit 3","env":{}}"#.to_string(),
            r#"{"run":"x","cmd":"sleep 5","env":{}}"#.to_string(),
            serde_json::json!({ "get": url }).to_string(),
        ];
        let mut got = start(keys.clone(), &dir, Duration::from_secs(1)).wait();
        got.sort_by_key(|(k, _)| keys.iter().position(|x| x == k));
        assert_eq!(got[0].1, Ok("Ann!".into()));
        assert!(got[1].1.is_err());
        assert!(got[2].1.as_ref().unwrap_err().contains("longer"), "{got:?}");
        assert_eq!(got[3].1, Ok("{\"a\": 1}".into()));
    }
}
