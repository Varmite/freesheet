// FreeSheet sound effects: small, soft UI sounds synthesised with the Web Audio API
// (no audio files). Every sound is built from three voices:
//   bell  - a sine with a couple of quieter, faster-fading overtones (chimes)
//   blip  - a short sine or triangle with an optional pitch glide (ticks, pops)
//   air   - band-passed noise with a moving centre (taps, whooshes)
// Notes come from one pentatonic scale (C major pentatonic), so sounds that overlap
// never clash. Everything goes through one quiet master gain and a compressor.
//
// Turned off by the "Sounds" setting. Silent while the app's transition warm-up runs.
(function () {
  'use strict';

  let ctx = null, master = null, noise = null;
  let enabled = true;
  const VOLUME = 0.9;
  const last = new Map(); // sound name -> time last played, to stop pile-ups

  function audio() {
    if (ctx) return ctx;
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC({ latencyHint: 'interactive' });
    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -18;
    comp.ratio.value = 4;
    master = ctx.createGain();
    master.gain.value = VOLUME;
    master.connect(comp).connect(ctx.destination);
    // One second of white noise, reused by every "air" voice.
    noise = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const d = noise.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
    return ctx;
  }

  // Note name -> frequency, e.g. 'C6', 'G5'.
  const SEMI = { C: -9, D: -7, E: -5, F: -4, G: -2, A: 0, B: 2 };
  function hz(n) { return 440 * Math.pow(2, (SEMI[n[0]] + (Number(n.slice(1)) - 4) * 12) / 12); }

  // Gain envelope: quick attack, exponential decay to silence.
  function env(g, t, peak, attack, decay) {
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(peak, t + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t + attack + decay);
  }

  function bell(note, at, vol, decay) {
    const c = ctx, t = c.currentTime + at, f = typeof note === 'number' ? note : hz(note);
    decay = decay || 0.5;
    // Fundamental, then gentle overtones that fade sooner (a soft glassy chime).
    [[1, 1, 1], [2, 0.18, 0.5], [3.01, 0.07, 0.3]].forEach(([mult, amp, len]) => {
      const o = c.createOscillator(), g = c.createGain();
      o.type = 'sine';
      o.frequency.value = f * mult;
      env(g, t, vol * amp, 0.004, decay * len);
      o.connect(g).connect(master);
      o.start(t);
      o.stop(t + decay * len + 0.05);
    });
  }

  function blip(f1, f2, at, vol, len, type) {
    const c = ctx, t = c.currentTime + at;
    const o = c.createOscillator(), g = c.createGain();
    o.type = type || 'sine';
    o.frequency.setValueAtTime(f1, t);
    if (f2 && f2 !== f1) o.frequency.exponentialRampToValueAtTime(f2, t + len);
    env(g, t, vol, 0.003, len);
    o.connect(g).connect(master);
    o.start(t);
    o.stop(t + len + 0.05);
  }

  function air(f1, f2, at, vol, len, q) {
    const c = ctx, t = c.currentTime + at;
    const src = c.createBufferSource(), bp = c.createBiquadFilter(), g = c.createGain();
    src.buffer = noise;
    bp.type = 'bandpass';
    bp.Q.value = q || 1.2;
    bp.frequency.setValueAtTime(f1, t);
    if (f2 && f2 !== f1) bp.frequency.exponentialRampToValueAtTime(f2, t + len);
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(vol, t + len * 0.35);
    g.gain.exponentialRampToValueAtTime(0.0001, t + len);
    src.connect(bp).connect(g).connect(master);
    src.start(t, Math.random() * 0.5);
    src.stop(t + len + 0.05);
  }

  // ---------------------------------------------------------------- the sounds

  const SOUNDS = {
    // a light tick under the finger for any press
    tap() { air(3200, 2600, 0, 0.15, 0.03, 2); blip(2400, 1800, 0, 0.036, 0.018); },
    // choosing an option, a segment, a mark
    select() { blip(hz('A5'), hz('E5'), 0, 0.1, 0.06); air(4000, 3000, 0, 0.04, 0.03, 3); },
    toggleOn() { blip(hz('G5'), hz('G5'), 0, 0.04, 0.05); blip(hz('C6'), hz('C6'), 0.06, 0.045, 0.07); },
    toggleOff() { blip(hz('C6'), hz('C6'), 0, 0.04, 0.05); blip(hz('G5'), hz('G5'), 0.06, 0.035, 0.07); },

    // page changes
    push() { air(500, 1900, 0, 0.07, 0.24, 0.9); blip(hz('E5'), hz('A5'), 0.02, 0.036, 0.16); },
    pop() { air(1900, 500, 0, 0.064, 0.22, 0.9); blip(hz('A5'), hz('E5'), 0.02, 0.032, 0.15); },
    tab() { air(1200, 1500, 0, 0.055, 0.12, 1.4); blip(hz('D5'), hz('D5'), 0, 0.05, 0.06); },
    swap() { air(1400, 1700, 0, 0.045, 0.1, 1.6); blip(hz('G5'), hz('G5'), 0, 0.025, 0.05); },
    reveal() { air(700, 2400, 0, 0.025, 0.28, 0.8); bell('E6', 0.08, 0.025, 0.4); },

    // answers
    correct() { bell('C6', 0, 0.11, 0.5); bell('G6', 0.085, 0.1, 0.7); },
    partial() { bell('A5', 0, 0.08, 0.45); bell('C6', 0.08, 0.06, 0.5); },
    wrong() {
      blip(hz('E3'), hz('C3'), 0, 0.09, 0.16, 'triangle');
      blip(hz('C3'), hz('A2'), 0.12, 0.07, 0.2, 'triangle');
    },

    // finishing things
    complete() { ['C5', 'E5', 'G5', 'C6', 'E6'].forEach((n, i) => bell(n, i * 0.07, 0.07, 0.8)); },
    finishSoft() { ['G4', 'C5', 'E5'].forEach((n, i) => bell(n, i * 0.09, 0.06, 0.7)); },
    streak() {
      ['C5', 'E5', 'G5', 'C6', 'E6', 'G6'].forEach((n, i) => bell(n, i * 0.06, 0.065, 0.9));
      air(5000, 9000, 0.25, 0.02, 0.5, 0.7);
    },

    // dialogs and notices
    open() { blip(hz('E5'), hz('A5'), 0, 0.08, 0.09); air(900, 1800, 0, 0.03, 0.1, 1.2); },
    close() { blip(hz('A5'), hz('E5'), 0, 0.07, 0.08); },
    toast() { bell('E6', 0, 0.045, 0.4); bell('A6', 0.07, 0.035, 0.45); },
    error() { blip(hz('D4'), hz('C4'), 0, 0.06, 0.12, 'triangle'); blip(hz('C4'), hz('A3'), 0.11, 0.05, 0.14, 'triangle'); },

    // exams
    examStart() { bell('C5', 0, 0.06, 0.8); bell('G5', 0, 0.045, 0.8); bell('C6', 0.12, 0.06, 0.9); },
    submit() { bell('G5', 0, 0.06, 0.6); bell('D6', 0.09, 0.055, 0.7); },
    timeWarning() { bell('A5', 0, 0.08, 0.5); bell('A5', 0.22, 0.08, 0.5); },

    // study buddy
    buddy() {
      const n = ['A5', 'C6', 'D6', 'E6'][Math.floor(Math.random() * 4)];
      blip(hz(n), hz(n) * 1.12, 0, 0.05, 0.05);
      blip(hz(n) * 1.19, hz(n) * 1.3, 0.06, 0.045, 0.06);
    },
    buddyHappy() { blip(hz('C6'), hz('G6'), 0, 0.06, 0.09); blip(hz('E6'), hz('C7'), 0.1, 0.055, 0.11); },
    hint() { bell('E6', 0, 0.06, 0.45); bell('B6', 0.07, 0.05, 0.6); },

    // read aloud
    playStart() { blip(hz('D5'), hz('A5'), 0, 0.09, 0.08); },
    playPause() { blip(hz('A5'), hz('D5'), 0, 0.08, 0.08); },
    playStop() { blip(hz('D5'), hz('G4'), 0, 0.08, 0.1); },
  };

  // Self-marking: the more marks you give yourself, the higher the note.
  function mark(n, max) {
    const scale = ['C5', 'D5', 'E5', 'G5', 'A5', 'C6', 'D6', 'E6', 'G6', 'A6'];
    const i = max ? Math.round((n / max) * (scale.length - 1)) : 0;
    run(`mark${i}`, () => { blip(hz(scale[i]), hz(scale[i]), 0, 0.05, 0.07); air(3800, 3000, 0, 0.015, 0.03, 3); });
  }

  // Text size picker: a tick that rises with the size.
  function size(i) {
    const notes = ['E5', 'G5', 'A5', 'C6'];
    run(`size${i}`, () => blip(hz(notes[i] || 'G5'), hz(notes[i] || 'G5'), 0, 0.045, 0.06));
  }

  function run(name, fn) {
    if (!enabled || document.documentElement.classList.contains('rehearsing')) return;
    const c = audio();
    if (!c) return;
    if (c.state === 'suspended') c.resume();
    const now = performance.now();
    if (now - (last.get(name) || 0) < 45) return; // the same sound twice at once just sounds louder
    last.set(name, now);
    try { fn(); } catch (e) { /* a failed sound must never break the app */ }
  }

  function play(name) { if (SOUNDS[name]) run(name, SOUNDS[name]); }
  function setEnabled(v) { enabled = v !== false; }

  window.Sound = { play, mark, size, setEnabled, enabled: () => enabled };
})();
