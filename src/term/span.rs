//! Styled text runs and how a line of them is printed.

use unicode_width::UnicodeWidthStr;

use super::style::{Style, paint, sanitize};

/// A run of text that shares one style. A text of `"\n"` is a hard break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    /// The text is sanitized here, so widths are measured on what is shown.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Span {
            text: sanitize(&text.into()),
            style,
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Span::new(text, Style::default())
    }

    pub fn width(&self) -> usize {
        self.text.width()
    }
}

/// Display width of a run of spans, in terminal cells.
pub fn width(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// Concatenated text of spans without styles.
pub fn text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Print one line. Adjacent spans with the same style share one escape
/// sequence, and trailing whitespace is dropped.
pub fn paint_line(spans: &[Span], color: bool) -> String {
    let mut merged: Vec<Span> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if last.style == span.style => last.text.push_str(&span.text),
            _ => merged.push(span.clone()),
        }
    }
    while let Some(last) = merged.last_mut() {
        let trimmed = last.text.trim_end().len();
        last.text.truncate(trimmed);
        if !last.text.is_empty() {
            break;
        }
        merged.pop();
    }
    merged
        .iter()
        .map(|s| paint(&s.text, &s.style, color))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::style::Color;

    #[test]
    fn wide_characters_count_twice() {
        assert_eq!(width(&[Span::plain("한a")]), 3);
    }

    #[test]
    fn merges_same_style_and_trims_end() {
        let s = Style::fg(Color::Red);
        let line = [Span::new("a", s.clone()), Span::new("b  ", s)];
        assert_eq!(paint_line(&line, true), "\x1b[31mab\x1b[0m");
    }
}
