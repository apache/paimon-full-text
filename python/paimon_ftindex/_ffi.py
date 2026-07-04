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
        os.path.join(pkg_dir, "..", "..", "target", "release", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "debug", lib_name),
        os.path.join(pkg_dir, "..", "..", "..", "target", "release", lib_name),
    ]
    for candidate in candidates:
        candidate = os.path.abspath(candidate)
        if os.path.isfile(candidate):
            return ctypes.CDLL(candidate)
    return ctypes.CDLL(lib_name)


lib = _load_library()

WRITE_FN = CFUNCTYPE(c_int, c_void_p, POINTER(c_uint8), c_size_t)
FLUSH_FN = CFUNCTYPE(c_int, c_void_p)
READ_AT_FN = CFUNCTYPE(c_int, c_void_p, c_uint64, POINTER(c_uint8), c_size_t)


class PaimonFtindexOutputFile(Structure):
    _fields_ = [
        ("ctx", c_void_p),
        ("write_fn", WRITE_FN),
        ("flush_fn", FLUSH_FN),
    ]


class PaimonFtindexInputFile(Structure):
    _fields_ = [
        ("ctx", c_void_p),
        ("read_at_fn", READ_AT_FN),
    ]


lib.paimon_ftindex_last_error.argtypes = []
lib.paimon_ftindex_last_error.restype = c_char_p

lib.paimon_ftindex_writer_open.argtypes = [POINTER(c_char_p), POINTER(c_char_p), c_size_t]
lib.paimon_ftindex_writer_open.restype = c_void_p

lib.paimon_ftindex_writer_add_document.argtypes = [c_void_p, c_int64, c_char_p]
lib.paimon_ftindex_writer_add_document.restype = c_int

lib.paimon_ftindex_writer_write_index.argtypes = [c_void_p, PaimonFtindexOutputFile]
lib.paimon_ftindex_writer_write_index.restype = c_int

lib.paimon_ftindex_writer_free.argtypes = [c_void_p]
lib.paimon_ftindex_writer_free.restype = None

lib.paimon_ftindex_reader_open.argtypes = [PaimonFtindexInputFile]
lib.paimon_ftindex_reader_open.restype = c_void_p

lib.paimon_ftindex_reader_search_json.argtypes = [
    c_void_p,
    c_char_p,
    c_size_t,
    POINTER(c_int64),
    POINTER(c_float),
    c_size_t,
    POINTER(c_size_t),
]
lib.paimon_ftindex_reader_search_json.restype = c_int

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
