# AGENTS.md instructions for d:\technology\test\rust\test\python-manager

<INSTRUCTIONS>
## Skills
A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used in this project session.

### Available skills
- meetai-architecture-insight: Project-local architecture and file-routing insight for this repository. Use when modifying features, fixing bugs, refactoring modules, or planning code changes with minimal discovery cost. (file: d:/technology/test/rust/test/python-manager/skills/meetai-architecture-insight/SKILL.md)

### How to use skills
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches the skill description, you must use that skill for that turn.
- Scope: This AGENTS file is project-local and only affects this repository session.
- Loading: Open the skill `SKILL.md` first, then read only the needed reference files.
- Context hygiene: Keep context small; load only files directly needed for the requested change.
- Fallback: If the skill file is missing or blocked, state the issue briefly and continue with the best direct code analysis workflow.
</INSTRUCTIONS>
