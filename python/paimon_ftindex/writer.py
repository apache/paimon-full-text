import ctypes
from ctypes import c_char_p, c_void_p

from ._ffi import (
    FLUSH_FN,
    WRITE_FN,
    PaimonFtindexOutputFile,
    check_ptr,
    check_status,
    lib,
)


class FullTextIndexWriter:
    def __init__(self, options=None):
        options = options or {}
        self._closed = False
        self._output_refs = None
        keys = [str(k).encode("utf-8") for k in options.keys()]
        values = [str(v).encode("utf-8") for v in options.values()]
        key_array = (c_char_p * len(keys))(*keys) if keys else None
        value_array = (c_char_p * len(values))(*values) if values else None
        self._ptr = check_ptr(
            lib.paimon_ftindex_writer_open(key_array, value_array, len(keys))
        )

    def add_document(self, row_id, text):
        if self._closed:
            raise RuntimeError("FullTextIndexWriter is closed")
        check_status(
            lib.paimon_ftindex_writer_add_document(
                self._ptr, int(row_id), str(text).encode("utf-8")
            )
        )

    def write(self, output):
        if self._closed:
            raise RuntimeError("FullTextIndexWriter is closed")

        @WRITE_FN
        def write_fn(ctx, buf, length):
            try:
                data = ctypes.string_at(buf, length)
                output.write(data)
                return 0
            except Exception:
                return -1

        @FLUSH_FN
        def flush_fn(ctx):
            try:
                flush = getattr(output, "flush", None)
                if flush is not None:
                    flush()
                return 0
            except Exception:
                return -1

        self._output_refs = (write_fn, flush_fn)
        native_output = PaimonFtindexOutputFile(
            c_void_p(0),
            write_fn,
            flush_fn,
        )
        check_status(lib.paimon_ftindex_writer_write_index(self._ptr, native_output))

    def close(self):
        if not self._closed:
            self._closed = True
            if self._ptr:
                lib.paimon_ftindex_writer_free(self._ptr)
                self._ptr = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def __del__(self):
        self.close()
