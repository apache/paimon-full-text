# Java API

Package:

```text
org.apache.paimon.index.fulltext
```

Main classes:

- `FullTextIndexWriter`
- `FullTextIndexReader`
- `FullTextQuery`
- `FullTextSearchResult`
- `FullTextIndexInput`
- `FullTextIndexOutput`

Example:

```java
try (FullTextIndexWriter writer = FullTextIndexWriter.create(Collections.emptyMap())) {
    writer.addDocument(1L, "Apache Paimon full text search");
    writer.writeIndex(output);
}

try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
    FullTextSearchResult result = reader.search(FullTextQuery.match("paimon", "text"), 10);
}
```

Native loading:

- Set `PAIMON_FTINDEX_JNI_LIB_PATH` to the full path of
  `libpaimon_ftindex_jni.dylib` / `.so`, or
- put the library on `java.library.path` as `paimon_ftindex_jni`.
