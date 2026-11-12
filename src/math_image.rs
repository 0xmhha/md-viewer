//! Display math drawn as a PNG image with RaTeX.

/// An 8-bit RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A PNG image and its size in pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Png {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Draw `latex` in `color` on a transparent background, with one em
/// `em_px` pixels tall. Fails on LaTeX that RaTeX cannot parse or lay out.
pub fn render(latex: &str, color: Rgb, em_px: f32) -> Result<Png, String> {
    // RaTeX prints which system font it found to stderr; on a terminal
    // that would land in the middle of the page.
    quiet::without_stderr(|| draw(latex, color, em_px))
}

fn draw(latex: &str, color: Rgb, em_px: f32) -> Result<Png, String> {
    let nodes = ratex_parser::parse(latex).map_err(|e| e.to_string())?;
    let layout_opts = ratex_layout::LayoutOptions {
        color: to_ratex(color, 1.0),
        ..Default::default()
    };
    let root = ratex_layout::layout(&nodes, &layout_opts);
    ratex_layout::validate_layout(&root)?;
    let list = ratex_layout::to_display_list(&root);
    let render_opts = ratex_render::RenderOptions {
        font_size: em_px,
        padding: (em_px / 8.0).round(),
        background_color: to_ratex(color, 0.0),
        ..Default::default()
    };
    let data = ratex_render::render_to_png(&list, &render_opts)?;
    let (width, height) = png_size(&data).ok_or("RaTeX returned an invalid PNG")?;
    Ok(Png {
        data,
        width,
        height,
    })
}

#[cfg(unix)]
mod quiet {
    use rustix::fs::{Mode, OFlags, open};
    use rustix::stdio::{dup2_stderr, stderr};

    /// Run `f` with stderr sent to `/dev/null`, then put it back. If the
    /// redirection cannot be set up, `f` runs with stderr unchanged.
    pub fn without_stderr<T>(f: impl FnOnce() -> T) -> T {
        let Ok(saved) = rustix::io::dup(stderr()) else {
            return f();
        };
        let silenced = open("/dev/null", OFlags::WRONLY | OFlags::CLOEXEC, Mode::empty())
            .and_then(|null| dup2_stderr(&null));
        let result = f();
        if silenced.is_ok() {
            let _ = dup2_stderr(&saved);
        }
        result
    }
}

#[cfg(not(unix))]
mod quiet {
    pub fn without_stderr<T>(f: impl FnOnce() -> T) -> T {
        f()
    }
}

fn to_ratex(c: Rgb, alpha: f32) -> ratex_types::color::Color {
    let unit = |v: u8| f32::from(v) / 255.0;
    ratex_types::color::Color::new(unit(c.0), unit(c.1), unit(c.2), alpha)
}

/// Width and height from the IHDR chunk, which a PNG must start with.
pub(crate) fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < 24 || !data.starts_with(SIGNATURE) || &data[12..16] != b"IHDR" {
        return None;
    }
    let read =
        |at: usize| -> Option<u32> { Some(u32::from_be_bytes(data[at..at + 4].try_into().ok()?)) };
    Some((read(16)?, read(20)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FG: Rgb = Rgb(0xcd, 0xd6, 0xf4);

    fn decode(png: &Png) -> (png::OutputInfo, Vec<u8>) {
        let decoder = png::Decoder::new(png.data.as_slice());
        let mut reader = decoder.read_info().expect("valid PNG");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("one frame");
        buf.truncate(info.buffer_size());
        (info, buf)
    }

    #[test]
    fn renders_a_formula_to_a_png_of_the_reported_size() {
        let png = render(r"\frac{a}{b} + \sqrt{x}", FG, 20.0).expect("renders");
        let (info, _) = decode(&png);
        assert_eq!((info.width, info.height), (png.width, png.height));
        assert!(png.width > png.height, "{}x{}", png.width, png.height);
    }

    #[test]
    fn background_is_transparent_and_ink_uses_the_color() {
        let png = render("x", FG, 40.0).expect("renders");
        let (info, buf) = decode(&png);
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let pixels: Vec<&[u8]> = buf.chunks(4).collect();
        assert_eq!(pixels[0][3], 0, "corner pixel must be transparent");
        let solid = pixels.iter().find(|p| p[3] == 255).expect("some ink");
        assert_eq!((solid[0], solid[1], solid[2]), (FG.0, FG.1, FG.2));
    }

    #[test]
    fn size_follows_the_em() {
        let small = render("x^2", FG, 10.0).expect("renders");
        let large = render("x^2", FG, 40.0).expect("renders");
        assert!(
            large.height > small.height * 3,
            "{} vs {}",
            large.height,
            small.height
        );
    }

    /// Every display formula in a corpus must draw:
    /// `MDV_CORPUS=<file listing paths> cargo test --release --lib math_image -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs MDV_CORPUS"]
    fn corpus_display_math_renders() {
        use crate::parse::{SpecialBlock, special_blocks};

        let list = std::env::var("MDV_CORPUS").expect("MDV_CORPUS is not set");
        let paths = std::fs::read_to_string(list).expect("cannot read MDV_CORPUS");
        let start = std::time::Instant::now();
        let (mut count, mut failures) = (0, Vec::new());
        for path in paths.lines().filter(|p| !p.trim().is_empty()) {
            let md = std::fs::read_to_string(path).expect(path);
            for block in special_blocks(&md) {
                if let SpecialBlock::Math {
                    literal,
                    display: true,
                } = block
                {
                    count += 1;
                    if let Err(e) = render(&literal, FG, 21.0) {
                        failures.push(format!("{path}: {e}: {}", literal.trim()));
                    }
                }
            }
        }
        eprintln!("display math: {count} formulas in {:?}", start.elapsed());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn invalid_latex_is_an_error() {
        assert!(render(r"\frac{a}", FG, 20.0).is_err());
        assert!(render(r"\left( x", FG, 20.0).is_err());
    }
}
