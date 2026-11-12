//! Mermaid diagrams drawn with merman: as a PNG image for terminals that
//! show images, or as box-drawing text for the rest.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use merman::ascii::{AsciiLayoutProfile, AsciiRenderOptions, AsciiViewportPolicy, OverflowPolicy};
use merman::svg::SvgPipeline;
use merman::{
    AsciiRequest, MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgRequest,
};
use resvg::usvg::fontdb::Database;

use crate::math_image::{Png, Rgb, png_size};

/// Pixels per text line in merman's SVG, whose labels are 16px type.
/// Scaling by `cell height / LINE_PX` makes labels as tall as the
/// terminal's own text.
pub const LINE_PX: f32 = 19.0;

/// Time one diagram may take to lay out. merman is young; a diagram that
/// runs past this is shown in another form instead of hanging the viewer.
const DEADLINE: Duration = Duration::from_secs(2);

/// How an image is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    /// Lines and text.
    pub foreground: Rgb,
    /// The terminal's background, put under edge labels. `None` guesses it
    /// from the foreground.
    pub background: Option<Rgb>,
    /// Times merman's natural size.
    pub scale: f32,
    /// Largest image in pixels. A bigger one is refused before its pixels
    /// are allocated: it could not be shown anyway.
    pub max_width: u32,
    pub max_height: u32,
}

/// Why a diagram was not drawn as an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// merman ran past `DEADLINE`. Drawing the same source as text would
    /// parse it again and take as long.
    TimedOut,
    /// Invalid source, a diagram too large to show, and the like.
    Other(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::TimedOut => write!(f, "the diagram took longer than {DEADLINE:?}"),
            Failure::Other(reason) => f.write_str(reason),
        }
    }
}

impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Failure::Other(reason)
    }
}

/// Draw `source` on a transparent background.
pub fn image(source: &str, target: &Target) -> Result<Png, Failure> {
    image_with(source, target, control())
}

fn image_with(source: &str, target: &Target, control: OperationControl) -> Result<Png, Failure> {
    let foreground = target.foreground;
    let background = target
        .background
        .unwrap_or_else(|| guess_background(foreground));
    let renderer = Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(
        SourcePresentationConfig::from_value(theme(foreground, background)),
    ));
    let request = SvgRequest {
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..Default::default()
    };
    let output = renderer
        .render(RenderRequest::svg(source, control.clone(), request))
        .map_err(|e| {
            // The clone shares the deadline; a failed checkpoint means the
            // time ran out, whatever error merman reported for it.
            if control.checkpoint().is_err() {
                Failure::TimedOut
            } else {
                Failure::Other(e.to_string())
            }
        })?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err(Failure::Other("not a Mermaid diagram".into()));
    };
    Ok(rasterize(svg.svg(), target)?)
}

/// The diagram as box-drawing lines no wider than `width` cells. Fails
/// when merman cannot draw it in text or it does not fit.
pub fn text(source: &str, width: usize) -> Result<Vec<String>, Failure> {
    // The compact layout fits more diagrams in a narrow terminal, but only
    // flowcharts and sequence diagrams accept it.
    // Both attempts share one deadline: each gets a child control that
    // stops when the parent's time is up.
    let budget = control();
    [AsciiLayoutProfile::Auto, AsciiLayoutProfile::Canonical]
        .into_iter()
        .find_map(|layout| text_with(source, width, layout, budget.child()))
        .ok_or_else(|| {
            if budget.checkpoint().is_err() {
                Failure::TimedOut
            } else {
                Failure::Other(format!("cannot be drawn in {width} columns"))
            }
        })
}

fn text_with(
    source: &str,
    width: usize,
    layout: AsciiLayoutProfile,
    control: OperationControl,
) -> Option<Vec<String>> {
    let request = AsciiRequest {
        options: AsciiRenderOptions::unicode().with_layout_profile(layout),
        viewport: AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Error),
        ..Default::default()
    };
    let output = Renderer::new()
        .render(RenderRequest::ascii(source, control, request))
        .ok()?;
    let RenderOutput::Ascii(Some(report)) = output else {
        return None;
    };
    let lines: Vec<String> = report
        .text
        .lines()
        .map(|l| l.trim_end().to_owned())
        .collect();
    (!lines.iter().all(String::is_empty)).then_some(lines)
}

fn control() -> OperationControl {
    OperationControl::new().with_deadline(DEADLINE)
}

/// Mermaid's `base` theme with every color taken from the terminal, so the
/// diagram reads like the text around it.
fn theme(fg: Rgb, bg: Rgb) -> serde_json::Value {
    let (fg_hex, bg_hex) = (hex(fg), hex(bg));
    let fill = hex(mix(bg, fg, 0.15));
    serde_json::json!({
        "theme": "base",
        "fontFamily": "sans-serif",
        "themeVariables": {
            "darkMode": luminance(bg) < 0.5,
            "background": bg_hex,
            "primaryColor": fill,
            "primaryTextColor": fg_hex,
            "primaryBorderColor": fg_hex,
            "lineColor": fg_hex,
            "textColor": fg_hex,
            "edgeLabelBackground": bg_hex,
            "fontFamily": "sans-serif",
            "packet": {
                "blockFillColor": fill,
                "blockStrokeColor": fg_hex,
                "labelColor": fg_hex,
                "titleColor": fg_hex,
                "startByteColor": fg_hex,
                "endByteColor": fg_hex,
            },
            "xyChart": {
                "backgroundColor": "transparent",
                "titleColor": fg_hex,
                "xAxisLabelColor": fg_hex,
                "yAxisLabelColor": fg_hex,
                "xAxisTitleColor": fg_hex,
                "yAxisTitleColor": fg_hex,
                "xAxisLineColor": fg_hex,
                "yAxisLineColor": fg_hex,
                "xAxisTickColor": fg_hex,
                "yAxisTickColor": fg_hex,
            },
        },
    })
}

fn rasterize(svg: &str, target: &Target) -> Result<Png, String> {
    let scale = target.scale;
    // Parsing lays out every label, which takes seconds for a huge diagram;
    // its size is known from the root element before that.
    if let Some((width, height)) = view_box_size(svg) {
        check_size(width * scale, height * scale, target)?;
    }
    let options = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (width, height) = (
        (size.width() * scale).ceil(),
        (size.height() * scale).ceil(),
    );
    check_size(width, height, target)?;
    let (width, height) = (width as u32, height as u32);
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("the diagram has no area")?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let data = pixmap.encode_png().map_err(|e| e.to_string())?;
    let (width, height) = png_size(&data).ok_or("resvg returned an invalid PNG")?;
    Ok(Png {
        data,
        width,
        height,
    })
}

fn check_size(width: f32, height: f32, target: &Target) -> Result<(), String> {
    if width > target.max_width as f32 || height > target.max_height as f32 {
        return Err(format!(
            "the diagram is too large ({}x{} pixels)",
            width.ceil(),
            height.ceil()
        ));
    }
    Ok(())
}

/// Width and height from the `viewBox` of the root `<svg>` element.
fn view_box_size(svg: &str) -> Option<(f32, f32)> {
    let start = svg.find("<svg")?;
    let tag = &svg[start..start + svg[start..].find('>')?];
    let (_, rest) = tag.split_once("viewBox=\"")?;
    let (value, _) = rest.split_once('"')?;
    let numbers: Vec<f32> = value
        .split([' ', ','])
        .filter(|n| !n.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match numbers[..] {
        [_, _, width, height] => Some((width, height)),
        _ => None,
    }
}

/// Fonts for diagram labels: a Latin and a CJK face from known system
/// paths, loaded once. Scanning every system font made drawing twice as
/// slow in the prototype, so that happens only when no CJK face is found
/// where expected.
fn fonts() -> Arc<Database> {
    static FONTS: OnceLock<Arc<Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = Database::new();
            let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from);
            let load = |db: &mut Database, files: &[&str]| {
                files.iter().any(|file| {
                    let path = match (&windows, file.strip_prefix("%WINDIR%\\")) {
                        (Some(dir), Some(rest)) => dir.join("Fonts").join(rest),
                        (None, Some(_)) => return false,
                        (_, None) => std::path::PathBuf::from(file),
                    };
                    std::fs::read(path)
                        .map(|data| db.load_font_data(data))
                        .is_ok()
                })
            };
            load(&mut db, LATIN_FONTS);
            if !load(&mut db, CJK_FONTS) {
                db.load_system_fonts();
            }
            // The Latin face, loaded first, becomes `sans-serif`.
            let family = db
                .faces()
                .find_map(|face| face.families.first().map(|(name, _)| name.clone()));
            if let Some(family) = family {
                db.set_sans_serif_family(family);
            }
            Arc::new(db)
        })
        .clone()
}

const LATIN_FONTS: &[&str] = &[
    "/System/Library/Fonts/Helvetica.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "%WINDIR%\\segoeui.ttf",
];

const CJK_FONTS: &[&str] = &[
    "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.otf.ttc",
    "%WINDIR%\\malgun.ttf",
];

fn guess_background(fg: Rgb) -> Rgb {
    if luminance(fg) > 0.5 {
        Rgb(0, 0, 0)
    } else {
        Rgb(255, 255, 255)
    }
}

fn luminance(c: Rgb) -> f32 {
    (0.2126 * f32::from(c.0) + 0.7152 * f32::from(c.1) + 0.0722 * f32::from(c.2)) / 255.0
}

/// `from` moved `amount` of the way towards `to`.
fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Rgb(
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FG: Rgb = Rgb(0xcd, 0xd6, 0xf4);
    const BG: Rgb = Rgb(0x1e, 0x1e, 0x2e);
    const FLOW: &str = "flowchart LR\n  A[Start] --> B[End]\n";

    fn at(scale: f32) -> Target {
        Target {
            foreground: FG,
            background: Some(BG),
            scale,
            max_width: 4000,
            max_height: 4000,
        }
    }

    fn rgba(png: &Png) -> Vec<u8> {
        let decoder = png::Decoder::new(png.data.as_slice());
        let mut reader = decoder.read_info().expect("valid PNG");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("one frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        buf.truncate(info.buffer_size());
        buf
    }

    #[test]
    fn draws_a_flowchart_of_the_reported_size() {
        let png = image(FLOW, &at(1.0)).expect("renders");
        let decoder = png::Decoder::new(png.data.as_slice());
        let info = decoder.read_info().expect("valid PNG").info().clone();
        assert_eq!((info.width, info.height), (png.width, png.height));
        assert!(
            png.width > png.height,
            "LR flows sideways: {}x{}",
            png.width,
            png.height
        );
    }

    #[test]
    fn background_is_transparent_and_lines_use_the_text_color() {
        let png = image(FLOW, &at(1.0)).expect("renders");
        let pixels = rgba(&png);
        assert_eq!(pixels[3], 0, "corner pixel must be transparent");
        let near_fg = pixels.chunks(4).any(|p| {
            p[3] == 255
                && p[0].abs_diff(FG.0) < 8
                && p[1].abs_diff(FG.1) < 8
                && p[2].abs_diff(FG.2) < 8
        });
        assert!(near_fg, "some solid pixel must be the text color");
    }

    #[test]
    fn scale_sets_the_pixel_size() {
        let one = image(FLOW, &at(1.0)).expect("renders");
        let two = image(FLOW, &at(2.0)).expect("renders");
        assert!(
            two.width.abs_diff(one.width * 2) <= 2,
            "{} vs {}",
            two.width,
            one.width
        );
    }

    #[test]
    fn a_huge_diagram_is_refused_before_its_svg_is_parsed() {
        // Fuzzing regression: a 130,000px wide flowchart took 11s in the SVG
        // parser before its size was checked.
        let label = "word ".repeat(4000);
        let source = format!("flowchart LR\n  A[\"{label}\"] --> B\n");
        let start = std::time::Instant::now();
        let err = image(&source, &at(1.0)).expect_err("too large");
        assert!(err.to_string().contains("too large"), "{err}");
        assert!(start.elapsed().as_secs_f32() < 1.0, "{:?}", start.elapsed());
    }

    #[test]
    fn an_attempt_under_a_spent_budget_draws_nothing() {
        let spent = OperationControl::new().with_deadline(Duration::ZERO);
        let layout = AsciiLayoutProfile::Canonical;
        assert_eq!(text_with(FLOW, 80, layout, spent.child()), None);
        assert!(text_with(FLOW, 80, layout, control().child()).is_some());
    }

    #[test]
    fn an_image_under_a_spent_budget_times_out() {
        let spent = OperationControl::new().with_deadline(Duration::ZERO);
        assert_eq!(image_with(FLOW, &at(1.0), spent), Err(Failure::TimedOut));
        let err = image("flowchart LR\n  A -->", &at(1.0)).unwrap_err();
        assert!(matches!(err, Failure::Other(_)), "{err}");
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
    fn text_attempts_share_one_deadline() {
        // Fuzzing regression: merman's sequence parser is quadratic on a long
        // invalid line, and each of the two layouts used to get its own
        // deadline.
        let source = format!("sequenceDiagram\nparticipant A\n{}\n", "@".repeat(40_000));
        let start = std::time::Instant::now();
        assert_eq!(text(&source, 80), Err(Failure::TimedOut));
        let spent = start.elapsed();
        assert!(spent < DEADLINE + Duration::from_millis(500), "{spent:?}");
    }

    #[test]
    fn view_box_gives_the_size() {
        let svg = r#"<svg id="m" width="100%" style="max-width: 9px;" viewBox="-8 0 120.5 40">"#;
        assert_eq!(view_box_size(svg), Some((120.5, 40.0)));
        assert_eq!(view_box_size("<svg width=\"10\">"), None);
        assert_eq!(view_box_size(r#"<svg viewBox="0 0 x 4">"#), None);
    }

    #[test]
    fn a_huge_image_is_refused_before_allocating() {
        let err = image(FLOW, &at(1000.0)).expect_err("too large");
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[test]
    fn invalid_source_is_an_error() {
        assert!(image("flowchart LR\n  A -->", &at(1.0)).is_err());
        assert!(image("not a diagram", &at(1.0)).is_err());
    }

    #[test]
    fn text_draws_boxes_within_the_width() {
        let lines = text(FLOW, 80).expect("draws");
        let all = lines.join("\n");
        assert!(all.contains("Start") && all.contains("End"), "{all}");
        assert!(all.contains('┌'), "{all}");
        assert!(
            lines
                .iter()
                .all(|l| unicode_width::UnicodeWidthStr::width(l.as_str()) <= 80)
        );
    }

    #[test]
    fn text_gives_up_when_the_diagram_is_too_wide() {
        assert!(matches!(text(FLOW, 8), Err(Failure::Other(_))));
        assert!(matches!(text("not a diagram", 80), Err(Failure::Other(_))));
    }

    #[test]
    fn theme_follows_the_terminal_colors() {
        let t = theme(FG, BG);
        assert_eq!(t["themeVariables"]["lineColor"], "#cdd6f4");
        assert_eq!(t["themeVariables"]["darkMode"], true);
        assert_eq!(
            theme(Rgb(0, 0, 0), Rgb(255, 255, 255))["themeVariables"]["darkMode"],
            false
        );
        assert_eq!(guess_background(FG), Rgb(0, 0, 0));
    }

    /// Every Mermaid block in a corpus must draw as an image:
    /// `MDV_CORPUS=<file listing paths> cargo test --release --lib mermaid -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs MDV_CORPUS"]
    fn corpus_mermaid_renders() {
        use crate::parse::{SpecialBlock, special_blocks};

        let list = std::env::var("MDV_CORPUS").expect("MDV_CORPUS is not set");
        let paths = std::fs::read_to_string(list).expect("cannot read MDV_CORPUS");
        let start = std::time::Instant::now();
        let (mut count, mut drawn_as_text, mut failures) = (0, 0, Vec::new());
        for path in paths.lines().filter(|p| !p.trim().is_empty()) {
            let md = std::fs::read_to_string(path).expect(path);
            for block in special_blocks(&md) {
                if let SpecialBlock::Mermaid { source } = block {
                    count += 1;
                    let unbounded = Target {
                        max_width: u32::MAX,
                        max_height: u32::MAX,
                        ..at(1.0)
                    };
                    if let Err(e) = image(&source, &unbounded) {
                        failures.push(format!("{path}: {e}"));
                    }
                    drawn_as_text += usize::from(text(&source, 120).is_ok());
                }
            }
        }
        eprintln!(
            "mermaid: {count} diagrams in {:?}, {drawn_as_text} fit 120 columns as text",
            start.elapsed()
        );
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
