//! Lowercase hex for identities in workbench files and `--ids` output.

use core::fmt::Write as _;

/// Encodes bytes as lowercase hex.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Decodes lowercase hex; uppercase and odd lengths are refused.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let raw = text.as_bytes();
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(raw.len() / 2);
    for pair in raw.chunks_exact(2) {
        out.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
    }
    Some(out)
}

/// Decodes exactly 32 bytes of lowercase hex.
#[must_use]
pub fn decode32(text: &str) -> Option<[u8; 32]> {
    let bytes = decode(text)?;
    bytes.try_into().ok()
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// The first eight hex digits of an identity.
#[must_use]
pub fn short(bytes: &[u8; 32]) -> String {
    encode(&bytes[..4])
}
