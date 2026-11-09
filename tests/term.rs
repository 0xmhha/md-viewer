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

// Math and Mermaid (shown as source until M2 and M3)

#[test]
fn inline_math_drops_dollars() {
    assert_eq!(plain("Energy $E=mc^2$.\n").trim_end(), "Energy E=mc^2.");
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
    let out = plain("```rust\nfn main() {}\n```\n\n```mermaid\nflowchart LR\n  A-->B\n```\n");
    assert!(!out.contains("```"), "{out}");
    let l = lines(&out);
    assert!(l.iter().any(|x| x.contains("rust")));
    assert!(l.contains(&"│ fn main() {}"), "{out}");
    assert!(l.iter().any(|x| x.contains("mermaid")));
    assert!(l.contains(&"│   A-->B"), "{out}");
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
