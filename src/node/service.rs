use crate::node::installer::NodeInstaller;
use crate::node::project::resolve_project_version_from_nvmrc;
use crate::node::version::NodeVersionManager;
use crate::runtime::common::{
    EnsureShimsResult, GenericRuntimeService, RuntimeInstaller, RuntimeUninstaller, UsePathStatus,
    VersionManager,
};
use anyhow::{Context, Result};
use std::sync::Arc;

/// Node.js 领域服务。
///
/// 安装、卸载、激活与 PATH 引导等共享流程委托给 [`GenericRuntimeService`]，
/// 只有官方可安装版本列表这类 Node.js 专属能力仍直接访问底层安装器。
pub struct NodeService {
    runtime: GenericRuntimeService,
}

impl NodeService {
    /// 构建 `NodeService`，初始化版本管理器与安装器。
    pub fn new() -> Result<Self> {
        let version_manager_impl = Arc::new(NodeVersionManager::new()?);
        let version_manager: Arc<dyn VersionManager> = version_manager_impl.clone();
        let uninstaller: Arc<dyn RuntimeUninstaller> = version_manager_impl;
        let installer: Arc<dyn RuntimeInstaller> = Arc::new(NodeInstaller::new()?);
        Ok(Self {
            runtime: GenericRuntimeService::new(version_manager, installer, uninstaller),
        })
    }

    /// 列出已安装的 Node.js 版本。
    pub fn list_installed(&self) -> Result<Vec<String>> {
        self.runtime.list_installed()
    }

    /// 列出官方可安装的 Node.js 版本（含 LTS 标记）。
    pub async fn list_available(
        &self,
    ) -> Result<Vec<crate::node::installer::AvailableNodeVersion>> {
        // 该能力尚未进入共享 runtime 层，因此保留 Node 专属直连路径。
        let installer = NodeInstaller::new()?;
        installer.list_available_versions().await
    }

    /// 安装指定 Node.js 版本。
    pub async fn install(&self, version: &str) -> Result<String> {
        self.runtime.install(version).await
    }

    /// 卸载指定 Node.js 版本。
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        self.runtime.uninstall(version).await
    }

    /// 设置当前激活版本。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.runtime.set_current_version(version)
    }

    /// 获取当前激活版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        self.runtime.get_current_version()
    }

    /// 检测 `node use` 后的 PATH 状态。
    pub fn detect_use_path_status(&self, version: &str) -> Result<UsePathStatus> {
        self.runtime.detect_use_path_status(version)
    }

    /// 确保 shims 目录在 PATH 中。
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        self.runtime.ensure_shims_in_path()
    }

    /// 激活版本（设置版本 + PATH 引导）。
    pub fn activate_version(&self, version: &str) -> Result<()> {
        self.runtime.activate_version(version)
    }
}

/// `node use` 命令的调用表面（用于错误消息定制）。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeCommandSurface {
    Node,
    Runtime,
}

/// `meetai node install` 的统一入口（处理表面差异）。
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

/// `meetai node use` 的统一入口（处理表面差异）。
pub(crate) fn use_node_for_surface(version: &str, surface: NodeCommandSurface) -> Result<()> {
    let resolved_version = if version == "project" {
        resolve_project_version_from_nvmrc()?
    } else {
        version.to_string()
    };
    let service = NodeService::new()?;
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
            println!("  meetai runtime list nodejs   # 查看所有已安装版本");
        }
    }
    Ok(())
}

/// `meetai node uninstall` 的统一入口（处理表面差异）。
pub(crate) async fn uninstall_node_for_surface(
    version: &str,
    surface: NodeCommandSurface,
) -> Result<()> {
    let service = NodeService::new()?;
    service
        .uninstall(version)
        .await
        .with_context(|| build_uninstall_failure_message(surface, version))?;
    println!("✅ Node.js {} 已卸载", version);
    println!("下一步你可以执行：");
    match surface {
        NodeCommandSurface::Node => {
            println!("  meetai node list                      # 查看剩余版本");
            println!("  meetai node install latest            # 安装最新版本");
        }
        NodeCommandSurface::Runtime => {
            println!("  meetai runtime list nodejs            # 查看剩余版本");
            println!("  meetai runtime install nodejs latest   # 安装最新版本");
        }
    }
    Ok(())
}

/// 安装失败时的表面定制错误消息。
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

/// 切换版本失败时的表面定制错误消息。
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

/// 卸载失败时的表面定制错误消息。
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
