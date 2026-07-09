<!--
  Licensed to the Apache Software Foundation (ASF) under one
  or more contributor license agreements.  See the NOTICE file
  distributed with this work for additional information
  regarding copyright ownership.  The ASF licenses this file
  to you under the Apache License, Version 2.0 (the
  "License"); you may not use this file except in compliance
  with the License.  You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing,
  software distributed under the License is distributed on an
  "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
  KIND, either express or implied.  See the License for the
  specific language governing permissions and limitations
  under the License.
-->

# Paimon Integration

The standalone library is intentionally independent of Paimon core. Paimon
integration should be a thin adapter:

- Pass the query DSL as a JSON string to the standalone reader.
- Implement a Paimon `GlobalIndexerFactory` that delegates to Java
  `FullTextIndexWriter` and `FullTextIndexReader`.
- Pass serialized 64-bit Roaring row-id filters to reader search when another
  index or predicate pushdown has already produced an allowed candidate set.
- Store produced files as global index files.

Suggested index identifier:

```text
fulltext
```

Suggested option namespace:

```text
fulltext.tokenizer
fulltext.text-fields
fulltext.ngram.min-gram
fulltext.ngram.max-gram
fulltext.ngram.prefix-only
fulltext.jieba.search-mode
fulltext.jieba.ordinal-position
fulltext.lower-case
fulltext.max-token-length
fulltext.ascii-folding
fulltext.stem
fulltext.language
fulltext.remove-stop-words
fulltext.stop-words
fulltext.with-position
```

The standalone library accepts both unprefixed keys and `fulltext.` prefixed
keys.

Query JSON supports `match`, `multi_match`, `match_phrase`, `boolean`, and
boost-demotion queries. `match` accepts `fuzziness`, `max_expansions`, and
`prefix_length`; use `fuzziness: "auto"` for automatic edit distance. If
`match.column` is omitted, the native reader searches every text field
configured in the index, so Paimon can derive native text fields from its
extra-fields mechanism without adding a user-facing `multi_match` requirement.
