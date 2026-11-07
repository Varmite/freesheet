//! Read-along: which word (and sentence) is being spoken while notes audio plays.
//!
//! The audio has no word timings, so they are worked out (as in 1.1's readalong.js):
//!  1. tts/generate.py puts 0.35 s of silence after every chunk it speaks, so the gaps in
//!     the decoded audio give the time span of each notes block.
//!  2. Within a block, time is shared between the words on screen in proportion to how
//!     long each takes to say (maths weighed by its spoken form), with extra time for
//!     pauses at punctuation, re-syncing at every pause in the audio.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seg {
    pub start: f32,
    pub end: f32,
}

/// The stretches of speech between the gaps (48 kHz mono samples).
pub fn analyse(pcm: &[f32], rate: f32) -> Vec<Seg> {
    let hop = (rate * 0.01).round() as usize; // 10 ms frames
    if hop == 0 || pcm.len() < hop {
        return vec![];
    }
    let n = pcm.len() / hop;
    let rms: Vec<f32> = (0..n).map(|f| (pcm[f * hop..(f + 1) * hop].iter().map(|s| s * s).sum::<f32>() / hop as f32).sqrt()).collect();
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let loud = sorted.get((n as f32 * 0.9) as usize).copied().unwrap_or(0.0);
    let thr = (loud * 0.03).max(1e-4);
    let min_gap = 22; // frames: just under the generator's 35-frame gap
    let sec = |f: usize| (f * hop) as f32 / rate;
    let mut segs = Vec::new();
    let mut start: isize = -1;
    let mut quiet = 0;
    for (f, &r) in rms.iter().enumerate() {
        if r > thr {
            if start < 0 {
                start = f as isize;
            }
            quiet = 0;
        } else if start >= 0 {
            quiet += 1;
            if quiet >= min_gap {
                segs.push(Seg { start: sec(start as usize), end: sec(f + 1 - quiet) });
                start = -1;
                quiet = 0;
            }
        }
    }
    if start >= 0 {
        segs.push(Seg { start: sec(start as usize), end: sec(n - quiet) });
    }
    segs
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub weight: f32,
    /// Where the word sits (block, line), for sentence breaks and pauses.
    pub line: (usize, usize),
    pub stop: bool,
    pub pause: bool,
    pub t: f32,
    pub sentence: usize,
}

pub struct Unit {
    pub toks: Vec<Tok>,
    /// Length of the block's speech text.
    pub chars: usize,
}

const CHUNK_GAP: f32 = 0.95;

fn chunks_of(segs: &[Seg]) -> Vec<Vec<Seg>> {
    let mut out: Vec<Vec<Seg>> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        if i == 0 || s.start - segs[i - 1].end >= CHUNK_GAP {
            out.push(vec![]);
        }
        out.last_mut().unwrap().push(*s);
    }
    out
}

/// Share out `items` as one contiguous run per block, each run's length matching the
/// block's share of the text as well as possible.
fn assign(units: &[Unit], items: &[Vec<Seg>]) -> Option<Vec<Vec<Seg>>> {
    let (b_n, n) = (units.len(), items.len());
    if b_n == 0 || n < b_n {
        return None;
    }
    let mut pre = vec![0.0f64];
    for it in items {
        pre.push(pre.last().unwrap() + (it.last().unwrap().end - it[0].start) as f64);
    }
    let chars: f64 = units.iter().map(|u| u.chars as f64).sum();
    let exp: Vec<f64> = units.iter().map(|u| pre[n] * u.chars as f64 / chars).collect();
    let mut cost = vec![vec![f64::INFINITY; n + 1]; b_n + 1];
    let mut from = vec![vec![0usize; n + 1]; b_n + 1];
    cost[0][0] = 0.0;
    for b in 1..=b_n {
        for s in b..=(n - (b_n - b)) {
            for k in (b - 1)..s {
                if cost[b - 1][k].is_infinite() {
                    continue;
                }
                let d = pre[s] - pre[k] - exp[b - 1];
                let c = cost[b - 1][k] + d * d / exp[b - 1].max(0.3);
                if c < cost[b][s] {
                    cost[b][s] = c;
                    from[b][s] = k;
                }
            }
        }
    }
    let mut groups = vec![Vec::new(); b_n];
    let mut s = n;
    for b in (1..=b_n).rev() {
        let k = from[b][s];
        groups[b - 1] = items[k..s].iter().flatten().copied().collect();
        s = k;
    }
    Some(groups)
}

/// Spread tokens over the speech in `segs` by weight, skipping the silences between.
fn spread(toks: &mut [Tok], segs: &[Seg]) {
    if segs.is_empty() {
        return;
    }
    let w: f32 = toks.iter().map(|t| t.weight).sum::<f32>().max(1e-6);
    let d: f32 = segs.iter().map(|s| s.end - s.start).sum();
    let mut acc = 0.0;
    for t in toks.iter_mut() {
        let mut x = acc / w * d;
        t.t = segs.last().unwrap().end;
        for s in segs {
            let dd = s.end - s.start;
            if x <= dd {
                t.t = s.start + x;
                break;
            }
            x -= dd;
        }
        acc += t.weight;
    }
}

/// Match the pauses inside a block's audio to punctuation in its text.
fn place(toks: &mut [Tok], segs: &[Seg]) {
    let k_n = segs.len();
    let cands: Vec<usize> = (1..toks.len()).filter(|&j| toks[j - 1].pause).collect();
    if k_n < 2 || cands.len() < k_n - 1 {
        return spread(toks, segs);
    }
    let mut wp = vec![0.0f64];
    for t in toks.iter() {
        wp.push(wp.last().unwrap() + t.weight as f64);
    }
    let w = wp[toks.len()];
    let durs: Vec<f64> = segs.iter().map(|s| (s.end - s.start) as f64).collect();
    let d: f64 = durs.iter().sum();
    let mut target = Vec::new();
    let mut acc = 0.0;
    for dur in durs.iter().take(k_n - 1) {
        acc += dur;
        target.push(w * acc / d);
    }
    let c_n = cands.len();
    let mut dp = vec![vec![f64::INFINITY; c_n]; k_n - 1];
    let mut back = vec![vec![-1isize; c_n]; k_n - 1];
    for c in 0..c_n {
        dp[0][c] = (wp[cands[c]] - target[0]).powi(2);
    }
    for k in 1..k_n - 1 {
        let mut best = f64::INFINITY;
        let mut best_c: isize = -1;
        for c in 0..c_n {
            if c > 0 && dp[k - 1][c - 1] < best {
                best = dp[k - 1][c - 1];
                best_c = c as isize - 1;
            }
            if best_c >= 0 {
                dp[k][c] = best + (wp[cands[c]] - target[k]).powi(2);
                back[k][c] = best_c;
            }
        }
    }
    let mut c: isize = -1;
    let mut best = f64::INFINITY;
    for i in 0..c_n {
        if dp[k_n - 2][i] < best {
            best = dp[k_n - 2][i];
            c = i as isize;
        }
    }
    if c < 0 {
        return spread(toks, segs);
    }
    let mut cuts = vec![0usize; k_n - 1];
    for k in (0..k_n - 1).rev() {
        cuts[k] = cands[c as usize];
        c = back[k][c as usize];
    }
    let mut bounds = vec![0];
    bounds.extend(cuts);
    bounds.push(toks.len());
    for k in 0..k_n {
        spread(&mut toks[bounds[k]..bounds[k + 1]], &segs[k..k + 1]);
    }
}

/// Times for every word of a section (`units` in order), with sentence numbers.
pub fn plan(mut units: Vec<Unit>, segs: &[Seg]) -> Option<Vec<Tok>> {
    units.retain(|u| !u.toks.is_empty());
    if units.is_empty() || segs.is_empty() {
        return None;
    }
    for u in units.iter_mut() {
        let n = u.toks.len();
        for i in 0..n {
            let next_line = if i + 1 < n { Some(u.toks[i + 1].line) } else { None };
            let t = &mut u.toks[i];
            t.pause = t.pause || i == n - 1 || next_line != Some(t.line);
        }
    }
    let groups = assign(&units, &chunks_of(segs)).or_else(|| assign(&units, &segs.iter().map(|s| vec![*s]).collect::<Vec<_>>()));
    match groups {
        Some(g) => {
            for (u, segs) in units.iter_mut().zip(&g) {
                place(&mut u.toks, segs);
            }
        }
        None => {
            let mut all: Vec<Tok> = units.iter().flat_map(|u| u.toks.clone()).collect();
            spread(&mut all, segs);
            let mut i = 0;
            for u in units.iter_mut() {
                for t in u.toks.iter_mut() {
                    t.t = all[i].t;
                    i += 1;
                }
            }
        }
    }
    let mut toks: Vec<Tok> = units.into_iter().flat_map(|u| u.toks).collect();
    let mut sentence = 0;
    for i in 0..toks.len() {
        if i > 0 && (toks[i - 1].stop || toks[i - 1].line != toks[i].line) {
            sentence += 1;
        }
        toks[i].sentence = sentence;
    }
    Some(toks)
}

/// The word being spoken at `t` seconds.
pub fn current(toks: &[Tok], t: f32) -> Option<usize> {
    let t = t + 0.04; // a hair early reads as "on time"
    let i = toks.partition_point(|x| x.t <= t);
    if i == 0 { None } else { Some(i - 1) }
}

/// A word's weight: how long it takes to say, plus pauses at punctuation.
pub fn word_weight(text: &str, math: &[String]) -> (f32, bool, bool) {
    let trimmed = text.trim_end_matches(['"', '\'', ')', ']', '\u{201D}', '\u{2019}']);
    let stop = trimmed.ends_with(['.', '!', '?']);
    let comma = trimmed.ends_with([',', ';', ':']);
    let sentence_end = stop || trimmed.ends_with(':');
    let spoken = crate::sections::plain(text).chars().count().max(if text.is_empty() { 0 } else { 1 });
    let mut w = spoken as f32 + 1.0 + if stop { 3.0 } else if comma { 1.0 } else { 0.0 };
    for m in math {
        w += crate::sections::latex(m).chars().count() as f32 + 1.0;
    }
    if text.is_empty() && math.is_empty() {
        w = 1.0;
    }
    (w, sentence_end, stop || comma)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_speech_and_places_words() {
        let rate = 48000.0;
        // 1 s speech, 0.6 s silence, 1 s speech
        let mut pcm = Vec::new();
        for i in 0..48000 {
            pcm.push((i as f32 * 0.05).sin() * 0.3);
        }
        pcm.extend(std::iter::repeat(0.0).take(28800));
        for i in 0..48000 {
            pcm.push((i as f32 * 0.05).sin() * 0.3);
        }
        let segs = analyse(&pcm, rate);
        assert_eq!(segs.len(), 2);
        let mk = |line| Tok { weight: 4.0, line, stop: false, pause: false, t: 0.0, sentence: 0 };
        let units = vec![Unit { toks: vec![mk((0, 0)), mk((0, 0))], chars: 10 }, Unit { toks: vec![mk((1, 0)), mk((1, 0))], chars: 10 }];
        let toks = plan(units, &segs).unwrap();
        assert!(toks[2].t > 1.4, "{:?}", toks);
        assert_eq!(current(&toks, 0.1), Some(0));
        assert_eq!(toks[3].sentence, 1);
    }
}
