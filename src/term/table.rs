//! GFM tables drawn with box characters.

use comrak::nodes::{AstNode, NodeTable, NodeValue, TableAlignment};

use super::block::{Writer, gutter_style};
use super::inline;
use super::prefix::Prefix;
use super::span::{self, Span};
use super::style::Style;

struct Row {
    header: bool,
    cells: Vec<Vec<Span>>,
}

pub fn render<'a>(w: &mut Writer, node: &'a AstNode<'a>, table: &NodeTable, prefix: &mut Prefix) {
    let rows = collect_rows(w, node);
    let columns = table.num_columns;
    let widths: Vec<usize> = (0..columns)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.cells.get(c))
                .map(|cell| span::width(cell))
                .max()
                .unwrap_or(0)
        })
        .collect();
    w.emit(prefix, &[border(&widths, '┌', '┬', '┐')]);
    for row in &rows {
        w.emit(prefix, &row_line(row, &widths, &table.alignments));
        if row.header {
            w.emit(prefix, &[border(&widths, '├', '┼', '┤')]);
        }
    }
    w.emit(prefix, &[border(&widths, '└', '┴', '┘')]);
}

fn collect_rows<'a>(w: &Writer, node: &'a AstNode<'a>) -> Vec<Row> {
    let ctx = w.inline_ctx();
    node.children()
        .filter_map(|row| {
            let header = match row.data.borrow().value {
                NodeValue::TableRow(header) => header,
                _ => return None,
            };
            let base = if header {
                Style::default().bold()
            } else {
                Style::default()
            };
            let cells = row
                .children()
                .map(|cell| one_line(inline::children(cell, &base, &ctx)))
                .collect();
            Some(Row { header, cells })
        })
        .collect()
}

/// Cells stay on one line: hard breaks become spaces.
fn one_line(spans: Vec<Span>) -> Vec<Span> {
    spans
        .into_iter()
        .map(|s| Span::new(s.text.replace(['\n', '\t'], " "), s.style))
        .collect()
}

fn border(widths: &[usize], left: char, mid: char, right: char) -> Span {
    let inner: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
    Span::new(
        format!("{left}{}{right}", inner.join(&mid.to_string())),
        gutter_style(),
    )
}

fn row_line(row: &Row, widths: &[usize], alignments: &[TableAlignment]) -> Vec<Span> {
    let bar = || Span::new("│", gutter_style());
    let mut line = vec![bar()];
    for (c, width) in widths.iter().enumerate() {
        let cell = row.cells.get(c).cloned().unwrap_or_default();
        let gap = width - span::width(&cell);
        let (left, right) = match alignments.get(c) {
            Some(TableAlignment::Right) => (gap, 0),
            Some(TableAlignment::Center) => (gap / 2, gap - gap / 2),
            _ => (0, gap),
        };
        line.push(Span::plain(" ".repeat(left + 1)));
        line.extend(cell);
        line.push(Span::plain(" ".repeat(right + 1)));
        line.push(bar());
    }
    line
}
