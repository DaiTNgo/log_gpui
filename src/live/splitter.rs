/// Cuts a stream of bytes into lines and keeps the unfinished last line until
/// its newline arrives, however the stream is cut (a UTF-8 character
/// included).
#[derive(Debug, Default, Clone)]
pub struct LineSplitter {
    pending: Vec<u8>,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// The lines completed by `bytes`, without their line ends.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut rest = bytes;
        while let Some(pos) = rest.iter().position(|b| *b == b'\n') {
            self.pending.extend_from_slice(&rest[..pos]);
            lines.push(decode_line(&std::mem::take(&mut self.pending)));
            rest = &rest[pos + 1..];
        }
        self.pending.extend_from_slice(rest);
        lines
    }

    /// Bytes of the unfinished line.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

pub(crate) fn decode_line(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}
