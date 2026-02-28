# Step 2: Comment and Doc Standard Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. After finishing this step, stop and ask the user whether to continue to Step 3.

## Goal

Ensure comments are accurate, minimal, and Rust-idiomatic (`//!`, `///`, `//`).

## Rules

1. Module docs:
   - Use `//!` where module-level context is needed.
2. Public API docs:
   - Use `///` for externally consumed public functions/types.
3. Inline comments:
   - Use `//` only for non-obvious logic.
4. Do not add Java-style file headers (`@author`, `@version`) to Rust source files.
5. Keep examples executable when practical:
   - Prefer Rust code fences in docs.

## Required checks

1. Comments do not contradict current behavior.
2. Error/help messages shown to users are clear and actionable.
3. Public methods with non-trivial behavior have concise rustdoc.
4. No large stale comment blocks describing removed logic.

## Suggested commands

```bash
rg -n "TODO|FIXME|XXX|NOTE" src
rg -n "^///|^//!" src
```

## Common mistakes

1. Comment restates obvious code with no value.
2. Comments mention old behavior after refactor.
3. Public APIs changed but docs were not updated.

## Mandatory rerun rule

If any comment/doc update is made in this step, rerun Step 2 completely before moving on.
