// FreeSheet appearance: colour themes, accent colours and reading comfort. Loaded in <head>,
// before the stylesheets and the first paint, so a saved theme is on screen from the very
// start (the Rust side passes it in as window.__FREESHEET_APPEARANCE__).
// The colours themselves are in styles.css; app/src/ui/theme.rs has the same ones.
(function () {
  'use strict';

  // `dark` undefined: follows the system. `p`: the colours for the little preview tile.
  const THEMES = [
    { id: 'system', label: 'Automatic' },
    { id: 'light', label: 'Light', dark: false, p: { bg: '#faf8f4', card: '#ebe8df', line: '#2c2a26', btn: '#1f5fcc', accent: '#1f5fcc' } },
    { id: 'dark', label: 'Dark', dark: true, p: { bg: '#1a1b1e', card: '#2a2c30', line: '#dcdcd8', btn: '#3063c6', accent: '#7aa2ee' } },
    { id: 'sepia', label: 'Sepia', dark: false, p: { bg: '#f8f1e3', card: '#e9dfc9', line: '#33291c', btn: '#9a5b13', accent: '#9a5b13' } },
    { id: 'midnight', label: 'Midnight', dark: true, p: { bg: '#0f1626', card: '#1d2a42', line: '#dbe2ef', btn: '#2f6ad9', accent: '#6ea8fe' } },
    { id: 'forest', label: 'Forest', dark: true, p: { bg: '#121a15', card: '#1f2c24', line: '#dde6df', btn: '#237a4d', accent: '#57c489' } },
    { id: 'black', label: 'Black', dark: true, p: { bg: '#000000', card: '#181818', line: '#e4e4e0', btn: '#2f68d8', accent: '#6b9cf5' } },
    { id: 'contrast', label: 'High contrast', dark: false, p: { bg: '#ffffff', card: '#e4e4e4', line: '#000000', btn: '#0040b0', accent: '#0040b0' } },
  ];
  const ACCENTS = [
    { id: 'blue', label: 'Blue', light: '#1f5fcc', dark: '#5b8ff0' },
    { id: 'violet', label: 'Violet', light: '#6d3fd1', dark: '#a78bfa' },
    { id: 'pink', label: 'Pink', light: '#c0307f', dark: '#f17ab9' },
    { id: 'red', label: 'Red', light: '#c0372b', dark: '#f0807a' },
    { id: 'orange', label: 'Orange', light: '#b75a0a', dark: '#f5a25c' },
    { id: 'green', label: 'Green', light: '#1b7f4b', dark: '#56c98b' },
    { id: 'teal', label: 'Teal', light: '#0b7c86', dark: '#4fc5cf' },
  ];
  const SPACING = { compact: 0.92, normal: 1, relaxed: 1.16 };
  const WIDTH = { narrow: '600px', normal: '680px', wide: '860px' };

  const root = document.documentElement;
  const systemDark = window.matchMedia('(prefers-color-scheme: dark)');
  let chosen = 'system';

  function byId(id) { return THEMES.find((t) => t.id === id) || THEMES[0]; }

  // The look actually on screen: "Automatic" becomes light or dark.
  function resolve(id) {
    const t = byId(id);
    return t.id === 'system' ? byId(systemDark.matches ? 'dark' : 'light') : t;
  }

  function paint() {
    const t = resolve(chosen);
    root.dataset.theme = t.id;
    root.classList.toggle('is-dark', !!t.dark);
    root.style.colorScheme = t.dark ? 'dark' : 'light';
  }

  function apply(s) {
    s = s || {};
    chosen = byId(s.theme).id;
    root.dataset.accent = ACCENTS.some((a) => a.id === s.accent) ? s.accent : 'default';
    root.style.setProperty('--ls', SPACING[s.lineSpacing] || 1);
    root.style.setProperty('--read-w', WIDTH[s.readingWidth] || WIDTH.normal);
    paint();
  }

  systemDark.addEventListener('change', () => { if (chosen === 'system') paint(); });

  const saved = window.__FREESHEET_APPEARANCE__ || {};
  apply(saved);
  // The font is asked for now rather than once progress has loaded, so text appears sooner.
  root.classList.toggle('smooth-text', saved.textRendering !== 'system');

  window.Theme = { THEMES, ACCENTS, apply, resolve, isDark: () => !!resolve(chosen).dark };
})();
