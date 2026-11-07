//! Time-stretching that keeps pitch (WSOLA: waveform-similarity overlap-add), so read-aloud
//! can play faster or slower without sounding like a chipmunk, as browsers do. It runs as
//! the audio plays, a few milliseconds at a time.

use std::sync::Arc;

/// Pulls stretched samples from a recording.
pub struct Stretcher {
    input: Arc<Vec<f32>>,
    /// Where in `input` this stretcher started.
    start: usize,
    speed: f32,
    frame: usize,
    hop: usize,
    search: usize,
    window: Vec<f32>,
    acc: Vec<f32>,
    out: std::collections::VecDeque<f32>,
    k: usize,
    prev_end: Option<usize>,
    done: bool,
}

impl Stretcher {
    pub fn new(input: Arc<Vec<f32>>, start: usize, speed: f32, rate: f32) -> Stretcher {
        let frame = ((0.040 * rate) as usize / 2) * 2; // 40 ms windows
        let hop = frame / 2;
        // A periodic Hann window: halves overlap to exactly 1, so no normalising is needed.
        let window = (0..frame).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / frame as f32).cos()).collect();
        Stretcher {
            input,
            start,
            speed,
            frame,
            hop,
            search: (0.012 * rate) as usize,
            window,
            acc: vec![0.0; frame],
            out: Default::default(),
            k: 0,
            prev_end: None,
            done: false,
        }
    }

    fn step(&mut self) {
        let len = self.input.len();
        let nominal = self.start + (self.k as f32 * self.hop as f32 * self.speed) as usize;
        if nominal >= len {
            // Flush what is left in the overlap buffer.
            self.out.extend(self.acc.drain(..self.hop));
            self.done = true;
            return;
        }
        let mut best = nominal;
        if let Some(prev) = self.prev_end {
            // The offset near `nominal` whose start best matches the previous frame's continuation.
            let lo = nominal.saturating_sub(self.search);
            let hi = (nominal + self.search).min(len.saturating_sub(1));
            let n = self.hop.min(480);
            let mut best_score = f32::MIN;
            let mut c = lo;
            while c <= hi {
                let mut score = 0.0;
                let mut j = 0;
                while j < n {
                    let a = self.input.get(c + j).copied().unwrap_or(0.0);
                    let b = self.input.get(prev + j).copied().unwrap_or(0.0);
                    score += a * b;
                    j += 3;
                }
                if score > best_score {
                    best_score = score;
                    best = c;
                }
                c += 2;
            }
        }
        for i in 0..self.frame {
            self.acc[i] += self.input.get(best + i).copied().unwrap_or(0.0) * self.window[i];
        }
        self.out.extend(self.acc.drain(..self.hop));
        self.acc.extend(std::iter::repeat_n(0.0, self.hop));
        self.prev_end = Some(best + self.hop);
        self.k += 1;
    }

    pub fn next(&mut self) -> Option<f32> {
        while self.out.is_empty() {
            if self.done {
                return None;
            }
            self.step();
        }
        self.out.pop_front()
    }
}

/// The whole recording stretched at once (for tests).
pub fn wsola(input: &[f32], speed: f32, rate: f32) -> Vec<f32> {
    let mut s = Stretcher::new(Arc::new(input.to_vec()), 0, speed, rate);
    let mut out = Vec::new();
    while let Some(x) = s.next() {
        out.push(x);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_pitch_changes_length() {
        let rate = 48000.0;
        let tone: Vec<f32> = (0..48000).map(|i| (i as f32 / rate * 440.0 * std::f32::consts::TAU).sin() * 0.5).collect();
        let fast = super::wsola(&tone, 1.5, rate);
        assert!((fast.len() as f32 - 32000.0).abs() < 2400.0, "{}", fast.len());
        let mid = &fast[8000..24000];
        let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f32;
        let hz = crossings / (mid.len() as f32 / rate);
        assert!((hz - 440.0).abs() < 25.0, "{hz}");
        let peak = mid.iter().fold(0f32, |a, b| a.max(b.abs()));
        assert!((peak - 0.5).abs() < 0.08, "{peak}");
    }
}
