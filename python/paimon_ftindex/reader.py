import ctypes
from dataclasses import dataclass
from ctypes import c_float, c_int64, c_size_t, c_uint8, c_void_p

from ._ffi import (
    PREAD_FN,
    PaimonFtindexInputFile,
    PaimonFtindexReadMetrics,
    check_ptr,
    check_status,
    lib,
)


@dataclass(frozen=True)
class FullTextReadMetrics:
    pread_calls: int
    pread_ranges: int
    pread_bytes: int
    cache_hits: int
    cache_misses: int
    cache_evictions: int
    cached_blocks: int


class FullTextIndexReader:
    def __init__(self, input_):
        self._closed = False
        self._input = input_

        @PREAD_FN
        def pread_fn(ctx, pos, buf, length):
            try:
                data = input_.pread(int(pos), int(length))
                if len(data) != length:
                    return -1
                ctypes.memmove(buf, data, length)
                return 0
            except Exception:
                return -1

        self._pread_fn = pread_fn
        native_input = PaimonFtindexInputFile(c_void_p(0), pread_fn)
        self._ptr = check_ptr(lib.paimon_ftindex_reader_open(native_input))

    def search(self, query, limit=10, filter_bytes=None):
        if self._closed:
            raise RuntimeError("FullTextIndexReader is closed")
        query = str(query)
        capacity = int(limit)
        row_ids = (c_int64 * capacity)()
        scores = (c_float * capacity)()
        result_len = c_size_t()
        if filter_bytes is None:
            status = lib.paimon_ftindex_reader_search(
                self._ptr,
                query.encode("utf-8"),
                capacity,
                row_ids,
                scores,
                capacity,
                ctypes.byref(result_len),
            )
        else:
            filter_bytes = bytes(filter_bytes)
            filter_buf = (c_uint8 * len(filter_bytes)).from_buffer_copy(filter_bytes)
            status = lib.paimon_ftindex_reader_search_with_roaring_filter(
                self._ptr,
                query.encode("utf-8"),
                capacity,
                filter_buf,
                len(filter_bytes),
                row_ids,
                scores,
                capacity,
                ctypes.byref(result_len),
            )
        check_status(status)
        size = result_len.value
        return list(row_ids[:size]), list(scores[:size])

    def prewarm(self):
        if self._closed:
            raise RuntimeError("FullTextIndexReader is closed")
        check_status(lib.paimon_ftindex_reader_prewarm(self._ptr))

    def read_metrics(self):
        if self._closed:
            raise RuntimeError("FullTextIndexReader is closed")
        native = PaimonFtindexReadMetrics()
        check_status(
            lib.paimon_ftindex_reader_read_metrics(self._ptr, ctypes.byref(native))
        )
        return FullTextReadMetrics(
            pread_calls=native.pread_calls,
            pread_ranges=native.pread_ranges,
            pread_bytes=native.pread_bytes,
            cache_hits=native.cache_hits,
            cache_misses=native.cache_misses,
            cache_evictions=native.cache_evictions,
            cached_blocks=native.cached_blocks,
        )

    def close(self):
        if not self._closed:
            self._closed = True
            if self._ptr:
                lib.paimon_ftindex_reader_free(self._ptr)
                self._ptr = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def __del__(self):
        self.close()
