# Handoff

This note is the state of `mdv` after milestone M4 (2026-11-14). It is the
only record of the project's decisions: the planning notes and session logs
they came from are not kept. Update it when a milestone lands or a decision
changes.

## 0. Goal and scope

**Goal:** a single Rust binary, `mdv`, that opens Obsidian vault notes and
plain Markdown from the command line with no other tool.

- **Default view:** the terminal. `--browser` opens a local browser view.
- **What it must show unbroken:** math, Mermaid, images and Obsidian syntax
  (wikilinks, callouts, embeds).
- **`mdv check`:** lets an agent find a document's errors itself.

### Requirements

| Id | Requirement | v1? |
|---|---|---|
| P1 | Mermaid in the terminal: an image, else box drawings | yes (done) |
| P2 | The terminal view works over SSH, and code blocks can be copied with OSC 52 | yes (copy **not done**, see section 2) |
| P3 | KaTeX/Mermaid assets inside the binary; no network at run time; no node, bun or Chromium | yes |
| P5 | `mdv check` reports Mermaid syntax, math parse errors, broken images and missing wikilinks as JSON | yes (M5) |
| P4 | Original and translation side by side | v2 |
| P6 | A viewer for Claude and Codex session JSONL | separate tool, not mdv |

**v1 boundary:**
- **In v1:** the criteria in section 2, plus opening notes by name, terminal
  embeds, and link following and backlinks in the browser.
- **v2:** P4, `mdv check --pair`, and an interactive terminal view (moving
  between links without leaving `mdv`).

### Constraints

- No network access at run time.
- Do not copy GPL code. mdfried and utftex were read for structure only.
- merman is used without its EPL-2.0 ELK layout feature. Keep it off.
- Do not invent a new Markdown syntax. Anything added must render cleanly in
  Obsidian and on GitHub.
- Commits and PRs only when the owner asks.

### Decisions and why

| # | Decision | Why | Rejected |
|---|---|---|---|
| D1 | A fast single-binary CLI | The owner opened other tools to read each document, and they failed or drew things wrong | Editor extension, other apps |
| D2 | Terminal by default, browser with a flag | The owner's choice | A native window (system webview) |
| D4 | Rust | The spike measured 8.2 MB, 0 errors on 173 formulas and 165 diagrams, and 40–70 ms per math document. Only Rust had libraries that draw both without a browser | Go (glamour is mature but has no math images and only two Mermaid types) |
| D6 | Terminal Mermaid: an image drawn ahead and cached by content hash when the terminal shows images, else Unicode box drawing. Math: an image, else LaTeX source in color | Keeps both P1 and P2 (SSH) | Images only, box drawing only |
| D7 | Linking in v1: open by name, terminal embeds, browser links and backlinks. Interactive terminal view in v2 | Linking is the core of Obsidian. Adding it in steps keeps the 100 ms start | Browser only; both modes interactive from the start |
| D7-2 | P4 (v2): headings aligned left and right in the browser, per-section mismatch of math, code, numbers and links, and `mdv check --pair`. Paragraph level later | Catches the worst translation errors without changing the converter | Drop P4; paragraph level from the start |
| D9 | Pairing syntax (v2): the translation's front matter has `translation_of: "[[id]]"`. Matching paragraphs end with the same Obsidian block id (`^p12`). Without ids, pair by heading order | File names cannot tell which note translates which. Both are existing Obsidian syntax, so nothing breaks elsewhere | A new `<<pair:...>>` syntax (shows raw in Obsidian and GitHub) |
| D10 | v1 = the criteria in section 2 + D7. v2 = P4, `check --pair`, the interactive terminal | v1 does not wait for the converter work P4 needs | All in v1; the base criteria only |
| D11 | merman draws both the images (SVG rasterized by resvg) and the box drawings; its crates are built for size | On 168 real diagrams both libraries drew images with 0 errors, but mermaid-rs-renderer overlapped or lost flowchart labels. merman SVG averages 6 ms (34 ms for the other). Binary 19.05 MB | mermaid-rs-renderer for images plus merman for boxes (13.2 MB, label problems) |

D9 and D7-2 are written in, and read by, other tools: the vault's policy and
the Markdown converter. `mdv` only reads them.

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
the reporting is missing. The JSON shape has not been designed yet; settle it
first, since agents will parse it.

### Requirement with no milestone yet

**OSC 52 copy (P2):** a way to copy a code block to the clipboard with the
OSC 52 escape. This works over SSH too. It is a v1 requirement but no
milestone has it and nothing is built. Put it before M7, so the release check
covers it.

### End-to-end items on the test vault

`tests/fixtures/vault/` is synthetic. Items 1 and 2 are done in
`tests/vault.rs`.

1. `mdv <name>` finds a note in a subfolder. With two notes of that name, it
   lists both and fails.
2. `![[note]]`, `![[note#Heading]]` and `![[image.png]]` expand. Notes that
   embed each other expand once and stop.
3. (M6) In the browser, clicking `[[note]]`, `[[note|alias]]`,
   `[[note#Heading]]` and `[[note#^block]]` goes to that place.
4. (M6) The backlink list equals the set of notes that link to the note.
5. (M5, M6) A link to a missing note is marked on screen and reported by
   `mdv check`.

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

The baseline documents are private notes. Section 5 says how to collect
them; `scripts/check_residue.py` takes them as arguments.

The five documents for the captures (criterion 7) are private too. Each was
chosen to cover one thing:
1. many display and inline formulas
2. nine flowcharts with Korean labels
3. seven sequence diagrams, the most in the vault
4. embeds and wikilinks together
5. callouts: with one other note, it covers all five callout types

Their names are recorded in the owner's vault, in the session record for
this handoff.

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
- **Raw syntax check (criterion 2):** `scripts/check_residue.py`. It exits
  with 1 when any document has residue.
  ```sh
  scripts/check_residue.py --mdv target/release/mdv --vault <vault> --docs <folder>
  ```
  - `--vault` collects the vault notes that hold Mermaid or a callout.
  - `--docs` adds every `.md` file in a folder.
  - Code block frames and inline code are skipped. Closed `==text==` counts;
    a lone `==` is a comparison in prose and does not.
  - The last run found 0 on the 132 baseline documents.
- **Baseline documents (criterion 1):** they are private, so only how to
  collect them is written here:
  - the chapters of the LLM book used for math (12 files)
  - every vault note with a Mermaid block (40)
  - every vault note with a callout (83)

  That made 118 unique files at first and 132 by the end of 2026-11-14, as
  the vault grows. Collect them again for each check (`--vault` and
  `--docs` do this), and record the vault's git commit with the result.
- **Fuzzing (done in M3, not kept in the repository):**
  - **Seeds:** the 168 real diagrams and merman's 114 examples.
  - **Mutations:** truncated lines, deep nesting, very long lines, odd
    tokens and directives.
  - **Run:** fed to `mermaid::image` and `mermaid::text` in a
    `panic = "unwind"` build, with and without overflow checks.
  - **Result:** 323k inputs, 0 panics.
  - **Bugs found:** a 130,000 px SVG took 11 s to parse, and the sequence
    parser is quadratic on long lines. Both have regression tests in
    `src/mermaid.rs`.

### Measurements to compare against

Release build, macOS, Ghostty, measured 2026-11-11 to 2026-11-14.

| What | Result |
|---|---|
| Real diagrams drawn as images | 168 of 168, 0 errors |
| Real diagrams that fit 120 columns as box drawings | 115 of 168 |
| First run of a Mermaid document | 85–140 ms |
| Second run (disk cache) | 17–21 ms |
| 18 diagrams as box drawings, no graphics | 20 ms |
| A vault note with embeds, by path or by name | ≤10 ms |
| Binary | 19.40 MB |

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
