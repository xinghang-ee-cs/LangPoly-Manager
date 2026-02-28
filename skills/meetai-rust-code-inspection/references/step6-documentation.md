# Step 6: Documentation Check (Rust)

Read `references/README.md` first.

## Step gate (must follow)

1. Before running this step, only load:
   - `references/README.md`
   - this file
2. Do not read other `step*.md` files.
3. After finishing this step, stop and ask the user whether to continue to Step 7.

## Goal

Keep CLI and module documentation consistent with actual Rust implementation.

## Required checks

1. Root README command examples match real CLI behavior.
2. If `src/cli.rs` changed, update help text examples in docs.
3. If runtime support matrix changed, update README support status.
4. If fallback/error guidance changed, update user-facing docs accordingly.
5. If public module APIs changed, update relevant rustdoc (`///` or `//!`).

## Recommended documentation structure

1. What the module/command does.
2. Main public entry points.
3. Known limitations and fallback behavior.
4. Typical usage examples.
5. Troubleshooting hints.

## Suggested checks

```bash
rg -n "quick-install|runtime install python|latest|fallback" README.md src
cargo run -- --help
```

## Common mistakes

1. Docs still claim behavior that code no longer has.
2. Example commands omit required flags/arguments.
3. Limitations section not updated after feature changes.

## Mandatory rerun rule

If any documentation is modified in this step, rerun Step 6 completely before moving on.
