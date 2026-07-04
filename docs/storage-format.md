# Storage Format

Version 1 uses a front-loaded self-describing envelope:

```text
magic:      8 bytes  "PFTIDX01"
version:    u32      1
header_len: u32
header:     JSON metadata and archive directory
body:       concatenated Tantivy files
```

The header contains:

- `metadata.format_version`
- `metadata.config`
- `metadata.document_count`
- `metadata.tantivy_version`
- `files[]`: Tantivy file name, body-relative offset, and length

Readers first read the 16-byte fixed prefix, then the JSON header, then the
listed Tantivy files by positional reads. The current reader loads listed files
into Tantivy `RamDirectory`; a future reader can replace this with a custom
seek-on-demand `Directory` without changing the envelope.
