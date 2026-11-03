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

// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The Lance Authors
// Adapted from Lance's streaming ICU tokenizer for Tantivy's tokenizer API.

use icu_segmenter::iterators::WordBreakIterator;
use icu_segmenter::options::WordBreakInvariantOptions;
use icu_segmenter::scaffold::Utf8;
use icu_segmenter::{WordSegmenter, WordSegmenterBorrowed};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

#[derive(Clone)]
pub(crate) struct IcuTokenizer {
    segmenter: WordSegmenterBorrowed<'static>,
    split_on_non_alphanumeric: bool,
}

impl Default for IcuTokenizer {
    fn default() -> Self {
        Self {
            segmenter: WordSegmenter::new_dictionary(WordBreakInvariantOptions::default()),
            split_on_non_alphanumeric: false,
        }
    }
}

impl IcuTokenizer {
    pub(crate) fn with_simple_split(mut self) -> Self {
        self.split_on_non_alphanumeric = true;
        self
    }
}

impl Tokenizer for IcuTokenizer {
    type TokenStream<'a> = IcuTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        IcuTokenStream {
            text,
            boundaries: self.segmenter.segment_str(text),
            offset: 0,
            segment_end: 0,
            split_on_non_alphanumeric: self.split_on_non_alphanumeric,
            next_position: 0,
            token: Token::default(),
        }
    }
}

pub(crate) struct IcuTokenStream<'a> {
    text: &'a str,
    boundaries: WordBreakIterator<'static, 'a, Utf8>,
    offset: usize,
    segment_end: usize,
    split_on_non_alphanumeric: bool,
    next_position: usize,
    token: Token,
}

impl TokenStream for IcuTokenStream<'_> {
    fn advance(&mut self) -> bool {
        loop {
            if self.offset == self.segment_end {
                let Some(end) = self.boundaries.next() else {
                    return false;
                };
                self.segment_end = end;
                if self.offset == end {
                    continue;
                }
            }
            let start = self.offset;
            let mut end = self.segment_end;
            self.offset = self.segment_end;
            if self.split_on_non_alphanumeric {
                for (relative, character) in self.text[start..end].char_indices() {
                    if !character.is_alphanumeric() {
                        end = start + relative;
                        self.offset = end + character.len_utf8();
                        break;
                    }
                }
            }
            if start == end || !self.text[start..end].chars().any(char::is_alphanumeric) {
                continue;
            }
            self.token.offset_from = start;
            self.token.offset_to = end;
            self.token.position = self.next_position;
            self.token.position_length = 1;
            self.token.text.clear();
            self.token.text.push_str(&self.text[start..end]);
            self.next_position += 1;
            return true;
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(tokenizer: &mut IcuTokenizer, text: &str) -> Vec<Token> {
        let mut stream = tokenizer.token_stream(text);
        let mut tokens = Vec::new();
        stream.process(&mut |token| tokens.push(token.clone()));
        tokens
    }

    #[test]
    fn icu_modes_segment_mixed_text_with_utf8_offsets() {
        for mut tokenizer in [
            IcuTokenizer::default(),
            IcuTokenizer::default().with_simple_split(),
        ] {
            let tokens = tokens(&mut tokenizer, "Hello，世界！Rust");
            assert_eq!(
                tokens
                    .iter()
                    .map(|token| (
                        token.text.as_str(),
                        token.offset_from,
                        token.offset_to,
                        token.position,
                        token.position_length,
                    ))
                    .collect::<Vec<_>>(),
                vec![
                    ("Hello", 0, 5, 0, 1),
                    ("世界", 8, 14, 1, 1),
                    ("Rust", 17, 21, 2, 1)
                ]
            );
        }
    }

    #[test]
    fn icu_modes_preserve_or_split_word_internal_punctuation() {
        for (mut tokenizer, expected) in [
            (
                IcuTokenizer::default(),
                vec!["hello_world", "alpha.beta", "don't", "getUserName"],
            ),
            (
                IcuTokenizer::default().with_simple_split(),
                vec!["hello", "world", "alpha", "beta", "don", "t", "getUserName"],
            ),
        ] {
            let tokens = tokens(&mut tokenizer, "hello_world alpha.beta don't getUserName");
            assert_eq!(
                tokens
                    .iter()
                    .map(|token| token.text.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn icu_split_tracks_offsets_and_positions_across_repeated_delimiters() {
        let tokens = tokens(
            &mut IcuTokenizer::default().with_simple_split(),
            "foo_bar__baz-alpha.beta",
        );
        assert_eq!(
            tokens
                .iter()
                .map(|token| (
                    token.text.as_str(),
                    token.offset_from,
                    token.offset_to,
                    token.position
                ))
                .collect::<Vec<_>>(),
            vec![
                ("foo", 0, 3, 0),
                ("bar", 4, 7, 1),
                ("baz", 9, 12, 2),
                ("alpha", 13, 18, 3),
                ("beta", 19, 23, 4),
            ]
        );
    }

    #[test]
    fn icu_modes_ignore_empty_and_nonword_text_and_reset_between_documents() {
        for mut tokenizer in [
            IcuTokenizer::default(),
            IcuTokenizer::default().with_simple_split(),
        ] {
            for text in ["Hello世界Rust", "", " \t\n，。🙂-_", "Hello世界Rust"] {
                let tokens = tokens(&mut tokenizer, text);
                let expected = if text == "Hello世界Rust" {
                    vec!["Hello", "世界", "Rust"]
                } else {
                    vec![]
                };
                assert_eq!(
                    tokens
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
                if let Some(first) = tokens.first() {
                    assert_eq!(first.position, 0);
                }
            }
        }
    }

    #[test]
    fn icu_stream_resets_fields_modified_by_filters() {
        let mut tokenizer = IcuTokenizer::default().with_simple_split();
        let mut stream = tokenizer.token_stream("_hello__世界_");
        assert!(stream.advance());
        assert_eq!(stream.token().text, "hello");
        let token = stream.token_mut();
        token.text.push_str("changed");
        token.position = 99;
        token.position_length = 7;
        token.offset_to = 999;

        assert!(stream.advance());
        assert_eq!(
            stream.token(),
            &Token {
                text: "世界".to_string(),
                offset_from: 8,
                offset_to: 14,
                position: 1,
                position_length: 1,
            }
        );
        assert!(!stream.advance());
        assert!(!stream.advance());
    }
}
