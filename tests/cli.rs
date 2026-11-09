use clap::Parser;
use md_viewer::cli::{Cli, ColorChoice, Command, color_enabled, no_color_requested, output_width};

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

#[test]
fn parses_width_and_color() {
    let cli = Cli::try_parse_from(["mdv", "--width", "60", "--color", "never", "a.md"]).unwrap();
    assert_eq!(cli.width, Some(60));
    assert_eq!(cli.color, ColorChoice::Never);
}

#[test]
fn rejects_too_narrow_width() {
    assert!(Cli::try_parse_from(["mdv", "--width", "5", "a.md"]).is_err());
}

#[test]
fn color_follows_tty_and_no_color() {
    assert!(color_enabled(ColorChoice::Auto, true, false));
    assert!(!color_enabled(ColorChoice::Auto, false, false));
    assert!(!color_enabled(ColorChoice::Auto, true, true));
    assert!(color_enabled(ColorChoice::Always, false, true));
    assert!(!color_enabled(ColorChoice::Never, true, false));
}

#[test]
fn width_prefers_flag_then_terminal_then_default() {
    assert_eq!(output_width(Some(70), Some(120)), 70);
    assert_eq!(output_width(None, Some(120)), 120);
    assert_eq!(output_width(None, None), 80);
}

// The binary itself.

fn mdv(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_mdv"))
        .args(args)
        .output()
        .expect("run mdv")
}

#[test]
fn renders_a_file_without_escapes_when_piped() {
    let out = mdv(&["--width", "60", "tests/fixtures/syntax.md"]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Heading one"));
    assert!(!text.contains('\x1b'));
}

#[test]
fn color_always_emits_escapes() {
    let out = mdv(&["--color", "always", "tests/fixtures/syntax.md"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains('\x1b'));
}

#[test]
fn missing_file_fails_with_a_message() {
    let out = mdv(&["no-such-file.md"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no-such-file.md"));
}

#[test]
fn rejects_absurd_width() {
    assert!(Cli::try_parse_from(["mdv", "--width", "18446744073709551615", "a.md"]).is_err());
    assert!(Cli::try_parse_from(["mdv", "--width", "100000", "a.md"]).is_err());
}

#[test]
fn empty_no_color_does_not_disable_color() {
    assert!(no_color_requested(Some(std::ffi::OsStr::new("1"))));
    assert!(!no_color_requested(Some(std::ffi::OsStr::new(""))));
    assert!(!no_color_requested(None));
}
