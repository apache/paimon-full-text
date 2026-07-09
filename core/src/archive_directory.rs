// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use crate::error::{FtIndexError, Result};
use crate::io::{ReadMetrics, ReadRequest, SeekRead};
use crate::storage::ArchiveFileEntry;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tantivy::directory::error::{DeleteError, LockError, OpenReadError, OpenWriteError};
use tantivy::directory::WritePtr;
use tantivy::directory::{
    Directory, DirectoryLock, FileHandle, Lock, OwnedBytes, WatchCallback, WatchHandle, META_LOCK,
};
use tantivy_common::HasLen;

const READ_CACHE_BLOCK_BYTES: usize = 64 * 1024;
const READ_CACHE_MAX_BLOCKS: usize = 256;

pub(crate) struct ArchiveDirectory<R> {
    input: Arc<R>,
    files: Arc<HashMap<PathBuf, ArchivedFile>>,
    cache: Arc<ArchiveReadCache>,
}

impl<R> Clone for ArchiveDirectory<R> {
    fn clone(&self) -> Self {
        Self {
            input: Arc::clone(&self.input),
            files: Arc::clone(&self.files),
            cache: Arc::clone(&self.cache),
        }
    }
}

impl<R> ArchiveDirectory<R> {
    #[cfg(test)]
    pub(crate) fn new(input: Arc<R>, body_start: u64, files: &[ArchiveFileEntry]) -> Result<Self> {
        Self::new_with_metrics(input, body_start, files, Arc::new(ReadMetrics::default()))
    }

    pub(crate) fn new_with_metrics(
        input: Arc<R>,
        body_start: u64,
        files: &[ArchiveFileEntry],
        metrics: Arc<ReadMetrics>,
    ) -> Result<Self> {
        let mut mapped_files = HashMap::with_capacity(files.len());
        for file in files {
            let start = body_start.checked_add(file.offset).ok_or_else(|| {
                FtIndexError::InvalidStorage(format!(
                    "archive file '{}' offset overflow",
                    file.name
                ))
            })?;
            let length = usize::try_from(file.length).map_err(|_| {
                FtIndexError::InvalidStorage(format!("archive file '{}' is too large", file.name))
            })?;
            mapped_files.insert(PathBuf::from(&file.name), ArchivedFile { start, length });
        }
        Ok(Self {
            input,
            files: Arc::new(mapped_files),
            cache: Arc::new(ArchiveReadCache::new(metrics)),
        })
    }
}

impl<R> fmt::Debug for ArchiveDirectory<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchiveDirectory")
            .field("files", &self.files)
            .finish_non_exhaustive()
    }
}

impl<R: SeekRead + 'static> Directory for ArchiveDirectory<R> {
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> std::result::Result<Arc<dyn FileHandle>, OpenReadError> {
        let file = self.file(path)?;
        Ok(Arc::new(ArchiveFileHandle {
            input: Arc::clone(&self.input),
            cache: Arc::clone(&self.cache),
            path: path.to_path_buf(),
            start: file.start,
            length: file.length,
        }))
    }

    fn delete(&self, path: &Path) -> std::result::Result<(), DeleteError> {
        if self.files.contains_key(path) {
            Err(DeleteError::IoError {
                io_error: Arc::new(read_only_error()),
                filepath: path.to_path_buf(),
            })
        } else {
            Err(DeleteError::FileDoesNotExist(path.to_path_buf()))
        }
    }

    fn exists(&self, path: &Path) -> std::result::Result<bool, OpenReadError> {
        Ok(self.files.contains_key(path))
    }

    fn open_write(&self, path: &Path) -> std::result::Result<WritePtr, OpenWriteError> {
        Err(OpenWriteError::wrap_io_error(
            read_only_error(),
            path.to_path_buf(),
        ))
    }

    fn atomic_read(&self, path: &Path) -> std::result::Result<Vec<u8>, OpenReadError> {
        let bytes = self
            .open_read(path)?
            .read_bytes()
            .map_err(|io_error| OpenReadError::wrap_io_error(io_error, path.to_path_buf()))?;
        Ok(bytes.as_slice().to_vec())
    }

    fn atomic_write(&self, _path: &Path, _data: &[u8]) -> io::Result<()> {
        Err(read_only_error())
    }

    fn sync_directory(&self) -> io::Result<()> {
        Ok(())
    }

    fn acquire_lock(&self, lock: &Lock) -> std::result::Result<DirectoryLock, LockError> {
        if lock.filepath == META_LOCK.filepath {
            Ok(DirectoryLock::from(Box::new(())))
        } else {
            Err(LockError::wrap_io_error(read_only_error()))
        }
    }

    fn watch(&self, _watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        Ok(WatchHandle::empty())
    }
}

impl<R> ArchiveDirectory<R> {
    fn file(&self, path: &Path) -> std::result::Result<ArchivedFile, OpenReadError> {
        self.files
            .get(path)
            .copied()
            .ok_or_else(|| OpenReadError::FileDoesNotExist(path.to_path_buf()))
    }
}

#[derive(Clone, Copy, Debug)]
struct ArchivedFile {
    start: u64,
    length: usize,
}

struct ArchiveFileHandle<R> {
    input: Arc<R>,
    cache: Arc<ArchiveReadCache>,
    path: PathBuf,
    start: u64,
    length: usize,
}

impl<R> fmt::Debug for ArchiveFileHandle<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchiveFileHandle")
            .field("path", &self.path)
            .field("start", &self.start)
            .field("length", &self.length)
            .finish_non_exhaustive()
    }
}

impl<R> HasLen for ArchiveFileHandle<R> {
    fn len(&self) -> usize {
        self.length
    }
}

impl<R: SeekRead + 'static> FileHandle for ArchiveFileHandle<R> {
    fn read_bytes(&self, range: Range<usize>) -> io::Result<OwnedBytes> {
        if range.start > range.end || range.end > self.length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "invalid range {}..{} for archive file '{}' with length {}",
                    range.start,
                    range.end,
                    self.path.display(),
                    self.length
                ),
            ));
        }
        if range.is_empty() {
            return Ok(OwnedBytes::new(Vec::new()));
        }

        if let Some(bytes) = self.read_cached_block(&range)? {
            return Ok(bytes);
        }

        let relative_start = u64::try_from(range.start)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
        let pos = self
            .start
            .checked_add(relative_start)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
        let data = vec![0u8; range.len()];
        self.read_direct(pos, data)
    }
}

impl<R: SeekRead + 'static> ArchiveFileHandle<R> {
    fn read_cached_block(&self, range: &Range<usize>) -> io::Result<Option<OwnedBytes>> {
        if range.len() > READ_CACHE_BLOCK_BYTES {
            return Ok(None);
        }
        let block_start = (range.start / READ_CACHE_BLOCK_BYTES) * READ_CACHE_BLOCK_BYTES;
        let block_end = self.length.min(block_start + READ_CACHE_BLOCK_BYTES);
        if range.end > block_end {
            return Ok(None);
        }

        let relative_start = u64::try_from(block_start)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
        let pos = self
            .start
            .checked_add(relative_start)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
        let block = if let Some(block) = self.cache.get(pos)? {
            block
        } else {
            self.cache.record_miss();
            let block_len = block_end - block_start;
            let block = self.read_direct(pos, vec![0u8; block_len])?;
            self.cache.insert(pos, block)?
        };
        let start = range.start - block_start;
        Ok(Some(block.slice(start..start + range.len())))
    }

    fn read_direct(&self, pos: u64, mut data: Vec<u8>) -> io::Result<OwnedBytes> {
        let mut request = [ReadRequest::new(pos, data.as_mut_slice())];
        self.input.pread(&mut request)?;
        Ok(OwnedBytes::new(data))
    }
}

struct ArchiveReadCache {
    inner: Mutex<ArchiveReadCacheInner>,
    metrics: Arc<ReadMetrics>,
}

#[derive(Default)]
struct ArchiveReadCacheInner {
    blocks: HashMap<u64, OwnedBytes>,
    order: VecDeque<u64>,
}

impl ArchiveReadCache {
    fn new(metrics: Arc<ReadMetrics>) -> Self {
        Self {
            inner: Mutex::new(ArchiveReadCacheInner::default()),
            metrics,
        }
    }

    fn get(&self, pos: u64) -> io::Result<Option<OwnedBytes>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("archive read cache lock poisoned"))?;
        let block = inner.blocks.get(&pos).cloned();
        if block.is_some() {
            self.metrics.record_cache_hit();
        }
        Ok(block)
    }

    fn record_miss(&self) {
        self.metrics.record_cache_miss();
    }

    fn insert(&self, pos: u64, block: OwnedBytes) -> io::Result<OwnedBytes> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("archive read cache lock poisoned"))?;
        if let Some(existing) = inner.blocks.get(&pos) {
            return Ok(existing.clone());
        }
        while inner.blocks.len() >= READ_CACHE_MAX_BLOCKS {
            if let Some(evicted) = inner.order.pop_front() {
                inner.blocks.remove(&evicted);
                self.metrics.record_cache_eviction();
            } else {
                break;
            }
        }
        inner.order.push_back(pos);
        inner.blocks.insert(pos, block.clone());
        self.metrics.record_cache_insert();
        Ok(block)
    }
}

fn read_only_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "archive directory is read-only",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct SyncCountingReader {
        calls: AtomicUsize,
    }

    impl SeekRead for SyncCountingReader {
        fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            for range in ranges {
                range.buf.fill(0);
            }
            Ok(())
        }
    }

    #[test]
    fn archive_directory_accepts_shared_sync_reader_without_mutex() {
        let input = Arc::new(SyncCountingReader::default());
        let directory = ArchiveDirectory::new(
            Arc::clone(&input),
            0,
            &[ArchiveFileEntry {
                name: "meta.json".to_string(),
                offset: 0,
                length: 1,
            }],
        )
        .expect("archive directory");

        let bytes = directory
            .open_read(Path::new("meta.json"))
            .expect("file")
            .read_bytes()
            .expect("bytes");

        assert_eq!(bytes.as_slice(), &[0]);
        assert_eq!(input.calls.load(Ordering::SeqCst), 1);
    }

    struct ConcurrentReader {
        data: Vec<u8>,
        read_calls: AtomicUsize,
        active_calls: AtomicUsize,
        max_active_calls: AtomicUsize,
    }

    impl SeekRead for ConcurrentReader {
        fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active_calls.fetch_max(active, Ordering::SeqCst);

            let start = Instant::now();
            while self.max_active_calls.load(Ordering::SeqCst) < 2
                && start.elapsed() < Duration::from_millis(500)
            {
                std::thread::yield_now();
            }

            for range in ranges {
                let start = usize::try_from(range.pos)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
                let end = start
                    .checked_add(range.buf.len())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
                range.buf.copy_from_slice(&self.data[start..end]);
            }
            self.active_calls.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct CountingDataReader {
        data: Vec<u8>,
        read_calls: AtomicUsize,
    }

    impl SeekRead for CountingDataReader {
        fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> io::Result<()> {
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            for range in ranges {
                let start = usize::try_from(range.pos)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
                let end = start
                    .checked_add(range.buf.len())
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
                range.buf.copy_from_slice(&self.data[start..end]);
            }
            Ok(())
        }
    }

    #[test]
    fn archive_directory_allows_concurrent_pread_calls() {
        let input = Arc::new(ConcurrentReader {
            data: b"abcdef".to_vec(),
            read_calls: AtomicUsize::new(0),
            active_calls: AtomicUsize::new(0),
            max_active_calls: AtomicUsize::new(0),
        });
        let directory = ArchiveDirectory::new(
            Arc::clone(&input),
            0,
            &[ArchiveFileEntry {
                name: "meta.json".to_string(),
                offset: 0,
                length: 6,
            }],
        )
        .expect("archive directory");

        let start_reads = Arc::new(Barrier::new(3));
        let first_directory = directory.clone();
        let first_start = Arc::clone(&start_reads);
        let second_directory = directory.clone();
        let second_start = Arc::clone(&start_reads);
        let first = std::thread::spawn(move || {
            first_start.wait();
            first_directory
                .open_read(Path::new("meta.json"))
                .expect("file")
                .read_bytes()
                .expect("bytes")
                .as_slice()
                .to_vec()
        });
        let second = std::thread::spawn(move || {
            second_start.wait();
            second_directory
                .open_read(Path::new("meta.json"))
                .expect("file")
                .read_bytes()
                .expect("bytes")
                .as_slice()
                .to_vec()
        });

        start_reads.wait();
        assert_eq!(first.join().expect("first read"), b"abcdef");
        assert_eq!(second.join().expect("second read"), b"abcdef");
        assert_eq!(input.max_active_calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn archive_directory_caches_repeated_small_reads() {
        let metrics = Arc::new(ReadMetrics::default());
        let input = Arc::new(CountingDataReader {
            data: b"abcdefghijklmnopqrstuvwxyz".to_vec(),
            read_calls: AtomicUsize::new(0),
        });
        let directory = ArchiveDirectory::new_with_metrics(
            Arc::clone(&input),
            0,
            &[ArchiveFileEntry {
                name: "meta.json".to_string(),
                offset: 0,
                length: 26,
            }],
            Arc::clone(&metrics),
        )
        .expect("archive directory");
        let file = directory.open_read(Path::new("meta.json")).expect("file");

        let first = file.read_bytes_slice(3..9).expect("first");
        let second = file.read_bytes_slice(3..9).expect("second");

        assert_eq!(first.as_slice(), b"defghi");
        assert_eq!(second.as_slice(), b"defghi");
        assert_eq!(input.read_calls.load(Ordering::SeqCst), 1);
        let metrics = metrics.snapshot();
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
        assert_eq!(metrics.cached_blocks, 1);
    }
}
