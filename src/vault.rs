//! An Obsidian vault: the folder that holds `.obsidian/`, and the notes
//! and attachments under it, found by name as Obsidian links them.

use std::path::{Path, PathBuf};

/// The folder Obsidian keeps its settings in; its parent is the vault.
const SETTINGS_DIR: &str = ".obsidian";
/// Files read from one vault. A vault this large is searched only in
/// part rather than slowing every start-up.
const MAX_FILES: usize = 200_000;

/// Files below a vault, by path relative to its root.
#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
    files: Vec<PathBuf>,
}

/// What a link name points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// The one file it names, as an absolute path.
    Found(PathBuf),
    Missing,
    /// Several files match and nothing picks one; relative paths, sorted.
    Ambiguous(Vec<PathBuf>),
}

impl Vault {
    /// The nearest folder at or above `start` that holds `.obsidian/`.
    pub fn find_root(start: &Path) -> Option<PathBuf> {
        start
            .ancestors()
            .find(|dir| dir.join(SETTINGS_DIR).is_dir())
            .map(Path::to_path_buf)
    }

    /// Every file below `root`, skipping hidden folders as Obsidian does
    /// (`.obsidian`, `.git`, `.trash`).
    pub fn open(root: &Path) -> Self {
        let mut files = Vec::new();
        collect(root, Path::new(""), &mut files);
        files.sort();
        Vault {
            root: root.to_path_buf(),
            files,
        }
    }

    /// The files directly in `dir`, for a document outside any vault:
    /// its folder may be a home directory, too large to search.
    pub fn open_flat(dir: &Path) -> Self {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| PathBuf::from(e.file_name()))
            .filter(|p| !p.to_string_lossy().starts_with('.'))
            .take(MAX_FILES)
            .collect();
        files.sort();
        Vault {
            root: dir.to_path_buf(),
            files,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file a link name such as `Note`, `folder/Note`, `Note.md` or
    /// `image.png` points at. Names match without regard to case, from the
    /// end of the path. When several match, the one in `from`'s folder
    /// wins, as Obsidian prefers the closest note.
    pub fn resolve(&self, name: &str, from: Option<&Path>) -> Resolved {
        let name = name.trim().trim_start_matches('/');
        if name.is_empty() {
            return Resolved::Missing;
        }
        let mut found = self.matching(name);
        if found.is_empty() {
            found = self.matching(&format!("{name}.md"));
        }
        let here = from
            .and_then(|f| f.parent())
            .and_then(|dir| dir.strip_prefix(&self.root).ok());
        match found.as_slice() {
            [] => Resolved::Missing,
            [one] => Resolved::Found(self.root.join(one)),
            many => match here.and_then(|dir| many.iter().find(|f| f.parent() == Some(dir))) {
                Some(close) => Resolved::Found(self.root.join(close)),
                None => Resolved::Ambiguous(many.to_vec()),
            },
        }
    }

    fn matching(&self, name: &str) -> Vec<PathBuf> {
        let wanted: Vec<String> = name.split('/').map(str::to_lowercase).collect();
        self.files
            .iter()
            .filter(|file| {
                let parts: Vec<String> = file
                    .iter()
                    .map(|c| c.to_string_lossy().to_lowercase())
                    .collect();
                parts.ends_with(&wanted)
            })
            .cloned()
            .collect()
    }
}

/// Why `mdv <name>` found nothing to open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    /// Not a file, and no vault above the current folder to look in.
    NoVault(String),
    NotFound(String),
    Ambiguous(String, Vec<PathBuf>),
    NotANote(String, PathBuf),
}

impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LookupError::NoVault(name) => write!(
                f,
                "{name}: no such file, and no Obsidian vault (a folder with .obsidian/) above the current folder"
            ),
            LookupError::NotFound(name) => write!(f, "{name}: no such file or note in the vault"),
            LookupError::Ambiguous(name, files) => {
                write!(f, "{name}: several notes have this name:")?;
                for file in files {
                    write!(f, "\n  {}", file.display())?;
                }
                Ok(())
            }
            LookupError::NotANote(name, file) => {
                write!(f, "{name}: {} is not a Markdown note", file.display())
            }
        }
    }
}

/// The file `mdv <arg>` shows: `arg` itself when it is a file, else the
/// note of that name in the vault around `cwd`.
pub fn locate(arg: &Path, cwd: &Path) -> Result<PathBuf, LookupError> {
    let path = cwd.join(arg);
    if path.is_file() {
        return Ok(path);
    }
    let name = arg.to_string_lossy().into_owned();
    let root = Vault::find_root(cwd).ok_or_else(|| LookupError::NoVault(name.clone()))?;
    match Vault::open(&root).resolve(&name, None) {
        Resolved::Found(file)
            if file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md")) =>
        {
            Ok(file)
        }
        Resolved::Found(file) => Err(LookupError::NotANote(name, file)),
        Resolved::Missing => Err(LookupError::NotFound(name)),
        Resolved::Ambiguous(files) => Err(LookupError::Ambiguous(name, files)),
    }
}

fn collect(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = rel.join(&name);
        match entry.file_type() {
            Ok(t) if t.is_dir() => collect(root, &path, out),
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
    }

    fn rel(vault: &Vault, r: Resolved) -> Option<PathBuf> {
        match r {
            Resolved::Found(p) => p.strip_prefix(vault.root()).ok().map(Path::to_path_buf),
            _ => None,
        }
    }

    #[test]
    fn the_root_is_the_folder_with_obsidian_settings() {
        let root = fixture();
        assert_eq!(Vault::find_root(&root.join("notes")), Some(root.clone()));
        assert_eq!(Vault::find_root(&root), Some(root));
        assert_eq!(Vault::find_root(Path::new("/")), None);
    }

    #[test]
    fn hidden_folders_are_not_part_of_the_vault() {
        let vault = Vault::open(&fixture());
        assert!(vault.files.iter().all(|f| !f.starts_with(".obsidian")));
        assert!(vault.files.contains(&PathBuf::from("notes/Recipe.md")));
    }

    #[test]
    fn a_note_is_found_by_name_in_any_folder() {
        let vault = Vault::open(&fixture());
        let found = |name| rel(&vault, vault.resolve(name, None));
        assert_eq!(found("Recipe"), Some("notes/Recipe.md".into()));
        assert_eq!(found("recipe"), Some("notes/Recipe.md".into()));
        assert_eq!(found("Recipe.md"), Some("notes/Recipe.md".into()));
        assert_eq!(found("notes/Recipe"), Some("notes/Recipe.md".into()));
        assert_eq!(found("Loop A"), Some("topics/Loop A.md".into()));
        assert_eq!(found("한글 노트"), Some("한글/한글 노트.md".into()));
        assert_eq!(found("diagram.png"), Some("attachments/diagram.png".into()));
    }

    #[test]
    fn a_partial_folder_name_does_not_match() {
        let vault = Vault::open(&fixture());
        assert_eq!(vault.resolve("es/Recipe", None), Resolved::Missing);
        assert_eq!(vault.resolve("No such note", None), Resolved::Missing);
        assert_eq!(vault.resolve("  ", None), Resolved::Missing);
    }

    #[test]
    fn the_same_name_in_two_folders_is_ambiguous() {
        let vault = Vault::open(&fixture());
        assert_eq!(
            vault.resolve("Same", None),
            Resolved::Ambiguous(vec!["a/Same.md".into(), "b/Same.md".into()])
        );
        assert_eq!(
            rel(&vault, vault.resolve("b/Same", None)),
            Some("b/Same.md".into())
        );
    }

    #[test]
    fn a_folder_outside_a_vault_is_read_one_level_deep() {
        let flat = Vault::open_flat(&fixture());
        assert!(flat.files.contains(&PathBuf::from("Home.md")));
        assert!(
            flat.files.iter().all(|f| f.components().count() == 1),
            "{:?}",
            flat.files
        );
    }

    #[test]
    fn the_note_beside_the_linking_one_wins() {
        let vault = Vault::open(&fixture());
        let from = fixture().join("b/Other.md");
        assert_eq!(
            rel(&vault, vault.resolve("Same", Some(&from))),
            Some("b/Same.md".into())
        );
    }
}
