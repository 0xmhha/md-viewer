//! Obsidian embeds (`![[note]]`, `![[note#Heading]]`, `![[note#^block]]`,
//! `![[image.png]]`) spliced into the Markdown before it is rendered, so
//! embedded math and diagrams are drawn like the document's own.
//!
//! An embed is expanded when its paragraph holds nothing but embeds, as
//! Obsidian shows those as blocks. One inside a sentence stays a link.

use std::path::{Path, PathBuf};

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, parse_document};

use crate::parse;
use crate::vault::{Resolved, Vault};

/// Callout type that marks embedded content. The terminal draws it as a
/// titled box; no Obsidian callout uses this name.
pub const CALLOUT: &str = "mdv-embed";
/// Embeds inside embeds are followed this deep.
const MAX_DEPTH: usize = 8;
/// Embeds expanded in one document. Notes that embed each other fan out
/// without a loop: ten notes embedding all the others would expand
/// millions of times.
const MAX_EMBEDS: usize = 64;
/// Image formats the terminal can draw; other embedded files stay links.
const IMAGES: [&str; 1] = ["png"];

/// `markdown`, read from `doc`, with its block embeds replaced by what
/// they point at in `vault`. Line endings become `\n`.
pub fn expand(markdown: &str, doc: &Path, vault: &Vault) -> String {
    let markdown = normalize(markdown);
    if !markdown.contains("![[") {
        return markdown;
    }
    let mut state = State {
        stack: vec![(canonical(doc), String::new())],
        left: MAX_EMBEDS,
    };
    expand_in(&markdown, doc, vault, &mut state)
}

/// The notes being expanded, outermost first, and how many more embeds
/// may be expanded.
struct State {
    stack: Vec<(PathBuf, String)>,
    left: usize,
}

/// `\r\n` and a lone `\r` end lines as `\n` does, as comrak counts them.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn expand_in(markdown: &str, doc: &Path, vault: &Vault, state: &mut State) -> String {
    let embeds = find_embeds(markdown);
    if embeds.is_empty() {
        return markdown.to_owned();
    }
    let mut lines: Vec<String> = markdown.split('\n').map(str::to_owned).collect();
    // Back to front, so earlier line numbers stay valid.
    for embed in embeds.into_iter().rev() {
        let Some(first) = lines.get(embed.first - 1) else {
            continue;
        };
        let Some(lead) = first.get(..embed.column - 1).map(str::to_owned) else {
            continue;
        };
        let mut blocks = Vec::new();
        for target in &embed.targets {
            match replacement(target, doc, vault, state) {
                Some(block) => blocks.push(block),
                None => blocks.push(format!("![[{target}]]\n")),
            }
        }
        if blocks
            .iter()
            .zip(&embed.targets)
            .all(|(b, t)| b.trim_end() == format!("![[{t}]]"))
        {
            continue;
        }
        let block = blocks.join("\n");
        let spliced: Vec<String> = block
            .trim_end()
            .split('\n')
            .enumerate()
            .map(|(i, line)| {
                let prefix = if i == 0 { &lead } else { &embed.continuation };
                if line.is_empty() {
                    prefix.trim_end().to_owned()
                } else {
                    format!("{prefix}{line}")
                }
            })
            .collect();
        let end = embed.last.min(lines.len());
        lines.splice(embed.first - 1..end, spliced);
    }
    lines.join("\n")
}

/// What an embed of `target` becomes, or `None` to leave it as written.
fn replacement(target: &str, doc: &Path, vault: &Vault, state: &mut State) -> Option<String> {
    let (name, part) = target.split_once('#').unwrap_or((target, ""));
    let title = title(target, doc);
    let file = if name.is_empty() {
        doc.to_path_buf()
    } else {
        match vault.resolve(name, Some(doc)) {
            Resolved::Found(file) => file,
            Resolved::Missing => return Some(boxed(&title, "Not found in the vault.")),
            Resolved::Ambiguous(files) => {
                let list: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
                return Some(boxed(
                    &title,
                    &format!("Several files match: {}.", list.join(", ")),
                ));
            }
        }
    };
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase());
    match ext.as_deref() {
        Some("md") => {}
        Some(e) if IMAGES.contains(&e) => {
            return Some(format!("![{}]({})\n", file_name(&file), destination(&file)));
        }
        _ => return None,
    }
    let key = (canonical(&file), part.to_owned());
    if state.stack.contains(&key) {
        return Some(boxed(&title, "Already embedded above."));
    }
    if state.stack.len() > MAX_DEPTH {
        return Some(boxed(&title, "Embedded too deeply to show."));
    }
    if state.left == 0 {
        return Some(boxed(&title, "Too many embeds to show."));
    }
    state.left -= 1;
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Some(boxed(&title, "Cannot be read."));
    };
    let text = normalize(&text);
    let body = strip_front_matter(&text);
    let content = if let Some(id) = part.strip_prefix('^') {
        block_with_id(body, id)
    } else if part.is_empty() {
        Some(body.to_owned())
    } else {
        section(body, part.rsplit('#').next().unwrap_or(part))
    };
    let Some(content) = content else {
        return Some(boxed(&title, "That part of the note was not found."));
    };
    let content = match file.parent() {
        Some(dir) => absolute_images(&content, dir),
        None => content,
    };
    state.stack.push(key);
    let inner = expand_in(&content, &file, vault, state);
    state.stack.pop();
    Some(boxed(&title, &inner))
}

/// `body` in an embed callout titled `title`.
fn boxed(title: &str, body: &str) -> String {
    let mut out = format!("> [!{CALLOUT}] {title}\n");
    for line in body.trim_end().split('\n') {
        if line.is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// A link destination for `path`: in angle brackets so spaces stay, with
/// the characters that would end or escape it percent-encoded.
pub fn destination(path: &Path) -> String {
    let mut out = String::from("<");
    for c in path.display().to_string().chars() {
        match c {
            '%' => out.push_str("%25"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '\\' => out.push_str("%5C"),
            c => out.push(c),
        }
    }
    out.push('>');
    out
}

/// `markdown` with its relative image paths made absolute from `dir`, so
/// they still point at the right file once spliced into another note.
fn absolute_images(markdown: &str, dir: &Path) -> String {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    let mut spans: Vec<(usize, usize, usize, String)> = root
        .descendants()
        .filter_map(|node| {
            let data = node.data.borrow();
            let NodeValue::Image(link) = &data.value else {
                return None;
            };
            let url = &link.url;
            let relative = !url.is_empty()
                && !url.contains("://")
                && !url.starts_with('/')
                && !url.starts_with('#');
            let (start, end) = (data.sourcepos.start, data.sourcepos.end);
            (relative && start.line == end.line).then(|| {
                let path = dir.join(parse::percent_decode(url));
                (start.line, start.column, end.column, destination(&path))
            })
        })
        .collect();
    if spans.is_empty() {
        return markdown.to_owned();
    }
    spans.sort();
    let mut lines: Vec<String> = markdown.split('\n').map(str::to_owned).collect();
    for (line, start, end, dest) in spans.into_iter().rev() {
        let Some(text) = lines.get_mut(line - 1) else {
            continue;
        };
        let Some(image) = text.get(start - 1..end) else {
            continue;
        };
        if let Some(fixed) = with_destination(image, &dest) {
            text.replace_range(start - 1..end, &fixed);
        }
    }
    lines.join("\n")
}

/// `![alt](old "title")` with `old` replaced by `dest`.
fn with_destination(image: &str, dest: &str) -> Option<String> {
    let open = image.rfind("](")? + 2;
    let rest = &image[open..];
    let old_len = if rest.starts_with('<') {
        rest.find('>')? + 1
    } else {
        rest.find(|c: char| c.is_whitespace() || c == ')')?
    };
    Some(format!("{}{dest}{}", &image[..open], &rest[old_len..]))
}

/// `Note › Heading`, as Obsidian names an embed; a link into the same
/// note is named after it.
fn title(target: &str, doc: &Path) -> String {
    let own = doc
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    target
        .split('#')
        .enumerate()
        .filter_map(|(i, part)| match (i, part.is_empty()) {
            (0, true) => Some(own.clone()),
            (_, true) => None,
            _ => Some(part.to_owned()),
        })
        .collect::<Vec<_>>()
        .join(" › ")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// A paragraph of nothing but embeds: their targets, its lines, the byte
/// column it starts at, and what goes before each line after its first so
/// it stays inside the same quotes, list item or footnote.
#[derive(Debug, PartialEq, Eq)]
struct Embed {
    targets: Vec<String>,
    first: usize,
    last: usize,
    column: usize,
    continuation: String,
}

fn find_embeds(markdown: &str) -> Vec<Embed> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    root.descendants()
        .filter_map(|node| {
            let data = node.data.borrow();
            if !matches!(data.value, NodeValue::Paragraph) || on_task_line(node) {
                return None;
            }
            let targets = only_embeds(node)?;
            Some(Embed {
                targets,
                first: data.sourcepos.start.line,
                last: data.sourcepos.end.line,
                column: data.sourcepos.start.column,
                continuation: continuation(node),
            })
        })
        .collect()
}

/// The targets of a paragraph's embeds, when it holds nothing else.
fn only_embeds<'a>(paragraph: &'a AstNode<'a>) -> Option<Vec<String>> {
    let mut found = Vec::new();
    for child in paragraph.children() {
        match &child.data.borrow().value {
            NodeValue::WikiLink(w) if w.embed => found.push(w.url.clone()),
            NodeValue::Text(t) if t.trim().is_empty() => {}
            NodeValue::SoftBreak | NodeValue::LineBreak => {}
            _ => return None,
        }
    }
    (!found.is_empty()).then_some(found)
}

/// The first paragraph of a task item shares its line with the checkbox,
/// and a box there would read as text after `[ ]`.
fn on_task_line<'a>(paragraph: &'a AstNode<'a>) -> bool {
    paragraph.parent().is_some_and(|item| {
        matches!(item.data.borrow().value, NodeValue::TaskItem(_))
            && item.first_child().is_some_and(|c| c.same_node(paragraph))
    })
}

/// What continues every container around `node` on a following line:
/// `> ` for a quote, the content indent for a list item or footnote.
fn continuation<'a>(node: &'a AstNode<'a>) -> String {
    let mut parts = Vec::new();
    for container in node.ancestors().skip(1) {
        let indent = match &container.data.borrow().value {
            NodeValue::BlockQuote | NodeValue::Alert(_) | NodeValue::Callout(_) => {
                parts.push("> ".to_owned());
                continue;
            }
            NodeValue::Item(list) => list.marker_offset + list.padding,
            NodeValue::TaskItem(_) => container
                .parent()
                .and_then(|list| match &list.data.borrow().value {
                    NodeValue::List(l) => Some(l.marker_offset + l.padding),
                    _ => None,
                })
                .unwrap_or(2),
            NodeValue::FootnoteDefinition(_) => 4,
            _ => continue,
        };
        parts.push(" ".repeat(indent));
    }
    parts.reverse();
    parts.concat()
}

fn strip_front_matter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    match rest.find("\n---\n") {
        Some(end) => &rest[end + 5..],
        None if rest.ends_with("\n---") => "",
        None => text,
    }
}

/// From the heading named `name` up to the next heading of the same or a
/// higher level. Names match without regard to case, as in Obsidian.
fn section(markdown: &str, name: &str) -> Option<String> {
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    let headings: Vec<(usize, u8, String)> = root
        .children()
        .filter_map(|node| {
            let data = node.data.borrow();
            let NodeValue::Heading(h) = data.value else {
                return None;
            };
            Some((data.sourcepos.start.line, h.level, text_of(node)))
        })
        .collect();
    let wanted = name.trim().to_lowercase();
    let at = headings
        .iter()
        .position(|(_, _, text)| text.trim().to_lowercase() == wanted)?;
    let (start, level, _) = headings[at];
    let end = headings[at + 1..]
        .iter()
        .find(|(_, l, _)| *l <= level)
        .map(|(line, _, _)| line - 1);
    let lines: Vec<&str> = markdown.split('\n').collect();
    let end = end.unwrap_or(lines.len()).min(lines.len());
    Some(lines[start - 1..end].join("\n"))
}

/// The block that ends with ` ^id`, without the marker. An id on a line of
/// its own names the block before it, as Obsidian writes ids for quotes,
/// lists and tables.
fn block_with_id(markdown: &str, id: &str) -> Option<String> {
    let marker = format!("^{id}");
    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &parse::options());
    let lines: Vec<&str> = markdown.split('\n').collect();
    let text_of_lines = |node: &AstNode| {
        let pos = node.data.borrow().sourcepos;
        lines
            .get(pos.start.line - 1..pos.end.line)
            .map(|l| l.join("\n"))
    };
    root.descendants().find_map(|node| {
        if !matches!(node.data.borrow().value, NodeValue::Paragraph) {
            return None;
        }
        let text = text_of_lines(node)?;
        if text.trim() == marker {
            return node.previous_sibling().and_then(&text_of_lines);
        }
        let kept = text.trim_end().strip_suffix(&marker)?;
        (kept.is_empty() || kept.ends_with(char::is_whitespace)).then(|| kept.trim_end().to_owned())
    })
}

fn text_of<'a>(node: &'a AstNode<'a>) -> String {
    node.descendants()
        .filter_map(|n| match &n.data.borrow().value {
            NodeValue::Text(t) => Some(t.to_string()),
            NodeValue::Code(c) => Some(c.literal.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
    }

    fn expand_home(markdown: &str) -> String {
        let root = fixture();
        expand(markdown, &root.join("Home.md"), &Vault::open(&root))
    }

    #[test]
    fn a_note_embed_becomes_a_titled_box_without_front_matter() {
        assert_eq!(
            expand_home("Before\n\n![[Recipe]]\n\nAfter\n"),
            "Before\n\n> [!mdv-embed] Recipe\n> Mix **flour** and water.\n\nAfter\n"
        );
    }

    #[test]
    fn a_heading_embed_takes_the_section_and_its_subsections() {
        assert_eq!(
            expand_home("![[Guide#Install]]\n"),
            "> [!mdv-embed] Guide › Install\n> ## Install\n>\n> Run the installer.\n>\n> ### Options\n>\n> Pick a folder.\n"
        );
    }

    #[test]
    fn a_block_embed_takes_the_marked_paragraph() {
        assert_eq!(
            expand_home("![[Guide#^tip]]\n"),
            "> [!mdv-embed] Guide › ^tip\n> Keep backups.\n"
        );
    }

    #[test]
    fn an_image_embed_becomes_an_image_with_its_path() {
        let path = fixture().join("attachments/diagram.png");
        assert_eq!(
            expand_home("![[diagram.png]]\n"),
            format!("![diagram.png](<{}>)\n", path.display())
        );
    }

    #[test]
    fn notes_embedding_each_other_are_expanded_once() {
        let out = expand_home("![[Loop A]]\n");
        assert_eq!(out.matches("A text.").count(), 1, "{out}");
        assert_eq!(out.matches("B text.").count(), 1, "{out}");
        assert!(out.contains("Already embedded above."), "{out}");
    }

    #[test]
    fn a_missing_or_ambiguous_note_says_so() {
        assert!(expand_home("![[Missing note]]\n").contains("Not found in the vault."));
        let out = expand_home("![[Same]]\n");
        assert!(out.contains("a/Same.md, b/Same.md"), "{out}");
    }

    #[test]
    fn an_embed_in_a_quote_keeps_the_quote_marker() {
        assert_eq!(
            expand_home("> Quoted:\n>\n> ![[Recipe]]\n"),
            "> Quoted:\n>\n> > [!mdv-embed] Recipe\n> > Mix **flour** and water.\n"
        );
    }

    #[test]
    fn embeds_inside_a_sentence_or_code_are_left_alone() {
        let md = "Inline ![[Recipe]] mention.\n\n```\n![[Recipe]]\n```\n\n`![[Recipe]]`\n";
        assert_eq!(expand_home(md), md);
    }

    #[test]
    fn a_missing_section_says_so() {
        assert!(expand_home("![[Guide#Nope]]\n").contains("That part of the note was not found."));
    }

    /// A throwaway vault of `notes` (name, text) under the temp folder.
    fn temp_vault(name: &str, notes: &[(&str, String)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("mdv-embed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        for (note, text) in notes {
            std::fs::write(root.join(format!("{note}.md")), text).unwrap();
        }
        root
    }

    // Review regressions

    #[test]
    fn an_embed_in_a_list_item_stays_one_item() {
        assert_eq!(
            expand_home("- ![[Recipe]]\n- next\n"),
            "- > [!mdv-embed] Recipe\n  > Mix **flour** and water.\n- next\n"
        );
        assert_eq!(
            expand_home("1. ![[Recipe]]\n"),
            "1. > [!mdv-embed] Recipe\n   > Mix **flour** and water.\n"
        );
    }

    #[test]
    fn an_embed_in_a_footnote_stays_in_the_footnote() {
        assert_eq!(
            expand_home("x[^1]\n\n[^1]: ![[Recipe]]\n"),
            "x[^1]\n\n[^1]: > [!mdv-embed] Recipe\n    > Mix **flour** and water.\n"
        );
        // A multi-byte label: comrak columns count bytes.
        assert_eq!(
            expand_home("x[^é]\n\n[^é]: ![[Recipe]]\n"),
            "x[^é]\n\n[^é]: > [!mdv-embed] Recipe\n    > Mix **flour** and water.\n"
        );
    }

    #[test]
    fn an_embed_on_a_task_line_stays_a_link() {
        let md = "- [ ] ![[Recipe]]\n";
        assert_eq!(expand_home(md), md);
    }

    #[test]
    fn notes_that_all_embed_each_other_stay_bounded() {
        let names: Vec<String> = (0..10).map(|i| format!("N{i}")).collect();
        let notes: Vec<(&str, String)> = names
            .iter()
            .map(|n| {
                let body: String = names.iter().map(|m| format!("![[{m}]]\n\n")).collect();
                (n.as_str(), format!("{n} text\n\n{body}"))
            })
            .collect();
        let root = temp_vault("fan", &notes);
        let start = std::time::Instant::now();
        let out = expand("![[N0]]\n", &root.join("Doc.md"), &Vault::open(&root));
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        // Each of the at most MAX_EMBEDS expanded notes leaves one box per
        // embed it holds, so the output grows linearly, not by factorial.
        assert_eq!(out.matches(" text").count(), MAX_EMBEDS, "{}", out.len());
        assert!(
            out.matches("[!mdv-embed]").count() <= (MAX_EMBEDS + 1) * (names.len() + 1),
            "{}",
            out.len()
        );
        assert!(out.contains("Too many embeds to show."), "{out}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_lone_carriage_return_ends_a_line() {
        let out = expand_home("a\r\r![[Recipe]]\n\nkeep\n");
        assert!(
            out.contains("Mix **flour**") && out.contains("keep"),
            "{out:?}"
        );
        assert!(!out.contains("![[Recipe]]"), "{out:?}");
    }

    #[test]
    fn a_section_of_the_same_note_can_be_embedded() {
        let root = temp_vault(
            "self",
            &[(
                "Self",
                "## Part\n\npart text\n\n## Use\n\n![[#Part]]\n\n![[Self#Part]]\n".into(),
            )],
        );
        let doc = root.join("Self.md");
        let text = std::fs::read_to_string(&doc).unwrap();
        let out = expand(&text, &doc, &Vault::open(&root));
        assert_eq!(out.matches("part text").count(), 3, "{out}");
        assert!(!out.contains("Already embedded above."), "{out}");
        // The whole note inside itself is still a loop.
        let whole = expand("![[Self]]\n", &doc, &Vault::open(&root));
        assert!(whole.contains("Already embedded above."), "{whole}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn relative_images_in_an_embedded_note_point_at_its_folder() {
        let out = expand_home("![[Pictured]]\n");
        let image = fixture().join("notes/../attachments/diagram.png");
        assert!(out.contains(&format!("(<{}>)", image.display())), "{out}");
    }

    #[test]
    fn a_block_id_on_its_own_line_names_the_block_before_it() {
        assert_eq!(
            expand_home("![[Guide#^qid]]\n"),
            "> [!mdv-embed] Guide › ^qid\n> > Quoted rule.\n"
        );
    }

    #[test]
    fn crlf_front_matter_is_removed() {
        let root = temp_vault("crlf", &[("Win", "---\r\na: 1\r\n---\r\nbody\r\n".into())]);
        let out = expand("![[Win]]\n", &root.join("Doc.md"), &Vault::open(&root));
        assert_eq!(out, "> [!mdv-embed] Win\n> body\n");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn several_embeds_in_one_paragraph_are_all_expanded() {
        let out = expand_home("![[Recipe]]\n![[Guide#^tip]]\n");
        assert!(
            out.contains("Mix **flour**") && out.contains("Keep backups."),
            "{out}"
        );
        assert_eq!(out.matches("[!mdv-embed]").count(), 2, "{out}");
    }

    #[test]
    fn image_destinations_escape_angle_brackets_and_percent() {
        assert_eq!(
            destination(Path::new("/a b/<x>%.png")),
            "</a b/%3Cx%3E%25.png>"
        );
    }

    #[test]
    fn front_matter_is_removed() {
        assert_eq!(strip_front_matter("---\na: 1\n---\nbody\n"), "body\n");
        assert_eq!(strip_front_matter("no front matter\n"), "no front matter\n");
    }
}
