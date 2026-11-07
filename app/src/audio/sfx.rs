//! FreeSheet's sound effects: small, soft UI sounds synthesised on the fly (no audio
//! files), as in 1.1's Web Audio version. Every sound is built from three voices:
//!   bell - a sine with a couple of quieter, faster-fading overtones (chimes)
//!   blip - a short sine or triangle with an optional pitch glide (ticks, pops)
//!   air  - band-passed noise with a moving centre (taps, whooshes)
//! Notes come from C major pentatonic, so sounds that overlap never clash.

pub const MASTER: f32 = 0.9;

/// Note name -> frequency, e.g. "C6".
pub fn hz(n: &str) -> f32 {
    let semi = match &n[..1] {
        "C" => -9,
        "D" => -7,
        "E" => -5,
        "F" => -4,
        "G" => -2,
        "A" => 0,
        _ => 2,
    };
    let oct: i32 = n[1..].parse().unwrap_or(4);
    440.0 * 2f32.powf((semi + (oct - 4) * 12) as f32 / 12.0)
}

/// Gain envelope: from 0.0001 up to `peak` over `attack`, then down to 0.0001 over `decay`
/// (exponential ramps, as Web Audio's exponentialRampToValueAtTime).
#[derive(Clone, Copy)]
struct Env {
    delay: f32,
    peak: f32,
    attack: f32,
    decay: f32,
}

impl Env {
    fn at(&self, t: f32) -> f32 {
        let t = t - self.delay;
        if t < 0.0 {
            return 0.0;
        }
        let lo: f32 = 0.0001;
        if t < self.attack {
            lo * (self.peak / lo).powf(t / self.attack)
        } else if t < self.attack + self.decay {
            self.peak * (lo / self.peak).powf((t - self.attack) / self.decay)
        } else {
            0.0
        }
    }
    fn end(&self) -> f32 {
        self.delay + self.attack + self.decay + 0.05
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Wave {
    Sine,
    Triangle,
}

pub enum Voice {
    Osc { wave: Wave, f1: f32, f2: f32, len: f32, env: Env, t: f32, phase: f32 },
    Air { f1: f32, f2: f32, len: f32, q: f32, delay: f32, vol: f32, t: f32, rng: u32, z1: f32, z2: f32 },
}

impl Voice {
    pub fn sample(&mut self, dt: f32) -> f32 {
        match self {
            Voice::Osc { wave, f1, f2, len, env, t, phase } => {
                let local = (*t - env.delay).max(0.0);
                let f = if *f2 != *f1 && local < *len { *f1 * (*f2 / *f1).powf(local / *len) } else if local >= *len { *f2 } else { *f1 };
                let s = if *t >= env.delay {
                    let x = *phase;
                    let w = match wave {
                        Wave::Sine => (x * std::f32::consts::TAU).sin(),
                        Wave::Triangle => 1.0 - 4.0 * (x - (x + 0.5).floor()).abs(),
                    };
                    *phase = (*phase + f * dt).fract();
                    w * env.at(*t)
                } else {
                    0.0
                };
                *t += dt;
                s
            }
            Voice::Air { f1, f2, len, q, delay, vol, t, rng, z1, z2 } => {
                let local = *t - *delay;
                *t += dt;
                if local < 0.0 {
                    return 0.0;
                }
                // White noise.
                *rng ^= *rng << 13;
                *rng ^= *rng >> 17;
                *rng ^= *rng << 5;
                let n = (*rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
                // Band-pass biquad (RBJ), centre gliding from f1 to f2.
                let f = if local < *len && *f2 != *f1 { *f1 * (*f2 / *f1).powf(local / *len) } else { *f2 };
                let w0 = std::f32::consts::TAU * f * dt;
                let alpha = w0.sin() / (2.0 * *q);
                let a0 = 1.0 + alpha;
                let b0 = alpha / a0;
                let a1 = -2.0 * w0.cos() / a0;
                let a2 = (1.0 - alpha) / a0;
                // Transposed direct form II (b1 = 0, b2 = -b0).
                let y = b0 * n + *z1;
                *z1 = -a1 * y + *z2;
                *z2 = -b0 * n - a2 * y;
                let lo: f32 = 0.0001;
                let rise = *len * 0.35;
                let g = if local < rise { lo * (*vol / lo).powf(local / rise) } else if local < *len { *vol * (lo / *vol).powf((local - rise) / (*len - rise)) } else { 0.0 };
                y * g
            }
        }
    }

    pub fn finished(&self) -> bool {
        match self {
            Voice::Osc { env, t, .. } => *t > env.end(),
            Voice::Air { len, delay, t, .. } => *t > *delay + *len + 0.05,
        }
    }
}

fn seed() -> u32 {
    fastrand::u32(1..u32::MAX)
}

pub fn bell(out: &mut Vec<Voice>, f: f32, at: f32, vol: f32, decay: f32) {
    for (mult, amp, len) in [(1.0, 1.0, 1.0), (2.0, 0.18, 0.5), (3.01, 0.07, 0.3)] {
        out.push(Voice::Osc { wave: Wave::Sine, f1: f * mult, f2: f * mult, len: decay * len, env: Env { delay: at, peak: vol * amp, attack: 0.004, decay: decay * len }, t: 0.0, phase: 0.0 });
    }
}

pub fn blip(out: &mut Vec<Voice>, f1: f32, f2: f32, at: f32, vol: f32, len: f32, wave: Wave) {
    out.push(Voice::Osc { wave, f1, f2, len, env: Env { delay: at, peak: vol, attack: 0.003, decay: len }, t: 0.0, phase: 0.0 });
}

pub fn air(out: &mut Vec<Voice>, f1: f32, f2: f32, at: f32, vol: f32, len: f32, q: f32) {
    out.push(Voice::Air { f1, f2, len, q, delay: at, vol, t: 0.0, rng: seed(), z1: 0.0, z2: 0.0 });
}

use Wave::*;

pub fn sound(name: &str) -> Vec<Voice> {
    let mut v = Vec::new();
    let o = &mut v;
    match name {
        "tap" => {
            air(o, 3200.0, 2600.0, 0.0, 0.15, 0.03, 2.0);
            blip(o, 2400.0, 1800.0, 0.0, 0.036, 0.018, Sine);
        }
        "select" => {
            blip(o, hz("A5"), hz("E5"), 0.0, 0.1, 0.06, Sine);
            air(o, 4000.0, 3000.0, 0.0, 0.04, 0.03, 3.0);
        }
        "toggleOn" => {
            blip(o, hz("G5"), hz("G5"), 0.0, 0.04, 0.05, Sine);
            blip(o, hz("C6"), hz("C6"), 0.06, 0.045, 0.07, Sine);
        }
        "toggleOff" => {
            blip(o, hz("C6"), hz("C6"), 0.0, 0.04, 0.05, Sine);
            blip(o, hz("G5"), hz("G5"), 0.06, 0.035, 0.07, Sine);
        }
        "push" => {
            air(o, 500.0, 1900.0, 0.0, 0.07, 0.24, 0.9);
            blip(o, hz("E5"), hz("A5"), 0.02, 0.036, 0.16, Sine);
        }
        "pop" => {
            air(o, 1900.0, 500.0, 0.0, 0.064, 0.22, 0.9);
            blip(o, hz("A5"), hz("E5"), 0.02, 0.032, 0.15, Sine);
        }
        "tab" => {
            air(o, 1200.0, 1500.0, 0.0, 0.055, 0.12, 1.4);
            blip(o, hz("D5"), hz("D5"), 0.0, 0.05, 0.06, Sine);
        }
        "swap" => {
            air(o, 1400.0, 1700.0, 0.0, 0.045, 0.1, 1.6);
            blip(o, hz("G5"), hz("G5"), 0.0, 0.025, 0.05, Sine);
        }
        "reveal" => {
            air(o, 700.0, 2400.0, 0.0, 0.025, 0.28, 0.8);
            bell(o, hz("E6"), 0.08, 0.025, 0.4);
        }
        "correct" => {
            bell(o, hz("C6"), 0.0, 0.11, 0.5);
            bell(o, hz("G6"), 0.085, 0.1, 0.7);
        }
        "partial" => {
            bell(o, hz("A5"), 0.0, 0.08, 0.45);
            bell(o, hz("C6"), 0.08, 0.06, 0.5);
        }
        "wrong" => {
            blip(o, hz("E3"), hz("C3"), 0.0, 0.09, 0.16, Triangle);
            blip(o, hz("C3"), hz("A2"), 0.12, 0.07, 0.2, Triangle);
        }
        "complete" => {
            for (i, n) in ["C5", "E5", "G5", "C6", "E6"].iter().enumerate() {
                bell(o, hz(n), i as f32 * 0.07, 0.07, 0.8);
            }
        }
        "finishSoft" => {
            for (i, n) in ["G4", "C5", "E5"].iter().enumerate() {
                bell(o, hz(n), i as f32 * 0.09, 0.06, 0.7);
            }
        }
        "streak" => {
            for (i, n) in ["C5", "E5", "G5", "C6", "E6", "G6"].iter().enumerate() {
                bell(o, hz(n), i as f32 * 0.06, 0.065, 0.9);
            }
            air(o, 5000.0, 9000.0, 0.25, 0.02, 0.5, 0.7);
        }
        "open" => {
            blip(o, hz("E5"), hz("A5"), 0.0, 0.08, 0.09, Sine);
            air(o, 900.0, 1800.0, 0.0, 0.03, 0.1, 1.2);
        }
        "close" => blip(o, hz("A5"), hz("E5"), 0.0, 0.07, 0.08, Sine),
        "toast" => {
            bell(o, hz("E6"), 0.0, 0.045, 0.4);
            bell(o, hz("A6"), 0.07, 0.035, 0.45);
        }
        "error" => {
            blip(o, hz("D4"), hz("C4"), 0.0, 0.06, 0.12, Triangle);
            blip(o, hz("C4"), hz("A3"), 0.11, 0.05, 0.14, Triangle);
        }
        "examStart" => {
            bell(o, hz("C5"), 0.0, 0.06, 0.8);
            bell(o, hz("G5"), 0.0, 0.045, 0.8);
            bell(o, hz("C6"), 0.12, 0.06, 0.9);
        }
        "submit" => {
            bell(o, hz("G5"), 0.0, 0.06, 0.6);
            bell(o, hz("D6"), 0.09, 0.055, 0.7);
        }
        "timeWarning" => {
            bell(o, hz("A5"), 0.0, 0.08, 0.5);
            bell(o, hz("A5"), 0.22, 0.08, 0.5);
        }
        "buddy" => {
            let n = ["A5", "C6", "D6", "E6"][fastrand::usize(0..4)];
            blip(o, hz(n), hz(n) * 1.12, 0.0, 0.05, 0.05, Sine);
            blip(o, hz(n) * 1.19, hz(n) * 1.3, 0.06, 0.045, 0.06, Sine);
        }
        "buddyHappy" => {
            blip(o, hz("C6"), hz("G6"), 0.0, 0.06, 0.09, Sine);
            blip(o, hz("E6"), hz("C7"), 0.1, 0.055, 0.11, Sine);
        }
        "hint" => {
            bell(o, hz("E6"), 0.0, 0.06, 0.45);
            bell(o, hz("B6"), 0.07, 0.05, 0.6);
        }
        "playStart" => blip(o, hz("D5"), hz("A5"), 0.0, 0.09, 0.08, Sine),
        "playPause" => blip(o, hz("A5"), hz("D5"), 0.0, 0.08, 0.08, Sine),
        "playStop" => blip(o, hz("D5"), hz("G4"), 0.0, 0.08, 0.1, Sine),
        _ => {}
    }
    v
}

/// Self-marking: the more marks you give yourself, the higher the note.
pub fn mark(n: f64, max: f64) -> (usize, Vec<Voice>) {
    let scale = ["C5", "D5", "E5", "G5", "A5", "C6", "D6", "E6", "G6", "A6"];
    let i = if max > 0.0 { ((n / max) * (scale.len() - 1) as f64).round() as usize } else { 0 };
    let i = i.min(scale.len() - 1);
    let mut v = Vec::new();
    blip(&mut v, hz(scale[i]), hz(scale[i]), 0.0, 0.05, 0.07, Sine);
    air(&mut v, 3800.0, 3000.0, 0.0, 0.015, 0.03, 3.0);
    (i, v)
}

/// Text size picker: a tick that rises with the size.
pub fn size(i: usize) -> Vec<Voice> {
    let n = ["E5", "G5", "A5", "C6"].get(i).copied().unwrap_or("G5");
    let mut v = Vec::new();
    blip(&mut v, hz(n), hz(n), 0.0, 0.045, 0.06, Sine);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_and_voices() {
        assert!((hz("A4") - 440.0).abs() < 0.01);
        assert!((hz("C5") - 523.25).abs() < 0.1);
        for name in ["tap", "select", "correct", "wrong", "complete", "streak", "buddy"] {
            let mut vs = sound(name);
            assert!(!vs.is_empty(), "{name}");
            let mut peak: f32 = 0.0;
            for _ in 0..48000 {
                let s: f32 = vs.iter_mut().map(|v| v.sample(1.0 / 48000.0)).sum();
                peak = peak.max(s.abs());
                assert!(s.is_finite());
            }
            assert!(peak > 0.001 && peak < 1.0, "{name}: {peak}");
        }
    }
}
