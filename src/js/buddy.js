// FreeSheet study buddy: an animated character that sits in the corner, reacts to
// what you do, gives tips and hints and points you to the right notes. Hidden during
// exams. The app decides what the buddy says (see "Study buddy" in app.js); this file
// draws it, animates it and runs the speech bubble, menu and setup sheet.
//
// The art is drawn in SVG from simple shapes: five body types, eight colours and six
// accessories, all sharing one face so every expression and animation works on all
// of them. Animation:
//   - idle: breathing, a gentle bob, blinking, eyes that follow the pointer,
//     an antenna/tail/light that moves
//   - reactions: happy hop with squash and stretch and sparkles, a sympathetic droop,
//     thinking with thought dots and a lightbulb, waving, listening (while notes are
//     read aloud), dozing off when you're idle (with floating Zs)
//   - talking: the mouth moves while the speech bubble types out
// Respects the Animations setting (no movement when off) and the Sounds setting.
(function () {
  'use strict';

  const M = window.Markup;
  const esc = M.escapeHtml;

  // ---------------------------------------------------------------- looks

  const KINDS = [
    { id: 'blob', label: 'Blob' },
    { id: 'owl', label: 'Owl' },
    { id: 'robot', label: 'Robot' },
    { id: 'fox', label: 'Fox' },
    { id: 'dino', label: 'Dino' },
  ];
  const COLOURS = [
    { id: 'sky', label: 'Sky', hex: '#4F8DF7' },
    { id: 'mint', label: 'Mint', hex: '#2EC4A0' },
    { id: 'sun', label: 'Sun', hex: '#F7B731' },
    { id: 'coral', label: 'Coral', hex: '#FF6B6B' },
    { id: 'grape', label: 'Grape', hex: '#9B6DFF' },
    { id: 'bubblegum', label: 'Bubblegum', hex: '#FF7EB6' },
    { id: 'ocean', label: 'Ocean', hex: '#22AFCF' },
    { id: 'leaf', label: 'Leaf', hex: '#74C043' },
  ];
  const ACCESSORIES = [
    { id: 'none', label: 'Nothing' },
    { id: 'glasses', label: 'Glasses' },
    { id: 'cap', label: 'Grad cap' },
    { id: 'headphones', label: 'Headphones' },
    { id: 'bowtie', label: 'Bow tie' },
    { id: 'flower', label: 'Flower' },
  ];
  const NAMES = ['Pip', 'Nova', 'Milo', 'Zuri', 'Neo', 'Tumi', 'Kiki', 'Sol', 'Bean', 'Juno', 'Ollie', 'Zinzi', 'Kai', 'Lulu', 'Bongi', 'Taz', 'Mika', 'Ziggy', 'Lerato', 'Sipho'];

  const pick = (arr) => arr[Math.floor(Math.random() * arr.length)];
  function randomLook() {
    return { kind: pick(KINDS).id, color: pick(COLOURS).id, accessory: pick(ACCESSORIES.slice(1)).id, name: pick(NAMES) };
  }
  // Names appear in speech bubbles, which use the app's text markup, so drop the
  // characters that markup treats specially ($ for maths, * for bold).
  function cleanName(n) {
    const t = String(n || '').replace(/[\u0000-\u001f<>$*\\`]/g, '').replace(/\s+/g, ' ').trim().slice(0, 16);
    return t || 'Pip';
  }
  function normalise(look) {
    const l = Object.assign({}, look || {});
    if (!KINDS.some((k) => k.id === l.kind)) l.kind = 'blob';
    if (!COLOURS.some((c) => c.id === l.color)) l.color = 'sky';
    if (!ACCESSORIES.some((a) => a.id === l.accessory)) l.accessory = 'none';
    l.name = cleanName(l.name);
    return l;
  }

  // Mix a hex colour towards black (amt < 0) or white (amt > 0).
  function shade(hex, amt) {
    const n = parseInt(hex.slice(1), 16);
    const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.round(amt < 0 ? v * (1 + amt) : v + (255 - v) * amt));
    return '#' + ch.map((v) => v.toString(16).padStart(2, '0')).join('');
  }

  // ---------------------------------------------------------------- drawing

  // The shared face. fy: eye height; my: mouth height; eye/mouth colours (robot glows).
  function face(fy, my, o) {
    const eye = o.eye || '#1E1E28';
    const line = o.line || '#1E1E28';
    const hl = o.robot ? '' : `<circle cx="-1.9" cy="-2.7" r="2" fill="#fff"/><circle cx="1.6" cy="2.2" r="0.9" fill="#fff" opacity=".7"/>`;
    const eyeShape = o.robot ? `<rect x="-5" y="-6.5" width="10" height="13" rx="4" fill="${eye}"/>` : `<ellipse rx="5.8" ry="7.2" fill="${eye}"/>`;
    const one = (x) => `<g class="b-eye" transform="translate(${x} ${fy})"><g class="b-eye-in">` +
      `<g class="b-eye-open">${eyeShape}${hl}</g>` +
      `<path class="b-eye-happy" d="M-5.5 1.5 Q0 -5 5.5 1.5" stroke="${eye}" stroke-width="2.8" fill="none" stroke-linecap="round"/>` +
      `<path class="b-eye-closed" d="M-5.5 0 Q0 3.6 5.5 0" stroke="${eye}" stroke-width="2.6" fill="none" stroke-linecap="round"/>` +
      `</g></g>`;
    const cheeks = o.cheeks === false ? '' :
      `<g class="b-cheeks"><ellipse cx="38" cy="${fy + 9}" rx="5" ry="3" fill="#FF7A9A" opacity=".45"/><ellipse cx="82" cy="${fy + 9}" rx="5" ry="3" fill="#FF7A9A" opacity=".45"/></g>`;
    const brow = (d) => `<path d="${d}" stroke="${line}" stroke-width="2.4" fill="none" stroke-linecap="round"/>`;
    return `<g class="b-face">${cheeks}<g class="b-eyes">${one(48)}${one(72)}</g>` +
      `<g class="b-brows b-brows-sad">${brow(`M41 ${fy - 11} L52 ${fy - 14}`)}${brow(`M79 ${fy - 11} L68 ${fy - 14}`)}</g>` +
      `<g class="b-brows b-brows-think">${brow(`M41 ${fy - 13} Q47 ${fy - 17} 53 ${fy - 13}`)}${brow(`M67 ${fy - 11} L79 ${fy - 11}`)}</g>` +
      `<g class="b-mouth" transform="translate(60 ${my})">` +
      `<path class="b-m b-m-smile" d="M-6.5 -1 Q0 5.5 6.5 -1" stroke="${line}" stroke-width="2.6" fill="none" stroke-linecap="round"/>` +
      `<path class="b-m b-m-grin" d="M-8 -2 Q0 11 8 -2 Z" fill="${line}"/>` +
      `<ellipse class="b-m b-m-open" cy="0.5" rx="4" ry="3.8" fill="${line}"/>` +
      `<path class="b-m b-m-sad" d="M-5.5 3 Q0 -2.5 5.5 3" stroke="${line}" stroke-width="2.6" fill="none" stroke-linecap="round"/>` +
      `<circle class="b-m b-m-o" cy="0.5" r="2.6" fill="${line}"/>` +
      `<path class="b-m b-m-flat" d="M-4.5 0.5 L4.5 0.5" stroke="${line}" stroke-width="2.6" stroke-linecap="round"/>` +
      `</g></g>`;
  }

  // Each body: drawing layers plus where accessories sit.
  // Arms and tails are groups with their own pivot so they can wave and wag.
  function body(kind, c, acc) {
    const d = shade(c, -0.28);
    const l = shade(c, 0.6);
    const arms = (lx, ly, rx, ry, shapeL, shapeR) =>
      `<g class="b-arm b-arm-l" style="transform-origin:${lx}px ${ly}px">${shapeL}</g>` +
      `<g class="b-arm b-arm-r" style="transform-origin:${rx}px ${ry}px">${shapeR}</g>`;
    const sheen = (x, y) => `<ellipse cx="${x}" cy="${y}" rx="11" ry="6.5" fill="#fff" opacity=".22" transform="rotate(-28 ${x} ${y})"/>`;
    switch (kind) {
      case 'owl': return {
        geo: { fy: 62, my: 84, top: 30, sl: 23, sr: 97, neck: 99 },
        back: `<path d="M34 43 L27 15 L51 33 Z" fill="${d}"/><path d="M86 43 L93 15 L69 33 Z" fill="${d}"/>`,
        arms: arms(30, 64, 90, 64,
          `<path d="M29 62 C14 72 15 92 30 100 C27 87 31 74 29 62 Z" fill="${d}"/>`,
          `<path d="M91 62 C106 72 105 92 90 100 C93 87 89 74 91 62 Z" fill="${d}"/>`),
        main: `<path d="M60 28 C85 28 97 47 97 71 C97 97 81 109 60 109 C39 109 23 97 23 71 C23 47 35 28 60 28 Z" fill="${c}"/>${sheen(42, 42)}` +
          `<ellipse cx="60" cy="93" rx="21" ry="14" fill="${l}"/>` +
          `<path d="M50 89 q3 3 6 0 M62 89 q3 3 6 0 M56 96 q3 3 6 0" stroke="${d}" stroke-width="1.6" fill="none" opacity=".5" stroke-linecap="round"/>` +
          `<circle cx="48" cy="62" r="12.5" fill="${l}"/><circle cx="72" cy="62" r="12.5" fill="${l}"/>`,
        front: `<path d="M55.5 71 L64.5 71 L60 78.5 Z" fill="#F5A524"/>` +
          `<ellipse cx="50" cy="109" rx="5.5" ry="2.8" fill="#F5A524"/><ellipse cx="70" cy="109" rx="5.5" ry="2.8" fill="#F5A524"/>`,
        faceOpts: { cheeks: false },
      };
      case 'robot': return {
        geo: { fy: 61, my: 73, top: 32, sl: 24, sr: 96, neck: 88 },
        back: (acc === 'cap' ? '' : `<line x1="60" y1="33" x2="60" y2="19" stroke="${d}" stroke-width="3" stroke-linecap="round"/><circle class="b-light" cx="60" cy="16" r="4.6" fill="#FF5D73"/>`) +
          `<rect x="16" y="52" width="10" height="21" rx="3.5" fill="${d}"/><rect x="94" y="52" width="10" height="21" rx="3.5" fill="${d}"/>`,
        arms: arms(26, 82, 94, 82,
          `<rect x="14" y="80" width="12" height="23" rx="6" fill="${d}" transform="rotate(14 20 82)"/>`,
          `<rect x="94" y="80" width="12" height="23" rx="6" fill="${d}" transform="rotate(-14 100 82)"/>`),
        main: `<rect x="24" y="32" width="72" height="75" rx="19" fill="${c}"/>${sheen(38, 41)}` +
          `<rect x="31" y="43" width="58" height="38" rx="12" fill="#1B2232"/>` +
          `<path d="M37 49 Q46 45 55 47" stroke="#fff" stroke-width="2" opacity=".18" fill="none" stroke-linecap="round"/>` +
          `<circle cx="50" cy="97" r="3.2" fill="${l}"/><circle cx="60" cy="97" r="3.2" fill="#FFD166"/><circle cx="70" cy="97" r="3.2" fill="#FF5D73"/>`,
        front: `<rect x="37" y="105" width="17" height="7" rx="3.5" fill="${d}"/><rect x="66" y="105" width="17" height="7" rx="3.5" fill="${d}"/>`,
        faceOpts: { eye: '#7EF2FF', line: '#7EF2FF', robot: true, cheeks: false },
      };
      case 'fox': return {
        geo: { fy: 64, my: 83, top: 33, sl: 23, sr: 97, neck: 99 },
        back: `<g class="b-tail" style="transform-origin:82px 99px"><path d="M82 101 C112 101 119 68 102 51 C107 71 99 86 79 90 Z" fill="${c}"/>` +
          `<path d="M102.5 52 C109 59 112 68 111 76 C106.5 70 103 62 102.5 52 Z" fill="#FFF8EE"/></g>` +
          `<path d="M32 53 L29 13 L57 37 Z" fill="${c}"/><path d="M35.5 45 L34 23 L50 37 Z" fill="${d}"/>` +
          `<path d="M88 53 L91 13 L63 37 Z" fill="${c}"/><path d="M84.5 45 L86 23 L70 37 Z" fill="${d}"/>`,
        arms: arms(30, 86, 90, 86,
          `<ellipse cx="25" cy="92" rx="7" ry="9" fill="${d}" transform="rotate(22 25 92)"/>`,
          `<ellipse cx="95" cy="92" rx="7" ry="9" fill="${d}" transform="rotate(-22 95 92)"/>`),
        main: `<path d="M60 32 C86 32 97 52 97 74 C97 98 81 109 60 109 C39 109 23 98 23 74 C23 52 34 32 60 32 Z" fill="${c}"/>${sheen(42, 46)}` +
          `<ellipse cx="60" cy="101" rx="15" ry="8" fill="#FFF8EE" opacity=".95"/>` +
          `<ellipse cx="60" cy="81" rx="18" ry="12.5" fill="#FFF8EE"/>`,
        front: `<ellipse cx="60" cy="74.5" rx="3.8" ry="2.7" fill="#2B2B33"/>` +
          `<ellipse cx="47" cy="109" rx="7" ry="3.2" fill="${d}"/><ellipse cx="73" cy="109" rx="7" ry="3.2" fill="${d}"/>`,
        faceOpts: {},
      };
      case 'dino': return {
        geo: { fy: 62, my: 79, top: 31, sl: 24, sr: 95, neck: 97 },
        back: `<g class="b-tail" style="transform-origin:88px 92px"><path d="M84 86 C103 92 114 86 119 72 C112 79 102 81 86 76 Z" fill="${c}"/></g>` +
          `<g fill="${d}"><path d="M40 38 L41 25 L50 33 Z"/><path d="M52 31 L57 18 L63 30 Z"/><path d="M66 30 L73 18 L76 32 Z"/>` +
          `<path d="M79 36 L89 28 L87 42 Z"/><path d="M87 46 L98 42 L93 55 Z"/></g>`,
        arms: arms(30, 84, 88, 84,
          `<ellipse cx="25" cy="88" rx="5.5" ry="8" fill="${d}" transform="rotate(30 25 88)"/>`,
          `<ellipse cx="93" cy="88" rx="5.5" ry="8" fill="${d}" transform="rotate(-30 93 88)"/>`),
        main: `<path d="M58 30 C83 30 95 48 95 72 C95 98 80 109 58 109 C37 109 24 98 24 74 C24 50 35 30 58 30 Z" fill="${c}"/>${sheen(41, 44)}` +
          `<ellipse cx="58" cy="93" rx="19" ry="13" fill="${l}"/>` +
          `<path d="M46 89 H70 M44 95 H72 M47 101 H69" stroke="${d}" stroke-width="1.5" opacity=".35" stroke-linecap="round"/>`,
        front: `<ellipse cx="45" cy="109" rx="7.5" ry="3.3" fill="${d}"/><ellipse cx="71" cy="109" rx="7.5" ry="3.3" fill="${d}"/>`,
        faceOpts: {},
      };
      default: return { // blob
        geo: { fy: 66, my: 80, top: 31, sl: 21, sr: 99, neck: 96 },
        back: (acc === 'cap' || acc === 'headphones' ? '' :
          `<g class="b-antenna" style="transform-origin:60px 31px"><path d="M60 32 Q57 18 66 12" stroke="${d}" stroke-width="3" fill="none" stroke-linecap="round"/><circle cx="67" cy="11" r="5.2" fill="${l}"/></g>`),
        arms: arms(26, 76, 94, 76,
          `<ellipse cx="18" cy="80" rx="7" ry="10.5" fill="${d}" transform="rotate(25 18 80)"/>`,
          `<ellipse cx="102" cy="80" rx="7" ry="10.5" fill="${d}" transform="rotate(-25 102 80)"/>`),
        main: `<path d="M60 30 C87 30 99 50 99 73 C99 97 83 108 60 108 C37 108 21 97 21 73 C21 50 33 30 60 30 Z" fill="${c}"/>${sheen(42, 47)}`,
        front: `<ellipse cx="46" cy="107.5" rx="9" ry="4.2" fill="${d}"/><ellipse cx="74" cy="107.5" rx="9" ry="4.2" fill="${d}"/>`,
        faceOpts: {},
      };
    }
  }

  function accessory(id, g) {
    const { fy, top: t, sl, sr, neck: n } = g;
    switch (id) {
      case 'glasses': return `<g class="b-acc"><circle cx="48" cy="${fy}" r="10.5" fill="#fff" fill-opacity=".16" stroke="#23232B" stroke-width="2.5"/>` +
        `<circle cx="72" cy="${fy}" r="10.5" fill="#fff" fill-opacity=".16" stroke="#23232B" stroke-width="2.5"/>` +
        `<path d="M58.5 ${fy - 1} Q60 ${fy - 3} 61.5 ${fy - 1}" stroke="#23232B" stroke-width="2.5" fill="none"/></g>`;
      case 'cap': return `<g class="b-acc"><path d="M43 ${t + 2} L43 ${t + 10} Q60 ${t + 16} 77 ${t + 10} L77 ${t + 2} Z" fill="#2A2A33"/>` +
        `<path d="M60 ${t - 10} L91 ${t} L60 ${t + 10} L29 ${t} Z" fill="#383843"/>` +
        `<path d="M60 ${t} L85 ${t + 5} L85 ${t + 17}" stroke="#F5C542" stroke-width="2.2" fill="none" stroke-linecap="round"/><circle cx="85" cy="${t + 18}" r="2.8" fill="#F5C542"/></g>`;
      case 'headphones': return `<g class="b-acc"><path d="M${sl + 5} ${fy + 2} C${sl + 2} ${t - 18} ${sr - 2} ${t - 18} ${sr - 5} ${fy + 2}" stroke="#2A2A33" stroke-width="5.5" fill="none" stroke-linecap="round"/>` +
        `<rect x="${sl - 4}" y="${fy - 11}" width="13" height="24" rx="6" fill="#FF5D73"/><rect x="${sr - 9}" y="${fy - 11}" width="13" height="24" rx="6" fill="#FF5D73"/></g>`;
      case 'bowtie': return `<g class="b-acc"><path d="M60 ${n} L47 ${n - 7} L47 ${n + 7} Z" fill="#E5484D"/><path d="M60 ${n} L73 ${n - 7} L73 ${n + 7} Z" fill="#E5484D"/>` +
        `<circle cx="60" cy="${n}" r="3.3" fill="#B8323A"/></g>`;
      case 'flower': {
        const x = sl + 15, y = t + 8;
        const petals = [0, 72, 144, 216, 288].map((a) => `<ellipse cx="${x}" cy="${y - 5.5}" rx="3.6" ry="5.2" fill="#FFF3F8" transform="rotate(${a} ${x} ${y})"/>`).join('');
        return `<g class="b-acc">${petals}<circle cx="${x}" cy="${y}" r="3.4" fill="#FFC531"/></g>`;
      }
      default: return '';
    }
  }

  // Effects. Each sits in a positioning group so its CSS animation (which replaces
  // the element's own transform) can scale and float it in place.
  const FX = `<g class="b-fx">` +
    [[16, 34], [104, 30], [10, 72], [110, 68]].map(([x, y], i) =>
      `<g transform="translate(${x} ${y})"><path class="b-spark" style="--i:${i}" d="M0 -6.5 L1.7 -1.7 L6.5 0 L1.7 1.7 L0 6.5 L-1.7 1.7 L-6.5 0 L-1.7 -1.7 Z" fill="#FFD166"/></g>`).join('') +
    [[88, 30, 9], [96, 20, 11], [104, 9, 13]].map(([x, y, s], i) =>
      `<g transform="translate(${x} ${y})"><text class="b-z" style="--i:${i}" font-size="${s}" font-weight="800" fill="#8AA4FF">z</text></g>`).join('') +
    [[86, 28, 2.4], [93, 21, 3], [101, 14, 3.6]].map(([x, y, r], i) =>
      `<g transform="translate(${x} ${y})"><circle class="b-dot" style="--i:${i}" r="${r}" fill="#B7C3D9"/></g>`).join('') +
    `<g transform="translate(100 15)"><g class="b-bulb"><circle r="7.5" fill="#FFD84D"/><path d="M-3 7 h6 v3.5 h-6 Z" fill="#A6A6B3"/>` +
    `<path d="M-11 -9 l-3 -3 M11 -9 l3 -3 M0 -12 v-4" stroke="#FFD84D" stroke-width="2" stroke-linecap="round"/></g></g>` +
    `</g>`;

  // The buddy as SVG markup. cls: extra class (e.g. "static" for previews).
  function svg(look, cls) {
    const lk = normalise(look);
    const c = (COLOURS.find((x) => x.id === lk.color) || COLOURS[0]).hex;
    const b = body(lk.kind, c, lk.accessory);
    const g = b.geo;
    return `<svg class="buddy-svg ${cls || ''}" viewBox="0 0 120 120" data-kind="${lk.kind}" data-mood="idle" aria-hidden="true" focusable="false">` +
      `<ellipse class="b-shadow" cx="60" cy="113" rx="27" ry="4.5" fill="#000" opacity=".16"/>` +
      `<g class="b-float"><g class="b-actor"><g class="b-body">` +
      b.back + b.arms + b.main + b.front + face(g.fy, g.my, b.faceOpts) + accessory(lk.accessory, g) +
      `</g></g></g>${FX}</svg>`;
  }

  // ---------------------------------------------------------------- animation helpers

  const motionOn = () => !window.Motion || Motion.enabled();
  const sound = (n) => { if (window.Sound) Sound.play(n); };

  function anim(el, frames, opts) {
    if (!el || !motionOn() || !el.animate) return null;
    return el.animate(frames, Object.assign({ duration: 500, easing: 'cubic-bezier(.2,.9,.3,1.2)' }, opts));
  }

  // Run fn once an animation ends. A timer backs up the animation's own "finished"
  // signal, which Chromium holds back while the window is hidden or covered.
  function afterAnim(a, fn) {
    let called = false;
    const once = () => { if (!called) { called = true; fn(); } };
    if (!a) return once();
    a.finished.then(once, once);
    const t = a.effect ? a.effect.getComputedTiming() : null;
    setTimeout(once, ((t && t.endTime) || 500) + 80);
  }

  // Blinking for any buddy SVG (the corner buddy and the setup preview).
  function startBlinking(svgEl) {
    let timer = 0;
    const blink = () => {
      if (!svgEl.isConnected) return;
      const mood = svgEl.getAttribute('data-mood');
      if (mood === 'idle' || mood === 'think' || mood === 'sad' || mood === 'surprised') {
        const twice = Math.random() < 0.18;
        svgEl.querySelectorAll('.b-eye-in').forEach((e) => anim(e,
          twice ? [{ transform: 'scaleY(1)' }, { transform: 'scaleY(.1)', offset: 0.2 }, { transform: 'scaleY(1)', offset: 0.42 }, { transform: 'scaleY(.1)', offset: 0.62 }, { transform: 'scaleY(1)' }]
            : [{ transform: 'scaleY(1)' }, { transform: 'scaleY(.1)', offset: 0.45 }, { transform: 'scaleY(1)' }],
          { duration: twice ? 380 : 170, easing: 'ease-in-out' }));
      }
      timer = setTimeout(blink, 2200 + Math.random() * 3800);
    };
    timer = setTimeout(blink, 900 + Math.random() * 1500);
    return () => clearTimeout(timer);
  }

  // Happy hop with squash and stretch.
  function hop(svgEl, big) {
    const actor = svgEl.querySelector('.b-actor');
    const shadow = svgEl.querySelector('.b-shadow');
    const h = big ? -22 : -13;
    anim(actor, [
      { transform: 'translateY(0) scale(1, 1)' },
      { transform: 'translateY(0) scale(1.12, .86)', offset: 0.18 },
      { transform: `translateY(${h}px) scale(.94, 1.1)`, offset: 0.45 },
      { transform: 'translateY(0) scale(1.1, .9)', offset: 0.72 },
      { transform: 'translateY(0) scale(.98, 1.03)', offset: 0.86 },
      { transform: 'translateY(0) scale(1, 1)' },
    ], { duration: big ? 720 : 560, easing: 'ease-out' });
    anim(shadow, [{ transform: 'scale(1)', opacity: 0.16 }, { transform: 'scale(.7)', opacity: 0.08, offset: 0.45 }, { transform: 'scale(1)', opacity: 0.16 }],
      { duration: big ? 720 : 560, easing: 'ease-out' });
    svgEl.querySelectorAll('.b-arm').forEach((a, i) => anim(a, [
      { transform: 'rotate(0deg)' }, { transform: `rotate(${i ? -70 : 70}deg)`, offset: 0.4 }, { transform: 'rotate(0deg)' }],
    { duration: big ? 720 : 560, easing: 'ease-out' }));
  }

  // ---------------------------------------------------------------- the corner buddy

  const PRIORITY = { reaction: 1, tip: 2, user: 3 };

  const state = {
    look: null, on: false, quiet: false, min: false,
    hidden: new Set(), // reasons: 'exam', 'setup', 'welcome'
    mood: 'idle', moodTimer: 0, current: null, hideTimer: 0, typeTimer: 0, talkTimer: 0,
    stopBlink: null, sleeping: false, idleTimer: 0, listening: false, fidgetTimer: 0,
    pointer: null, eyeRaf: 0,
  };
  let root = null, fig = null, bubble = null, svgEl = null;
  let hooks = { menu: () => [], onPrefs: () => {}, onAction: () => {} };

  function mount() {
    if (root) return;
    root = document.createElement('div');
    root.className = 'buddy';
    root.hidden = true;
    root.innerHTML = `<div class="buddy-bubble" role="status" aria-live="polite" hidden></div>` +
      `<button type="button" class="buddy-fig" aria-haspopup="true"></button>`;
    document.body.appendChild(root);
    fig = root.querySelector('.buddy-fig');
    bubble = root.querySelector('.buddy-bubble');
    fig.addEventListener('click', onFigureClick);
    bubble.addEventListener('click', (e) => {
      const a = e.target.closest('[data-act]');
      if (a) { e.stopPropagation(); runAction(a.dataset.act); return; }
      if (e.target.closest('.buddy-x')) { closeBubble(); return; }
      finishTyping();
    });
    bubble.addEventListener('pointerenter', () => clearTimeout(state.hideTimer));
    bubble.addEventListener('pointerleave', () => scheduleHide(2500));
    // Eyes follow the pointer; any activity wakes the buddy.
    window.addEventListener('pointermove', (e) => { state.pointer = { x: e.clientX, y: e.clientY }; queueEyes(); activity(); }, { passive: true });
    for (const t of ['keydown', 'wheel', 'pointerdown']) window.addEventListener(t, activity, { passive: true, capture: true });
    window.addEventListener('resize', () => queueEyes());
  }

  function draw() {
    if (!fig) return;
    if (state.stopBlink) state.stopBlink();
    fig.innerHTML = svg(state.look);
    svgEl = fig.querySelector('svg');
    svgEl.setAttribute('data-mood', state.mood);
    fig.setAttribute('aria-label', `${state.look.name}, your study buddy. Open the help menu.`);
    state.stopBlink = startBlinking(svgEl);
    fidget();
  }

  function visible() { return state.on && !state.hidden.size; }

  function sync() {
    if (!root) return;
    const show = visible();
    root.classList.toggle('is-min', state.min);
    if (show && root.hidden) {
      root.hidden = false;
      anim(fig, [{ transform: 'translateY(140%) scale(.6)' }, { transform: 'translateY(0) scale(1)' }], { duration: 650, easing: 'cubic-bezier(.2,1.4,.4,1)' });
      if (!state.sleeping) { setTimeout(() => wave(), 380); }
      resetIdle();
    } else if (!show && !root.hidden) {
      closeBubble(true);
      const a = anim(fig, [{ transform: 'translateY(0)' }, { transform: 'translateY(150%)' }], { duration: 380, easing: 'cubic-bezier(.5,0,.75,0)', fill: 'forwards' });
      const done = () => { if (!visible()) { root.hidden = true; } if (a) a.cancel(); };
      afterAnim(a, done);
    }
  }

  function configure(opts) {
    mount();
    if (opts.hooks) hooks = Object.assign(hooks, opts.hooks);
    const prevLook = JSON.stringify(state.look);
    if (opts.look) state.look = normalise(opts.look);
    if ('on' in opts) state.on = !!opts.on;
    if ('quiet' in opts) state.quiet = !!opts.quiet;
    if ('min' in opts) state.min = !!opts.min;
    if (state.look && JSON.stringify(state.look) !== prevLook) draw();
    sync();
  }

  function hide(reason, yes) {
    if (yes) state.hidden.add(reason); else state.hidden.delete(reason);
    sync();
  }

  // ---- mood and expressions

  function setMood(m, ms) {
    clearTimeout(state.moodTimer);
    state.mood = m;
    if (svgEl) svgEl.setAttribute('data-mood', m);
    if (ms) state.moodTimer = setTimeout(() => setMood(state.listening ? 'listen' : 'idle'), ms);
  }

  function wave() {
    if (!svgEl || !visible()) return;
    const arm = svgEl.querySelector('.b-arm-r');
    anim(arm, [{ transform: 'rotate(0deg)' }, { transform: 'rotate(-100deg)', offset: 0.2 }, { transform: 'rotate(-70deg)', offset: 0.35 },
      { transform: 'rotate(-100deg)', offset: 0.5 }, { transform: 'rotate(-70deg)', offset: 0.65 }, { transform: 'rotate(-100deg)', offset: 0.8 }, { transform: 'rotate(0deg)' }],
    { duration: 1300, easing: 'ease-in-out' });
    setMood('happy', 1300);
  }

  function fx(cls, ms) {
    if (!svgEl || !motionOn()) return;
    svgEl.classList.remove(cls);
    void svgEl.getBoundingClientRect();
    svgEl.classList.add(cls);
    setTimeout(() => svgEl && svgEl.classList.remove(cls), ms);
  }

  // Reactions to what's happening. Movement only; speech comes from say().
  function react(kind) {
    if (!svgEl || !visible()) return;
    wake();
    switch (kind) {
      case 'correct':
        setMood('happy', 1500); hop(svgEl, false); fx('fx-spark', 1000); break;
      case 'celebrate':
        setMood('happy', 2600); hop(svgEl, true); setTimeout(() => hop(svgEl, true), 760); fx('fx-spark', 1800);
        sound('buddyHappy'); break;
      case 'wrong':
        setMood('sad', 1700);
        anim(svgEl.querySelector('.b-actor'), [{ transform: 'rotate(0deg) scale(1)' }, { transform: 'rotate(-6deg) scale(.96)', offset: 0.3 }, { transform: 'rotate(4deg) scale(.97)', offset: 0.6 }, { transform: 'rotate(0deg) scale(1)' }],
          { duration: 900, easing: 'ease-in-out' });
        break;
      case 'partial':
        setMood('think', 1400); hop(svgEl, false); break;
      case 'hint':
        setMood('think', 1500); fx('fx-think', 900);
        anim(svgEl.querySelector('.b-actor'), [{ transform: 'rotate(0deg)' }, { transform: 'rotate(-7deg)', offset: 0.4 }, { transform: 'rotate(-7deg)', offset: 0.7 }, { transform: 'rotate(0deg)' }], { duration: 1100, easing: 'ease-in-out' });
        setTimeout(() => { if (svgEl) { fx('fx-bulb', 1200); setMood('surprised', 700); } }, 900);
        break;
      case 'wave': wave(); break;
      case 'think': setMood('think', 1600); fx('fx-think', 1200); break;
      default: break;
    }
  }

  // Reading aloud: the buddy closes its eyes and sways along.
  function listen(on) {
    state.listening = !!on;
    if (!svgEl) return;
    if (on) { wake(); setMood('listen'); svgEl.classList.add('listening'); }
    else { svgEl.classList.remove('listening'); if (state.mood === 'listen') setMood('idle'); }
  }

  // ---- idle life, in short bursts
  //
  // Every few seconds the buddy does one small thing (breathes and bobs, wags its
  // tail, sways its antenna, pulses its light, glances around) and is otherwise still,
  // so an idle app draws no frames. Asleep, the Zs drift up now and then.
  function fidget() {
    clearTimeout(state.fidgetTimer);
    state.fidgetTimer = setTimeout(() => {
      if (svgEl && visible() && !state.min && motionOn() && !document.hidden && !state.listening) {
        if (state.sleeping) {
          fx('zzz', 2800);
        } else {
          const tail = svgEl.querySelector('.b-tail');
          const ant = svgEl.querySelector('.b-antenna');
          const light = svgEl.querySelector('.b-light');
          const moves = ['breathe', 'breathe', 'look'];
          if (tail) moves.push('wag', 'wag');
          if (ant) moves.push('sway');
          if (light) moves.push('light');
          const m = pick(moves);
          if (m === 'breathe') {
            anim(svgEl.querySelector('.b-body'), [{ transform: 'scale(1, 1)' }, { transform: 'scale(.99, 1.03)' }, { transform: 'scale(1, 1)' }], { duration: 2000, easing: 'ease-in-out' });
            anim(svgEl.querySelector('.b-float'), [{ transform: 'translateY(0)' }, { transform: 'translateY(-3px)' }, { transform: 'translateY(0)' }], { duration: 2000, easing: 'ease-in-out' });
          } else if (m === 'wag') {
            anim(tail, [{ transform: 'rotate(0deg)' }, { transform: 'rotate(-9deg)' }, { transform: 'rotate(9deg)' }, { transform: 'rotate(-7deg)' }, { transform: 'rotate(6deg)' }, { transform: 'rotate(0deg)' }], { duration: 1400, easing: 'ease-in-out' });
          } else if (m === 'sway') {
            anim(ant, [{ transform: 'rotate(0deg)' }, { transform: 'rotate(-10deg)' }, { transform: 'rotate(8deg)' }, { transform: 'rotate(0deg)' }], { duration: 1600, easing: 'ease-in-out' });
          } else if (m === 'light') {
            anim(light, [{ opacity: 1 }, { opacity: 0.3 }, { opacity: 1 }, { opacity: 0.3 }, { opacity: 1 }], { duration: 1500, easing: 'ease-in-out' });
          } else {
            anim(svgEl.querySelector('.b-eyes'), [{ transform: 'translate(0, 0)' }, { transform: 'translate(-2.6px, -0.6px)', offset: 0.25 }, { transform: 'translate(-2.6px, -0.6px)', offset: 0.45 },
              { transform: 'translate(2.6px, -0.6px)', offset: 0.7 }, { transform: 'translate(0, 0)' }], { duration: 1700, easing: 'ease-in-out' });
          }
        }
      }
      fidget();
    }, 6000 + Math.random() * 8000);
  }

  // ---- idle and sleep

  const SLEEP_AFTER = 90 * 1000;
  function resetIdle() {
    clearTimeout(state.idleTimer);
    state.idleTimer = setTimeout(() => {
      if (!visible() || state.listening || state.current) return resetIdle();
      state.sleeping = true;
      setMood('sleep');
      if (svgEl) { svgEl.classList.add('sleeping'); fx('zzz', 2800); }
    }, SLEEP_AFTER);
  }
  function wake() {
    if (state.sleeping) {
      state.sleeping = false;
      if (svgEl) {
        svgEl.classList.remove('sleeping');
        setMood('surprised', 500);
        anim(svgEl.querySelector('.b-actor'), [{ transform: 'scale(1, 1)' }, { transform: 'scale(.94, 1.1)', offset: 0.35 }, { transform: 'scale(1.04, .97)', offset: 0.7 }, { transform: 'scale(1, 1)' }], { duration: 600 });
      }
    }
    resetIdle();
  }
  let lastActivity = 0;
  function activity() {
    const now = performance.now();
    if (now - lastActivity < 400) return;
    lastActivity = now;
    wake();
  }

  // ---- eyes follow the pointer

  function queueEyes() {
    if (state.eyeRaf || !svgEl) return;
    state.eyeRaf = requestAnimationFrame(() => {
      state.eyeRaf = 0;
      if (!svgEl || !state.pointer || root.hidden || state.sleeping) return;
      const r = svgEl.getBoundingClientRect();
      const cx = r.left + r.width / 2, cy = r.top + r.height * 0.52;
      const dx = state.pointer.x - cx, dy = state.pointer.y - cy;
      const dist = Math.hypot(dx, dy) || 1;
      const k = Math.min(1, dist / 300);
      const ex = (dx / dist) * 2.8 * k, ey = (dy / dist) * 2.4 * k;
      const eyes = svgEl.querySelector('.b-eyes');
      const faceEl = svgEl.querySelector('.b-face');
      if (eyes) eyes.style.transform = `translate(${ex.toFixed(2)}px, ${ey.toFixed(2)}px)`;
      if (faceEl) faceEl.style.transform = `translate(${(ex * 0.35).toFixed(2)}px, ${(ey * 0.35).toFixed(2)}px)`;
    });
  }

  // ---- speech bubble

  function textOf(html) {
    const d = document.createElement('div');
    d.innerHTML = html;
    return d.textContent || '';
  }

  // say(text, { mood, actions: [{ id, label, primary }], priority: 'tip'|'reaction'|'user', ms })
  // Text may use **bold** and $maths$ (it is rendered with the app's markup).
  function say(text, opts) {
    const o = Object.assign({ priority: 'tip' }, opts);
    if (!visible() || state.min) return false;
    if (state.quiet && o.priority !== 'user') return false;
    const cur = state.current;
    if (cur && PRIORITY[cur.priority] > PRIORITY[o.priority] && performance.now() < cur.until) return false;
    wake();
    const html = M.inline(String(text));
    const actions = o.actions || [];
    state.current = { priority: o.priority, until: performance.now() + 4000, actions };
    clearTimeout(state.hideTimer);
    clearInterval(state.typeTimer);
    clearInterval(state.talkTimer);
    bubble.hidden = false;
    bubble.classList.toggle('has-actions', actions.length > 0);
    bubble.innerHTML = `<button type="button" class="buddy-x" aria-label="Close">×</button>` +
      `<div class="buddy-name">${esc(state.look.name)}</div><div class="buddy-text"></div>` +
      (actions.length ? `<div class="buddy-acts">${actions.map((a) =>
        `<button type="button" class="buddy-act${a.primary ? ' primary' : ''}" data-act="${esc(a.id)}">${M.inline(a.label)}</button>`).join('')}</div>` : '');
    const textEl = bubble.querySelector('.buddy-text');
    anim(bubble, [{ opacity: 0, transform: 'translateY(10px) scale(.9)' }, { opacity: 1, transform: 'translateY(0) scale(1)' }], { duration: 380 });
    if (o.mood) setMood(o.mood, 2600);
    sound('buddy');
    // Type it out while the mouth moves.
    const plainText = textOf(html);
    if (!motionOn() || plainText.length > 260) {
      textEl.innerHTML = html;
    } else {
      let i = 0;
      textEl.textContent = '';
      textEl.dataset.full = html;
      svgEl && svgEl.setAttribute('data-talk', '1');
      state.talkTimer = setInterval(() => { if (svgEl) svgEl.setAttribute('data-talk', svgEl.getAttribute('data-talk') === '1' ? '0' : '1'); }, 115);
      state.typeTimer = setInterval(() => {
        i += 2;
        textEl.textContent = plainText.slice(0, i);
        if (i >= plainText.length) finishTyping();
      }, 32);
    }
    const ms = o.ms || (actions.length ? 16000 : Math.max(5200, 2600 + plainText.length * 55));
    state.current.until = performance.now() + Math.min(ms, 8000);
    scheduleHide(ms);
    return true;
  }

  function finishTyping() {
    clearInterval(state.typeTimer);
    clearInterval(state.talkTimer);
    if (svgEl) svgEl.removeAttribute('data-talk');
    const textEl = bubble && bubble.querySelector('.buddy-text');
    if (textEl && textEl.dataset.full != null) { textEl.innerHTML = textEl.dataset.full; delete textEl.dataset.full; }
  }

  function scheduleHide(ms) {
    clearTimeout(state.hideTimer);
    state.hideTimer = setTimeout(() => closeBubble(), ms);
  }

  function closeBubble(instant) {
    clearTimeout(state.hideTimer);
    finishTyping();
    state.current = null;
    if (!bubble || bubble.hidden) return;
    if (instant || !motionOn()) { bubble.hidden = true; return; }
    const a = anim(bubble, [{ opacity: 1, transform: 'scale(1)' }, { opacity: 0, transform: 'translateY(6px) scale(.94)' }], { duration: 180, easing: 'ease-in', fill: 'forwards' });
    const done = () => { if (!state.current) bubble.hidden = true; if (a) a.cancel(); };
    afterAnim(a, done);
  }

  function onFigureClick() {
    if (state.min) { setMin(false); return; }
    wake();
    hop(svgEl, false);
    if (!bubble.hidden && state.current && state.current.menu) { closeBubble(); return; }
    const items = (hooks.menu() || []).concat([
      { id: '_look', label: 'Change how I look' },
      { id: '_min', label: 'Hide me for now' },
    ]);
    const greet = pick(['How can I help?', 'What do you need?', 'Hi! What can I do for you?', 'Need a hand?']);
    say(greet, { actions: items, priority: 'user', mood: 'happy' });
    if (state.current) state.current.menu = true;
  }

  function setMin(v) {
    state.min = v;
    closeBubble(true);
    sync();
    hooks.onPrefs({ min: v });
    if (!v) setTimeout(() => wave(), 200);
  }

  function runAction(id) {
    const cur = state.current;
    closeBubble();
    if (id === '_min') return setMin(true);
    if (id === '_look') return hooks.onAction('_look');
    const act = cur && cur.actions.find((a) => a.id === id);
    if (act && typeof act.run === 'function') act.run();
    else hooks.onAction(id);
  }

  // ---------------------------------------------------------------- setup sheet

  // Opens "Meet your study buddy". Resolves with the chosen look, or null if
  // cancelled (only possible when changing an existing buddy).
  function openSetup(opts) {
    const o = Object.assign({ first: true, look: null, intro: '' }, opts);
    let look = normalise(o.look || randomLook());
    hide('setup', true);
    return new Promise((resolve) => {
      const back = document.createElement('div');
      back.className = 'buddy-setup-back';
      back.innerHTML = `<div class="buddy-setup" role="dialog" aria-modal="true" aria-labelledby="bs-title">
        <div class="bs-head">
          <div><h2 id="bs-title">${o.first ? 'Meet your study buddy' : 'Change your buddy'}</h2>
          <p class="muted">${o.intro ? esc(o.intro) + ' ' : ''}Your buddy cheers you on, gives hints and shows you where answers are in the notes. Pick a look and a name. You can change this any time in Settings.</p></div>
          <button type="button" class="btn bs-surprise">Surprise me</button>
        </div>
        <div class="bs-body">
          <div class="bs-preview"><div class="bs-stage"></div><div class="bs-name-show"></div></div>
          <div class="bs-options">
            <div class="bs-group"><div class="bs-label">Look</div><div class="bs-row bs-kinds"></div></div>
            <div class="bs-group"><div class="bs-label">Colour</div><div class="bs-row bs-colours"></div></div>
            <div class="bs-group"><div class="bs-label">Extra</div><div class="bs-row bs-accs"></div></div>
            <div class="bs-group"><label class="bs-label" for="bs-name">Name</label>
              <div class="bs-name-row"><input type="text" id="bs-name" maxlength="16" autocomplete="off" spellcheck="false">
              <button type="button" class="btn bs-dice" title="Random name" aria-label="Pick a random name">Random name</button></div></div>
          </div>
        </div>
        <div class="bs-foot">${o.first ? '<button type="button" class="btn bs-skip">Skip for now</button>' : '<button type="button" class="btn bs-cancel">Cancel</button>'}
          <button type="button" class="btn primary bs-done">Let's go!</button></div>
      </div>`;
      document.body.appendChild(back);
      const sheet = back.querySelector('.buddy-setup');
      const stage = back.querySelector('.bs-stage');
      const nameIn = back.querySelector('#bs-name');
      const nameShow = back.querySelector('.bs-name-show');
      let stopBlink = null;

      function drawPreview(react) {
        if (stopBlink) stopBlink();
        stage.innerHTML = svg(look, 'bs-big');
        const s = stage.querySelector('svg');
        stopBlink = startBlinking(s);
        nameShow.textContent = look.name;
        if (react) { s.setAttribute('data-mood', 'happy'); hop(s, false); setTimeout(() => s.isConnected && s.setAttribute('data-mood', 'idle'), 900); }
      }
      function tile(kind, attr, id, label, inner, on) {
        return `<button type="button" class="bs-tile${on ? ' on' : ''}" data-${attr}="${id}" aria-pressed="${on}" title="${esc(label)}">${inner}<span>${esc(label)}</span></button>`;
      }
      function drawOptions() {
        back.querySelector('.bs-kinds').innerHTML = KINDS.map((k) =>
          tile('kind', 'kind', k.id, k.label, svg(Object.assign({}, look, { kind: k.id, accessory: 'none' }), 'static'), look.kind === k.id)).join('');
        back.querySelector('.bs-colours').innerHTML = COLOURS.map((c) =>
          `<button type="button" class="bs-swatch${look.color === c.id ? ' on' : ''}" data-color="${c.id}" aria-pressed="${look.color === c.id}" title="${c.label}" aria-label="${c.label}" style="--sw:${c.hex}"></button>`).join('');
        back.querySelector('.bs-accs').innerHTML = ACCESSORIES.map((a) =>
          tile('acc', 'acc', a.id, a.label, svg(Object.assign({}, look, { accessory: a.id }), 'static'), look.accessory === a.id)).join('');
      }
      function update(patch, react) {
        look = normalise(Object.assign({}, look, patch));
        drawOptions();
        drawPreview(react);
        if (window.Sound) Sound.play('select');
      }

      nameIn.value = look.name;
      drawOptions();
      drawPreview(false);
      back.addEventListener('click', (e) => {
        const t = e.target.closest('button');
        if (!t) return;
        if (t.dataset.kind) update({ kind: t.dataset.kind }, true);
        else if (t.dataset.color) update({ color: t.dataset.color }, true);
        else if (t.dataset.acc) update({ accessory: t.dataset.acc }, true);
        else if (t.classList.contains('bs-dice')) { const n = pick(NAMES.filter((x) => x !== look.name)); nameIn.value = n; update({ name: n }, true); }
        else if (t.classList.contains('bs-surprise')) { const r = randomLook(); nameIn.value = r.name; update(r, true); }
        else if (t.classList.contains('bs-done')) finish(true);
        else if (t.classList.contains('bs-skip')) finish(true);
        else if (t.classList.contains('bs-cancel')) finish(false);
      });
      nameIn.addEventListener('input', () => { look.name = cleanName(nameIn.value); nameShow.textContent = look.name; });
      nameIn.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); finish(true); } });
      const onKey = (e) => { if (e.key === 'Escape') { e.preventDefault(); finish(!!o.first); } };
      document.addEventListener('keydown', onKey, true);

      anim(back, [{ opacity: 0 }, { opacity: 1 }], { duration: 240, easing: 'linear' });
      anim(sheet, [{ opacity: 0, transform: 'translateY(40px) scale(.96)' }, { opacity: 1, transform: 'translateY(0) scale(1)' }], { duration: 560 });
      setTimeout(() => { const s = stage.querySelector('svg'); if (s) { s.setAttribute('data-mood', 'happy'); hop(s, true); setTimeout(() => s.isConnected && s.setAttribute('data-mood', 'idle'), 1000); } }, 300);
      if (window.Sound) Sound.play('open');
      nameIn.focus({ preventScroll: true });
      nameIn.select();

      let done = false;
      function finish(ok) {
        if (done) return;
        done = true;
        document.removeEventListener('keydown', onKey, true);
        if (stopBlink) stopBlink();
        look.name = cleanName(nameIn.value || look.name);
        const a = anim(back, [{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: 'ease-in', fill: 'forwards' });
        anim(sheet, [{ transform: 'scale(1)' }, { transform: 'scale(.96)' }], { duration: 200, easing: 'ease-in', fill: 'forwards' });
        const close = () => back.remove();
        afterAnim(a, close);
        hide('setup', false);
        resolve(ok ? look : null);
      }
    });
  }

  window.Buddy = {
    KINDS, COLOURS, ACCESSORIES, svg, randomLook, normalise,
    configure, hide, say, react, listen, closeBubble, openSetup,
    name: () => (state.look ? state.look.name : ''),
    isOn: () => visible() && !state.min,
  };
})();
