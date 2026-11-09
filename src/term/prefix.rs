//! Line prefixes for nested blocks (quote bars, list markers, indents).

use super::span::{self, Span};

/// The prefix of the next line and of the lines after it. A list item's
/// first line carries its marker; later lines carry only the indent.
#[derive(Debug, Clone, Default)]
pub struct Prefix {
    first: Option<Vec<Span>>,
    rest: Vec<Span>,
}

impl Prefix {
    /// Prefix for a block nested in `self`: `first` goes on the block's
    /// first line after the parent's own pending prefix, `rest` on the others.
    pub fn nest(&mut self, first: Vec<Span>, rest: Vec<Span>) -> Prefix {
        let mut head = self.take();
        head.extend(first);
        let mut tail = self.rest.clone();
        tail.extend(rest);
        Prefix {
            first: Some(head),
            rest: tail,
        }
    }

    /// Same prefix on every line, nested in `self`.
    pub fn nest_same(&mut self, both: Vec<Span>) -> Prefix {
        self.nest(both.clone(), both)
    }

    /// The prefix for the next line. The first-line prefix is used once.
    pub fn take(&mut self) -> Vec<Span> {
        self.first.take().unwrap_or_else(|| self.rest.clone())
    }

    /// Prefix of a line that is not the first one, without consuming.
    pub fn rest(&self) -> &[Span] {
        &self.rest
    }

    /// Cells the widest prefix uses.
    pub fn width(&self) -> usize {
        let first = self.first.as_deref().map_or(0, span::width);
        first.max(span::width(&self.rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_prefix_is_used_once() {
        let mut root = Prefix::default();
        let mut item = root.nest(vec![Span::plain("• ")], vec![Span::plain("  ")]);
        assert_eq!(span::text(&item.take()), "• ");
        assert_eq!(span::text(&item.take()), "  ");
    }

    #[test]
    fn nested_first_line_keeps_the_parent_marker() {
        let mut root = Prefix::default();
        let mut outer = root.nest(vec![Span::plain("• ")], vec![Span::plain("  ")]);
        let mut inner = outer.nest(vec![Span::plain("◦ ")], vec![Span::plain("  ")]);
        assert_eq!(span::text(&inner.take()), "• ◦ ");
        assert_eq!(span::text(&inner.take()), "    ");
    }
}
