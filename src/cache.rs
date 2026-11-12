//! Drawn images kept on disk under a hash of everything that shaped them,
//! so the next run reads a file instead of drawing again.

use std::ffi::OsString;
use std::path::PathBuf;

/// A cache directory. Failing to read or write it only costs a redraw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Cache { dir: dir.into() }
    }

    /// `$XDG_CACHE_HOME/mdv`, else `~/.cache/mdv`, else (Windows)
    /// `%LOCALAPPDATA%\mdv`.
    pub fn user() -> Option<Self> {
        user_dir(
            std::env::var_os("XDG_CACHE_HOME"),
            std::env::var_os("HOME"),
            std::env::var_os("LOCALAPPDATA"),
        )
        .map(Cache::new)
    }

    pub fn get(&self, key: u64) -> Option<Vec<u8>> {
        std::fs::read(self.path(key)).ok()
    }

    /// Store `data`. It goes to a temporary file first and is renamed into
    /// place, so a reader never sees half a file.
    pub fn put(&self, key: u64, data: &[u8]) {
        let _ = create_private_dir(&self.dir);
        let tmp = self
            .dir
            .join(format!("{key:016x}.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, data).is_ok() && std::fs::rename(&tmp, self.path(key)).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    fn path(&self, key: u64) -> PathBuf {
        self.dir.join(format!("{key:016x}.png"))
    }
}

/// The images show what private documents contain, so only their owner
/// may read them.
fn create_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

fn user_dir(
    xdg: Option<OsString>,
    home: Option<OsString>,
    local_app_data: Option<OsString>,
) -> Option<PathBuf> {
    let non_empty = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
    non_empty(xdg)
        .or_else(|| non_empty(home).map(|h| h.join(".cache")))
        .or_else(|| non_empty(local_app_data))
        .map(|base| base.join("mdv"))
}

/// A 64-bit FNV-1a hash of `parts` and the mdv version. Each part is
/// prefixed with its length, so `["ab", "c"]` and `["a", "bc"]` differ.
pub fn key(parts: &[&[u8]]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    std::iter::once(version)
        .chain(parts.iter().copied())
        .flat_map(|part| {
            (part.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(part.iter().copied())
        })
        .fold(OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mdv-cache-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn stores_and_reads_back() {
        let dir = temp_dir("roundtrip");
        let cache = Cache::new(&dir);
        assert_eq!(cache.get(7), None);
        cache.put(7, b"png bytes");
        assert_eq!(cache.get(7).as_deref(), Some(&b"png bytes"[..]));
        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().path().extension() == Some("tmp".as_ref()))
            .count();
        assert_eq!(leftovers, 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("private");
        Cache::new(&dir).put(1, b"x");
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unwritable_directory_is_not_an_error() {
        let cache = Cache::new("/dev/null/mdv");
        cache.put(1, b"x");
        assert_eq!(cache.get(1), None);
    }

    #[test]
    fn key_depends_on_every_part_and_its_boundaries() {
        let base = key(&[b"ab", b"c"]);
        assert_eq!(base, key(&[b"ab", b"c"]));
        assert_ne!(base, key(&[b"a", b"bc"]));
        assert_ne!(base, key(&[b"ab", b"d"]));
        assert_ne!(base, key(&[b"ab"]));
    }

    #[test]
    fn user_dir_prefers_xdg_then_home() {
        let some = |s: &str| Some(OsString::from(s));
        assert_eq!(
            user_dir(some("/x"), some("/h"), None),
            Some(PathBuf::from("/x/mdv"))
        );
        assert_eq!(
            user_dir(some(""), some("/h"), None),
            Some(PathBuf::from("/h/.cache/mdv"))
        );
        assert_eq!(
            user_dir(None, None, some("C:\\L")),
            Some(PathBuf::from("C:\\L").join("mdv"))
        );
        assert_eq!(user_dir(None, None, None), None);
    }
}
