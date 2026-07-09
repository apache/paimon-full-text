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

# Java API

Package:

```text
org.apache.paimon.index.fulltext
```

Main classes:

- `FullTextIndexWriter`
- `FullTextIndexReader`
- `FullTextSearchResult`
- `FullTextIndexInput`
- `FullTextIndexOutput`

Example:

```java
Map<String, String> options = Collections.singletonMap("text-fields", "title,body");
try (FullTextIndexWriter writer = FullTextIndexWriter.create(options)) {
    Map<String, String> fields = new LinkedHashMap<>();
    fields.put("title", "Apache Paimon");
    fields.put("body", "lake storage");
    writer.addDocument(1L, fields);
    writer.writeIndex(output);
}

try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
    reader.prewarm();
    FullTextSearchResult result =
            reader.search("{\"match\":{\"query\":\"paimon\"}}", 10);
    FullTextSearchResult filtered =
            reader.search("{\"match\":{\"query\":\"paimno\",\"column\":\"title\",\"fuzziness\":1}}",
                    10,
                    roaringFilterBytes);
    FullTextReadMetrics metrics = reader.readMetrics();
}
```

`search()` accepts the query DSL as a JSON string. `match` supports `column`,
`operator`, `boost`, `fuzziness`, `max_expansions`, and `prefix_length`. If
`column` is omitted, a multi-field index searches all indexed text fields. Use
`"fuzziness":"auto"` for auto fuzziness. Boolean and boost-demotion queries use
the same JSON DSL.

`roaringFilterBytes` must be a serialized 64-bit Roaring bitmap
(`RoaringTreemap`) containing the allowed row ids. The filter is applied during
Tantivy collection, before the top results are selected.

`prewarm()` eagerly initializes the underlying search reader and archive cache
before a query burst. `readMetrics()` returns a snapshot with `preadCalls`,
`preadRanges`, `preadBytes`, `cacheHits`, `cacheMisses`, `cacheEvictions`, and
`cachedBlocks`.

Input reads:

- Implement `FullTextIndexInput.pread(long position, byte[] buffer, int offset,
  int length)` as a single positional read. Rust owns any batching or
  parallelism above this callback.
- The implementation must be safe for concurrent calls. Synchronize inside
  `pread` if the backing input keeps mutable state.

Native loading:

- Set `PAIMON_FTINDEX_JNI_LIB_PATH` to the full path of
  `libpaimon_ftindex_jni.dylib` / `.so`, or
- put the library on `java.library.path` as `paimon_ftindex_jni`.
