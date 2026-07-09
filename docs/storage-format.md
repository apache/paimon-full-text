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

- `metadata.config`
- `metadata.document_count`
- `metadata.tantivy_version`
- `files[]`: Tantivy file name, body-relative offset, and length

Readers first read the 16-byte fixed prefix and JSON header. The Rust reader
then exposes listed Tantivy files through a read-only seek-on-demand directory,
so segment file bytes are fetched by positional reads only when Tantivy asks for
the corresponding byte range. Readers reject headers larger than 16 MiB before
allocating the header buffer.

The body stores Tantivy segment files directly. Readers reject index files whose
recorded `metadata.tantivy_version` does not match the linked Tantivy runtime.
