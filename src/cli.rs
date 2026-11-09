//! Command-line interface definition.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Width used when the output is not a terminal and no `--width` is given.
pub const DEFAULT_WIDTH: usize = 80;
/// Narrowest width accepted by `--width`.
pub const MIN_WIDTH: usize = 20;
/// Widest layout; wider requests are refused or clamped.
pub const MAX_WIDTH: usize = 1000;

/// View Markdown with math, Mermaid and Obsidian syntax in the terminal.
#[derive(Debug, Parser)]
#[command(name = "mdv", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Markdown file to view.
    pub file: Option<PathBuf>,

    /// Open the document in a local browser view instead of the terminal.
    #[arg(long)]
    pub browser: bool,

    /// Wrap text to this many columns (default: terminal width, or 80).
    #[arg(long, value_name = "COLS", value_parser = parse_width)]
    pub width: Option<usize>,

    /// When to use colors and styles. `auto` honours NO_COLOR.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Report rendering problems as JSON.
    Check {
        /// Markdown files to check.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

fn parse_width(s: &str) -> Result<usize, String> {
    let n: usize = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&n) {
        return Err(format!("width must be between {MIN_WIDTH} and {MAX_WIDTH}"));
    }
    Ok(n)
}

/// Whether to emit ANSI styles, given where the output goes.
pub fn color_enabled(choice: ColorChoice, stdout_is_tty: bool, no_color_set: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => stdout_is_tty && !no_color_set,
    }
}

/// The layout width: the flag, else the terminal, else the default.
pub fn output_width(flag: Option<usize>, terminal: Option<usize>) -> usize {
    flag.or(terminal)
        .unwrap_or(DEFAULT_WIDTH)
        .clamp(MIN_WIDTH, MAX_WIDTH)
}

/// NO_COLOR counts only when it is set to a non-empty value (no-color.org).
pub fn no_color_requested(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}
