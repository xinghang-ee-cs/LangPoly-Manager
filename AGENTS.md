# AGENTS.md instructions for d:\technology\test\rust\test\python-manager

<INSTRUCTIONS>
## Skills
A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used in this project session.

### Available skills
- meetai-architecture-insight: Project-local architecture and file-routing insight for this repository. Use when modifying features, fixing bugs, refactoring modules, or planning code changes with minimal discovery cost. (file: d:/technology/test/rust/test/python-manager/skills/meetai-architecture-insight/SKILL.md)
- meetai-rust-code-inspection: Project-local 7-step Rust code inspection and quality workflow adapted from ai-reading for this repository. Use when the user asks for comprehensive/complete checks of changed code (e.g. “全面检查”, “完整检查”, “改动后检查”, “提交前检查”, “全量巡检”) or when running naming/comment/quality/architecture/testing/documentation/commit checks in sequence. (file: d:/technology/test/rust/test/python-manager/skills/meetai-rust-code-inspection/SKILL.md)

### How to use skills
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches the skill description, you must use that skill for that turn.
- Scope: This AGENTS file is project-local and only affects this repository session.
- Loading: Open the skill `SKILL.md` first, then read only the needed reference files.
- Context hygiene: Keep context small; load only files directly needed for the requested change.
- Fallback: If the skill file is missing or blocked, state the issue briefly and continue with the best direct code analysis workflow.
</INSTRUCTIONS>
