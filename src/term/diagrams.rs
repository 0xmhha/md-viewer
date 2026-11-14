//! Draw every Mermaid diagram of a document at once, on worker threads,
//! before the page is laid out. Drawn images are cached on disk, so a
//! second run only reads files.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use comrak::nodes::{AstNode, NodeValue};

use super::probe::Graphics;
use crate::cache::{self, Cache};
use crate::math_image::{Png, png_size};
use crate::mermaid::{self, Failure, Target};

/// What drawing a diagram ahead of layout gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drawn {
    Image(Png),
    /// Box-drawing lines that fit `width` columns.
    Text {
        width: usize,
        lines: Vec<String>,
    },
    /// It ran out of time. Drawing it another way would parse it again and
    /// take as long, so its source is shown instead.
    TimedOut,
}

/// Drawings by diagram source. A diagram missing here could be drawn
/// neither as an image nor as text.
pub type Diagrams = HashMap<String, Drawn>;

/// Smallest scale a diagram is shrunk to, to fit the text column and the
/// screen, before it is drawn in text instead.
pub const MIN_SCALE: f32 = 0.5;

/// Where a diagram image goes: the cells it covers, and whether the
/// terminal has to scale it into them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub columns: u32,
    pub rows: u32,
    pub scaled: bool,
}

/// Fit `png` into `columns` and the screen height, shrinking it if needed.
/// `None` when it would have to shrink below `MIN_SCALE`.
pub fn placement(png: &Png, g: &Graphics, columns: u32) -> Option<Placement> {
    fit(png, g, columns, MIN_SCALE)
}

/// Fit `png` into `columns` and the screen height, shrinking it as far as
/// `min_scale`. `None` when it would have to shrink further.
pub fn fit(png: &Png, g: &Graphics, columns: u32, min_scale: f32) -> Option<Placement> {
    let max_rows = u32::from(g.screen_rows).saturating_sub(1).max(1);
    let natural_columns = png.width as f32 / g.cell_width as f32;
    let natural_rows = png.height as f32 / g.cell_height as f32;
    let scale = (columns as f32 / natural_columns)
        .min(max_rows as f32 / natural_rows)
        .min(1.0);
    if scale < min_scale || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let fit = |cells: f32, limit: u32| ((cells * scale).ceil() as u32).clamp(1, limit);
    Some(Placement {
        columns: fit(natural_columns, columns.max(1)),
        rows: fit(natural_rows, max_rows),
        scaled: scale < 1.0,
    })
}

/// Whether a fenced block holds Mermaid source.
pub fn is_mermaid(info: &str) -> bool {
    info.split_whitespace().next() == Some("mermaid")
}

/// Draw the document's Mermaid diagrams on worker threads: as images
/// when the terminal has `graphics`, else (or when that fails) as text
/// `columns` wide.
pub fn draw<'a>(
    root: &'a AstNode<'a>,
    graphics: Option<&Graphics>,
    columns: usize,
    cache: Option<&Cache>,
) -> Diagrams {
    let images = graphics.map(|g| (g, target(g, columns)));
    let mut sources: Vec<String> = root
        .descendants()
        .filter_map(|node| match &node.data.borrow().value {
            NodeValue::CodeBlock(code)
                if is_mermaid(&code.info) && !code.literal.trim().is_empty() =>
            {
                Some(code.literal.clone())
            }
            _ => None,
        })
        .collect();
    sources.sort_unstable();
    sources.dedup();
    if sources.is_empty() {
        return Diagrams::new();
    }

    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(sources.len());
    let next = AtomicUsize::new(0);
    let drawn = Mutex::new(Diagrams::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(source) = sources.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Some(drawing) = drawing(source, images.as_ref(), columns, cache) {
                        drawn
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(source.clone(), drawing);
                    }
                }
            });
        }
    });
    drawn.into_inner().unwrap_or_else(|e| e.into_inner())
}

/// One diagram as an image, else as text. Text is tried only when the
/// image fails for a reason other than time: that would be slow again.
fn drawing(
    source: &str,
    images: Option<&(&Graphics, Target)>,
    columns: usize,
    cache: Option<&Cache>,
) -> Option<Drawn> {
    if let Some((g, target)) = images {
        match cached_image(source, g, target, cache) {
            Ok(png) => return Some(Drawn::Image(png)),
            Err(Failure::TimedOut) => return Some(Drawn::TimedOut),
            Err(Failure::Other(_)) => {}
        }
    }
    match mermaid::text(source, columns) {
        Ok(lines) => Some(Drawn::Text {
            width: columns,
            lines,
        }),
        Err(Failure::TimedOut) => Some(Drawn::TimedOut),
        Err(Failure::Other(_)) => None,
    }
}

/// The image from the cache, or drawn and then stored there. A timeout
/// is not stored: the next run, on a less busy machine, may finish.
fn cached_image(
    source: &str,
    g: &Graphics,
    target: &Target,
    cache: Option<&Cache>,
) -> Result<Png, Failure> {
    let key = image_key(source, g);
    if let Some(data) = cache.and_then(|c| c.get(key))
        && data.ends_with(PNG_END)
        && let Some((width, height)) = png_size(&data)
    {
        return Ok(Png {
            data,
            width,
            height,
        });
    }
    let png = mermaid::image(source, target)?;
    if let Some(cache) = cache {
        cache.put(key, &png.data);
    }
    Ok(png)
}

/// Labels as tall as the terminal's text, and no larger than what
/// `placement` could still show at `MIN_SCALE`.
fn target(g: &Graphics, columns: usize) -> Target {
    let shown = |cells: u32, cell: u32| cells.max(1) as f32 * cell as f32 / MIN_SCALE;
    let columns = u32::try_from(columns).unwrap_or(u32::MAX);
    let rows = u32::from(g.screen_rows).saturating_sub(1);
    Target {
        foreground: g.foreground,
        background: g.background,
        scale: g.cell_height as f32 / mermaid::LINE_PX,
        max_width: shown(columns, g.cell_width) as u32,
        max_height: shown(rows, g.cell_height) as u32,
    }
}

/// The IEND chunk every PNG ends with. A cached file without it was cut
/// short and is drawn again.
const PNG_END: &[u8] = b"IEND\xae\x42\x60\x82";

/// The drawing libraries; a new version may draw differently. merman is
/// pinned exactly in Cargo.toml, so this changes only with it.
const RENDERER: &str = "mermaid merman=0.10.0-alpha.2 resvg=0.49";

/// Everything that changes the drawn pixels: the source, the colors and
/// the text size.
fn image_key(source: &str, g: &Graphics) -> u64 {
    let rgb = |c: crate::math_image::Rgb| [c.0, c.1, c.2];
    let background = g.background.map_or([0, 0, 0, 0], |c| [1, c.0, c.1, c.2]);
    cache::key(&[
        RENDERER.as_bytes(),
        source.as_bytes(),
        &rgb(g.foreground),
        &background,
        &g.cell_height.to_le_bytes(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math_image::Rgb;

    const G: Graphics = Graphics {
        cell_width: 8,
        cell_height: 19,
        foreground: Rgb(0xcd, 0xd6, 0xf4),
        background: Some(Rgb(0x1e, 0x1e, 0x2e)),
        screen_rows: 50,
    };

    fn parse_and_draw(md: &str, cache: Option<&Cache>) -> Diagrams {
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, md, &crate::parse::options());
        draw(root, Some(&G), 80, cache)
    }

    #[test]
    fn draws_each_distinct_diagram_once() {
        let md = "```mermaid\nflowchart LR\n A-->B\n```\n\n```mermaid\nflowchart LR\n A-->B\n```\n\n```mermaid\nflowchart LR\n C-->D\n```\n\n```mermaid\nnot a diagram\n```\n";
        let drawn = parse_and_draw(md, None);
        assert_eq!(drawn.len(), 2, "{:?}", drawn.keys().collect::<Vec<_>>());
    }

    #[test]
    fn without_graphics_diagrams_are_drawn_as_text() {
        let arena = comrak::Arena::new();
        let md = "```mermaid\nflowchart LR\n A-->B\n```\n\n```mermaid\nnot a diagram\n```\n";
        let root = comrak::parse_document(&arena, md, &crate::parse::options());
        let drawn = draw(root, None, 80, None);
        assert_eq!(drawn.len(), 1);
        assert!(
            matches!(&drawn["flowchart LR\n A-->B\n"], Drawn::Text { width: 80, lines } if !lines.is_empty()),
            "{drawn:?}"
        );
    }

    #[test]
    fn a_second_run_reads_the_cache() {
        let dir = std::env::temp_dir().join(format!("mdv-diagrams-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = Cache::new(&dir);
        let md = "```mermaid\nflowchart LR\n A-->B\n```\n";
        let first = parse_and_draw(md, Some(&cache));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        // A planted file under the same key proves the second run reads it.
        let source = first.keys().next().unwrap().clone();
        let planted = crate::math_image::render("x", G.foreground, 20.0).unwrap();
        cache.put(image_key(&source, &G), &planted.data);
        let second = parse_and_draw(md, Some(&cache));
        assert_eq!(second[&source], Drawn::Image(planted));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_truncated_cache_file_is_drawn_again() {
        let dir = std::env::temp_dir().join(format!("mdv-diagrams-cut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = Cache::new(&dir);
        let source = "flowchart LR\n A-->B\n";
        let good = cached_image(source, &G, &target(&G, 80), Some(&cache)).unwrap();
        let key = image_key(source, &G);
        cache.put(key, &good.data[..good.data.len() / 2]);
        let again = cached_image(source, &G, &target(&G, 80), Some(&cache));
        assert_eq!(again, Ok(good));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_key_changes_with_colors_and_text_size() {
        let base = image_key("graph", &G);
        assert_ne!(
            base,
            image_key(
                "graph",
                &Graphics {
                    cell_height: 20,
                    ..G
                }
            )
        );
        assert_ne!(
            base,
            image_key(
                "graph",
                &Graphics {
                    background: None,
                    ..G
                }
            )
        );
        assert_ne!(
            base,
            image_key(
                "graph",
                &Graphics {
                    foreground: Rgb(0, 0, 0),
                    ..G
                }
            )
        );
        assert_eq!(
            base,
            image_key(
                "graph",
                &Graphics {
                    screen_rows: 10,
                    ..G
                }
            )
        );
    }

    fn png(width: u32, height: u32) -> Png {
        Png {
            data: Vec::new(),
            width,
            height,
        }
    }

    #[test]
    fn a_diagram_that_fits_keeps_its_size() {
        let at = placement(&png(80, 38), &G, 40).unwrap();
        assert_eq!(
            at,
            Placement {
                columns: 10,
                rows: 2,
                scaled: false
            }
        );
    }

    #[test]
    fn a_wide_diagram_is_scaled_to_the_columns() {
        let at = placement(&png(480, 190), &G, 40).unwrap();
        assert!(at.scaled && at.columns == 40, "{at:?}");
        assert_eq!(at.rows, 7);
    }

    #[test]
    fn a_diagram_shrunk_only_by_the_screen_height_is_scaled() {
        // Review regression: 213px at 19px a row is 11.2 rows; shrunk by
        // 0.98 it still rounds to 11 columns, but it must be scaled.
        let short = Graphics {
            screen_rows: 12,
            ..G
        };
        let at = placement(&png(87, 213), &short, 80).unwrap();
        assert!(at.scaled, "{at:?}");
        assert!(at.rows <= 11, "{at:?}");
    }

    #[test]
    fn a_diagram_needing_more_than_half_size_reduction_is_refused() {
        assert_eq!(placement(&png(1000, 19), &G, 40), None);
        assert_eq!(placement(&png(8, 19 * 200), &G, 40), None);
    }

    #[test]
    fn fence_info_names_mermaid() {
        assert!(is_mermaid("mermaid"));
        assert!(is_mermaid("mermaid title=x"));
        assert!(!is_mermaid("mermaidx"));
        assert!(!is_mermaid(""));
    }
}
