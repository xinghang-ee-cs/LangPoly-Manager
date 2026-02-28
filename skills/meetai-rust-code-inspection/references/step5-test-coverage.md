# Step 5: Test Coverage Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. After finishing this step, stop and ask the user whether to continue to Step 6.

## Goal

Ensure changed behavior is covered by reliable tests, and all related tests pass.

## Coverage principles

1. Every behavior change must have at least one direct test.
2. Keep unit tests close to module code when practical:
   - `#[cfg(test)] mod tests` in the same file.
3. Use integration tests (`tests/`) when multiple modules interact.
4. Async logic should use `#[tokio::test]` when needed.
5. External side effects should be mocked or isolated.

## Repository-specific focus

1. CLI parsing behavior:
   - `src/cli.rs`
2. Quick-install orchestration:
   - `src/quick_install/installer.rs`
3. Validation hardening:
    - `src/utils/validator.rs`
4. Python install/version flows:
    - `src/python/installer.rs`
    - `src/python/installer/*.rs`
    - `src/python/service.rs`
    - `src/python/version.rs`
5. Quick-install validation layering:
    - `src/quick_install/config.rs`
    - `src/quick_install/validator.rs`

## Suggested commands

```bash
cargo test --locked
cargo test --locked quick_install::installer::tests
cargo test --locked cli::tests
```

Optional coverage tooling (if installed):

```bash
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

## Failure handling

1. If tests fail, fix root cause first.
2. Re-run affected tests.
3. Re-run full `cargo test --locked` before closing Step 5.

## Mandatory rerun rule

If any test or implementation file changes in this step, rerun Step 5 completely before moving on.
