//! Client -> server message builders (RFB 3.8).

/// SetEncodings (msg type 2).
pub fn set_encodings(encodings: &[i32]) -> Vec<u8> {
    let mut m = Vec::with_capacity(4 + encodings.len() * 4);
    m.push(2);
    m.push(0);
    m.extend_from_slice(&(encodings.len() as u16).to_be_bytes());
    for e in encodings {
        m.extend_from_slice(&e.to_be_bytes());
    }
    m
}

/// FramebufferUpdateRequest (msg type 3).
pub fn framebuffer_update_request(x: u16, y: u16, w: u16, h: u16, incremental: bool) -> Vec<u8> {
    let mut m = Vec::with_capacity(10);
    m.push(3);
    m.push(u8::from(incremental));
    m.extend_from_slice(&x.to_be_bytes());
    m.extend_from_slice(&y.to_be_bytes());
    m.extend_from_slice(&w.to_be_bytes());
    m.extend_from_slice(&h.to_be_bytes());
    m
}

/// PointerEvent (msg type 4). Button mask bit0=left, bit1=middle, bit2=right.
pub fn pointer_event(x: u16, y: u16, mask: u8) -> Vec<u8> {
    let mut m = Vec::with_capacity(6);
    m.push(4);
    m.push(mask);
    m.extend_from_slice(&x.to_be_bytes());
    m.extend_from_slice(&y.to_be_bytes());
    m
}

/// KeyEvent (msg type 5). Keysym: ASCII maps directly; special keys use X11
/// keysyms (0xFF1B = Esc, 0xFF0D = Return, 0xFF08 = BackSpace ...).
pub fn key_event(keysym: u32, down: bool) -> Vec<u8> {
    let mut m = Vec::with_capacity(8);
    m.push(5);
    m.push(u8::from(down));
    m.push(0);
    m.push(0);
    m.extend_from_slice(&keysym.to_be_bytes());
    m
}

/// ClientCutText (msg type 6).
pub fn client_cut_text(text: &str) -> Vec<u8> {
    let mut m = Vec::with_capacity(8 + text.len());
    m.push(6);
    m.extend_from_slice(&[0, 0, 0]);
    m.extend_from_slice(&(text.len() as u32).to_be_bytes());
    m.extend_from_slice(text.as_bytes());
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_request_wire_format() {
        let m = framebuffer_update_request(0, 0, 800, 480, true);
        assert_eq!(m, vec![3, 1, 0, 0, 0, 0, 0x03, 0x20, 0x01, 0xE0]);
    }

    #[test]
    fn pointer_event_wire_format() {
        let m = pointer_event(100, 200, 1);
        assert_eq!(m, vec![4, 1, 0, 100, 0, 200]);
    }

    #[test]
    fn key_event_down_and_up() {
        assert_eq!(key_event(b'a' as u32, true)[0], 5);
        assert_eq!(key_event(b'a' as u32, true)[1], 1);
        assert_eq!(key_event(b'a' as u32, false)[1], 0);
        assert_eq!(&key_event(0xFF1B, true)[4..], &0xFF1Bu32.to_be_bytes());
    }

    #[test]
    fn set_encodings_counts() {
        let m = set_encodings(&[0, 5]);
        assert_eq!(m[0], 2);
        assert_eq!(&m[2..4], &2u16.to_be_bytes());
        assert_eq!(m.len(), 4 + 8);
    }
}
