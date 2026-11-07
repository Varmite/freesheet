//! Text markup used in every content field (docs/CONTENT_FORMAT.md):
//! `**bold**`, `*italic*`, `$maths$`, `\$` for a dollar sign, `\n` line breaks and
//! `\n\n` paragraph breaks.
//!
//! `inline()` turns markup into a small HTML subset, exactly as the JavaScript version
//! did (maths as `<code>tex</code>`). The screens draw that subset (see ui::rich), and the
//! tests compare it with the JavaScript output character for character.

use fancy_regex::Regex;
use std::sync::OnceLock;

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape_html(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", "\u{a0}").replace("&amp;", "&")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub math: bool,
    pub text: String,
}

/// Split text into plain and `$maths$` segments. `\$` is a literal dollar sign.
pub fn segments(text: &str) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_math = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && chars.get(i + 1) == Some(&'$') {
            buf.push_str(if in_math { "\\$" } else { "$" });
            i += 2;
            continue;
        }
        if ch == '$' {
            if !buf.is_empty() || in_math {
                out.push(Segment { math: in_math, text: std::mem::take(&mut buf) });
            }
            in_math = !in_math;
            i += 1;
            continue;
        }
        buf.push(ch);
        i += 1;
    }
    if in_math {
        // Unclosed maths: treat as plain text so nothing disappears.
        out.push(Segment { math: false, text: format!("${buf}") });
    } else if !buf.is_empty() {
        out.push(Segment { math: false, text: buf });
    }
    out
}

fn bold_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\*\*(?=\S)([\s\S]*?\S)\*\*").unwrap())
}
fn italic_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(^|[^*A-Za-z0-9_])\*(?=\S)([^*\n]*?\S)\*(?![A-Za-z0-9_])").unwrap())
}

pub fn format_plain(text: &str) -> String {
    let h = escape_html(text);
    let h = bold_re().replace_all(&h, "<strong>$1</strong>");
    let h = italic_re().replace_all(&h, "$1<em>$2</em>");
    h.replace('\n', "<br>")
}

pub fn render_math(tex: &str) -> String {
    format!("<code>{}</code>", escape_html(tex))
}

/// Inline markup to the HTML subset.
pub fn inline(text: &str) -> String {
    segments(text).iter().map(|s| if s.math { render_math(&s.text) } else { format_plain(&s.text) }).collect()
}

/// Blank lines become separate paragraphs.
pub fn paragraphs(text: &str) -> String {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    let re = R.get_or_init(|| regex::Regex::new(r"\n\s*\n").unwrap());
    re.split(text).map(|p| format!("<p>{}</p>", inline(p))).collect()
}

/// The paragraphs of a text, each as inline HTML.
pub fn paragraph_list(text: &str) -> Vec<String> {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    let re = R.get_or_init(|| regex::Regex::new(r"\n\s*\n").unwrap());
    re.split(text).map(inline).collect()
}

/// Plain text of an HTML-subset string (tags dropped, entities decoded).
pub fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    unescape_html(&out)
}

/// A note block as HTML, as the JavaScript drew it (used by the parity tests; the app
/// draws blocks natively). Diagrams are shown as a bare `<img>`.
pub fn block_html(b: &crate::model::Block) -> String {
    let o = |s: &Option<String>| s.clone().unwrap_or_default();
    let nonempty = |s: &Option<String>| s.as_deref().filter(|t| !t.is_empty()).map(str::to_string);
    match b.kind.as_str() {
        "heading" => format!("<h3 class=\"n-heading\">{}</h3>", inline(&o(&b.text))),
        "paragraph" => format!("<div class=\"n-para\">{}</div>", paragraphs(&o(&b.text))),
        "list" => {
            let tag = if b.ordered { "ol" } else { "ul" };
            let items: String = b.items.iter().map(|i| format!("<li>{}</li>", inline(i))).collect();
            format!("<{tag} class=\"n-list\">{items}</{tag}>")
        }
        "definition" => format!(
            "<div class=\"n-def\"><span class=\"n-def-term\">{}</span><span class=\"n-def-text\">{}</span></div>",
            inline(&o(&b.term)),
            inline(&o(&b.text))
        ),
        "example" => {
            let title = nonempty(&b.title).unwrap_or_else(|| "Worked example".into());
            let mut h = format!(
                "<div class=\"n-example\"><div class=\"n-label\">{}</div><div class=\"n-problem\">{}</div>",
                inline(&title),
                paragraphs(&o(&b.problem))
            );
            if !b.steps.is_empty() {
                let steps: String = b.steps.iter().map(|s| format!("<li>{}</li>", inline(s))).collect();
                h += &format!("<ol class=\"n-steps\">{steps}</ol>");
            }
            if let Some(a) = nonempty(&b.answer) {
                h += &format!("<div class=\"n-answer\"><strong>Answer:</strong> {}</div>", inline(&a));
            }
            h + "</div>"
        }
        "callout" => format!(
            "<div class=\"n-callout\"><div class=\"n-label\">{}</div>{}</div>",
            inline(&nonempty(&b.label).unwrap_or_else(|| "Note".into())),
            paragraphs(&o(&b.text))
        ),
        "table" => {
            let head: String = b.headers.iter().map(|h| format!("<th>{}</th>", inline(h))).collect();
            let rows: String = b
                .rows
                .iter()
                .map(|r| format!("<tr>{}</tr>", r.iter().map(|c| format!("<td>{}</td>", inline(c))).collect::<String>()))
                .collect();
            let cap = nonempty(&b.caption).map(|c| format!("<caption>{}</caption>", inline(&c))).unwrap_or_default();
            format!("<div class=\"n-table-wrap\"><table class=\"n-table\">{cap}<thead><tr>{head}</tr></thead><tbody>{rows}</tbody></table></div>")
        }
        "formula" => {
            let cap = nonempty(&b.caption).map(|c| format!("<div class=\"n-caption\">{}</div>", inline(&c))).unwrap_or_default();
            format!("<div class=\"n-formula\">{}{cap}</div>", render_math(&o(&b.latex)))
        }
        "passage" => {
            let title = nonempty(&b.title).map(|t| format!("<div class=\"n-label\">{}</div>", inline(&t))).unwrap_or_default();
            format!("<div class=\"n-passage\">{title}<div class=\"n-passage-text\">{}</div></div>", inline(&o(&b.text)))
        }
        "diagram" => {
            let cap = nonempty(&b.caption).map(|c| format!("<figcaption>{}</figcaption>", inline(&c))).unwrap_or_default();
            format!("<figure class=\"n-diagram\"><img>{cap}</figure>")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_and_inline() {
        assert_eq!(segments("a $x^2$ b"), vec![
            Segment { math: false, text: "a ".into() },
            Segment { math: true, text: "x^2".into() },
            Segment { math: false, text: " b".into() },
        ]);
        assert_eq!(segments("US\\$5"), vec![Segment { math: false, text: "US$5".into() }]);
        assert_eq!(segments("$open")[0].text, "$open");
        assert_eq!(inline("**Bold** and *it* <x>"), "<strong>Bold</strong> and <em>it</em> &lt;x&gt;");
        assert_eq!(inline("2 * 3 * 4"), "2 * 3 * 4");
        assert_eq!(inline("a\nb $\\frac{1}{2}$"), "a<br>b <code>\\frac{1}{2}</code>");
        assert_eq!(paragraphs("a\n\nb"), "<p>a</p><p>b</p>");
        assert_eq!(strip_tags("<strong>a &amp; b</strong>"), "a & b");
    }
}
