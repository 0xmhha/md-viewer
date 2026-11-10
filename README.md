# md-viewer

`mdv` is a lightweight, single-binary Markdown viewer for the terminal.
It renders math, Mermaid diagrams, images and Obsidian syntax without a
browser, Node.js or network access.

> **Status:** early development. The design is settled; the first version
> is being implemented. Features below are planned unless marked **(done)**.

## Why

Many Markdown previewers show LaTeX math and Mermaid code as raw text, or
need a browser, an editor or a JavaScript runtime to draw them. `mdv` draws
them directly in the terminal from one binary, so you can read a document
right where you work, including over SSH.

## Planned features (v1)

- Terminal rendering by default **(done: text, lists, tables, code, links)**;
  `--browser` opens a local browser view.
- Math (`$...$`, `$$...$$`) rendered as images, with LaTeX source as a
  fallback when the terminal has no graphics support **(done in the
  terminal)**.
- Mermaid diagrams rendered as images, with Unicode box drawings as a
  fallback.
- GitHub Flavored Markdown and relative images.
- Obsidian syntax: wikilinks, embeds, callouts, highlights, front matter
  **(done in the terminal; embeds show as placeholders for now)**.
- Open a note by name inside an Obsidian vault.
- `mdv check`: report Mermaid syntax errors, math parse errors, broken image
  paths and missing wikilinks as JSON, so scripts and agents can verify
  documents.
- No network access at run time; all assets are inside the binary.

## Usage

```sh
mdv note.md                 # render in the terminal
mdv --width 100 note.md     # wrap at 100 columns instead of the terminal width
mdv --color never note.md   # plain text; `auto` (default) also honours NO_COLOR
mdv note.md | less -R       # page long documents
```

Math in the terminal:

- Display math (`$$...$$`) is drawn as an image in the terminal's text color
  when the terminal supports the kitty graphics protocol (checked with
  Ghostty). `mdv` asks the terminal once at start-up; this takes a few
  milliseconds.
- Inline math (`$...$`) becomes Unicode text such as `x²`, `dₖ` and `√dₖ`, so
  it flows with the sentence. A formula that cannot be written this way is
  shown as its LaTeX source.
- Display math is shown as LaTeX source when output goes to a pipe or a file,
  with `--color never`, inside tmux or screen, when the terminal does not
  answer, and when a formula cannot be parsed or does not fit.
- A terminal that answers the start-up question later than 200 ms (1 s over
  SSH) gets LaTeX source, and its late answer may show up at the shell prompt.

Until image rendering lands, Mermaid diagrams are shown as their source code.

## Building from source

Requires the Rust toolchain pinned in `rust-toolchain.toml` (installed
automatically by `rustup`).

```sh
git clone https://github.com/0xmhha/md-viewer.git
cd md-viewer
cargo build --release
./target/release/mdv README.md
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

Third-party components bundled into the binary (such as KaTeX fonts under
the SIL Open Font License 1.1) keep their own licenses; their notices are
listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
