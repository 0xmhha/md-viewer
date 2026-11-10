//! Inline math as Unicode text: `\alpha` becomes `α`, `x^2` becomes `x²`.
//! A formula with any construct the table does not know is left alone, so
//! the reader sees either a full conversion or the original source.

use super::math_symbols as sym;

/// Deepest nesting of groups and arguments followed before giving up, so
/// hostile input cannot exhaust the stack.
const MAX_DEPTH: usize = 64;

/// Convert a LaTeX formula to Unicode text, or `None` when it uses
/// something this converter does not handle.
pub fn to_unicode(latex: &str) -> Option<String> {
    let mut parser = Parser {
        chars: latex.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let out = parser.sequence(None)?;
    Some(out.trim().to_owned())
}

/// Converted text being built, with the spacing rules of one group.
#[derive(Default)]
struct Out {
    text: String,
    /// Drop the next space: the last thing written opens a pair.
    glue: bool,
    /// Put a space before the next letter: the last thing written is an
    /// operator name such as `sin`, or a script such as `^α` that would
    /// run into it.
    word_gap: bool,
    /// Where a fraction or root body starts that needs parentheses if a
    /// term follows it directly: `a/b` then `c` must read `(a/b)c`.
    loose: Option<usize>,
    /// The kind of script written last (`true` for a superscript), so a
    /// second one of the same kind is refused instead of merged.
    script: Option<bool>,
}

impl Out {
    /// Write `s` as is, applying the pending word gap.
    fn raw(&mut self, s: &str) {
        if self.word_gap && s.starts_with(char::is_alphanumeric) {
            self.text.push(' ');
        }
        self.text.push_str(s);
        self.glue = false;
        self.word_gap = false;
        self.script = None;
    }

    /// Parenthesize a loose fraction or root when a term comes next.
    fn settle(&mut self, term_follows: bool) {
        if let Some(start) = self.loose.take()
            && term_follows
        {
            self.text.insert(start, '(');
            self.text.push(')');
        }
    }

    fn push(&mut self, s: &str) {
        self.settle(starts_term(s));
        self.raw(s);
    }

    /// Write a group's text, keeping what matters about its end.
    fn push_group(&mut self, group: Out) {
        self.push(&group.text);
        let start = self.text.len() - group.text.len();
        self.loose = group.loose.map(|i| start + i);
        self.script = group.script;
    }

    /// Write a script right after its base. A second script of the same
    /// kind, as in `{x^2}^3`, would read as one, so it is refused.
    fn attach(&mut self, script: &Script) -> Option<()> {
        if self.script == Some(script.sup) {
            return None;
        }
        self.settle(true);
        self.word_gap = false;
        self.raw(&script.text);
        self.script = Some(script.sup);
        self.word_gap = script.fallback;
        Some(())
    }

    /// Write an operator name, apart from a term right before it.
    fn word(&mut self, s: &str) {
        self.settle(true);
        if self
            .text
            .ends_with(|c: char| c.is_alphanumeric() || sym::is_combining(c) || c == ')')
        {
            self.text.push(' ');
        }
        self.raw(s);
        self.word_gap = true;
    }

    /// Write `top/bottom`, in parentheses when a term is right before it;
    /// a term right after it is handled by `settle`.
    fn fraction(&mut self, text: &str) {
        self.settle(true);
        if self.text.ends_with(ends_term) {
            self.raw(&format!("({text})"));
        } else {
            self.raw(text);
            self.loose = Some(self.text.len() - text.len());
        }
    }

    /// Write a radical and its body; the body gets parentheses later if
    /// a term follows it, as in `√(2)x`.
    fn root(&mut self, radical: &str, body: &str) {
        self.settle(true);
        self.raw(radical);
        self.raw(body);
        if !body.starts_with('(') {
            self.loose = Some(self.text.len() - body.len());
        }
    }

    fn space(&mut self) {
        self.loose = None;
        if !self.glue && !self.text.is_empty() && !self.text.ends_with(' ') {
            self.text.push(' ');
        }
        self.word_gap = false;
    }

    /// Write a closing delimiter right after the text before it.
    fn close(&mut self, s: &str) {
        self.loose = None;
        let trimmed = self.text.trim_end().len();
        self.text.truncate(trimmed);
        self.push(s);
    }

    /// Write an opening delimiter; the space after it is dropped.
    fn open(&mut self, s: &str) {
        self.push(s);
        self.glue = true;
    }
}

/// A converted superscript or subscript.
struct Script {
    text: String,
    sup: bool,
    /// Written as `^x` or `_x` because Unicode has no script form.
    fallback: bool,
}

/// Whether text starting with this would be read as multiplying the term
/// before it.
fn starts_term(s: &str) -> bool {
    s.starts_with(|c: char| {
        c.is_alphanumeric() || sym::is_combining(c) || matches!(c, '(' | '[' | '√' | '∛' | '∜')
    })
}

/// Whether text ending with this character ends a term, so a fraction
/// after it needs parentheses.
fn ends_term(c: char) -> bool {
    c.is_alphanumeric() || sym::is_combining(c) || matches!(c, ')' | ']' | '/' | '′')
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    /// Run `f` one level deeper. Every recursive path goes through here,
    /// so nesting is bounded by `MAX_DEPTH`.
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        if self.depth >= MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// Text up to `close` (consumed), or to the end when `close` is `None`.
    fn sequence(&mut self, close: Option<char>) -> Option<String> {
        self.group(close).map(|out| out.text)
    }

    fn group(&mut self, close: Option<char>) -> Option<Out> {
        self.nested(|p| p.group_inner(close))
    }

    fn group_inner(&mut self, close: Option<char>) -> Option<Out> {
        let mut out = Out::default();
        loop {
            let Some(c) = self.peek() else {
                if close.is_some() {
                    return None;
                }
                break;
            };
            if Some(c) == close {
                self.pos += 1;
                break;
            }
            match c {
                c if c.is_whitespace() => {
                    self.skip_spaces();
                    out.space();
                }
                '{' => {
                    self.pos += 1;
                    let inner = self.group(Some('}'))?;
                    out.push_group(inner);
                }
                '^' | '_' => {
                    self.pos += 1;
                    let arg = self.argument()?;
                    out.attach(&scripted(&arg, c == '^'))?;
                }
                '\\' => self.command(&mut out)?,
                '}' => return None,
                _ => {
                    self.pos += 1;
                    out.push(&plain_char(c));
                }
            }
        }
        Some(out)
    }

    /// One argument: a `{group}`, a command or a single character.
    fn argument(&mut self) -> Option<String> {
        self.nested(Self::argument_inner)
    }

    fn argument_inner(&mut self) -> Option<String> {
        self.skip_spaces();
        match self.peek()? {
            '{' => {
                self.pos += 1;
                self.sequence(Some('}'))
            }
            '\\' => {
                let mut out = Out::default();
                self.command(&mut out)?;
                Some(out.text)
            }
            '}' | '^' | '_' | ']' => None,
            c => {
                self.pos += 1;
                Some(plain_char(c))
            }
        }
    }

    /// The name after a backslash: a run of letters or one other character.
    fn command_name(&mut self) -> Option<String> {
        let first = self.next()?;
        let mut name = String::from(first);
        if first.is_ascii_alphabetic() {
            while let Some(c) = self.peek().filter(char::is_ascii_alphabetic) {
                name.push(c);
                self.pos += 1;
            }
        }
        Some(name)
    }

    fn command(&mut self, out: &mut Out) -> Option<()> {
        self.pos += 1; // the backslash
        let name = self.command_name()?;
        let n = name.as_str();
        // A space after a command name only ends the name, except that
        // `a \le b` and `\alpha \in X` keep the spaces the author put
        // around an operator.
        let spaced_operator = sym::is_operator(n) && out.text.ends_with(' ');
        let mut gap_before_operator = false;
        if n.starts_with(|c: char| c.is_ascii_alphabetic()) && !spaced_operator {
            let start = self.pos;
            self.skip_spaces();
            gap_before_operator = self.pos > start && self.operator_follows();
        }
        self.command_body(n, out)?;
        if gap_before_operator {
            out.space();
        }
        Some(())
    }

    /// Whether the next token is an operator command such as `\le`.
    fn operator_follows(&self) -> bool {
        if self.peek() != Some('\\') {
            return false;
        }
        let name: String = self.chars[self.pos + 1..]
            .iter()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        sym::is_operator(&name)
    }

    fn command_body(&mut self, n: &str, out: &mut Out) -> Option<()> {
        match n {
            "," | ":" | ";" | " " | ">" | "\\" | "quad" | "qquad" | "enspace" | "thinspace" => {
                out.space();
            }
            "!" | "displaystyle" | "textstyle" | "scriptstyle" | "limits" | "nolimits" => {}
            "{" | "}" | "%" | "$" | "&" | "#" | "_" => out.push(n),
            "|" => out.push("‖"),
            "big" | "Big" | "bigg" | "Bigg" | "middle" => {}
            "left" | "bigl" | "Bigl" | "biggl" | "Biggl" => {
                let d = self.delimiter()?;
                out.open(&d);
            }
            "right" | "bigr" | "Bigr" | "biggr" | "Biggr" => {
                let d = self.delimiter()?;
                out.close(&d);
            }
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let top = self.argument()?;
                let bottom = self.argument()?;
                out.fraction(&format!("{}/{}", grouped(&top), grouped(&bottom)));
            }
            "sqrt" => {
                let root = self.sqrt_root()?;
                let body = self.argument()?;
                out.root(&root, &grouped(&body));
            }
            "text" | "textrm" | "textit" | "textbf" | "textsf" | "texttt" | "textnormal"
            | "mbox" => out.push(&self.raw_text()?),
            "mathrm" | "mathit" | "mathbf" | "mathsf" | "mathtt" | "mathnormal" | "boldsymbol"
            | "bm" => out.push(&self.argument()?),
            "operatorname" => {
                if self.peek() == Some('*') {
                    self.pos += 1;
                }
                out.word(&self.argument()?);
            }
            "mathbb" => out.push(&map_all(&self.argument()?, sym::double_struck)?),
            "mathcal" | "mathscr" => out.push(&map_all(&self.argument()?, sym::script)?),
            "not" => {
                let target = self.negated()?;
                out.push(&format!("{target}\u{338}"));
            }
            "mod" | "bmod" => {
                out.space();
                out.push("mod");
                out.space();
            }
            _ => self.simple_command(n, out)?,
        }
        Some(())
    }

    /// Commands that take no argument other than accents: symbols and
    /// operator names.
    fn simple_command(&mut self, n: &str, out: &mut Out) -> Option<()> {
        if let Some(mark) = sym::accent(n) {
            let arg = self.argument()?;
            if arg.is_empty() {
                return None;
            }
            let marked: String = arg
                .chars()
                .filter(|c| !c.is_whitespace())
                .flat_map(|c| [Some(c), (!sym::is_combining(c)).then_some(mark)])
                .flatten()
                .collect();
            out.push(&marked);
        } else if sym::is_function(n) {
            out.word(n);
        } else {
            let s = sym::symbol(n)?;
            if sym::opens(n) {
                out.open(s);
            } else if sym::closes(n) {
                out.close(s);
            } else {
                out.push(s);
            }
        }
        Some(())
    }

    /// The symbol after `\not`. It is read here rather than as an
    /// argument so the space after it stays, as in `a \not\in B`.
    fn negated(&mut self) -> Option<String> {
        self.skip_spaces();
        let target = match self.peek()? {
            '\\' => {
                self.pos += 1;
                let name = self.command_name()?;
                sym::symbol(&name)?.to_owned()
            }
            '{' => self.argument()?,
            '}' | '^' | '_' => return None,
            c => {
                self.pos += 1;
                c.to_string()
            }
        };
        (!target.trim().is_empty()).then_some(target)
    }

    /// The delimiter after `\left` or `\right`; `.` means none.
    fn delimiter(&mut self) -> Option<String> {
        self.skip_spaces();
        match self.next()? {
            '.' => Some(String::new()),
            '\\' => {
                let name = self.command_name()?;
                match name.as_str() {
                    "{" | "}" => Some(name),
                    "|" => Some("‖".to_owned()),
                    n => sym::symbol(n).map(str::to_owned),
                }
            }
            c => Some(c.to_string()),
        }
    }

    /// The radical sign for `\sqrt` and its optional `[n]`.
    fn sqrt_root(&mut self) -> Option<String> {
        self.skip_spaces();
        if self.peek() != Some('[') {
            return Some("√".to_owned());
        }
        self.pos += 1;
        let index = self.sequence(Some(']'))?;
        Some(match index.trim() {
            "" | "2" => "√".to_owned(),
            "3" => "∛".to_owned(),
            "4" => "∜".to_owned(),
            other => format!("{}√", map_all(other, sym::superscript)?),
        })
    }

    /// The literal text of a `\text{...}` argument.
    fn raw_text(&mut self) -> Option<String> {
        self.skip_spaces();
        if self.peek() != Some('{') {
            return self.argument();
        }
        self.pos += 1;
        let mut depth = 0usize;
        let mut text = String::new();
        loop {
            match self.next()? {
                '{' => depth += 1,
                '}' if depth == 0 => return Some(text),
                '}' => depth -= 1,
                '\\' => {
                    let name = self.command_name()?;
                    match name.as_str() {
                        "{" | "}" | "%" | "$" | "&" | "#" | "_" | " " => text.push_str(&name),
                        _ => return None,
                    }
                }
                c => text.push(c),
            }
        }
    }
}

/// How a character outside any command is shown.
fn plain_char(c: char) -> String {
    match c {
        '\'' => "′".to_owned(),
        '~' => " ".to_owned(),
        c => c.to_string(),
    }
}

/// A superscript or subscript: Unicode script characters when every
/// character has one, else `^` or `_` with the text, in parentheses when
/// it could be misread.
fn scripted(arg: &str, sup: bool) -> Script {
    let text: String = arg.chars().filter(|c| !c.is_whitespace()).collect();
    let map = if sup {
        sym::superscript
    } else {
        sym::subscript
    };
    if let Some(mapped) = map_all(&text, map) {
        return Script {
            text: mapped,
            sup,
            fallback: false,
        };
    }
    let marker = if sup { '^' } else { '_' };
    // `d_model` reads as one name, but `e^iπ` hides where the power ends.
    let needs_parens = if sup {
        letters(&text) > 1
    } else {
        letters(&text) > 1 && !is_atom(&text)
    };
    let text = if needs_parens {
        format!("{marker}({text})")
    } else {
        format!("{marker}{text}")
    };
    Script {
        text,
        sup,
        fallback: true,
    }
}

/// Map every character, or `None` when one has no mapping.
fn map_all(text: &str, map: fn(char) -> Option<char>) -> Option<String> {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .map(map)
        .collect()
}

/// Wrap `text` in parentheses unless it is a single term.
fn grouped(text: &str) -> String {
    let text = text.trim();
    if is_atom(text) {
        text.to_owned()
    } else {
        format!("({text})")
    }
}

/// Letters, digits and marks only: a name or a number such as `2x`,
/// `dₖ`, `1.5` or `√dₖ`.
fn is_atom(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| {
            c.is_alphanumeric() || sym::is_combining(c) || matches!(c, '.' | '′' | '√' | '∛' | '∜')
        })
}

/// Characters a reader sees, not counting combining marks.
fn letters(text: &str) -> usize {
    text.chars().filter(|c| !sym::is_combining(*c)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(src: &str) -> Option<String> {
        to_unicode(src)
    }

    #[track_caller]
    fn ok(src: &str, want: &str) {
        assert_eq!(conv(src).as_deref(), Some(want), "source: {src}");
    }

    #[test]
    fn plain_text_is_unchanged() {
        ok("E=mc", "E=mc");
        ok("a + b = c", "a + b = c");
        ok("QK", "QK");
    }

    #[test]
    fn symbols() {
        ok(r"a \to b \le \infty", "a → b ≤ ∞");
        ok(r"\alpha\beta", "αβ");
        ok(r"\Omega\cdot\pi", "Ω⋅π");
        ok(r"x \in \mathbb{R}", "x ∈ ℝ");
        ok(r"\{a\}", "{a}");
    }

    #[test]
    fn scripts_use_unicode_when_every_character_has_one() {
        ok("E=mc^2", "E=mc²");
        ok("d_k", "dₖ");
        ok("x^{n+1}", "xⁿ⁺¹");
        ok("e^{-x}", "e⁻ˣ");
        ok("W^T", "Wᵀ");
        ok("x_i^2", "xᵢ²");
        ok(r"90^\circ", "90°");
    }

    #[test]
    fn scripts_fall_back_to_markers() {
        ok("d_{model}", "d_model");
        ok(r"e^{i\pi}", "e^(iπ)");
        ok("x_{a,b}", "x_(a,b)");
        ok("x^q", "x^q");
    }

    #[test]
    fn fractions_and_roots() {
        ok(r"\frac{a}{b}", "a/b");
        ok(r"\frac{a+b}{2}", "(a+b)/2");
        ok(r"\frac{QK^T}{\sqrt{d_k}}", "QKᵀ/√dₖ");
        ok(r"\sqrt{x+1}", "√(x+1)");
        ok(r"\sqrt[3]{x}", "∛x");
        ok(r"\frac12", "1/2");
    }

    #[test]
    fn fonts_text_and_operators() {
        ok(r"\text{loss} = 0", "loss = 0");
        ok(r"\text{if } x", "if x");
        ok(r"\mathbf{W}x", "Wx");
        ok(r"\mathcal{L}", "ℒ");
        ok(r"\sin x + \log y", "sin x + log y");
        ok(r"\operatorname{softmax}(z)", "softmax(z)");
    }

    #[test]
    fn accents_combine() {
        ok(r"\hat{y}", "y\u{302}");
        ok(r"\vec v", "v\u{20d7}");
        ok(r"\bar{x}", "x\u{304}");
    }

    #[test]
    fn spacing_and_delimiters() {
        ok(r"a\,b\quad c", "a b c");
        ok(r"\left( x \right)", "(x)");
        ok(r"\left. x \right|", "x|");
        ok(r"\lVert x \rVert", "‖x‖");
        ok(r"\not\in", "∈\u{338}");
    }

    #[test]
    fn space_after_a_command_name_only_ends_it() {
        ok(r"\partial L/\partial W", "∂L/∂W");
        ok(r"d\times r", "d×r");
        ok(r"p(A\mid B)", "p(A∣B)");
        ok(r"x \in \mathbb{R}", "x ∈ ℝ");
        ok(r"h_t^\top \bar h_s", "hₜᵀh̄ₛ");
    }

    #[test]
    fn operator_names_are_set_apart_from_letters() {
        ok(r"\log_k n", "logₖ n");
        ok(r"v^\top\tanh(x)", "vᵀ tanh(x)");
        ok(r"-\log p", "-log p");
        ok(r"\sin\theta", "sin θ");
    }

    #[test]
    fn a_single_character_script_needs_no_parentheses() {
        ok(r"\mathcal L_\infty", "ℒ_∞");
        ok(r"\pi_\theta", "π_θ");
    }

    #[test]
    fn unknown_constructs_give_up() {
        for src in [
            r"\begin{matrix} a \end{matrix}",
            r"\overbrace{a}",
            r"\frac{a}",
            "x^",
            "{a",
            "a}",
            r"\unknowncommand",
        ] {
            assert_eq!(conv(src), None, "source: {src}");
        }
    }

    #[test]
    fn a_fraction_or_root_next_to_a_term_is_parenthesized() {
        ok(r"\frac{a}{b}c", "(a/b)c");
        ok(r"\frac{1}{2}\pi", "(1/2)π");
        ok(r"\frac{a}{b}\frac{c}{d}", "(a/b)(c/d)");
        ok(r"a/\frac{b}{c}", "a/(b/c)");
        ok(r"2\frac{1}{2}", "2(1/2)");
        ok(r"\frac{a}{b}^2", "(a/b)²");
        ok(r"{\frac{a}{b}}c", "(a/b)c");
        ok(r"\frac{a}{b} = c", "a/b = c");
        ok(r"\frac{a}{b}\cos y", "(a/b) cos y");
        ok(r"\sqrt{2}x", "√(2)x");
        ok(r"\sqrt{x}y", "√(x)y");
        ok(r"1/\sqrt{d_k}", "1/√dₖ");
    }

    #[test]
    fn stacked_scripts_of_one_kind_give_up() {
        assert_eq!(conv("{x^2}^3"), None);
        assert_eq!(conv("x^2^3"), None);
        assert_eq!(conv("x_1_2"), None);
        ok("x_i^2", "xᵢ²");
    }

    #[test]
    fn a_fallback_script_is_kept_apart_from_the_next_letter() {
        ok(r"x^\alpha y", "x^α y");
        ok(r"x_\alpha y", "x_α y");
    }

    #[test]
    fn review_regressions() {
        ok(r"\operatorname*{arg\,max}_x f", "arg maxₓ f");
        ok(r"a \not\in B", "a ∈\u{338} B");
        ok(r"a \mod b", "a mod b");
        ok(r"\hat{x}\cos y", "x\u{302} cos y");
        assert_eq!(conv(r"\not{} a"), None);
        assert_eq!(conv(r"\hat{\not{}}"), None);
    }

    #[test]
    fn deep_nesting_gives_up_without_overflow() {
        for src in [
            "{".repeat(100_000) + &"}".repeat(100_000),
            r"\hat".repeat(100_000) + "x",
            r"\not".repeat(100_000) + "x",
            "x^{".repeat(100_000) + "y" + &"}".repeat(100_000),
            r"\frac".repeat(100_000),
        ] {
            assert_eq!(conv(&src), None);
        }
    }

    /// How much of a real corpus converts:
    /// `MDV_CORPUS=<file listing paths> cargo test --lib math_text -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs MDV_CORPUS"]
    fn corpus_coverage() {
        use crate::parse::{SpecialBlock, special_blocks};
        use std::collections::BTreeMap;

        let list = std::env::var("MDV_CORPUS").expect("MDV_CORPUS is not set");
        let paths = std::fs::read_to_string(list).expect("cannot read MDV_CORPUS");
        let (mut total, mut converted) = (0, 0);
        let mut missing: BTreeMap<String, usize> = BTreeMap::new();
        for path in paths.lines().filter(|p| !p.trim().is_empty()) {
            let md = std::fs::read_to_string(path).expect(path);
            for block in special_blocks(&md) {
                let SpecialBlock::Math {
                    literal,
                    display: false,
                } = block
                else {
                    continue;
                };
                total += 1;
                if to_unicode(&literal).is_some() {
                    converted += 1;
                } else {
                    *missing.entry(first_unknown(&literal)).or_default() += 1;
                }
            }
        }
        let mut by_count: Vec<_> = missing.into_iter().collect();
        by_count.sort_by_key(|a| std::cmp::Reverse(a.1));
        eprintln!("inline math: {converted}/{total} converted");
        for (cmd, n) in by_count.iter().take(30) {
            eprintln!("{n:5}  {cmd}");
        }
    }

    /// The first command in `src` the converter rejects on its own, for the
    /// coverage report.
    fn first_unknown(src: &str) -> String {
        let mut rest = src;
        while let Some(i) = rest.find('\\') {
            let tail = &rest[i + 1..];
            let len = tail
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(tail.len())
                .max(1.min(tail.len()));
            let cmd = &tail[..len];
            if to_unicode(&format!("\\{cmd}{{x}}{{y}}")).is_none()
                && to_unicode(&format!("\\{cmd}")).is_none()
            {
                return format!("\\{cmd}");
            }
            rest = &tail[len..];
        }
        "(structure)".to_owned()
    }
}
