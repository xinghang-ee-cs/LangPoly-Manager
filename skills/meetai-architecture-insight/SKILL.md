---
name: meetai-architecture-insight
description: Fast architecture and code-navigation guide for the MeetAI Rust CLI project. Use when adding features, fixing bugs, refactoring, or reviewing behavior across runtime, python, pip, quick-install, config, and utils modules, and when you need to locate the minimal set of files to read before editing.
---

# MeetAI Architecture Insight

Use this skill to cut discovery time before edits.

## Route in 30 seconds

1. Identify CLI entry and command parsing first:
   - `src/main.rs`
   - `src/cli.rs`
2. Open only the command module you need:
   - `runtime` -> `src/runtime/mod.rs`
   - `python` / `venv` -> `src/python/mod.rs`
   - `pip` -> `src/pip/mod.rs`
   - `quick-install` -> `src/quick_install/mod.rs`
3. Load task-specific file list from:
   - `references/file-router.md`

## Core module boundaries

- Keep command routing in `src/main.rs` and `src/cli.rs`.
- Keep business behavior in domain folders:
  - `src/python/`
  - `src/pip/`
  - `src/runtime/`
  - `src/quick_install/`
- Keep shared capabilities in `src/utils/`.
- Keep persistent path/layout decisions in `src/config.rs`.

## Architectural invariants

- Treat `PythonInstaller` as the single source of truth for Python `latest` resolution and download fallback behavior.
- Keep quick-install aligned with runtime behavior for Python install/switch flows.
- Keep app data rooted at `.meetai` near executable path per `Config::app_home_dir`.
- Keep user guidance and diagnostics centralized in `src/utils/guidance.rs`.
- Keep progress styles centralized in `src/utils/progress.rs`.
- Keep command execution and download plumbing centralized in:
  - `src/utils/executor.rs`
  - `src/utils/downloader.rs`

## Edit workflow

1. Pick scenario from `references/file-router.md`.
2. Read only listed files.
3. Edit in the lowest-level module that owns behavior.
4. Confirm no duplicated logic is introduced across `runtime` and `quick-install`.
5. Validate with:
   - `cargo fmt --check`
   - `cargo test --locked`
   - `cargo build --release`

## Search shortcuts

- Find command handling: `rg -n "handle_.*command|match args.action" src`
- Find Python install flow: `rg -n "install\\(|resolve_latest|get_download_sources|download_installer" src/python`
- Find quick-install flow: `rg -n "install_python|install_pip|verify_installation|build_step_failure_message" src/quick_install`
- Find shared guidance/progress usage: `rg -n "network_diagnostic_tips|quick_install_help_commands|print_python_path_guidance|moon_bar_style|moon_spinner_style" src`

