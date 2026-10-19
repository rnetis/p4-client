//! Ebook reader library (PRD §13, §14).
//!
//! Formats: TXT, Markdown, EPUB (MVP). Streaming-friendly: EPUB chapters are
//! parsed from the zip one at a time, never the whole book into memory.

pub mod epub;
pub mod library;

pub use library::{BookId, Bookmark, Library, ReadingPosition};

use p4_core::P4Result;

/// A parsed chapter ready for pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    pub index: usize,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub chapters: Vec<Chapter>,
}

impl Book {
    pub fn toc(&self) -> Vec<TocEntry> {
        self.chapters
            .iter()
            .enumerate()
            .map(|(i, c)| TocEntry {
                index: i,
                title: c.title.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Txt,
    Markdown,
    Epub,
}

impl BookFormat {
    pub fn from_path(path: &str) -> Option<BookFormat> {
        let lower = path.to_lowercase();
        if lower.ends_with(".epub") {
            Some(BookFormat::Epub)
        } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
            Some(BookFormat::Markdown)
        } else if lower.ends_with(".txt") {
            Some(BookFormat::Txt)
        } else {
            None
        }
    }
}

/// Parse a book from its raw bytes. Input size is validated first (PRD §26,
/// §28: never load complete ebooks unnecessarily — EPUB parses lazily through
/// `epub::open`, this path is for fully-in-memory formats).
pub fn parse(id: &str, title: &str, format: BookFormat, data: &[u8]) -> P4Result<Book> {
    let text = match format {
        BookFormat::Txt => String::from_utf8_lossy(data).into_owned(),
        BookFormat::Markdown => markdown_to_text(&String::from_utf8_lossy(data)),
        BookFormat::Epub => {
            let book = epub::open(id, title, data)?;
            return Ok(book);
        }
    };
    Ok(Book {
        id: id.to_string(),
        title: title.to_string(),
        author: None,
        chapters: vec![Chapter {
            title: title.to_string(),
            text,
        }],
    })
}

/// Convert light Markdown to plain text for rendering (MVP: strip headings
/// markers, emphasis, links, code fences; keep line structure).
pub fn markdown_to_text(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    for line in md.lines() {
        let l = line.trim();
        if l.starts_with("```") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut l = l.to_string();
        // headings
        while l.starts_with('#') {
            l.remove(0);
        }
        let l = l.trim().to_string();
        if l.is_empty() {
            out.push('\n');
            continue;
        }
        // images ![alt](url) -> drop, links [text](url) -> text
        let mut s = l;
        while let Some(start) = s.find("![") {
            if let Some(end) = s[start..].find(')') {
                s.replace_range(start..start + end + 1, "");
            } else {
                break;
            }
        }
        while let Some(start) = s.find('[') {
            if let Some(mid) = s[start..].find("](") {
                if let Some(end) = s[mid..].find(')') {
                    let text = s[start + 1..mid].to_string();
                    s.replace_range(start..mid + end + 1, &text);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        // bold/italic/inline code markers
        let s = s.replace("**", "").replace('*', "").replace('`', "");
        out.push_str(&s);
        out.push('\n');
    }
    out
}

/// Pagination: split chapter text into pages of `chars_per_page` characters,
/// breaking at word boundaries when possible.
pub fn paginate(text: &str, chars_per_page: usize) -> Vec<String> {
    assert!(chars_per_page > 0);
    let mut pages = Vec::new();
    let mut current = String::new();
    for word in text.split_inclusive(char::is_whitespace) {
        if current.len() + word.len() > chars_per_page && !current.is_empty() {
            pages.push(current.clone());
            current.clear();
        }
        current.push_str(word);
    }
    if !current.trim().is_empty() || pages.is_empty() {
        pages.push(current);
    }
    pages
}

/// Reading progress percent (PRD §13: progress indicator).
pub fn progress_percent(
    chapter: usize,
    page: usize,
    total_chapters: usize,
    total_pages: usize,
) -> u8 {
    if total_chapters == 0 || total_pages == 0 {
        return 0;
    }
    let pos = chapter as f64 * 100.0 + (page as f64 / total_pages as f64) * 100.0;
    ((pos / total_chapters as f64).min(100.0)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txt_book_single_chapter() {
        let b = parse("id1", "My Notes", BookFormat::Txt, b"hello world").unwrap();
        assert_eq!(b.chapters.len(), 1);
        assert_eq!(b.chapters[0].text, "hello world");
    }

    #[test]
    fn markdown_strips_syntax() {
        let md = "# Title\n\nSome **bold** and *ital* and `code`.\n\n[link](http://x.y)\n";
        let t = markdown_to_text(md);
        assert!(t.contains("Some bold and ital and code."));
        assert!(t.contains("link"));
        assert!(!t.contains("http://x.y"));
        assert!(!t.contains('#'));
    }

    #[test]
    fn pagination_respects_limit() {
        let text = "word ".repeat(100); // 500 chars
        let pages = paginate(&text, 100);
        assert!(pages.iter().all(|p| p.len() <= 100));
        assert!(pages.len() >= 4);
    }

    #[test]
    fn format_from_path() {
        assert_eq!(BookFormat::from_path("a/b.epub"), Some(BookFormat::Epub));
        assert_eq!(BookFormat::from_path("x.MD"), Some(BookFormat::Markdown));
        assert_eq!(BookFormat::from_path("y.txt"), Some(BookFormat::Txt));
        assert_eq!(BookFormat::from_path("z.pdf"), None);
    }

    #[test]
    fn progress_bounds() {
        assert_eq!(progress_percent(0, 0, 10, 10), 0);
        assert_eq!(progress_percent(5, 5, 10, 10), 55);
        assert_eq!(progress_percent(9, 9, 10, 10), 99);
        assert_eq!(progress_percent(0, 0, 0, 0), 0);
    }
}
