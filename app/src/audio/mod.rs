//! Sound: a small mixer on the system's audio output, playing synthesised UI sounds
//! (see sfx.rs) and read-aloud recordings (Ogg Opus, decoded in pure Rust, with
//! pitch-keeping speed changes). The output stream opens on first use and closes again
//! after a few seconds of silence, so an idle app uses no audio resources.

pub mod readalong;
pub mod sfx;
pub mod stretch;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opus_pure::{OggOpusReader, Trim, MAX_PACKET_SAMPLES};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CLIP_RATE: f32 = 48000.0;

/// Decode an Ogg Opus file to mono samples at 48 kHz.
pub fn decode(bytes: &[u8]) -> Result<Vec<f32>, String> {
    let mut r = OggOpusReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let ch = r.head().channel_count as usize;
    let mut dec = r.head().decoder(48000).map_err(|e| e.to_string())?;
    let mut trim = Trim::new(r.head(), 48000, ch).map_err(|e| e.to_string())?;
    let mut block = vec![0f32; MAX_PACKET_SAMPLES * ch];
    let mut out = Vec::new();
    for pk in r.packets() {
        let pk = pk.map_err(|e| e.to_string())?;
        let n = dec.decode(&pk.data, MAX_PACKET_SAMPLES, &mut block).map_err(|e| e.to_string())?;
        let kept = trim.keep(&pk, &block[..n * ch]);
        if ch == 1 {
            out.extend_from_slice(kept);
        } else {
            out.extend(kept.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- the mixer

pub(crate) struct Clip {
    pub id: u64,
    pub pcm: Arc<Vec<f32>>,
    /// Stretched samples, made as they play.
    src: stretch::Stretcher,
    /// The two samples being interpolated between, and how far between them.
    a: f32,
    b: f32,
    frac: f64,
    /// Samples (48 kHz) taken from the stretcher so far.
    taken: u64,
    pub speed: f32,
    /// Original-time offset where this stretcher starts (seconds).
    pub start_offset: f64,
    pub paused: bool,
    pub done: bool,
}

impl Clip {
    fn new(id: u64, pcm: Arc<Vec<f32>>, from: usize, speed: f32) -> Clip {
        let mut src = stretch::Stretcher::new(pcm.clone(), from, speed, CLIP_RATE);
        let a = src.next().unwrap_or(0.0);
        let b = src.next().unwrap_or(0.0);
        Clip { id, pcm, src, a, b, frac: 0.0, taken: 2, speed, start_offset: from as f64 / CLIP_RATE as f64, paused: false, done: false }
    }
    /// Where playback is in the original recording (seconds).
    pub fn position(&self) -> f64 {
        self.start_offset + (self.taken as f64 - 2.0 + self.frac) / CLIP_RATE as f64 * self.speed as f64
    }
}

pub(crate) struct Mixer {
    pub rate: f32,
    pub voices: Vec<sfx::Voice>,
    pub clip: Option<Clip>,
    comp_gain: f32,
    pub last_sound: Instant,
}

impl Mixer {
    fn render(&mut self, out: &mut [f32], channels: usize) {
        let frames = out.len() / channels;
        let dt = 1.0 / self.rate;
        let step = CLIP_RATE as f64 / self.rate as f64;
        for f in 0..frames {
            let mut fx = 0.0;
            for v in &mut self.voices {
                fx += v.sample(dt);
            }
            fx *= sfx::MASTER;
            // A gentle compressor: threshold -18 dB, ratio 4.
            let level = fx.abs();
            let thr = 0.126;
            let target = if level > thr { (thr + (level - thr) / 4.0) / level } else { 1.0 };
            let k = if target < self.comp_gain { 0.02 } else { 0.0005 };
            self.comp_gain += (target - self.comp_gain) * k;
            let mut s = fx * self.comp_gain;
            if let Some(c) = &mut self.clip {
                if !c.paused && !c.done {
                    s += c.a + (c.b - c.a) * c.frac as f32;
                    c.frac += step;
                    while c.frac >= 1.0 {
                        c.frac -= 1.0;
                        c.a = c.b;
                        match c.src.next() {
                            Some(x) => {
                                c.b = x;
                                c.taken += 1;
                            }
                            None => {
                                c.done = true;
                                break;
                            }
                        }
                    }
                }
            }
            let s = s.clamp(-1.0, 1.0);
            for ch in 0..channels {
                out[f * channels + ch] = s;
            }
        }
        self.voices.retain(|v| !v.finished());
        if !self.voices.is_empty() || self.clip.as_ref().is_some_and(|c| !c.paused && !c.done) {
            self.last_sound = Instant::now();
        }
    }
}

pub struct Engine {
    mixer: Arc<Mutex<Mixer>>,
    stream: Option<cpal::Stream>,
    failed: bool,
    pub enabled: bool,
    next_clip: u64,
    last_played: std::collections::HashMap<&'static str, Instant>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            mixer: Arc::new(Mutex::new(Mixer { rate: 48000.0, voices: Vec::new(), clip: None, comp_gain: 1.0, last_sound: Instant::now() })),
            stream: None,
            failed: false,
            enabled: true,
            next_clip: 1,
            last_played: Default::default(),
        }
    }

    fn ensure_stream(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }
        if self.failed {
            return false;
        }
        let host = cpal::default_host();
        let Some(dev) = host.default_output_device() else {
            self.failed = true;
            return false;
        };
        let Ok(cfg) = dev.default_output_config() else {
            self.failed = true;
            return false;
        };
        let channels = cfg.channels() as usize;
        let rate = cfg.sample_rate() as f32;
        self.mixer.lock().unwrap().rate = rate;
        let mixer = self.mixer.clone();
        let err = |e| log::warn!("audio stream error: {e}");
        let config: cpal::StreamConfig = cfg.clone().into();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_output_stream(config.clone(), move |out: &mut [f32], _| mixer.lock().unwrap().render(out, channels), err, None),
            cpal::SampleFormat::I16 => {
                let mut buf = Vec::new();
                dev.build_output_stream(
                    config.clone(),
                    move |out: &mut [i16], _| {
                        buf.resize(out.len(), 0.0);
                        mixer.lock().unwrap().render(&mut buf, channels);
                        for (o, s) in out.iter_mut().zip(&buf) {
                            *o = (s * 32767.0) as i16;
                        }
                    },
                    err,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut buf = Vec::new();
                dev.build_output_stream(
                    config.clone(),
                    move |out: &mut [u16], _| {
                        buf.resize(out.len(), 0.0);
                        mixer.lock().unwrap().render(&mut buf, channels);
                        for (o, s) in out.iter_mut().zip(&buf) {
                            *o = ((s * 32767.0) + 32768.0) as u16;
                        }
                    },
                    err,
                    None,
                )
            }
            other => {
                log::warn!("audio format {other:?} not supported");
                self.failed = true;
                return false;
            }
        };
        match stream {
            Ok(s) => {
                if s.play().is_err() {
                    self.failed = true;
                    return false;
                }
                self.mixer.lock().unwrap().last_sound = Instant::now();
                self.stream = Some(s);
                true
            }
            Err(e) => {
                log::warn!("no audio output: {e}");
                self.failed = true;
                false
            }
        }
    }

    /// Call every frame: closes the output after a while of silence.
    pub fn tick(&mut self) {
        if self.stream.is_some() {
            let m = self.mixer.lock().unwrap();
            let idle = m.voices.is_empty() && m.clip.as_ref().is_none_or(|c| c.paused || c.done);
            let quiet_for = m.last_sound.elapsed();
            drop(m);
            if idle && quiet_for > Duration::from_secs(8) {
                self.stream = None;
            }
        }
    }

    /// Play a named UI sound.
    pub fn play(&mut self, name: &'static str) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if self.last_played.get(name).is_some_and(|t| now.duration_since(*t) < Duration::from_millis(45)) {
            return;
        }
        self.last_played.insert(name, now);
        let voices = sfx::sound(name);
        self.add(voices);
    }

    pub fn play_voices(&mut self, key: &'static str, voices: Vec<sfx::Voice>) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if self.last_played.get(key).is_some_and(|t| now.duration_since(*t) < Duration::from_millis(45)) {
            return;
        }
        self.last_played.insert(key, now);
        self.add(voices);
    }

    fn add(&mut self, voices: Vec<sfx::Voice>) {
        if voices.is_empty() || !self.ensure_stream() {
            return;
        }
        let mut m = self.mixer.lock().unwrap();
        m.voices.extend(voices);
        if m.voices.len() > 64 {
            let n = m.voices.len() - 64;
            m.voices.drain(..n);
        }
    }

    /// Start playing a recording (mono, 48 kHz) at `speed`. Returns its id.
    pub fn play_clip(&mut self, pcm: Arc<Vec<f32>>, speed: f32) -> Option<u64> {
        if !self.ensure_stream() {
            return None;
        }
        let id = self.next_clip;
        self.next_clip += 1;
        self.mixer.lock().unwrap().clip = Some(Clip::new(id, pcm, 0, speed));
        Some(id)
    }

    pub fn stop_clip(&mut self) {
        self.mixer.lock().unwrap().clip = None;
    }

    pub fn pause_clip(&mut self, paused: bool) {
        if !paused {
            self.ensure_stream();
        }
        if let Some(c) = &mut self.mixer.lock().unwrap().clip {
            c.paused = paused;
        }
    }

    /// Change the speed of the playing recording (from where it is now).
    pub fn set_clip_speed(&mut self, original: &Arc<Vec<f32>>, speed: f32) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(c) = &mut m.clip {
            let from = ((c.position() * CLIP_RATE as f64) as usize).min(original.len());
            let (id, paused) = (c.id, c.paused);
            *c = Clip::new(id, original.clone(), from, speed);
            c.paused = paused;
        }
    }

    /// (clip id, position in the original recording in seconds, finished, paused)
    pub fn clip_state(&self) -> Option<(u64, f64, bool, bool)> {
        let m = self.mixer.lock().unwrap();
        m.clip.as_ref().map(|c| (c.id, c.position(), c.done, c.paused))
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }
}
