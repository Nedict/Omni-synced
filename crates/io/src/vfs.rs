//! File access for the engine. Natively it is the disk. In the browser (wasm32) there is no
//! file system to call synchronously, so files live in memory here: the web host loads them
//! (from the browser's private storage, a file picker or a drop) before a command reads them,
//! and takes what commands wrote ([`take_writes`]) to store it or hand it out as a download.

use std::io::{Error, ErrorKind, Result};

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// At most `max` bytes of a regular file. A character device such as `/dev/zero` reports a
    /// length of 0, so checking the metadata and then reading to the end is unbounded; a FIFO
    /// blocks the caller instead. Refuse anything that is not a regular file, and never read
    /// more than the caller's cap plus the byte that proves it was exceeded.
    pub fn read(path: &str, max: usize) -> Result<Vec<u8>> {
        check(path)?;
        // Cheap and non-blocking: refuse before opening, in case the open itself would block.
        let meta = std::fs::metadata(path)?;
        if !meta.is_file() {
            return Err(Error::new(ErrorKind::InvalidInput, "not a regular file"));
        }
        if meta.len() > max as u64 {
            return Err(Error::new(ErrorKind::InvalidData, "file too large"));
        }
        let f = std::fs::File::open(path)?;
        // The path may have been swapped between the two calls; the handle decides.
        if !f.metadata()?.is_file() {
            return Err(Error::new(ErrorKind::InvalidInput, "not a regular file"));
        }
        let mut out = Vec::with_capacity(meta.len().min(16 << 20) as usize);
        std::io::Read::read_to_end(&mut std::io::Read::take(f, max as u64 + 1), &mut out)?;
        if out.len() > max {
            return Err(Error::new(ErrorKind::InvalidData, "file too large"));
        }
        Ok(out)
    }
    pub fn write(path: &str, bytes: &[u8]) -> Result<()> {
        check(path)?;
        std::fs::write(path, bytes)
    }
    pub fn len(path: &str) -> Result<u64> {
        check(path)?;
        std::fs::metadata(path).map(|m| m.len())
    }
    pub fn exists(path: &str) -> bool {
        check(path).is_ok() && std::path::Path::new(path).exists()
    }
    pub fn remove(path: &str) -> Result<()> {
        check(path)?;
        std::fs::remove_file(path)
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    thread_local! {
        static FILES: RefCell<BTreeMap<String, Vec<u8>>> = const { RefCell::new(BTreeMap::new()) };
        static WRITES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn missing(path: &str) -> Error {
        Error::new(ErrorKind::NotFound, format!("{path}: no such file in this browser"))
    }

    /// At most `max` bytes, to match the native signature.
    pub fn read(path: &str, max: usize) -> Result<Vec<u8>> {
        let bytes = FILES.with(|f| f.borrow().get(path).cloned()).ok_or_else(|| missing(path))?;
        if bytes.len() > max {
            return Err(Error::new(ErrorKind::InvalidData, "file too large"));
        }
        Ok(bytes)
    }
    pub fn write(path: &str, bytes: &[u8]) -> Result<()> {
        FILES.with(|f| f.borrow_mut().insert(path.to_string(), bytes.to_vec()));
        WRITES.with(|w| {
            let mut w = w.borrow_mut();
            w.retain(|p| p != path);
            w.push(path.to_string());
        });
        Ok(())
    }
    pub fn len(path: &str) -> Result<u64> {
        FILES.with(|f| f.borrow().get(path).map(|b| b.len() as u64)).ok_or_else(|| missing(path))
    }
    pub fn exists(path: &str) -> bool {
        FILES.with(|f| f.borrow().contains_key(path))
    }
    pub fn remove(path: &str) -> Result<()> {
        FILES.with(|f| f.borrow_mut().remove(path)).map(|_| ()).ok_or_else(|| missing(path))
    }
    pub fn insert(path: &str, bytes: Vec<u8>) {
        FILES.with(|f| f.borrow_mut().insert(path.to_string(), bytes));
    }
    pub fn take_writes() -> Vec<(String, Vec<u8>)> {
        let paths = WRITES.with(|w| std::mem::take(&mut *w.borrow_mut()));
        paths.into_iter().filter_map(|p| FILES.with(|f| f.borrow().get(&p).cloned()).map(|b| (p, b))).collect()
    }
    pub fn list(prefix: &str) -> Vec<(String, u64)> {
        FILES.with(|f| f.borrow().iter().filter(|(p, _)| p.starts_with(prefix)).map(|(p, b)| (p.clone(), b.len() as u64)).collect())
    }
}

pub use imp::*;

/// The folders an agent's commands may read, write and delete in (an MCP server's, see
/// docs/mcp.md). While a scope is active on this thread ([`scoped`]), every file call here
/// refuses paths outside it; without one (the user's own file dialogs, the CLI) nothing changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathScope {
    roots: Vec<std::path::PathBuf>,
}

thread_local! {
    static SCOPE: std::cell::RefCell<Option<PathScope>> = const { std::cell::RefCell::new(None) };
}

impl PathScope {
    /// Confine file access to `roots` (resolved now; folders that don't exist are left out, so an
    /// empty scope refuses every path).
    pub fn new(roots: impl IntoIterator<Item = std::path::PathBuf>) -> PathScope {
        let mut out: Vec<std::path::PathBuf> = Vec::new();
        for r in roots {
            if let Ok(r) = std::fs::canonicalize(&r)
                && !out.contains(&r)
            {
                out.push(r);
            }
        }
        PathScope { roots: out }
    }

    pub fn roots(&self) -> &[std::path::PathBuf] {
        &self.roots
    }

    /// Whether `path` lies inside one of the roots, after following symlinks and `..`.
    pub fn check(&self, path: &str) -> Result<()> {
        let refused = || {
            let roots: Vec<String> = self.roots.iter().map(|r| r.display().to_string()).collect();
            Error::new(
                ErrorKind::PermissionDenied,
                format!(
                    "{path}: outside the folders this agent may use ({}); allow another folder with `--allow-path DIR`",
                    if roots.is_empty() { "none".into() } else { roots.join(", ") }
                ),
            )
        };
        // A UNC path (`\\server\share`) would make Windows connect out before any check could run.
        if cfg!(windows) && matches!(path.as_bytes().get(..2), Some([b'\\' | b'/', b'\\' | b'/'])) {
            return Err(refused());
        }
        let resolved = resolve(std::path::Path::new(path)).map_err(|_| refused())?;
        if self.roots.iter().any(|r| resolved.starts_with(r)) { Ok(()) } else { Err(refused()) }
    }
}

/// `path` with symlinks and `..` resolved. A file that doesn't exist yet resolves through its
/// folder, which must exist; a symlink whose target is missing is refused (writing through it
/// would land wherever it points).
fn resolve(path: &std::path::Path) -> Result<std::path::PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(p) => Ok(p),
        Err(e) if e.kind() == ErrorKind::NotFound && std::fs::symlink_metadata(path).is_err() => {
            let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else { return Err(e) };
            let parent = if parent.as_os_str().is_empty() { std::path::Path::new(".") } else { parent };
            Ok(std::fs::canonicalize(parent)?.join(name))
        }
        Err(e) => Err(e),
    }
}

/// Run `f` with file access confined to `scope` on this thread (`None`: unconfined).
pub fn scoped<T>(scope: Option<PathScope>, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathScope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            SCOPE.with(|s| *s.borrow_mut() = prev);
        }
    }
    let _restore = Restore(SCOPE.with(|s| s.replace(scope)));
    f()
}

/// The scope active on this thread.
pub fn current_scope() -> Option<PathScope> {
    SCOPE.with(|s| s.borrow().clone())
}

/// Refuse `path` if a scope is active and it lies outside. Code that reaches the disk without
/// going through this module calls it first. In the browser files are the host's, never the
/// disk, so there is nothing to confine.
// ponytail: checked then opened, so a local process racing a symlink swap in an allowed folder
// could still redirect one call; opening by handle (openat/O_NOFOLLOW) would close that.
pub fn check(path: &str) -> Result<()> {
    if cfg!(target_arch = "wasm32") {
        return Ok(());
    }
    SCOPE.with(|s| s.borrow().as_ref().map_or(Ok(()), |scope| scope.check(path)))
}

/// A file's text, at most `max` bytes.
pub fn read_to_string(path: &str, max: usize) -> Result<String> {
    String::from_utf8(read(path, max)?).map_err(|e| Error::new(ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    #[test]
    fn files_round_trip() {
        let dir = std::env::temp_dir().join(format!("solidcraft-vfs-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("a.txt").to_string_lossy().to_string();
        assert!(!super::exists(&p));
        assert!(super::read(&p, 16).is_err());
        super::write(&p, b"hello").unwrap();
        assert!(super::exists(&p));
        assert_eq!(super::len(&p).unwrap(), 5);
        assert_eq!(super::read_to_string(&p, 16).unwrap(), "hello");
        // The cap is the caller's, not the file's.
        assert!(super::read(&p, 4).is_err());
        assert_eq!(super::read(&p, 5).unwrap(), b"hello");
        super::remove(&p).unwrap();
        assert!(!super::exists(&p));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `metadata().len()` is 0 for a character device and for a FIFO, so a cap checked against
    /// it passes and the read then runs to an end that never comes. Refuse what is not a
    /// regular file, and do it before opening: opening a FIFO blocks until a writer appears.
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    #[test]
    fn special_files_are_refused_without_blocking() {
        let e = super::read("/dev/zero", 1024).unwrap_err();
        assert!(e.to_string().contains("not a regular file"), "{e}");
        assert!(super::read_to_string("/dev/zero", 1024).is_err());

        let dir = std::env::temp_dir().join(format!("solidcraft-vfs-fifo-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let fifo = dir.join("p");
        let made = std::process::Command::new("mkfifo").arg(&fifo).status().map(|s| s.success()).unwrap_or(false);
        if made {
            let path = fifo.to_string_lossy().to_string();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(super::read(&path, 1024).is_err());
            });
            let refused = rx.recv_timeout(std::time::Duration::from_secs(5));
            assert_eq!(refused, Ok(true), "reading a FIFO must be refused, not block");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An agent's scope (#205): inside its folders files work as before; outside, through `..`
    /// or through a symlink that leads out, every call is refused with a clear error and the
    /// disk is left alone. Without a scope nothing is confined.
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    #[test]
    fn a_scope_confines_every_call_to_its_folders() {
        use super::{PathScope, scoped};
        let base = std::env::temp_dir().join(format!("solidcraft-vfs-scope-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (root, out) = (base.join("root"), base.join("out"));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(&out, root.join("link")).unwrap();
        std::os::unix::fs::symlink(out.join("missing.txt"), root.join("dangling")).unwrap();
        let s = |p: std::path::PathBuf| p.to_string_lossy().to_string();
        let scope = PathScope::new([root.clone()]);
        scoped(Some(scope.clone()), || {
            let inside = s(root.join("sub/a.txt"));
            super::write(&inside, b"ok").unwrap();
            assert_eq!(super::read(&inside, 16).unwrap(), b"ok");
            assert_eq!(super::len(&inside).unwrap(), 2);
            assert!(super::exists(&inside));
            super::remove(&inside).unwrap();
            for escape in [out.join("secret.txt"), root.join("../out/secret.txt"), root.join("link/secret.txt"), root.join("dangling")] {
                let p = s(escape);
                let e = super::read(&p, 16).unwrap_err();
                assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied, "{p}: {e}");
                assert!(e.to_string().contains("outside the folders this agent may use"), "{e}");
                assert!(super::write(&p, b"x").is_err() && super::remove(&p).is_err() && super::len(&p).is_err() && !super::exists(&p), "{p}");
            }
            assert!(super::write(&s(out.join("new.txt")), b"x").is_err());
            assert!(super::check(&s(root.join("sub/../sub/b.txt"))).is_ok(), "`..` that stays inside is fine");
        });
        assert_eq!(std::fs::read_to_string(out.join("secret.txt")).unwrap(), "secret");
        assert!(!out.join("new.txt").exists() && !out.join("missing.txt").exists());
        // The scope ends with the call; an empty one refuses everything.
        assert!(super::current_scope().is_none());
        assert_eq!(super::read(&s(out.join("secret.txt")), 16).unwrap(), b"secret");
        assert!(PathScope::new([base.join("nonexistent")]).check(&s(root.join("sub/a.txt"))).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A directory is not a file either.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn directories_are_not_files() {
        let dir = std::env::temp_dir().to_string_lossy().to_string();
        assert!(super::read(&dir, 1024).is_err());
    }
}
