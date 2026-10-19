//! End-to-end integration tests across crates: full VNC session against a
//! mock RFB server, MVP workflow smoke (PRD §34), and EPUB reading offline.

use p4_reader::{BookFormat, Library};
use p4_settings::{ProfileStore, Settings, StaticKeyProvider, VncProfile, VncSettings};
use p4_storage::HostStorage;
use p4_storage::Storage as _;
use p4_vnc::framebuffer::{Framebuffer, PixelFormat};
use p4_vnc::{QualityMode, VncClient};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Mock RFB 3.8 server: VNC auth, sends one 2x1 raw update.
fn mock_rfb_server(port: u16, password: &'static str) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        let (mut sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        // version
        sock.write_all(b"RFB 003.008\n").unwrap();
        let mut ver = [0u8; 12];
        sock.read_exact(&mut ver).unwrap();
        // security types: VNC auth only
        sock.write_all(&[1u8, 2]).unwrap();
        let mut sel = [0u8; 1];
        sock.read_exact(&mut sel).unwrap();
        assert_eq!(sel[0], 2);
        // challenge
        sock.write_all(&[0x42u8; 16]).unwrap();
        let mut resp = [0u8; 16];
        sock.read_exact(&mut resp).unwrap();
        // result ok
        sock.write_all(&[0, 0, 0, 0]).unwrap();
        // SetPixelFormat + ClientInit
        let mut init = [0u8; 20];
        sock.read_exact(&mut init).unwrap();
        sock.read_exact(&mut [0u8; 1]).unwrap();
        // ServerInit
        let mut si = Vec::new();
        si.extend_from_slice(&800u16.to_be_bytes());
        si.extend_from_slice(&480u16.to_be_bytes());
        si.extend_from_slice(&[16u8, 8, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        si.extend_from_slice(&4u32.to_be_bytes());
        si.extend_from_slice(b"test");
        sock.write_all(&si).unwrap();
        // SetEncodings
        let mut hdr = [0u8; 4];
        sock.read_exact(&mut hdr).unwrap();
        let n = u16::from_be_bytes([hdr[2], hdr[3]]);
        let mut skip = vec![0u8; n as usize * 4];
        sock.read_exact(&mut skip).unwrap();
        // FB update request
        let mut req = [0u8; 10];
        sock.read_exact(&mut req).unwrap();
        // update: 2 rects (raw 2x1, copyrect 1x1)
        let mut up = vec![0u8, 0];
        up.extend_from_slice(&2u16.to_be_bytes());
        // rect1 raw: x, y, w, h (u16 each) + encoding (i32) + 2 RGB565 pixels
        up.extend_from_slice(&0u16.to_be_bytes());
        up.extend_from_slice(&0u16.to_be_bytes());
        up.extend_from_slice(&2u16.to_be_bytes());
        up.extend_from_slice(&1u16.to_be_bytes());
        up.extend_from_slice(&0i32.to_be_bytes());
        up.extend_from_slice(&[0xF8, 0x1F, 0x07, 0xE0]);
        // rect2 copyrect: x, y, w, h + encoding 1 + src x,y
        up.extend_from_slice(&1u16.to_be_bytes());
        up.extend_from_slice(&0u16.to_be_bytes());
        up.extend_from_slice(&1u16.to_be_bytes());
        up.extend_from_slice(&1u16.to_be_bytes());
        up.extend_from_slice(&1i32.to_be_bytes());
        up.extend_from_slice(&0u16.to_be_bytes());
        up.extend_from_slice(&0u16.to_be_bytes());
        sock.write_all(&up).unwrap();
        sock.flush().unwrap();
        // Keep the link open so the client can send input events.
        std::thread::sleep(Duration::from_millis(300));
    })
}

#[test]
fn full_vnc_session_with_auth() {
    let server = mock_rfb_server(15910, "pw");
    std::thread::sleep(Duration::from_millis(100));

    let stream = TcpStream::connect(("127.0.0.1", 15910)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut client = VncClient::new(stream, QualityMode::Balanced);
    let (w, h) = client.connect(Some("pw")).unwrap();
    assert_eq!((w, h), (800, 480));

    let mut fb = Framebuffer::new(800, 480, PixelFormat::RGB565);
    client.request_update(false).unwrap();
    let dirty = client.read_update(&mut fb).unwrap();
    assert_eq!(dirty.len(), 2);
    // raw rect pixels
    assert_eq!(fb.pixel(0, 0), 0xF81F);
    // copyrect copied fb(0,0) -> (1,0), overwriting the raw pixel there
    assert_eq!(fb.pixel(1, 0), 0xF81F);

    client.send_pointer(100, 100, 1).unwrap();
    client.send_key(b'x' as u32, true).unwrap();
    client.send_key(b'x' as u32, false).unwrap();
    server.join().unwrap();
}

#[test]
fn vnc_auth_rejected_stops_connect() {
    // Server that always fails auth.
    let listener = TcpListener::bind(("127.0.0.1", 15911)).unwrap();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        sock.write_all(b"RFB 003.008\n").unwrap();
        let mut v = [0u8; 12];
        sock.read_exact(&mut v).unwrap();
        sock.write_all(&[1u8, 2]).unwrap();
        let mut sel = [0u8; 1];
        sock.read_exact(&mut sel).unwrap();
        sock.write_all(&[0u8; 16]).unwrap();
        let mut resp = [0u8; 16];
        sock.read_exact(&mut resp).unwrap();
        sock.write_all(&[0, 0, 0, 1]).unwrap(); // failure
    });
    std::thread::sleep(Duration::from_millis(100));
    let stream = TcpStream::connect(("127.0.0.1", 15911)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut client = VncClient::new(stream, QualityMode::Balanced);
    assert!(client.connect(Some("wrong")).is_err());
    handle.join().unwrap();
}

/// PRD §34 MVP workflow smoke: settings -> profiles -> browse books -> read.
#[test]
fn mvp_workflow_smoke() {
    let base = std::env::temp_dir().join(format!("p4-mvp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mut sd = HostStorage::new(base.join("sd"), "SD").unwrap();

    // 1. Persisted settings
    let mut settings = Settings::default_device();
    settings.vnc.quality = p4_settings::VncQuality::Balanced;
    let toml_text = settings.to_toml().unwrap();
    let reloaded = Settings::from_toml(&toml_text).unwrap();
    assert_eq!(reloaded.device_name, "p4-client");

    // 2. VNC profiles with protected password
    let keys = StaticKeyProvider([9u8; 32]);
    let mut store = ProfileStore::new();
    store.upsert(VncProfile {
        name: "work".into(),
        hostname: "192.168.1.20".into(),
        port: 5900,
        username: Some("dev".into()),
        password_blob: None,
        scaling_mode: "fit".into(),
        color_depth: 16,
    });
    store.set_password("work", "s3cret", &keys).unwrap();
    let json = store.to_json().unwrap();
    assert!(!json.contains("s3cret"));
    let restored = ProfileStore::from_json(&json).unwrap();
    assert_eq!(restored.password("work", &keys).unwrap(), b"s3cret");

    // 3. Copy a book and read it offline
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts: zip::write::SimpleFileOptions = Default::default();
        w.start_file("mimetype", opts).unwrap();
        w.write_all(b"application/epub+zip").unwrap();
        w.start_file("META-INF/container.xml", opts).unwrap();
        w.write_all(
            br#"<container><rootfiles><rootfile full-path="book.opf"/></rootfiles></container>"#,
        )
        .unwrap();
        w.start_file("book.opf", opts).unwrap();
        w.write_all(br#"<package><manifest><item id="c1" href="c1.xhtml"/></manifest><spine><itemref idref="c1"/></spine></package>"#).unwrap();
        w.start_file("c1.xhtml", opts).unwrap();
        w.write_all(b"<html><body><p>The Pragmatic Programmer body text.</p></body></html>")
            .unwrap();
        w.finish().unwrap();
    }
    let epub = buf.into_inner();
    sd.write("books/pragmatic.epub", &epub).unwrap();

    // offline: no network involved from here on
    let data = sd.read("books/pragmatic.epub").unwrap();
    let book = p4_reader::parse("epub:pragmatic", "pragmatic", BookFormat::Epub, &data).unwrap();
    assert_eq!(book.chapters.len(), 1);
    assert!(book.chapters[0].text.contains("Pragmatic Programmer"));

    // 4. Reader library tracks position
    let mut lib = Library::default();
    lib.entry(&book.id, &book.title, 42);
    lib.add_bookmark(&book.id, 0, 0, "start");
    assert_eq!(lib.recent()[0].book_id, "epub:pragmatic");
    // 5. Storage survives roundtrip
    assert!(sd.search("books", "pragmatic", 5).unwrap().len() == 1);
}
