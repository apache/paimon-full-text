/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

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
