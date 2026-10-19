//! Storage abstraction (PRD §15, §14) with path sanitization (PRD §26).
//!
//! The trait is the only thing apps see; back-ends: SD card, internal flash
//! (FAT partition), USB mass storage. `HostStorage` is a rooted std::fs
//! implementation used for host tests and the simulator.

use p4_core::{P4Error, P4Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
}

/// Storage back-end. All paths are virtual, rooted at the volume.
pub trait Storage {
    fn root_label(&self) -> &str;
    fn list(&self, dir: &str) -> P4Result<Vec<FileEntry>>;
    fn read(&self, path: &str) -> P4Result<Vec<u8>>;
    fn write(&mut self, path: &str, data: &[u8]) -> P4Result<()>;
    fn delete(&mut self, path: &str) -> P4Result<()>;
    fn rename(&mut self, from: &str, to: &str) -> P4Result<()>;
    fn mkdir(&mut self, path: &str) -> P4Result<()>;
    fn copy(&mut self, from: &str, to: &str) -> P4Result<()> {
        let data = self.read(from)?;
        self.write(to, &data)
    }
    fn move_path(&mut self, from: &str, to: &str) -> P4Result<()> {
        self.rename(from, to)
    }
    fn search(&self, dir: &str, needle: &str, limit: usize) -> P4Result<Vec<String>> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_string()];
        while let Some(d) = stack.pop() {
            for e in self.list(&d)? {
                let p = join(&d, &e.name);
                if e.name.to_lowercase().contains(&needle.to_lowercase()) {
                    out.push(p.clone());
                    if out.len() >= limit {
                        return Ok(out);
                    }
                }
                if e.kind == EntryKind::Dir {
                    stack.push(p);
                }
            }
        }
        Ok(out)
    }
}

/// Sanitize a virtual path: reject traversal, control chars, absolute roots.
/// Returns a normalized relative path (`a/b/c`).
pub fn sanitize(path: &str) -> P4Result<String> {
    let mut parts: Vec<&str> = Vec::new();
    for comp in path.split(['/', '\\']) {
        match comp {
            "" | "." => {}
            ".." => return Err(P4Error::Security(format!("path traversal in '{path}'"))),
            c if c.chars().any(|ch| ch.is_control()) => {
                return Err(P4Error::Security("control char in path".into()))
            }
            c => parts.push(c),
        }
    }
    if parts.is_empty() {
        return Err(P4Error::Security("empty path".into()));
    }
    Ok(parts.join("/"))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// std::fs-backed storage rooted at a directory (host + simulator).
pub struct HostStorage {
    root: PathBuf,
    label: String,
}

impl HostStorage {
    pub fn new(root: impl Into<PathBuf>, label: impl Into<String>) -> P4Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)
            .map_err(|e| P4Error::Storage(format!("root {}: {e}", root.display())))?;
        let canon = fs::canonicalize(&root).unwrap_or(root);
        Ok(Self {
            root: canon,
            label: label.into(),
        })
    }

    fn resolve(&self, path: &str) -> P4Result<PathBuf> {
        // Empty path addresses the volume root (list/search).
        if path.is_empty() {
            return Ok(self.root.clone());
        }
        let rel = sanitize(path)?;
        let p = self.root.join(rel);
        if !p.starts_with(&self.root) {
            return Err(P4Error::Security("resolved path escapes root".into()));
        }
        Ok(p)
    }
}

impl Storage for HostStorage {
    fn root_label(&self) -> &str {
        &self.label
    }

    fn list(&self, dir: &str) -> P4Result<Vec<FileEntry>> {
        let p = self.resolve(dir)?;
        let mut out = Vec::new();
        for entry in fs::read_dir(&p).map_err(|e| P4Error::Storage(e.to_string()))? {
            let entry = entry.map_err(|e| P4Error::Storage(e.to_string()))?;
            let md = entry
                .metadata()
                .map_err(|e| P4Error::Storage(e.to_string()))?;
            out.push(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                kind: if md.is_dir() {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                },
                size: md.len(),
            });
        }
        out.sort_by(
            |a, b| match (a.kind == EntryKind::Dir, b.kind == EntryKind::Dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            },
        );
        Ok(out)
    }

    fn read(&self, path: &str) -> P4Result<Vec<u8>> {
        // Streamed read with a size guard would be added per-volume on device;
        // PRD §28 requires streaming large files, so avoid unbounded allocs by
        // reading in chunks up to a cap of 64 MiB for the host impl.
        let p = self.resolve(path)?;
        let mut f = fs::File::open(&p).map_err(|e| P4Error::Storage(e.to_string()))?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let n = f
                .read(&mut chunk)
                .map_err(|e| P4Error::Storage(e.to_string()))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() > 64 * 1024 * 1024 {
                return Err(P4Error::Storage("file exceeds 64 MiB read cap".into()));
            }
        }
        Ok(buf)
    }

    fn write(&mut self, path: &str, data: &[u8]) -> P4Result<()> {
        let p = self.resolve(path)?;
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| P4Error::Storage(e.to_string()))?;
        }
        fs::write(&p, data).map_err(|e| P4Error::Storage(e.to_string()))
    }

    fn delete(&mut self, path: &str) -> P4Result<()> {
        let p = self.resolve(path)?;
        let md = fs::metadata(&p).map_err(|e| P4Error::Storage(e.to_string()))?;
        let r = if md.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        r.map_err(|e| P4Error::Storage(e.to_string()))
    }

    fn rename(&mut self, from: &str, to: &str) -> P4Result<()> {
        let f = self.resolve(from)?;
        let t = self.resolve(to)?;
        if let Some(parent) = t.parent() {
            fs::create_dir_all(parent).map_err(|e| P4Error::Storage(e.to_string()))?;
        }
        fs::rename(&f, &t).map_err(|e| P4Error::Storage(e.to_string()))
    }

    fn mkdir(&mut self, path: &str) -> P4Result<()> {
        let p = self.resolve(path)?;
        fs::create_dir_all(&p).map_err(|e| P4Error::Storage(e.to_string()))
    }

    /// Recursive copy: directories are copied tree-wise, files byte-wise.
    fn copy(&mut self, from: &str, to: &str) -> P4Result<()> {
        fn rec(src: &Path, dst: &Path) -> std::io::Result<()> {
            if src.is_dir() {
                fs::create_dir_all(dst)?;
                for e in fs::read_dir(src)? {
                    let e = e?;
                    rec(&e.path(), &dst.join(e.file_name()))?;
                }
                Ok(())
            } else {
                fs::copy(src, dst).map(|_| ())
            }
        }
        let f = self.resolve(from)?;
        let t = self.resolve(to)?;
        rec(&f, &t).map_err(|e| P4Error::Storage(e.to_string()))
    }
}

/// PRD §14 layout helpers.
pub const BOOKS_DIR: &str = "books";
pub const APP_DIR: &str = ".app";

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> HostStorage {
        let dir = std::env::temp_dir().join(format!("p4-storage-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        HostStorage::new(dir, "sd").unwrap()
    }

    #[test]
    fn sanitize_rejects_traversal() {
        assert!(matches!(
            sanitize("../etc/passwd"),
            Err(P4Error::Security(_))
        ));
        assert!(matches!(
            sanitize("books/../../x"),
            Err(P4Error::Security(_))
        ));
        assert!(matches!(sanitize(""), Err(P4Error::Security(_))));
        assert_eq!(sanitize("books//a.epub").unwrap(), "books/a.epub");
    }

    #[test]
    fn host_storage_ignores_traversal_attempts() {
        let mut s = tmp("trav");
        assert!(s.read("../../etc/passwd").is_err());
        assert!(s.write("../outside", b"x").is_err()); // traversal rejected
        let _ = fs::remove_dir_all(s.root.clone());
    }

    #[test]
    fn crud_and_sorting() {
        let mut s = tmp("crud");
        s.mkdir("books").unwrap();
        s.write("README.txt", b"hi").unwrap();
        s.write("books/b.epub", b"book").unwrap();
        s.write("config.json", b"{}").unwrap();
        let entries = s.list("").unwrap();
        // dirs first, then files alphabetically
        assert_eq!(entries[0].name, "books");
        assert_eq!(entries[0].kind, EntryKind::Dir);
        let names: Vec<_> = entries.iter().skip(1).map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["README.txt", "config.json"]);
        s.rename("README.txt", "docs/readme.txt").unwrap();
        assert!(s.read("docs/readme.txt").unwrap() == b"hi");
        s.copy("config.json", "docs/config.json").unwrap();
        s.delete("config.json").unwrap();
        assert!(s.read("config.json").is_err());
        let found = s.search("", "readme", 10).unwrap();
        assert_eq!(found, vec!["docs/readme.txt"]);
    }
}
