//! Word wrapping of styled spans by display width.

use unicode_width::UnicodeWidthChar;

use super::span::{self, Span};

enum Token {
    /// Consecutive non-space text, possibly crossing style boundaries.
    Word(Vec<Span>),
    Space(Span),
    Break,
}

#[derive(PartialEq)]
enum Class {
    Word,
    Space,
    Break,
}

fn class(c: char) -> Class {
    match c {
        '\n' => Class::Break,
        // No-break spaces glue the words around them.
        '\u{a0}' | '\u{2007}' | '\u{202f}' => Class::Word,
        c if c.is_whitespace() => Class::Space,
        _ => Class::Word,
    }
}

fn tokens(spans: &[Span]) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    for s in spans {
        let mut rest = s.text.as_str();
        while let Some(first) = rest.chars().next() {
            let kind = class(first);
            let end = if kind == Class::Break {
                first.len_utf8()
            } else {
                rest.find(|c| class(c) != kind).unwrap_or(rest.len())
            };
            let (chunk, tail) = rest.split_at(end);
            rest = tail;
            let piece = Span::new(chunk, s.style.clone());
            match (kind, out.last_mut()) {
                (Class::Break, _) => out.push(Token::Break),
                (Class::Space, Some(Token::Space(_))) => {}
                (Class::Space, _) => out.push(Token::Space(Span::new(" ", s.style.clone()))),
                (Class::Word, Some(Token::Word(pieces))) => pieces.push(piece),
                (Class::Word, _) => out.push(Token::Word(vec![piece])),
            }
        }
    }
    out
}

struct Lines {
    width: usize,
    done: Vec<Vec<Span>>,
    line: Vec<Span>,
    used: usize,
    space: Option<Span>,
}

impl Lines {
    fn new(width: usize) -> Self {
        Lines {
            width: width.max(1),
            done: Vec::new(),
            line: Vec::new(),
            used: 0,
            space: None,
        }
    }

    fn break_line(&mut self) {
        self.done.push(std::mem::take(&mut self.line));
        self.used = 0;
        self.space = None;
    }

    fn push_word(&mut self, pieces: Vec<Span>) {
        let word = span::width(&pieces);
        let gap = self.space.as_ref().map_or(0, Span::width);
        let fits = self.used + gap + word <= self.width;
        if self.used > 0 && !fits && starts_breakable(&pieces) {
            // Chinese and Japanese break between any two characters.
            if let Some(space) = self.space.take() {
                self.used += space.width();
                self.line.push(space);
            }
            self.push_split(pieces);
            return;
        }
        if self.used > 0 && !fits {
            self.break_line();
        } else if let Some(space) = self.space.take() {
            self.used += space.width();
            self.line.push(space);
        }
        if word <= self.width {
            self.used += word;
            self.line.extend(pieces);
        } else {
            self.push_split(pieces);
        }
    }

    /// Break a word longer than the line at character boundaries.
    fn push_split(&mut self, pieces: Vec<Span>) {
        for piece in pieces {
            for c in piece.text.chars() {
                let w = c.width().unwrap_or(0);
                if self.used > 0 && self.used + w > self.width {
                    self.break_line();
                }
                self.used += w;
                match self.line.last_mut() {
                    Some(last) if last.style == piece.style => last.text.push(c),
                    _ => self.line.push(Span::new(c, piece.style.clone())),
                }
            }
        }
    }

    fn finish(mut self) -> Vec<Vec<Span>> {
        if !self.line.is_empty() {
            self.done.push(self.line);
        }
        self.done
    }
}

/// Wide characters other than Hangul may break anywhere; Korean breaks at
/// spaces like Latin text.
fn starts_breakable(pieces: &[Span]) -> bool {
    let first = pieces.iter().find_map(|p| p.text.chars().next());
    first.is_some_and(|c| {
        let hangul = matches!(c, '\u{1100}'..='\u{11ff}' | '\u{3130}'..='\u{318f}' | '\u{ac00}'..='\u{d7a3}');
        c.width() == Some(2) && !hangul
    })
}

/// Split spans into lines no wider than `width` cells. Whitespace runs
/// collapse to one space and vanish at line edges.
pub fn wrap(spans: &[Span], width: usize) -> Vec<Vec<Span>> {
    let mut lines = Lines::new(width);
    for token in tokens(spans) {
        match token {
            Token::Break => lines.break_line(),
            Token::Space(s) if lines.used > 0 => lines.space = Some(s),
            Token::Space(_) => {}
            Token::Word(pieces) => lines.push_word(pieces),
        }
    }
    lines.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::style::Style;

    fn texts(lines: &[Vec<Span>]) -> Vec<String> {
        lines.iter().map(|l| span::text(l)).collect()
    }

    #[test]
    fn wraps_at_word_boundaries() {
        let out = wrap(&[Span::plain("aa bb cc")], 5);
        assert_eq!(texts(&out), ["aa bb", "cc"]);
    }

    #[test]
    fn keeps_a_word_together_across_styles() {
        let bold = Style::default().bold();
        let spans = [
            Span::plain("xx "),
            Span::new("bold", bold),
            Span::plain(", yy"),
        ];
        assert_eq!(texts(&wrap(&spans, 6)), ["xx", "bold,", "yy"]);
    }

    #[test]
    fn splits_words_longer_than_the_line() {
        assert_eq!(
            texts(&wrap(&[Span::plain("abcdefg")], 3)),
            ["abc", "def", "g"]
        );
    }

    #[test]
    fn hard_breaks_start_new_lines() {
        assert_eq!(texts(&wrap(&[Span::plain("a\nb")], 10)), ["a", "b"]);
    }

    #[test]
    fn collapses_and_trims_whitespace() {
        assert_eq!(texts(&wrap(&[Span::plain("  a   b  ")], 10)), ["a b"]);
    }
}
