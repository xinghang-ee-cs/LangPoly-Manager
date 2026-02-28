# Step 3: Code Quality Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. After finishing this step, stop and ask the user whether to continue to Step 4.

## Goal

Remove quality risks and enforce robust Rust patterns.

## Focus areas

1. Dead code and unused imports:
   - Remove unused code paths and stale helpers.
2. Error handling quality:
   - Prefer `Result` with `anyhow::Context`.
   - Avoid `unwrap()`/`expect()` in production paths.
3. Input validation:
   - Keep validation centralized (e.g., `src/utils/validator.rs`).
4. Duplicate logic:
   - Extract shared logic to module-level helpers when appropriate.
5. TODO/FIXME debt:
   - Resolve or remove before final delivery whenever possible.
6. Async flow correctness:
   - Ensure awaited calls preserve intended ordering.

## Suggested commands

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
rg -n "TODO|FIXME|XXX" src
```

## High-risk patterns

1. Swallowed errors without context.
2. Partial fallback logic duplicated across modules.
3. Security-sensitive string handling without validation.
4. Command execution paths that allow option-like injection.

## Repository hotspots

1. `src/python/installer.rs`
2. `src/python/installer/*.rs`
3. `src/python/service.rs`
4. `src/utils/downloader.rs`
5. `src/utils/executor.rs`
6. `src/utils/validator.rs`
7. `src/quick_install/installer.rs`
8. `src/quick_install/validator.rs`

## Mandatory rerun rule

If any quality fix is applied in this step, rerun Step 3 completely before moving on.
