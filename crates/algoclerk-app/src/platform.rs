// SPDX-License-Identifier: Apache-2.0
//! Per-platform glue: data directory and safe-area insets.

use crate::cli::Args;
use std::path::PathBuf;

pub fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

/// Where saves and settings live: `--data-dir`, then the platform default.
pub fn data_dir(args: &Args) -> PathBuf {
    if let Some(d) = &args.data_dir {
        return d.clone();
    }
    #[cfg(target_os = "android")]
    {
        if let Some(p) = bevy::android::ANDROID_APP.get().and_then(|a| a.internal_data_path()) {
            return p;
        }
    }
    algoclerk_game::default_data_dir()
}

/// Safe-area insets `(left, top, right, bottom)` in logical pixels. winit 0.30 has no safe-area query, so
/// phones use conservative defaults; `ALGOCLERK_SAFE_INSETS=l,t,r,b` overrides them.
pub fn safe_insets() -> (f32, f32, f32, f32) {
    if let Ok(v) = std::env::var("ALGOCLERK_SAFE_INSETS") {
        let n: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if n.len() == 4 {
            return (n[0], n[1], n[2], n[3]);
        }
    }
    if cfg!(target_os = "ios") {
        (0.0, 47.0, 0.0, 21.0)
    } else if cfg!(target_os = "android") {
        (0.0, 28.0, 0.0, 12.0)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}
