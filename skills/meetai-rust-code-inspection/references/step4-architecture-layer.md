# Step 4: Architecture Boundary Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. After finishing this step, stop and ask the user whether to continue to Step 5.

## Goal

Verify module boundaries, ownership, and dependency direction are consistent with this Rust CLI architecture.

## Architecture map for this repo

1. Entry and routing:
   - `src/main.rs`
   - `src/cli.rs`
2. Domain modules:
   - `src/runtime/`
   - `src/python/`
   - `src/pip/`
   - `src/quick_install/`
3. Shared utilities:
   - `src/utils/`
4. Path/layout policy:
   - `src/config.rs`

## Required checks

1. `main.rs` only routes commands; no business implementation is embedded there.
2. CLI schema changes remain in `cli.rs`, not scattered across domain modules.
3. Python latest resolution and download fallback are not re-implemented in multiple places.
4. `runtime` and `python` command paths stay aligned by reusing shared Python service/path setup flow.
5. Quick-install keeps layered validation responsibilities:
   - option/parameter validation in `src/quick_install/config.rs`
   - post-install verification in `src/quick_install/validator.rs`
6. Pip current-Python executable resolution remains centralized in `src/pip/mod.rs`.
7. Shared concerns (guidance/progress/executor/downloader) stay in `src/utils/`.
8. Install path and app-home decisions stay centralized in `src/config.rs`.

## Suggested commands

```bash
rg -n "match cli.command|handle_.*command" src
rg -n "latest|resolve|fallback|download" src/python src/quick_install src/runtime
rg -n "PythonService|handle_python_use_path_setup|resolve_current_python_executable|from_args|verify_installation" src
cargo check --locked
```

## Typical violations

1. Repeating fallback/download logic in multiple modules.
2. Moving path policy out of `config.rs`.
3. Mixing CLI parsing concerns into implementation modules.

## Mandatory rerun rule

If any structural refactor/edit is done in this step, rerun Step 4 completely before moving on.
