pub mod installer;
pub mod service;
pub mod version;

pub use service::NodeService;
pub use version::NodeVersionManager;

pub(crate) use service::{
    install_node_for_surface, uninstall_node_for_surface, use_node_for_surface, NodeCommandSurface,
};

use crate::cli::{NodeAction, NodeArgs};
use crate::utils::validator::Validator;
use anyhow::Result;
use semver::Version;
use std::path::{Path, PathBuf};

/// 规范化版本 token：去除 `v` 前缀并确认为合法 semver，返回 `X.Y.Z` 形式字符串。
pub(super) fn normalize_version_token(raw: &str) -> Option<String> {
    let token = raw.trim();
    let token = token.strip_prefix('v').unwrap_or(token);
    Version::parse(token).ok().map(|v| v.to_string())
}

/// 返回指定安装目录内的 node 可执行文件路径（平台差异封装）。
pub(super) fn node_executable_in_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.join("node.exe")
    } else {
        install_dir.join("bin/node")
    }
}

/// 处理 Node.js 相关命令
pub async fn handle_node_command(args: NodeArgs) -> Result<()> {
    let validator = Validator::new();

    match args.action {
        NodeAction::List => {
            let service = NodeService::new()?;
            let versions = service.list_installed()?;
            let current = service.get_current_version()?;

            if versions.is_empty() {
                println!("当前还没有安装任何 Node.js 版本。");
                println!("下一步你可以执行：");
                println!("  meetai node install latest   # 安装最新稳定版（Windows）");
                println!("  meetai runtime list nodejs   # 统一入口查看");
            } else {
                println!("已安装的 Node.js 版本（共 {} 个）：", versions.len());
                for version in versions {
                    let version_text = version.to_string();
                    if current.as_deref() == Some(version_text.as_str()) {
                        println!("  - {}  (current)", version_text);
                    } else {
                        println!("  - {}", version_text);
                    }
                }
                println!("下一步你可以执行：");
                println!("  meetai node use <version>      # 切换当前版本");
                println!("  meetai runtime list nodejs     # 统一入口查看");
            }
        }
        NodeAction::Install { version } => {
            validator.validate_node_install_version(&version)?;
            install_node_for_surface(&version, NodeCommandSurface::Node).await?;
        }
        NodeAction::Use { version } => {
            validator.validate_node_selected_version(&version)?;
            use_node_for_surface(&version, NodeCommandSurface::Node)?;
        }
        NodeAction::Uninstall { version } => {
            validator.validate_node_selected_version(&version)?;
            uninstall_node_for_surface(&version, NodeCommandSurface::Node).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn node_install_rejects_path_like_version() {
        let err = handle_node_command(NodeArgs {
            action: NodeAction::Install {
                version: r"..\20.11.1".to_string(),
            },
        })
        .await
        .expect_err("path-like version should be rejected before install");

        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn node_uninstall_rejects_latest() {
        let err = handle_node_command(NodeArgs {
            action: NodeAction::Uninstall {
                version: "latest".to_string(),
            },
        })
        .await
        .expect_err("latest should not be accepted for uninstall");

        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn node_use_rejects_latest() {
        let err = handle_node_command(NodeArgs {
            action: NodeAction::Use {
                version: "latest".to_string(),
            },
        })
        .await
        .expect_err("latest should not be accepted for use");

        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn node_use_rejects_path_like_version() {
        let err = handle_node_command(NodeArgs {
            action: NodeAction::Use {
                version: "../20.11.1".to_string(),
            },
        })
        .await
        .expect_err("path-like version should be rejected for use");

        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn node_uninstall_rejects_path_like_version() {
        let err = handle_node_command(NodeArgs {
            action: NodeAction::Uninstall {
                version: "../20.11.1".to_string(),
            },
        })
        .await
        .expect_err("path-like version should be rejected for uninstall");

        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }
}
