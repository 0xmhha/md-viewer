# Handoff

This note is the state of `mdv` after milestone M4 (2026-11-14), so the next
session can start M5 without reading the earlier history. Update it when a
milestone lands.

## 1. Where things stand

`mdv` renders Markdown in the terminal: GFM, Obsidian syntax, math, Mermaid
and vault embeds. `mdv check` and `--browser` are not built yet; both print
`not implemented yet` and exit with status 2.

| Milestone | What it added | PR |
|---|---|---|
| M0 | Crate skeleton, CLI, CI | #1, #2 |
| M1 | GFM and Obsidian syntax as ANSI text | #3 |
| M2 | Display math as images, inline math as Unicode text | #4 |
| M3 | Mermaid as images, box drawings or source; disk cache | #5 |
| — | Stable toolchain locally and in CI | #6, #8 |
| M4 | `mdv <name>` in a vault, block embeds, local PNG images | #7 |

Each milestone is one PR, written test first, squash-merged into `main`.

## 2. What is left for v1

| Milestone | Builds | Done when |
|---|---|---|
| M5 check | `mdv check <files>` reports problems as JSON | The v1 baseline documents report no errors, and a test document broken on purpose has every error found |
| M6 browser | `--browser`, following links, backlinks | End-to-end tests on the test vault pass; the page makes no network requests |
| M7 release check | Size and speed scripts, captures of five representative documents | All seven v1 criteria below pass and the user approves the captures |

`mdv check` should report Mermaid syntax errors, math parse errors, broken
image paths and missing wikilinks. The pieces exist already:

- `mermaid::image` and `mermaid::text` return `Failure`.
- `math_image::render` returns an error.
- `vault::Vault::resolve` returns `Missing` and `Ambiguous`.
- `embed` already writes "Not found in the vault." into its boxes.

The CLI already parses `mdv check <files...>` into `Command::Check`. Only
the reporting is missing.

### v1 completion criteria

1. The baseline documents render with no raw syntax or errors, in both the
   terminal and the browser.
2. Automated check: outside code, the terminal output contains no `$$`,
   `` ```mermaid ``, `[[`, `> [!` or `==text==`.
3. `mdv check` reports 0 errors on the baseline documents.
4. First screen in 100 ms or less (Mermaid images excluded).
5. Release binary 20 MB or smaller.
6. End-to-end tests on the test vault in this repository pass: open by name,
   embeds, following links, backlinks.
7. The user approves terminal captures of five representative documents.

The baseline documents are private notes. Their list lives outside this
repository and is passed to the corpus tests as a file (section 5).

### Not finished from earlier milestones

- **Visual check:** math and Mermaid images, and PNGs drawn by M4, have not
  been looked at in a real kitty-protocol terminal (Ghostty). Do this before
  the M7 captures.
- **Cache size:** `~/.cache/mdv` is never cleaned; the README says so.
- **Hidden diagrams:** Mermaid blocks nested past `MAX_NESTING` are still
  drawn ahead of layout, then not shown. This wastes time but gives the
  right output.
- **Bases embeds:** `![[x.base]]` (Obsidian Bases) and other non-note files
  stay links. A Bases file is a query that only Obsidian can run.
- **Image formats:** only PNG is drawn. Other formats keep the
  `[image: …]` placeholder.
- **Upstream bug:** merman's sequence diagram parser is quadratic on a long
  invalid line. `mdv` caps it with a 2 s budget per diagram. The bug has not
  been reported upstream.

### Decision still open before M6

How the browser view draws math and Mermaid:

- **Server-side SVG:** serve SVG made by the same RaTeX and merman code the
  terminal uses. The output matches the terminal and the binary stays small.
- **Bundled KaTeX and mermaid.js:** the best drawing quality, but an
  estimated 4 MB more (not measured).

The binary is 19.40 MB against the 20 MB limit, so the second option needs
measuring first.

## 3. Code map

| Path | Does |
|---|---|
| `src/main.rs` | Finds the file (`vault::locate`), expands embeds, probes the terminal, renders |
| `src/cli.rs` | clap definition: `FILE`, `--width`, `--color`, `--browser`, `check` |
| `src/parse.rs` | comrak options shared by everything; `percent_decode` |
| `src/vault.rs` | Vault root (`.obsidian/`), file list, link-name resolution, `mdv <name>` lookup |
| `src/embed.rs` | Splices block embeds into the Markdown text before rendering |
| `src/math_image.rs` | RaTeX to PNG |
| `src/mermaid.rs` | merman to SVG to PNG (resvg), and to box-drawing text; 2 s budget |
| `src/cache.rs` | Disk cache for drawn images, keyed by content and renderer versions |
| `src/term/mod.rs` | `render` and `RenderOptions` |
| `src/term/block.rs` | Block nodes to lines; images placed with the kitty protocol |
| `src/term/inline.rs` | Inline nodes to styled spans |
| `src/term/diagrams.rs` | Draws all Mermaid diagrams on worker threads before layout; `fit`/`placement` |
| `src/term/probe.rs` | Asks the terminal for kitty support, colors and cell size |
| `src/term/callout.rs` | Callouts, including the `mdv-embed` box |
| `tests/term.rs` | Rendering behaviour on strings |
| `tests/vault.rs` | End-to-end: runs the `mdv` binary inside `tests/fixtures/vault` |
| `tests/fixtures/vault/` | Synthetic test vault. Never put real vault content here |

## 4. Decisions to keep

- **merman, not mermaid.js:** merman draws both images and box drawings,
  with ELK left out. It is pinned with `=` because it is alpha.
- **`panic = "abort"`:** `unwind` measured 22.5 MB, over the size limit.
  Fuzzing 323k inputs found no panic in merman on mdv's paths. merman's own
  `catch_unwind` is only in a feature mdv does not enable.
- **Size:** merman crates are built with `opt-level = "s"`
  (`Cargo.toml`), which keeps the binary under 20 MB.
- **Pre-drawing:** diagrams are drawn before layout, in parallel. Text
  drawings are made too when there is no graphics. A diagram that runs out
  of time as an image is not tried again as text.
- **Embeds as text:**
  - Embeds are spliced into the Markdown, not rendered as separate
    documents. Math, Mermaid and the cache then work inside them, and M6
    can reuse the same step.
  - Container prefixes are rebuilt from the AST, not copied from the first
    line.
  - Limits: depth 8, 64 expansions per document, and exact
    `(file, part)` loop detection.
- **Toolchain:** `rust-toolchain.toml` follows `stable`, and CI runs
  `rustup update stable` first. A runner image once had an older stable
  than local, and the two clippy versions disagreed on a lint.

## 5. Working on it

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --release            # also runs the timing tests ignored in debug
cargo build --release && stat -f %z target/release/mdv   # size (macOS)
```

- **Corpus tests:** run them on private documents without adding those
  documents here. Give a file that lists one path per line:
  ```sh
  MDV_CORPUS=list.txt cargo test --release --lib mermaid -- --ignored --nocapture
  MDV_CORPUS=list.txt cargo test --release --lib math_image -- --ignored --nocapture
  ```
- **Raw syntax check (criterion 2):** this was run with a script that is
  not in the repository yet.
  - It renders each document with `--color always --width 120`.
  - It drops code block frames (`╭─` … `╰─`) and inline code (SGR 32).
  - It then counts `$$`, `` ```mermaid ``, `[[`, `> [!` and closed
    `==text==` pairs. A lone `==` is a comparison, and Obsidian shows it as
    is.
  - Move it into `scripts/` with the M7 measurement scripts. The last run
    found 0 on the 118 baseline documents.

### Things that cost time before

- **Byte columns:** comrak `sourcepos` columns count bytes, not characters.
  Slice lines with byte ranges.
- **Pipes in tables:** in a GFM table, `|` splits the cell even inside
  inline code. Write `\|`, or reword the cell to avoid the pipe.
- **Line endings:** comrak treats a lone `\r` as a line end, while
  `str::lines()` does not. `embed` normalizes line endings first.
- **Mutual embeds:** notes that embed each other fan out by factorial
  unless the total number of expansions is capped.
- **Timing tests:** wall-clock assertions fail on slower CI machines.
  Release-only timing tests use
  `#[cfg_attr(debug_assertions, ignore = "timing needs a release build")]`.
  Prefer a check that does not depend on time.
- **Public repository:** no paths, names or text from private notes in
  tests, fixtures, commits or PRs.

## 6. Branches

- `main` holds everything above.
- `spike/rust-render` is the original measurement spike, using
  mermaid-rs-renderer and viuer. It is kept for testing. Decide at v1
  release whether to delete it.
