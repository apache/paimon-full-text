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

use crate::tokenizer::TokenizerConfig;
use crate::{FtIndexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FullTextIndexConfig {
    #[serde(default = "default_row_id_field")]
    pub row_id_field: String,
    #[serde(default = "default_text_fields")]
    pub text_fields: Vec<String>,
    #[serde(default)]
    pub tokenizer: TokenizerConfig,
}

impl Default for FullTextIndexConfig {
    fn default() -> Self {
        Self {
            row_id_field: default_row_id_field(),
            text_fields: default_text_fields(),
            tokenizer: TokenizerConfig::default(),
        }
    }
}

fn default_row_id_field() -> String {
    "row_id".to_string()
}

fn default_text_fields() -> Vec<String> {
    vec!["text".to_string()]
}

impl FullTextIndexConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_options(options: &HashMap<String, String>) -> Result<Self> {
        let tokenizer = TokenizerConfig::from_options(options)?;
        let mut row_id_field = None;
        let mut text_field = None;
        let mut text_fields = None;
        for (raw_key, value) in options {
            let key = normalize_option_key(raw_key);
            match key.as_str() {
                "row-id-field" => row_id_field = Some(clean_name(value, &key)?),
                "text-field" => text_field = Some(clean_name(value, &key)?),
                "text-fields" | "columns" => text_fields = Some(split_text_fields(value)?),
                _ => {}
            }
        }

        let mut fields = text_fields
            .or_else(|| text_field.clone().map(|field| vec![field]))
            .unwrap_or_else(default_text_fields);
        dedup_preserve_order(&mut fields);
        if fields.is_empty() {
            return invalid("text-fields", "must contain at least one field");
        }

        let config = Self {
            row_id_field: row_id_field.unwrap_or_else(default_row_id_field),
            text_fields: fields,
            tokenizer,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn tokenizer(mut self, tokenizer: TokenizerConfig) -> Self {
        self.tokenizer = tokenizer;
        self
    }

    pub fn with_text_fields<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.text_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_positions(mut self, with_positions: bool) -> Self {
        self.tokenizer.with_position = with_positions;
        self
    }

    pub fn indexed_text_fields(&self) -> Vec<&str> {
        let fields = self.text_fields.iter().map(String::as_str).collect();
        dedup_strs(fields)
    }

    pub fn default_text_field(&self) -> &str {
        self.text_fields
            .first()
            .map(String::as_str)
            .unwrap_or("text")
    }

    pub fn validate(&self) -> Result<()> {
        if self.row_id_field.trim().is_empty() {
            return invalid("row-id-field", "must not be empty");
        }
        let mut fields = self.text_fields.clone();
        dedup_preserve_order(&mut fields);
        if fields.is_empty() {
            return invalid("text-fields", "must contain at least one field");
        }
        for field in fields {
            if field.trim().is_empty() {
                return invalid("text-fields", "must not contain empty field names");
            }
            if field == self.row_id_field {
                return invalid("text-fields", "must not contain the row id field");
            }
        }
        self.tokenizer.validate()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FullTextIndexMetadata {
    #[serde(default)]
    pub config: FullTextIndexConfig,
    #[serde(default)]
    pub document_count: u64,
    #[serde(default)]
    pub tantivy_version: String,
}

fn normalize_option_key(raw_key: &str) -> String {
    raw_key
        .strip_prefix("fulltext.")
        .or_else(|| raw_key.strip_prefix("tantivy."))
        .unwrap_or(raw_key)
        .trim()
        .to_lowercase()
        .replace('_', "-")
}

fn clean_name(value: &str, key: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        invalid(key, "must not be empty")
    } else {
        Ok(value.to_string())
    }
}

fn split_text_fields(value: &str) -> Result<Vec<String>> {
    let mut fields = value
        .split([',', ';'])
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    dedup_preserve_order(&mut fields);
    if fields.is_empty() {
        invalid("text-fields", "must contain at least one field")
    } else {
        Ok(fields)
    }
}

fn dedup_preserve_order(fields: &mut Vec<String>) {
    let mut deduped = Vec::with_capacity(fields.len());
    for field in fields.drain(..) {
        if !deduped.contains(&field) {
            deduped.push(field);
        }
    }
    *fields = deduped;
}

fn dedup_strs(fields: Vec<&str>) -> Vec<&str> {
    let mut deduped = Vec::with_capacity(fields.len());
    for field in fields {
        if !deduped.contains(&field) {
            deduped.push(field);
        }
    }
    deduped
}

fn invalid<T>(key: &str, message: &str) -> Result<T> {
    Err(FtIndexError::InvalidOption {
        key: key.to_string(),
        message: message.to_string(),
    })
}
