// SPDX-License-Identifier: Apache-2.0
//! Command-line flags (desktop; harmless on mobile where there are none).

use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    /// Exit automatically after this many seconds (CI boot test).
    pub autoquit: Option<f32>,
    /// Feed scripted pointer clicks and keys (title -> level -> run) for smoke tests.
    pub smoke: bool,
    /// Save a screenshot of the real window (GPU-rendered) to this PNG path, after `window_shot_after` s.
    pub window_shot: Option<PathBuf>,
    pub window_shot_after: f32,
    /// Touch-first sizing even on desktop.
    pub touch: bool,
    pub data_dir: Option<PathBuf>,
    pub assets_dir: Option<PathBuf>,
    pub fullscreen: bool,
    pub help: bool,
    pub version: bool,
}

pub const HELP: &str = "algoclerk [options]\n\
  --autoquit SECS       quit automatically after SECS seconds\n\
  --smoke               inject scripted clicks and keys (for CI boot tests)\n\
  --window-shot FILE    save a PNG of the real window after --window-shot-after SECS (default 8)\n\
  --touch               touch-first sizing\n\
  --fullscreen          start fullscreen\n\
  --data-dir DIR        where saves and settings are stored\n\
  --assets DIR          load assets from DIR instead of the embedded copy (modding)\n\
  --version, --help\n";

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        window_shot_after: 8.0,
        ..Args::default()
    };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--autoquit" => {
                a.autoquit = Some(
                    value("--autoquit")?
                        .parse()
                        .map_err(|_| "--autoquit needs a number")?,
                )
            }
            "--smoke" => a.smoke = true,
            "--window-shot" => a.window_shot = Some(PathBuf::from(value("--window-shot")?)),
            "--window-shot-after" => {
                a.window_shot_after = value("--window-shot-after")?
                    .parse()
                    .map_err(|_| "needs a number")?
            }
            "--touch" => a.touch = true,
            "--fullscreen" => a.fullscreen = true,
            "--data-dir" => a.data_dir = Some(PathBuf::from(value("--data-dir")?)),
            "--assets" => a.assets_dir = Some(PathBuf::from(value("--assets")?)),
            "--help" | "-h" => a.help = true,
            "--version" | "-V" => a.version = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &[&str]) -> Result<Args, String> {
        parse(s.iter().map(|x| x.to_string()))
    }

    #[test]
    fn parses_flags_and_values() {
        let a = p(&["--smoke", "--autoquit", "3.5", "--data-dir", "/tmp/x"]).unwrap();
        assert!(a.smoke);
        assert_eq!(a.autoquit, Some(3.5));
        assert_eq!(a.data_dir, Some(PathBuf::from("/tmp/x")));
        assert!(p(&["--bogus"]).is_err());
        assert!(p(&["--autoquit"]).is_err());
    }
}
