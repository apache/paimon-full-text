use crate::config::FullTextIndexMetadata;
use crate::error::{FtIndexError, Result};
use crate::io::{ReadRequest, SeekRead, SeekWrite};
use serde::{Deserialize, Serialize};

pub const FORMAT_MAGIC: &[u8; 8] = b"PFTIDX01";
pub const FORMAT_VERSION: u32 = 1;

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

pub fn read_header<R: SeekRead>(input: &mut R) -> Result<(IndexHeader, u64)> {
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
    let mut header_json = vec![0u8; header_len];
    read_exact_at(input, 16, &mut header_json)?;
    let header = serde_json::from_slice(&header_json)?;
    Ok((header, 16 + header_len as u64))
}

pub fn read_exact_at<R: SeekRead>(input: &mut R, pos: u64, buf: &mut [u8]) -> Result<()> {
    let mut request = [ReadRequest { pos, buf }];
    input.pread(&mut request)?;
    Ok(())
}
