use crate::node::installer::NodeInstaller;
use crate::node::version::{NodeVersion, NodeVersionManager};
use crate::utils::guidance::print_node_path_guidance;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Node.js 领域服务，统一封装安装、卸载、版本切换与 PATH 引导逻辑。
pub struct NodeService {
    installer: NodeInstaller,
    manager: NodeVersionManager,
}

/// `node use` 后当前终端的可用状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeUsePathStatus {
    ShimsInPath,
    CommandReady,
    NeedsPathConfiguration,
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
            installer: NodeInstaller::new()?,
            manager: NodeVersionManager::new()?,
        })
    }

    /// 列出 MeetAI 管理目录下已安装的 Node.js 版本。
    pub fn list_installed(&self) -> Result<Vec<NodeVersion>> {
        self.manager.list_installed()
    }

    /// 安装指定 Node.js 版本，返回实际安装版本号。
    pub async fn install(&self, version: &str) -> Result<String> {
        self.installer.install(version).await
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

    /// 检测 `node use` 后当前会话可用性状态。
    pub fn detect_use_path_status(&self, version: &str) -> Result<NodeUsePathStatus> {
        let shims_in_path = self.manager.is_shims_in_path()?;
        let command_ready = !shims_in_path && self.manager.node_command_matches_version(version);
        Ok(classify_use_path_status(shims_in_path, command_ready))
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
    service
        .set_current_version(version)
        .with_context(|| build_use_failure_message(surface, version))?;
    println!("✅ 已切换到 Node.js {}", version);
    handle_node_use_path_setup(&service, version)?;
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
            println!("  运行 node --version 即可确认。");
        }
        NodeUsePathStatus::CommandReady => {
            println!("  当前终端已可直接使用目标版本，运行 node --version 确认。");
            println!("  如果后续在其他终端未生效，请重启终端后再试。");
        }
        NodeUsePathStatus::NeedsPathConfiguration => {
            let shims_dir = service.shims_dir()?;
            print_node_path_guidance(false, &shims_dir);
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
}
