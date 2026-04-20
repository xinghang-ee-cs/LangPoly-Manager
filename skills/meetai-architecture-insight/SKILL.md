---
name: meetai-architecture-insight
description: Fast architecture and code-navigation guide for the MeetAI Rust CLI project. Use when adding features, fixing bugs, refactoring, or reviewing behavior across runtime, python, node, pip, quick-install, config, and shared utility layers, and when you need the minimum file set before editing.
---

# MeetAI Architecture Insight

Use this skill to reduce discovery cost before changing code.

## Route in 30 seconds

1. Confirm the CLI entry stack first:
   - `src/main.rs`
   - `src/cli.rs`
   - `src/lib.rs`
2. Open only the command module you need:
   - `runtime` -> `src/runtime/mod.rs`
   - `python` / `venv` -> `src/python/mod.rs`
   - `node` -> `src/node/mod.rs`
   - `pip` -> `src/pip/mod.rs`
   - `quick-install` -> `src/quick_install/mod.rs`
3. If the behavior looks shared across Python and Node, jump to:
   - `src/runtime/common.rs`
4. Then decide the owning layer before reading more:
   - Python install/latest/adopt/verify -> `src/python/installer.rs` + `src/python/installer/*.rs`
   - Python current version / shims / PATH -> `src/python/version.rs`
   - Python surface-aware command flow -> `src/python/service.rs`
   - Venv behavior -> `src/python/environment.rs`
   - Node install / available / latest-lts / project resolution -> `src/node/installer.rs` + `src/node/project.rs`
   - Node current version / shims / PATH -> `src/node/version.rs`
   - Node surface-aware command flow -> `src/node/service.rs`
   - Quick-install config / orchestration / verify -> `src/quick_install/config.rs`, `src/quick_install/installer.rs`, `src/quick_install/validator.rs`
   - App-home layout / config / legacy migration -> `src/config.rs`
   - User-facing recovery and PATH guidance -> `src/utils/guidance.rs`
5. Load task-specific file sets from:
   - `references/file-router.md`

## Core module boundaries

- Keep CLI argument schema and top-level dispatch in:
  - `src/main.rs`
  - `src/cli.rs`
  - `src/lib.rs`
- Keep domain command handlers in:
  - `src/runtime/mod.rs`
  - `src/python/mod.rs`
  - `src/node/mod.rs`
  - `src/pip/mod.rs`
  - `src/quick_install/mod.rs`
- Treat `src/runtime/common.rs` as the shared runtime orchestration layer for:
  - install / uninstall coordination
  - current-version activation
  - PATH / shims state detection and guidance flow
- Treat `src/python/service.rs` and `src/node/service.rs` as surface-aware wrappers around shared runtime behavior.
- Treat `src/python/version.rs` and `src/node/version.rs` as the owners of:
  - current-version persistence
  - shim refresh / cleanup
  - PATH guidance hooks and command readiness checks
- Treat `src/python/installer.rs` plus `src/python/installer/*.rs` as the single owner of Python install behavior:
  - latest resolution
  - official / mirror download selection
  - Windows installer execution
  - verify and adopt/import flows
- Treat `src/node/installer.rs` and `src/node/project.rs` as the owners of Node-specific install behavior:
  - remote available-version listing
  - `latest` / `newest` / `lts` resolution
  - `.nvmrc` project resolution
  - archive download / extract / verify flow
- Treat `src/python/environment.rs` as the owner of cross-platform venv creation and activation output.
- Treat `src/quick_install/config.rs`, `src/quick_install/installer.rs`, and `src/quick_install/validator.rs` as the three-layer quick-install pipeline:
  - config translation and validation
  - orchestration
  - post-install verification
- Treat `src/config.rs` as the owner of app-home policy, config persistence, and legacy migration/repair.
- Treat shared helper layers as:
  - `src/utils/downloader.rs` for raw download mechanics
  - `src/utils/http_client.rs` for shared HTTP policy
  - `src/utils/executor.rs` for command execution
  - `src/utils/guidance.rs` for user-facing recovery / next-step wording
  - `src/utils/progress.rs` for progress styles
  - `src/utils/validator.rs` for input validation rules

## Platform boundaries

- Python auto-install is Windows-only today. On non-Windows platforms, the Python install layer currently supports:
  - managing already-available MeetAI-controlled versions
  - falling back from `latest` to the highest locally managed version when possible
  - failing with guidance when no managed version exists yet
- Node auto-install is Windows-only today. On non-Windows platforms, the Node layer currently still matters for:
  - managed-version activation
  - current-version persistence and shim refresh
  - `project` / `.nvmrc` resolution
  - path guidance and command readiness flow
- Python and Node shims / PATH flows are designed as cross-platform behavior and live primarily in:
  - `src/runtime/common.rs`
  - `src/python/version.rs`
  - `src/node/version.rs`
- Venv activation output is already platform-aware and belongs to:
  - `src/python/environment.rs`
- When planning Linux work, split the problem into three separate capabilities before reading deeper:
  - auto-install support
  - use / shim / PATH support
  - venv / shell guidance support

## Architectural invariants

- Treat `GenericRuntimeService` in `src/runtime/common.rs` as the shared orchestration layer for Python and Node runtime flows.
- Keep surface-specific user guidance in:
  - `src/python/service.rs`
  - `src/node/service.rs`
  Do not move command-surface wording back into the shared runtime layer.
- Keep Python state split as:
  - `config.json`-backed current-version state in `src/config.rs`
  - shim synchronization in `src/python/version.rs`
- Keep Node state split as:
  - current-version file plus shims in `src/node/version.rs`
  - install/version resolution in `src/node/installer.rs`
- Keep quick-install layered:
  - argument/config validation in `src/quick_install/config.rs`
  - orchestration in `src/quick_install/installer.rs`
  - post-install verification in `src/quick_install/validator.rs`
- Keep app-home layout and legacy repair policy centralized in `src/config.rs`.
- Keep network diagnostics and PATH help centralized in `src/utils/guidance.rs`.
- Keep Java / Go behavior treated as planned-only inside quick-install and runtime messaging until actual runtime-management code exists.

## Edit workflow

1. Pick the change scenario from `references/file-router.md`.
2. Read only the listed files plus the nearest shared layer if the behavior crosses Python and Node.
3. Edit in the lowest-level owner of the behavior.
4. Re-check that logic is not duplicated across:
   - `runtime` and language-specific surfaces
   - service wrappers and version-manager layers
   - quick-install config/orchestration/validator layers
5. Validate with:
   - `cargo fmt --check`
   - `cargo test --locked`
   - targeted tests when the change is localized

## Search shortcuts

- Find command handling and routing:
  - `rg -n "handle_.*command|match args.action|Commands::" src`
- Find the shared runtime abstraction and path status flow:
  - `rg -n "GenericRuntimeService|UsePathStatus|EnsureShimsResult|detect_use_path_status|ensure_shims_in_path" src/runtime src/python src/node`
- Find Python surface-aware flow:
  - `rg -n "PythonCommandSurface|install_python_for_surface|use_python_for_surface|uninstall_python_for_surface" src/python src/runtime`
- Find Python install internals:
  - `rg -n "resolve_target_version|resolve_latest|get_download_sources|download_installer|verify_installation|try_adopt_existing|recover_after_verification_failure" src/python`
- Find Python shim/config sync logic:
  - `rg -n "sync_python_shims_for_config|refresh_python_shims|current_python_executable|print_python_path_guidance" src/python src/config src/utils`
- Find Node surface-aware flow:
  - `rg -n "NodeCommandSurface|install_node_for_surface|use_node_for_surface|uninstall_node_for_surface" src/node src/runtime`
- Find Node install and project-version logic:
  - `rg -n "list_available_versions|resolve_target_version|resolve_latest_lts|resolve_project_version_from_nvmrc|build_download_url|verify_installation" src/node`
- Find Node shim/current-version behavior:
  - `rg -n "current_version_file|refresh_node_shims|remove_node_shims|print_node_path_guidance|command_matches_version" src/node src/utils`
- Find quick-install layering:
  - `rg -n "QuickInstallConfig|from_args|validate_with_config|QuickInstaller|verify_installation|build_step_failure_message" src/quick_install`
- Find pip executable resolution and package commands:
  - `rg -n "resolve_current_python_executable|current_python_executable|install\\(|uninstall\\(|upgrade\\(|list\\(" src/pip src/python`
- Find shared guidance / network help:
  - `rg -n "network_diagnostic_tips|quick_install_help_commands|print_python_path_guidance|print_node_path_guidance" src/utils src/quick_install src/python src/node`

## Validation shortcuts

- Broad repo check:
  - `cargo fmt --check`
  - `cargo test --locked`
- Targeted integration tests:
  - `cargo test --locked --test runtime_python_flow`
  - `cargo test --locked --test runtime_node_flow`
- Targeted quick-install example:
  - `cargo test --locked quick_install::installer::tests::install_latest_delegates_to_python_installer_directly`
