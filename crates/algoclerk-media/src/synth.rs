// SPDX-License-Identifier: Apache-2.0
//! Procedural chiptune synthesiser. Songs and sound effects are plain text (`assets/music/*.song`):
//!
//! ```text
//! # comment
//! name: town
//! bpm: 112              # a "step" is a sixteenth note
//! loop: yes
//! lead = square 0.5 : E5:2 G5:2 A5:4 -:2      # NOTE:STEPS ; '-' or '.' is a rest
//! bass = triangle 0.8 : A2:4 E3:4
//! perc = noise 0.3 : x:1 .:1 h:1 .:1          # x = noise hit, h = short hat
//! blip = square 0.4 : C5>C6:2                 # NOTE>NOTE:STEPS is a pitch slide
//! ```
//!
//! Output is 16-bit mono PCM at [`SAMPLE_RATE`], wrapped into a WAV by [`wav_bytes`].
//! Nothing here is derived from any third-party recording or composition.
//!
//! The synthesiser design (text song format, voices, WAV output) comes from the sibling project
//! Falabichos (Apache-2.0, same owner); every song in `assets/music` is new.

pub const SAMPLE_RATE: u32 = 22_050;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Wave {
    Square(f32),
    Triangle,
    Saw,
    Noise,
}

#[derive(Debug, Clone)]
struct Event {
    /// Start/end frequency in Hz (0 = rest). For noise: 1.0 = hit, 2.0 = hat.
    f0: f32,
    f1: f32,
    steps: u32,
}

#[derive(Debug, Clone)]
struct Channel {
    wave: Wave,
    vol: f32,
    events: Vec<Event>,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub name: String,
    pub bpm: f32,
    pub looped: bool,
    channels: Vec<Channel>,
}

/// Strip a trailing `# comment` (a `#` at line start or after whitespace; `C#4` is a note).
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

pub fn note_freq(name: &str) -> Option<f32> {
    let mut cs = name.chars();
    let letter = cs.next()?;
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = cs.collect();
    let (acc, oct) = match rest.chars().next()? {
        '#' => (1, &rest[1..]),
        'b' => (-1, &rest[1..]),
        _ => (0, &rest[..]),
    };
    let oct: i32 = oct.parse().ok()?;
    let midi = 12 * (oct + 1) + base + acc;
    Some(440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0))
}

impl Song {
    pub fn parse(text: &str) -> Result<Song, String> {
        let mut song = Song {
            name: String::new(),
            bpm: 120.0,
            looped: true,
            channels: vec![],
        };
        for (n, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let ln = n + 1;
            if let Some((lhs, rhs)) = line.split_once(" = ") {
                let mut spec = rhs.splitn(2, ':');
                let head = spec.next().unwrap_or("");
                let body = spec
                    .next()
                    .ok_or(format!("line {ln}: missing `:` before notes"))?;
                let mut h = head.split_whitespace();
                let wave = match h.next() {
                    Some("square") => Wave::Square(0.5),
                    Some("pulse") => Wave::Square(0.25),
                    Some("triangle") => Wave::Triangle,
                    Some("saw") => Wave::Saw,
                    Some("noise") => Wave::Noise,
                    other => return Err(format!("line {ln}: unknown wave {other:?}")),
                };
                let vol: f32 = h
                    .next()
                    .unwrap_or("0.5")
                    .parse()
                    .map_err(|_| format!("line {ln}: bad volume"))?;
                let mut events = vec![];
                for tok in body.split_whitespace() {
                    let (note, len) = tok.split_once(':').unwrap_or((tok, "1"));
                    let steps: u32 = len
                        .parse()
                        .map_err(|_| format!("line {ln}: bad length in `{tok}`"))?;
                    if steps == 0 {
                        return Err(format!("line {ln}: zero length in `{tok}`"));
                    }
                    let (f0, f1) = if matches!(wave, Wave::Noise) {
                        match note {
                            "x" => (1.0, 1.0),
                            "h" => (2.0, 2.0),
                            "-" | "." => (0.0, 0.0),
                            _ => return Err(format!("line {ln}: noise token `{note}` (use x, h, .)")),
                        }
                    } else if note == "-" || note == "." {
                        (0.0, 0.0)
                    } else if let Some((a, b)) = note.split_once('>') {
                        (
                            note_freq(a).ok_or(format!("line {ln}: bad note `{a}`"))?,
                            note_freq(b).ok_or(format!("line {ln}: bad note `{b}`"))?,
                        )
                    } else {
                        let f = note_freq(note).ok_or(format!("line {ln}: bad note `{note}`"))?;
                        (f, f)
                    };
                    events.push(Event { f0, f1, steps });
                }
                if events.is_empty() {
                    return Err(format!("line {ln}: channel `{}` has no notes", lhs.trim()));
                }
                song.channels.push(Channel { wave, vol, events });
            } else if let Some((k, v)) = line.split_once(':') {
                match k.trim() {
                    "name" => song.name = v.trim().to_string(),
                    "bpm" => song.bpm = v.trim().parse().map_err(|_| format!("line {ln}: bad bpm"))?,
                    "loop" => song.looped = matches!(v.trim(), "yes" | "true"),
                    other => return Err(format!("line {ln}: unknown header `{other}`")),
                }
            } else {
                return Err(format!("line {ln}: cannot parse `{line}`"));
            }
        }
        if song.channels.is_empty() {
            return Err("song has no channels".into());
        }
        Ok(song)
    }

    pub fn total_steps(&self) -> u32 {
        self.channels
            .iter()
            .map(|c| c.events.iter().map(|e| e.steps).sum::<u32>())
            .max()
            .unwrap_or(0)
    }

    pub fn duration_secs(&self) -> f32 {
        self.total_steps() as f32 * 60.0 / self.bpm / 4.0
    }

    /// Render to 16-bit mono PCM.
    pub fn render(&self) -> Vec<i16> {
        let step_secs = 60.0 / self.bpm / 4.0;
        let step_samples = (step_secs * SAMPLE_RATE as f32) as usize;
        let total = self.total_steps() as usize * step_samples;
        let mut mix = vec![0f32; total];
        let mut rng: u32 = 0x1234_5678;
        for ch in &self.channels {
            let mut pos = 0usize; // sample position
            let mut phase = 0f32;
            let mut lfsr_val = 0f32;
            let mut ev_idx = 0usize;
            while pos < total {
                let ev = &ch.events[ev_idx % ch.events.len()];
                let len = ev.steps as usize * step_samples;
                for i in 0..len {
                    if pos + i >= total {
                        break;
                    }
                    let t = i as f32 / len as f32;
                    let secs = i as f32 / SAMPLE_RATE as f32;
                    // envelope: quick attack, decay to sustain, short release
                    let attack = (secs / 0.004).min(1.0);
                    let release = (((len - i) as f32 / SAMPLE_RATE as f32) / 0.012).min(1.0);
                    let env = match ch.wave {
                        Wave::Noise => (1.0 - secs / if ev.f0 > 1.5 { 0.04 } else { 0.12 }).max(0.0),
                        _ => (0.55 + 0.45 * (-secs * 9.0).exp()) * attack * release,
                    };
                    let s = if ev.f0 == 0.0 {
                        0.0
                    } else {
                        match ch.wave {
                            Wave::Noise => {
                                if i % 3 == 0 {
                                    rng ^= rng << 13;
                                    rng ^= rng >> 17;
                                    rng ^= rng << 5;
                                    lfsr_val = if rng & 1 == 1 { 1.0 } else { -1.0 };
                                }
                                lfsr_val
                            }
                            w => {
                                let f = ev.f0 + (ev.f1 - ev.f0) * t;
                                phase = (phase + f / SAMPLE_RATE as f32).fract();
                                match w {
                                    Wave::Square(d) => {
                                        if phase < d {
                                            1.0
                                        } else {
                                            -1.0
                                        }
                                    }
                                    Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                                    Wave::Saw => 2.0 * phase - 1.0,
                                    Wave::Noise => 0.0,
                                }
                            }
                        }
                    };
                    mix[pos + i] += s * env * ch.vol;
                }
                pos += len;
                ev_idx += 1;
            }
        }
        let peak = mix.iter().fold(0f32, |m, v| m.max(v.abs())).max(1.0);
        mix.iter()
            .map(|v| ((v / peak) * 0.8 * i16::MAX as f32) as i16)
            .collect()
    }
}

/// Wrap mono 16-bit PCM in a WAV container.
pub fn wav_bytes(pcm: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + pcm.len() * 2);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONG: &str = "name: t\nbpm: 120\nlead = square 0.5 : C4:2 E4:2 -:2 G4>C5:2\nbass = triangle 0.7 : C2:4\nperc = noise 0.3 : x:1 .:1 h:1 .:1\n";

    #[test]
    fn note_frequencies() {
        assert!((note_freq("A4").unwrap() - 440.0).abs() < 0.01);
        assert!((note_freq("A5").unwrap() - 880.0).abs() < 0.01);
        assert!(note_freq("C#4").unwrap() > note_freq("C4").unwrap());
        assert!(note_freq("Bb3").unwrap() < note_freq("B3").unwrap());
        assert!(note_freq("H4").is_none());
    }

    #[test]
    fn renders_non_silent_audio_of_expected_length() {
        let s = Song::parse(SONG).unwrap();
        assert_eq!(s.total_steps(), 8);
        let pcm = s.render();
        let expect = (s.duration_secs() * SAMPLE_RATE as f32) as usize;
        assert!((pcm.len() as i64 - expect as i64).abs() < 64);
        assert!(pcm.iter().any(|&x| x.abs() > 1000), "not silent");
    }

    #[test]
    fn wav_header_is_valid() {
        let w = wav_bytes(&[0, 1, -1, 5], 22050);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes([w[40], w[41], w[42], w[43]]), 8);
        assert_eq!(w.len(), 44 + 8);
    }

    #[test]
    fn parse_errors() {
        assert!(Song::parse("lead = sine 0.5 : C4").is_err());
        assert!(Song::parse("lead = square 0.5 : Z9:1").is_err());
        assert!(Song::parse("name: x").is_err());
        assert!(Song::parse("lead = square 0.5 : C4:0").is_err());
    }
}
