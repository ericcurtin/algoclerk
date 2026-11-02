// SPDX-License-Identifier: Apache-2.0
//! Audio: procedural chiptune music and sound effects (text `.song` files rendered at runtime by
//! `algoclerk-media`). A missing or broken song is silently skipped; the game is fully playable muted.

use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use algoclerk_core::progress::Settings;
use algoclerk_media::Assets as AssetBundle;
use algoclerk_game::GameEvent;
use algoclerk_media::synth::{SAMPLE_RATE, Song, wav_bytes};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Component)]
pub struct MusicMarker;

#[derive(Resource)]
pub struct Sound {
    assets: Arc<AssetBundle>,
    cache: HashMap<String, Option<(Handle<AudioSource>, bool)>>,
    music: Option<Entity>,
    music_name: Option<String>,
}

fn gain(v: u8) -> f32 {
    let x = v.min(10) as f32 / 10.0;
    x * x * 0.7
}

impl Sound {
    pub fn new(assets: Arc<AssetBundle>) -> Sound {
        Sound {
            assets,
            cache: HashMap::new(),
            music: None,
            music_name: None,
        }
    }

    /// Render (once) and cache a song / sfx by name. Returns the handle and whether it loops.
    fn song(&mut self, name: &str, sources: &mut Assets<AudioSource>) -> Option<(Handle<AudioSource>, bool)> {
        if let Some(c) = self.cache.get(name) {
            return c.clone();
        }
        let entry = self
            .assets
            .text(&format!("music/{name}.song"))
            .and_then(|t| Song::parse(t).ok())
            .map(|s| {
                let wav = wav_bytes(&s.render(), SAMPLE_RATE);
                (sources.add(AudioSource { bytes: wav.into() }), s.looped)
            });
        self.cache.insert(name.to_string(), entry.clone());
        entry
    }

    pub fn handle(
        &mut self,
        ev: &GameEvent,
        commands: &mut Commands,
        sources: &mut Assets<AudioSource>,
        settings: &Settings,
    ) {
        match ev {
            GameEvent::Sfx(name) => {
                if settings.sfx_volume == 0 || !settings.sound {
                    return;
                }
                if let Some((h, _)) = self.song(name, sources) {
                    commands.spawn((
                        AudioPlayer::new(h),
                        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain(settings.sfx_volume))),
                    ));
                }
            }
            GameEvent::Music(name) => {
                if let Some(e) = self.music.take() {
                    commands.entity(e).despawn();
                }
                self.music_name = name.map(str::to_string);
                if let Some(n) = name
                    && let Some((h, looped)) = self.song(n, sources)
                {
                    let base = if looped {
                        PlaybackSettings::LOOP
                    } else {
                        PlaybackSettings::DESPAWN
                    };
                    let e = commands
                        .spawn((
                            AudioPlayer::new(h),
                            base.with_volume(Volume::Linear(gain(settings.music_volume))),
                            MusicMarker,
                        ))
                        .id();
                    self.music = Some(e);
                }
            }
            _ => {}
        }
    }

    pub fn apply_volume(&self, sinks: &mut Query<&mut AudioSink, With<MusicMarker>>, settings: &Settings) {
        for mut s in sinks.iter_mut() {
            s.set_volume(Volume::Linear(gain(settings.music_volume)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_is_monotonic_and_bounded() {
        assert_eq!(gain(0), 0.0);
        assert!(gain(5) < gain(6));
        assert!(gain(10) <= 0.7 + 1e-6);
        assert_eq!(gain(200), gain(10));
    }
}
