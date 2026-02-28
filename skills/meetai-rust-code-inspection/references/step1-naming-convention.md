# Step 1: Naming Convention Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. Preflight must pass before this step starts:
   - `skills/meetai-rust-code-inspection/me.config.json` exists
   - contains `name`, `date`, `datetime`
   - `date` equals today's date
4. After finishing this step, stop and ask the user whether to continue to Step 2.

## Goal

Ensure naming consistency for Rust files, modules, types, functions, constants, and CLI command names.

## Rules

1. Files and modules:
   - Use `snake_case` (`python/version.rs`, `quick_install/installer.rs`).
   - Keep `mod.rs` only where directory-style modules are intended.
2. Struct, enum, trait, type alias:
   - Use `PascalCase` (`PythonInstaller`, `RuntimeType`).
3. Function, method, variable:
   - Use `snake_case` (`handle_python_command`, `target_dir`).
4. Constants and static values:
   - Use `SCREAMING_SNAKE_CASE`.
5. CLI names in clap:
   - User-facing command names should be kebab-case (`quick-install`).
6. Test names:
   - Use descriptive snake_case (`install_latest_delegates_to_python_installer_directly`).

## What to check in this repo

1. `src/main.rs` + `src/cli.rs` for command names and action naming.
2. Domain modules for item naming consistency:
   - `src/runtime/`
   - `src/python/`
   - `src/pip/`
   - `src/quick_install/`
3. Shared modules:
   - `src/utils/`

## Suggested commands

```bash
rg --files src
rg -n "enum |struct |trait |const " src
cargo check --locked
```

## Common mistakes

1. Mixing camelCase in Rust function names.
2. Using vague abbreviations that hide meaning.
3. Inconsistent CLI naming between clap schema and help text.

## Mandatory rerun rule

If any rename/move/edit is done in this step, rerun Step 1 completely before moving on.
