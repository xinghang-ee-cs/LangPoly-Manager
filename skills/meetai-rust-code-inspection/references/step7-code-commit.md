# Step 7: Code Commit Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. This is the final step. After reporting, stop and wait for user follow-up.

## Goal

Prepare clean, traceable commits for inspected Rust changes.

## Core principles

1. Never modify or revert out-of-scope files without explicit user request.
2. Review all current git changes before commit planning.
3. Group commits by logical change type (style/fix/refactor/test/docs/feat).
4. Commit only files in agreed scope.

## Pre-commit checklist

1. `git status`
2. `git diff --name-only`
3. Validation commands:
   - `cargo fmt --check`
   - `cargo test --locked`
   - Optional: `cargo clippy --locked --all-targets -- -D warnings`

## Commit message guidance

Use conventional style when possible:

1. `style(scope): ...`
2. `fix(scope): ...`
3. `refactor(scope): ...`
4. `test(scope): ...`
5. `docs(scope): ...`
6. `feat(scope): ...`

Example:

```text
fix(quick_install): preserve latest delegation in python install flow
```

## Push and PR guidance

1. Confirm target remote with the user before push.
2. Use non-destructive git operations only.
3. Include verification summary in PR description.

## Mandatory rerun rule

If any code is further changed while preparing commit artifacts, rerun Step 7 checks before finalizing.
