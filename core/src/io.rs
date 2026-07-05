use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ReadRequest<'a> {
    pub pos: u64,
    pub buf: &'a mut [u8],
}

impl<'a> ReadRequest<'a> {
    pub fn new(pos: u64, buf: &'a mut [u8]) -> Self {
        Self { pos, buf }
    }
}

/// Positional reader shared by Tantivy file handles.
///
/// Implementations must be safe to call concurrently. Each call may contain one
/// or more disjoint read requests, and callers may issue multiple `pread` calls
/// at the same time through shared references.
pub trait SeekRead: Send + Sync {
    fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FullTextReadMetrics {
    pub pread_calls: u64,
    pub pread_ranges: u64,
    pub pread_bytes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_evictions: u64,
    pub cached_blocks: u64,
}

#[derive(Default)]
pub(crate) struct ReadMetrics {
    pread_calls: AtomicU64,
    pread_ranges: AtomicU64,
    pread_bytes: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    cache_evictions: AtomicU64,
    cached_blocks: AtomicU64,
}

impl ReadMetrics {
    pub(crate) fn record_pread(&self, ranges: &[ReadRequest<'_>]) {
        self.pread_calls.fetch_add(1, Ordering::Relaxed);
        self.pread_ranges
            .fetch_add(ranges.len() as u64, Ordering::Relaxed);
        self.pread_bytes.fetch_add(
            ranges.iter().map(|range| range.buf.len() as u64).sum(),
            Ordering::Relaxed,
        );
    }

    pub(crate) fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_cache_insert(&self) {
        self.cached_blocks.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_cache_eviction(&self) {
        self.cache_evictions.fetch_add(1, Ordering::Relaxed);
        self.cached_blocks.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> FullTextReadMetrics {
        FullTextReadMetrics {
            pread_calls: self.pread_calls.load(Ordering::Relaxed),
            pread_ranges: self.pread_ranges.load(Ordering::Relaxed),
            pread_bytes: self.pread_bytes.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            cache_evictions: self.cache_evictions.load(Ordering::Relaxed),
            cached_blocks: self.cached_blocks.load(Ordering::Relaxed),
        }
    }
}

pub trait SeekWrite: Send {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct PosWriter<W> {
    inner: W,
}

impl<W> PosWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: io::Write + Send> SeekWrite for PosWriter<W> {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        io::Write::write_all(&mut self.inner, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.inner)
    }
}

pub struct SliceReader {
    data: Vec<u8>,
}

impl SliceReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl SeekRead for SliceReader {
    fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
        for range in ranges {
            let start = usize::try_from(range.pos)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
            let end = start
                .checked_add(range.buf.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
            if end > self.data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "read past end of slice",
                ));
            }
            range.buf.copy_from_slice(&self.data[start..end]);
        }
        Ok(())
    }
}
