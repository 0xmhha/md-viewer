//! Inline nodes to styled spans.

use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeValue};

use super::span::{self, Span};
use super::style::{Color, Style};

/// What inline rendering needs to know about the whole document.
pub struct InlineCtx<'a> {
    pub color: bool,
    /// Footnote name to the number shown for it.
    pub footnotes: &'a HashMap<String, u32>,
}

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"];

pub fn code_style() -> Style {
    Style::fg(Color::Green)
}

pub fn math_style() -> Style {
    Style::fg(Color::Magenta)
}

fn placeholder_style() -> Style {
    Style::fg(Color::BrightBlack).italic()
}

/// Spans for every child of `node`, starting from `base`.
pub fn children<'a>(node: &'a AstNode<'a>, base: &Style, ctx: &InlineCtx) -> Vec<Span> {
    let mut out = Vec::new();
    for child in node.children() {
        walk(child, base, ctx, &mut out);
    }
    out
}

/// Spans for one inline node.
pub fn node<'a>(node: &'a AstNode<'a>, base: &Style, ctx: &InlineCtx) -> Vec<Span> {
    let mut out = Vec::new();
    walk(node, base, ctx, &mut out);
    out
}

fn walk<'a>(node: &'a AstNode<'a>, style: &Style, ctx: &InlineCtx, out: &mut Vec<Span>) {
    let nested = |s: Style, out: &mut Vec<Span>| {
        for child in node.children() {
            walk(child, &s, ctx, out);
        }
    };
    match &node.data.borrow().value {
        NodeValue::Text(t) => out.push(Span::new(t.as_ref(), style.clone())),
        NodeValue::SoftBreak => out.push(Span::new(" ", style.clone())),
        NodeValue::LineBreak => out.push(Span::plain("\n")),
        NodeValue::Code(c) => out.push(Span::new(&c.literal, code_style())),
        NodeValue::Math(m) => out.push(Span::new(m.literal.trim(), math_style())),
        NodeValue::Emph => nested(style.clone().italic(), out),
        NodeValue::Strong => nested(style.clone().bold(), out),
        NodeValue::Strikethrough => nested(style.clone().strike(), out),
        NodeValue::Underline => nested(style.clone().underline(), out),
        NodeValue::Highlight => nested(
            style.clone().with_fg(Color::Black).with_bg(Color::Yellow),
            out,
        ),
        NodeValue::Link(link) => {
            let link_style = style.clone().with_fg(Color::Blue).underline();
            let label = children(node, &link_style.clone().with_link(&link.url), ctx);
            let shown = span::text(&label);
            out.extend(label);
            if !ctx.color && !same_address(&shown, &link.url) {
                out.push(Span::plain(format!(" <{}>", link.url)));
            }
        }
        NodeValue::Image(_) => {
            let alt = span::text(&children(node, style, ctx));
            out.push(Span::new(format!("[image: {alt}]"), placeholder_style()));
        }
        NodeValue::WikiLink(w) => {
            let label = span::text(&children(node, style, ctx));
            let shown = if label == w.url || (w.embed && is_size_spec(&label)) {
                wikilink_target(&w.url)
            } else {
                label
            };
            out.push(wikilink_span(&w.url, w.embed, shown, style));
        }
        NodeValue::FootnoteReference(f) => {
            let n = ctx.footnotes.get(&f.name).copied().unwrap_or(f.ix);
            out.push(Span::new(format!("[{n}]"), Style::fg(Color::Cyan)));
        }
        NodeValue::HtmlInline(html) if is_line_break(html) => out.push(Span::plain("\n")),
        NodeValue::HtmlInline(html) => out.push(Span::new(html, style.clone().dim())),
        NodeValue::Raw(raw) => out.push(Span::new(raw, style.clone())),
        NodeValue::EscapedTag(tag) => out.push(Span::new(*tag, style.clone())),
        NodeValue::CommentInline(_) => {}
        _ => nested(style.clone(), out),
    }
}

fn wikilink_span(url: &str, embed: bool, shown: String, style: &Style) -> Span {
    if !embed {
        return Span::new(shown, style.clone().with_fg(Color::Cyan).underline());
    }
    let ext = url.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if url.contains('.') && IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Span::new(format!("[image: {shown}]"), placeholder_style())
    } else {
        Span::new(format!("↳ {shown}"), Style::fg(Color::Cyan).italic())
    }
}

/// `note#Head#Sub` reads as `note › Head › Sub`, as Obsidian shows it.
fn wikilink_target(target: &str) -> String {
    target
        .split('#')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" › ")
}

/// Autolinks show their address already; `mailto:` and `http://` are
/// added by the parser.
fn same_address(shown: &str, url: &str) -> bool {
    let bare = ["mailto:", "https://", "http://"]
        .iter()
        .find_map(|p| url.strip_prefix(p))
        .unwrap_or(url);
    url.is_empty() || shown == url || shown == bare
}

/// `![[img.png|100]]` and `![[img.png|640x480]]` set a size, not a label.
fn is_size_spec(label: &str) -> bool {
    let mut parts = label.splitn(2, 'x');
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    parts.next().is_some_and(digits) && parts.next().is_none_or(digits)
}

fn is_line_break(html: &str) -> bool {
    let tag: String = html
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '/')
        .collect();
    tag.eq_ignore_ascii_case("<br>")
}

/// Remove a trailing Obsidian block ID such as ` ^p12` from the last span.
/// Only text in the block's own style counts, so a link label or code that
/// ends in `^x` is kept.
pub fn strip_block_id(spans: &mut Vec<Span>, base: &Style) {
    while spans.last().is_some_and(|s| s.text.trim().is_empty()) {
        spans.pop();
    }
    let Some(last) = spans.last().filter(|s| s.style == *base) else {
        return;
    };
    let trimmed = last.text.trim_end();
    let start = trimmed
        .char_indices()
        .rfind(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let at_word_start = start > 0 || spans.len() == 1 || ends_with_space(spans);
    if is_block_id(&trimmed[start..]) && at_word_start {
        let last = spans.last_mut().expect("checked above");
        last.text.truncate(start);
        let keep = last.text.trim_end().len();
        last.text.truncate(keep);
        if last.text.is_empty() {
            spans.pop();
        }
    }
}

fn ends_with_space(spans: &[Span]) -> bool {
    spans
        .len()
        .checked_sub(2)
        .and_then(|i| spans[i].text.chars().last())
        .is_some_and(char::is_whitespace)
}

fn is_block_id(s: &str) -> bool {
    s.strip_prefix('^').is_some_and(|id| {
        !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripped(parts: &[&str]) -> String {
        let mut spans: Vec<Span> = parts.iter().map(|p| Span::plain(*p)).collect();
        strip_block_id(&mut spans, &Style::default());
        span::text(&spans)
    }

    #[test]
    fn strips_trailing_block_id() {
        assert_eq!(stripped(&["text ^p12"]), "text");
        assert_eq!(stripped(&["^only"]), "");
        assert_eq!(stripped(&["a ", "^b-1  "]), "a ");
    }

    #[test]
    fn keeps_carets_that_are_not_block_ids() {
        assert_eq!(stripped(&["x^2"]), "x^2");
        assert_eq!(stripped(&["2^10"]), "2^10");
        assert_eq!(stripped(&["see ^ here"]), "see ^ here");
    }

    #[test]
    fn keeps_block_id_like_text_in_other_styles() {
        let mut spans = vec![Span::plain("see "), Span::new("^p1", code_style())];
        strip_block_id(&mut spans, &Style::default());
        assert_eq!(span::text(&spans), "see ^p1");
    }

    #[test]
    fn wikilink_targets_read_like_obsidian() {
        assert_eq!(wikilink_target("note#Head"), "note › Head");
        assert_eq!(wikilink_target("#Head"), "Head");
    }

    #[test]
    fn recognises_br_tags() {
        assert!(is_line_break("<br>"));
        assert!(is_line_break("<br />"));
        assert!(!is_line_break("<b>"));
    }
}
