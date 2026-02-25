use anyhow::{bail, Context, Result};
use indicatif::ProgressBar;
use std::time::Duration;

use crate::cli::{RuntimeAction, RuntimeArgs, RuntimeType};
use crate::python::version::PathConfigResult;
use crate::python::{PythonInstaller, PythonVersionManager};
use crate::utils::guidance::print_python_path_guidance;
use crate::utils::progress::moon_spinner_style;

/// 处理统一 runtime 命令
pub async fn handle_runtime_command(args: RuntimeArgs) -> Result<()> {
    match args.action {
        RuntimeAction::List { runtime } => {
            if let Some(target) = runtime {
                list_runtime_versions(target)?;
            } else {
                print_supported_runtime_matrix();
            }
        }
        RuntimeAction::Install { runtime, version } => {
            install_runtime(runtime, &version).await?;
        }
        RuntimeAction::Use { runtime, version } => {
            use_runtime(runtime, &version)?;
        }
        RuntimeAction::Uninstall { runtime, version } => {
            uninstall_runtime(runtime, &version).await?;
        }
    }

    Ok(())
}

fn list_runtime_versions(runtime: RuntimeType) -> Result<()> {
    match runtime {
        RuntimeType::Python => {
            let manager = PythonVersionManager::new()?;
            let versions = manager.list_installed()?;
            if versions.is_empty() {
                println!("当前还没有安装任何 {} 版本。", runtime.display_name());
                println!("下一步你可以执行：");
                println!("  meetai runtime install python latest   # 安装最新稳定版");
                println!("  meetai python install <version>        # 安装指定版本");
            } else {
                println!(
                    "已安装的 {} 版本（共 {} 个）：",
                    runtime.display_name(),
                    versions.len()
                );
                for version in versions {
                    println!("  - {}", version);
                }
                println!("下一步你可以执行：");
                println!("  meetai runtime use python <version>    # 切换版本");
                println!("  meetai python list                     # Python 专项管理");
            }
        }
        RuntimeType::Nodejs | RuntimeType::Java | RuntimeType::Go => {
            println!("{} 的自动安装即将开放，敬请期待。", runtime.display_name());
            println!("你可以先用官方安装包手动安装，MeetAI 后续版本将支持统一管理。");
        }
    }

    Ok(())
}

async fn install_runtime(runtime: RuntimeType, version: &str) -> Result<()> {
    match runtime {
        RuntimeType::Python => {
            let installer = PythonInstaller::new()?;
            let installed_version = installer.install(version).await.with_context(|| {
                format!(
                    "{} 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime install python latest",
                    runtime.display_name(),
                    version
                )
            })?;
            println!(
                "{} {} 已准备就绪。",
                runtime.display_name(),
                installed_version
            );
            println!("下一步你可以执行：");
            println!(
                "  meetai runtime use python {}   # 切换到该版本",
                installed_version
            );
            println!("  meetai runtime list python      # 查看所有已安装版本");
            Ok(())
        }
        RuntimeType::Nodejs | RuntimeType::Java | RuntimeType::Go => {
            bail!(
                "{} 的自动安装即将开放，当前版本请手动安装。Node.js / Java / Go 支持正在积极开发中。",
                runtime.display_name()
            )
        }
    }
}

fn use_runtime(runtime: RuntimeType, version: &str) -> Result<()> {
    match runtime {
        RuntimeType::Python => {
            let manager = PythonVersionManager::new()?;
            manager.set_current_version(version).with_context(|| {
                format!(
                    "{} 版本切换失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai python list",
                    runtime.display_name(),
                    version
                )
            })?;
            println!("✅ 已切换到 {} {}", runtime.display_name(), version);
            let python_command_ready = manager.python_command_matches_version(version);
            if manager.is_shims_in_path()? {
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
                        println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                        println!("  重启终端后运行 python --version 即可（仅需配置一次）。");
                    }
                    PathConfigResult::AlreadyConfigured => {
                        println!("  重启终端后运行 python --version 即可生效。");
                    }
                    PathConfigResult::Failed(reason) => {
                        let shims_dir = manager.shims_dir()?;
                        println!("自动配置 PATH 失败（{}）。", reason);
                        print_python_path_guidance(false, &shims_dir);
                    }
                }
            }
            println!("  meetai runtime list python   # 查看所有已安装版本");
            Ok(())
        }
        RuntimeType::Nodejs | RuntimeType::Java | RuntimeType::Go => {
            bail!(
                "{} 的版本切换即将开放，当前版本仅支持 Python。",
                runtime.display_name()
            )
        }
    }
}

async fn uninstall_runtime(runtime: RuntimeType, version: &str) -> Result<()> {
    match runtime {
        RuntimeType::Python => {
            let installer = PythonInstaller::new()?;
            installer.uninstall(version).await.with_context(|| {
                format!(
                    "{} 卸载失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime uninstall python {}",
                    runtime.display_name(),
                    version,
                    version
                )
            })?;
            println!("✅ {} {} 已卸载", runtime.display_name(), version);
            println!("下一步你可以执行：");
            println!("  meetai runtime list python              # 查看剩余版本");
            println!("  meetai runtime install python latest    # 安装最新版本");
            Ok(())
        }
        RuntimeType::Nodejs | RuntimeType::Java | RuntimeType::Go => {
            bail!(
                "{} 的卸载即将开放，当前版本仅支持 Python。",
                runtime.display_name()
            )
        }
    }
}

fn print_supported_runtime_matrix() {
    println!("MeetAI 运行时管理支持情况：");
    println!("  ✅ Python   已支持（安装 / 切换 / 卸载）");
    println!("  🔜 Node.js  即将开放");
    println!("  🔜 Java     即将开放");
    println!("  🔜 Go       即将开放");
    println!();
    println!("  meetai runtime install python latest   # 立即安装 Python");
}
