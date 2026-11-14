//! Obsidian callouts: a colored bar, an icon and a title.

use comrak::nodes::{AstNode, NodeCallout, NodeValue};

use super::block::Writer;
use super::inline;
use super::prefix::Prefix;
use super::span::Span;
use super::style::{Color, Style};

/// Color and icon for a callout type, following Obsidian's groups.
fn kind(callout_type: &str) -> (Color, &'static str) {
    match callout_type {
        "abstract" | "summary" | "tldr" => (Color::Cyan, "≡"),
        "info" => (Color::Blue, "ℹ"),
        "todo" => (Color::Blue, "☐"),
        "tip" | "hint" | "important" => (Color::Cyan, "✦"),
        "success" | "check" | "done" => (Color::Green, "✓"),
        "question" | "help" | "faq" => (Color::Yellow, "?"),
        "warning" | "caution" | "attention" => (Color::Yellow, "!"),
        "failure" | "fail" | "missing" | "danger" | "error" | "bug" => (Color::Red, "✕"),
        "example" => (Color::Magenta, "»"),
        "quote" | "cite" => (Color::BrightBlack, "❝"),
        crate::embed::CALLOUT => (Color::Cyan, "↳"),
        _ => (Color::Blue, "ℹ"),
    }
}

pub fn render<'a>(
    w: &mut Writer,
    node: &'a AstNode<'a>,
    callout: &NodeCallout,
    prefix: &mut Prefix,
) {
    let (color, icon) = kind(&callout.callout_type.to_ascii_lowercase());
    let mut inner = prefix.nest_same(vec![Span::new("│ ", Style::fg(color))]);
    let title_style = Style::fg(color).bold();
    let mut title = vec![Span::new(format!("{icon} "), title_style.clone())];
    let custom = node
        .first_child()
        .filter(|c| matches!(c.data.borrow().value, NodeValue::CalloutTitle));
    match custom {
        Some(t) => title.extend(inline::children(t, &title_style, &w.inline_ctx())),
        None => title.push(Span::new(callout.default_title(), title_style)),
    }
    w.emit_wrapped(&mut inner, &title);
    let body = node
        .children()
        .filter(|c| !matches!(c.data.borrow().value, NodeValue::CalloutTitle));
    w.blocks(body, &mut inner, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_share_a_group() {
        assert_eq!(kind("faq"), kind("question"));
        assert_eq!(kind("tldr"), kind("summary"));
        assert_eq!(kind("unknown"), kind("note"));
    }
}
