use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use md_viewer::cache::Cache;
use md_viewer::cli::{Cli, Command, color_enabled, no_color_requested, output_width};
use md_viewer::embed;
use md_viewer::term::{RenderOptions, probe, render};
use md_viewer::vault::{self, Vault};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match (&cli.command, &cli.file) {
        (Some(Command::Check { .. }), _) => not_yet("mdv check"),
        (None, Some(_)) if cli.browser => not_yet("--browser"),
        (None, Some(file)) => view(file, &cli),
        (None, None) => {
            eprintln!("error: a Markdown file is required (see `mdv --help`)");
            ExitCode::from(2)
        }
    }
}

fn view(arg: &Path, cli: &Cli) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_default();
    let file = match vault::locate(arg, &cwd) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let file = file.as_path();
    let markdown = match std::fs::read_to_string(file) {
        Ok(text) => expand_embeds(text, file),
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", file.display());
            return ExitCode::FAILURE;
        }
    };
    let stdout = io::stdout();
    let is_tty = stdout.is_terminal();
    let terminal = is_tty
        .then(terminal_size::terminal_size)
        .flatten()
        .map(|(w, _)| usize::from(w.0));
    let color = color_enabled(
        cli.color,
        is_tty,
        no_color_requested(std::env::var_os("NO_COLOR").as_deref()),
    );
    // Images go only to a terminal that is showing colors; a pipe or a
    // file gets the math source and Mermaid box drawings. Documents with
    // nothing to draw skip asking the terminal.
    let has_images = markdown.contains("$$")
        || markdown.contains("```mermaid")
        || markdown.contains("~~~mermaid")
        || (markdown.contains("![") && markdown.to_ascii_lowercase().contains(".png"));
    let opts = RenderOptions {
        width: output_width(cli.width, terminal),
        color,
        graphics: (is_tty && color && has_images)
            .then(probe::detect)
            .flatten(),
        cache: Cache::user(),
        base_dir: file.parent().map(Path::to_path_buf),
    };
    let text = render(&markdown, &opts);
    match stdout.lock().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        // `mdv file | head` closes the pipe early; that is not an error.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: cannot write output: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `markdown` with its embeds expanded from the vault around `file`, or
/// from `file`'s folder when it is in no vault.
fn expand_embeds(markdown: String, file: &Path) -> String {
    if !markdown.contains("![[") {
        return markdown;
    }
    let dir = file.parent().unwrap_or(Path::new("."));
    let vault = match Vault::find_root(dir) {
        Some(root) => Vault::open(&root),
        None => Vault::open_flat(dir),
    };
    embed::expand(&markdown, file, &vault)
}

fn not_yet(feature: &str) -> ExitCode {
    eprintln!("error: {feature} is not implemented yet");
    ExitCode::from(2)
}
