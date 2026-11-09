//! Markdown parsing and extraction of blocks that need special rendering.

use comrak::nodes::NodeValue;
use comrak::{Arena, Options, parse_document};

/// A block that cannot be printed as plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialBlock {
    Math { literal: String, display: bool },
    Mermaid { source: String },
}

/// Parser options: GFM plus the extensions used by Obsidian vaults.
pub fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.math_dollars = true;
    o.extension.wikilinks_title_after_pipe = true;
    o.extension.ofm_callouts = true;
    o.extension.ofm_comments = true;
    o.extension.footnotes = true;
    o.extension.highlight = true;
    o.extension.front_matter_delimiter = Some("---".into());
    o
}

/// Collect math and Mermaid blocks in document order.
pub fn special_blocks(markdown: &str) -> Vec<SpecialBlock> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &options());
    root.descendants()
        .filter_map(|node| match &node.data.borrow().value {
            NodeValue::Math(m) => Some(SpecialBlock::Math {
                literal: m.literal.clone(),
                display: m.display_math,
            }),
            NodeValue::CodeBlock(cb) if cb.info.trim() == "mermaid" => {
                Some(SpecialBlock::Mermaid {
                    source: cb.literal.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn math(literal: &str, display: bool) -> SpecialBlock {
        SpecialBlock::Math {
            literal: literal.into(),
            display,
        }
    }

    #[test]
    fn finds_inline_and_display_math_in_order() {
        let md = "Energy $E=mc^2$ here.\n\n$$\n\\frac{a}{b}\n$$\n";
        let blocks = special_blocks(md);
        assert_eq!(
            blocks,
            vec![math("E=mc^2", false), math("\n\\frac{a}{b}\n", true)]
        );
    }

    #[test]
    fn finds_mermaid_code_block() {
        let md = "```mermaid\nflowchart LR\n  A-->B\n```\n";
        assert_eq!(
            special_blocks(md),
            vec![SpecialBlock::Mermaid {
                source: "flowchart LR\n  A-->B\n".into()
            }]
        );
    }

    #[test]
    fn ignores_other_code_blocks_and_dollars_in_code() {
        let md = "```rust\nlet x = \"$a$\";\n```\n\n`$b$`\n";
        assert!(special_blocks(md).is_empty());
    }

    #[test]
    fn ignores_math_in_front_matter() {
        let md = "---\ntitle: $x$\n---\n\nbody\n";
        assert!(special_blocks(md).is_empty());
    }
}
