//! LaTeX (the subset FreeSheet's content uses) to a small tree.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Main,
    MainBold,
    MathItalic,
    Ams,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Accent {
    Hat,
    Bar,
    Dot,
    Vec,
    Overline,
    Underline,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Char { ch: char, class: Class, font: Font },
    /// Upright text (\text, operator names), drawn in the Main font.
    Text { text: String, class: Class, bold: bool },
    Group(Vec<Node>),
    Frac { num: Vec<Node>, den: Vec<Node>, display: Option<bool> },
    Sqrt { index: Option<Vec<Node>>, body: Vec<Node> },
    Scripts { base: Box<Node>, sup: Option<Vec<Node>>, sub: Option<Vec<Node>> },
    Accent { kind: Accent, body: Vec<Node> },
    Delimited { left: Option<char>, body: Vec<Node>, right: Option<char> },
    /// A delimiter of a fixed bigger size (\big).
    Big { ch: char, size: u8, class: Class },
    /// A large operator (\sum), with limits in display style.
    BigOp { ch: char },
    /// An operator name with limits in display style (\lim).
    Limits { text: String },
    Underbrace { body: Vec<Node>, label: Option<Vec<Node>> },
    Boxed(Vec<Node>),
    XArrow { above: Vec<Node> },
    Space(f32),
    /// Switch to display style for the rest of the group.
    DisplayStyle,
    /// Something we can't draw: shown as its source, in red.
    Error(String),
}

fn symbol(cmd: &str) -> Option<(char, Class, Font)> {
    use Class::*;
    use Font::*;
    Some(match cmd {
        "times" => ('×', Bin, Main),
        "div" => ('÷', Bin, Main),
        "cdot" => ('⋅', Bin, Main),
        "pm" => ('±', Bin, Main),
        "mp" => ('∓', Bin, Main),
        "cup" => ('∪', Bin, Main),
        "cap" => ('∩', Bin, Main),
        "wedge" | "land" => ('∧', Bin, Main),
        "vee" | "lor" => ('∨', Bin, Main),
        "ast" => ('∗', Bin, Main),
        "star" => ('⋆', Bin, Main),
        "circ" => ('∘', Bin, Main),
        "bullet" => ('∙', Bin, Main),
        "setminus" => ('∖', Bin, Main),
        "approx" => ('≈', Rel, Main),
        "neq" | "ne" => ('≠', Rel, Main),
        "leq" | "le" => ('≤', Rel, Main),
        "geq" | "ge" => ('≥', Rel, Main),
        "leqslant" => ('⩽', Rel, Ams),
        "geqslant" => ('⩾', Rel, Ams),
        "ll" => ('≪', Rel, Main),
        "gg" => ('≫', Rel, Main),
        "equiv" => ('≡', Rel, Main),
        "sim" => ('∼', Rel, Main),
        "simeq" => ('≃', Rel, Main),
        "cong" => ('≅', Rel, Main),
        "propto" => ('∝', Rel, Main),
        "in" => ('∈', Rel, Main),
        "notin" => ('∉', Rel, Main),
        "ni" => ('∋', Rel, Main),
        "subset" => ('⊂', Rel, Main),
        "supset" => ('⊃', Rel, Main),
        "subseteq" => ('⊆', Rel, Main),
        "supseteq" => ('⊇', Rel, Main),
        "mid" => ('∣', Rel, Main),
        "perp" => ('⊥', Rel, Main),
        "parallel" => ('∥', Rel, Main),
        "to" | "rightarrow" => ('→', Rel, Main),
        "leftarrow" | "gets" => ('←', Rel, Main),
        "Rightarrow" => ('⇒', Rel, Main),
        "Leftarrow" => ('⇐', Rel, Main),
        "leftrightarrow" => ('↔', Rel, Main),
        "Leftrightarrow" => ('⇔', Rel, Main),
        "implies" | "Longrightarrow" => ('⟹', Rel, Main),
        "iff" | "Longleftrightarrow" => ('⟺', Rel, Main),
        "longrightarrow" => ('⟶', Rel, Main),
        "mapsto" => ('↦', Rel, Main),
        "therefore" => ('∴', Rel, Ams),
        "because" => ('∵', Rel, Ams),
        "infty" => ('∞', Ord, Main),
        "ell" => ('ℓ', Ord, Main),
        "angle" => ('∠', Ord, Main),
        "triangle" => ('△', Ord, Main),
        "square" => ('□', Ord, Ams),
        "degree" => ('°', Ord, Main),
        "prime" => ('′', Ord, Main),
        "partial" => ('∂', Ord, Main),
        "nabla" => ('∇', Ord, Main),
        "forall" => ('∀', Ord, Main),
        "exists" => ('∃', Ord, Main),
        "emptyset" | "varnothing" => ('∅', Ord, Main),
        "neg" | "lnot" => ('¬', Ord, Main),
        "ldots" | "dots" => ('…', Inner, Main),
        "cdots" => ('⋯', Inner, Main),
        "vdots" => ('⋮', Ord, Main),
        "%" => ('%', Ord, Main),
        "$" => ('$', Ord, Main),
        "&" => ('&', Ord, Main),
        "#" => ('#', Ord, Main),
        "_" => ('_', Ord, Main),
        "{" | "lbrace" => ('{', Open, Main),
        "}" | "rbrace" => ('}', Close, Main),
        "langle" => ('⟨', Open, Main),
        "rangle" => ('⟩', Close, Main),
        "lfloor" => ('⌊', Open, Main),
        "rfloor" => ('⌋', Close, Main),
        "lceil" => ('⌈', Open, Main),
        "rceil" => ('⌉', Close, Main),
        "vert" | "lvert" | "rvert" => ('|', Ord, Main),
        "Vert" => ('‖', Ord, Main),
        // Greek: lowercase italic, uppercase upright.
        "alpha" => ('α', Ord, MathItalic),
        "beta" => ('β', Ord, MathItalic),
        "gamma" => ('γ', Ord, MathItalic),
        "delta" => ('δ', Ord, MathItalic),
        "epsilon" => ('ϵ', Ord, MathItalic),
        "varepsilon" => ('ε', Ord, MathItalic),
        "zeta" => ('ζ', Ord, MathItalic),
        "eta" => ('η', Ord, MathItalic),
        "theta" => ('θ', Ord, MathItalic),
        "vartheta" => ('ϑ', Ord, MathItalic),
        "iota" => ('ι', Ord, MathItalic),
        "kappa" => ('κ', Ord, MathItalic),
        "lambda" => ('λ', Ord, MathItalic),
        "mu" => ('μ', Ord, MathItalic),
        "nu" => ('ν', Ord, MathItalic),
        "xi" => ('ξ', Ord, MathItalic),
        "pi" => ('π', Ord, MathItalic),
        "rho" => ('ρ', Ord, MathItalic),
        "sigma" => ('σ', Ord, MathItalic),
        "tau" => ('τ', Ord, MathItalic),
        "upsilon" => ('υ', Ord, MathItalic),
        "phi" => ('ϕ', Ord, MathItalic),
        "varphi" => ('φ', Ord, MathItalic),
        "chi" => ('χ', Ord, MathItalic),
        "psi" => ('ψ', Ord, MathItalic),
        "omega" => ('ω', Ord, MathItalic),
        "Gamma" => ('Γ', Ord, Main),
        "Delta" => ('Δ', Ord, Main),
        "Theta" => ('Θ', Ord, Main),
        "Lambda" => ('Λ', Ord, Main),
        "Xi" => ('Ξ', Ord, Main),
        "Pi" => ('Π', Ord, Main),
        "Sigma" => ('Σ', Ord, Main),
        "Phi" => ('Φ', Ord, Main),
        "Psi" => ('Ψ', Ord, Main),
        "Omega" => ('Ω', Ord, Main),
        _ => return None,
    })
}

const OPERATORS: &[&str] = &["sin", "cos", "tan", "sec", "csc", "cot", "log", "ln", "exp", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "det", "gcd", "deg", "dim", "ker", "arg"];
const LIMIT_OPS: &[&str] = &["lim", "max", "min", "sup", "inf", "limsup", "liminf"];

fn char_node(c: char) -> Node {
    use Class::*;
    use Font::*;
    let (class, font) = match c {
        'a'..='z' | 'A'..='Z' => (Ord, MathItalic),
        'α'..='ω' => (Ord, MathItalic),
        '0'..='9' => (Ord, Main),
        '+' | '×' | '÷' | '±' | '·' | '∓' => (Bin, Main),
        '-' | '−' => return Node::Char { ch: '−', class: Bin, font: Main },
        '*' => return Node::Char { ch: '∗', class: Bin, font: Main },
        '=' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '→' | '⇒' | '≡' | ':' => (Rel, Main),
        '(' | '[' => (Open, Main),
        ')' | ']' => (Close, Main),
        '!' | '?' => (Close, Main),
        ',' | ';' => (Punct, Main),
        '\'' => return Node::Char { ch: '′', class: Ord, font: Main },
        _ => (Ord, Main),
    };
    Node::Char { ch: c, class, font }
}

pub struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    pub fn parse(tex: &str) -> Vec<Node> {
        let mut p = Parser { s: tex.chars().collect(), i: 0 };
        p.list(None)
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// A command name after a backslash.
    fn command(&mut self) -> String {
        self.i += 1; // the backslash
        let start = self.i;
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                self.i += 1;
            }
        } else if self.peek().is_some() {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// Parse until `end` (a closing char) or the end of input. `\right` also ends a list.
    fn list(&mut self, end: Option<char>) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        loop {
            self.skip_ws();
            let Some(c) = self.peek() else { break };
            if Some(c) == end {
                self.i += 1;
                break;
            }
            if c == '\\' && self.starts_with_cmd("right") {
                break;
            }
            match c {
                '^' | '_' => {
                    self.i += 1;
                    let arg = self.argument();
                    let base = out.pop().unwrap_or(Node::Group(vec![]));
                    let (b, mut sup, mut sub) = match base {
                        Node::Scripts { base, sup, sub } => (*base, sup, sub),
                        other => (other, None, None),
                    };
                    if c == '^' {
                        // x'^2 style: append to an existing sup.
                        if let Some(s) = sup.as_mut() { s.extend(arg) } else { sup = Some(arg) }
                    } else {
                        sub = Some(arg);
                    }
                    out.push(Node::Scripts { base: Box::new(b), sup, sub });
                }
                '\'' => {
                    self.i += 1;
                    let base = out.pop().unwrap_or(Node::Group(vec![]));
                    let prime = Node::Char { ch: '′', class: Class::Ord, font: Font::Main };
                    out.push(match base {
                        Node::Scripts { base, sup, sub } => {
                            let mut s = sup.unwrap_or_default();
                            s.push(prime);
                            Node::Scripts { base, sup: Some(s), sub }
                        }
                        other => Node::Scripts { base: Box::new(other), sup: Some(vec![prime]), sub: None },
                    });
                }
                '{' => {
                    self.i += 1;
                    // {,} is a decimal comma: no space after it.
                    let g = self.list(Some('}'));
                    if g.len() == 1 {
                        if let Node::Char { ch: ',', font, .. } = g[0] {
                            out.push(Node::Char { ch: ',', class: Class::Ord, font });
                            continue;
                        }
                    }
                    out.push(Node::Group(g));
                }
                '}' => {
                    self.i += 1; // stray
                }
                '~' => {
                    self.i += 1;
                    out.push(Node::Space(0.333));
                }
                '&' => {
                    self.i += 1;
                    out.push(Node::Space(0.5));
                }
                '\\' => {
                    if let Some(n) = self.cmd_node() {
                        out.push(n);
                    }
                }
                _ => {
                    self.i += 1;
                    out.push(char_node(c));
                }
            }
        }
        out
    }

    fn starts_with_cmd(&self, name: &str) -> bool {
        let lit: Vec<char> = format!("\\{name}").chars().collect();
        self.s.len() >= self.i + lit.len()
            && self.s[self.i..self.i + lit.len()] == lit[..]
            && !self.s.get(self.i + lit.len()).is_some_and(|c| c.is_ascii_alphabetic())
    }

    /// One argument: a {group} or a single token.
    fn argument(&mut self) -> Vec<Node> {
        self.skip_ws();
        match self.peek() {
            Some('{') => {
                self.i += 1;
                self.list(Some('}'))
            }
            Some('\\') => self.cmd_node().into_iter().collect(),
            Some(c) => {
                self.i += 1;
                vec![char_node(c)]
            }
            None => vec![],
        }
    }

    /// Raw text of a {group} (for \text).
    fn raw_group(&mut self) -> String {
        self.skip_ws();
        if self.peek() != Some('{') {
            return self.peek().map(|c| {
                self.i += 1;
                c.to_string()
            }).unwrap_or_default();
        }
        self.i += 1;
        let mut depth = 1;
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == '\\' {
                self.i += 2;
                continue;
            }
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    let s: String = self.s[start..self.i].iter().collect();
                    self.i += 1;
                    return s;
                }
            }
            self.i += 1;
        }
        self.s[start..].iter().collect()
    }

    fn optional(&mut self) -> Option<Vec<Node>> {
        self.skip_ws();
        if self.peek() != Some('[') {
            return None;
        }
        self.i += 1;
        Some(self.list(Some(']')))
    }

    fn delimiter(&mut self) -> Option<char> {
        self.skip_ws();
        let c = self.peek()?;
        if c == '\\' {
            let save = self.i;
            let name = self.command();
            return match name.as_str() {
                "{" | "lbrace" => Some('{'),
                "}" | "rbrace" => Some('}'),
                "|" | "Vert" => Some('‖'),
                "vert" | "lvert" | "rvert" => Some('|'),
                "langle" => Some('⟨'),
                "rangle" => Some('⟩'),
                "lfloor" => Some('⌊'),
                "rfloor" => Some('⌋'),
                "lceil" => Some('⌈'),
                "rceil" => Some('⌉'),
                _ => {
                    self.i = save;
                    None
                }
            };
        }
        self.i += 1;
        if c == '.' { None } else { Some(c) }
    }

    fn cmd_node(&mut self) -> Option<Node> {
        let name = self.command();
        let n = match name.as_str() {
            "frac" => Node::Frac { num: self.argument(), den: self.argument(), display: None },
            "dfrac" => Node::Frac { num: self.argument(), den: self.argument(), display: Some(true) },
            "tfrac" => Node::Frac { num: self.argument(), den: self.argument(), display: Some(false) },
            "sqrt" => {
                let index = self.optional();
                Node::Sqrt { index, body: self.argument() }
            }
            "text" | "textrm" | "mathrm" | "textnormal" | "textup" | "operatorname" | "mbox" | "textbf" | "mathbf" | "textit" | "mathit" => {
                let raw = self.raw_group();
                let text = raw
                    .replace("\\,", "\u{2009}")
                    .replace("\\ ", " ")
                    .replace("\\;", "\u{2005}")
                    .replace("\\%", "%")
                    .replace("\\$", "$")
                    .replace("\\&", "&")
                    .replace("{,}", ",")
                    .replace(['{', '}'], "");
                let bold = name == "textbf" || name == "mathbf";
                if name == "mathit" || name == "textit" {
                    return Some(Node::Group(text.chars().map(|c| Node::Char { ch: c, class: Class::Ord, font: Font::MathItalic }).collect()));
                }
                Node::Text { text, class: if name == "operatorname" { Class::Op } else { Class::Ord }, bold }
            }
            "mathbb" => {
                let raw = self.raw_group();
                Node::Group(raw.chars().map(|c| Node::Char { ch: c, class: Class::Ord, font: Font::Ams }).collect())
            }
            "hat" | "widehat" => Node::Accent { kind: Accent::Hat, body: self.argument() },
            "bar" => Node::Accent { kind: Accent::Bar, body: self.argument() },
            "overline" => Node::Accent { kind: Accent::Overline, body: self.argument() },
            "underline" => Node::Accent { kind: Accent::Underline, body: self.argument() },
            "dot" => Node::Accent { kind: Accent::Dot, body: self.argument() },
            "vec" | "overrightarrow" => Node::Accent { kind: Accent::Vec, body: self.argument() },
            "left" => {
                let left = self.delimiter();
                let body = self.list(None);
                let right = if self.starts_with_cmd("right") {
                    self.command();
                    self.delimiter()
                } else {
                    None
                };
                Node::Delimited { left, body, right }
            }
            "right" => return None,
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" => {
                let size = match name.trim_end_matches(['l', 'r']) { "big" => 1, "Big" => 2, "bigg" => 3, _ => 4 };
                let ch = self.delimiter()?;
                let class = if "([{⟨⌊⌈".contains(ch) { Class::Open } else if ")]}⟩⌋⌉".contains(ch) { Class::Close } else { Class::Ord };
                Node::Big { ch, size, class }
            }
            "sum" => Node::BigOp { ch: '∑' },
            "prod" => Node::BigOp { ch: '∏' },
            "int" => Node::BigOp { ch: '∫' },
            "underbrace" => {
                let body = self.argument();
                self.skip_ws();
                let label = if self.peek() == Some('_') {
                    self.i += 1;
                    Some(self.argument())
                } else {
                    None
                };
                Node::Underbrace { body, label }
            }
            "boxed" | "fbox" => Node::Boxed(self.argument()),
            "xrightarrow" => {
                let _ = self.optional();
                Node::XArrow { above: self.argument() }
            }
            "displaystyle" => Node::DisplayStyle,
            "textstyle" | "scriptstyle" | "limits" | "nolimits" | "nonumber" => return None,
            "," | "thinspace" => Node::Space(3.0 / 18.0),
            ":" | ">" | "medspace" => Node::Space(4.0 / 18.0),
            ";" | "thickspace" => Node::Space(5.0 / 18.0),
            "!" | "negthinspace" => Node::Space(-3.0 / 18.0),
            " " => Node::Space(0.333),
            "quad" => Node::Space(1.0),
            "qquad" => Node::Space(2.0),
            "\\" | "newline" | "cr" => Node::Space(1.0),
            "not" => {
                // \not= and friends: the next symbol with a stroke. Only \not= is common.
                self.skip_ws();
                if self.peek() == Some('=') {
                    self.i += 1;
                    Node::Char { ch: '≠', class: Class::Rel, font: Font::Main }
                } else {
                    return None;
                }
            }
            other => {
                if let Some((ch, class, font)) = symbol(other) {
                    Node::Char { ch, class, font }
                } else if OPERATORS.contains(&other) {
                    Node::Text { text: other.to_string(), class: Class::Op, bold: false }
                } else if LIMIT_OPS.contains(&other) {
                    Node::Limits { text: other.to_string() }
                } else {
                    Node::Error(format!("\\{other}"))
                }
            }
        };
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        let n = Parser::parse("\\dfrac{x^2}{3} \\times 1{,}5");
        assert!(matches!(n[0], Node::Frac { display: Some(true), .. }));
        assert!(matches!(n[2], Node::Char { ch: '1', .. }));
        assert!(matches!(n[3], Node::Char { ch: ',', class: Class::Ord, .. }));
        let n = Parser::parse("\\left( \\frac{1}{2} \\right)^2");
        assert!(matches!(&n[0], Node::Scripts { base, .. } if matches!(**base, Node::Delimited { left: Some('('), right: Some(')'), .. })));
        let n = Parser::parse("\\text{price incl. VAT} = \\sqrt[3]{8}");
        assert!(matches!(&n[0], Node::Text { text, .. } if text == "price incl. VAT"));
        assert!(matches!(n[2], Node::Sqrt { index: Some(_), .. }));
        assert!(matches!(Parser::parse("\\foo")[0], Node::Error(_)));
    }
}
