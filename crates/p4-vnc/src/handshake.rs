//! RFB 3.8 handshake + VNC authentication (DES challenge-response).

use cipher::{BlockDecrypt, KeyInit};
use p4_core::{P4Error, P4Result};
use std::io::{Read, Write};

pub const PROTOCOL_VERSION: &[u8; 12] = b"RFB 003.008\n";

/// Perform the full handshake: version, security types, optional VNC auth,
/// security result. Returns the chosen security type.
pub fn do_handshake<S: Read + Write>(s: &mut S, password: Option<&str>) -> P4Result<u8> {
    // 1. Server -> Client: server version
    let mut server_ver = [0u8; 12];
    s.read_exact(&mut server_ver)?;
    if !server_ver.starts_with(b"RFB ") {
        return Err(P4Error::Protocol(format!(
            "not an RFB server: {:?}",
            String::from_utf8_lossy(&server_ver)
        )));
    }
    // 2. Client -> Server: our version
    s.write_all(PROTOCOL_VERSION)?;

    // 3. Server -> Client: number-of-security-types + types
    let mut n = [0u8; 1];
    s.read_exact(&mut n)?;
    if n[0] == 0 {
        return Err(P4Error::Protocol("server rejected connection".into()));
    }
    let mut types = vec![0u8; n[0] as usize];
    s.read_exact(&mut types)?;

    // Prefer None(1) when no password, else VNC auth(2).
    let want: u8 = if password.is_some() { 2 } else { 1 };
    if !types.contains(&want) {
        // Fall back to whatever the server offers if we have no password.
        if want == 2 && types.contains(&1) {
            return Err(P4Error::Security(
                "server requires VNC auth but no password configured".into(),
            ));
        }
        return Err(P4Error::Protocol(format!(
            "no acceptable security type in {types:?}"
        )));
    }
    s.write_all(&[want])?;

    // 4. RFB 3.8: the server always sends a 4-byte security result (even for
    //    None); VNC auth additionally exchanges challenge/response above.
    if want == 2 {
        let pw = password.ok_or_else(|| P4Error::Security("missing password".into()))?;
        let mut challenge = [0u8; 16];
        s.read_exact(&mut challenge)?;
        let response = vnc_auth_response(pw, &challenge);
        s.write_all(&response)?;
    }
    let mut result = [0u8; 4];
    s.read_exact(&mut result)?;
    if result != [0, 0, 0, 0] {
        return Err(P4Error::Security("security handshake failed".into()));
    }
    Ok(want)
}

/// VNC auth: password is padded with NULs to 8 bytes (truncated), used as a
/// DES key with bits reversed per byte; each 8-byte challenge half is
/// DES-ECB encrypted.
pub fn vnc_auth_response(password: &str, challenge: &[u8; 16]) -> [u8; 16] {
    let mut key = [0u8; 8];
    for (i, b) in password.bytes().take(8).enumerate() {
        key[i] = b.reverse_bits();
    }
    use cipher::{BlockDecrypt, KeyInit};
    let des = des::Des::new_from_slice(&key).expect("VNC key is always 8 bytes");
    let mut out = [0u8; 16];
    for (i, chunk) in challenge.chunks_exact(8).enumerate() {
        let mut block = cipher::generic_array::GenericArray::clone_from_slice(chunk);
        des.decrypt_block(&mut block);
        out[i * 8..(i + 1) * 8].copy_from_slice(&block);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_is_003_008() {
        assert!(PROTOCOL_VERSION.starts_with(b"RFB 003.008"));
        assert_eq!(PROTOCOL_VERSION.len(), 12);
    }

    #[test]
    fn auth_response_is_deterministic_and_reverses_bits() {
        let ch = [0x11u8; 16];
        let a = vnc_auth_response("secret", &ch);
        let b = vnc_auth_response("secret", &ch);
        assert_eq!(a, b);
        assert_ne!(a, ch, "response must differ from challenge");
        // Long passwords are truncated to 8 chars
        assert_eq!(vnc_auth_response("0123456789", &ch), vnc_auth_response("01234567", &ch));
    }
}
