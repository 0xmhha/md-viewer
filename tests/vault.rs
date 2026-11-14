//! The test vault end to end: `mdv` run as a program, from inside the
//! vault, the way it is used.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn vault() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
}

fn mdv_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mdv"))
        .args(["--color", "never", "--width", "80"])
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .output()
        .expect("mdv runs")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn a_note_is_opened_by_name_from_any_folder_of_the_vault() {
    for dir in [vault(), vault().join("notes"), vault().join("한글")] {
        let out = mdv_in(&dir, &["Recipe"]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(
            stdout(&out).contains("Mix flour and water."),
            "{}",
            stdout(&out)
        );
    }
    let out = mdv_in(&vault(), &["한글 노트"]);
    assert!(stdout(&out).contains("본문입니다."), "{}", stderr(&out));
}

#[test]
fn a_name_shared_by_two_notes_lists_both_and_fails() {
    let out = mdv_in(&vault(), &["Same"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("a/Same.md") && err.contains("b/Same.md"),
        "{err}"
    );
    assert!(stdout(&out).is_empty());
}

#[test]
fn an_unknown_name_fails_with_a_message() {
    let out = mdv_in(&vault(), &["No such note"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("No such note"), "{}", stderr(&out));
    let outside = mdv_in(&std::env::temp_dir(), &["Recipe"]);
    assert!(!outside.status.success());
    assert!(stderr(&outside).contains("vault"), "{}", stderr(&outside));
}

#[test]
fn a_file_path_still_opens_that_file() {
    let out = mdv_in(&vault(), &["notes/Guide.md"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("Run the installer."));
}

#[test]
fn embeds_are_expanded_in_place() {
    let out = mdv_in(&vault(), &["Home"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    for expected in [
        "↳ Recipe",
        "Mix flour and water.",
        "↳ Guide › Install",
        "Run the installer.",
        "Pick a folder.",
        "↳ Guide › ^tip",
        "Keep backups.",
        "↳ Loop A",
        "A text.",
        "B text.",
        "Already embedded above.",
        "↳ Missing note",
        "Not found in the vault.",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    // The section stops at the next heading of its level.
    assert!(!text.contains("Open the app."), "{text}");
    // A loop is shown once.
    assert_eq!(text.matches("A text.").count(), 1, "{text}");
    // Without terminal graphics the image is a placeholder.
    assert!(text.contains("[image: diagram.png]"), "{text}");
    // An embed in a sentence stays a link, and code is left alone.
    assert!(
        text.contains("Inline mention ↳ Recipe stays a link."),
        "{text}"
    );
    assert!(text.contains("![[Recipe]]"), "{text}");
}

#[test]
fn no_raw_embed_syntax_is_left_outside_code() {
    let out = mdv_in(&vault(), &["Home"]);
    let text = stdout(&out);
    let outside_code: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start_matches(['│', ' ']).starts_with('│'))
        .filter(|l| !l.contains("![[Recipe]]"))
        .collect();
    for line in outside_code {
        assert!(!line.contains("[[") && !line.contains("[!"), "{line:?}");
    }
}
