# TODO: Rust Improvements

This document tracks remaining idiomatic Rust improvements for Mortimer.

## ✅ Completed

- [x] Newtype wrappers for IDs (CommandId, HostId, SessionId)
- [x] From trait conversions between types
- [x] #[must_use] attributes on important functions
- [x] Tracing for observability (structured logging)
- [x] Trait abstraction for backends (HistoryProvider trait)

## 📋 Medium Priority

### 6. Builder Pattern for SearchEngine
**Effort**: ~30 minutes
**Impact**: High usability improvement

Currently `SearchEngine::with_config()` takes 6+ parameters. Replace with builder pattern:

```rust
// Current (verbose)
SearchEngine::with_config(query, case_sensitive, fuzzy, threshold, limit, highlight)

// Better (builder pattern)
SearchEngine::builder()
    .query(query)
    .case_sensitive(true)
    .fuzzy_threshold(0.8)
    .limit(100)
    .build()
```

**Files to modify**: `src/search.rs`

### 7. Property-Based Tests
**Effort**: ~1 hour
**Impact**: Better test coverage, catch edge cases

Add `proptest` to test redaction and search with random inputs:

```rust
proptest! {
    #[test]
    fn test_redaction_idempotent(cmd: String) {
        let once = engine.redact(&cmd)?;
        let twice = engine.redact(&once)?;
        assert_eq!(once, twice);
    }
}
```

**Dependencies**: Add `proptest` to dev-dependencies
**Files to modify**: `src/redaction.rs`, `src/search.rs` test modules

### 8. enum_dispatch for HistoryBackend
**Effort**: ~20 minutes
**Impact**: Zero-cost abstraction (performance)

Replace dynamic dispatch with `enum_dispatch`:

```rust
#[enum_dispatch]
trait HistoryProvider { ... }

#[enum_dispatch(HistoryProvider)]
enum HistoryBackend {
    File(HistoryManager),
    Database(HistoryManagerDb),
}
```

**Dependencies**: Add `enum_dispatch` crate
**Benefits**: Static dispatch, no vtable overhead
**Files to modify**: `src/backend.rs`, `src/cli/mod.rs`

## 📋 Low Priority

### 9. Benchmarks with Criterion
**Effort**: ~1 hour
**Impact**: Performance monitoring

Add benchmarks for:
- Redaction engine performance
- Search with various patterns
- Database vs file backend comparison

**Dependencies**: Add `criterion` to dev-dependencies
**Files to create**: `benches/redaction.rs`, `benches/search.rs`

### 10. typed-builder for Query Builders
**Effort**: ~30 minutes
**Impact**: Better ergonomics

Use `typed-builder` crate for compile-time query validation.

**Dependencies**: Add `typed-builder` crate

### 11. Implement AsRef/Into More Broadly
**Effort**: ~1 hour
**Impact**: More flexible APIs

Add `AsRef<str>` and `Into<T>` implementations for better API flexibility:

```rust
pub fn search<S: AsRef<str>>(&self, query: S) -> Result<Vec<Entry>>
```

**Files to modify**: Throughout codebase where `&str` is used

### 12. cargo-deny for Supply Chain Security
**Effort**: ~30 minutes
**Impact**: Security auditing

Set up `cargo-deny` for:
- License compliance checking
- Dependency auditing
- Security vulnerability scanning

**Files to create**: `deny.toml`
**CI Integration**: Add to GitHub Actions

## Notes

- See `rust_improvements.md` for detailed examples and rationale
- All completed items have been committed with proper documentation
- Prioritize based on your immediate needs vs. long-term maintenance
