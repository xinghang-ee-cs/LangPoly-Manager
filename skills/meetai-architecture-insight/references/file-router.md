# File Router

Use this map to open the fewest files for each change type.

## 1. CLI syntax, command names, argument schema, or new subcommands

- `src/main.rs`
- `src/cli.rs`
- `src/lib.rs`
- Target command module:
  - `src/runtime/mod.rs`
  - `src/python/mod.rs`
  - `src/node/mod.rs`
  - `src/pip/mod.rs`
  - `src/quick_install/mod.rs`

Reason: CLI shape lives in `cli.rs`, dispatch happens in `main.rs`, and public module wiring stays visible from `lib.rs`.

## 2. Shared runtime routing, surface alignment, or PATH/shims orchestration

- `src/runtime/mod.rs`
- `src/runtime/common.rs`
- `src/python/service.rs`
- `src/node/service.rs`

Reason: `runtime/mod.rs` routes top-level runtime commands, while `runtime/common.rs` owns shared orchestration and the service layers own surface-specific wording.

## 3. Python install failure, latest resolution, adopt/import, verify, or download fallback

- `src/python/installer.rs`
- `src/python/installer/latest.rs`
- `src/python/installer/adopt.rs`
- `src/python/installer/verify.rs`
- `src/python/installer/windows_installer.rs`
- `src/utils/downloader.rs`
- `src/utils/http_client.rs`
- `src/utils/guidance.rs`

Reason: Python install behavior is split into focused submodules, with shared downloader / HTTP / diagnostics support outside the installer folder.

## 4. Python use, shims, PATH behavior, current-version state, or config repair side effects

- `src/python/service.rs`
- `src/python/version.rs`
- `src/runtime/common.rs`
- `src/config.rs`
- `src/utils/guidance.rs`

Reason: activation is surfaced through the service layer, but version persistence, shim refresh, and config-triggered repair live below it.

## 5. Node install, available versions, latest/lts resolution, or `.nvmrc` project behavior

- `src/node/mod.rs`
- `src/node/service.rs`
- `src/node/installer.rs`
- `src/node/project.rs`
- `src/utils/http_client.rs`
- `src/utils/downloader.rs`

Reason: Node install and remote version listing stay in the installer, while `.nvmrc` parsing and project lookup stay in `project.rs`.

## 6. Node use, shims, PATH behavior, current-version file, or uninstall side effects

- `src/node/service.rs`
- `src/node/version.rs`
- `src/runtime/common.rs`
- `src/utils/guidance.rs`

Reason: Node surface flow is thin; current version state, shim refresh/removal, and PATH guidance hooks live in `version.rs`.

## 7. Config, app-home layout, directory policy, or legacy migration / repair

- `src/config.rs`
- `src/python/version.rs`
- `src/node/version.rs`
- `src/python/installer.rs`
- `src/node/installer.rs`

Reason: `config.rs` owns app-home policy and repair flow, while runtime layers consume those paths and may need adjustment when the layout changes.

## 8. Quick-install orchestration, validation, summary, or post-install verification

- `src/quick_install/mod.rs`
- `src/quick_install/config.rs`
- `src/quick_install/installer.rs`
- `src/quick_install/validator.rs`
- `src/python/installer.rs`
- `src/python/environment.rs`
- `src/node/service.rs`
- `src/utils/guidance.rs`

Reason: quick-install is explicitly layered, but it still delegates Python install, optional Node activation, venv creation, and user guidance to lower modules.

## 9. Pip executable resolution, package command behavior, or package/version validation

- `src/pip/mod.rs`
- `src/pip/manager.rs`
- `src/pip/version.rs`
- `src/python/version.rs`
- `src/utils/executor.rs`
- `src/utils/validator.rs`

Reason: pip command behavior depends on the currently selected Python executable and shared validation / execution helpers.

## 10. Shared network, downloader, executor, or HTTP policy changes

- `src/utils/http_client.rs`
- `src/utils/downloader.rs`
- `src/utils/executor.rs`
- `src/python/installer.rs`
- `src/node/installer.rs`
- `src/quick_install/installer.rs`

Reason: these helpers sit below runtime-specific flows and are reused by Python, Node, and quick-install.

## 11. Diagnostics, recovery wording, or user-facing guidance ownership

- `src/utils/guidance.rs`
- `src/python/service.rs`
- `src/node/service.rs`
- `src/runtime/common.rs`
- `src/quick_install/mod.rs`
- `src/quick_install/installer.rs`

Reason: keep reusable help text centralized in `guidance.rs`, while surface-specific next-step wording remains in Python/Node service layers.

## 12. Linux or non-Windows adaptation entry points

- `src/python/installer.rs`
- `src/node/installer.rs`
- `src/python/version.rs`
- `src/node/version.rs`
- `src/python/environment.rs`
- `src/runtime/common.rs`
- `src/utils/guidance.rs`
- `src/config.rs`

Reason: platform enablement should be split into three separate concerns before implementation:
- auto-install support
- use / shim / PATH support
- venv / shell guidance support

## 13. Security and robustness hotspots

- `src/utils/validator.rs`
- `src/utils/executor.rs`
- `src/utils/downloader.rs`
- `src/utils/http_client.rs`
- `src/python/installer.rs` + `src/python/installer/*.rs`
- `src/node/installer.rs`
- `src/node/project.rs`

Focus checks:
- reject path-like or option-like version/package input early
- avoid partial download artifact leaks
- keep verification after install/adopt/extract flows
- avoid duplicated latest-resolution logic across layers
- keep project-version parsing bounded to `.nvmrc` semantics

## 14. Test entry points by subsystem

- CLI parsing:
  - `src/cli.rs`
- Shared runtime routing:
  - `src/runtime/mod.rs`
  - `src/runtime/common.rs`
- Python service / version / installer:
  - `src/python/service.rs`
  - `src/python/version.rs`
  - `src/python/installer.rs` + `src/python/installer/*.rs`
  - `tests/runtime_python_flow.rs`
- Node service / version / installer / project:
  - `src/node/service.rs`
  - `src/node/version.rs`
  - `src/node/installer.rs`
  - `src/node/project.rs`
  - `tests/runtime_node_flow.rs`
- Quick-install:
  - `src/quick_install/config.rs`
  - `src/quick_install/installer.rs`
  - `src/quick_install/validator.rs`

Run:
- `cargo fmt --check`
- `cargo test --locked`
- `cargo test --locked --test runtime_python_flow`
- `cargo test --locked --test runtime_node_flow`
- `cargo test --locked quick_install::installer::tests::install_latest_delegates_to_python_installer_directly`
