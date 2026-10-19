//! Compressed Rotated Files: the kind is told by the first bytes of the file,
//! never by its name, and every kind is decoded by a pure Rust crate so no C
//! library has to be built. A decoder reads the compressed bytes through a
//! plain `Read`; `AsyncBridge` makes an SFTP handle one.

use flate2::read::MultiGzDecoder;
use std::io::{self, BufReader, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use tokio::io::{AsyncRead, AsyncReadExt};

/// How many compressed bytes the bridge to SFTP reads ahead.
pub const READ_AHEAD: usize = 1024 * 1024;
/// How many leading bytes tell the kind apart.
pub const MAGIC_BYTES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    Zstd,
    Xz,
    Bzip2,
    /// A compressed format this app does not read (name shown to the user).
    Unsupported(&'static str),
}

impl Compression {
    /// The kind by the first bytes of a file (at least `MAGIC_BYTES` when the
    /// file has that many).
    pub fn detect(head: &[u8]) -> Self {
        if head.starts_with(&[0x1f, 0x8b]) {
            Compression::Gzip
        } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            Compression::Zstd
        } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
            Compression::Xz
        } else if head.starts_with(b"BZh") {
            Compression::Bzip2
        } else if head.starts_with(&[0x04, 0x22, 0x4d, 0x18]) {
            Compression::Unsupported("lz4")
        } else if head.starts_with(&[0x1f, 0x9d]) {
            Compression::Unsupported("compress")
        } else if head.starts_with(b"LZIP") {
            Compression::Unsupported("lzip")
        } else if head.starts_with(&[b'7', b'z', 0xbc, 0xaf, 0x27, 0x1c]) {
            Compression::Unsupported("7z")
        } else {
            Compression::None
        }
    }

    pub fn is_compressed(self) -> bool {
        self != Compression::None
    }

    /// Whether `decoder` can read it (a plain file always can).
    pub fn is_supported(self) -> bool {
        !matches!(self, Compression::Unsupported(_))
    }

    pub fn label(self) -> &'static str {
        match self {
            Compression::None => "plain",
            Compression::Gzip => "gzip",
            Compression::Zstd => "zstd",
            Compression::Xz => "xz",
            Compression::Bzip2 => "bzip2",
            Compression::Unsupported(_) => "compression not supported",
        }
    }
}

/// A reader of the text inside `reader`, by `kind`. A plain file is passed
/// through. Gzip reads files of several members as one.
pub fn decoder<R: Read + Send + 'static>(kind: Compression, reader: R) -> io::Result<Box<dyn Read + Send>> {
    Ok(match kind {
        Compression::None => Box::new(reader),
        Compression::Gzip => Box::new(MultiGzDecoder::new(reader)),
        Compression::Zstd => {
            let dec = ruzstd::decoding::StreamingDecoder::new(reader)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            Box::new(dec)
        }
        Compression::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(reader)),
        Compression::Xz => Box::new(xz_reader(reader)),
        Compression::Unsupported(name) => {
            return Err(io::Error::new(io::ErrorKind::Unsupported, format!("{name} is not supported")));
        }
    })
}

/// `lzma-rs` pushes decoded bytes into a writer, so the decoding runs on its
/// own thread and a channel turns it into a reader. Dropping the reader ends
/// the thread at its next write.
fn xz_reader<R: Read + Send + 'static>(reader: R) -> ChannelReader {
    let (tx, rx) = mpsc::sync_channel::<io::Result<Vec<u8>>>(8);
    std::thread::spawn(move || {
        let mut input = BufReader::new(reader);
        let mut out = ChannelWriter { tx: tx.clone(), buf: Vec::with_capacity(64 * 1024) };
        let result = lzma_rs::xz_decompress(&mut input, &mut out);
        match result {
            Ok(()) => {
                let _ = out.flush();
            }
            Err(e) => {
                let _ = tx.send(Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string())));
            }
        }
    });
    ChannelReader { rx, cur: Vec::new(), pos: 0 }
}

struct ChannelWriter {
    tx: mpsc::SyncSender<io::Result<Vec<u8>>>,
    buf: Vec<u8>,
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= 64 * 1024 {
            self.flush()?;
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(64 * 1024));
        self.tx.send(Ok(chunk)).map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "reader dropped"))
    }
}

struct ChannelReader {
    rx: mpsc::Receiver<io::Result<Vec<u8>>>,
    cur: Vec<u8>,
    pos: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.pos >= self.cur.len() {
            match self.rx.recv() {
                Ok(Ok(chunk)) => {
                    self.cur = chunk;
                    self.pos = 0;
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.cur.len() - self.pos);
        out[..n].copy_from_slice(&self.cur[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// A blocking `Read` over an async reader (an SFTP file), with read-ahead.
/// It must be used on a thread of `spawn_blocking`, never on the runtime.
pub struct AsyncBridge<R> {
    handle: tokio::runtime::Handle,
    inner: R,
}

impl<R: AsyncRead + Unpin> AsyncBridge<R> {
    /// The buffered reader to give to `decoder`.
    pub fn buffered(handle: tokio::runtime::Handle, inner: R) -> BufReader<Self> {
        BufReader::with_capacity(READ_AHEAD, Self { handle, inner })
    }
}

impl<R: AsyncRead + Unpin> Read for AsyncBridge<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.handle.block_on(self.inner.read(out))
    }
}

/// Counts the newlines that pass through while reading, so a compressed file
/// gets its real line count as it is decoded.
pub struct LineCounter<R> {
    inner: R,
    lines: Arc<AtomicU64>,
    last: u8,
    any: bool,
}

impl<R: Read> LineCounter<R> {
    pub fn new(inner: R) -> (Self, Arc<AtomicU64>) {
        let lines = Arc::new(AtomicU64::new(0));
        (Self { inner, lines: lines.clone(), last: b'\n', any: false }, lines)
    }

    /// Lines so far: every newline, plus the unfinished last line if any.
    pub fn count_so_far(&self) -> u64 {
        self.lines.load(Ordering::Relaxed) + u64::from(self.any && self.last != b'\n')
    }
}

impl<R: Read> Read for LineCounter<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(out)?;
        if n > 0 {
            let newlines = out[..n].iter().filter(|b| **b == b'\n').count() as u64;
            self.lines.fetch_add(newlines, Ordering::Relaxed);
            self.last = out[n - 1];
            self.any = true;
        }
        Ok(n)
    }
}

/// The number of lines in everything `reader` gives.
pub fn count_lines(reader: impl Read) -> io::Result<u64> {
    let (mut counter, _) = LineCounter::new(reader);
    let mut buf = vec![0u8; 256 * 1024];
    while counter.read(&mut buf)? > 0 {}
    Ok(counter.count_so_far())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const TEXT: &str = "first line\nsecond line\nthird line\n";

    // The same text packed by the gzip, zstd, xz and bzip2 command line tools.
    const GZ: &[u8] = include_bytes!("testdata/t.gz");
    const ZST: &[u8] = include_bytes!("testdata/t.zst");
    const XZ: &[u8] = include_bytes!("testdata/t.xz");
    const BZ2: &[u8] = include_bytes!("testdata/t.bz2");

    #[test]
    fn kinds_by_first_bytes() {
        assert_eq!(Compression::detect(GZ), Compression::Gzip);
        assert_eq!(Compression::detect(&[0x28, 0xb5, 0x2f, 0xfd, 0, 0]), Compression::Zstd);
        assert_eq!(Compression::detect(&[0xfd, b'7', b'z', b'X', b'Z', 0]), Compression::Xz);
        assert_eq!(Compression::detect(b"BZh91AY"), Compression::Bzip2);
        assert_eq!(Compression::detect(b"2026-10-19 10:00"), Compression::None);
        assert_eq!(Compression::detect(b""), Compression::None);
        let lz4 = Compression::detect(&[0x04, 0x22, 0x4d, 0x18, 0x64, 0x40]);
        assert_eq!(lz4, Compression::Unsupported("lz4"));
        assert!(!lz4.is_supported());
        assert!(Compression::Gzip.is_supported() && Compression::None.is_supported());
    }

    fn read_all(kind: Compression, bytes: &[u8]) -> String {
        let mut out = String::new();
        decoder(kind, Cursor::new(bytes.to_vec())).unwrap().read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn every_kind_round_trips() {
        for (kind, bytes) in [
            (Compression::Gzip, GZ),
            (Compression::Zstd, ZST),
            (Compression::Xz, XZ),
            (Compression::Bzip2, BZ2),
        ] {
            assert_eq!(Compression::detect(bytes), kind);
            assert_eq!(read_all(kind, bytes), TEXT, "{kind:?}");
        }
    }

    #[test]
    fn corrupt_xz_is_an_error_not_a_hang() {
        let mut bad = XZ.to_vec();
        let mid = bad.len() / 2;
        bad[mid] ^= 0xff;
        let mut out = Vec::new();
        let res = decoder(Compression::Xz, Cursor::new(bad)).and_then(|mut d| d.read_to_end(&mut out));
        assert!(res.is_err());
    }

    #[test]
    fn gzip_of_several_members_reads_as_one() {
        let mut two = GZ.to_vec();
        two.extend_from_slice(GZ);
        assert_eq!(read_all(Compression::Gzip, &two), format!("{TEXT}{TEXT}"));
    }

    #[test]
    fn plain_file_with_a_compressed_name_reads_as_plain() {
        // A name never decides: bytes of text are text.
        let kind = Compression::detect(TEXT.as_bytes());
        assert_eq!(kind, Compression::None);
        assert_eq!(read_all(kind, TEXT.as_bytes()), TEXT);
    }

    #[test]
    fn unsupported_kind_has_no_decoder() {
        let err = decoder(Compression::Unsupported("lz4"), Cursor::new(Vec::new())).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn garbage_after_the_magic_is_an_error() {
        let mut out = Vec::new();
        let bad = decoder(Compression::Gzip, Cursor::new(vec![0x1f, 0x8b, 0xff, 0xff, 0xff]))
            .and_then(|mut d| d.read_to_end(&mut out));
        assert!(bad.is_err());
    }

    #[test]
    fn counts_lines_while_reading() {
        assert_eq!(count_lines(Cursor::new(TEXT)).unwrap(), 3);
        assert_eq!(count_lines(Cursor::new("a\nb")).unwrap(), 2);
        assert_eq!(count_lines(Cursor::new("")).unwrap(), 0);
        assert_eq!(count_lines(decoder(Compression::Gzip, Cursor::new(GZ.to_vec())).unwrap()).unwrap(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bridge_reads_an_async_source_on_a_blocking_thread() {
        let handle = tokio::runtime::Handle::current();
        let src = Cursor::new(GZ.to_vec());
        let text = tokio::task::spawn_blocking(move || {
            let mut out = String::new();
            let reader = AsyncBridge::buffered(handle, src);
            decoder(Compression::Gzip, reader).unwrap().read_to_string(&mut out).unwrap();
            out
        })
        .await
        .unwrap();
        assert_eq!(text, TEXT);
    }
}
