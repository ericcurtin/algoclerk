// SPDX-License-Identifier: Apache-2.0
//! Optional sound effects, synthesised in code (no audio files). Enabled with the `audio` feature.

use std::cell::Cell;

/// The sound effects of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Click,
    Error,
    Win,
}

/// Plays effects through the default output device (silently does nothing without the `audio` feature).
pub struct Sound {
    enabled: Cell<bool>,
    #[cfg(feature = "audio")]
    inner: Option<(rodio::MixerDeviceSink, rodio::Player)>,
}

impl std::fmt::Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound")
            .field("enabled", &self.enabled.get())
            .finish_non_exhaustive()
    }
}

/// `(frequency in Hz, duration in ms)` notes of an effect.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub fn notes(sfx: Sfx) -> &'static [(f32, u32)] {
    match sfx {
        Sfx::Click => &[(880.0, 35)],
        Sfx::Error => &[(260.0, 110), (196.0, 180)],
        Sfx::Win => &[(523.25, 90), (659.25, 90), (783.99, 90), (1046.5, 240)],
    }
}

/// Render notes into mono float samples.
#[cfg_attr(not(feature = "audio"), allow(dead_code))]
pub fn synthesize(notes: &[(f32, u32)], sample_rate: u32) -> Vec<f32> {
    let mut out = Vec::new();
    for &(freq, ms) in notes {
        let n = (sample_rate as u64 * u64::from(ms) / 1000) as usize;
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            let env = (1.0 - i as f32 / n.max(1) as f32)
                .powf(1.5)
                .min((i as f32 / 120.0).min(1.0));
            let wave = (std::f32::consts::TAU * freq * t).sin() * 0.6
                + (std::f32::consts::TAU * freq * 2.0 * t).sin() * 0.15;
            out.push(wave * env * 0.35);
        }
    }
    out
}

impl Sound {
    /// Open the default audio output (if the feature is on and a device exists).
    pub fn new() -> Sound {
        Sound {
            enabled: Cell::new(true),
            #[cfg(feature = "audio")]
            inner: open(),
        }
    }

    /// Turn sound on or off.
    pub fn set_enabled(&self, on: bool) {
        self.enabled.set(on);
    }

    /// Play an effect.
    pub fn play(&self, sfx: Sfx) {
        if !self.enabled.get() {
            return;
        }
        #[cfg(feature = "audio")]
        if let Some((_, player)) = &self.inner {
            let samples = synthesize(notes(sfx), 44_100);
            let ch = std::num::NonZero::new(1u16);
            let sr = std::num::NonZero::new(44_100u32);
            if let (Some(ch), Some(sr)) = (ch, sr) {
                player.append(rodio::buffer::SamplesBuffer::new(ch, sr, samples));
            }
        }
        #[cfg(not(feature = "audio"))]
        let _ = sfx;
    }
}

impl Default for Sound {
    fn default() -> Self {
        Sound::new()
    }
}

#[cfg(feature = "audio")]
fn open() -> Option<(rodio::MixerDeviceSink, rodio::Player)> {
    // The device sink must stay alive for as long as sounds should play.
    let device = rodio::DeviceSinkBuilder::open_default_sink().ok()?;
    let player = rodio::Player::connect_new(device.mixer());
    Some((device, player))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_synthesise_bounded_samples() {
        for sfx in [Sfx::Click, Sfx::Error, Sfx::Win] {
            let s = synthesize(notes(sfx), 22_050);
            assert!(!s.is_empty());
            assert!(s.iter().all(|v| v.abs() <= 1.0));
        }
    }

    #[test]
    fn disabled_sound_is_silent_and_safe() {
        let s = Sound::new();
        s.set_enabled(false);
        s.play(Sfx::Win);
    }
}
