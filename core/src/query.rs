use crate::error::{FtIndexError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchOperator {
    #[default]
    Or,
    And,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BooleanOccur {
    Should,
    Must,
    MustNot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FullTextQuery {
    Match {
        column: String,
        terms: String,
        #[serde(default)]
        operator: MatchOperator,
        #[serde(default = "default_boost")]
        boost: f32,
    },
    MatchPhrase {
        column: String,
        terms: String,
        #[serde(default)]
        slop: u32,
    },
    Boolean {
        queries: Vec<(BooleanOccur, FullTextQuery)>,
    },
    Boost {
        positive: Box<FullTextQuery>,
        negative: Box<FullTextQuery>,
        #[serde(default = "default_negative_boost")]
        negative_boost: f32,
    },
}

fn default_boost() -> f32 {
    1.0
}

fn default_negative_boost() -> f32 {
    0.5
}

impl FullTextQuery {
    pub fn match_query(terms: impl Into<String>, column: impl Into<String>) -> Self {
        Self::Match {
            column: column.into(),
            terms: terms.into(),
            operator: MatchOperator::Or,
            boost: 1.0,
        }
    }

    pub fn phrase(terms: impl Into<String>, column: impl Into<String>) -> Self {
        Self::MatchPhrase {
            column: column.into(),
            terms: terms.into(),
            slop: 0,
        }
    }

    pub fn operator_and(mut self) -> Self {
        if let Self::Match { operator, .. } = &mut self {
            *operator = MatchOperator::And;
        }
        self
    }

    pub fn operator_or(mut self) -> Self {
        if let Self::Match { operator, .. } = &mut self {
            *operator = MatchOperator::Or;
        }
        self
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(FtIndexError::from)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(FtIndexError::from)
    }
}
