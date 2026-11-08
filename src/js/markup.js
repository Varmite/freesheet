// Text markup and note-block rendering. Works in the browser (window.Markup) and in Node
// (require) so the content validator can use exactly the same parser.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.Markup = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  function escapeHtml(s) {
    return String(s == null ? '' : s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  // Split text into [{math:false,text}, {math:true,text}] segments.
  // `$...$` delimits inline maths; `\$` is a literal dollar sign.
  function segments(text) {
    const src = String(text == null ? '' : text);
    const out = [];
    let buf = '';
    let inMath = false;
    for (let i = 0; i < src.length; i++) {
      const ch = src[i];
      if (ch === '\\' && src[i + 1] === '$') {
        buf += inMath ? '\\$' : '$';
        i++;
        continue;
      }
      if (ch === '$') {
        if (buf || inMath) out.push({ math: inMath, text: buf });
        buf = '';
        inMath = !inMath;
        continue;
      }
      buf += ch;
    }
    if (inMath) {
      // Unclosed maths: treat as plain text so nothing disappears.
      out.push({ math: false, text: '$' + buf, unclosed: true });
    } else if (buf) {
      out.push({ math: false, text: buf });
    }
    return out;
  }

  function formatPlain(text) {
    let h = escapeHtml(text);
    h = h.replace(/\*\*(?=\S)([\s\S]*?\S)\*\*/g, '<strong>$1</strong>');
    h = h.replace(/(^|[^*\w])\*(?=\S)([^*\n]*?\S)\*(?!\w)/g, '$1<em>$2</em>');
    h = h.replace(/\n/g, '<br>');
    return h;
  }

  // A formula is drawn once: pages are rebuilt often (back, retry, the transition warm-up).
  const mathCache = new Map();

  function renderMath(tex, displayMode) {
    const k = (typeof katex !== 'undefined') ? katex : (typeof window !== 'undefined' ? window.katex : null);
    if (!k) return `<code>${escapeHtml(tex)}</code>`;
    const key = (displayMode ? 'D' : 'I') + tex;
    const hit = mathCache.get(key);
    if (hit !== undefined) return hit;
    let html;
    try {
      html = k.renderToString(tex, { displayMode: !!displayMode, throwOnError: false, output: 'html', strict: 'ignore' });
    } catch (e) {
      html = `<code>${escapeHtml(tex)}</code>`;
    }
    if (mathCache.size >= 4000) mathCache.clear();
    mathCache.set(key, html);
    return html;
  }

  // Inline markup -> HTML string.
  function inline(text) {
    return segments(text).map((s) => (s.math ? renderMath(s.text, false) : formatPlain(s.text))).join('');
  }

  // Paragraph-aware: blank lines become separate <p>.
  function paragraphs(text) {
    return String(text == null ? '' : text)
      .split(/\n\s*\n/)
      .map((p) => `<p>${inline(p)}</p>`)
      .join('');
  }

  function svgDataUrl(svg) {
    return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(String(svg));
  }

  function block(b) {
    if (!b || typeof b !== 'object') return '';
    switch (b.type) {
      case 'heading':
        return `<h3 class="n-heading">${inline(b.text)}</h3>`;
      case 'paragraph':
        return `<div class="n-para">${paragraphs(b.text)}</div>`;
      case 'list': {
        const tag = b.ordered ? 'ol' : 'ul';
        return `<${tag} class="n-list">${(b.items || []).map((i) => `<li>${inline(i)}</li>`).join('')}</${tag}>`;
      }
      case 'definition':
        return `<div class="n-def"><span class="n-def-term">${inline(b.term)}</span><span class="n-def-text">${inline(b.text)}</span></div>`;
      case 'example':
        return `<div class="n-example"><div class="n-label">${inline(b.title || 'Worked example')}</div>` +
          `<div class="n-problem">${paragraphs(b.problem)}</div>` +
          ((b.steps && b.steps.length) ? `<ol class="n-steps">${b.steps.map((s) => `<li>${inline(s)}</li>`).join('')}</ol>` : '') +
          (b.answer ? `<div class="n-answer"><strong>Answer:</strong> ${inline(b.answer)}</div>` : '') +
          `</div>`;
      case 'callout':
        return `<div class="n-callout"><div class="n-label">${inline(b.label || 'Note')}</div>${paragraphs(b.text)}</div>`;
      case 'table': {
        const head = (b.headers || []).map((h) => `<th>${inline(h)}</th>`).join('');
        const rows = (b.rows || []).map((r) => `<tr>${(r || []).map((c) => `<td>${inline(c)}</td>`).join('')}</tr>`).join('');
        return `<div class="n-table-wrap"><table class="n-table">` +
          (b.caption ? `<caption>${inline(b.caption)}</caption>` : '') +
          `<thead><tr>${head}</tr></thead><tbody>${rows}</tbody></table></div>`;
      }
      case 'formula':
        return `<div class="n-formula">${renderMath(b.latex, true)}` +
          (b.caption ? `<div class="n-caption">${inline(b.caption)}</div>` : '') + `</div>`;
      case 'passage':
        return `<div class="n-passage">${b.title ? `<div class="n-label">${inline(b.title)}</div>` : ''}` +
          `<div class="n-passage-text">${inline(b.text)}</div></div>`;
      case 'diagram':
        return `<figure class="n-diagram"><img alt="${escapeHtml(b.caption || 'Diagram')}" src="${svgDataUrl(b.svg)}">` +
          (b.caption ? `<figcaption>${inline(b.caption)}</figcaption>` : '') + `</figure>`;
      default:
        return '';
    }
  }

  function blocks(list) {
    return (list || []).map(block).join('');
  }

  return { escapeHtml, segments, inline, paragraphs, block, blocks, renderMath };
});
