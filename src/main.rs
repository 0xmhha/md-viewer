//! Spike: measure binary size, render time and output quality for
//! math (RaTeX) and Mermaid (mermaid-rs-renderer) without a browser.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};

enum Block {
    Math { literal: String, display: bool },
    Mermaid(String),
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.math_dollars = true;
    o.extension.wikilinks_title_after_pipe = true;
    o.extension.alerts = true;
    o.extension.highlight = true;
    o.extension.front_matter_delimiter = Some("---".into());
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o
}

fn collect(text: &str) -> Vec<Block> {
    let arena = Arena::new();
    let root = parse_document(&arena, text, &options());
    let mut out = Vec::new();
    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Math(m) => out.push(Block::Math {
                literal: m.literal.clone(),
                display: m.display_math,
            }),
            NodeValue::CodeBlock(cb) if cb.info.trim() == "mermaid" => {
                out.push(Block::Mermaid(cb.literal.clone()))
            }
            _ => {}
        }
    }
    out
}

fn render_math(expr: &str, display: bool) -> Result<Vec<u8>, String> {
    use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
    use ratex_render::{render_to_png, RenderOptions};
    use ratex_types::color::Color;
    use ratex_types::math_style::MathStyle;

    let style = if display { MathStyle::Display } else { MathStyle::Text };
    let ast = ratex_parser::parser::parse(expr).map_err(|e| format!("parse: {e}"))?;
    let lbox = layout(&ast, &LayoutOptions::default().with_style(style).with_color(Color::BLACK));
    validate_layout(&lbox)?;
    let opts = RenderOptions {
        font_size: 32.0,
        padding: 6.0,
        background_color: Color::WHITE,
        font_dir: String::new(),
        device_pixel_ratio: 1.0,
    };
    render_to_png(&to_display_list(&lbox), &opts)
}

fn render_mermaid(src: &str, fontdb: &Arc<resvg::usvg::fontdb::Database>) -> Result<Vec<u8>, String> {
    let t0 = Instant::now();
    let svg = mermaid_rs_renderer::render(src).map_err(|e| format!("mermaid: {e}"))?;
    let t_svg = t0.elapsed();
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb.clone();
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).map_err(|e| format!("svg: {e}"))?;
    let size = tree.size().to_int_size();
    let mut pix = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty svg")?;
    pix.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pix.as_mut());
    let t_raster = t0.elapsed() - t_svg;
    let png = pix.encode_png().map_err(|e| format!("png: {e}"))?;
    if std::env::var_os("MDV_TRACE").is_some() {
        eprintln!("trace mermaid svg_ms={:.1} raster_ms={:.1} encode_ms={:.1} px={}x{}",
            ms(t_svg), ms(t_raster), ms(t0.elapsed() - t_svg - t_raster), size.width(), size.height());
    }
    Ok(png)
}

fn load_fonts() -> Arc<resvg::usvg::fontdb::Database> {
    let mut db = resvg::usvg::fontdb::Database::new();
    // Spike: MDV_FONTS=all loads every system font; default loads two files only.
    if std::env::var("MDV_FONTS").as_deref() == Ok("all") {
        db.load_system_fonts();
    } else {
        for f in ["/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/AppleSDGothicNeo.ttc"] {
            let _ = db.load_font_file(f);
        }
        db.set_sans_serif_family("Helvetica");
    }
    Arc::new(db)
}

/// `mdv --measure <out_dir> <files...>`: render every math/mermaid block to PNG,
/// print one TSV line per file (parse ms, math ms, mermaid ms, counts, errors).
fn measure(out_dir: &Path, files: &[PathBuf]) {
    std::fs::create_dir_all(out_dir).expect("create out dir");
    let t = Instant::now();
    let fontdb = load_fonts();
    println!("fontdb_ms\t{:.1}", ms(t.elapsed()));
    println!("file\tparse_ms\tmath_n\tmath_ms\tmath_err\tmermaid_n\tmermaid_ms\tmermaid_err");
    for f in files {
        let text = std::fs::read_to_string(f).expect("read");
        let stem = f.file_stem().unwrap().to_string_lossy().to_string();
        let t = Instant::now();
        let blocks = collect(&text);
        let parse_ms = ms(t.elapsed());
        let (mut mn, mut mt, mut me, mut gn, mut gt, mut ge) = (0, Duration::ZERO, 0, 0, Duration::ZERO, 0);
        for (i, b) in blocks.iter().enumerate() {
            let t = Instant::now();
            let (res, kind) = match b {
                Block::Math { literal, display } => (render_math(literal, *display), "math"),
                Block::Mermaid(src) => (render_mermaid(src, &fontdb), "mermaid"),
            };
            let el = t.elapsed();
            match (kind, &res) {
                ("math", Ok(_)) => { mn += 1; mt += el }
                ("math", Err(e)) => { me += 1; eprintln!("{stem}#{i} math: {e}") }
                (_, Ok(_)) => { gn += 1; gt += el }
                (_, Err(e)) => { ge += 1; eprintln!("{stem}#{i} mermaid: {e}") }
            }
            if let Ok(png) = res {
                std::fs::write(out_dir.join(format!("{stem}-{i:03}-{kind}.png")), png).ok();
            }
        }
        println!("{stem}\t{parse_ms:.2}\t{mn}\t{:.1}\t{me}\t{gn}\t{:.1}\t{ge}", ms(mt), ms(gt));
    }
}

/// `mdv <file>`: print each math/mermaid block as an image in the terminal.
fn show(file: &Path) {
    let text = std::fs::read_to_string(file).expect("read");
    let fontdb = load_fonts();
    let cfg = viuer::Config { absolute_offset: false, ..Default::default() };
    for b in collect(&text) {
        let res = match &b {
            Block::Math { literal, display } => {
                println!("$ {literal}");
                render_math(literal, *display)
            }
            Block::Mermaid(src) => {
                println!("```mermaid ({} lines)", src.lines().count());
                render_mermaid(src, &fontdb)
            }
        };
        match res.and_then(|png| image::load_from_memory(&png).map_err(|e| e.to_string())) {
            Ok(img) => { viuer::print(&img, &cfg).ok(); }
            Err(e) => eprintln!("  error: {e}"),
        }
    }
}

fn ms(d: Duration) -> f64 { d.as_secs_f64() * 1000.0 }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, out, files @ ..] if flag == "--measure" => {
            measure(Path::new(out), &files.iter().map(PathBuf::from).collect::<Vec<_>>())
        }
        [file] => show(Path::new(file)),
        _ => eprintln!("usage: mdv <file> | mdv --measure <out_dir> <files...>"),
    }
}
