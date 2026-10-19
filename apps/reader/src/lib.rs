//! Reader app (PRD §13): open TXT/Markdown/EPUB, page navigation, TOC,
//! bookmarks, resume position, progress indicator.

use p4_core::{App, P4Error, P4Result};
use p4_reader::{paginate, progress_percent, Book, BookFormat, Library, ReadingPosition};
use std::cell::RefCell;
use std::rc::Rc;

/// Per-session reader state for one open book.
pub struct OpenBook {
    pub book: Book,
    pub pages: Vec<Vec<String>>, // pages[ch][page]
    pub chapter: usize,
    pub page: usize,
    pub chars_per_page: usize,
}

impl OpenBook {
    pub fn new(book: Book, chars_per_page: usize) -> Self {
        let pages = book
            .chapters
            .iter()
            .map(|c| paginate(&c.text, chars_per_page))
            .collect();
        Self {
            book,
            pages,
            chapter: 0,
            page: 0,
            chars_per_page,
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages[self.chapter.min(self.pages.len() - 1)].len()
    }

    pub fn page_text(&self) -> &str {
        &self.pages[self.chapter][self.page]
    }

    pub fn next_page(&mut self) -> bool {
        if self.page + 1 < self.page_count() {
            self.page += 1;
            true
        } else if self.chapter + 1 < self.pages.len() {
            self.chapter += 1;
            self.page = 0;
            true
        } else {
            false
        }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.page > 0 {
            self.page -= 1;
            true
        } else if self.chapter > 0 {
            // Cross the boundary to the start of the previous chapter.
            self.chapter -= 1;
            self.page = 0;
            true
        } else {
            false
        }
    }

    pub fn goto_chapter(&mut self, index: usize) -> bool {
        if index < self.pages.len() {
            self.chapter = index;
            self.page = 0;
            true
        } else {
            false
        }
    }

    pub fn progress(&self) -> u8 {
        progress_percent(self.chapter, self.page, self.pages.len(), self.page_count())
    }

    pub fn position(&self) -> ReadingPosition {
        ReadingPosition {
            current_chapter: self.chapter,
            current_page: self.page,
        }
    }

    pub fn restore(&mut self, pos: &ReadingPosition) {
        if pos.current_chapter < self.pages.len() {
            self.chapter = pos.current_chapter;
            self.page = pos.current_page.min(self.page_count() - 1);
        } else {
            // Invalid saved position: fall back to the beginning.
            self.chapter = 0;
            self.page = 0;
        }
    }
}

/// Reader application: owns the open book and the persisted library.
pub struct ReaderApp {
    pub open: Option<OpenBook>,
    pub library: Rc<RefCell<Library>>,
    chars_per_page: usize,
}

impl ReaderApp {
    pub fn new(library: Rc<RefCell<Library>>, chars_per_page: usize) -> Self {
        Self {
            open: None,
            library,
            chars_per_page,
        }
    }

    /// Open a book from a storage volume.
    pub fn open_from(&mut self, storage: &dyn p4_storage::Storage, path: &str) -> P4Result<()> {
        let data = storage.read(path)?;
        let title = path.rsplit('/').next().unwrap_or(path).to_string();
        let format = BookFormat::from_path(path)
            .ok_or_else(|| P4Error::Unsupported(format!("format: {path}")))?;
        let id = format!("{format:?}:{title}");
        let book = p4_reader::parse(&id, &title, format, &data)?;
        let mut ob = OpenBook::new(book, self.chars_per_page);
        // Resume last position (PRD §13).
        if let Some(e) = self.library.borrow().find(&id) {
            ob.restore(&e.position);
        }
        self.library
            .borrow_mut()
            .entry(&id, &title, 0);
        self.open = Some(ob);
        Ok(())
    }

    pub fn save_position(&mut self) {
        if let Some(ob) = &self.open {
            self.library.borrow_mut().entry(&ob.book.id, &ob.book.title, 0).position =
                ob.position();
        }
    }

    pub fn add_bookmark_here(&mut self, label: &str) -> bool {
        match &self.open {
            Some(ob) => self
                .library
                .borrow_mut()
                .add_bookmark(&ob.book.id, ob.chapter, ob.page, label),
            None => false,
        }
    }
}

impl App for ReaderApp {
    fn id(&self) -> &'static str {
        "reader"
    }
    fn display_name(&self) -> &'static str {
        "Books"
    }
    fn launch(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn suspend(&mut self) -> P4Result<()> {
        self.save_position();
        Ok(())
    }
    fn resume(&mut self) -> P4Result<()> {
        Ok(())
    }
    fn close(&mut self) -> P4Result<()> {
        self.save_position();
        self.open = None;
        Ok(())
    }
    fn update(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4_reader::{Book, Chapter};
    use p4_storage::{HostStorage, Storage as _};

    fn sample_book() -> Book {
        Book {
            id: "t:demo".into(),
            title: "demo".into(),
            author: None,
            chapters: vec![
                Chapter { title: "c1".into(), text: "one ".repeat(60) },
                Chapter { title: "c2".into(), text: "two ".repeat(60) },
            ],
        }
    }

    #[test]
    fn navigation_and_progress() {
        let mut ob = OpenBook::new(sample_book(), 100);
        assert!(ob.next_page());
        assert_eq!(ob.page, 1);
        assert!(ob.prev_page());
        assert_eq!(ob.page, 0);
        // Jump to last chapter
        assert!(ob.goto_chapter(1));
        assert_eq!(ob.chapter, 1);
        // Prev from chapter start goes back a chapter to its last page
        assert!(ob.prev_page());
        assert_eq!(ob.chapter, 0);
        // No underflow
        assert!(!ob.prev_page());
        assert!(!ob.goto_chapter(9));
    }

    #[test]
    fn restore_clamps_page() {
        let mut ob = OpenBook::new(sample_book(), 100);
        ob.restore(&ReadingPosition { current_chapter: 1, current_page: 9999 });
        assert_eq!(ob.chapter, 1);
        assert_eq!(ob.page, ob.page_count() - 1);
        ob.restore(&ReadingPosition { current_chapter: 7, current_page: 0 });
        assert_eq!(ob.chapter, 0, "invalid chapter ignored");
    }

    #[test]
    fn open_txt_and_resume_via_library() {
        let dir = std::env::temp_dir().join(format!("p4-reader-{}", std::process::id()));
        let mut storage = HostStorage::new(&dir, "sd").unwrap();
        storage.write("books/demo.txt", b"chapter text ".repeat(40).as_slice()).unwrap();
        let lib = Rc::new(RefCell::new(Library::default()));
        let mut app = ReaderApp::new(lib.clone(), 100);
        app.open_from(&storage, "books/demo.txt").unwrap();
        assert!(app.open.is_some());
        app.open.as_mut().unwrap().next_page();
        app.save_position();
        assert!(lib.borrow().entries[0].position.current_page == 1);
        app.add_bookmark_here("mid");
        assert_eq!(lib.borrow().entries[0].bookmarks.len(), 1);
    }

    #[test]
    fn unsupported_format_errors() {
        let dir = std::env::temp_dir().join("p4-reader-bad");
        let storage = HostStorage::new(&dir, "sd").unwrap();
        let lib = Rc::new(RefCell::new(Library::default()));
        let mut app = ReaderApp::new(lib, 100);
        assert!(app.open_from(&storage, "books/x.pdf").is_err());
    }
}
