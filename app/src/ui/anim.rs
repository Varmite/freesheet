//! Keyframe animations with CSS easing, for the buddy and other motion.

/// CSS cubic-bezier(x1, y1, x2, y2) at progress `t` (0..1).
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let cx = |s: f32| 3.0 * x1 * s * (1.0 - s).powi(2) + 3.0 * x2 * s * s * (1.0 - s) + s * s * s;
    let cy = |s: f32| 3.0 * y1 * s * (1.0 - s).powi(2) + 3.0 * y2 * s * s * (1.0 - s) + s * s * s;
    // Solve cx(s) = t by bisection (monotonic in s for valid x1, x2).
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if cx(mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    cy((lo + hi) / 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// The buddy's springy default: cubic-bezier(.2,.9,.3,1.2)
    Spring,
    Bezier(f32, f32, f32, f32),
}

impl Ease {
    pub fn at(self, t: f32) -> f32 {
        match self {
            Ease::Linear => t.clamp(0.0, 1.0),
            Ease::Ease => bezier(0.25, 0.1, 0.25, 1.0, t),
            Ease::EaseIn => bezier(0.42, 0.0, 1.0, 1.0, t),
            Ease::EaseOut => bezier(0.0, 0.0, 0.58, 1.0, t),
            Ease::EaseInOut => bezier(0.42, 0.0, 0.58, 1.0, t),
            Ease::Spring => bezier(0.2, 0.9, 0.3, 1.2, t),
            Ease::Bezier(a, b, c, d) => bezier(a, b, c, d, t),
        }
    }
}

/// Keyframes (offset 0..1, value), evaluated with `ease` between each pair, as CSS does.
pub fn keyframes(keys: &[(f32, f32)], ease: Ease, t: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    if t <= keys[0].0 {
        return keys[0].1;
    }
    for w in keys.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            let span = (b.0 - a.0).max(1e-6);
            let k = ease.at((t - a.0) / span);
            return a.1 + (b.1 - a.1) * k;
        }
    }
    keys.last().unwrap().1
}

/// Even keyframes: values spread over 0..1.
pub fn even(values: &[f32]) -> Vec<(f32, f32)> {
    let n = values.len().max(2) - 1;
    values.iter().enumerate().map(|(i, v)| (i as f32 / n as f32, *v)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prop {
    FloatY,
    FloatRot,
    ActorTy,
    ActorSx,
    ActorSy,
    ActorRot,
    BodySx,
    BodySy,
    ShadowS,
    ShadowO,
    ArmL,
    ArmR,
    Tail,
    Antenna,
    Light,
    EyesX,
    EyesY,
    Blink,
    Sparks,
    Bulb,
    Zzz,
    /// Whole-figure entrance (slides up from below): translateY in % of the figure.
    EnterY,
    EnterS,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub prop: Prop,
    pub start: f64,
    pub dur: f32,
    pub keys: Vec<(f32, f32)>,
    pub ease: Ease,
}

impl Track {
    pub fn progress(&self, now: f64) -> f32 {
        ((now - self.start) as f32 / self.dur).clamp(0.0, 1.0)
    }
    pub fn value(&self, now: f64) -> f32 {
        keyframes(&self.keys, self.ease, self.progress(now))
    }
    pub fn done(&self, now: f64) -> bool {
        now - self.start >= self.dur as f64
    }
}

/// A set of running tracks; the newest track for a property wins.
#[derive(Default, Clone, Debug)]
pub struct Tracks(pub Vec<Track>);

impl Tracks {
    pub fn add(&mut self, prop: Prop, start: f64, dur_ms: f32, keys: Vec<(f32, f32)>, ease: Ease) {
        self.0.retain(|t| t.prop != prop);
        self.0.push(Track { prop, start, dur: dur_ms / 1000.0, keys, ease });
    }
    pub fn get(&self, prop: Prop, now: f64) -> Option<f32> {
        self.0.iter().rev().find(|t| t.prop == prop && t.start <= now).map(|t| t.value(now))
    }
    pub fn prune(&mut self, now: f64) {
        self.0.retain(|t| !t.done(now));
    }
    pub fn active(&self, now: f64) -> bool {
        self.0.iter().any(|t| !t.done(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing() {
        assert!((bezier(0.25, 0.1, 0.25, 1.0, 0.5) - 0.8024).abs() < 0.01);
        assert_eq!(keyframes(&[(0.0, 0.0), (0.5, 10.0), (1.0, 0.0)], Ease::Linear, 0.25), 5.0);
        assert!(Ease::Spring.at(0.6) > 1.0); // overshoots
    }
}
