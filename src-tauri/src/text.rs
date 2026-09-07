//! Text decoding helpers for byte streams that arrive in arbitrary chunks.

/// Incremental UTF-8 decoder.
///
/// SSH packets and PTY reads split wherever the transport decides, so a
/// multi-byte character routinely straddles two chunks. Decoding each chunk
/// independently with `String::from_utf8_lossy` turns that character into
/// U+FFFD permanently — the bytes are already gone by the time the rest of the
/// character arrives. When the mangled bytes land inside an escape sequence
/// the terminal parser can also stall waiting for a terminator that never
/// comes, which reads as output lagging a keystroke behind.
///
/// This holds an incomplete trailing character back and prepends it to the
/// next chunk. The held tail is at most 3 bytes, since that is the longest
/// prefix of a UTF-8 character that can be incomplete.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode `chunk`, carrying any incomplete trailing character forward to
    /// the next call. Returns the text that is complete now, which is empty
    /// when the chunk was nothing but the start of a character.
    pub fn push(&mut self, chunk: &[u8]) -> String {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(chunk);

        match std::str::from_utf8(&buf) {
            Ok(s) => s.to_string(),
            Err(e) => {
                let good = e.valid_up_to();
                // error_len() == None means the bytes from `good` onward are a
                // valid prefix of a character that simply has not finished
                // arriving — hold them. Some(_) means they are genuinely
                // invalid, so emit lossily now rather than holding garbage
                // that will never become valid.
                if e.error_len().is_none() {
                    let out = String::from_utf8_lossy(&buf[..good]).into_owned();
                    self.pending = buf[good..].to_vec();
                    out
                } else {
                    String::from_utf8_lossy(&buf).into_owned()
                }
            }
        }
    }

    /// Emit any held bytes because the stream ended mid-character. They cannot
    /// be completed now, so they decode lossily.
    pub fn flush(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let out = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_passes_through() {
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(b"hello"), "hello");
        assert!(s.flush().is_none());
    }

    /// The regression: a character split across two chunks must survive.
    #[test]
    fn character_split_across_chunks_is_not_corrupted() {
        // U+00E9 (e-acute) is 0xC3 0xA9.
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(&[b'a', 0xC3]), "a", "incomplete tail must be held");
        assert_eq!(s.push(&[0xA9, b'b']), "\u{e9}b", "held byte must be rejoined");
        assert!(s.flush().is_none());

        // Compare against the old per-chunk behavior, which loses it.
        assert_eq!(String::from_utf8_lossy(&[b'a', 0xC3]), "a\u{fffd}");
    }

    #[test]
    fn three_byte_character_split_at_every_offset() {
        // U+2500 BOX DRAWINGS LIGHT HORIZONTAL is 0xE2 0x94 0x80 — the kind of
        // character an OpenWrt banner is full of.
        let ch = [0xE2u8, 0x94, 0x80];
        for split in 1..ch.len() {
            let mut s = Utf8Stream::new();
            let mut out = s.push(&ch[..split]);
            out.push_str(&s.push(&ch[split..]));
            assert_eq!(out, "\u{2500}", "split at {} lost the character", split);
        }
    }

    #[test]
    fn four_byte_character_split() {
        // U+1F600, 0xF0 0x9F 0x98 0x80.
        let ch = [0xF0u8, 0x9F, 0x98, 0x80];
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(&ch[..2]), "");
        assert_eq!(s.push(&ch[2..]), "\u{1f600}");
    }

    #[test]
    fn genuinely_invalid_bytes_are_not_held_forever() {
        let mut s = Utf8Stream::new();
        // 0xFF can never start a valid character, so it must be emitted now.
        assert_eq!(s.push(&[0xFF]), "\u{fffd}");
        assert!(s.flush().is_none(), "nothing should be held");
    }

    #[test]
    fn flush_emits_a_dangling_partial_character() {
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(&[0xE2, 0x94]), "");
        assert_eq!(s.flush(), Some("\u{fffd}".to_string()));
        assert!(s.flush().is_none());
    }

    #[test]
    fn held_tail_never_exceeds_three_bytes() {
        let mut s = Utf8Stream::new();
        s.push(&[0xF0, 0x9F, 0x98]);
        assert!(s.pending.len() <= 3);
    }
}
