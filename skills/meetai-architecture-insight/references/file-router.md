# File Router

Use this map to open the fewest files for each change type.

## 1. CLI syntax, command names, or new subcommands

- `src/cli.rs`
- `src/main.rs`
- Target module entry:
  - `src/runtime/mod.rs`
  - `src/python/mod.rs`
  - `src/pip/mod.rs`
  - `src/quick_install/mod.rs`

## 2. Python install failure, download source, latest resolution, mirror fallback

- `src/python/installer.rs`
- `src/python/installer/latest.rs`
- `src/python/installer/windows_installer.rs`
- `src/python/installer/verify.rs`
- `src/python/installer/adopt.rs`
- `src/utils/downloader.rs`
- `src/utils/guidance.rs`
- `src/runtime/mod.rs`
- `src/quick_install/installer.rs`

Reason: `PythonInstaller` now delegates to focused submodules; mirror fallback, verify, and adopt/copy behavior live in separate files.

## 3. Python version switch, PATH/shims behavior, python use effect

- `src/python/version.rs`
- `src/python/service.rs`
- `src/python/mod.rs`
- `src/runtime/mod.rs`
- `src/utils/guidance.rs`

Reason: switching writes current version + refreshes shims, while `service.rs` and `handle_python_use_path_setup` keep `python`/`runtime` behavior aligned.

## 4. Installation directory or data layout changes (.meetai, cache, venvs, python)

- `src/config.rs`
- `src/python/installer.rs`
- `src/python/version.rs`
- `src/python/environment.rs`

Reason: path policy belongs to `Config`; installers/managers must follow it.

## 5. Quick-install pipeline changes (step order, progress, optional runtimes)

- `src/quick_install/mod.rs`
- `src/quick_install/config.rs`
- `src/quick_install/installer.rs`
- `src/quick_install/validator.rs`
- `src/python/installer.rs`
- `src/utils/progress.rs`
- `src/utils/guidance.rs`

Reason: quick-install has split responsibilities (config validation vs post-install verify) and Python installation still delegates to Python installer flow.

## 6. Pip install/uninstall/upgrade/list or package validation hardening

- `src/pip/mod.rs`
- `src/pip/manager.rs`
- `src/pip/version.rs`
- `src/utils/validator.rs`
- `src/utils/executor.rs`

Reason: command-level input validation and execution plumbing are split across these files, with shared Python executable resolution in `src/pip/mod.rs`.

## 7. Error context quality, diagnostics, and user-facing prompts

- `src/utils/guidance.rs`
- `src/runtime/mod.rs`
- `src/python/mod.rs`
- `src/quick_install/mod.rs`
- `src/quick_install/installer.rs`
- `src/python/installer.rs`

Reason: avoid duplicated wording and inconsistent next-step suggestions.

## 8. Security review hotspots

- `src/utils/validator.rs` for input constraints
- `src/utils/executor.rs` for command execution error context
- `src/utils/downloader.rs` for safe temp-file download and rename
- `src/python/installer.rs` + `src/python/installer/*.rs` for installer source trust, fallback, verify, and copy/adopt logic

Focus checks:
- no option-like pip package/version injection
- no partial download artifact leak
- no duplicated or conflicting latest-resolution logic
- no path traversal in install copy/adopt paths

## 9. Test entry points by module

- CLI parsing: `src/cli.rs`
- Python installer core: `src/python/installer.rs` + `src/python/installer/*.rs`
- Python service decision mapping: `src/python/service.rs`
- Quick-install orchestration: `src/quick_install/installer.rs`
- Downloader and executor behavior: `src/utils/downloader.rs`, `src/utils/executor.rs`
- Validation rules: `src/utils/validator.rs`, `src/quick_install/config.rs`, `src/pip/mod.rs`

Run:
- `cargo test --locked`
- Targeted (example): `cargo test --locked quick_install::installer::tests::install_latest_delegates_to_python_installer_directly`
