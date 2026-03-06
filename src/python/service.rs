use crate::python::version::{PathConfigResult, PythonVersion};
use crate::python::{PythonInstaller, PythonVersionManager};
use crate::utils::guidance::print_python_path_guidance;
use crate::utils::progress::moon_spinner_style;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use std::path::PathBuf;
use std::time::Duration;

/// Python 领域服务，统一封装安装、卸载、版本切换与 PATH 引导逻辑。
pub struct PythonService {
    installer: PythonInstaller,
    manager: PythonVersionManager,
}

/// `python use` 后当前终端的可用状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PythonUsePathStatus {
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
pub(crate) enum PythonCommandSurface {
    Python,
    Runtime,
}

impl PythonService {
    /// 构建 `PythonService`，并初始化安装器与版本管理器依赖。
    pub fn new() -> Result<Self> {
        Ok(Self {
            installer: PythonInstaller::new()?,
            manager: PythonVersionManager::new()?,
        })
    }

    /// 列出 MeetAI 管理目录下已安装的 Python 版本。
    pub fn list_installed(&self) -> Result<Vec<PythonVersion>> {
        self.manager.list_installed()
    }

    /// 安装指定 Python 版本，返回实际安装版本号（支持 `latest` 解析）。
    pub async fn install(&self, version: &str) -> Result<String> {
        self.installer.install(version).await
    }

    /// 卸载指定 Python 版本。
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        self.installer.uninstall(version).await
    }

    /// 将指定版本设置为当前激活版本（更新 MeetAI 配置）。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.manager.set_current_version(version)
    }

    /// 检测 `python use` 后当前会话的可用性状态，用于决定是否需要 PATH 处理。
    pub fn detect_use_path_status(&self, version: &str) -> Result<PythonUsePathStatus> {
        let shims_in_path = self.manager.is_shims_in_path()?;
        let command_ready = !shims_in_path && self.manager.python_command_matches_version(version);
        Ok(classify_use_path_status(shims_in_path, command_ready))
    }

    /// 尝试自动确保 shims 目录加入 PATH，并返回结果类型供上层做用户提示。
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        let result = self.manager.ensure_shims_in_path()?;
        map_ensure_shims_result(result, || self.manager.shims_dir())
    }
}

pub(crate) async fn install_python_for_surface(
    version: &str,
    surface: PythonCommandSurface,
) -> Result<()> {
    let service = PythonService::new()?;
    let installed_version = service
        .install(version)
        .await
        .with_context(|| build_install_failure_message(surface, version))?;

    println!("Python {} 已准备就绪。", installed_version);
    println!("下一步你可以执行：");
    println!(
        "  meetai runtime use python {}   # 切换到该版本",
        installed_version
    );
    match surface {
        PythonCommandSurface::Python => {
            println!("  meetai python list      # 查看所有已安装版本");
        }
        PythonCommandSurface::Runtime => {
            println!("  meetai runtime list python      # 查看所有已安装版本");
        }
    }

    Ok(())
}

pub(crate) fn use_python_for_surface(version: &str, surface: PythonCommandSurface) -> Result<()> {
    let service = PythonService::new()?;
    service
        .set_current_version(version)
        .with_context(|| build_use_failure_message(surface, version))?;
    println!("✅ 已切换到 Python {}", version);
    handle_python_use_path_setup(&service, version)?;
    match surface {
        PythonCommandSurface::Python => {
            println!("下一步你可以执行：");
            println!("  meetai python list   # 查看所有已安装版本");
        }
        PythonCommandSurface::Runtime => {
            println!("  meetai runtime list python   # 查看所有已安装版本");
        }
    }
    Ok(())
}

pub(crate) async fn uninstall_python_for_surface(
    version: &str,
    surface: PythonCommandSurface,
) -> Result<()> {
    let service = PythonService::new()?;
    service
        .uninstall(version)
        .await
        .with_context(|| build_uninstall_failure_message(surface, version))?;
    println!("✅ Python {} 已卸载", version);
    println!("下一步你可以执行：");
    match surface {
        PythonCommandSurface::Python => {
            println!("  meetai python list                 # 查看剩余版本");
            if cfg!(windows) {
                println!("  meetai python install latest       # 安装最新版本");
            } else {
                println!("  meetai runtime list python         # 查看 MeetAI 已管理版本");
            }
        }
        PythonCommandSurface::Runtime => {
            println!("  meetai runtime list python              # 查看剩余版本");
            if cfg!(windows) {
                println!("  meetai runtime install python latest    # 安装最新版本");
            } else {
                println!("  meetai runtime use python <version>     # 切换到已管理版本");
            }
        }
    }
    Ok(())
}

fn build_install_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => {
            if cfg!(windows) {
                format!(
                    "Python 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai python install latest",
                    version
                )
            } else {
                format!(
                    "Python 安装失败（请求版本: {}）。\n当前平台暂不支持自动安装。\n下一步你可以执行：\n  meetai python list\n  meetai runtime list python\n  meetai runtime use python <version>",
                    version
                )
            }
        }
        PythonCommandSurface::Runtime => {
            if cfg!(windows) {
                format!(
                    "Python 安装失败（请求版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime install python latest",
                    version
                )
            } else {
                format!(
                    "Python 安装失败（请求版本: {}）。\n当前平台暂不支持自动安装。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime use python <version>",
                    version
                )
            }
        }
    }
}

fn build_use_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => format!(
            "切换 Python 版本失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime list python",
            version
        ),
        PythonCommandSurface::Runtime => format!(
            "Python 版本切换失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai python list",
            version
        ),
    }
}

fn build_uninstall_failure_message(surface: PythonCommandSurface, version: &str) -> String {
    match surface {
        PythonCommandSurface::Python => format!(
            "卸载 Python 失败（目标版本: {}）。\n下一步你可以执行：\n  meetai python list\n  meetai runtime uninstall python {}",
            version, version
        ),
        PythonCommandSurface::Runtime => format!(
            "Python 卸载失败（目标版本: {}）。\n下一步你可以执行：\n  meetai runtime list python\n  meetai runtime uninstall python {}",
            version, version
        ),
    }
}

/// 处理 `python use` 的后续 PATH 引导流程，尽可能自动完成配置并输出下一步提示。
pub fn handle_python_use_path_setup(service: &PythonService, version: &str) -> Result<()> {
    match service.detect_use_path_status(version)? {
        PythonUsePathStatus::ShimsInPath => {
            // 当前终端已能感知 shims，直接可用
            println!("  运行 python --version 即可确认。");
        }
        PythonUsePathStatus::CommandReady => {
            println!("  当前终端已可直接使用目标版本，运行 python --version 确认。");
            println!("  如果后续在其他终端未生效，请重启终端后再试。");
        }
        PythonUsePathStatus::NeedsPathConfiguration => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(moon_spinner_style());
            pb.enable_steady_tick(Duration::from_millis(120));
            pb.set_message("正在配置 PATH...");

            let result = service.ensure_shims_in_path()?;
            pb.finish_and_clear();

            match result {
                EnsureShimsResult::JustConfigured => {
                    // 首次配置：写入了永久 PATH，需重启终端一次
                    println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                    println!("  重启终端后运行 python --version 即可（仅需配置一次）。");
                }
                EnsureShimsResult::AlreadyConfigured => {
                    // 永久 PATH 已有 shims，但本次终端窗口未刷新（在配置前打开的终端）
                    println!("  重启终端后运行 python --version 即可生效。");
                }
                EnsureShimsResult::Failed { reason, shims_dir } => {
                    // 自动配置失败，回退到手动引导
                    println!("自动配置 PATH 失败（{}）。", reason);
                    print_python_path_guidance(false, &shims_dir);
                }
            }
        }
    }

    Ok(())
}

fn classify_use_path_status(shims_in_path: bool, command_ready: bool) -> PythonUsePathStatus {
    if shims_in_path {
        PythonUsePathStatus::ShimsInPath
    } else if command_ready {
        PythonUsePathStatus::CommandReady
    } else {
        PythonUsePathStatus::NeedsPathConfiguration
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
            PythonUsePathStatus::ShimsInPath
        );
        assert_eq!(
            classify_use_path_status(true, true),
            PythonUsePathStatus::ShimsInPath
        );
    }

    #[test]
    fn classify_use_path_status_marks_command_ready_without_shims() {
        assert_eq!(
            classify_use_path_status(false, true),
            PythonUsePathStatus::CommandReady
        );
    }

    #[test]
    fn classify_use_path_status_requires_path_configuration_when_needed() {
        assert_eq!(
            classify_use_path_status(false, false),
            PythonUsePathStatus::NeedsPathConfiguration
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

    #[test]
    fn install_failure_message_uses_surface_specific_guidance() {
        let python_msg = build_install_failure_message(PythonCommandSurface::Python, "3.13.2");
        assert!(python_msg.contains("meetai python list"));
        if cfg!(windows) {
            assert!(python_msg.contains("meetai python install latest"));
            assert!(!python_msg.contains("meetai runtime install python latest"));
        } else {
            assert!(python_msg.contains("当前平台暂不支持自动安装"));
            assert!(python_msg.contains("meetai runtime use python <version>"));
        }

        let runtime_msg = build_install_failure_message(PythonCommandSurface::Runtime, "3.13.2");
        assert!(runtime_msg.contains("meetai runtime list python"));
        if cfg!(windows) {
            assert!(runtime_msg.contains("meetai runtime install python latest"));
        } else {
            assert!(runtime_msg.contains("meetai runtime use python <version>"));
        }
    }

    #[test]
    fn use_failure_message_uses_surface_specific_guidance() {
        let python_msg = build_use_failure_message(PythonCommandSurface::Python, "3.13.2");
        assert!(python_msg.contains("切换 Python 版本失败"));
        assert!(python_msg.contains("meetai python list"));
        assert!(python_msg.contains("meetai runtime list python"));

        let runtime_msg = build_use_failure_message(PythonCommandSurface::Runtime, "3.13.2");
        assert!(runtime_msg.contains("Python 版本切换失败"));
        assert!(runtime_msg.contains("meetai runtime list python"));
        assert!(runtime_msg.contains("meetai python list"));
    }

    #[test]
    fn uninstall_failure_message_uses_surface_specific_guidance() {
        let python_msg = build_uninstall_failure_message(PythonCommandSurface::Python, "3.13.2");
        assert!(python_msg.contains("meetai python list"));
        assert!(python_msg.contains("meetai runtime uninstall python 3.13.2"));

        let runtime_msg = build_uninstall_failure_message(PythonCommandSurface::Runtime, "3.13.2");
        assert!(runtime_msg.contains("meetai runtime list python"));
        assert!(runtime_msg.contains("meetai runtime uninstall python 3.13.2"));
    }
}
