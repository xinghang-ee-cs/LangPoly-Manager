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
   - `node` -> `src/node/mod.rs`
   - `pip` -> `src/pip/mod.rs`
   - `quick-install` -> `src/quick_install/mod.rs`
3. For runtime behavior, decide language scope before deep read:
   - shared domain API -> `src/python/service.rs`
   - install orchestrator -> `src/python/installer.rs`
   - installer internals -> `src/python/installer/*.rs`
   - Node service and PATH flow -> `src/node/service.rs`
   - Node install/version resolution -> `src/node/installer.rs`
   - project `.nvmrc` resolution -> `src/node/project.rs`
4. Load task-specific file list from:
   - `references/file-router.md`

## Core module boundaries

- Keep command routing in `src/main.rs` and `src/cli.rs`.
- Keep business behavior in domain folders:
  - `src/node/`
  - `src/python/`
  - `src/pip/`
  - `src/runtime/`
  - `src/quick_install/`
- Keep Python command and runtime command aligned via:
  - `src/python/service.rs`
  - `src/python/mod.rs` (`handle_python_use_path_setup`)
- Keep Node command and runtime command aligned via:
  - `src/node/service.rs`
  - `src/node/mod.rs`
  - `src/node/project.rs`
- Keep shared capabilities in `src/utils/`.
- Keep persistent path/layout decisions in `src/config.rs`.

## Architectural invariants

- Treat `PythonInstaller` as the single source of truth for Python `latest` resolution and download fallback behavior.
- Keep `runtime`/`python` CLI Python behavior aligned by reusing `PythonService` and shared PATH guidance flow.
- Keep `runtime`/`node` CLI Node behavior aligned by reusing `NodeService`, `.nvmrc` project resolution, and shared PATH guidance flow.
- Keep quick-install validation layered:
  - parameter/option validation in `src/quick_install/config.rs`
  - post-install verification in `src/quick_install/validator.rs`
- Keep app data rooted at `.meetai` near executable path per `Config::app_home_dir`.
- Keep user guidance and diagnostics centralized in `src/utils/guidance.rs`.
- Keep progress styles centralized in `src/utils/progress.rs`.
- Keep command execution and download plumbing centralized in:
  - `src/utils/executor.rs`
  - `src/utils/downloader.rs`
  - `src/utils/http_client.rs`
- Keep Pip current-Python resolution centralized in:
  - `src/pip/mod.rs` (`resolve_current_python_executable`)

## Edit workflow

1. Pick scenario from `references/file-router.md`.
2. Read only listed files.
3. Edit in the lowest-level module that owns behavior.
4. Confirm no duplicated logic is introduced across:
   - `runtime` and `python` (`python use`/PATH flow)
   - `runtime` and `node` (`node install/use` + `.nvmrc` flow)
   - quick-install config validation and post-install verification
5. Validate with:
  - `cargo fmt --check`
  - `cargo test --locked`
   - `cargo build --release`

## Search shortcuts

- Find command handling: `rg -n "handle_.*command|match args.action" src`
- Find Python service and shared use flow: `rg -n "PythonService|handle_python_use_path_setup|detect_use_path_status|ensure_shims_in_path" src/python src/runtime`
- Find Node service and shared use flow: `rg -n "NodeService|resolve_project_version_from_nvmrc|detect_use_path_status|ensure_shims_in_path" src/node src/runtime`
- Find Python install flow: `rg -n "install\\(|resolve_latest|get_download_sources|download_installer|verify_installation|copy_or_adopt" src/python`
- Find Node install flow: `rg -n "list_available_versions|resolve_target_version|parse_latest|build_download_url|verify_installation" src/node`
- Find quick-install flow: `rg -n "install_python|install_pip|verify_installation|build_step_failure_message" src/quick_install`
- Find quick-install validation layering: `rg -n "from_args|config\\.validate|verify_installation" src/quick_install`
- Find Pip executable resolution and commands: `rg -n "resolve_current_python_executable|get_python_exe|current_python_executable" src/pip`
- Find shared guidance/progress/network usage: `rg -n "network_diagnostic_tips|print_python_path_guidance|print_node_path_guidance|moon_bar_style|moon_spinner_style|build_http_client" src`
