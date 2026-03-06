use anyhow::{bail, Result};

use crate::cli::{RuntimeAction, RuntimeArgs, RuntimeType};
use crate::python::{
    install_python_for_surface, uninstall_python_for_surface, use_python_for_surface,
    PythonCommandSurface, PythonService,
};
use crate::utils::validator::Validator;

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
            let service = PythonService::new()?;
            let versions = service.list_installed()?;
            if versions.is_empty() {
                println!("当前还没有安装任何 {} 版本。", runtime.display_name());
                println!("下一步你可以执行：");
                if cfg!(windows) {
                    println!("  meetai runtime install python latest   # 安装最新稳定版");
                    println!("  meetai python install <version>        # 安装指定版本");
                } else {
                    println!("  当前平台暂不支持自动安装。");
                    println!("  meetai runtime list python             # 查看 MeetAI 已管理版本");
                    println!("  meetai runtime use python <version>    # 切换到已管理版本");
                }
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
            Validator::new().validate_python_install_version(version)?;
            install_python_for_surface(version, PythonCommandSurface::Runtime).await
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
            Validator::new().validate_python_selected_version(version)?;
            use_python_for_surface(version, PythonCommandSurface::Runtime)
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
            Validator::new().validate_python_selected_version(version)?;
            uninstall_python_for_surface(version, PythonCommandSurface::Runtime).await
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
    if cfg!(windows) {
        println!("  meetai runtime install python latest   # 立即安装 Python");
    } else {
        println!("  meetai runtime list python             # 查看 MeetAI 已管理版本");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_install_rejects_path_like_python_version() {
        let err = handle_runtime_command(RuntimeArgs {
            action: RuntimeAction::Install {
                runtime: RuntimeType::Python,
                version: "../3.13.2".to_string(),
            },
        })
        .await
        .expect_err("path-like version should be rejected before installation");

        assert!(
            err.to_string().contains("Python 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn runtime_use_rejects_latest_as_selected_version() {
        let err = handle_runtime_command(RuntimeArgs {
            action: RuntimeAction::Use {
                runtime: RuntimeType::Python,
                version: "latest".to_string(),
            },
        })
        .await
        .expect_err("latest should be rejected for runtime use");

        assert!(
            err.to_string().contains("Python 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
    }
}
