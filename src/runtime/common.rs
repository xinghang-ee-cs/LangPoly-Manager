//! 运行时服务的共享抽象层。
//!
//! 本模块只保留真正跨运行时通用的流程：
//! - PATH / shims 状态检测
//! - 当前版本激活后的引导
//! - 安装、卸载能力的统一调度
//!
//! 边界按职责拆分为三类能力，避免把“安装”和“卸载”强塞进同一个抽象后再次出现
//! 递归委托或职责错位：
//! - `VersionManager`: 版本查询、切换、PATH / shims 管理
//! - `RuntimeInstaller`: 安装能力
//! - `RuntimeUninstaller`: 卸载能力
//!
//! 共享层不再承载各运行时的错误消息模板；Python / Node 仍在各自 service 中保留
//! 面向命令表面的文案，以免共享抽象反向侵入领域语义。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use crate::runtime::common::{
//!     GenericRuntimeService, RuntimeInstaller, RuntimeUninstaller, VersionManager,
//! };
//! use std::sync::Arc;
//!
//! let version_manager: Arc<dyn VersionManager> = Arc::new(MyVersionManager::new()?);
//! let installer_impl = Arc::new(MyInstaller::new()?);
//! let installer: Arc<dyn RuntimeInstaller> = installer_impl.clone();
//! let uninstaller: Arc<dyn RuntimeUninstaller> = installer_impl;
//! let service = GenericRuntimeService::new(version_manager, installer, uninstaller);
//!
//! let versions = service.list_installed()?;
//! service.install("latest").await?;
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::utils::progress::moon_spinner_style;
use anyhow::Result;
use async_trait::async_trait;
use indicatif::ProgressBar;

/// 共享的 PATH 配置结果枚举。
///
/// 该类型由 `runtime/common.rs` 统一定义，并由 Python/Node 的版本管理器复用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathConfigResult {
    /// shims 目录已在用户级永久 PATH 中，无需重复配置。
    AlreadyConfigured,
    /// shims 目录本次已成功加入用户级永久 PATH。
    JustConfigured,
    /// 自动配置失败，含失败原因（供回退到手动提示时使用）。
    Failed(String),
}

/// 版本管理器 trait - 管理特定运行时的版本、shims 和 PATH。
///
/// 封装了与 PATH、shims、命令检测相关的操作。这些操作对于所有运行时都是相同的，
/// 因此提取到 trait 中共享实现。此 trait 是 `dyn` safe 的，可用于 trait 对象。
///
/// # 设计说明
/// - 所有方法均为同步，因为涉及文件系统和环境变量操作
/// - 版本信息以 `String` 形式传递，避免在 trait 中暴露具体版本类型
pub trait VersionManager: Send + Sync {
    /// 获取运行时命令行工具名称（如 "python"、"node"）。
    fn command_name(&self) -> &'static str;

    /// 获取 shims 目录路径。
    fn shims_dir(&self) -> Result<PathBuf>;

    /// 检查 shims 目录是否已在当前 PATH 中。
    fn is_shims_in_path(&self) -> Result<bool>;

    /// 检查当前终端直接执行 `command_name() --version` 是否命中目标版本。
    fn command_matches_version(&self, expected_version: &str) -> bool;

    /// 自动确保 shims 目录加入用户级永久 PATH（仅首次写入）。
    fn ensure_shims_in_path(&self) -> Result<PathConfigResult>;

    /// 打印 PATH 配置指导（自动配置失败时调用）。
    fn print_path_guidance(&self, shims_dir: &Path);

    /// 列出所有已安装版本（返回版本字符串列表）。
    fn list_installed(&self) -> Result<Vec<String>>;

    /// 获取当前激活版本。
    fn get_current_version(&self) -> Result<Option<String>>;

    /// 设置当前激活版本。
    fn set_current_version(&self, version: &str) -> Result<()>;
}

/// 安装能力 trait。
///
/// trait 方法显式使用 `install_version` 命名，避免与具体类型的固有方法同名后再次出现
/// 递归委托风险。
#[async_trait]
pub trait RuntimeInstaller: Send + Sync {
    /// 安装指定版本，返回实际安装的版本号。
    async fn install_version(&self, version: &str) -> Result<String>;
}

/// 卸载能力 trait。
///
/// 卸载并不总是属于“安装器”本身，因此单独拆分为能力边界：
/// - Python 由 `PythonInstaller` 实现
/// - Node.js 由 `NodeVersionManager` 实现
#[async_trait]
pub trait RuntimeUninstaller: Send + Sync {
    /// 卸载指定版本。
    async fn uninstall_version(&self, version: &str) -> Result<()>;
}

/// 运行时版本使用 PATH 的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsePathStatus {
    /// shims 目录已在 PATH 中，直接可用。
    ShimsInPath,
    /// shims 不在 PATH，但当前终端可直接执行目标版本命令。
    CommandReady,
    /// 需要配置 PATH 才能使用。
    NeedsPathConfiguration,
}

/// 自动确保 shims 目录加入 PATH 的执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureShimsResult {
    /// 本次首次配置（写入了永久 PATH）。
    JustConfigured,
    /// 之前已配置过（永久 PATH 中已有 shims）。
    AlreadyConfigured,
    /// 配置失败，包含失败原因和 shims 目录。
    Failed { reason: String, shims_dir: PathBuf },
}

/// 通用运行时服务 - 封装所有共享逻辑。
///
/// 此结构体是 Python/Node/Java 等运行时服务的核心实现，
/// 通过组合三类职责边界来实现共享编排，而不强迫具体运行时接受不自然的抽象。
///
/// 所有与 PATH 配置、状态机、用户引导相关的逻辑都在这里集中实现，
/// 具体的运行时服务只需提供具体的版本管理、安装、卸载能力即可。
pub struct GenericRuntimeService {
    version_manager: Arc<dyn VersionManager>,
    installer: Arc<dyn RuntimeInstaller>,
    uninstaller: Arc<dyn RuntimeUninstaller>,
}

impl GenericRuntimeService {
    /// 创建新的运行时服务。
    pub fn new(
        version_manager: Arc<dyn VersionManager>,
        installer: Arc<dyn RuntimeInstaller>,
        uninstaller: Arc<dyn RuntimeUninstaller>,
    ) -> Self {
        Self {
            version_manager,
            installer,
            uninstaller,
        }
    }

    /// 列出所有已安装版本。
    pub fn list_installed(&self) -> Result<Vec<String>> {
        self.version_manager.list_installed()
    }

    /// 安装指定版本。
    pub async fn install(&self, version: &str) -> Result<String> {
        self.installer.install_version(version).await
    }

    /// 卸载指定版本。
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        self.uninstaller.uninstall_version(version).await
    }

    /// 获取当前激活版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        self.version_manager.get_current_version()
    }

    /// 设置当前激活版本。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.version_manager.set_current_version(version)
    }

    /// 检测 `use` 命令后当前会话的可用性状态。
    pub fn detect_use_path_status(&self, version: &str) -> Result<UsePathStatus> {
        let shims_in_path = self.version_manager.is_shims_in_path()?;
        let command_ready = !shims_in_path && self.version_manager.command_matches_version(version);
        Ok(classify_use_path_status(shims_in_path, command_ready))
    }

    /// 自动确保 shims 目录加入 PATH。
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        let result = self.version_manager.ensure_shims_in_path()?;
        map_ensure_shims_result(result, || self.version_manager.shims_dir())
    }

    /// 完成版本激活，并复用统一的 PATH 引导流程。
    pub fn activate_version(&self, version: &str) -> Result<()> {
        self.set_current_version(version)?;
        self.handle_path_setup(version)
    }

    /// 处理 `use` 命令的后续 PATH 引导流程。
    pub fn handle_path_setup(&self, version: &str) -> Result<()> {
        match self.detect_use_path_status(version)? {
            UsePathStatus::ShimsInPath => {
                println!(
                    "  运行 {} --version 即可确认。",
                    self.version_manager.command_name()
                );
            }
            UsePathStatus::CommandReady => {
                println!(
                    "  当前终端已可直接使用目标版本，运行 {} --version 确认。",
                    self.version_manager.command_name()
                );
                println!("  如果后续在其他终端未生效，请重启终端后再试。");
            }
            UsePathStatus::NeedsPathConfiguration => {
                let pb = ProgressBar::new_spinner();
                pb.set_style(moon_spinner_style());
                pb.enable_steady_tick(Duration::from_millis(120));
                pb.set_message("正在配置 PATH...");

                let result = self.ensure_shims_in_path()?;
                pb.finish_and_clear();

                match result {
                    EnsureShimsResult::JustConfigured => {
                        println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                        println!(
                            "  重启终端后运行 {} --version 即可（仅需配置一次）。",
                            self.version_manager.command_name()
                        );
                    }
                    EnsureShimsResult::AlreadyConfigured => {
                        println!(
                            "  重启终端后运行 {} --version 即可生效。",
                            self.version_manager.command_name()
                        );
                    }
                    EnsureShimsResult::Failed { reason, shims_dir } => {
                        println!("自动配置 PATH 失败（{}）。", reason);
                        self.version_manager.print_path_guidance(&shims_dir);
                    }
                }
            }
        }

        Ok(())
    }
}

/// 根据 shims_in_path 和 command_ready 两个布尔值，分类 UsePathStatus。
pub fn classify_use_path_status(shims_in_path: bool, command_ready: bool) -> UsePathStatus {
    if shims_in_path {
        UsePathStatus::ShimsInPath
    } else if command_ready {
        UsePathStatus::CommandReady
    } else {
        UsePathStatus::NeedsPathConfiguration
    }
}

/// 将 PathConfigResult 映射为 EnsureShimsResult，并在失败时调用 loader 获取 shims_dir。
pub fn map_ensure_shims_result<F>(
    result: PathConfigResult,
    shims_dir_loader: F,
) -> Result<EnsureShimsResult>
where
    F: FnOnce() -> Result<PathBuf>,
{
    let mapped = match result {
        PathConfigResult::JustConfigured => EnsureShimsResult::JustConfigured,
        PathConfigResult::AlreadyConfigured => EnsureShimsResult::AlreadyConfigured,
        PathConfigResult::Failed(reason) => EnsureShimsResult::Failed {
            reason,
            shims_dir: shims_dir_loader()?,
        },
    };
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_use_path_status_prefers_shims_in_path() {
        assert_eq!(
            classify_use_path_status(true, false),
            UsePathStatus::ShimsInPath
        );
        assert_eq!(
            classify_use_path_status(true, true),
            UsePathStatus::ShimsInPath
        );
    }

    #[test]
    fn classify_use_path_status_marks_command_ready_without_shims() {
        assert_eq!(
            classify_use_path_status(false, true),
            UsePathStatus::CommandReady
        );
    }

    #[test]
    fn classify_use_path_status_requires_path_configuration_when_needed() {
        assert_eq!(
            classify_use_path_status(false, false),
            UsePathStatus::NeedsPathConfiguration
        );
    }

    #[test]
    fn map_ensure_shims_result_passes_through_success_states() {
        assert_eq!(
            map_ensure_shims_result(PathConfigResult::JustConfigured, || {
                panic!("shims_dir_loader should not be called")
            })
            .expect("success mapping should not fail"),
            EnsureShimsResult::JustConfigured,
        );
        assert_eq!(
            map_ensure_shims_result(PathConfigResult::AlreadyConfigured, || {
                panic!("shims_dir_loader should not be called")
            })
            .expect("success mapping should not fail"),
            EnsureShimsResult::AlreadyConfigured,
        );
    }

    #[test]
    fn map_ensure_shims_result_preserves_failure_reason_and_path() {
        let shims_dir = PathBuf::from(".meetai/shims");
        let mapped = map_ensure_shims_result(
            PathConfigResult::Failed("permission denied".to_string()),
            || Ok(shims_dir.clone()),
        )
        .expect("failed mapping should include shims path");
        assert_eq!(
            mapped,
            EnsureShimsResult::Failed {
                reason: "permission denied".to_string(),
                shims_dir
            }
        );
    }
}
