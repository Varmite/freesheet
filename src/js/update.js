// Looks for a newer FreeSheet on GitHub. Works in the app (window.Updates) and in Node
// (require) so scripts/test-update.js can check the logic. Nothing about the learner is
// sent: it is one plain request for the public list of releases.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.Updates = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const API = 'https://api.github.com/repos/Varmite/freesheet/releases';
  const RELEASES = 'https://github.com/Varmite/freesheet/releases/';
  const LATEST_PAGE = RELEASES + 'latest';

  // "v1.2.3" or "android-v1.2.3" -> [1, 2, 3]; null if it isn't a version.
  function parts(tag) {
    const m = /^(?:android-)?v?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/i.exec(String(tag == null ? '' : tag).trim());
    return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
  }

  function isNewer(other, current) {
    const a = parts(other), b = parts(current);
    if (!a || !b) return false;
    for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
    return false;
  }

  function usable(r, android) {
    if (!r || r.draft || r.prerelease || !parts(r.tag_name)) return false;
    return /^android-/i.test(r.tag_name) === android;
  }

  // What GitHub's answer means for this version: { status: 'current' } or
  // { status: 'available', version, url }. `data` is the release list, or one release.
  function interpret(data, current, android) {
    const list = Array.isArray(data) ? data : [data];
    const best = list.filter((r) => usable(r, android)).sort((a, b) => (isNewer(a.tag_name, b.tag_name) ? -1 : isNewer(b.tag_name, a.tag_name) ? 1 : 0))[0];
    if (!best) throw new Error('No release found');
    if (!isNewer(best.tag_name, current)) return { status: 'current' };
    const url = typeof best.html_url === 'string' && best.html_url.startsWith(RELEASES) ? best.html_url : LATEST_PAGE;
    const out = { status: 'available', version: parts(best.tag_name).join('.'), url };
    // Roughly how much data the update will use, to the nearest 10 MB.
    const sizes = (Array.isArray(best.assets) ? best.assets : []).filter((a) => a && a.size > 0 && !String(a.name).toLowerCase().endsWith('.txt')).map((a) => a.size);
    if (sizes.length) out.sizeMB = Math.round(Math.min(...sizes) / 10485760) * 10;
    return out;
  }

  // Never throws for the caller's sake alone: being offline simply rejects, and the app
  // says nothing unless the learner pressed "Check now".
  async function check(current, android) {
    // The newest desktop release is GitHub's "latest"; phone releases are listed separately.
    const url = android ? `${API}?per_page=30` : `${API}/latest`;
    const res = await fetch(url, { headers: { Accept: 'application/vnd.github+json' }, cache: 'no-store', signal: AbortSignal.timeout(15000) });
    if (!res.ok) throw new Error(`GitHub answered ${res.status}`);
    return interpret(await res.json(), current, android);
  }

  return { isNewer, interpret, check, RELEASES };
});
