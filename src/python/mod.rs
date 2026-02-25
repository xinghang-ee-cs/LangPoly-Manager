pub mod environment;
pub mod installer;
pub mod version;

pub use environment::VenvManager;
pub use installer::PythonInstaller;
pub use version::PythonVersionManager;

use crate::cli::{PythonAction, PythonArgs, VenvAction, VenvArgs};
use crate::utils::guidance::print_python_path_guidance;
use crate::utils::progress::moon_spinner_style;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::time::Duration;
use version::PathConfigResult;

/// 处理 Python 相关命令
pub async fn handle_python_command(args: PythonArgs) -> Result<()> {
    match args.action {
        PythonAction::List => {
            let manager = PythonVersionManager::new()?;
            let versions = manager.list_installed()?;
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
            let installer = PythonInstaller::new()?;
            let installed_version = installer.install(&version).await.with_context(|| {
                format!(
                    "Python 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai python install latest",
                    version
                )
            })?;
            println!("Python {} 已准备就绪。", installed_version);
            println!("下一步你可以执行：");
            println!(
                "  meetai runtime use python {}   # 切换到该版本",
                installed_version
            );
            println!("  meetai python list      # 查看所有已安装版本");
        }
        PythonAction::Use { version } => {
            let manager = PythonVersionManager::new()?;
            manager.set_current_version(&version).with_context(|| {
                format!(
                    "切换 Python 版本失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime list python",
                    version
                )
            })?;
            println!("✅ 已切换到 Python {}", version);
            let python_command_ready = manager.python_command_matches_version(&version);
            if manager.is_shims_in_path()? {
                // 当前终端已能感知 shims，直接可用
                println!("  运行 python --version 即可确认。");
            } else if python_command_ready {
                println!("  当前终端已可直接使用目标版本，运行 python --version 确认。");
                println!("  如果后续在其他终端未生效，请重启终端后再试。");
            } else {
                let pb = ProgressBar::new_spinner();
                pb.set_style(moon_spinner_style());
                pb.enable_steady_tick(Duration::from_millis(120));
                pb.set_message("正在配置 PATH...");

                let result = manager.ensure_shims_in_path()?;
                pb.finish_and_clear();

                match result {
                    PathConfigResult::JustConfigured => {
                        // 首次配置：写入了永久 PATH，需重启终端一次
                        println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                        println!("  重启终端后运行 python --version 即可（仅需配置一次）。");
                    }
                    PathConfigResult::AlreadyConfigured => {
                        // 永久 PATH 已有 shims，但本次终端窗口未刷新（在配置前打开的终端）
                        println!("  重启终端后运行 python --version 即可生效。");
                    }
                    PathConfigResult::Failed(reason) => {
                        // 自动配置失败，回退到手动引导
                        let shims_dir = manager.shims_dir()?;
                        println!("自动配置 PATH 失败（{}）。", reason);
                        print_python_path_guidance(false, &shims_dir);
                    }
                }
            }
            println!("下一步你可以执行：");
            println!("  meetai python list   # 查看所有已安装版本");
        }
        PythonAction::Uninstall { version } => {
            let installer = PythonInstaller::new()?;
            installer.uninstall(&version).await.with_context(|| {
                format!(
                    "卸载 Python 失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime uninstall python {}",
                    version, version
                )
            })?;
            println!("✅ Python {} 已卸载", version);
            println!("下一步你可以执行：");
            println!("  meetai python list                 # 查看剩余版本");
            println!("  meetai python install latest       # 安装最新版本");
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
