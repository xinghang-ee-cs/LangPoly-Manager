---
name: meetai-rust-code-inspection
description: 7-step Rust code inspection workflow for this MeetAI CLI repository. Use when users request comprehensive checks of changed code, ask to check the code in general, or want a full pre-commit quality pass. Default to current Git changes first.
---

# MeetAI Rust Code Inspection

Use this skill when the user asks for a step-by-step code inspection workflow similar to the original `ai-reading` flow, but aligned to Rust code in this repository.

## Trigger phrases

Auto-trigger this skill when user intent matches comprehensive code inspection, including requests like:

- “帮我对修改后的代码进行全面检查”
- “帮我对代码做检查”
- “全面检查这次改动”
- “做一次完整检查/全量巡检”
- “提交前帮我把代码全检查一遍”
- “按 7 步做代码检查”
- “naming/comment/quality/architecture/testing/documentation/commit 全流程检查”

If the user explicitly asks only for a single narrow task (for example only formatting or only one bug fix), this skill is optional and can be skipped.

## What this skill replaces

The original `ai-reading` docs are oriented to NestJS/TypeScript server code.
This skill provides the same 7-step process for the Rust CLI codebase under `src/`.

## Entry checklist

1. Use full 7-step scope by default:
   - Step 1-7 are all part of the standard high-value workflow.
   - Step 6 (documentation) and Step 7 (commit readiness) are not optional in default flow.
   - Only skip steps if the user explicitly asks to narrow scope.
2. Read `references/README.md` before any step.
3. Discover inspection scope from Git first:
   - Run `git status --porcelain`
   - Run `git diff --name-only`
   - If staged changes may differ from working tree, also run `git diff --cached --name-only`
   - Use modified/untracked files as the default inspection focus before widening to repo-wide analysis
4. Execute one step at a time in strict order (Step 1 -> Step 7).
5. Optional user-info setup (only when needed for review logs or commit metadata):
    - Run `node skills/meetai-rust-code-inspection/tools/setup-user-info.js`
    - Read `skills/meetai-rust-code-inspection/me.config.json`
    - This setup is not required for pure technical inspection.

## Strict step gate (non-negotiable)

1. Progressive loading:
   - Before Step `N`, only read:
     - `references/README.md`
     - `references/stepN-*.md`
   - Do not open Step `N+1..7` files in advance.
2. One turn, one step:
   - Each inspection turn may execute exactly one step.
   - Do not bundle multiple step reports in one response.
3. Mandatory stop-and-ask:
   - After finishing a step, stop and ask for explicit confirmation before the next step.
   - Acceptable confirmations: `继续`, `下一步`, `进入 Step X`.
   - Without explicit confirmation, do not run next-step checks.
4. Step rerun precedence:
   - If this step edits any file, rerun this same step and report rerun results first.
   - Still wait for user confirmation before moving forward.
5. Violation prevention:
   - If you accidentally loaded future step files, discard them and restart from current step boundaries.
6. Full-flow requirement:
   - In standard execution, include Step 1-7.
   - Do not treat Step 6/7 as optional by default.
7. Skip low-value checks:
   - Do not block technical inspection on identity/date metadata.
8. Baseline evidence:
   - Run objective gates when feasible: `cargo fmt --check`, `cargo test --locked`, and optional `cargo clippy --locked --all-targets -- -D warnings`.
   - Report command outcomes together with static findings in the relevant step report.

## Report style

1. Findings first, sorted by severity (High/Medium/Low).
2. Each finding should include concrete file references and impact.
3. Keep summary brief; include assumptions or residual risk only after findings.

## Step files

- `references/step1-naming-convention.md`
- `references/step2-comment-standard.md`
- `references/step3-code-quality.md`
- `references/step4-architecture-layer.md`
- `references/step5-test-coverage.md`
- `references/step6-documentation.md`
- `references/step7-code-commit.md`

## Repository-specific boundaries

- Command routing stays in `src/main.rs` and `src/cli.rs`.
- Domain behavior stays in:
  - `src/runtime/`
  - `src/python/`
  - `src/pip/`
  - `src/quick_install/`
- Keep shared Python CLI/runtime behavior aligned via:
  - `src/python/service.rs`
  - `src/python/mod.rs` (`handle_python_use_path_setup`)
- Shared capabilities stay in `src/utils/`.
- Persistent path/data layout stays in `src/config.rs`.
- Keep Python latest-resolution/fallback logic centered in:
  - `src/python/installer.rs`
  - `src/python/installer/*.rs`

## Baseline verification commands

- `cargo fmt --check`
- `cargo test --locked`
- Optional stricter lint gate:
  - `cargo clippy --locked --all-targets -- -D warnings`
