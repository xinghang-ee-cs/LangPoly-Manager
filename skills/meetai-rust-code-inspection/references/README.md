# Rust Code Inspection Guide (MeetAI)

This is a Rust adaptation of the original `ai-reading` process.

## Mandatory pre-step setup

Before starting Step 1, complete all of the following:

1. Understand the project shape:
   - This is a Rust CLI project (`meetai`) using `clap`, `tokio`, and `anyhow`.
   - Core code lives in `src/`, with domain modules and shared utilities.
2. Use the full 7-step inspection scope by default:
   - Step 1-7 are all important and should be executed in order.
   - Step 6 and Step 7 should not be skipped unless the user explicitly requests narrowing.
3. Discover review scope from Git before Step 1:
   - Run `git status --porcelain`
   - Run `git diff --name-only`
   - If staged changes differ from the working tree, also run `git diff --cached --name-only`
   - Treat modified/untracked files as the default inspection scope, then widen only when the current step needs broader context
4. Optional user-info setup:
   - Run `node skills/meetai-rust-code-inspection/tools/setup-user-info.js` only when review logs or commit metadata require identity fields.
   - Do not block technical inspection on missing `me.config.json`.
5. Read all execution principles below.

## Execution principles

1. Run one step at a time in strict order (1 -> 7).
2. Before Step `N`, only read:
   - this `README.md`
   - `stepN-*.md`
   - Do not read `stepN+1..7` files in advance.
3. Start each inspection run by reporting the current Git change scope when available, so the user knows which files are under review first.
4. After each step, provide a report and wait for explicit confirmation (`继续` / `下一步` / `进入 Step X`).
5. Without explicit confirmation, stop and do not execute next-step checks.
6. If any file is modified during a step, rerun the same step immediately.
7. If a step finds no issue and no file change is made:
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

## High-value inspection priorities

1. Architecture:
   - Boundary clarity and dependency direction (`main/cli` routing vs domain logic).
   - Unnecessary coupling and duplicate cross-module logic.
2. Code risk:
   - Missing input validation before path/command/file operations.
   - Long functions with mixed responsibilities.
   - Risky operations (`remove_dir_all`, external command execution, PATH mutation) without guard rails.
3. Test quality:
   - Boundary and malicious-input cases.
   - Realism of tests vs production flow.
   - Runtime cost and redundant execution.
4. Low-value checks to avoid:
   - Blocking technical conclusions on personal identity/date metadata.

## Standard command set

Use this command sequence for validation when relevant:

1. `git status --porcelain`
2. `git diff --name-only`
3. `cargo fmt --check`
4. `cargo test --locked`
5. Optional strict lint:
   - `cargo clippy --locked --all-targets -- -D warnings`

## Unified step report template

```markdown
## Step X: [Step Name] Report

### Findings
- [High/Medium/Low findings first, with file evidence]

### Fix Plan
- [What to change, or "No change needed"]

### Status
- Item 1: pass/fail
- Item 2: pass/fail

Please confirm before moving to the next step.
```
