// SPDX-License-Identifier: Apache-2.0
//! Localisation-ready strings.
//!
//! All player-visible text from the core goes through message keys that are looked up in
//! translation catalogues (`lang/<code>.toml`). English is the fallback for every key.
//! See `docs/TRANSLATING.md`.

use std::collections::HashMap;
use std::sync::OnceLock;

#[allow(missing_docs)]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/locales.rs"));
}
use embedded::LOCALES;

/// A flat key -> text catalogue.
#[derive(Clone, Debug, Default)]
pub struct Strings {
    map: HashMap<String, String>,
}

fn flatten(prefix: &str, v: &toml::Value, out: &mut HashMap<String, String>) {
    match v {
        toml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        toml::Value::Table(t) => {
            for (k, v) in t {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, out);
            }
        }
        _ => {}
    }
}

impl Strings {
    /// Parse a catalogue from TOML text. Nested tables become dotted keys.
    pub fn from_toml(text: &str) -> Result<Strings, String> {
        let v: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        flatten("", &v, &mut map);
        Ok(Strings { map })
    }

    /// Look a key up; returns `None` when this catalogue lacks it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    /// All keys (sorted), for translation tooling and tests.
    pub fn keys(&self) -> Vec<&str> {
        let mut k: Vec<&str> = self.map.keys().map(String::as_str).collect();
        k.sort_unstable();
        k
    }

    /// Format a message, substituting `{name}` placeholders. Falls back to English, then to the key.
    pub fn format(&self, key: &str, args: &[(&str, String)]) -> String {
        let template = self
            .get(key)
            .or_else(|| english().get(key))
            .unwrap_or(key);
        let mut out = template.to_string();
        for (k, v) in args {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }

    /// Format a message without arguments.
    pub fn t(&self, key: &str) -> String {
        self.format(key, &[])
    }
}

/// The built-in English catalogue.
pub fn english() -> &'static Strings {
    static EN: OnceLock<Strings> = OnceLock::new();
    EN.get_or_init(|| {
        let text = LOCALES
            .iter()
            .find(|(c, _)| *c == "en")
            .map(|(_, t)| *t)
            .unwrap_or_default();
        Strings::from_toml(text).unwrap_or_default()
    })
}

/// Language codes that ship with the binary.
pub fn available_languages() -> Vec<&'static str> {
    LOCALES.iter().map(|(c, _)| *c).collect()
}

/// Load a shipped catalogue by language code (`"en"`, ...).
pub fn load_language(code: &str) -> Option<Strings> {
    LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, t)| Strings::from_toml(t).ok())
}

/// Errors and problems that can be rendered in any language.
pub trait Diagnostic {
    /// Message key in the catalogue.
    fn key(&self) -> &'static str;
    /// Placeholder values.
    fn args(&self) -> Vec<(&'static str, String)>;
    /// Render with a catalogue.
    fn message(&self, strings: &Strings) -> String {
        let args = self.args();
        let borrowed: Vec<(&str, String)> = args.iter().map(|(k, v)| (*k, v.clone())).collect();
        strings.format(self.key(), &borrowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_loads() {
        assert!(english().get("run.empty_hand").is_some());
    }

    #[test]
    fn format_substitutes_and_falls_back() {
        let s = Strings::from_toml("[a]\nb = \"x {n} y\"").unwrap();
        assert_eq!(s.format("a.b", &[("n", "5".into())]), "x 5 y");
        assert_eq!(s.format("run.empty_tile", &[("slot", "3".into())]).contains('3'), true);
        assert_eq!(s.format("no.such.key", &[]), "no.such.key");
    }
}
