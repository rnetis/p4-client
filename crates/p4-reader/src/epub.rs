//! Minimal EPUB (zip of XHTML) parser. Streams entries from the zip rather
//! than loading the whole book (PRD §28).

use crate::{Book, Chapter};
use p4_core::{P4Error, P4Result};
use std::io::Read;

/// Open an EPUB from raw bytes. Chapter order comes from the OPF spine.
pub fn open(id: &str, title: &str, data: &[u8]) -> P4Result<Book> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| P4Error::Parse(format!("bad epub zip: {e}")))?;

    // 1. META-INF/container.xml -> rootfile path
    let container = read_entry(&mut archive, "META-INF/container.xml")?;
    let opf_path = extract_attr(&container, "full-path")
        .ok_or_else(|| P4Error::Parse("container.xml missing rootfile full-path".into()))?;

    // 2. OPF -> spine itemrefs + manifest id->href map + metadata
    let opf = read_entry(&mut archive, &opf_path)?;
    let opf_dir = match opf_path.rfind('/') {
        Some(i) => &opf_path[..=i],
        None => "",
    };
    let mut hrefs = Vec::new();
    for (id_attr, href) in manifest_items(&opf) {
        if spine_has(&opf, &id_attr) {
            hrefs.push(href);
        }
    }
    if hrefs.is_empty() {
        // Fall back to manifest order
        hrefs = manifest_items(&opf).into_iter().map(|(_, h)| h).collect();
    }
    let author = extract_attr(&opf, "creator").or_else(|| {
        // <dc:creator> is an element, not an attribute.
        let start = opf.find("<dc:creator")?;
        let text_start = opf[start..].find('>')? + start + 1;
        let end = opf[text_start..].find("</dc:creator>")?;
        let v = opf[text_start..text_start + end].trim();
        if v.is_empty() {
            None
        } else {
            Some(html_unescape(v).trim().to_string())
        }
    });

    // 3. Read each spine doc, strip tags
    let mut chapters = Vec::new();
    for href in hrefs {
        let path = format!("{opf_dir}{href}");
        let Ok(html) = read_entry(&mut archive, &path) else {
            continue;
        };
        let ch_title = extract_attr(&html, "title")
            .or_else(|| {
                // <title> is an element inside <head>.
                let start = html.find("<title")?;
                let text_start = html[start..].find('>')? + start + 1;
                let end = html[text_start..].find("</title>")?;
                let v = html[text_start..text_start + end].trim();
                if v.is_empty() {
                    None
                } else {
                    Some(html_unescape(v).trim().to_string())
                }
            })
            .unwrap_or_else(|| href.clone());
        let text = strip_tags(&html);
        if !text.trim().is_empty() {
            chapters.push(Chapter {
                title: ch_title,
                text,
            });
        }
    }
    if chapters.is_empty() {
        return Err(P4Error::Parse("epub has no readable chapters".into()));
    }

    Ok(Book {
        id: id.to_string(),
        title: title.to_string(),
        author,
        chapters,
    })
}

fn read_entry(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> P4Result<String> {
    // Normalize: EPUB entries may be resolved relative; try exact then with any prefix.
    let mut file = open_entry(archive, name)
        .map_err(|_| P4Error::Parse(format!("missing epub entry '{name}'")))?;
    let mut buf = Vec::new();
    // Cap a single entry at 8 MiB (PRD §26: validate parsed data).
    file.by_ref()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut buf)
        .map_err(|e| P4Error::Parse(e.to_string()))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn open_entry<'a, 'd>(
    archive: &'a mut zip::ZipArchive<std::io::Cursor<&'d [u8]>>,
    name: &str,
) -> Result<zip::read::ZipFile<'a, std::io::Cursor<&'d [u8]>>, zip::result::ZipError> {
    if archive.by_name(name).is_ok() {
        return archive.by_name(name);
    }
    // Probe each entry name first (borrow dropped), then reopen the match.
    for i in 0..archive.len() {
        let matches = archive
            .by_index(i)
            .map(|f| f.name().ends_with(name))
            .unwrap_or(false);
        if matches {
            return archive.by_index(i);
        }
    }
    archive.by_name(name)
}

/// (id, href) pairs from the OPF manifest.
fn manifest_items(opf: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(start) = opf.find("<manifest") {
        if let Some(end) = opf[start..].find("</manifest>") {
            let section = &opf[start..start + end];
            let mut rest = section;
            while let Some(i) = rest.find("<item ") {
                let tag_end = rest[i..].find("/>").map(|e| i + e).unwrap_or(rest.len());
                let tag = &rest[i..tag_end];
                if let (Some(id), Some(href)) = (extract_attr(tag, "id"), extract_attr(tag, "href"))
                {
                    out.push((id, href));
                }
                rest = &rest[tag_end..];
            }
        }
    }
    out
}

fn spine_has(opf: &str, idref: &str) -> bool {
    match (opf.find("<spine"), opf.find("</spine>")) {
        (Some(s), Some(e)) => opf[s..e].contains(&format!("idref=\"{idref}\"")),
        _ => false,
    }
}

/// Extract `attr="value"` from the first tag containing `attr`.
pub fn extract_attr(doc: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let i = doc.find(&needle)? + needle.len();
    let rest = &doc[i..];
    let end = rest.find('"')?;
    let v = &rest[..end];
    if v.is_empty() {
        None
    } else {
        Some(html_unescape(v))
    }
}

/// Strip XHTML tags and unescape entities to plain text.
pub fn strip_tags(html: &str) -> String {
    let no_script = remove_between(html, "<script", "</script>");
    let no_style = remove_between(&no_script, "<style", "</style>");
    let mut out = String::with_capacity(no_style.len());
    let mut in_tag = false;
    for c in no_style.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push('\n'); // tag boundary == soft break
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let collapsed = html_unescape(&out)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    collapsed
}

fn remove_between(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i]);
        match rest[i..].find(close) {
            Some(j) => rest = &rest[i + j + close.len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

pub fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn build_epub() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts: zip::write::SimpleFileOptions = Default::default();
            w.start_file("mimetype", opts).unwrap();
            w.write_all(b"application/epub+zip").unwrap();
            w.start_file("META-INF/container.xml", opts).unwrap();
            w.write_all(
                br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
            )
            .unwrap();
            w.start_file("OEBPS/content.opf", opts).unwrap();
            w.write_all(
                br#"<?xml version="1.0"?>
                <package><metadata><dc:creator>Jane Doe</dc:creator></metadata>
                <manifest>
                  <item id="c1" href="ch1.xhtml"/>
                  <item id="c2" href="ch2.xhtml"/>
                </manifest>
                <spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#,
            )
            .unwrap();
            w.start_file("OEBPS/ch1.xhtml", opts).unwrap();
            w.write_all(
                br#"<html><head><title>One</title></head><body><p>Hello &amp; welcome.</p></body></html>"#,
            )
            .unwrap();
            w.start_file("OEBPS/ch2.xhtml", opts).unwrap();
            w.write_all(br#"<html><body><p>Second chapter text.</p></body></html>"#)
                .unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn parses_spine_order_and_metadata() {
        let data = build_epub();
        let book = open("b1", "Test Book", &data).unwrap();
        assert_eq!(book.author.as_deref(), Some("Jane Doe"));
        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].title, "One");
        assert!(book.chapters[0].text.contains("Hello & welcome."));
        assert_eq!(book.chapters[1].title, "ch2.xhtml"); // no <title> tag
        assert!(book.chapters[1].text.contains("Second chapter"));
    }

    #[test]
    fn rejects_non_zip() {
        assert!(open("x", "X", b"not a zip").is_err());
    }

    #[test]
    fn strip_tags_unescapes_and_collapses() {
        let t = strip_tags("<p>a &lt;b&gt;</p><p>c</p>");
        assert!(t.contains("a <b>"));
        assert!(t.contains("c"));
        assert!(!t.contains("&lt;"));
        assert!(!t.contains("</p>"));
    }
}
