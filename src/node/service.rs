use crate::node::installer::{AvailableNodeVersion, NodeInstaller};
use crate::node::project::resolve_project_version_from_nvmrc;
use crate::node::version::{NodeVersion, NodeVersionManager, PathConfigResult};
use crate::utils::guidance::print_node_path_guidance;
use crate::utils::progress::moon_spinner_style;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::path::PathBuf;
use std::time::Duration;

/// Node.js 领域服务，统一封装安装、卸载、版本切换与 PATH 引导逻辑。
pub struct NodeService {
    manager: NodeVersionManager,
}

/// `node use` 后当前终端的可用状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeUsePathStatus {
    ShimsInPath,
    CommandReady,
    NeedsPathConfiguration,
}

/// 自动确保 shims 目录加入 PATH 的执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureShimsResult {
    JustConfigured,
    AlreadyConfigured,
    Failed { reason: String, shims_dir: PathBuf },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeCommandSurface {
    Node,
    Runtime,
}

impl NodeService {
    /// 构建 `NodeService`。
    pub fn new() -> Result<Self> {
        Ok(Self {
            manager: NodeVersionManager::new()?,
        })
    }

    /// 列出 MeetAI 管理目录下已安装的 Node.js 版本。
    pub fn list_installed(&self) -> Result<Vec<NodeVersion>> {
        self.manager.list_installed()
    }

    /// 列出官方可安装的 Node.js 版本（含 LTS 标记）。
    pub async fn list_available(&self) -> Result<Vec<AvailableNodeVersion>> {
        NodeInstaller::new()?.list_available_versions().await
    }

    /// 安装指定 Node.js 版本，返回实际安装版本号。
    pub async fn install(&self, version: &str) -> Result<String> {
        NodeInstaller::new()?.install(version).await
    }

    /// 卸载指定 Node.js 版本。
    pub fn uninstall(&self, version: &str) -> Result<()> {
        self.manager.uninstall(version)
    }

    /// 设置当前激活版本。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.manager.set_current_version(version)
    }

    /// 获取当前激活版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        self.manager.get_current_version()
    }

    /// 获取 shims 目录路径。
    pub fn shims_dir(&self) -> Result<PathBuf> {
        self.manager.shims_dir()
    }

    /// 检查 shims 目录是否已在当前 PATH。
    pub fn is_shims_in_path(&self) -> Result<bool> {
        self.manager.is_shims_in_path()
    }

    /// 检测 `node use` 后当前会话可用性状态。
    pub fn detect_use_path_status(&self, version: &str) -> Result<NodeUsePathStatus> {
        let shims_in_path = self.manager.is_shims_in_path()?;
        let command_ready = !shims_in_path && self.manager.node_command_matches_version(version);
        Ok(classify_use_path_status(shims_in_path, command_ready))
    }

    /// 尝试自动确保 shims 目录加入 PATH，并返回结果类型供上层做用户提示。
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        let result = self.manager.ensure_shims_in_path()?;
        map_ensure_shims_result(result, || self.manager.shims_dir())
    }

    /// 完成版本激活，并复用统一的 PATH 引导流程。
    pub fn activate_version(&self, version: &str) -> Result<()> {
        self.set_current_version(version)?;
        handle_node_use_path_setup(self, version)
    }
}

pub(crate) async fn install_node_for_surface(
    version: &str,
    surface: NodeCommandSurface,
) -> Result<()> {
    let service = NodeService::new()?;
    let installed_version = service
        .install(version)
        .await
        .with_context(|| build_install_failure_message(surface, version))?;

    println!("Node.js {} 已准备就绪。", installed_version);
    println!("下一步你可以执行：");
    println!(
        "  meetai runtime use nodejs {}   # 切换到该版本",
        installed_version
    );
    match surface {
        NodeCommandSurface::Node => {
            println!("  meetai node list                # 查看所有已安装版本");
        }
        NodeCommandSurface::Runtime => {
            println!("  meetai runtime list nodejs      # 查看所有已安装版本");
        }
    }

    Ok(())
}

pub(crate) fn use_node_for_surface(version: &str, surface: NodeCommandSurface) -> Result<()> {
    let service = NodeService::new()?;
    let resolved_version = if version == "project" {
        resolve_project_version_from_nvmrc()?
    } else {
        version.to_string()
    };
    service
        .activate_version(&resolved_version)
        .with_context(|| build_use_failure_message(surface, version))?;
    println!("✅ 已切换到 Node.js {}", resolved_version);
    match surface {
        NodeCommandSurface::Node => {
            println!("下一步你可以执行：");
            println!("  meetai node list            # 查看所有已安装版本");
        }
        NodeCommandSurface::Runtime => {
            println!("下一步你可以执行：");
            println!("  meetai runtime list nodejs  # 查看所有已安装版本");
        }
    }
    Ok(())
}

pub(crate) async fn uninstall_node_for_surface(
    version: &str,
    surface: NodeCommandSurface,
) -> Result<()> {
    let service = NodeService::new()?;
    service
        .uninstall(version)
        .with_context(|| build_uninstall_failure_message(surface, version))?;
    println!("✅ Node.js {} 已卸载", version);
    println!("下一步你可以执行：");
    match surface {
        NodeCommandSurface::Node => {
            println!("  meetai node list                    # 查看剩余版本");
            println!("  meetai node install <version>       # 安装指定版本");
        }
        NodeCommandSurface::Runtime => {
            println!("  meetai runtime list nodejs              # 查看剩余版本");
            println!("  meetai runtime install nodejs <version> # 安装指定版本");
        }
    }
    Ok(())
}

fn handle_node_use_path_setup(service: &NodeService, version: &str) -> Result<()> {
    match service.detect_use_path_status(version)? {
        NodeUsePathStatus::ShimsInPath => {
            println!("  运行 node --version / npm --version / npx --version 即可确认。");
        }
        NodeUsePathStatus::CommandReady => {
            println!("  当前终端已可直接使用目标版本，运行 node --version / npm --version 确认。");
            println!("  如果后续在其他终端未生效，请重启终端后再试。");
        }
        NodeUsePathStatus::NeedsPathConfiguration => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(moon_spinner_style());
            pb.enable_steady_tick(Duration::from_millis(120));
            pb.set_message("正在配置 PATH...");

            let result = service.ensure_shims_in_path()?;
            pb.finish_and_clear();

            match result {
                EnsureShimsResult::JustConfigured => {
                    println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                    println!(
                        "  重启终端后运行 node --version / npm --version 即可（仅需配置一次）。"
                    );
                }
                EnsureShimsResult::AlreadyConfigured => {
                    println!("  重启终端后运行 node --version / npm --version 即可生效。");
                }
                EnsureShimsResult::Failed { reason, shims_dir } => {
                    println!("自动配置 PATH 失败（{}）。", reason);
                    print_node_path_guidance(false, &shims_dir);
                }
            }
        }
    }
    Ok(())
}

fn build_install_failure_message(surface: NodeCommandSurface, version: &str) -> String {
    match surface {
        NodeCommandSurface::Node => format!(
            "Node.js 安装失败（请求版本: {}）。\n若为 Windows，请检查网络后重试；macOS/Linux 当前仅支持手动安装后切换。\n下一步你可以执行：\n  meetai node list\n  meetai runtime list nodejs",
            version
        ),
        NodeCommandSurface::Runtime => format!(
            "Node.js 安装失败（请求版本: {}）。\n若为 Windows，请检查网络后重试；macOS/Linux 当前仅支持手动安装后切换。\n下一步你可以执行：\n  meetai runtime list nodejs\n  meetai node list",
            version
        ),
    }
}

fn build_use_failure_message(surface: NodeCommandSurface, version: &str) -> String {
    match surface {
        NodeCommandSurface::Node => format!(
            "切换 Node.js 版本失败（目标版本: {}）。\n下一步你可以执行：\n  meetai node list\n  meetai runtime list nodejs",
            version
        ),
        NodeCommandSurface::Runtime => format!(
            "Node.js 版本切换失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list nodejs\n  meetai node list",
            version
        ),
    }
}

fn build_uninstall_failure_message(surface: NodeCommandSurface, version: &str) -> String {
    match surface {
        NodeCommandSurface::Node => format!(
            "卸载 Node.js 失败（目标版本: {}）。\n下一步你可以执行：\n  meetai node list\n  meetai runtime uninstall nodejs {}",
            version, version
        ),
        NodeCommandSurface::Runtime => format!(
            "Node.js 卸载失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list nodejs\n  meetai runtime uninstall nodejs {}",
            version, version
        ),
    }
}

fn classify_use_path_status(shims_in_path: bool, command_ready: bool) -> NodeUsePathStatus {
    if shims_in_path {
        NodeUsePathStatus::ShimsInPath
    } else if command_ready {
        NodeUsePathStatus::CommandReady
    } else {
        NodeUsePathStatus::NeedsPathConfiguration
    }
}

fn map_ensure_shims_result<F>(
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
            NodeUsePathStatus::ShimsInPath
        );
        assert_eq!(
            classify_use_path_status(true, true),
            NodeUsePathStatus::ShimsInPath
        );
    }

    #[test]
    fn classify_use_path_status_marks_command_ready_without_shims() {
        assert_eq!(
            classify_use_path_status(false, true),
            NodeUsePathStatus::CommandReady
        );
    }

    #[test]
    fn classify_use_path_status_requires_path_configuration_when_needed() {
        assert_eq!(
            classify_use_path_status(false, false),
            NodeUsePathStatus::NeedsPathConfiguration
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
                shims_dir,
            }
        );
    }
}
