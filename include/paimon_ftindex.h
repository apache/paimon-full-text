#ifndef PAIMON_FTINDEX_H
#define PAIMON_FTINDEX_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PaimonFtindexWriterHandle PaimonFtindexWriterHandle;
typedef struct PaimonFtindexReaderHandle PaimonFtindexReaderHandle;

typedef struct {
    void *ctx;
    int (*write_fn)(void *ctx, const uint8_t *buf, size_t len);
    int (*flush_fn)(void *ctx);
} PaimonFtindexOutputFile;

typedef struct {
    void *ctx;
    /* Must be safe for concurrent calls. Rust owns batching and parallelism. */
    int (*pread_fn)(void *ctx, uint64_t pos, uint8_t *buf, size_t len);
} PaimonFtindexInputFile;

typedef struct {
    uint64_t pread_calls;
    uint64_t pread_ranges;
    uint64_t pread_bytes;
    uint64_t cache_hits;
    uint64_t cache_misses;
    uint64_t cache_evictions;
    uint64_t cached_blocks;
} PaimonFtindexReadMetrics;

const char *paimon_ftindex_last_error(void);

PaimonFtindexWriterHandle *paimon_ftindex_writer_open(
    const char **keys,
    const char **values,
    size_t len);

int paimon_ftindex_writer_add_document(
    PaimonFtindexWriterHandle *writer,
    int64_t row_id,
    const char *text);

int paimon_ftindex_writer_add_document_fields(
    PaimonFtindexWriterHandle *writer,
    int64_t row_id,
    const char **field_names,
    const char **texts,
    size_t len);

int paimon_ftindex_writer_write_index(
    PaimonFtindexWriterHandle *writer,
    PaimonFtindexOutputFile output);

void paimon_ftindex_writer_free(PaimonFtindexWriterHandle *writer);

PaimonFtindexReaderHandle *paimon_ftindex_reader_open(PaimonFtindexInputFile input);

int paimon_ftindex_reader_search(
    PaimonFtindexReaderHandle *reader,
    const char *query,
    size_t limit,
    int64_t *row_ids,
    float *scores,
    size_t capacity,
    size_t *result_len);

int paimon_ftindex_reader_search_with_roaring_filter(
    PaimonFtindexReaderHandle *reader,
    const char *query,
    size_t limit,
    const uint8_t *roaring_filter,
    size_t roaring_filter_len,
    int64_t *row_ids,
    float *scores,
    size_t capacity,
    size_t *result_len);

int paimon_ftindex_reader_prewarm(PaimonFtindexReaderHandle *reader);

int paimon_ftindex_reader_read_metrics(
    PaimonFtindexReaderHandle *reader,
    PaimonFtindexReadMetrics *metrics);

void paimon_ftindex_reader_free(PaimonFtindexReaderHandle *reader);

#ifdef __cplusplus
}
#endif

#endif
