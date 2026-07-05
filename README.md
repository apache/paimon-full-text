# Apache Paimon Full Text Index

Standalone Tantivy-based full-text index library for Apache Paimon-style data
lake storage. The project follows the same shape as `paimon-vector-index`:

- `core`: Rust implementation and v1 storage format.
- `ffi`: C ABI over the Rust core.
- `jni`: Java JNI bridge over the Rust core.
- `java`: public Java API.
- `python`: Python ctypes API over the C ABI.

The index file is self-describing. Readers only need positional `pread` I/O
and do not depend on Paimon manifest metadata.

## Current Status

Implemented:

- Rust writer, reader, v1 envelope, and search.
- Single-field, multi-field, repeated-field string-array, and dotted-path text
  fields.
- Match, fuzzy match (`fuzziness`, `auto`, `max_expansions`,
  `prefix_length`), phrase, boolean, multi-match, and boost-demotion queries.
- C FFI writer/reader/search with query JSON strings, including serialized
  64-bit Roaring row-id filters.
- Java API and JNI bridge.
- Python ctypes package.
- Cross-boundary round-trip tests for Rust core, FFI, Java/JNI, and Python.

Supported tokenizers in this first implementation:

- `default`
- `simple`
- `whitespace`
- `raw`
- `ngram`
- `jieba`

Default tokenizer behavior uses English full-text defaults: lower-case,
stemming, stop-word removal, ASCII folding, max token length 40, and no
positions unless `with-position=true` is set. Phrase search requires positions.

Readers expose archived Tantivy files through a seek-on-demand directory, so
opening an index reads the envelope and Tantivy metadata without loading all
segment files into memory.

## Build

```bash
cargo test -p paimon-ftindex-core
cargo test -p paimon-ftindex-ffi
cargo build -p paimon-ftindex-ffi
cargo build -p paimon-ftindex-jni
mvn -q -f java/pom.xml test
PYTHONPATH=python python3 -m pytest -q python/tests
```

## Rust Example

```rust
use paimon_ftindex_core::io::{PosWriter, SliceReader};
use paimon_ftindex_core::{FullTextIndexConfig, FullTextIndexReader, FullTextIndexWriter};

let mut writer = FullTextIndexWriter::new(FullTextIndexConfig::new())?;
writer.add_document(1, "Apache Paimon full text search")?;

let mut bytes = Vec::new();
writer.write(&mut PosWriter::new(&mut bytes))?;

let reader = FullTextIndexReader::open(SliceReader::new(bytes))?;
reader.prewarm()?;
let result = reader.search(r#"{"match":{"query":"paimon","column":"text"}}"#, 10)?;
```

Multi-field indexes can be configured with named fields:

```rust
let config = FullTextIndexConfig::new().with_text_fields(["title", "body"]);
let mut writer = FullTextIndexWriter::new(config)?;
writer.add_document_fields(
    1,
    [
        ("title".to_string(), "Apache Paimon".to_string()),
        ("body".to_string(), "lake storage".to_string()),
    ],
)?;
```

When a `match` query omits `column`, the reader searches all indexed text
fields. This lets a Paimon adapter populate extra fields internally without
requiring callers to build a `multi_match` query.

To restrict search to an upstream candidate set, pass a serialized
`RoaringTreemap` of allowed row ids:

```rust
let filtered = reader.search_with_roaring_filter(
    r#"{"match":{"query":"paimon","column":"text"}}"#,
    10,
    roaring_filter_bytes,
)?;
```

## Python Example

```python
from io import BytesIO
from paimon_ftindex import FullTextIndexReader, FullTextIndexWriter

out = BytesIO()
with FullTextIndexWriter({"text-fields": "title,body"}) as writer:
    writer.add_document_fields(1, {"title": "Apache Paimon", "body": "lake storage"})
    writer.write(out)

class Input:
    def __init__(self, data):
        self.data = data
    def pread(self, pos, length):
        return self.data[pos:pos + length]

with FullTextIndexReader(Input(out.getvalue())) as reader:
    reader.prewarm()
    ids, scores = reader.search('{"match":{"query":"paimon"}}', limit=10)
    filtered_ids, filtered_scores = reader.search(
        '{"match":{"query":"paimno","column":"title","fuzziness":1}}',
        limit=10,
        filter_bytes=roaring_filter_bytes,
    )
    metrics = reader.read_metrics()
```

Search APIs accept the query DSL as a JSON string.

`prewarm()` eagerly initializes the underlying search reader and archive cache
before a query burst. `read_metrics()` reports positional read calls/bytes and
archive cache hit/miss counters for tuning reader reuse and object-store access
patterns.
