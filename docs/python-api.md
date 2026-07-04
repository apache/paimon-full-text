# Python API

Package:

```text
paimon_ftindex
```

Example:

```python
from paimon_ftindex import FullTextIndexReader, FullTextIndexWriter, MatchQuery

with FullTextIndexWriter({"tokenizer": "ngram"}) as writer:
    writer.add_document(1, "Apache Paimon full text search")
    writer.write(output)

with FullTextIndexReader(input_) as reader:
    ids, scores = reader.search(MatchQuery("paimon"), limit=10)
    filtered_ids, filtered_scores = reader.search(
        MatchQuery("paimon"), limit=10, filter_bytes=roaring_filter_bytes
    )
```

`filter_bytes` must be a serialized 64-bit Roaring bitmap (`RoaringTreemap`)
containing the allowed row ids. The filter is applied during Tantivy
collection, before the top results are selected.

The output object must provide:

- `write(bytes)`
- optional `flush()`

The input object must provide:

- `pread(pos: int, length: int) -> bytes`

Native loading:

- Set `PAIMON_FTINDEX_LIB_PATH` to a library file or directory, or
- build `paimon-ftindex-ffi` so the package can discover `target/debug`,
  `target/debug/deps`, `target/release`, or `target/release/deps`.
