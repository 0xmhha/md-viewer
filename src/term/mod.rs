//! Terminal rendering: Markdown to ANSI text.

mod block;
mod callout;
mod inline;
mod kitty;
mod math_symbols;
mod math_text;
mod prefix;
pub mod probe;
mod span;
mod style;
mod table;
mod wrap;

use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, parse_document};

use crate::parse;

/// How the document is laid out and whether ANSI styles are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Total output width in terminal cells.
    pub width: usize,
    /// Emit ANSI escape sequences (SGR styles and OSC 8 links).
    pub color: bool,
    /// Draw display math as images with the kitty graphics protocol.
    /// `None` prints its source instead.
    pub graphics: Option<probe::Graphics>,
}

/// Render a Markdown document for the terminal.
pub fn render(markdown: &str, opts: &RenderOptions) -> String {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    let footnotes = footnote_numbers(root);
    let mut writer = block::Writer::new(opts, &footnotes);
    writer.block(root, &mut prefix::Prefix::default());
    let mut out = writer.finish().join("\n");
    out.push('\n');
    out
}

/// Number each footnote by its first reference, as it is shown in text.
fn footnote_numbers<'a>(root: &'a AstNode<'a>) -> HashMap<String, u32> {
    let mut numbers = HashMap::new();
    for node in root.descendants() {
        if let NodeValue::FootnoteReference(f) = &node.data.borrow().value {
            numbers.entry(f.name.clone()).or_insert(f.ix);
        }
    }
    numbers
}
