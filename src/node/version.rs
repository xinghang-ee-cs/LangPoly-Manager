//! Node.js 版本管理器实现。
//!
//! 本模块提供 Node.js 版本的检测、比较、安装目录管理和 shims 生成功能。
//! Node.js 的 npm 全局包使用每个版本独立的 `npm-global` prefix，并通过
//! shims 暴露全局包提供的 CLI。
//! 核心类型包括：
//! - `NodeVersion`: 表示单个 Node.js 版本，支持版本比较和显示
//! - `NodeVersionManager`: 管理多个 Node.js 版本，负责版本切换、shims 维护和 PATH 配置
//!
//! 主要功能：
//! 1. 版本解析：从目录名提取 `NodeVersion`，支持 `v` 前缀
//! 2. 版本比较：通过 `semver::Version` 实现语义化版本排序
//! 3. 版本列表：扫描安装目录，返回所有已安装版本
//! 4. 版本切换：通过 shims 目录和 PATH 配置实现当前版本激活
//! 5. shims 管理：生成平台特定的 Node.js、npm、npx 和 npm 全局 CLI 启动脚本
//! 6. npm 全局包隔离：为每个 Node.js 版本维护独立的 `npm-global` 目录
//! 7. PATH 检测：检查 shims 是否在 PATH 中，并提供修复指导
//!
//! 与 Python 版本管理器的差异：
//! - Node.js 版本号使用 `semver` 格式（`MAJOR.MINOR.PATCH`）
//! - 支持 `v` 前缀（如 `v18.17.0`），自动规范化
//! - 可执行文件名固定为 `node`（而非 `python`）
//! - 额外管理 `npm`、`npx` 和 npm 全局包 CLI 的 shims
//!
//! 目录结构：
//! ```text
//! <app_home>/
//! ├── versions/           # 各版本安装目录
//! │   ├── v18.17.0/
//! │   │   ├── bin/       # Unix
//! │   │   │   ├── node
//! │   │   │   ├── npm
//! │   │   │   └── npx
//! │   │   └── npm-global/ # 当前 Node.js 版本的 npm 全局包 prefix
//! │   │   └── node.exe   # Windows
//! │   └── v20.5.0/
//! └── shims/             # 版本选择器脚本
//!     ├── node           # 指向当前版本的 shim
//!     ├── npm            # 指向当前版本 npm 的 shim
//!     ├── npx            # 指向当前版本 npx 的 shim
//!     └── eslint         # npm 全局包 CLI shim 示例
//! ```
//!
//! 平台差异：
//! - Windows: 使用 PowerShell 脚本作为 shim，包含 stderr 前缀 echo
//! - Unix/macOS: 使用 shell 脚本作为 shim，支持 shebang 执行
//!
//! 错误处理：
//! - 版本解析失败返回 `anyhow::Error`
//! - 目录操作失败返回 `std::io::Error`
//! - 命令执行失败通过 `anyhow::Result` 传播
//! - shims 配置问题返回 `PathConfigResult` 枚举
//!
//! 测试：
//! - 模块内 `mod tests` 包含版本解析、shim 生成、PATH 配置等单元测试
//! - 验证 `v` 前缀处理、版本排序、平台特定路径

use crate::config::Config;
use crate::runtime::common::{PathConfigResult, RuntimeUninstaller, VersionManager};
use anyhow::{Context, Result};
use semver::Version;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Node.js 版本信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeVersion {
    pub version: Version,
    pub path: PathBuf,
}

impl std::fmt::Display for NodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl NodeVersion {
    fn from_dir_name(dir_name: &str, path: PathBuf) -> Option<Self> {
        let version = Self::extract_version(dir_name)?;
        Some(Self { version, path })
    }

    fn extract_version(dir_name: &str) -> Option<Version> {
        if let Ok(version) = Version::parse(dir_name) {
            return Some(version);
        }

        if let Some(stripped) = dir_name.strip_prefix('v') {
            if let Ok(version) = Version::parse(stripped) {
                return Some(version);
            }
        }

        if let Some(stripped) = dir_name.strip_prefix("node-v") {
            let token = stripped.split('-').next()?;
            if let Ok(version) = Version::parse(token) {
                return Some(version);
            }
        }

        None
    }
}

/// Node.js 版本管理器。
pub struct NodeVersionManager {
    config: Config,
}

impl NodeVersionManager {
    /// 创建 Node.js 版本管理器。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let manager = Self { config };
        manager.ensure_node_dirs()?;
        Ok(manager)
    }

    /// 列出已安装的 Node.js 版本（按版本从高到低）。
    pub fn list_installed(&self) -> Result<Vec<NodeVersion>> {
        let versions_dir = self.versions_dir()?;
        if !versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&versions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(version) = NodeVersion::from_dir_name(dir_name, path.clone()) {
                versions.push(version);
            }
        }

        versions.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(versions)
    }

    /// 设置当前使用的 Node.js 版本，并刷新 shims。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        let normalized = Self::normalize_version_token(version)
            .with_context(|| format!("Node.js 版本号格式不正确：{}", version))?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == normalized)
            .with_context(|| {
                format!(
                    "找不到已安装的 Node.js {} 版本，请先执行: meetai node list 确认版本号",
                    version
                )
            })?;

        let node_exe = Self::node_executable_in_dir(&target.path);
        if !node_exe.exists() {
            anyhow::bail!(
                "Node.js {} 的可执行文件不存在（{}），请尝试重新安装。",
                normalized,
                node_exe.display()
            );
        }

        let npm_exe = Self::npm_executable_in_dir(&target.path);
        let npx_exe = Self::npx_executable_in_dir(&target.path);

        fs::write(self.current_version_file()?, &normalized)
            .context("写入当前 Node.js 版本失败")?;
        self.refresh_node_shims(&node_exe, &npm_exe, &npx_exe)?;

        Ok(())
    }

    /// 获取当前 Node.js 版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        let file_path = self.current_version_file()?;
        if !file_path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&file_path).context("读取当前 Node.js 版本失败")?;
        let version = raw.trim().to_string();
        if version.is_empty() {
            return Ok(None);
        }
        Ok(Some(version))
    }

    /// 获取当前选中 Node.js 可执行文件路径，并校验存在性。
    pub fn current_node_executable(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        let current_version = self
            .get_current_version()?
            .context(missing_selection_message)?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == current_version)
            .with_context(|| {
                format!(
                    "当前 Node.js 版本 {} 不存在，请先执行: meetai node list",
                    current_version
                )
            })?;

        let node_exe = Self::node_executable_in_dir(&target.path);
        if !node_exe.exists() {
            anyhow::bail!(
                "当前 Node.js 可执行文件不存在：{}。请尝试重新安装或切换版本：meetai node use <version>",
                node_exe.display()
            );
        }
        Ok(node_exe)
    }

    pub fn install_dir_for_version(&self, version: &str) -> Result<PathBuf> {
        let normalized = Self::normalize_version_token(version)
            .with_context(|| format!("Node.js 版本号格式不正确：{}", version))?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == normalized)
            .with_context(|| {
                format!(
                    "找不到已安装的 Node.js {} 版本，请先执行: meetai node list",
                    version
                )
            })?;
        Ok(target.path.clone())
    }

    pub fn current_install_dir(&self, missing_selection_message: &'static str) -> Result<PathBuf> {
        let current_version = self
            .get_current_version()?
            .context(missing_selection_message)?;
        self.install_dir_for_version(&current_version)
    }

    pub fn current_npm_executable(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        let install_dir = self.current_install_dir(missing_selection_message)?;
        let npm_exe = Self::npm_executable_for_install_dir(&install_dir);
        if !npm_exe.exists() {
            anyhow::bail!("当前 npm 可执行文件不存在：{}", npm_exe.display());
        }
        Ok(npm_exe)
    }

    pub fn npm_executable_for_install_dir(install_dir: &Path) -> PathBuf {
        Self::npm_executable_in_dir(install_dir)
    }

    pub fn npm_global_prefix_for_install_dir(install_dir: &Path) -> PathBuf {
        install_dir.join("npm-global")
    }

    pub fn npm_global_bin_for_prefix(prefix: &Path) -> PathBuf {
        if cfg!(windows) {
            prefix.to_path_buf()
        } else {
            prefix.join("bin")
        }
    }

    pub fn ensure_current_npm_global_dirs(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        let prefix = Self::npm_global_prefix_for_install_dir(
            &self.current_install_dir(missing_selection_message)?,
        );
        fs::create_dir_all(Self::npm_global_bin_for_prefix(&prefix))
            .with_context(|| format!("创建 npm 全局 bin 目录失败：{}", prefix.display()))?;
        Ok(prefix)
    }

    pub fn refresh_current_global_cli_shims(&self) -> Result<()> {
        let install_dir = self
            .current_install_dir("还没有选择 Node.js 版本，请先执行: meetai node use <version>")?;
        let prefix = Self::npm_global_prefix_for_install_dir(&install_dir);
        let bin_dir = Self::npm_global_bin_for_prefix(&prefix);
        let shims_dir = self.shims_dir()?;
        fs::create_dir_all(&shims_dir)
            .with_context(|| format!("创建 shims 目录失败：{}", shims_dir.display()))?;

        Self::remove_generated_npm_cli_shims(&shims_dir)?;
        if !bin_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&bin_dir)
            .with_context(|| format!("读取 npm 全局 bin 目录失败：{}", bin_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !Self::is_npm_global_cli_candidate(&path) {
                continue;
            }

            let Some(command_name) = Self::npm_global_cli_shim_name(&path) else {
                continue;
            };
            if Self::is_reserved_shim_name(&command_name) {
                continue;
            }

            if cfg!(windows) {
                Self::write_windows_npm_global_cli_shim(&shims_dir, &command_name, &path)?;
            } else {
                Self::write_unix_npm_global_cli_shim(&shims_dir, &command_name, &path)?;
            }
        }

        Ok(())
    }

    /// 卸载指定 Node.js 版本。
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let normalized = Self::normalize_version_token(version)
            .with_context(|| format!("Node.js 版本号格式不正确：{}", version))?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == normalized)
            .with_context(|| {
                format!(
                    "找不到已安装的 Node.js {} 版本，请先执行: meetai node list 确认版本号",
                    version
                )
            })?;

        if self.get_current_version()?.as_deref() == Some(normalized.as_str()) {
            self.clear_current_version()?;
            self.remove_node_shims()?;
        }

        fs::remove_dir_all(&target.path).with_context(|| {
            format!(
                "卸载 Node.js {} 失败，无法删除目录：{}",
                normalized,
                target.path.display()
            )
        })?;

        Ok(())
    }

    /// 获取 shims 目录路径。
    pub fn shims_dir(&self) -> Result<PathBuf> {
        Ok(self.app_home_dir()?.join("shims"))
    }

    /// 检查 shims 是否已在当前 PATH。
    pub fn is_shims_in_path(&self) -> Result<bool> {
        let shims_dir = self.shims_dir()?;
        let Some(path_var) = env::var_os("PATH") else {
            return Ok(false);
        };

        for entry in env::split_paths(&path_var) {
            if Self::paths_equal(&entry, &shims_dir) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 检查当前终端 `node --version` 是否与目标版本一致。
    pub fn node_command_matches_version(&self, expected_version: &str) -> bool {
        let output = match Command::new("node").args(["--version"]).output() {
            Ok(output) if output.status.success() => output,
            _ => return false,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Self::parse_node_version_output(&stdout)
            .or_else(|| Self::parse_node_version_output(&stderr))
            .map(|version| version == expected_version)
            .unwrap_or(false)
    }

    /// 将 shims 目录自动加入用户级永久 PATH（仅首次调用时实际写入）。
    pub fn ensure_shims_in_path(&self) -> Result<PathConfigResult> {
        let shims_dir = self.shims_dir()?;
        Ok(Self::ensure_shims_in_path_platform(&shims_dir))
    }
    fn ensure_node_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.versions_dir()?).context("创建 Node.js 版本目录失败")?;
        fs::create_dir_all(self.shims_dir()?).context("创建 shims 目录失败")?;
        Ok(())
    }

    fn app_home_dir(&self) -> Result<PathBuf> {
        self.config.app_home_dir_path()
    }

    fn node_root_dir(&self) -> Result<PathBuf> {
        Ok(self.app_home_dir()?.join("nodejs"))
    }

    fn versions_dir(&self) -> Result<PathBuf> {
        Ok(self.node_root_dir()?.join("versions"))
    }

    fn current_version_file(&self) -> Result<PathBuf> {
        Ok(self.node_root_dir()?.join("current"))
    }

    fn clear_current_version(&self) -> Result<()> {
        let file_path = self.current_version_file()?;
        if file_path.exists() {
            fs::remove_file(file_path).context("清除当前 Node.js 版本失败")?;
        }
        Ok(())
    }

    fn node_executable_in_dir(install_dir: &Path) -> PathBuf {
        super::node_executable_in_dir(install_dir)
    }

    fn npm_executable_in_dir(install_dir: &Path) -> PathBuf {
        if cfg!(windows) {
            install_dir.join("npm.cmd")
        } else {
            install_dir.join("bin/npm")
        }
    }

    fn npx_executable_in_dir(install_dir: &Path) -> PathBuf {
        if cfg!(windows) {
            install_dir.join("npx.cmd")
        } else {
            install_dir.join("bin/npx")
        }
    }

    fn refresh_node_shims(&self, node_exe: &Path, npm_exe: &Path, npx_exe: &Path) -> Result<()> {
        let shims_dir = self.shims_dir()?;
        fs::create_dir_all(&shims_dir)
            .with_context(|| format!("创建 shims 目录失败：{}", shims_dir.display()))?;
        let install_dir = if cfg!(windows) {
            npm_exe.parent().unwrap_or_else(|| Path::new("."))
        } else {
            npm_exe
                .parent()
                .and_then(Path::parent)
                .unwrap_or_else(|| Path::new("."))
        };
        let npm_prefix = Self::npm_global_prefix_for_install_dir(install_dir);

        if cfg!(windows) {
            Self::write_windows_executable_shim(
                &shims_dir,
                "node.cmd",
                "MEETAI_NODE_EXE",
                node_exe,
                "Node.js",
                "meetai node use <version>",
                false,
            )?;
            Self::write_windows_npm_shim(
                &shims_dir,
                "npm.cmd",
                "MEETAI_NPM_EXE",
                npm_exe,
                &npm_prefix,
                true,
            )?;
            Self::write_windows_npm_shim(
                &shims_dir,
                "npx.cmd",
                "MEETAI_NPX_EXE",
                npx_exe,
                &npm_prefix,
                false,
            )?;
        } else {
            Self::write_unix_executable_shim(
                &shims_dir,
                "node",
                "MEETAI_NODE_EXE",
                node_exe,
                "Node.js",
                "meetai node use <version>",
            )?;
            Self::write_unix_npm_shim(
                &shims_dir,
                "npm",
                "MEETAI_NPM_EXE",
                npm_exe,
                &npm_prefix,
                true,
            )?;
            Self::write_unix_npm_shim(
                &shims_dir,
                "npx",
                "MEETAI_NPX_EXE",
                npx_exe,
                &npm_prefix,
                false,
            )?;
        }

        Ok(())
    }

    fn remove_node_shims(&self) -> Result<()> {
        let shims_dir = self.shims_dir()?;
        Self::remove_generated_npm_cli_shims(&shims_dir)?;
        let names = if cfg!(windows) {
            vec!["node.cmd", "npm.cmd", "npx.cmd"]
        } else {
            vec!["node", "npm", "npx"]
        };

        for name in names {
            let path = shims_dir.join(name);
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("删除 Node.js shim 失败：{}", path.display()))?;
            }
        }

        Ok(())
    }

    fn remove_generated_npm_cli_shims(shims_dir: &Path) -> Result<()> {
        if !shims_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(shims_dir)
            .with_context(|| format!("读取 shims 目录失败：{}", shims_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            if content.contains(Self::npm_global_cli_shim_marker()) {
                fs::remove_file(&path)
                    .with_context(|| format!("删除 npm 全局包 shim 失败：{}", path.display()))?;
            }
        }

        Ok(())
    }

    fn is_npm_global_cli_candidate(path: &Path) -> bool {
        if cfg!(windows) {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
        } else {
            true
        }
    }

    fn npm_global_cli_shim_name(path: &Path) -> Option<String> {
        let file_name = path.file_name()?.to_str()?;
        if cfg!(windows) {
            if file_name.to_ascii_lowercase().ends_with(".cmd") {
                return Some(file_name.to_string());
            }
            if file_name.to_ascii_lowercase().ends_with(".exe") {
                let stem = path.file_stem()?.to_str()?;
                return Some(format!("{stem}.cmd"));
            }
        }
        Some(file_name.to_string())
    }

    fn is_reserved_shim_name(name: &str) -> bool {
        let normalized = if cfg!(windows) {
            name.strip_suffix(".cmd").unwrap_or(name)
        } else {
            name
        };
        matches!(
            normalized.to_ascii_lowercase().as_str(),
            "node" | "npm" | "npx" | "meetai"
        )
    }

    fn npm_global_cli_shim_marker() -> &'static str {
        "Generated by MeetAI npm global CLI shim refresh"
    }

    fn write_windows_executable_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        runtime_name: &str,
        guidance: &str,
        use_call: bool,
    ) -> Result<()> {
        let executable_str = executable.display().to_string();
        let invoke = if use_call {
            format!("call \"%{}%\" %*\r\n", env_var)
        } else {
            format!("\"%{}%\" %*\r\n", env_var)
        };

        let runtime_name_echo = Self::escape_cmd_echo_text(runtime_name);
        let guidance_echo = Self::escape_cmd_echo_text(guidance);
        let script = format!(
            "@echo off\r\nset \"{env_var}={exe}\"\r\nif not exist \"%{env_var}%\" (\r\n  >&2 echo [meetai] 当前 {runtime_name} 可执行文件不存在: %{env_var}%\r\n  >&2 echo [meetai] 请先执行: {guidance}\r\n  exit /b 1\r\n)\r\n{invoke}",
            env_var = env_var,
            exe = executable_str,
            runtime_name = runtime_name_echo,
            guidance = guidance_echo,
            invoke = invoke
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 shim 失败：{}", shim_path.display()))?;
        Ok(())
    }

    fn write_windows_npm_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        prefix: &Path,
        refresh_after_global_change: bool,
    ) -> Result<()> {
        let executable_str = executable.display().to_string();
        let prefix_str = prefix.display().to_string();
        let refresh = if refresh_after_global_change {
            "\r\nset \"MEETAI_NPM_ACTION=%~1\"\r\nif /I \"%MEETAI_NPM_ACTION%\"==\"install\" meetai npm refresh-shims >NUL 2>NUL\r\nif /I \"%MEETAI_NPM_ACTION%\"==\"uninstall\" meetai npm refresh-shims >NUL 2>NUL\r\nif /I \"%MEETAI_NPM_ACTION%\"==\"update\" meetai npm refresh-shims >NUL 2>NUL\r\nif /I \"%MEETAI_NPM_ACTION%\"==\"upgrade\" meetai npm refresh-shims >NUL 2>NUL"
        } else {
            ""
        };
        let script = format!(
            "@echo off\r\nset \"{env_var}={exe}\"\r\nset \"NPM_CONFIG_PREFIX={prefix}\"\r\nset \"npm_config_prefix={prefix}\"\r\nif not exist \"%{env_var}%\" (\r\n  >&2 echo [meetai] 当前 npm 可执行文件不存在: %{env_var}%\r\n  >&2 echo [meetai] 请先执行: meetai node use ^<version^>\r\n  exit /b 1\r\n)\r\ncall \"%{env_var}%\" %*\r\nset \"MEETAI_NPM_EXIT=%ERRORLEVEL%\"{refresh}\r\nexit /b %MEETAI_NPM_EXIT%\r\n",
            env_var = env_var,
            exe = executable_str,
            prefix = prefix_str,
            refresh = refresh
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 npm shim 失败：{}", shim_path.display()))?;
        Ok(())
    }

    fn write_windows_npm_global_cli_shim(
        shims_dir: &Path,
        shim_name: &str,
        executable: &Path,
    ) -> Result<()> {
        let executable_str = executable.display().to_string();
        let script = format!(
            "@echo off\r\nrem {marker}\r\nset \"MEETAI_NPM_GLOBAL_CLI={exe}\"\r\nif not exist \"%MEETAI_NPM_GLOBAL_CLI%\" (\r\n  >&2 echo [meetai] npm 全局命令不存在: %MEETAI_NPM_GLOBAL_CLI%\r\n  >&2 echo [meetai] 请执行: meetai npm refresh-shims\r\n  exit /b 1\r\n)\r\ncall \"%MEETAI_NPM_GLOBAL_CLI%\" %*\r\nexit /b %ERRORLEVEL%\r\n",
            marker = Self::npm_global_cli_shim_marker(),
            exe = executable_str
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 npm 全局包 shim 失败：{}", shim_path.display()))?;
        Ok(())
    }

    fn write_unix_executable_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        runtime_name: &str,
        guidance: &str,
    ) -> Result<()> {
        let escaped = Self::escape_sh_single_quotes(&executable.display().to_string());

        let script = format!(
            "#!/usr/bin/env sh\n{env_var}='{executable}'\nif [ ! -x \"${env_var}\" ]; then\n  echo \"[meetai] 当前 {runtime_name} 可执行文件不存在: ${env_var}\" >&2\n  echo \"[meetai] 请先执行: {guidance}\" >&2\n  exit 1\nfi\nexec \"${env_var}\" \"$@\"\n",
            env_var = env_var,
            executable = escaped,
            runtime_name = runtime_name,
            guidance = guidance
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 shim 失败：{}", shim_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms)
                .with_context(|| format!("设置 shim 执行权限失败：{}", shim_path.display()))?;
        }

        Ok(())
    }

    fn write_unix_npm_global_cli_shim(
        shims_dir: &Path,
        shim_name: &str,
        executable: &Path,
    ) -> Result<()> {
        let escaped = Self::escape_sh_single_quotes(&executable.display().to_string());
        let script = format!(
            "#!/usr/bin/env sh\n# {marker}\nMEETAI_NPM_GLOBAL_CLI='{executable}'\nif [ ! -x \"$MEETAI_NPM_GLOBAL_CLI\" ]; then\n  echo \"[meetai] npm 全局命令不存在: $MEETAI_NPM_GLOBAL_CLI\" >&2\n  echo \"[meetai] 请执行: meetai npm refresh-shims\" >&2\n  exit 1\nfi\nexec \"$MEETAI_NPM_GLOBAL_CLI\" \"$@\"\n",
            marker = Self::npm_global_cli_shim_marker(),
            executable = escaped
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 npm 全局包 shim 失败：{}", shim_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms)
                .with_context(|| format!("设置 shim 执行权限失败：{}", shim_path.display()))?;
        }

        Ok(())
    }

    fn write_unix_npm_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        prefix: &Path,
        refresh_after_global_change: bool,
    ) -> Result<()> {
        let escaped = Self::escape_sh_single_quotes(&executable.display().to_string());
        let escaped_prefix = Self::escape_sh_single_quotes(&prefix.display().to_string());
        let refresh = if refresh_after_global_change {
            "\ncase \"${1:-}\" in install|uninstall|update|upgrade) meetai npm refresh-shims >/dev/null 2>/dev/null || true ;; esac"
        } else {
            ""
        };
        let script = format!(
            "#!/usr/bin/env sh\n{env_var}='{executable}'\nexport NPM_CONFIG_PREFIX='{prefix}'\nexport npm_config_prefix='{prefix}'\nif [ ! -x \"${env_var}\" ]; then\n  echo \"[meetai] 当前 npm 可执行文件不存在: ${env_var}\" >&2\n  echo \"[meetai] 请先执行: meetai node use <version>\" >&2\n  exit 1\nfi\n\"${env_var}\" \"$@\"\nstatus=$?{refresh}\nexit $status\n",
            env_var = env_var,
            executable = escaped,
            prefix = escaped_prefix,
            refresh = refresh
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 npm shim 失败：{}", shim_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms)
                .with_context(|| format!("设置 shim 执行权限失败：{}", shim_path.display()))?;
        }

        Ok(())
    }

    fn normalize_version_token(raw: &str) -> Option<String> {
        super::normalize_version_token(raw)
    }

    fn parse_node_version_output(output: &str) -> Option<String> {
        let first = output.split_whitespace().next()?;
        Self::normalize_version_token(first)
    }

    fn escape_cmd_echo_text(raw: &str) -> String {
        raw.replace('^', "^^")
            .replace('&', "^&")
            .replace('|', "^|")
            .replace('<', "^<")
            .replace('>', "^>")
            .replace('(', "^(")
            .replace(')', "^)")
    }

    fn escape_sh_single_quotes(raw: &str) -> String {
        raw.replace('\'', "'\"'\"'")
    }

    /// Windows 实现：通过 PowerShell 读写 HKCU\Environment 中的 Path。
    #[cfg(windows)]
    fn ensure_shims_in_path_platform(shims_dir: &Path) -> PathConfigResult {
        let shims_str = shims_dir.to_string_lossy();
        let shims_escaped = shims_str.replace('\'', "''");

        let script = format!(
            "$ErrorActionPreference='Stop';\
$s='{shims}';\
$p=[string][Environment]::GetEnvironmentVariable('Path','User');\
if(($p -split ';') -icontains $s){{Write-Output 'already'}}else{{\
if($p){{$np=$s+';'+$p}}else{{$np=$s}};\
[Environment]::SetEnvironmentVariable('Path',$np,'User');\
Write-Output 'added'}}",
            shims = shims_escaped
        );

        match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout == "already" {
                    PathConfigResult::AlreadyConfigured
                } else {
                    PathConfigResult::JustConfigured
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                PathConfigResult::Failed(if stderr.is_empty() {
                    "PowerShell 执行失败（退出码非零）".to_string()
                } else {
                    stderr
                })
            }
            Err(e) => PathConfigResult::Failed(format!("无法启动 PowerShell: {}", e)),
        }
    }

    /// Unix 实现：向 .bashrc / .zshrc / .bash_profile / .profile 追加 export 语句。
    #[cfg(not(windows))]
    fn ensure_shims_in_path_platform(shims_dir: &Path) -> PathConfigResult {
        use std::io::Write;

        let shims_str = shims_dir.to_string_lossy();
        let export_line = format!("\n# Added by MeetAI\nexport PATH=\"{}:$PATH\"\n", shims_str);

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return PathConfigResult::Failed("无法确定用户主目录".to_string()),
        };

        let candidates = [".bashrc", ".zshrc", ".bash_profile", ".profile"];

        for candidate in &candidates {
            let profile_path = home.join(candidate);
            if !profile_path.exists() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&profile_path) {
                if content.contains(shims_str.as_ref()) {
                    return PathConfigResult::AlreadyConfigured;
                }
            }
        }

        let mut written = false;
        for candidate in &candidates {
            let profile_path = home.join(candidate);
            if !profile_path.exists() {
                continue;
            }
            if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&profile_path) {
                if file.write_all(export_line.as_bytes()).is_ok() {
                    written = true;
                }
            }
        }

        if written {
            PathConfigResult::JustConfigured
        } else {
            PathConfigResult::Failed(
                "未找到可写入的 shell 配置文件（.bashrc/.zshrc/.bash_profile/.profile）"
                    .to_string(),
            )
        }
    }
    fn paths_equal(a: &Path, b: &Path) -> bool {
        if cfg!(windows) {
            let left = a.to_string_lossy().replace('/', "\\").to_lowercase();
            let right = b.to_string_lossy().replace('/', "\\").to_lowercase();
            left == right
        } else {
            a == b
        }
    }
}

impl VersionManager for NodeVersionManager {
    fn command_name(&self) -> &'static str {
        "node"
    }

    fn shims_dir(&self) -> Result<PathBuf> {
        NodeVersionManager::shims_dir(self)
    }

    fn is_shims_in_path(&self) -> Result<bool> {
        NodeVersionManager::is_shims_in_path(self)
    }

    fn command_matches_version(&self, expected_version: &str) -> bool {
        self.node_command_matches_version(expected_version)
    }

    fn ensure_shims_in_path(&self) -> Result<PathConfigResult> {
        NodeVersionManager::ensure_shims_in_path(self)
    }

    fn print_path_guidance(&self, shims_dir: &Path) {
        crate::utils::guidance::print_node_path_guidance(
            self.is_shims_in_path().unwrap_or(false),
            shims_dir,
        )
    }

    fn list_installed(&self) -> Result<Vec<String>> {
        let versions = NodeVersionManager::list_installed(self)?;
        Ok(versions.into_iter().map(|v| v.to_string()).collect())
    }

    fn get_current_version(&self) -> Result<Option<String>> {
        NodeVersionManager::get_current_version(self)
    }

    fn set_current_version(&self, version: &str) -> Result<()> {
        NodeVersionManager::set_current_version(self, version)
    }
}

#[async_trait::async_trait]
impl RuntimeUninstaller for NodeVersionManager {
    async fn uninstall_version(&self, version: &str) -> Result<()> {
        NodeVersionManager::uninstall(self, version)
    }
}

#[cfg(test)]
mod tests {
    use super::NodeVersionManager;
    use crate::config::Config;
    use crate::runtime::common::RuntimeUninstaller;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn make_manager(root: &std::path::Path) -> anyhow::Result<NodeVersionManager> {
        let config = Config {
            python_install_dir: root.join("python"),
            venv_dir: root.join("venvs"),
            cache_dir: root.join("cache"),
            current_python_version: None,
        };
        config.ensure_dirs()?;

        let manager = NodeVersionManager { config };
        manager.ensure_node_dirs()?;
        Ok(manager)
    }

    #[test]
    fn parse_node_version_output_supports_v_prefix() {
        let parsed = NodeVersionManager::parse_node_version_output("v20.11.1\r\n");
        assert_eq!(parsed.as_deref(), Some("20.11.1"));
    }

    #[test]
    fn parse_node_version_output_rejects_non_semver() {
        let parsed = NodeVersionManager::parse_node_version_output("node 20");
        assert!(parsed.is_none());
    }

    #[test]
    fn node_executable_in_dir_uses_platform_specific_location() {
        let install_dir = PathBuf::from("v20.11.1");
        let node_exe = NodeVersionManager::node_executable_in_dir(&install_dir);
        if cfg!(windows) {
            assert!(node_exe.ends_with("node.exe"));
        } else {
            assert!(node_exe.ends_with("bin/node"));
        }
    }

    #[test]
    fn windows_node_shim_uses_stderr_prefix_echo() {
        let temp = tempdir().expect("tempdir should be created");
        let shim_path = temp.path().join("node.cmd");
        NodeVersionManager::write_windows_executable_shim(
            temp.path(),
            "node.cmd",
            "MEETAI_NODE_EXE",
            &PathBuf::from(r"D:\Node\node.exe"),
            "Node.js",
            "meetai node use <version>",
            false,
        )
        .expect("shim should be written");

        let script = fs::read_to_string(shim_path).expect("shim should be readable");
        assert!(script.contains(">&2 echo [meetai] 当前 Node.js 可执行文件不存在"));
        assert!(script.contains(">&2 echo [meetai] 请先执行: meetai node use ^<version^>"));
        assert!(!script.contains(" 1>&2"));
    }

    #[test]
    fn refresh_current_global_cli_shims_generates_and_removes_npm_cli_shims() -> anyhow::Result<()>
    {
        let temp = tempdir()?;
        let manager = make_manager(temp.path())?;
        let install_dir = temp.path().join("nodejs").join("versions").join("v20.11.1");
        let bin_dir = if cfg!(windows) {
            install_dir.clone()
        } else {
            install_dir.join("bin")
        };
        fs::create_dir_all(&bin_dir)?;

        let node_exe = if cfg!(windows) {
            install_dir.join("node.exe")
        } else {
            bin_dir.join("node")
        };
        fs::write(&node_exe, b"node")?;
        if !cfg!(windows) {
            let npm = bin_dir.join("npm");
            fs::write(&npm, b"npm")?;
            let npx = bin_dir.join("npx");
            fs::write(&npx, b"npx")?;
        }

        fs::write(manager.current_version_file()?, "20.11.1")?;
        let prefix = NodeVersionManager::npm_global_prefix_for_install_dir(&install_dir);
        let global_bin = NodeVersionManager::npm_global_bin_for_prefix(&prefix);
        fs::create_dir_all(&global_bin)?;
        let cli_name = if cfg!(windows) {
            "eslint.cmd"
        } else {
            "eslint"
        };
        fs::write(global_bin.join(cli_name), b"eslint")?;

        manager.refresh_current_global_cli_shims()?;
        let shim_name = if cfg!(windows) {
            "eslint.cmd"
        } else {
            "eslint"
        };
        let shim_path = manager.shims_dir()?.join(shim_name);
        assert!(shim_path.exists());
        assert!(fs::read_to_string(&shim_path)?
            .contains(NodeVersionManager::npm_global_cli_shim_marker()));

        fs::remove_file(global_bin.join(cli_name))?;
        manager.refresh_current_global_cli_shims()?;
        assert!(!shim_path.exists());

        Ok(())
    }

    #[tokio::test]
    async fn runtime_uninstaller_trait_delegates_to_inherent_impl() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let manager: Arc<dyn RuntimeUninstaller> = Arc::new(make_manager(temp.path())?);

        let err = manager
            .uninstall_version("not-a-version")
            .await
            .expect_err("invalid version should reach inherent uninstall validation");
        assert!(
            !err.to_string().is_empty(),
            "uninstall error should come from inherent implementation"
        );

        Ok(())
    }
}
