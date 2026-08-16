# AGENTS-RUST.md

Universal baseline rules for AI coding agents on Rust projects.
Merge project-specific instructions **below** the `## Project Overrides` section.

> These rules are **mandatory**. Do not skip, reorder, or silently ignore any item.
> When a project-specific rule conflicts with a baseline rule, the project-specific rule wins — but the conflict must be noted in a comment.

---

## 0. Golden Rules (Always Apply)

1. **Read before writing.** Understand the existing code, conventions, and `Cargo.toml` before making any change.
2. **Smallest correct change wins.** If two solutions solve the problem equally well, prefer the shorter one.
3. **Leave the codebase cleaner than you found it** — but only within the scope of the current task.
4. **Every claim must be verifiable.** "It should work" is not acceptable. Run the checks. Show the output.
5. **Fail loudly and early.** A compile error now is cheaper than a silent bug in production.

---

## 1. Agent Behavior

### 1.1 Think Before Acting
- Start with the **smallest plausible interpretation** of the request.
- If uncertain, ask **one clarifying question** — never assume the larger interpretation.
- Surface tradeoffs. Push back when a simpler approach exists.
- Name what is unclear and stop. Do not hide confusion behind an elaborate plan.

### 1.2 Simplicity First
- Do the minimum that solves the problem. Nothing speculative or "just in case."
- If a task fits in 1–3 commands, do it directly without over-planning.
- No features, abstractions, or error handling beyond what was explicitly requested.
- If planning an options table with more than three rows, pause — you may have misread the request.

### 1.3 Surgical Changes
- Touch **only** what the request requires. Do not improve adjacent code, comments, or formatting.
- Do not refactor things that are not broken. Match the existing style exactly.
- Every changed line must trace directly to the user's request.
- Clean up only what **your changes** made unused. Never remove pre-existing dead code unless explicitly asked.

### 1.4 Goal-Driven Execution
- Transform vague requests into **verifiable goals** before starting.
- Define what "done" looks like. Loop until verified.
- For multi-step work, state a brief plan with a verification step after each stage.
- Strong success criteria let the agent work independently. Weak criteria require constant clarification — clarify first.

---

## 2. User Interaction Protocol

- Ask **one question at a time**. Never chain questions.
- When presenting a recap, summary, or plan:
  1. Print it as formatted text (numbered list, table, or markdown block).
  2. Ask a **single short confirmation**: "Proceed?" or "Any changes?"
  3. Never embed the recap inside the question itself.
- Make a recommendation, summarize, confirm once. Stop there.
- If the user provides new information mid-task, re-evaluate the plan before continuing.

---

## 3. Mandatory Checks Before Claiming Completion

Run these in order. Do not skip any step. Show output or declare clean.

```bash
cargo fmt --check                          # formatting
cargo check                                # type-check without building
cargo clippy -- -D warnings               # no warnings allowed
cargo test                                 # all tests must pass
cargo doc --no-deps 2>&1 | grep warning   # no doc warnings
```

If any step fails, fix the issue **before** reporting completion. Never report completion with known failures.

---

## 4. Project Setup Standards

### 4.1 Cargo.toml
- Always specify `edition = "2024"` (or the edition explicitly required by the project).
- Pin direct dependencies to `major.minor` (e.g., `serde = "1.0"`). Avoid `*` versions.
- Separate `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]` cleanly.
- Use `[profile.release]` tuning only when performance requirements are documented.
- Document why each dependency exists via an inline comment when it is not obvious.

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }  # serialization
thiserror = "2"                                       # structured errors in lib code
tokio = { version = "1", features = ["full"] }        # async runtime
```

### 4.2 Workspace Projects
- Keep a root `Cargo.toml` with `[workspace]` for all multi-crate projects.
- Share common dependencies via `[workspace.dependencies]` and inherit them in members.
- Each crate must have a clear, single responsibility stated in its `Cargo.toml` `description` field.

---

## 5. Rust Idioms & Safety

### 5.1 Ownership & Borrowing
- Prefer borrowing (`&T`, `&mut T`) over cloning unless ownership transfer is semantically correct.
- Use `Cow<'_, str>` when a function sometimes owns and sometimes borrows string data.
- Avoid `clone()` inside loops; refactor to pass references or restructure data flow.

### 5.2 Error Handling
- **Library crates**: use structured errors with `thiserror`. One error enum per public module boundary.
- **Binary / application crates**: use `anyhow` for rich context propagation.
- Always propagate errors with `?`. Never swallow them silently.
- Never use `unwrap()` or `expect()` in non-test code without a documented invariant.
  - If an invariant truly cannot be violated, use `expect("invariant: <explain why this is safe>")`.
- Map external errors to domain errors at crate boundaries — do not leak implementation details.

```rust
// ✅ Correct: structured library error
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid token at position {pos}: {msg}")]
    InvalidToken { pos: usize, msg: String },
}

// ✅ Correct: application context
fn load_config(path: &Path) -> anyhow::Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    Ok(toml::from_str(&raw)?)
}
```

### 5.3 Type Design
- Derive `Debug, Clone, PartialEq, Eq` where meaningful. Add `Hash` when the type will be used in collections.
- Avoid `Copy` unless the type is trivially copyable (plain data, no heap allocation, ≤ 16 bytes).
- Use newtypes to enforce domain invariants at compile time.
- Prefer `Option<T>` over sentinel values (empty string, `-1`, `0` used as "none").
- Use `NonZeroU*` types when a value is semantically nonzero.

```rust
// ✅ Newtype pattern
pub struct UserId(u64);
pub struct OrderId(u64);
// Prevents mixing up IDs at compile time
```

### 5.4 Unsafe Code
- Avoid `unsafe` unless strictly required by FFI, raw pointer arithmetic, or performance-critical hot paths.
- Every `unsafe` block **must** have a `// SAFETY:` comment explaining why the invariants hold.
- Encapsulate `unsafe` inside a safe abstraction. Never expose raw pointers in public APIs.
- Add a test that exercises every code path touching `unsafe` blocks.

### 5.5 Panics
- No `panic!`, `todo!`, `unimplemented!`, or `unreachable!` in production paths.
- `unreachable!` is acceptable only when a match arm is **provably** unreachable by construction; document why.
- Replace `todo!()` with a compile-time error if the feature is not yet implemented:
  ```rust
  compile_error!("feature X is not yet implemented — see issue #42");
  ```

### 5.6 Performance Awareness
- Prefer iterators and iterator adaptors over manual index loops.
- Avoid allocating in hot paths. Use `SmallVec`, `ArrayVec`, or stack buffers where appropriate.
- Profile before optimizing. Do not introduce complexity for hypothetical gains.
- Use `#[inline]` sparingly — only when profiling confirms a benefit.

---

## 6. Module & Visibility

- Keep `lib.rs` / `main.rs` thin: re-exports and top-level wiring only. Delegate to focused modules.
- Use `pub(crate)` for internal APIs. Only mark `pub` what is part of the intentional public surface.
- Group related types, traits, and functions in the same module or submodule.
- Avoid deeply nested module trees (more than 3 levels) unless the codebase is genuinely large.
- Use `mod.rs`-less module layout (file-per-module, e.g., `src/parser.rs` not `src/parser/mod.rs`) unless the module has sub-modules.
- Re-export carefully: a flat public API is easier to use than deep paths.

```
src/
├── lib.rs          # pub use, top-level types
├── error.rs        # unified Error / Result
├── config.rs
├── parser/
│   ├── mod.rs
│   ├── lexer.rs
│   └── ast.rs
└── db/
    ├── mod.rs
    └── queries.rs
```

---

## 7. Async Code (when applicable)

- Use `tokio` as the default async runtime unless the project specifies otherwise.
- Prefer `async fn` over manual `impl Future` unless fine-grained control is needed.
- Never block inside an async context: replace `std::thread::sleep` with `tokio::time::sleep`, `std::fs` with `tokio::fs`, etc.
- Use `tokio::spawn` for background tasks. Always `.await` or store the `JoinHandle`.
- Set timeouts on all I/O: `tokio::time::timeout(duration, future)`.
- Structure concurrency with `tokio::select!`, `FuturesUnordered`, or `JoinSet` — never busy-loop.
- Propagate cancellation: check for cancellation at every `.await` point in long-running tasks.

---

## 8. Testing Standards

### 8.1 Unit Tests
- Write unit tests in `#[cfg(test)]` modules alongside the implementation.
- Cover: happy path, edge cases (empty, boundary values), and error propagation.
- Name tests descriptively: `test_<unit>_<scenario>_<expected_outcome>`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_input_returns_ok() { ... }

    #[test]
    fn parse_empty_input_returns_error() { ... }

    #[test]
    fn parse_oversized_input_truncates_correctly() { ... }
}
```

### 8.2 Integration Tests
- Place integration tests in `tests/` at the crate root.
- Each integration test file should test one feature or flow end-to-end.
- Use `rstest` for parameterized tests when the same logic needs multiple input sets.

### 8.3 Async Tests
- Use `#[tokio::test]` for async unit tests.
- Use `#[tokio::test(flavor = "multi_thread")]` only when concurrency is part of what is being tested.

### 8.4 Test Hygiene
- No `unwrap()` in tests — use `?` with `-> Result<(), Box<dyn Error>>` return type, or `assert!(result.is_ok())`.
- Clean up test side effects (temp files, DB state) using RAII guards or `tempfile` crate.
- Tests must not depend on external services unless explicitly marked `#[ignore]` with a comment.

---

## 9. Documentation Standards

- Document all `pub` items with `///`. Include:
  - What it does (one line).
  - Parameters / fields (if non-obvious).
  - Return value and error cases.
  - A `# Examples` section with a runnable `\`\`\`rust` block for non-trivial APIs.
- Use `//!` module-level docs in every module explaining its purpose and responsibility.
- Keep docs accurate — outdated docs are worse than no docs.
- Run `cargo doc --no-deps --open` periodically to verify rendered output.

```rust
/// Parses a raw configuration string into a [`Config`].
///
/// # Errors
///
/// Returns [`ParseError::InvalidToken`] if the input contains an unrecognized key.
///
/// # Examples
///
/// ```rust
/// let cfg = parse_config("timeout = 30")?;
/// assert_eq!(cfg.timeout, 30);
/// ```
pub fn parse_config(raw: &str) -> Result<Config, ParseError> { ... }
```

---

## 10. Security Checklist

- Never log secrets, tokens, API keys, or PII.
- Validate and sanitize all external input before use (file paths, user strings, network data).
- Use `PathBuf::canonicalize()` and verify paths stay within expected roots to prevent traversal.
- Prefer `secrecy::Secret<T>` for sensitive values to prevent accidental debug-printing.
- Audit `Cargo.lock` with `cargo audit` before each release. Treat high-severity advisories as blockers.
- Avoid `unsafe` for parsing untrusted data — always use safe, validated parsers.

---

## 11. CI / Release Checklist

Before merging to main or cutting a release, confirm all of the following pass:

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo doc --no-deps
cargo audit          # requires cargo-audit
```

For releases, additionally:
- Bump version in `Cargo.toml` following [SemVer](https://semver.org/).
- Update `CHANGELOG.md` with a summary of changes.
- Tag the commit: `git tag -s v<version> -m "Release v<version>"`.
- Verify `cargo publish --dry-run` succeeds before publishing to crates.io (if applicable).

---

## 12. Refactoring Workflow

When applying these standards to an existing codebase, follow this sequence:

1. **Audit** — Run `cargo clippy -- -W clippy::all` and `cargo fmt --check`. List all violations. Do not modify code yet.
2. **Prioritize by tier:**
   - **Tier 1 (Safety & Correctness):** `unwrap` without invariant docs, missing error handling, `unsafe` without `// SAFETY:` comments, panics in production paths.
   - **Tier 2 (Idiomatic Patterns):** module visibility, trait usage, error type structure, clone-in-loop, blocking in async.
   - **Tier 3 (Style & Docs):** formatting, missing `///` docs, test coverage gaps.
3. **Surgical execution** — Refactor file-by-file or module-by-module. After each change:
   ```bash
   cargo check && cargo test && cargo clippy -- -D warnings
   ```
4. **Verify completeness** — Re-run the full CI checklist from §11.
5. **Document** — Update inline docs and `CHANGELOG.md` to reflect structural changes.

---

## 13. Common Anti-Patterns (Never Do)

| Anti-pattern | Preferred alternative |
|---|---|
| `unwrap()` / `expect("")` without docs | `?` with structured errors, or `expect("invariant: …")` |
| `clone()` inside a loop | pass references; restructure ownership |
| Returning `String` for errors | Return a typed error enum |
| `Vec<Box<dyn Trait>>` for homogeneous collections | Use a concrete type or enum |
| `pub` on everything | `pub(crate)` or `pub(super)` for internals |
| `std::sync::Mutex` in async code | `tokio::sync::Mutex` |
| Blocking I/O inside `async fn` | `tokio::fs`, `tokio::io`, `spawn_blocking` |
| `println!` for diagnostics in library code | `tracing` or `log` crate |
| Magic numbers / string literals | Named constants with `const` |
| Ignoring `#[must_use]` results | Always handle or explicitly `let _ =` with a comment |

---

## 14. Project Overrides

> Add project-specific rules here. Conflicting rules override the baseline above.
> Format: `[OVERRIDE §<section>] <rule>`
