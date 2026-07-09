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
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) enum MatchOperator {
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
pub(crate) enum BooleanOccur {
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
pub(crate) enum QuerySpec {
    Match {
        #[serde(default)]
        column: Option<String>,
        #[serde(alias = "query")]
        terms: String,
        #[serde(default)]
        operator: MatchOperator,
        #[serde(default = "default_boost")]
        boost: f32,
        #[serde(
            default = "default_fuzziness",
            deserialize_with = "deserialize_fuzziness"
        )]
        fuzziness: Option<u8>,
        #[serde(default = "default_max_expansions", alias = "maxExpansions")]
        max_expansions: usize,
        #[serde(default, alias = "prefixLength")]
        prefix_length: u32,
    },
    MultiMatch {
        #[serde(alias = "query")]
        terms: String,
        columns: Vec<String>,
        #[serde(default, alias = "boost")]
        boosts: Vec<f32>,
        #[serde(default)]
        operator: MatchOperator,
        #[serde(
            default = "default_fuzziness",
            deserialize_with = "deserialize_fuzziness"
        )]
        fuzziness: Option<u8>,
        #[serde(default = "default_max_expansions", alias = "maxExpansions")]
        max_expansions: usize,
        #[serde(default, alias = "prefixLength")]
        prefix_length: u32,
    },
    #[serde(alias = "phrase")]
    MatchPhrase {
        #[serde(default)]
        column: Option<String>,
        #[serde(alias = "query")]
        terms: String,
        #[serde(default)]
        slop: u32,
    },
    Boolean {
        #[serde(default)]
        should: Vec<QuerySpec>,
        #[serde(default)]
        must: Vec<QuerySpec>,
        #[serde(default)]
        must_not: Vec<QuerySpec>,
        #[serde(default)]
        queries: Vec<(BooleanOccur, QuerySpec)>,
    },
    Boost {
        positive: Box<QuerySpec>,
        negative: Box<QuerySpec>,
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

impl QuerySpec {
    pub(crate) fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(FtIndexError::from)
    }
}
