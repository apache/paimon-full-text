use crate::config::FullTextIndexMetadata;
use crate::error::{FtIndexError, Result};
use crate::io::{ReadRequest, SeekRead, SeekWrite};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

pub const FORMAT_MAGIC: &[u8; 8] = b"PFTIDX01";
pub const FORMAT_VERSION: u32 = 1;
const MAX_HEADER_BYTES: usize = 16 * 1024 * 1024;
const MAX_ARCHIVE_READ_BATCH_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARCHIVE_READ_BATCH_RANGES: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchiveFileEntry {
    pub name: String,
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexHeader {
    pub metadata: FullTextIndexMetadata,
    pub files: Vec<ArchiveFileEntry>,
}

pub fn write_envelope<W: SeekWrite>(
    output: &mut W,
    header: &IndexHeader,
    files: &[(String, Vec<u8>)],
) -> Result<()> {
    let header_json = serde_json::to_vec(header)?;
    if header_json.len() > MAX_HEADER_BYTES {
        return Err(FtIndexError::InvalidStorage(format!(
            "header too large: {} bytes exceeds {} bytes",
            header_json.len(),
            MAX_HEADER_BYTES
        )));
    }
    output.write_all(FORMAT_MAGIC)?;
    output.write_all(&FORMAT_VERSION.to_be_bytes())?;
    output.write_all(
        &u32::try_from(header_json.len())
            .map_err(|_| FtIndexError::InvalidStorage("header too large".to_string()))?
            .to_be_bytes(),
    )?;
    output.write_all(&header_json)?;
    for (_, data) in files {
        output.write_all(data)?;
    }
    output.flush()?;
    Ok(())
}

pub fn read_header<R: SeekRead>(input: &R) -> Result<(IndexHeader, u64)> {
    let mut fixed = [0u8; 16];
    read_exact_at(input, 0, &mut fixed)?;
    if &fixed[0..8] != FORMAT_MAGIC {
        return Err(FtIndexError::InvalidStorage("bad magic".to_string()));
    }
    let version = u32::from_be_bytes(fixed[8..12].try_into().expect("slice length"));
    if version != FORMAT_VERSION {
        return Err(FtIndexError::InvalidStorage(format!(
            "unsupported format version {version}"
        )));
    }
    let header_len = u32::from_be_bytes(fixed[12..16].try_into().expect("slice length")) as usize;
    if header_len > MAX_HEADER_BYTES {
        return Err(FtIndexError::InvalidStorage(format!(
            "header too large: {header_len} bytes exceeds {MAX_HEADER_BYTES} bytes"
        )));
    }
    let mut header_json = vec![0u8; header_len];
    read_exact_at(input, 16, &mut header_json)?;
    let header = serde_json::from_slice(&header_json)?;
    validate_header(&header)?;
    Ok((header, 16 + header_len as u64))
}

fn validate_header(header: &IndexHeader) -> Result<()> {
    header.metadata.config.validate()?;
    if header.metadata.tantivy_version.trim().is_empty() {
        return Err(FtIndexError::InvalidStorage(
            "missing Tantivy version in index metadata".to_string(),
        ));
    }
    if header.files.is_empty() {
        return Err(FtIndexError::InvalidStorage(
            "archive file list must not be empty".to_string(),
        ));
    }

    let mut names = HashSet::with_capacity(header.files.len());
    let mut ranges = Vec::with_capacity(header.files.len());
    for file in &header.files {
        validate_archive_file_name(&file.name)?;
        if !names.insert(file.name.as_str()) {
            return Err(FtIndexError::InvalidStorage(format!(
                "duplicate archive file '{}'",
                file.name
            )));
        }
        let end = file.offset.checked_add(file.length).ok_or_else(|| {
            FtIndexError::InvalidStorage(format!("archive file '{}' range overflow", file.name))
        })?;
        ranges.push((file.offset, end, file.name.as_str()));
    }

    ranges.sort_by_key(|(start, _, _)| *start);
    let mut previous_end = 0u64;
    for (start, end, name) in ranges {
        if start < previous_end {
            return Err(FtIndexError::InvalidStorage(format!(
                "archive file '{name}' overlaps a previous file"
            )));
        }
        previous_end = end;
    }
    Ok(())
}

fn validate_archive_file_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(FtIndexError::InvalidStorage(
            "archive file name must not be empty".to_string(),
        ));
    }
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(FtIndexError::InvalidStorage(format!(
            "archive file '{name}' must be a plain file name"
        ))),
    }
}

pub fn read_archive_files<R: SeekRead>(
    input: &R,
    body_start: u64,
    files: &[ArchiveFileEntry],
) -> Result<Vec<(String, Vec<u8>)>> {
    read_archive_files_with(input, body_start, files, |name, data| {
        Ok((name.to_string(), data.to_vec()))
    })
}

pub fn read_archive_files_with<R, F, T>(
    input: &R,
    body_start: u64,
    files: &[ArchiveFileEntry],
    mut consume: F,
) -> Result<Vec<T>>
where
    R: SeekRead,
    F: FnMut(&str, &[u8]) -> Result<T>,
{
    let mut output = Vec::with_capacity(files.len());
    let mut pending = Vec::new();
    let mut pending_bytes = 0usize;

    for file in files {
        let length = usize::try_from(file.length).map_err(|_| {
            FtIndexError::InvalidStorage(format!("archive file '{}' is too large", file.name))
        })?;
        let pos = body_start.checked_add(file.offset).ok_or_else(|| {
            FtIndexError::InvalidStorage(format!("archive file '{}' offset overflow", file.name))
        })?;

        if !pending.is_empty()
            && (pending.len() >= MAX_ARCHIVE_READ_BATCH_RANGES
                || pending_bytes.saturating_add(length) > MAX_ARCHIVE_READ_BATCH_BYTES)
        {
            read_archive_file_batch(input, &mut pending, &mut consume, &mut output)?;
            pending_bytes = 0;
        }

        pending.push(PendingArchiveFile {
            name: file.name.clone(),
            pos,
            data: vec![0u8; length],
        });
        pending_bytes = pending_bytes.saturating_add(length);
    }

    if !pending.is_empty() {
        read_archive_file_batch(input, &mut pending, &mut consume, &mut output)?;
    }

    Ok(output)
}

struct PendingArchiveFile {
    name: String,
    pos: u64,
    data: Vec<u8>,
}

fn read_archive_file_batch<R, F, T>(
    input: &R,
    pending: &mut Vec<PendingArchiveFile>,
    consume: &mut F,
    output: &mut Vec<T>,
) -> Result<()>
where
    R: SeekRead,
    F: FnMut(&str, &[u8]) -> Result<T>,
{
    {
        let mut requests: Vec<_> = pending
            .iter_mut()
            .map(|file| ReadRequest::new(file.pos, file.data.as_mut_slice()))
            .collect();
        input.pread(&mut requests)?;
    }

    for file in pending.drain(..) {
        output.push(consume(&file.name, &file.data)?);
    }
    Ok(())
}

pub fn read_exact_at<R: SeekRead>(input: &R, pos: u64, buf: &mut [u8]) -> Result<()> {
    let mut request = [ReadRequest::new(pos, buf)];
    input.pread(&mut request)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FullTextIndexConfig, FullTextIndexMetadata};
    use crate::io::{PosWriter, SliceReader};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingReader {
        data: Vec<u8>,
        pread_batches: AtomicUsize,
        max_ranges_per_batch: AtomicUsize,
    }

    impl SeekRead for CountingReader {
        fn pread(&self, ranges: &mut [ReadRequest<'_>]) -> std::io::Result<()> {
            self.pread_batches.fetch_add(1, Ordering::SeqCst);
            self.max_ranges_per_batch
                .fetch_max(ranges.len(), Ordering::SeqCst);
            for range in ranges {
                let start = usize::try_from(range.pos).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset overflow")
                })?;
                let end = start.checked_add(range.buf.len()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "range overflow")
                })?;
                if end > self.data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "read past end of slice",
                    ));
                }
                range.buf.copy_from_slice(&self.data[start..end]);
            }
            Ok(())
        }
    }

    #[test]
    fn read_archive_files_uses_one_batched_pread() {
        let reader = CountingReader {
            data: b"headerabcdef".to_vec(),
            pread_batches: AtomicUsize::new(0),
            max_ranges_per_batch: AtomicUsize::new(0),
        };
        let files = vec![
            ArchiveFileEntry {
                name: "a".to_string(),
                offset: 0,
                length: 2,
            },
            ArchiveFileEntry {
                name: "b".to_string(),
                offset: 2,
                length: 3,
            },
            ArchiveFileEntry {
                name: "c".to_string(),
                offset: 5,
                length: 1,
            },
        ];

        let files = read_archive_files(&reader, 6, &files).expect("archive files");

        assert_eq!(reader.pread_batches.load(Ordering::SeqCst), 1);
        assert_eq!(reader.max_ranges_per_batch.load(Ordering::SeqCst), 3);
        assert_eq!(
            files,
            vec![
                ("a".to_string(), b"ab".to_vec()),
                ("b".to_string(), b"cde".to_vec()),
                ("c".to_string(), b"f".to_vec()),
            ]
        );
    }

    #[test]
    fn read_header_rejects_invalid_archive_metadata() {
        let cases = [
            (
                vec![
                    ArchiveFileEntry {
                        name: "meta.json".to_string(),
                        offset: 0,
                        length: 2,
                    },
                    ArchiveFileEntry {
                        name: "meta.json".to_string(),
                        offset: 2,
                        length: 2,
                    },
                ],
                "duplicate archive file",
            ),
            (
                vec![ArchiveFileEntry {
                    name: "../meta.json".to_string(),
                    offset: 0,
                    length: 2,
                }],
                "must be a plain file name",
            ),
            (
                vec![
                    ArchiveFileEntry {
                        name: "a".to_string(),
                        offset: 0,
                        length: 3,
                    },
                    ArchiveFileEntry {
                        name: "b".to_string(),
                        offset: 2,
                        length: 3,
                    },
                ],
                "overlaps a previous file",
            ),
        ];

        for (files, expected) in cases {
            let bytes = encode_header(IndexHeader {
                metadata: valid_metadata(),
                files,
            });
            let err = read_header(&SliceReader::new(bytes)).expect_err("invalid header");
            assert!(
                err.to_string().contains(expected),
                "expected '{expected}', got '{err}'"
            );
        }
    }

    #[test]
    fn read_header_rejects_missing_tantivy_version() {
        let bytes = encode_header(IndexHeader {
            metadata: FullTextIndexMetadata {
                tantivy_version: String::new(),
                ..valid_metadata()
            },
            files: vec![ArchiveFileEntry {
                name: "meta.json".to_string(),
                offset: 0,
                length: 2,
            }],
        });

        let err = read_header(&SliceReader::new(bytes)).expect_err("invalid header");

        assert!(err
            .to_string()
            .contains("missing Tantivy version in index metadata"));
    }

    #[test]
    fn read_header_rejects_oversized_header_before_allocating() {
        const OVERSIZED_HEADER_LEN: u32 = 16 * 1024 * 1024 + 1;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(FORMAT_MAGIC);
        bytes.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        bytes.extend_from_slice(&OVERSIZED_HEADER_LEN.to_be_bytes());

        let err = read_header(&SliceReader::new(bytes)).expect_err("oversized header should fail");

        assert!(err.to_string().contains("header too large"));
    }

    fn valid_metadata() -> FullTextIndexMetadata {
        FullTextIndexMetadata {
            config: FullTextIndexConfig::new(),
            document_count: 1,
            tantivy_version: tantivy::version().to_string(),
        }
    }

    fn encode_header(header: IndexHeader) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_envelope(
            &mut PosWriter::new(&mut bytes),
            &header,
            &[("meta.json".to_string(), b"ab".to_vec())],
        )
        .expect("write envelope");
        bytes
    }
}
