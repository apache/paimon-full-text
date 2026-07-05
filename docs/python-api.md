# Python API

Package:

```text
paimon_ftindex
```

Example:

```python
from paimon_ftindex import (
    FullTextIndexReader,
    FullTextIndexWriter,
)

with FullTextIndexWriter({"text-fields": "title,body"}) as writer:
    writer.add_document_fields(
        1,
        {
            "title": "Apache Paimon",
            "body": "lake storage",
        },
    )
    writer.write(output)

with FullTextIndexReader(input_) as reader:
    reader.prewarm()
    ids, scores = reader.search(
        '{"match":{"query":"paimon"}}',
        limit=10,
    )
    filtered_ids, filtered_scores = reader.search(
        '{"match":{"query":"paimno","column":"title","fuzziness":1}}',
        limit=10,
        filter_bytes=roaring_filter_bytes,
    )
    metrics = reader.read_metrics()
```

`search()` accepts the query DSL as a JSON string. `match` supports `column`,
`operator`, `boost`, `fuzziness`, `max_expansions`, and `prefix_length`. If
`column` is omitted, a multi-field index searches all indexed text fields. Use
`"fuzziness":"auto"` for auto fuzziness. `match_phrase` requires the index to
be created with `with-position=true`.

`filter_bytes` must be a serialized 64-bit Roaring bitmap (`RoaringTreemap`)
containing the allowed row ids. The filter is applied during Tantivy
collection, before the top results are selected.

`prewarm()` eagerly initializes the underlying search reader and archive cache
before a query burst. `read_metrics()` returns a snapshot with `pread_calls`,
`pread_ranges`, `pread_bytes`, `cache_hits`, `cache_misses`, `cache_evictions`,
and `cached_blocks`.

The output object must provide:

- `write(bytes)`
- optional `flush()`

The input object must provide:

- `pread(pos: int, length: int) -> bytes`

`pread` must be safe for concurrent calls if the backing input keeps mutable
state. Rust owns batching and parallelism above this single-read callback.

Native loading:

- Set `PAIMON_FTINDEX_LIB_PATH` to a library file or directory, or
- build `paimon-ftindex-ffi` so the package can discover `target/debug`,
  `target/debug/deps`, `target/release`, or `target/release/deps`.
