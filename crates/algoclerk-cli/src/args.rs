// SPDX-License-Identifier: Apache-2.0
//! A very small argument parser (flags, `--key value`, positionals).

use std::collections::HashMap;

/// Parsed arguments of one subcommand.
#[derive(Debug, Default)]
pub struct Args {
    /// `--key value` options.
    pub opts: HashMap<String, String>,
    /// `--flag` switches.
    pub flags: Vec<String>,
    /// Everything else.
    pub positional: Vec<String>,
}

impl Args {
    /// Parse. `value_opts` lists the options that take a value; every other `--x` is a flag.
    pub fn parse(argv: &[String], value_opts: &[&str]) -> Result<Args, String> {
        let mut a = Args::default();
        let mut i = 0;
        while i < argv.len() {
            let s = &argv[i];
            if let Some(name) = s.strip_prefix("--") {
                if let Some((k, v)) = name.split_once('=') {
                    if !value_opts.contains(&k) {
                        return Err(format!("option --{k} does not take a value"));
                    }
                    a.opts.insert(k.to_string(), v.to_string());
                } else if value_opts.contains(&name) {
                    i += 1;
                    let v = argv
                        .get(i)
                        .ok_or_else(|| format!("option --{name} needs a value"))?;
                    a.opts.insert(name.to_string(), v.clone());
                } else {
                    a.flags.push(name.to_string());
                }
            } else {
                a.positional.push(s.clone());
            }
            i += 1;
        }
        Ok(a)
    }

    /// Whether a flag was given.
    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    /// An option value.
    pub fn opt(&self, name: &str) -> Option<&str> {
        self.opts.get(name).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_mixed() {
        let a = Args::parse(
            &v(&["x", "--level", "echo", "--json", "--lang=c", "y"]),
            &["level", "lang"],
        )
        .unwrap();
        assert_eq!(a.opt("level"), Some("echo"));
        assert_eq!(a.opt("lang"), Some("c"));
        assert!(a.flag("json"));
        assert_eq!(a.positional, ["x", "y"]);
    }

    #[test]
    fn missing_value_is_error() {
        assert!(Args::parse(&v(&["--level"]), &["level"]).is_err());
        assert!(Args::parse(&v(&["--json=1"]), &[]).is_err());
    }
}
