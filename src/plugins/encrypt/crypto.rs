//! Encrypted text in Meld Encrypt's format (W-134), so notes stay readable
//! by it: `%%🔐β 💡hint💡<base64> 🔐%%` (without the `%%`, the marker shows
//! when reading). Its third version (β) is written: AES-256-GCM with a
//! 16-byte nonce, the key from the password by PBKDF2-SHA-512 (210,000
//! rounds) and a 16-byte salt, the bytes nonce + salt + ciphertext. The
//! older ones are read too: α (PBKDF2-SHA-256, 1,000 rounds, a fixed salt)
//! and the first (the key the password's SHA-256, a fixed nonce).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::aes::Aes256;
use aes_gcm::{Aes256Gcm, AesGcm};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256, Sha512};

/// AES-256-GCM with the 16-byte nonce α and β use.
type Aes256Gcm16 = AesGcm<Aes256, aes_gcm::aead::consts::U16>;

/// One encrypted text, as written in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secret {
    /// 2 (β), 1 (α) or 0 (the first).
    pub version: u8,
    pub hint: String,
    /// The base64 of the encrypted bytes.
    pub data: String,
    /// Not inside `%%`: the marker shows when reading.
    pub visible: bool,
}

/// What starts an encrypted text, as the original looks for them, with
/// its version and whether it shows when reading.
pub const PREFIXES: [(&str, u8, bool); 6] = [
    ("%%🔐β ", 2, false),
    ("🔐β ", 2, true),
    ("%%🔐α ", 1, false),
    ("🔐α ", 1, true),
    ("%%🔐 ", 0, false),
    ("🔐 ", 0, true),
];

/// What ends one (`%%` with a hidden one).
pub const SUFFIXES: [&str; 2] = [" 🔐%%", " 🔐"];

/// The hint's marks.
const HINT: &str = "💡";

/// β's rounds, salt and nonce.
const ROUNDS: u32 = 210_000;
const SALT: usize = 16;
const NONCE: usize = 16;

/// α's rounds and salt (the same for every text).
const ALPHA_ROUNDS: u32 = 1000;
const ALPHA_SALT: &[u8] = b"XHWnDAT6ehMVY2zD";

/// The first version's nonce (the same for every text).
const FIRST_NONCE: [u8; 12] = [196, 190, 240, 190, 188, 78, 41, 132, 15, 220, 84, 211];

/// `text` as one encrypted text, if it's exactly one.
pub fn parse(text: &str) -> Option<Secret> {
    let &(prefix, version, visible) = PREFIXES.iter().find(|(p, ..)| text.starts_with(p))?;
    let suffix = SUFFIXES.iter().find(|s| text.ends_with(*s))?;
    let inner = text.get(prefix.len()..text.len().checked_sub(suffix.len())?)?;
    if PREFIXES.iter().any(|(p, ..)| inner.contains(p))
        || SUFFIXES.iter().any(|s| inner.contains(s))
    {
        return None;
    }
    let (hint, data) = match inner.strip_prefix(HINT) {
        Some(rest) => rest.split_once(HINT)?,
        None => ("", inner),
    };
    Some(Secret {
        version,
        hint: hint.to_string(),
        data: data.to_string(),
        visible,
    })
}

/// The encrypted texts in `line`: their byte ranges, in order.
pub fn spans(line: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(i) = line[at..].find('🔐').map(|i| at + i) {
        // Its `%%` first, when it has them.
        let start = if line[..i].ends_with("%%") { i - 2 } else { i };
        let span = PREFIXES
            .iter()
            .filter(|(p, ..)| line[start..].starts_with(p))
            .find_map(|(p, ..)| {
                let body = start + p.len();
                SUFFIXES.iter().find_map(|s| {
                    let end = body + line[body..].find(s)? + s.len();
                    parse(&line[start..end]).map(|_| (start, end))
                })
            });
        match span {
            Some((s, e)) => {
                found.push((s, e));
                at = e;
            }
            None => at = i + '🔐'.len_utf8(),
        }
    }
    found
}

/// Random bytes for a salt or a nonce.
fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).expect("the system's random numbers");
    bytes
}

/// β's key for `password` and `salt`.
fn key(password: &str, salt: &[u8]) -> [u8; 32] {
    pbkdf2::pbkdf2_hmac_array::<Sha512, 32>(password.as_bytes(), salt, ROUNDS)
}

/// `text` encrypted with `password`, written out (`visible`: without
/// `%%`). The hint loses its marks and line breaks.
pub fn encrypt(text: &str, password: &str, hint: &str, visible: bool) -> String {
    let (salt, nonce) = (random::<SALT>(), random::<NONCE>());
    let cipher = Aes256Gcm16::new(&key(password, &salt).into());
    let sealed = cipher
        .encrypt(&nonce.into(), text.as_bytes())
        .expect("AES-GCM encrypts any text");
    let mut bytes = Vec::with_capacity(NONCE + SALT + sealed.len());
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&salt);
    bytes.extend_from_slice(&sealed);
    let hint = hint.replace(HINT, "").replace(['\n', '\r'], " ");
    let hint = hint.trim();
    let (open, close) = if visible {
        ("🔐β ", " 🔐")
    } else {
        ("%%🔐β ", " 🔐%%")
    };
    let hint = if hint.is_empty() {
        String::new()
    } else {
        format!("{HINT}{hint}{HINT}")
    };
    format!("{open}{hint}{}{close}", STANDARD.encode(bytes))
}

/// The text `secret` holds, if `password` is right.
pub fn decrypt(secret: &Secret, password: &str) -> Option<String> {
    let bytes = STANDARD.decode(secret.data.trim()).ok()?;
    let plain = match secret.version {
        2 => {
            let (nonce, rest) = bytes.split_at_checked(NONCE)?;
            let (salt, sealed) = rest.split_at_checked(SALT)?;
            let cipher = Aes256Gcm16::new(&key(password, salt).into());
            cipher.decrypt(nonce.into(), sealed).ok()?
        }
        1 => {
            let (nonce, sealed) = bytes.split_at_checked(NONCE)?;
            let key = pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(
                password.as_bytes(),
                ALPHA_SALT,
                ALPHA_ROUNDS,
            );
            Aes256Gcm16::new(&key.into())
                .decrypt(nonce.into(), sealed)
                .ok()?
        }
        _ => {
            let key: [u8; 32] = Sha256::digest(password.as_bytes()).into();
            Aes256Gcm::new(&key.into())
                .decrypt(&FIRST_NONCE.into(), bytes.as_slice())
                .ok()?
        }
    };
    String::from_utf8(plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Made by Meld Encrypt's code (WebCrypto, in Node), one per version.
    const BETA: &str = "%%🔐β 💡the usual💡AmAJFj9S7yAiBnXjg5YsQeiWCe1Loe4RrrpM3FAj+AuJDdvpWo7ZYoCcoHr+Arvp7fBGz6qXsPajsihE1ZrDI/Rzjunv8ajIlwGxUw== 🔐%%";
    const ALPHA: &str = "🔐α 3GECY4iuvBjiQTZahoJh/o4fgZ6O/0ZhqtbP8fXm6ojD7F5vEYoZVkPr 🔐";
    const FIRST: &str = "%%🔐 7Am4Vg7oC9c2so79PU/FbRI38VDt4FMl 🔐%%";

    #[test]
    fn meld_encrypts_texts_are_read() {
        let beta = parse(BETA).expect("β");
        assert_eq!((beta.version, beta.visible), (2, false));
        assert_eq!(beta.hint, "the usual");
        assert_eq!(
            decrypt(&beta, "pässword").as_deref(),
            Some("Ünïcode secret\nsecond line")
        );
        assert_eq!(decrypt(&beta, "password"), None, "a wrong password");
        let alpha = parse(ALPHA).expect("α");
        assert_eq!((alpha.version, alpha.visible), (1, true));
        assert_eq!(decrypt(&alpha, "pw1").as_deref(), Some("alpha text"));
        let first = parse(FIRST).expect("the first version");
        assert_eq!(decrypt(&first, "pw0").as_deref(), Some("old text"));
        assert_eq!(parse("%%🔐β abc"), None, "no end");
        assert_eq!(parse("%%🔐β 💡hint abc 🔐%%"), None, "the hint not closed");
        assert_eq!(parse("plain"), None);
    }

    #[test]
    fn texts_are_encrypted_as_meld_encrypt_writes_them() {
        let written = encrypt("a secret", "pw", "the usual", false);
        assert!(written.starts_with("%%🔐β 💡the usual💡"), "{written}");
        assert!(written.ends_with(" 🔐%%"), "{written}");
        let secret = parse(&written).unwrap();
        assert_eq!(decrypt(&secret, "pw").as_deref(), Some("a secret"));
        // Salted: the same text twice isn't written the same.
        assert_ne!(written, encrypt("a secret", "pw", "the usual", false));
        let shown = encrypt("x", "pw", "", true);
        assert!(
            shown.starts_with("🔐β ") && shown.ends_with(" 🔐"),
            "{shown}"
        );
        assert!(!shown.contains(HINT), "no hint: no marks");
        // A hint can't hold its marks or a line break.
        let odd = parse(&encrypt("x", "pw", "a💡b\nc", false)).unwrap();
        assert_eq!(odd.hint, "ab c");
    }

    /// For checking by hand that Meld Encrypt reads what's written:
    /// `cargo test -q --lib meld_sample -- --ignored --nocapture`.
    #[test]
    #[ignore = "prints a sample"]
    fn meld_sample() {
        println!(
            "{}",
            encrypt("from blackglass\nline two", "pässword", "a hint", false)
        );
    }

    #[test]
    fn encrypted_texts_are_found_in_a_line() {
        let line = format!("a {BETA} b {ALPHA} c");
        let found = spans(&line);
        assert_eq!(found.len(), 2);
        assert_eq!(&line[found[0].0..found[0].1], BETA);
        assert_eq!(&line[found[1].0..found[1].1], ALPHA);
        assert!(spans("no secrets 🔐 here").is_empty());
    }
}
