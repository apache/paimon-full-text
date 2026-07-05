use crate::error::{FtIndexError, Result};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub enum MatchOperator {
    #[default]
    Or,
    And,
}

impl<'de> Deserialize<'de> for MatchOperator {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim() {
            "Or" | "or" | "OR" => Ok(Self::Or),
            "And" | "and" | "AND" => Ok(Self::And),
            _ => Err(DeError::custom(format!(
                "invalid full-text query operator: {value}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum BooleanOccur {
    Should,
    Must,
    MustNot,
}

impl<'de> Deserialize<'de> for BooleanOccur {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim() {
            "Should" | "should" | "SHOULD" => Ok(Self::Should),
            "Must" | "must" | "MUST" => Ok(Self::Must),
            "MustNot" | "must_not" | "MUST_NOT" | "mustnot" | "MUSTNOT" => Ok(Self::MustNot),
            _ => Err(DeError::custom(format!(
                "invalid boolean query occur: {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FullTextQuery {
    Match {
        column: String,
        #[serde(alias = "query")]
        terms: String,
        #[serde(default)]
        operator: MatchOperator,
        #[serde(default = "default_boost")]
        boost: f32,
        #[serde(default = "default_fuzziness", deserialize_with = "deserialize_fuzziness")]
        fuzziness: Option<u8>,
        #[serde(default = "default_max_expansions", alias = "maxExpansions")]
        max_expansions: usize,
        #[serde(default, alias = "prefixLength")]
        prefix_length: u32,
    },
    #[serde(alias = "phrase")]
    MatchPhrase {
        column: String,
        #[serde(alias = "query")]
        terms: String,
        #[serde(default)]
        slop: u32,
    },
    Boolean {
        #[serde(default)]
        should: Vec<FullTextQuery>,
        #[serde(default)]
        must: Vec<FullTextQuery>,
        #[serde(default)]
        must_not: Vec<FullTextQuery>,
        #[serde(default)]
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

fn default_fuzziness() -> Option<u8> {
    Some(0)
}

fn default_max_expansions() -> usize {
    50
}

fn deserialize_fuzziness<'de, D>(deserializer: D) -> std::result::Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(value)) if value.eq_ignore_ascii_case("auto") => Ok(None),
        Some(serde_json::Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u8::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| DeError::custom("fuzziness must be an unsigned byte")),
        Some(value) => Err(DeError::custom(format!("invalid fuzziness: {value}"))),
    }
}

impl FullTextQuery {
    pub fn match_query(terms: impl Into<String>, column: impl Into<String>) -> Self {
        Self::Match {
            column: column.into(),
            terms: terms.into(),
            operator: MatchOperator::Or,
            boost: 1.0,
            fuzziness: Some(0),
            max_expansions: 50,
            prefix_length: 0,
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
