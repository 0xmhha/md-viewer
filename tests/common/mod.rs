//! Shared test helpers.

use comrak::nodes::NodeValue;
use comrak::{Arena, parse_document};
use md_viewer::parse;

/// Markdown syntax that must not survive rendering outside code and math.
pub const RAW_SYNTAX: [&str; 5] = ["$$", "[[", "> [!", "==", "```"];

/// Count raw syntax left in `rendered`, ignoring what code blocks, code
/// spans and math literals contain (those are printed as written).
pub fn raw_syntax_residue(markdown: &str, rendered: &str) -> Vec<(&'static str, usize)> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    let literals: Vec<String> = root
        .descendants()
        .filter_map(|n| match &n.data.borrow().value {
            NodeValue::CodeBlock(c) => Some(c.literal.clone()),
            NodeValue::Code(c) => Some(c.literal.clone()),
            NodeValue::Math(m) => Some(m.literal.clone()),
            _ => None,
        })
        .collect();
    RAW_SYNTAX
        .iter()
        .filter_map(|tok| {
            let allowed: usize = literals.iter().map(|l| l.matches(tok).count()).sum();
            let found = rendered.matches(tok).count();
            (found > allowed).then(|| (*tok, found - allowed))
        })
        .collect()
}
