use clap::Parser;
use md_viewer::cli::{Cli, Command};

#[test]
fn parses_file_for_terminal_view() {
    let cli = Cli::try_parse_from(["mdv", "note.md"]).unwrap();
    assert_eq!(cli.file.unwrap().to_str(), Some("note.md"));
    assert!(!cli.browser);
    assert!(cli.command.is_none());
}

#[test]
fn parses_browser_flag() {
    let cli = Cli::try_parse_from(["mdv", "--browser", "note.md"]).unwrap();
    assert!(cli.browser);
}

#[test]
fn parses_check_with_files() {
    let cli = Cli::try_parse_from(["mdv", "check", "a.md", "b.md"]).unwrap();
    match cli.command {
        Some(Command::Check { files }) => assert_eq!(files.len(), 2),
        other => panic!("expected check, got {other:?}"),
    }
}

#[test]
fn check_requires_a_file() {
    assert!(Cli::try_parse_from(["mdv", "check"]).is_err());
}
