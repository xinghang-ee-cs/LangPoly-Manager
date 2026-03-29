pub mod manager;
pub mod version;

pub use manager::PipManager;

use crate::cli::{PipAction, PipArgs};
use crate::python::version::PythonVersionManager;
use crate::utils::validator::Validator;
use anyhow::Result;
use std::path::PathBuf;

pub(super) fn sanitize_terminal_text(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

pub(super) fn resolve_current_python_executable(
    missing_selection_message: &'static str,
) -> Result<PathBuf> {
    let version_manager = PythonVersionManager::new()?;
    version_manager.current_python_executable(missing_selection_message)
}

/// 处理 Pip 相关命令
pub async fn handle_pip_command(args: PipArgs) -> Result<()> {
    let validator = Validator::new();

    match args.action {
        PipAction::Install { package, version } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            let package_spec = if let Some(v) = version {
                validator.validate_pip_pin_version(&v)?;
                format!("{}=={}", package, v)
            } else {
                package
            };
            let display_spec = sanitize_terminal_text(&package_spec);
            manager.install(&package_spec).await?;
            println!("✅ {} 安装成功", display_spec);
            println!("  - 查看已安装的包: meetai pip list");
        }
        PipAction::Uninstall { package } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            manager.uninstall(&package).await?;
            println!("✅ {} 已卸载", sanitize_terminal_text(&package));
            println!("  - 查看已安装的包: meetai pip list");
        }
        PipAction::Upgrade { package } => {
            validator.validate_pip_package_name(&package)?;
            let manager = PipManager::new()?;
            manager.upgrade(&package).await?;
            println!("✅ {} 已升级到最新版本", sanitize_terminal_text(&package));
            println!("  - 查看版本: meetai pip list");
        }
        PipAction::List => {
            let manager = PipManager::new()?;
            let packages = manager.list().await?;
            if packages.is_empty() {
                println!("还没有安装任何包，来安装第一个吧！");
                println!("  meetai pip install <包名>   # 安装第一个包");
            } else {
                println!("📦 已安装的包（共 {} 个）:", packages.len());
                for pkg in packages {
                    println!("  - {}", pkg);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_terminal_text_replaces_control_characters() {
        let sanitized = sanitize_terminal_text("pip\nlist\t\x00ok");
        assert_eq!(sanitized, "pip list  ok");
    }
}
