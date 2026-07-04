import ctypes
from ctypes import c_float, c_int64, c_size_t, c_uint8, c_void_p

from ._ffi import (
    READ_AT_FN,
    PaimonFtindexInputFile,
    check_ptr,
    check_status,
    lib,
)


class FullTextIndexReader:
    def __init__(self, input_):
        self._closed = False
        self._input = input_

        @READ_AT_FN
        def read_at_fn(ctx, pos, buf, length):
            try:
                data = input_.pread(int(pos), int(length))
                if len(data) != length:
                    return -1
                ctypes.memmove(buf, data, length)
                return 0
            except Exception:
                return -1

        self._read_at_fn = read_at_fn
        native_input = PaimonFtindexInputFile(c_void_p(0), read_at_fn)
        self._ptr = check_ptr(lib.paimon_ftindex_reader_open(native_input))

    def search(self, query, limit=10, filter_bytes=None):
        if self._closed:
            raise RuntimeError("FullTextIndexReader is closed")
        query_json = query.to_json() if hasattr(query, "to_json") else str(query)
        capacity = int(limit)
        row_ids = (c_int64 * capacity)()
        scores = (c_float * capacity)()
        result_len = c_size_t()
        if filter_bytes is None:
            status = lib.paimon_ftindex_reader_search_json(
                self._ptr,
                query_json.encode("utf-8"),
                capacity,
                row_ids,
                scores,
                capacity,
                ctypes.byref(result_len),
            )
        else:
            filter_bytes = bytes(filter_bytes)
            filter_buf = (c_uint8 * len(filter_bytes)).from_buffer_copy(filter_bytes)
            status = lib.paimon_ftindex_reader_search_json_with_roaring_filter(
                self._ptr,
                query_json.encode("utf-8"),
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
