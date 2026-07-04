# Implementation Plan: Paimon Full Text Index

## Prerequisites

- [x] Confirm local Rust, Python, and Maven toolchains are available.
- [x] Confirm this repository is the standalone `paimon-full-text` workspace.

## Tasks

### Task 1: Create Rust workspace skeleton

- **Files**: `Cargo.toml`, `core/Cargo.toml`, `ffi/Cargo.toml`, `jni/Cargo.toml`
- **Changes**: Define a top-level Cargo workspace with `core`, `ffi`, and `jni`
  crates. Add Tantivy and serialization dependencies to `core`.
- **Verify**: `cargo metadata --no-deps` succeeds.
- **Dependencies**: None.

### Task 2: Implement Rust core config, query, I/O, and storage

- **Files**: `core/src/*.rs`
- **Changes**: Add tokenizer config parsing, structured query JSON parsing,
  seek/read-write traits, v1 storage envelope, Tantivy writer, and reader.
- **Verify**: `cargo test -p paimon-ftindex-core`.
- **Dependencies**: Task 1.

### Task 3: Add C FFI over Rust core

- **Files**: `ffi/src/lib.rs`, `include/paimon_ftindex.h`
- **Changes**: Expose writer/reader handles, I/O callbacks, result buffers,
  thread-local errors, and panic boundaries.
- **Verify**: `cargo test -p paimon-ftindex-ffi`.
- **Dependencies**: Task 2.

### Task 4: Add Java API and JNI bridge

- **Files**: `java/**`, `jni/src/lib.rs`
- **Changes**: Add Java wrapper classes and JNI methods backed by Rust core.
- **Verify**: `mvn test -pl java` where possible, and `cargo test -p
  paimon-ftindex-jni`.
- **Dependencies**: Task 2.

### Task 5: Add Python ctypes package

- **Files**: `python/**`
- **Changes**: Add ctypes binding, Python query helpers, input/output adapter,
  and unit tests using the local native library.
- **Verify**: `python3 -m pytest python/tests` after building FFI library.
- **Dependencies**: Task 3.

### Task 6: Add repository docs and examples

- **Files**: `README.md`, `docs/storage-format.md`, `docs/java-api.md`,
  `docs/python-api.md`, `docs/paimon-integration.md`
- **Changes**: Document architecture, build commands, API examples, and Paimon
  integration.
- **Verify**: `cargo test --workspace` and basic doc review.
- **Dependencies**: Tasks 2-5.

## Post-Implementation

- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo test --workspace`.
- [ ] Run targeted Python tests if native library builds.
- [ ] Run targeted Java compilation/tests if JNI library builds.
- [ ] Review `git diff`.
