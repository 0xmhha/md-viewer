use std::process::ExitCode;

use clap::Parser;
use md_viewer::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match (cli.command, cli.file) {
        (Some(Command::Check { .. }), _) => not_yet("mdv check"),
        (None, Some(_)) if cli.browser => not_yet("--browser"),
        (None, Some(_)) => not_yet("terminal rendering"),
        (None, None) => {
            eprintln!("error: a Markdown file is required (see `mdv --help`)");
            ExitCode::from(2)
        }
    }
}

fn not_yet(feature: &str) -> ExitCode {
    eprintln!("error: {feature} is not implemented yet");
    ExitCode::from(2)
}
