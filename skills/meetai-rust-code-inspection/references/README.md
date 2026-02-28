# Rust Code Inspection Guide (MeetAI)

This is a Rust adaptation of the original `ai-reading` process.

## Mandatory pre-step setup

Before starting Step 1, complete all of the following:

1. Understand the project shape:
   - This is a Rust CLI project (`meetai`) using `clap`, `tokio`, and `anyhow`.
   - Core code lives in `src/`, with domain modules and shared utilities.
2. Read user identity/time (required):
   - Run: `node skills/meetai-rust-code-inspection/tools/setup-user-info.js`
   - Read config: `skills/meetai-rust-code-inspection/me.config.json`
   - Required fields before Step 1: `name`, `date`, `datetime`
3. Read all execution principles below.

If setup is skipped, Step 1 must not start.

## Execution principles

1. Run one step at a time in strict order (1 -> 7).
2. Before Step `N`, only read:
   - this `README.md`
   - `stepN-*.md`
   - Do not read `stepN+1..7` files in advance.
3. After each step, provide a report and wait for explicit user confirmation (`继续` / `下一步` / `进入 Step X`).
4. Without explicit confirmation, stop and do not execute next-step checks.
5. If any file is modified during a step, rerun the same step immediately.
6. If a step finds no issue and no file change is made:
   - Do not fabricate modification records.
   - Do not change version/date metadata just for reporting.

## Rust-specific global constraints

1. Keep CLI argument schema in `src/cli.rs`; keep routing in `src/main.rs`.
2. Keep domain behavior in domain modules, not in `main.rs`.
3. Prefer `Result<T>` + `anyhow::Context` over opaque errors.
4. Avoid `unwrap()`/`expect()` in production paths.
5. Keep reusable command/download logic in shared utilities (`src/utils/`).
6. Keep install directory policy centralized in `src/config.rs`.
7. When changing behavior, add/adjust tests in the same module (`#[cfg(test)]`) or in `tests/` for integration scenarios.

## Standard command set

Use this command sequence for validation when relevant:

1. `cargo fmt --check`
2. `cargo test --locked`
3. Optional strict lint:
   - `cargo clippy --locked --all-targets -- -D warnings`

## Unified step report template

```markdown
## Step X: [Step Name] Report

### Findings
- [Issue or "No issues found"]

### Fix Plan
- [What to change, or "No change needed"]

### Status
- Item 1: pass/fail
- Item 2: pass/fail

Please confirm before moving to the next step.
```
