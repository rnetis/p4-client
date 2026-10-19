//! File manager app (PRD §15): browse/open/rename/delete/mkdir/copy/move/
//! search across volumes. Deletion requires confirmation (enforced by the
//! explicit `confirmed` argument here; the UI shows a dialog).

use p4_core::{App, P4Error, P4Result};
use p4_storage::{EntryKind, FileEntry, Storage};

pub struct FilesApp {
    pub volumes: Vec<Box<dyn Storage>>,
    pub active_volume: usize,
    pub cwd: String,
}

impl FilesApp {
    pub fn new(volumes: Vec<Box<dyn Storage>>) -> Self {
        Self {
            volumes,
            active_volume: 0,
            cwd: String::new(),
        }
    }

    fn volume(&self) -> Result<&dyn Storage, P4Error> {
        self.volumes
            .get(self.active_volume)
            .map(|v| v.as_ref())
            .ok_or_else(|| P4Error::Storage("no volume".into()))
    }

    fn volume_mut(&mut self) -> Result<&mut (dyn Storage + '_), P4Error> {
        match self.volumes.get_mut(self.active_volume) {
            Some(v) => Ok(v.as_mut()),
            None => Err(P4Error::Storage("no volume".into())),
        }
    }

    pub fn select_volume(&mut self, index: usize) -> P4Result<()> {
        if index >= self.volumes.len() {
            return Err(P4Error::Storage("bad volume index".into()));
        }
        self.active_volume = index;
        self.cwd.clear();
        Ok(())
    }

    pub fn list(&self) -> P4Result<Vec<FileEntry>> {
        self.volume()?.list(&self.cwd)
    }

    pub fn enter(&mut self, name: &str) -> P4Result<()> {
        let is_dir = self
            .list()?
            .iter()
            .any(|e| e.name == name && e.kind == EntryKind::Dir);
        if !is_dir {
            return Err(P4Error::Storage(format!("not a directory: {name}")));
        }
        self.cwd = if self.cwd.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.cwd, name)
        };
        Ok(())
    }

    pub fn up(&mut self) -> bool {
        match self.cwd.rfind('/') {
            Some(i) => {
                self.cwd.truncate(i);
                true
            }
            None if !self.cwd.is_empty() => {
                self.cwd.clear();
                true
            }
            _ => false,
        }
    }

    pub fn path_of(&self, name: &str) -> String {
        if self.cwd.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.cwd, name)
        }
    }

    /// Delete with explicit confirmation (PRD §15).
    pub fn delete(&mut self, name: &str, confirmed: bool) -> P4Result<()> {
        if !confirmed {
            return Err(P4Error::Security("delete requires confirmation".into()));
        }
        let path = self.path_of(name);
        self.volume_mut()?.delete(&path)
    }

    pub fn rename(&mut self, from: &str, to: &str) -> P4Result<()> {
        let (f, t) = (self.path_of(from), self.path_of(to));
        self.volume_mut()?.rename(&f, &t)
    }

    pub fn mkdir(&mut self, name: &str) -> P4Result<()> {
        let path = self.path_of(name);
        self.volume_mut()?.mkdir(&path)
    }

    pub fn copy(&mut self, from: &str, to: &str) -> P4Result<()> {
        let (f, t) = (self.path_of(from), self.path_of(to));
        self.volume_mut()?.copy(&f, &t)
    }

    pub fn move_file(&mut self, from: &str, to: &str) -> P4Result<()> {
        let (f, t) = (self.path_of(from), self.path_of(to));
        self.volume_mut()?.move_path(&f, &t)
    }

    pub fn search(&self, needle: &str, limit: usize) -> P4Result<Vec<String>> {
        self.volume()?.search(&self.cwd, needle, limit)
    }
}

impl App for FilesApp {
    fn id(&self) -> &'static str {
        "files"
    }
    fn display_name(&self) -> &'static str {
        "Files"
    }
    fn launch(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn suspend(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn close(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn update(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4_storage::HostStorage;

    fn setup(tag: &str) -> FilesApp {
        let base = std::env::temp_dir().join(format!("p4-files-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let sd = HostStorage::new(base.join("sd"), "SD").unwrap();
        let internal = HostStorage::new(base.join("internal"), "Internal").unwrap();
        let mut app = FilesApp::new(vec![Box::new(sd), Box::new(internal)]);
        app.mkdir("books").unwrap();
        app.select_volume(0).unwrap();
        app.mkdir("documents").unwrap();
        app.select_volume(0).unwrap();
        app
    }

    #[test]
    fn browse_navigate() {
        let mut app = setup("browse");
        app.enter("books").unwrap();
        assert_eq!(app.cwd, "books");
        assert!(app.up());
        assert_eq!(app.cwd, "");
        assert!(app.enter("books").is_ok());
        assert!(app.enter("nope").is_err());
    }

    #[test]
    fn delete_requires_confirmation() {
        let mut app = setup("delete");
        app.select_volume(0).unwrap();
        app.mkdir("temp").unwrap();
        assert!(matches!(
            app.delete("temp", false),
            Err(P4Error::Security(_))
        ));
        app.delete("temp", true).unwrap();
        assert!(!app.list().unwrap().iter().any(|e| e.name == "temp"));
    }

    #[test]
    fn rename_copy_move_search() {
        let mut app = setup("rename");
        app.select_volume(0).unwrap();
        app.rename("books", "library").unwrap();
        app.mkdir("docs").unwrap();
        app.copy("library", "docs/library-backup").unwrap();
        app.move_file("library", "docs/library-main").unwrap();
        let found = app.search("library", 10).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.contains(&"docs/library-main".to_string()));
    }

    #[test]
    fn volumes_switch_and_isolate() {
        let mut app = setup("volumes");
        app.select_volume(1).unwrap();
        assert!(app.list().unwrap().is_empty());
        app.select_volume(9).unwrap_err();
    }
}
