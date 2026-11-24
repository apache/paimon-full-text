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

//! Options that tune how a writer builds an index.
//!
//! They only affect the resources used while building. They never change tokenization or the
//! archive format, and they are not persisted in the index metadata. They are passed together
//! with the other writer options, with the same optional `fulltext.` prefix:
//!
//! | Option                     | Default    | Meaning                                         |
//! |----------------------------|------------|-------------------------------------------------|
//! | `build.writer-threads`     | `1`        | Tantivy indexing threads per writer             |
//! | `build.writer-memory-size` | `50000000` | Tantivy memory budget shared by those threads   |
//!
//! Sizes are bytes with an optional `k`/`kb`, `m`/`mb` or `g`/`gb` suffix (powers of 1024).
//!
//! Each thread indexes into its own segment and flushes it when its share of the budget is full,
//! so more threads build faster and produce more segments. Document order within the archive is
//! not affected: results are identified by row id.

use crate::config::normalize_option_key;
use crate::error::{FtIndexError, Result};
use std::collections::HashMap;

const WRITER_THREADS: &str = "build.writer-threads";
const WRITER_MEMORY_SIZE: &str = "build.writer-memory-size";

/// Bounds Tantivy puts on the memory budget of one indexing thread. Tantivy does not export them.
const MIN_MEMORY_BYTES_PER_THREAD: usize = 15_000_000;
const MAX_MEMORY_BYTES_PER_THREAD: usize = u32::MAX as usize - 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexBuildConfig {
    /// Number of Tantivy indexing threads.
    pub writer_threads: usize,
    /// Memory budget in bytes, split evenly between the indexing threads.
    pub writer_memory_bytes: usize,
}

impl Default for IndexBuildConfig {
    fn default() -> Self {
        Self {
            writer_threads: 1,
            writer_memory_bytes: 50_000_000,
        }
    }
}

impl IndexBuildConfig {
    /// Parses the `build.*` options; other keys are ignored.
    pub fn from_options(options: &HashMap<String, String>) -> Result<Self> {
        let mut config = Self::default();
        for (raw_key, value) in options {
            let key = normalize_option_key(raw_key);
            match key.as_str() {
                WRITER_THREADS => config.writer_threads = parse_count(&key, value)?,
                WRITER_MEMORY_SIZE => config.writer_memory_bytes = parse_size(&key, value)?,
                _ => {}
            }
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.writer_threads == 0 {
            return Err(FtIndexError::InvalidOption {
                key: WRITER_THREADS.to_string(),
                message: "must be greater than 0".to_string(),
            });
        }
        let per_thread = self.writer_memory_bytes / self.writer_threads;
        if !(MIN_MEMORY_BYTES_PER_THREAD..MAX_MEMORY_BYTES_PER_THREAD).contains(&per_thread) {
            return Err(FtIndexError::InvalidOption {
                key: WRITER_MEMORY_SIZE.to_string(),
                message: format!(
                    "must be at least {MIN_MEMORY_BYTES_PER_THREAD} and less than \
                     {MAX_MEMORY_BYTES_PER_THREAD} bytes per writer thread, got {per_thread} \
                     ({} bytes for {} threads)",
                    self.writer_memory_bytes, self.writer_threads
                ),
            });
        }
        Ok(())
    }
}

fn invalid(key: &str, expected: &str, value: &str) -> FtIndexError {
    FtIndexError::InvalidOption {
        key: key.to_string(),
        message: format!("expected {expected}, got '{value}'"),
    }
}

fn parse_count(key: &str, value: &str) -> Result<usize> {
    value
        .trim()
        .parse()
        .map_err(|_| invalid(key, "unsigned integer", value))
}

fn parse_size(key: &str, value: &str) -> Result<usize> {
    let v = value.trim().to_ascii_lowercase();
    let unit_start = v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len());
    let (digits, unit) = v.split_at(unit_start);
    let shift = match unit.trim_start() {
        "" | "b" => 0,
        "k" | "kb" => 10,
        "m" | "mb" => 20,
        "g" | "gb" => 30,
        _ => return Err(invalid(key, "size such as 256mb", value)),
    };
    digits
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_mul(1usize << shift))
        .ok_or_else(|| invalid(key, "size such as 256mb", value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(pairs: &[(&str, &str)]) -> Result<IndexBuildConfig> {
        let options = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        IndexBuildConfig::from_options(&options)
    }

    #[test]
    fn parses_overrides() {
        assert_eq!(parse(&[]).unwrap(), IndexBuildConfig::default());
        assert_eq!(
            parse(&[
                ("fulltext.build.writer_threads", " 4 "),
                ("build.writer-memory-size", "1 GB"),
                ("tokenizer", "jieba"),
            ])
            .unwrap(),
            IndexBuildConfig {
                writer_threads: 4,
                writer_memory_bytes: 1 << 30,
            }
        );
    }

    #[test]
    fn parses_sizes() {
        for (value, expected) in [
            ("50000000", 50_000_000),
            ("20000000b", 20_000_000),
            ("32768k", 32 << 20),
            ("64m", 64 << 20),
            (" 64 MB ", 64 << 20),
            ("2g", 2 << 30),
        ] {
            let config = parse(&[("build.writer-memory-size", value)]).unwrap();
            assert_eq!(config.writer_memory_bytes, expected, "{value}");
        }
    }

    #[test]
    fn rejects_invalid_values() {
        for (key, value) in [
            ("build.writer-threads", "x"),
            ("build.writer-threads", "-1"),
            ("build.writer-threads", "0"),
            ("build.writer-memory-size", "64x"),
            ("build.writer-memory-size", "mb"),
            ("build.writer-memory-size", "0"),
            ("build.writer-memory-size", "14mb"),
            ("build.writer-memory-size", "4gb"),
            ("build.writer-memory-size", "99999999999gb"),
        ] {
            let err = parse(&[(key, value)]).unwrap_err();
            assert!(err.to_string().contains(key), "{key}={value}: {err}");
        }
    }

    #[test]
    fn memory_budget_is_checked_per_thread() {
        // The default budget is enough for three threads, not for four.
        assert!(parse(&[("build.writer-threads", "3")]).is_ok());
        let err = parse(&[("build.writer-threads", "4")]).unwrap_err();
        assert!(
            err.to_string().contains("build.writer-memory-size"),
            "{err}"
        );
        assert!(parse(&[
            ("build.writer-threads", "4"),
            ("build.writer-memory-size", "8gb")
        ])
        .is_ok());
    }
}
