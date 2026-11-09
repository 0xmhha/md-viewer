//! Command-line interface definition.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
