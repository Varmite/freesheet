'use strict';
// Simple line icons (24×24, stroke = currentColor), drawn for FreeSheet.
// Icons.subject(id) returns a coloured tile for a subject; Icons.ui(name) returns a bare icon.
(function () {
  const P = {
    // --- subjects
    maths: '<path d="M4 7h6M7 4v6"/><path d="M14 7h6"/><path d="M4.8 14.8l4.4 4.4M9.2 14.8l-4.4 4.4"/><path d="M14 17h6"/><path d="M17 14h.01M17 20h.01"/>',
    calculator: '<rect x="5" y="2.5" width="14" height="19" rx="2"/><rect x="8" y="5.5" width="8" height="4" rx="0.5"/><path d="M8.5 13h.01M12 13h.01M15.5 13h.01M8.5 17h.01M12 17h.01M15.5 17h.01"/>',
    flask: '<path d="M9 3h6"/><path d="M10 3v6.5L4.6 18.2A1.8 1.8 0 0 0 6.1 21h11.8a1.8 1.8 0 0 0 1.5-2.8L14 9.5V3"/><path d="M7.3 15.5h9.4"/>',
    atom: '<circle cx="12" cy="12" r="1.2"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8" transform="rotate(120 12 12)"/>',
    leaf: '<path d="M5 19C5 10 11 5 20 4c-.5 9-6 15-15 15z"/><path d="M5 19c3-4.5 6.5-7.5 10.5-10"/>',
    landmark: '<path d="M12 3l8.5 5h-17z"/><path d="M6 11v7M10 11v7M14 11v7M18 11v7"/><path d="M3.5 21h17"/>',
    globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.6 2.5 3.8 5.5 3.8 9s-1.2 6.5-3.8 9c-2.6-2.5-3.8-5.5-3.8-9S9.4 5.5 12 3z"/>',
    wallet: '<rect x="3" y="7" width="18" height="13" rx="2"/><path d="M5.5 7l9.5-3.5L16.5 7"/><path d="M15 13.5h3"/>',
    receipt: '<path d="M6 2.5h12v19l-2-1.4-2 1.4-2-1.4-2 1.4-2-1.4-2 1.4z"/><path d="M9 7.5h6M9 11h6M9 14.5h3.5"/>',
    briefcase: '<rect x="3" y="7" width="18" height="13" rx="2"/><path d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><path d="M3 13h18"/>',
    trend: '<path d="M3.5 3.5v17h17"/><path d="M7.5 15l4-4 3 3 5.5-5.5"/><path d="M15.5 8.5H20V13"/>',
    gear: '<circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="6.5"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
    compass: '<circle cx="12" cy="12" r="9"/><path d="M15.8 8.2l-2.3 5.3-5.3 2.3 2.3-5.3z"/>',
    book: '<path d="M12 6.5C10 5 7 4.5 3 5v13c4-.5 7 0 9 1.5 2-1.5 5-2 9-1.5V5c-4-.5-7 0-9 1.5z"/><path d="M12 6.5v13"/>',
    file: '<path d="M14 2.5H6.5a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V8z"/><path d="M14 2.5V8h5.5"/>',
    // --- interface
    logo: '<path d="M14 2.5H6.5a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V8z"/><path d="M14 2.5V8h5.5"/><path d="M8.5 14.5l2.3 2.3 4.7-4.7"/>',
    study: '<path d="M13 3.5H6a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7"/><path d="M18.3 2.7a2 2 0 0 1 2.9 2.9L13 13.8l-3.6.8.8-3.6z"/>',
    exams: '<rect x="5" y="4" width="14" height="17.5" rx="2"/><path d="M9 2.5h6v3H9z"/><path d="M9 13.5l2 2 4-4"/>',
    speaker: '<path d="M11 5L6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M18.5 5.5a9 9 0 0 1 0 13"/>',
    play: '<path d="M7 4.5v15l12-7.5z"/>',
    pause: '<path d="M8 5v14M16 5v14"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="1.5"/>',
    flame: '<path d="M12 21.5c-3.9 0-7-2.9-7-6.8 0-3 1.7-5.1 3.5-6.9.3 1.9 1.3 3.2 2.6 3.8C11.2 8.1 12.4 5 15 2.5c.3 3 1.8 5 3 6.7 1 1.5 1 2.9 1 4.4 0 4.7-3.1 7.9-7 7.9z"/><path d="M12 21.5c-1.7 0-3-1.3-3-3 0-1.6 1.2-2.7 2.3-3.8.3 1 .9 1.6 1.6 1.9.3-1.2.9-2.2 1.8-2.9.4 1.4 1.3 2.4 1.3 4 0 2.2-1.5 3.8-4 3.8z"/>',
    progress: '<path d="M3.5 20.5h17"/><path d="M6.5 20.5v-6M12 20.5V9M17.5 20.5V4"/>',
    check: '<path d="M4.5 12.5l5 5 10-11"/>',
    close: '<path d="M6 6l12 12M18 6L6 18"/>',
    star: '<path d="M12 3.2l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17.2 6.6 20.1l1-6.1-4.4-4.3 6.1-.9z"/>',
    lock: '<rect x="5" y="10.5" width="14" height="10" rx="2.5"/><path d="M8.5 10.5V7.5a3.5 3.5 0 0 1 7 0v3"/>',
    chat: '<path d="M4 5.5h11a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H9l-4 3.5v-3.5H4a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2z"/><path d="M17 9.5h3a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-1v3l-3.5-3H12"/>',
    bulb: '<path d="M9 18h6M10 21.5h4"/><path d="M12 2.5a6.5 6.5 0 0 0-4 11.6c.7.6 1 1.3 1 2.2V17h6v-.7c0-.9.3-1.6 1-2.2a6.5 6.5 0 0 0-4-11.6z"/>',
    // settings cog: 8 flat-topped teeth (the 'gear' above is a subject icon)
    download: '<path d="M12 3.5v12"/><path d="M7 11l5 5 5-5"/><path d="M4.5 20.5h15"/>',
    settings: '<path d="M10.03 4.87L10.38 2.54L13.62 2.54L13.97 4.87A7.4 7.4 0 0 1 15.65 5.56L17.54 4.16L19.84 6.46L18.44 8.35A7.4 7.4 0 0 1 19.13 10.03L21.46 10.38L21.46 13.62L19.13 13.97A7.4 7.4 0 0 1 18.44 15.65L19.84 17.54L17.54 19.84L15.65 18.44A7.4 7.4 0 0 1 13.97 19.13L13.62 21.46L10.38 21.46L10.03 19.13A7.4 7.4 0 0 1 8.35 18.44L6.46 19.84L4.16 17.54L5.56 15.65A7.4 7.4 0 0 1 4.87 13.97L2.54 13.62L2.54 10.38L4.87 10.03A7.4 7.4 0 0 1 5.56 8.35L4.16 6.46L6.46 4.16L8.35 5.56A7.4 7.4 0 0 1 10.03 4.87Z"/><circle cx="12" cy="12" r="3"/>'
  };

  // Subject id → [icon, hue]. Unknown ids fall back on keywords, then a plain file icon.
  const SUBJECTS = {
    'mathematics': ['maths', 222],
    'mathematical-literacy': ['calculator', 200],
    'natural-sciences': ['flask', 165],
    'physical-sciences': ['atom', 262],
    'life-sciences': ['leaf', 128],
    'social-sciences-history': ['landmark', 24],
    'history': ['landmark', 24],
    'social-sciences-geography': ['globe', 190],
    'geography': ['globe', 190],
    'economic-and-management-sciences': ['wallet', 42],
    'accounting': ['receipt', 32],
    'business-studies': ['briefcase', 300],
    'economics': ['trend', 340],
    'technology': ['gear', 212],
    'life-orientation': ['compass', 352],
    'english-home-language': ['book', 280],
    'afrikaans': ['chat', 140]
  };
  const KEYWORDS = [
    ['afrikaans', 'chat', 140], ['english', 'book', 280], ['language', 'book', 280], ['math', 'maths', 222], ['science', 'flask', 165],
    ['history', 'landmark', 24], ['geograph', 'globe', 190], ['econom', 'trend', 340], ['account', 'receipt', 32],
    ['business', 'briefcase', 300], ['tech', 'gear', 212], ['life', 'compass', 352]
  ];

  function svg(name, cls) {
    return `<svg class="${cls || 'icon'}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" ` +
      `stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${P[name] || P.file}</svg>`;
  }

  function lookup(id) {
    if (SUBJECTS[id]) return SUBJECTS[id];
    const k = KEYWORDS.find(([w]) => String(id).includes(w));
    return k ? [k[1], k[2]] : ['file', 220];
  }

  window.Icons = {
    ui: (name, cls) => svg(name, cls),
    subject(id, size) {
      const [name, hue] = lookup(id);
      return `<span class="subject-icon${size ? ' ' + size : ''}" style="--h:${hue}">${svg(name)}</span>`;
    }
  };
})();
