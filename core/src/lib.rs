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

mod archive_directory;
pub mod config;
pub mod error;
pub mod index;
pub mod io;
mod query;
pub mod storage;
pub mod tokenizer;

pub use config::{FullTextIndexConfig, FullTextIndexMetadata};
pub use error::{FtIndexError, Result};
pub use index::{FullTextDocument, FullTextIndexReader, FullTextIndexWriter, FullTextSearchResult};
pub use io::FullTextReadMetrics;
pub use tokenizer::{TokenizerConfig, TokenizerKind};
