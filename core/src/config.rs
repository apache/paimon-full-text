use crate::tokenizer::TokenizerConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FullTextIndexConfig {
    pub row_id_field: String,
    pub text_field: String,
    pub tokenizer: TokenizerConfig,
}

impl Default for FullTextIndexConfig {
    fn default() -> Self {
        Self {
            row_id_field: "row_id".to_string(),
            text_field: "text".to_string(),
            tokenizer: TokenizerConfig::default(),
        }
    }
}

impl FullTextIndexConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tokenizer(mut self, tokenizer: TokenizerConfig) -> Self {
        self.tokenizer = tokenizer;
        self
    }

    pub fn with_positions(mut self, with_positions: bool) -> Self {
        self.tokenizer.with_position = with_positions;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FullTextIndexMetadata {
    pub format_version: u32,
    pub config: FullTextIndexConfig,
    pub document_count: u64,
    pub tantivy_version: String,
}
