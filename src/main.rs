use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use md_viewer::cli::{Cli, Command, color_enabled, no_color_requested, output_width};
use md_viewer::term::{RenderOptions, render};

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

fn view(file: &Path, cli: &Cli) -> ExitCode {
    let markdown = match std::fs::read_to_string(file) {
        Ok(text) => text,
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
    let opts = RenderOptions {
        width: output_width(cli.width, terminal),
        color: color_enabled(
            cli.color,
            is_tty,
            no_color_requested(std::env::var_os("NO_COLOR").as_deref()),
        ),
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

fn not_yet(feature: &str) -> ExitCode {
    eprintln!("error: {feature} is not implemented yet");
    ExitCode::from(2)
}
