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

use paimon_ftindex_core::io::{PosWriter, SliceReader};
use paimon_ftindex_core::{
    FullTextIndexConfig, FullTextIndexReader, FullTextIndexWriter, TokenizerConfig,
};
use std::collections::HashMap;

fn build_reader(
    tokenizer: &str,
    documents: &[(i64, &str)],
) -> anyhow::Result<FullTextIndexReader<SliceReader>> {
    let options = HashMap::from([
        ("fulltext.tokenizer".to_string(), tokenizer.to_string()),
        ("fulltext.stem".to_string(), "false".to_string()),
        (
            "fulltext.remove-stop-words".to_string(),
            "false".to_string(),
        ),
    ]);
    let config = FullTextIndexConfig::new().tokenizer(TokenizerConfig::from_options(&options)?);
    let mut writer = FullTextIndexWriter::new(config)?;
    for &(row_id, text) in documents {
        writer.add_document(row_id, text)?;
    }
    let mut bytes = Vec::new();
    writer.write(&mut PosWriter::new(&mut bytes))?;
    let reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
    assert_eq!(
        serde_json::to_value(&reader.metadata().config.tokenizer.tokenizer)?,
        serde_json::Value::String(tokenizer.to_string()),
    );
    Ok(reader)
}

fn matching_ids(
    reader: &FullTextIndexReader<SliceReader>,
    query: &str,
) -> anyhow::Result<Vec<i64>> {
    let query = serde_json::json!({
        "match": {"query": query, "column": "text", "operator": "AND"}
    });
    Ok(reader.search(query.to_string(), 10)?.row_ids)
}

fn phrase_ids(reader: &FullTextIndexReader<SliceReader>, query: &str) -> anyhow::Result<Vec<i64>> {
    let query = serde_json::json!({
        "match_phrase": {"query": query, "column": "text"}
    });
    Ok(reader.search(query.to_string(), 10)?.row_ids)
}

#[test]
fn mixed_language_tokenizers_search_chinese_and_english_terms() -> anyhow::Result<()> {
    for tokenizer in ["icu", "icu/split", "jieba"] {
        let reader = build_reader(
            tokenizer,
            &[
                (1, "Paimon支持中文搜索和Rust开发"),
                (2, "Paimon支持英文搜索和Java开发"),
            ],
        )?;

        for (query, expected) in [
            ("中文RUST", vec![1]),
            ("英文java", vec![2]),
            ("Paimon中文Rust", vec![1]),
            ("中文java", vec![]),
        ] {
            assert_eq!(
                matching_ids(&reader, query)?,
                expected,
                "{tokenizer}: {query}"
            );
        }
    }
    Ok(())
}

#[test]
fn icu_modes_control_identifier_boundaries_after_reopening_index() -> anyhow::Result<()> {
    for (tokenizer, partial_ids) in [("icu", vec![]), ("icu/split", vec![1])] {
        let reader = build_reader(
            tokenizer,
            &[
                (1, "中文 hello_world alpha.beta getUserName"),
                (2, "中文 hello planet alpha gamma"),
            ],
        )?;

        for query in ["world", "beta", "hello world"] {
            assert_eq!(
                matching_ids(&reader, query)?,
                partial_ids,
                "{tokenizer}: {query}"
            );
        }
        assert_eq!(matching_ids(&reader, "hello_world alpha.beta")?, vec![1]);
        assert_eq!(matching_ids(&reader, "getUserName")?, vec![1]);
        assert!(matching_ids(&reader, "user")?.is_empty());
    }
    Ok(())
}

#[test]
fn icu_modes_support_mixed_language_phrase_queries() -> anyhow::Result<()> {
    for tokenizer in ["icu", "icu/split"] {
        let reader = build_reader(
            tokenizer,
            &[
                (1, "中文 Rust 搜索"),
                (2, "中文 Java Rust 搜索"),
                (3, "中文 Rust 高级 搜索"),
            ],
        )?;

        assert_eq!(
            phrase_ids(&reader, "中文 rust 搜索")?,
            vec![1],
            "{tokenizer}"
        );
    }
    Ok(())
}

#[test]
fn icu_split_positions_support_identifier_phrases() -> anyhow::Result<()> {
    let reader = build_reader(
        "icu/split",
        &[
            (1, "中文 hello_world 搜索"),
            (2, "中文 hello brave world 搜索"),
            (3, "中文 world hello 搜索"),
        ],
    )?;

    assert_eq!(phrase_ids(&reader, "中文 hello world 搜索")?, vec![1]);
    Ok(())
}

#[test]
fn icu_modes_apply_configured_english_filters() -> anyhow::Result<()> {
    for tokenizer in ["icu", "icu/split"] {
        let options = HashMap::from([("tokenizer".to_string(), tokenizer.to_string())]);
        let config = FullTextIndexConfig::new().tokenizer(TokenizerConfig::from_options(&options)?);
        let mut writer = FullTextIndexWriter::new(config)?;
        writer.add_document(1, "中文 The runners visited cafés")?;
        writer.add_document(2, "中文 other text")?;
        let mut bytes = Vec::new();
        writer.write(&mut PosWriter::new(&mut bytes))?;
        let reader = FullTextIndexReader::open(SliceReader::new(bytes))?;

        assert_eq!(
            matching_ids(&reader, "中文 RUNNER cafe")?,
            vec![1],
            "{tokenizer}"
        );
        assert!(matching_ids(&reader, "the")?.is_empty(), "{tokenizer}");
    }
    Ok(())
}
