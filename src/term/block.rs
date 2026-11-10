//! Block nodes to lines.

use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeCodeBlock, NodeList, NodeValue};

use super::inline::{self, InlineCtx};
use super::kitty;
use super::prefix::Prefix;
use super::probe::Graphics;
use super::span::{self, Span, paint_line};
use super::style::{Color, Style};
use super::wrap::wrap;
use super::{RenderOptions, callout, table};
use crate::math_image::{self, Png};

/// Narrowest text column kept even when prefixes eat the whole width.
const MIN_TEXT_WIDTH: usize = 10;
const BULLETS: [&str; 3] = ["•", "◦", "▪"];
const DISPLAY_MATH_INDENT: &str = "    ";
/// Smallest scale a too-wide formula is shrunk to before its source is
/// shown instead.
const MIN_MATH_SCALE: f32 = 0.6;
/// Quotes, lists and callouts nested deeper than this print as plain text,
/// so hostile input cannot exhaust the stack or grow prefixes without bound.
const MAX_NESTING: usize = 32;

/// Collects the printed lines of a document.
pub struct Writer<'c> {
    opts: RenderOptions,
    footnotes: &'c HashMap<String, u32>,
    lines: Vec<String>,
    list_depth: usize,
    nesting: usize,
}

enum Segment {
    Text(Vec<Span>),
    DisplayMath(String),
}

pub fn gutter_style() -> Style {
    Style::fg(Color::BrightBlack)
}

impl<'c> Writer<'c> {
    pub fn new(opts: &RenderOptions, footnotes: &'c HashMap<String, u32>) -> Self {
        Writer {
            opts: *opts,
            footnotes,
            lines: Vec::new(),
            list_depth: 0,
            nesting: 0,
        }
    }

    pub fn finish(self) -> Vec<String> {
        self.lines
    }

    pub fn inline_ctx(&self) -> InlineCtx<'c> {
        InlineCtx {
            color: self.opts.color,
            footnotes: self.footnotes,
        }
    }

    /// Print one line: the pending prefix followed by `body`.
    pub fn emit(&mut self, prefix: &mut Prefix, body: &[Span]) {
        let mut line = prefix.take();
        line.extend_from_slice(body);
        self.lines.push(paint_line(&line, self.opts.color));
    }

    /// Cells left for text after the prefix.
    pub fn text_width(&self, prefix: &Prefix) -> usize {
        self.opts
            .width
            .saturating_sub(prefix.width())
            .max(MIN_TEXT_WIDTH)
    }

    /// Print spans wrapped to the width left after the prefix.
    pub fn emit_wrapped(&mut self, prefix: &mut Prefix, spans: &[Span]) {
        for line in wrap(spans, self.text_width(prefix)) {
            self.emit(prefix, &line);
        }
    }

    /// Print a sequence of blocks, with a blank line between those that
    /// printed something when `separate` is set.
    pub fn blocks<'a>(
        &mut self,
        nodes: impl Iterator<Item = &'a AstNode<'a>>,
        prefix: &mut Prefix,
        separate: bool,
    ) {
        let mut printed = false;
        for node in nodes {
            let start = self.lines.len();
            // A block that prints nothing must not use up a list marker.
            let saved = prefix.clone();
            self.block(node, prefix);
            if self.lines.len() == start {
                *prefix = saved;
            } else {
                if printed && separate {
                    self.insert_blank(start, prefix);
                }
                printed = true;
            }
        }
    }

    fn insert_blank(&mut self, at: usize, prefix: &Prefix) {
        let blank = paint_line(prefix.rest(), self.opts.color);
        self.lines.insert(at, blank);
    }

    pub fn block<'a>(&mut self, node: &'a AstNode<'a>, prefix: &mut Prefix) {
        let container = matches!(
            node.data.borrow().value,
            NodeValue::BlockQuote
                | NodeValue::MultilineBlockQuote(_)
                | NodeValue::Callout(_)
                | NodeValue::List(_)
                | NodeValue::FootnoteDefinition(_)
        );
        if !container {
            return self.leaf(node, prefix);
        }
        if self.nesting >= MAX_NESTING {
            return self.flat(node, prefix);
        }
        self.nesting += 1;
        self.leaf(node, prefix);
        self.nesting -= 1;
    }

    /// Print the text under `node` as one paragraph, without recursion.
    fn flat<'a>(&mut self, node: &'a AstNode<'a>, prefix: &mut Prefix) {
        let mut spans = Vec::new();
        for d in node.descendants() {
            match &d.data.borrow().value {
                NodeValue::Text(t) => spans.push(Span::plain(t.as_ref())),
                NodeValue::Code(c) => spans.push(Span::plain(c.literal.as_str())),
                NodeValue::Math(m) => spans.push(Span::plain(m.literal.as_str())),
                NodeValue::SoftBreak | NodeValue::LineBreak | NodeValue::Paragraph => {
                    spans.push(Span::plain(" "));
                }
                _ => {}
            }
        }
        self.emit_wrapped(prefix, &spans);
    }

    fn leaf<'a>(&mut self, node: &'a AstNode<'a>, prefix: &mut Prefix) {
        match &node.data.borrow().value {
            NodeValue::FrontMatter(_) | NodeValue::MultilineComment(_) => {}
            NodeValue::Paragraph => self.paragraph(node, prefix),
            NodeValue::Heading(h) => self.heading(node, h.level, prefix),
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let mut inner = prefix.nest_same(vec![Span::new("│ ", gutter_style())]);
                self.blocks(node.children(), &mut inner, true);
            }
            NodeValue::Callout(c) => callout::render(self, node, c, prefix),
            NodeValue::List(list) => self.list(node, list, prefix),
            NodeValue::CodeBlock(code) => self.code_block(code, prefix),
            NodeValue::HtmlBlock(html) => self.html_block(&html.literal, prefix),
            NodeValue::ThematicBreak => {
                let rule = "─".repeat(self.text_width(prefix));
                self.emit(prefix, &[Span::new(rule, gutter_style())]);
            }
            NodeValue::Table(t) => table::render(self, node, t, prefix),
            NodeValue::FootnoteDefinition(f) => {
                let n = self
                    .footnotes
                    .get(&f.name)
                    .map_or(f.name.clone(), u32::to_string);
                let label = format!("[{n}] ");
                let indent = " ".repeat(span::width(&[Span::plain(label.as_str())]));
                let mut inner = prefix.nest(
                    vec![Span::new(label, Style::fg(Color::Cyan))],
                    vec![Span::plain(indent)],
                );
                self.blocks(node.children(), &mut inner, true);
            }
            _ => self.blocks(node.children(), prefix, true),
        }
    }

    fn paragraph<'a>(&mut self, node: &'a AstNode<'a>, prefix: &mut Prefix) {
        let ctx = self.inline_ctx();
        let mut segments = vec![Segment::Text(Vec::new())];
        for child in node.children() {
            if let NodeValue::Math(m) = &child.data.borrow().value
                && m.display_math
            {
                segments.push(Segment::DisplayMath(m.literal.clone()));
                segments.push(Segment::Text(Vec::new()));
                continue;
            }
            if let Some(Segment::Text(spans)) = segments.last_mut() {
                spans.extend(inline::node(child, &Style::default(), &ctx));
            }
        }
        if let Some(Segment::Text(spans)) = segments.last_mut() {
            inline::strip_block_id(spans, &Style::default());
        }
        for segment in segments {
            match segment {
                Segment::Text(spans) => self.emit_wrapped(prefix, &spans),
                Segment::DisplayMath(literal) => self.display_math(&literal, prefix),
            }
        }
    }

    fn display_math(&mut self, literal: &str, prefix: &mut Prefix) {
        if let Some(graphics) = self.opts.graphics
            && !literal.trim().is_empty()
            && let Some(png) = self.fitted_image(literal, &graphics, prefix)
        {
            return self.emit_image(&png, &graphics, prefix);
        }
        for line in literal.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let body = [
                Span::plain(DISPLAY_MATH_INDENT),
                Span::new(line, inline::math_style()),
            ];
            self.emit(prefix, &body);
        }
    }

    /// The formula drawn at the terminal's text size, or smaller when that
    /// is wider than the text column. `None` when RaTeX cannot draw it, or
    /// it would be too small to read or taller than the screen.
    fn fitted_image(&self, literal: &str, g: &Graphics, prefix: &Prefix) -> Option<Png> {
        let indent = DISPLAY_MATH_INDENT.len() as u32;
        let columns = u32::try_from(self.text_width(prefix))
            .ok()?
            .saturating_sub(indent);
        let em = g.cell_height as f32;
        let mut png = math_image::render(literal, g.foreground, em).ok()?;
        let needed = png.width.div_ceil(g.cell_width);
        if needed > columns {
            let scale = columns as f32 / needed as f32;
            if scale < MIN_MATH_SCALE {
                return None;
            }
            png = math_image::render(literal, g.foreground, (em * scale).floor()).ok()?;
        }
        let rows = png.height.div_ceil(g.cell_height);
        (rows < u32::from(g.screen_rows) && png.width.div_ceil(g.cell_width) <= columns)
            .then_some(png)
    }

    /// Reserve the rows the image covers, each with the block's prefix,
    /// then go back up and draw the image over them after the indent.
    fn emit_image(&mut self, png: &Png, g: &Graphics, prefix: &mut Prefix) {
        let rows = png.height.div_ceil(g.cell_height).max(1);
        let column = prefix.width() + DISPLAY_MATH_INDENT.len();
        for _ in 0..rows {
            self.emit(prefix, &[]);
        }
        let up = rows - 1;
        let mut place = String::new();
        if up > 0 {
            place.push_str(&format!("\x1b[{up}A"));
        }
        place.push_str(&format!("\r\x1b[{column}C"));
        place.push_str(&kitty::place(&png.data));
        if up > 0 {
            place.push_str(&format!("\x1b[{up}B"));
        }
        if let Some(last) = self.lines.last_mut() {
            last.push_str(&place);
        }
    }

    fn heading<'a>(&mut self, node: &'a AstNode<'a>, level: u8, prefix: &mut Prefix) {
        let (style, rule) = match level {
            1 => (Style::fg(Color::Magenta).bold(), Some("═")),
            2 => (Style::fg(Color::Blue).bold(), Some("─")),
            3 => (Style::fg(Color::Cyan).bold(), None),
            _ => (Style::default().bold(), None),
        };
        let mut spans = inline::children(node, &style, &self.inline_ctx());
        inline::strip_block_id(&mut spans, &style);
        let lines = wrap(&spans, self.text_width(prefix));
        let widest = lines.iter().map(|l| span::width(l)).max().unwrap_or(0);
        for line in &lines {
            self.emit(prefix, line);
        }
        if let (Some(rule), true) = (rule, widest > 0) {
            let underline = Span::new(rule.repeat(widest), style.dim());
            self.emit(prefix, &[underline]);
        }
    }

    fn list<'a>(&mut self, node: &'a AstNode<'a>, list: &NodeList, prefix: &mut Prefix) {
        self.list_depth += 1;
        let items: Vec<_> = node.children().collect();
        let markers: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, item)| self.marker(list, i, item))
            .collect();
        let marker_width = markers
            .iter()
            .map(|m| span::width(&[Span::plain(m.as_str())]))
            .max()
            .unwrap_or(0);
        let mut printed = false;
        for (item, marker) in items.into_iter().zip(markers) {
            let start = self.lines.len();
            let mut inner = prefix.nest(
                vec![Span::new(
                    format!("{marker:>marker_width$} "),
                    Style::fg(Color::Cyan),
                )],
                vec![Span::plain(" ".repeat(marker_width + 1))],
            );
            self.blocks(item.children(), &mut inner, !list.tight);
            if self.lines.len() == start {
                self.emit(&mut inner, &[]);
            }
            if self.lines.len() > start {
                if printed && !list.tight {
                    self.insert_blank(start, prefix);
                }
                printed = true;
            }
        }
        self.list_depth -= 1;
    }

    fn marker<'a>(&self, list: &NodeList, index: usize, item: &'a AstNode<'a>) -> String {
        let task = match &item.data.borrow().value {
            NodeValue::TaskItem(t) => Some(t.symbol.is_some_and(|c| c != ' ')),
            _ => None,
        };
        let checkbox = task.map(|done| if done { "☑" } else { "☐" });
        match (list.list_type, checkbox) {
            (comrak::nodes::ListType::Ordered, check) => {
                let delim = match list.delimiter {
                    comrak::nodes::ListDelimType::Paren => ')',
                    comrak::nodes::ListDelimType::Period => '.',
                };
                let number = format!("{}{delim}", list.start + index);
                check.map_or(number.clone(), |c| format!("{number} {c}"))
            }
            (comrak::nodes::ListType::Bullet, Some(c)) => c.to_owned(),
            (comrak::nodes::ListType::Bullet, None) => {
                BULLETS[(self.list_depth - 1) % BULLETS.len()].to_owned()
            }
        }
    }

    fn code_block(&mut self, code: &NodeCodeBlock, prefix: &mut Prefix) {
        let label = code.info.split_whitespace().next().unwrap_or("");
        let top = if label.is_empty() {
            "╭─".to_owned()
        } else {
            format!("╭─ {label}")
        };
        self.emit(prefix, &[Span::new(top, gutter_style())]);
        for line in code.literal.lines() {
            let body = [
                Span::new("│ ", gutter_style()),
                Span::plain(line.replace('\t', "    ")),
            ];
            self.emit(prefix, &body);
        }
        self.emit(prefix, &[Span::new("╰─", gutter_style())]);
    }

    fn html_block(&mut self, literal: &str, prefix: &mut Prefix) {
        let trimmed = literal.trim();
        if trimmed.starts_with("<!--") && trimmed.ends_with("-->") {
            return;
        }
        for line in literal.lines() {
            self.emit(prefix, &[Span::new(line, Style::default().dim())]);
        }
    }
}
