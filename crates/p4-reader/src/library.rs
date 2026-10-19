//! Reading library: saved books, bookmarks and resume-position metadata
//! (PRD §13, §14). Persisted as JSON under `/.app/`.

use p4_core::P4Result;
use serde::{Deserialize, Serialize};

pub type BookId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ReadingPosition {
    pub current_chapter: usize,
    pub current_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub chapter: usize,
    pub page: usize,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibraryEntry {
    pub book_id: BookId,
    pub title: String,
    pub last_opened: u64,
    pub position: ReadingPosition,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Library {
    pub entries: Vec<LibraryEntry>,
}

impl Library {
    pub fn entry(&mut self, book_id: &str, title: &str, now: u64) -> &mut LibraryEntry {
        if let Some(pos) = self.entries.iter().position(|e| e.book_id == book_id) {
            self.entries[pos].last_opened = now;
            return &mut self.entries[pos];
        }
        self.entries.push(LibraryEntry {
            book_id: book_id.to_string(),
            title: title.to_string(),
            last_opened: now,
            position: ReadingPosition::default(),
            bookmarks: Vec::new(),
        });
        self.entries.last_mut().unwrap()
    }

    pub fn find(&self, book_id: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|e| e.book_id == book_id)
    }

    /// Most recently opened first (resume list).
    pub fn recent(&self) -> Vec<&LibraryEntry> {
        let mut v: Vec<_> = self.entries.iter().collect();
        v.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
        v
    }

    pub fn add_bookmark(&mut self, book_id: &str, chapter: usize, page: usize, label: &str) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.book_id == book_id) {
            if e.bookmarks.iter().any(|b| b.chapter == chapter && b.page == page) {
                return false;
            }
            e.bookmarks.push(Bookmark {
                chapter,
                page,
                label: label.to_string(),
            });
            return true;
        }
        false
    }

    pub fn remove_bookmark(&mut self, book_id: &str, index: usize) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.book_id == book_id) {
            if index < e.bookmarks.len() {
                e.bookmarks.remove(index);
                return true;
            }
        }
        false
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("library serializes")
    }

    pub fn from_json(s: &str) -> P4Result<Self> {
        serde_json::from_str(s).map_err(|e| p4_core::P4Error::Parse(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_creates_and_resumes() {
        let mut lib = Library::default();
        lib.entry("b1", "Book One", 100).position = ReadingPosition {
            current_chapter: 2,
            current_page: 7,
        };
        let e = lib.entry("b1", "Book One", 200);
        assert_eq!(e.position.current_chapter, 2);
        assert_eq!(e.position.current_page, 7);
        assert_eq!(e.last_opened, 200);
    }

    #[test]
    fn bookmarks_dedupe() {
        let mut lib = Library::default();
        lib.entry("b1", "T", 1);
        assert!(lib.add_bookmark("b1", 0, 3, "here"));
        assert!(!lib.add_bookmark("b1", 0, 3, "dup"));
        assert!(lib.remove_bookmark("b1", 0));
        assert!(!lib.remove_bookmark("b1", 5));
    }

    #[test]
    fn recent_sorted() {
        let mut lib = Library::default();
        lib.entry("a", "A", 10);
        lib.entry("b", "B", 30);
        lib.entry("c", "C", 20);
        let ids: Vec<_> = lib.recent().into_iter().map(|e| e.book_id.clone()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn json_roundtrip() {
        let mut lib = Library::default();
        lib.entry("b1", "T", 5);
        lib.add_bookmark("b1", 1, 2, "mark");
        let back = Library::from_json(&lib.to_json()).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].bookmarks.len(), 1);
    }
}
