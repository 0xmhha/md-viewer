//! Terminal rendering behaviour, checked on plain (no ANSI) output unless
//! a test is about styles.

mod common;

use md_viewer::term::{RenderOptions, render};
use unicode_width::UnicodeWidthStr;

fn plain_at(md: &str, width: usize) -> String {
    render(
        md,
        &RenderOptions {
            width,
            color: false,
            graphics: None,
            cache: None,
        },
    )
}

fn plain(md: &str) -> String {
    plain_at(md, 80)
}

fn colored(md: &str) -> String {
    render(
        md,
        &RenderOptions {
            width: 80,
            color: true,
            graphics: None,
            cache: None,
        },
    )
}

fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

// Headings

#[test]
fn heading_drops_hash_markers_and_is_underlined() {
    let out = plain("# Title\n\n## Sub\n\n### Third\n");
    let l = lines(&out);
    assert_eq!(l[0], "Title");
    assert_eq!(l[1], "═════");
    assert!(l.contains(&"Sub"));
    assert!(l.contains(&"───"));
    assert!(l.contains(&"Third"));
    assert!(!out.contains('#'));
}

// Inline styles

#[test]
fn inline_markers_are_removed() {
    let out = plain("**bold** *it* ~~gone~~ ==mark== `code`\n");
    assert_eq!(out.trim_end(), "bold it gone mark code");
}

#[test]
fn color_mode_emits_sgr_and_plain_mode_emits_none() {
    let md = "**bold** ==mark== `code`\n";
    assert!(colored(md).contains("\x1b[1m"));
    assert!(!plain(md).contains('\x1b'));
}

#[test]
fn links_use_osc8_in_color_and_show_url_in_plain() {
    let md = "[text](https://example.com) and https://bare.example\n";
    assert_eq!(
        plain(md).trim_end(),
        "text <https://example.com> and https://bare.example"
    );
    assert!(colored(md).contains("\x1b]8;;https://example.com\x1b\\"));
}

// Obsidian syntax

#[test]
fn wikilinks_show_label_or_target() {
    let out = plain("[[note]] [[note|alias]] [[note#Head]] [[note#^p1]]\n");
    assert_eq!(out.trim_end(), "note alias note › Head note › ^p1");
}

#[test]
fn embeds_become_placeholders() {
    let out = plain("![[other note]]\n\n![[diagram.png]]\n");
    assert!(out.contains("↳ other note"), "{out}");
    assert!(out.contains("[image: diagram.png]"), "{out}");
    assert!(!out.contains("[["));
}

#[test]
fn obsidian_only_callout_types_render_with_title() {
    let out = plain("> [!question] Why *this*?\n> body\n\n> [!summary]\n> short\n");
    assert!(!out.contains("[!"), "{out}");
    let l = lines(&out);
    assert!(
        l.iter()
            .any(|x| x.starts_with("│") && x.contains("Why this?"))
    );
    assert!(
        l.iter()
            .any(|x| x.starts_with("│") && x.contains("Summary"))
    );
    assert!(l.contains(&"│ body"));
    assert!(l.contains(&"│ short"));
}

#[test]
fn folded_callout_still_shows_body() {
    let out = plain("> [!tip]- Hidden title\n> secret body\n");
    assert!(out.contains("Hidden title"));
    assert!(out.contains("secret body"));
}

#[test]
fn highlight_has_its_own_style_in_color_mode() {
    let out = colored("==mark==\n");
    assert!(out.contains("mark"));
    assert!(!out.contains("=="));
    assert!(out.contains("\x1b["));
}

#[test]
fn block_ids_comments_and_front_matter_are_hidden() {
    let md = "---\ntitle: x\n---\n\nkept ^p12\n\n%% secret %%\nvisible\n\n%%\nblock secret\n%%\n";
    let out = plain(md);
    assert!(out.contains("kept"));
    assert!(!out.contains("^p12"), "{out}");
    assert!(!out.contains("secret"), "{out}");
    assert!(!out.contains("title"), "{out}");
    assert!(out.contains("visible"));
}

// Math and Mermaid

#[test]
fn inline_math_becomes_unicode_text() {
    assert_eq!(plain("Energy $E=mc^2$.\n").trim_end(), "Energy E=mc².");
    let out = plain("Scale by $\\frac{1}{\\sqrt{d_k}}$ with $\\alpha \\in \\mathbb{R}$.\n");
    assert_eq!(out.trim_end(), "Scale by 1/√dₖ with α ∈ ℝ.");
}

#[test]
fn inline_math_it_cannot_convert_keeps_its_source() {
    let md = "Matrix $\\begin{pmatrix} a \\end{pmatrix}$ here.\n";
    assert_eq!(
        plain(md).trim_end(),
        "Matrix \\begin{pmatrix} a \\end{pmatrix} here."
    );
}

#[test]
fn display_math_is_an_indented_block_without_dollars() {
    let out = plain("Before\n$$\n\\frac{a}{b}\n$$\nafter\n");
    assert!(!out.contains('$'), "{out}");
    assert!(lines(&out).contains(&"    \\frac{a}{b}"), "{out}");
    assert!(out.contains("Before") && out.contains("after"));
}

#[test]
fn code_blocks_are_framed_without_fences() {
    let out = plain("```rust\nfn main() {}\n```\n");
    assert!(!out.contains("```"), "{out}");
    let l = lines(&out);
    assert!(l.iter().any(|x| x.contains("rust")));
    assert!(l.contains(&"│ fn main() {}"), "{out}");
}

#[test]
fn mermaid_is_drawn_with_boxes_without_images() {
    let out = plain("```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n");
    assert!(!out.contains("```") && !out.contains("-->"), "{out}");
    assert!(
        out.contains('┌') && out.contains("Start") && out.contains("End"),
        "{out}"
    );
}

#[test]
fn mermaid_boxes_stay_within_the_width_and_keep_the_prefix() {
    let out = plain_at(
        "> ```mermaid\n> flowchart TD\n>   A[One] --> B[Two]\n> ```\n",
        40,
    );
    for line in lines(&out) {
        assert!(line.starts_with("│ "), "{out}");
        assert!(line.width() <= 40, "{out}");
    }
}

#[test]
fn mermaid_that_cannot_be_drawn_shows_its_source() {
    let out = plain("```mermaid\nflowchart LR\n  A -->\n```\n");
    assert!(out.contains("╭─ mermaid"), "{out}");
    assert!(lines(&out).contains(&"│   A -->"), "{out}");
    let narrow = plain_at(
        "```mermaid\nflowchart LR\n  A[Start here] --> B[End there]\n```\n",
        12,
    );
    assert!(narrow.contains("╭─ mermaid"), "{narrow}");
}

// Lists

#[test]
fn bullet_ordered_and_task_lists() {
    let out = plain("- a\n- b\n\n3. c\n4. d\n\n- [ ] todo\n- [x] done\n");
    let l = lines(&out);
    for want in ["• a", "• b", "3. c", "4. d", "☐ todo", "☑ done"] {
        assert!(l.contains(&want), "missing {want:?} in {out}");
    }
}

#[test]
fn nested_lists_are_indented() {
    let out = plain("- outer\n  - inner\n");
    let l = lines(&out);
    assert!(l.contains(&"• outer"), "{out}");
    assert!(l.contains(&"  ◦ inner"), "{out}");
}

// Layout

#[test]
fn paragraphs_wrap_to_width_counting_wide_characters() {
    let md = "한국어 문장은 글자 하나가 두 칸을 차지하므로 줄 바꿈 계산에서 넓이를 두 배로 세어야 합니다.\n";
    let out = plain_at(md, 20);
    assert!(lines(&out).len() > 1);
    for line in lines(&out) {
        assert!(line.width() <= 20, "too wide: {line:?}");
    }
}

#[test]
fn quote_prefix_repeats_on_wrapped_lines() {
    let out = plain_at("> one two three four five six seven eight\n", 16);
    let l = lines(&out);
    assert!(l.len() > 1);
    assert!(l.iter().all(|x| x.starts_with("│ ")), "{out}");
}

#[test]
fn table_columns_align_with_wide_characters() {
    let out = plain("| 이름 | v |\n|---|--:|\n| 가 | 10 |\n| abc | 2 |\n");
    let widths: Vec<usize> = lines(&out).iter().map(|l| l.width()).collect();
    assert!(widths.len() >= 4, "{out}");
    assert!(widths.iter().all(|w| *w == widths[0]), "{out}");
    assert!(!out.contains("|-"), "{out}");
}

#[test]
fn footnotes_render_reference_and_definition() {
    let out = plain("Claim[^1].\n\n[^1]: Source.\n");
    assert!(out.contains("Claim[1]."), "{out}");
    assert!(out.contains("[1] Source."), "{out}");
}

#[test]
fn thematic_break_is_a_rule() {
    let out = plain_at("a\n\n---\n\nb\n", 10);
    assert!(lines(&out).contains(&"──────────"), "{out}");
}

// Completion criterion for M1: no raw syntax outside code.

#[test]
fn fixture_has_no_raw_syntax_left() {
    let md = include_str!("fixtures/syntax.md");
    let residue = common::raw_syntax_residue(md, &plain(md));
    assert!(residue.is_empty(), "raw syntax left: {residue:?}");
}

/// Run on real documents: `MDV_CORPUS=<file listing paths> cargo test -- --ignored`.
#[test]
#[ignore = "needs MDV_CORPUS"]
fn corpus_has_no_raw_syntax_left() {
    let list = std::env::var("MDV_CORPUS").expect("MDV_CORPUS is not set");
    let paths = std::fs::read_to_string(list).expect("cannot read MDV_CORPUS");
    let mut failures = Vec::new();
    let mut count = 0;
    for path in paths.lines().filter(|p| !p.trim().is_empty()) {
        let md = std::fs::read_to_string(path).expect(path);
        let residue = common::raw_syntax_residue(&md, &plain(&md));
        if !residue.is_empty() {
            failures.push(format!("{path}: {residue:?}"));
        }
        count += 1;
    }
    eprintln!("checked {count} documents");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Regressions found in review.

#[test]
fn empty_list_items_keep_their_markers() {
    let l = plain("- a\n-\n- c\n\n- [ ]\n- [x]\n");
    assert!(lines(&l).contains(&"•"), "{l}");
    assert!(lines(&l).contains(&"☐") && lines(&l).contains(&"☑"), "{l}");
    let nested = plain("- - x\n");
    assert!(lines(&nested).contains(&"• ◦ x"), "{nested}");
    let quoted = plain("- > q\n");
    assert!(lines(&quoted).contains(&"• │ q"), "{quoted}");
}

#[test]
fn tabs_in_table_cells_do_not_break_alignment() {
    let out = plain("| a | b |\n|---|---|\n| x\ty | z |\n| long | q |\n");
    assert!(!out.contains('\t'));
    let widths: Vec<usize> = lines(&out).iter().map(|l| l.width()).collect();
    assert!(widths.iter().all(|w| *w == widths[0]), "{out}");
}

#[test]
fn deep_nesting_neither_overflows_nor_explodes() {
    let md = format!("{}deep text\n", "> ".repeat(50_000));
    let out = plain(&md);
    assert!(out.contains("deep text"));
    assert!(out.len() < 100_000, "output grew to {} bytes", out.len());
}

#[test]
fn no_break_space_keeps_words_together() {
    let out = plain_at(&format!("{} 10\u{a0}km\n", "a".repeat(15)), 20);
    assert!(
        lines(&out).iter().any(|l| l.contains("10\u{a0}km")),
        "{out}"
    );
}

#[test]
fn cjk_text_without_spaces_fills_the_line() {
    let out = plain_at("Intro: 日本語の文章はスペースなしで続きます。\n", 30);
    assert!(lines(&out)[0].width() > 20, "{out}");
    let korean = plain_at("가나다 라마바사아자차카 타파하\n", 12);
    assert!(lines(&korean).contains(&"라마바사아자"), "{korean}");
}

#[test]
fn autolinks_show_their_address_once() {
    let out = plain("<a@b.co> and www.example.com\n");
    assert_eq!(out.trim_end(), "a@b.co and www.example.com");
}

#[test]
fn image_embed_size_is_not_shown_as_its_name() {
    let out = plain("![[img.png|100]] ![[pic.jpg|640x480]]\n");
    assert!(out.contains("[image: img.png]"), "{out}");
    assert!(out.contains("[image: pic.jpg]"), "{out}");
}

#[test]
fn bidi_controls_are_replaced() {
    let out = plain("abc\u{202e}def\u{2066}g\n");
    assert!(
        !out.contains('\u{202e}') && !out.contains('\u{2066}'),
        "{out:?}"
    );
}

// Display math as images (M2)

use md_viewer::math_image::Rgb;
use md_viewer::term::probe::Graphics;

const GHOSTTY: Graphics = Graphics {
    cell_width: 8,
    cell_height: 21,
    foreground: Rgb(0xcd, 0xd6, 0xf4),
    background: Some(Rgb(0x1e, 0x1e, 0x2e)),
    screen_rows: 50,
};

fn with_graphics(md: &str, graphics: Graphics, width: usize) -> String {
    render(
        md,
        &RenderOptions {
            width,
            color: true,
            graphics: Some(graphics),
            cache: None,
        },
    )
}

const KITTY: &str = "\x1b_Ga=T,f=100,t=d,q=2,C=1";

#[test]
fn display_math_is_drawn_as_an_image_instead_of_source() {
    let out = with_graphics("Before\n$$\n\\frac{a}{b}\n$$\nafter\n", GHOSTTY, 80);
    assert!(out.contains(KITTY), "{out:?}");
    assert!(!out.contains("\\frac"), "{out:?}");
    assert!(out.contains("Before") && out.contains("after"));
}

#[test]
fn image_rows_are_reserved_and_the_cursor_returns_below_them() {
    let out = with_graphics("$$\n\\sum_{i=1}^{n} \\frac{a_i}{b_i}\n$$\n", GHOSTTY, 80);
    let up = out
        .split("\x1b[")
        .find_map(|s| s.strip_suffix_char_up())
        .expect("cursor moves up before the image");
    assert!(out.contains(&format!("\x1b[{up}B")), "{out:?}");
    let reserved = out.lines().count();
    assert_eq!(reserved, up + 1, "{out:?}");
}

#[test]
fn image_inside_a_quote_keeps_the_bar_on_every_row() {
    let out = with_graphics("> $$\n> \\int_0^1 \\frac{x}{2}\\,dx\n> $$\n", GHOSTTY, 80);
    assert!(out.contains(KITTY), "{out:?}");
    for line in out.lines() {
        assert!(line.contains('│'), "row without the bar: {line:?}");
    }
}

#[test]
fn math_that_cannot_be_drawn_falls_back_to_source() {
    let out = with_graphics("$$\n\\frac{a}\n$$\n", GHOSTTY, 80);
    assert!(!out.contains(KITTY), "{out:?}");
    assert!(out.contains("\\frac{a}"), "{out:?}");
}

#[test]
fn math_taller_than_the_screen_falls_back_to_source() {
    let tiny_screen = Graphics {
        screen_rows: 2,
        ..GHOSTTY
    };
    let out = with_graphics(
        "$$\n\\frac{\\frac{a}{b}}{\\frac{c}{d}}\n$$\n",
        tiny_screen,
        80,
    );
    assert!(!out.contains(KITTY), "{out:?}");
}

#[test]
fn math_too_wide_even_when_shrunk_falls_back_to_source() {
    let wide = format!("$$\n{}\n$$\n", "a+".repeat(80) + "a");
    let out = with_graphics(&wide, GHOSTTY, 30);
    assert!(!out.contains(KITTY), "{out:?}");
}

#[test]
fn no_graphics_means_no_image_escapes() {
    assert!(!colored("$$\nx^2\n$$\n").contains("\x1b_G"));
}

trait CursorUp {
    fn strip_suffix_char_up(&self) -> Option<usize>;
}

impl CursorUp for str {
    /// `12A...` (the text after `ESC [`) to 12.
    fn strip_suffix_char_up(&self) -> Option<usize> {
        let digits: String = self.chars().take_while(char::is_ascii_digit).collect();
        self[digits.len()..]
            .starts_with('A')
            .then(|| digits.parse().ok())
            .flatten()
    }
}

#[test]
fn empty_display_math_draws_nothing() {
    let out = with_graphics("a\n\n$$\n$$\n\nb\n", GHOSTTY, 80);
    assert!(!out.contains(KITTY), "{out:?}");
}

// Mermaid as images (M3)

const FLOW: &str = "```mermaid\nflowchart LR\n  A[Start] --> B[End]\n```\n";

#[test]
fn mermaid_is_drawn_as_an_image_with_graphics() {
    let out = with_graphics(&format!("Before\n\n{FLOW}\nafter\n"), GHOSTTY, 80);
    assert!(out.contains(KITTY), "{out:?}");
    assert!(!out.contains('┌') && !out.contains("mermaid"), "{out:?}");
    assert!(out.contains("Before") && out.contains("after"));
}

#[test]
fn a_wide_diagram_is_scaled_to_the_text_column() {
    let md = "```mermaid\nflowchart LR\n  A[First step] --> B[Second step] --> C[Third step] --> D[Fourth step]\n```\n";
    let out = with_graphics(md, GHOSTTY, 60);
    assert!(out.contains(",c=60,r="), "{out:?}");
}

#[test]
fn a_diagram_that_would_shrink_too_much_falls_back_to_text() {
    let tiny_screen = Graphics {
        screen_rows: 3,
        ..GHOSTTY
    };
    let md = "```mermaid\nflowchart TD\n  A[One] --> B[Two] --> C[Three] --> D[Four]\n```\n";
    let out = with_graphics(md, tiny_screen, 80);
    assert!(!out.contains(KITTY), "{out:?}");
    assert!(out.contains("One") && out.contains("Four"), "{out:?}");
}

#[test]
fn an_invalid_diagram_with_graphics_shows_its_source() {
    let out = with_graphics("```mermaid\nflowchart LR\n  A -->\n```\n", GHOSTTY, 80);
    assert!(!out.contains(KITTY), "{out:?}");
    assert!(out.contains("A -->"), "{out:?}");
}

#[test]
fn a_top_level_diagram_starts_at_the_first_column() {
    // `CSI 0 C` moves one column; nothing may be sent for column 0.
    let out = with_graphics(FLOW, GHOSTTY, 80);
    assert!(out.contains(KITTY), "{out:?}");
    assert!(!out.contains("\x1b[0C"), "{out:?}");
}

#[test]
fn a_diagram_in_a_quote_keeps_the_bar_on_every_row() {
    let quoted: String = FLOW.lines().map(|l| format!("> {l}\n")).collect();
    let out = with_graphics(&quoted, GHOSTTY, 80);
    assert!(out.contains("\r\x1b[2C\x1b_G"), "{out:?}");
    for line in out.lines().filter(|l| !l.is_empty()) {
        assert!(line.starts_with("\x1b[90m│"), "{line:?}");
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn a_diagram_whose_image_runs_out_of_time_is_not_tried_again_as_text() {
    // Fuzzing regression: merman's sequence parser is quadratic on a long
    // invalid line. The image spent its 2s deadline, then the text drawing
    // spent another 2s on the same parse.
    let md = format!(
        "```mermaid\nsequenceDiagram\nparticipant A\n{}\n```\n",
        "@".repeat(40_000)
    );
    let start = std::time::Instant::now();
    let out = with_graphics(&md, GHOSTTY, 80);
    let spent = start.elapsed();
    assert!(!out.contains(KITTY));
    assert!(out.contains("participant A"));
    assert!(spent < std::time::Duration::from_millis(2500), "{spent:?}");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]
fn diagrams_are_drawn_as_text_side_by_side() {
    // Without images, diagrams used to be drawn as text one after another
    // while the page was laid out: each slow diagram added its own 2s.
    let slow = |name: &str| {
        format!(
            "```mermaid\nsequenceDiagram\nparticipant {name}\n{}\n```\n\n",
            "@".repeat(40_000)
        )
    };
    let md = format!("{}{}{FLOW}", slow("A"), slow("B"));
    let start = std::time::Instant::now();
    let out = plain(&md);
    let spent = start.elapsed();
    assert!(out.contains("participant A") && out.contains("participant B"));
    assert!(out.contains("Start") && out.contains('┌'), "{out}");
    assert!(spent < std::time::Duration::from_millis(2500), "{spent:?}");
}
