pub mod environment;
pub mod installer;
pub mod service;
pub mod version;

pub use environment::VenvManager;
pub use installer::PythonInstaller;
pub use service::PythonService;
pub(crate) use service::{
    install_python_for_surface, uninstall_python_for_surface, use_python_for_surface,
    PythonCommandSurface,
};
pub use version::PythonVersionManager;

use crate::cli::{PythonAction, PythonArgs, VenvAction, VenvArgs};
use crate::utils::progress::moon_spinner_style;
use anyhow::Result;
use indicatif::ProgressBar;
use std::time::Duration;

/// 处理 Python 相关命令
pub async fn handle_python_command(args: PythonArgs) -> Result<()> {
    match args.action {
        PythonAction::List => {
            let service = PythonService::new()?;
            let versions = service.list_installed()?;
            if versions.is_empty() {
                println!("当前还没有安装任何 Python 版本。");
                println!("下一步你可以执行：");
                println!("  meetai python install latest   # 安装最新稳定版");
                println!("  meetai python install 3.13.2   # 安装指定版本");
            } else {
                println!("已安装的 Python 版本（共 {} 个）：", versions.len());
                for version in versions {
                    println!("  - {}", version);
                }
                println!("下一步你可以执行：");
                println!("  meetai runtime use python <version>    # 切换当前版本");
                println!("  meetai runtime list python     # 统一入口查看");
            }
        }
        PythonAction::Install { version } => {
            install_python_for_surface(&version, PythonCommandSurface::Python).await?;
        }
        PythonAction::Use { version } => {
            use_python_for_surface(&version, PythonCommandSurface::Python)?;
        }
        PythonAction::Uninstall { version } => {
            uninstall_python_for_surface(&version, PythonCommandSurface::Python).await?;
        }
    }
    Ok(())
}

/// 处理虚拟环境相关命令
pub async fn handle_venv_command(args: VenvArgs) -> Result<()> {
    match args.action {
        VenvAction::Create { name, target_dir } => {
            let manager = VenvManager::new()?;
            let pb = ProgressBar::new_spinner();
            pb.set_style(moon_spinner_style());
            pb.enable_steady_tick(Duration::from_millis(120));
            pb.set_message(format!("🐍 正在创建虚拟环境 {}...", name));
            manager.create(&name, &target_dir).await?;
            pb.finish_and_clear();
            println!("✅ 虚拟环境 {} 创建成功", name);
            println!("下一步你可以执行：");
            println!("  meetai venv activate {}   # 查看激活命令", name);
        }
        VenvAction::Activate { name } => {
            let manager = VenvManager::new()?;
            manager.activate(&name)?;
        }
        VenvAction::List => {
            let manager = VenvManager::new()?;
            let envs = manager.list()?;
            if envs.is_empty() {
                println!("当前还没有创建任何虚拟环境。");
                println!("  meetai venv create <名称>   # 创建一个新的虚拟环境");
            } else {
                println!("虚拟环境列表（共 {} 个）：", envs.len());
                for env in envs {
                    println!("  - {}", env);
                }
                println!("  meetai venv activate <名称>   # 获取激活命令");
            }
        }
    }
    Ok(())
}
