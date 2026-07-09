# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

import ctypes
import os
import platform
from ctypes import (
    CFUNCTYPE,
    POINTER,
    Structure,
    c_char_p,
    c_float,
    c_int,
    c_int64,
    c_size_t,
    c_uint8,
    c_uint64,
    c_void_p,
)


def _lib_name():
    system = platform.system()
    if system == "Darwin":
        return "libpaimon_ftindex_ffi.dylib"
    if system == "Windows":
        return "paimon_ftindex_ffi.dll"
    return "libpaimon_ftindex_ffi.so"


def _load_library():
    lib_name = _lib_name()
    env_path = os.environ.get("PAIMON_FTINDEX_LIB_PATH")
    if env_path:
        if os.path.isfile(env_path):
            return ctypes.CDLL(env_path)
        candidate = os.path.join(env_path, lib_name)
        if os.path.isfile(candidate):
            return ctypes.CDLL(candidate)

    pkg_dir = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(pkg_dir, lib_name),
        os.path.join(pkg_dir, "..", "..", "target", "debug", lib_name),
        os.path.join(pkg_dir, "..", "..", "target", "debug", "deps", lib_name),
        os.path.join(pkg_dir, "..", "..", "target", "release", lib_name),
        os.path.join(pkg_dir, "..", "..", "target", "release", "deps", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "debug", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "debug", "deps", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "release", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "release", "deps", lib_name),
    ]
    for candidate in candidates:
        candidate = os.path.abspath(candidate)
        if os.path.isfile(candidate):
            return ctypes.CDLL(candidate)
    return ctypes.CDLL(lib_name)


lib = _load_library()

WRITE_FN = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t)
FLUSH_FN = CFUNCTYPE(c_int, c_void_p)
PREAD_FN = CFUNCTYPE(c_int, c_void_p, c_uint64, POINTER(c_uint8), c_size_t)


class PaimonFtindexOutputFile(Structure):
    _fields_ = [
        ("ctx", c_void_p),
        ("write_fn", WRITE_FN),
        ("flush_fn", FLUSH_FN),
    ]


class PaimonFtindexInputFile(Structure):
    _fields_ = [
        ("ctx", c_void_p),
        ("pread_fn", PREAD_FN),
    ]


class PaimonFtindexReadMetrics(Structure):
    _fields_ = [
        ("pread_calls", c_uint64),
        ("pread_ranges", c_uint64),
        ("pread_bytes", c_uint64),
        ("cache_hits", c_uint64),
        ("cache_misses", c_uint64),
        ("cache_evictions", c_uint64),
        ("cached_blocks", c_uint64),
    ]


lib.paimon_ftindex_last_error.argtypes = []
lib.paimon_ftindex_last_error.restype = c_char_p

lib.paimon_ftindex_writer_open.argtypes = [POINTER(c_char_p), POINTER(c_char_p), c_size_t]
lib.paimon_ftindex_writer_open.restype = c_void_p

lib.paimon_ftindex_writer_add_document.argtypes = [c_void_p, c_int64, c_char_p]
lib.paimon_ftindex_writer_add_document.restype = c_int

lib.paimon_ftindex_writer_add_document_fields.argtypes = [
    c_void_p,
    c_int64,
    POINTER(c_char_p),
    POINTER(c_char_p),
    c_size_t,
]
lib.paimon_ftindex_writer_add_document_fields.restype = c_int

lib.paimon_ftindex_writer_write_index.argtypes = [c_void_p, PaimonFtindexOutputFile]
lib.paimon_ftindex_writer_write_index.restype = c_int

lib.paimon_ftindex_writer_free.argtypes = [c_void_p]
lib.paimon_ftindex_writer_free.restype = None

lib.paimon_ftindex_reader_open.argtypes = [PaimonFtindexInputFile]
lib.paimon_ftindex_reader_open.restype = c_void_p

lib.paimon_ftindex_reader_search.argtypes = [
    c_void_p,
    c_char_p,
    c_size_t,
    POINTER(c_int64),
    POINTER(c_float),
    c_size_t,
    POINTER(c_size_t),
]
lib.paimon_ftindex_reader_search.restype = c_int

lib.paimon_ftindex_reader_search_with_roaring_filter.argtypes = [
    c_void_p,
    c_char_p,
    c_size_t,
    POINTER(c_uint8),
    c_size_t,
    POINTER(c_int64),
    POINTER(c_float),
    c_size_t,
    POINTER(c_size_t),
]
lib.paimon_ftindex_reader_search_with_roaring_filter.restype = c_int

lib.paimon_ftindex_reader_prewarm.argtypes = [c_void_p]
lib.paimon_ftindex_reader_prewarm.restype = c_int

lib.paimon_ftindex_reader_read_metrics.argtypes = [
    c_void_p,
    POINTER(PaimonFtindexReadMetrics),
]
lib.paimon_ftindex_reader_read_metrics.restype = c_int

lib.paimon_ftindex_reader_free.argtypes = [c_void_p]
lib.paimon_ftindex_reader_free.restype = None


def check_status(status):
    if status != 0:
        err = lib.paimon_ftindex_last_error()
        if err:
            raise RuntimeError(err.decode("utf-8"))
        raise RuntimeError("native full-text index call failed")


def check_ptr(ptr):
    if not ptr:
        err = lib.paimon_ftindex_last_error()
        if err:
            raise RuntimeError(err.decode("utf-8"))
        raise RuntimeError("native full-text index returned null")
    return ptr
