//! Python 安装器实现。
//!
//! 本模块提供 Python 的自动下载、安装、验证和卸载功能。
//! 目前主要支持 Windows 平台的自动安装（通过 python.org 安装包），
//! macOS/Linux 平台仅支持管理已手动安装的版本。
//!
//! # 核心功能
//!
//! - **版本解析**: 从 python.org 获取可用版本列表，支持 `latest` 特殊标识
//! - **下载管理**: 从官方源或镜像站下载安装包，支持断点续传和进度显示
//! - **安装执行**: Windows 上静默运行安装程序，配置安装目录
//! - **安装验证**: 检查安装目录、可执行文件、版本输出
//! - **残留清理**: 自动清理不完整的安装残留
//! - **已有安装导入**: 检测系统已安装的 Python 并导入到 MeetAI 管理
//!
//! # 安装流程（Windows）
//!
//! ```text
//! 1. 解析版本（latest → 具体版本号）
//! 2. 检查是否已安装（避免重复）
//! 3. 检测系统已有安装（尝试导入）
//! 4. 下载安装包（.exe）到缓存目录
//! 5. 执行静默安装（/VERYSILENT /SUPPRESSMSGBOXES）
//! 6. 验证安装结果（检查 python.exe、版本输出）
//! 7. 失败时尝试恢复或清理
//! ```
//!
//! # 目录结构
//!
//! ```text
//! {cache_dir}/
//!   python-3.11.0.exe    # 下载的安装包
//!
//! {python_install_dir}/
//!   python-3.11.0/       # 安装后的版本目录
//!     python.exe
//!     Scripts/
//!     ...
//! ```
//!
//! # 平台差异
//!
//! - **Windows**: 完整自动安装流程（下载 → 安装 → 验证）
//! - **macOS/Linux**: 仅验证版本格式，实际安装需用户手动完成
//!
//! # 示例
//!
//! ```rust,ignore
//! use meetai::python::PythonInstaller;
//!
//! let installer = PythonInstaller::new()?;
//! let version = installer.install("3.11.0").await?;
//! println!("已安装 Python {}", version);
//! ```
//!

use crate::config::Config;
use crate::runtime::common::{RuntimeInstaller, RuntimeUninstaller};
use crate::utils::downloader::Downloader;
use crate::utils::executor::CommandExecutor;
use crate::utils::guidance::network_diagnostic_tips;
use crate::utils::progress::{moon_bar_style, moon_spinner_style};
use crate::utils::validator::Validator;
use anyhow::{Context, Result};
use dirs::home_dir;
use indicatif::ProgressBar;
use log::warn;
use regex::Regex;
use reqwest::StatusCode;
use semver::Version;
use std::collections::HashSet;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tokio::process::Command as TokioCommand;

mod adopt;
mod latest;
mod verify;
mod windows_installer;

const PYTHON_DOWNLOADS_URL: &str = "https://www.python.org/downloads/";
const PYTHON_FTP_INDEX_URL: &str = "https://www.python.org/ftp/python/";
const PYTHON_OFFICIAL_FTP_BASE_URL: &str = "https://www.python.org/ftp/python";
const PYTHON_TUNA_MIRROR_BASE_URL: &str = "https://mirrors.tuna.tsinghua.edu.cn/python";
const PYTHON_FALLBACK_VERSION: &str = "3.11.0";
const PYTHON_SIGNER_COMMON_NAME: &str = "Python Software Foundation";
const MAX_ADOPT_FILE_COUNT: u64 = 250_000;
const MAX_ADOPT_TOTAL_BYTES: u64 = 30 * 1024 * 1024 * 1024;
#[cfg(windows)]
const WINDOWS_REPARSE_POINT_ATTRIBUTE: u32 = 0x0400;
const WINDOWS_SYSTEM_ROOT_DEFAULT: &str = r"C:\Windows";
const WINDOWS_PROGRAM_FILES_DEFAULT: &str = r"C:\Program Files";
const WINDOWS_PROGRAM_FILES_X86_DEFAULT: &str = r"C:\Program Files (x86)";

/// Python 安装器
pub struct PythonInstaller {
    config: Config,
    downloader: Downloader,
    executor: CommandExecutor,
}

#[derive(Clone, Debug)]
struct DownloadSource {
    name: &'static str,
    url: String,
}

#[derive(Debug, Default, Clone, Copy)]
struct DirectoryCopyPlan {
    file_count: u64,
    total_bytes: u64,
}

#[derive(Debug, Default, Clone, Copy)]
struct DirectoryCopyStatus {
    copied_files: u64,
    copied_bytes: u64,
}

impl PythonInstaller {
    /// 创建 Python 安装器，并确保安装与缓存目录可用。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            config,
            downloader: Downloader::new()?,
            executor: CommandExecutor::new(),
        })
    }

    /// 安装指定版本的 Python。
    /// `version` 支持 `latest` 或具体版本号；非 Windows 平台不执行自动下载安装，
    /// 当 `latest` 且 MeetAI 管理目录存在已安装版本时，会回退使用其中最高稳定版本。
    pub async fn install(&self, version: &str) -> Result<String> {
        let validator = Validator::new();
        validator.validate_python_install_version(version)?;

        // 提前显示进度，避免在版本解析/本地探测阶段出现“无输出等待”
        let progress = ProgressBar::new(100);
        let install_style =
            moon_bar_style("{spinner} {elapsed_precise} [{bar:40}] {percent:>3}% {msg}");
        progress.set_style(install_style.clone());
        progress.enable_steady_tick(Duration::from_millis(120));
        progress.set_position(1);
        if version == "latest" {
            progress.set_message("🔍 正在解析 latest 对应的 Python 稳定版本...");
        } else {
            progress.set_message(format!("🐍 正在准备安装 Python {}...", version));
        }

        let resolved_version = if version == "latest" && !cfg!(windows) {
            if let Some(local_latest) = self.get_latest_installed_python_version()? {
                progress.println(format!(
                    "当前平台暂不支持自动解析/安装 latest，回退到 MeetAI 已管理版本 {}。",
                    local_latest
                ));
                local_latest
            } else {
                progress.abandon_with_message("❌ 当前平台暂不支持 latest 自动安装");
                anyhow::bail!(
                    "当前平台暂不支持自动下载安装 Python（我们正在努力支持）。\n\n你可以：\n  1. 先手动安装 Python（访问 python.org）\n  2. 然后用 MeetAI 管理：meetai runtime use python <version>\n  3. 或查看已有版本：meetai runtime list python"
                );
            }
        } else {
            self.resolve_target_version(version).await?
        };
        validator.validate_python_selected_version(&resolved_version)?;
        progress.set_position(12);
        progress.set_message(format!(
            "🔍 正在检查本地环境（Python {}）...",
            resolved_version
        ));

        let install_dir = self.get_install_dir(&resolved_version);
        let python_exe = self.get_python_exe(&resolved_version);

        // 清理历史失败残留目录，避免误判和安装冲突
        if install_dir.exists() && !python_exe.exists() {
            warn!(
                "Detected incomplete Python installation directory, cleaning up: {}",
                install_dir.display()
            );
            std::fs::remove_dir_all(&install_dir)
                .with_context(|| format!("清理残留安装目录失败：{}", install_dir.display()))?;
        }

        // 检查是否已安装
        if self.is_installed(&resolved_version)? {
            progress.set_position(100);
            progress.finish_with_message(format!("✅ Python {} 已就绪", resolved_version));
            println!("Python {} 已经安装，无需重复安装。", resolved_version);
            println!("下一步你可以执行：");
            println!(
                "  meetai runtime use python {}   # 切换到该版本",
                resolved_version
            );
            println!("  meetai python list      # 查看所有已安装版本");
            return Ok(resolved_version);
        }

        if !cfg!(windows) {
            progress.abandon_with_message(format!(
                "❌ 当前平台暂不支持自动安装 Python {}",
                resolved_version
            ));
            anyhow::bail!(
                "当前平台暂不支持自动下载安装 Python {}（我们正在努力支持）。\n\n你可以：\n  1. 先手动安装 Python（访问 python.org）\n  2. 然后用 MeetAI 管理：meetai runtime use python <version>\n  3. 或查看已有版本：meetai runtime list python",
                resolved_version,
            );
        }

        progress.set_position(18);
        progress.set_message(format!(
            "🔍 正在检测系统已有 Python {} 安装...",
            resolved_version
        ));
        if cfg!(windows)
            && self
                .try_adopt_existing_installation_with_progress(&resolved_version, Some(&progress))?
        {
            progress.set_position(100);
            progress.finish_with_message(format!("✅ Python {} 导入完成", resolved_version));
            println!(
                "Python {} 已导入到 MeetAI 管理目录，无需重复安装。",
                resolved_version
            );
            println!("下一步你可以执行：");
            println!(
                "  meetai runtime use python {}   # 切换到该版本",
                resolved_version
            );
            println!("  meetai python list      # 查看所有已安装版本");
            return Ok(resolved_version);
        }

        progress.set_position(25);
        progress.set_message("📦 准备下载 Python...");

        // 下载安装包
        let installer_path = self
            .config
            .cache_dir
            .join(format!("python-{}.exe", resolved_version));
        progress.set_message(format!("📦 下载 Python {}...", resolved_version));
        self.download_installer_with_fallback(&resolved_version, &installer_path, &progress)
            .await?;
        progress.set_style(install_style.clone());
        progress.set_length(100);

        progress.set_message("🔧 安装 Python...");
        progress.set_position(60);

        // 安装 Python
        if let Err(err) = self
            .install_python(&installer_path, &resolved_version, Some(&progress))
            .await
        {
            progress.abandon_with_message(format!("❌ Python {} 安装失败", resolved_version));
            self.cleanup_failed_install(&resolved_version, &installer_path);
            return Err(err.context(format!("Python {} 安装失败", resolved_version)));
        }
        progress.set_position(92);

        progress.set_message("🔍 验证安装结果...");
        progress.set_position(95);
        if let Err(verify_err) = self.verify_installation(&resolved_version).await {
            if cfg!(windows) {
                if let Err(recover_err) = self
                    .recover_after_verification_failure(
                        &resolved_version,
                        &installer_path,
                        verify_err,
                    )
                    .await
                {
                    progress
                        .abandon_with_message(format!("❌ Python {} 校验失败", resolved_version));
                    self.cleanup_failed_install(&resolved_version, &installer_path);
                    return Err(recover_err
                        .context(format!("Python {} 安装完成但验证失败", resolved_version)));
                }
            } else {
                progress.abandon_with_message(format!("❌ Python {} 校验失败", resolved_version));
                self.cleanup_failed_install(&resolved_version, &installer_path);
                return Err(
                    verify_err.context(format!("Python {} 安装完成但验证失败", resolved_version))
                );
            }
        }

        progress.set_position(100);
        progress.finish_with_message(format!("✅ Python {} 安装完成", resolved_version));
        println!("下一步你可以执行：");
        println!(
            "  meetai runtime use python {}   # 切换到该版本",
            resolved_version
        );
        println!("  meetai python list      # 查看所有已安装版本");

        Ok(resolved_version)
    }

    /// 卸载指定版本的 Python
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        Validator::new().validate_python_selected_version(version)?;

        if !self.is_installed(version)? {
            anyhow::bail!("Python {} 未安装", version);
        }

        let install_dir = self.get_install_dir(version);

        let pb = ProgressBar::new_spinner();
        pb.set_style(moon_spinner_style());
        pb.enable_steady_tick(Duration::from_millis(120));
        pb.set_message(format!("🗑️ 正在卸载 Python {}...", version));

        // Windows 上使用卸载程序
        if cfg!(windows) {
            let uninstaller = install_dir.join("unins000.exe");
            if uninstaller.exists() {
                pb.set_message(format!("🔧 正在运行 Python {} 卸载程序...", version));
                self.executor
                    .execute(&uninstaller, &["/VERYSILENT", "/SUPPRESSMSGBOXES"])
                    .await?;
            }
        }

        // 删除安装目录
        if install_dir.exists() {
            pb.set_message(format!("🗑️ 正在删除 Python {} 文件...", version));
            std::fs::remove_dir_all(&install_dir)
                .context("删除安装目录失败，请检查目录权限或手动删除")?;
        }

        pb.finish_and_clear();
        Ok(())
    }

    /// 检查指定版本是否已安装
    fn is_installed(&self, version: &str) -> Result<bool> {
        Validator::new().validate_python_selected_version(version)?;

        let install_dir = self.get_install_dir(version);
        if !install_dir.exists() {
            return Ok(false);
        }

        Ok(self.get_python_exe(version).exists())
    }

    /// 获取安装目录
    fn get_install_dir(&self, version: &str) -> PathBuf {
        self.config
            .python_install_dir
            .join(format!("python-{}", version))
    }

    fn target_arch_suffix() -> Result<&'static str> {
        if cfg!(target_arch = "x86_64") {
            Ok("amd64")
        } else if cfg!(target_arch = "aarch64") {
            Ok("arm64")
        } else {
            anyhow::bail!("不支持的系统架构");
        }
    }

    fn get_download_sources(&self, version: &str) -> Result<Vec<DownloadSource>> {
        if !cfg!(windows) {
            anyhow::bail!("当前仅支持 Windows 自动安装");
        }

        let arch = Self::target_arch_suffix()?;
        let mut sources = vec![
            DownloadSource {
                name: "Python 官方源",
                url: format!(
                    "{base}/{version}/python-{version}-{arch}.exe",
                    base = PYTHON_OFFICIAL_FTP_BASE_URL
                ),
            },
            DownloadSource {
                name: "Python 官方源",
                url: format!(
                    "{base}/{version}/python-{version}.exe",
                    base = PYTHON_OFFICIAL_FTP_BASE_URL
                ),
            },
        ];

        sources.extend([
            DownloadSource {
                name: "清华镜像源",
                url: format!(
                    "{base}/{version}/python-{version}-{arch}.exe",
                    base = PYTHON_TUNA_MIRROR_BASE_URL
                ),
            },
            DownloadSource {
                name: "清华镜像源",
                url: format!(
                    "{base}/{version}/python-{version}.exe",
                    base = PYTHON_TUNA_MIRROR_BASE_URL
                ),
            },
        ]);

        Ok(sources)
    }

    fn build_official_download_url(&self, version: &str) -> Result<String> {
        if !cfg!(windows) {
            anyhow::bail!("当前仅支持 Windows 自动安装");
        }

        let arch = Self::target_arch_suffix()?;
        Ok(format!(
            "{base}/{version}/python-{version}-{arch}.exe",
            base = PYTHON_OFFICIAL_FTP_BASE_URL
        ))
    }

    async fn download_installer_with_fallback(
        &self,
        version: &str,
        installer_path: &Path,
        progress: &ProgressBar,
    ) -> Result<()> {
        let sources = self.get_download_sources(version)?;
        self.download_installer_from_sources(version, installer_path, progress, &sources)
            .await
    }

    async fn download_installer_from_sources(
        &self,
        version: &str,
        installer_path: &Path,
        progress: &ProgressBar,
        sources: &[DownloadSource],
    ) -> Result<()> {
        let download_style = moon_bar_style(
            "{spinner} {elapsed_precise} [{bar:40}] {bytes:>10}/{total_bytes} {bytes_per_sec:>12} eta {eta_precise} {msg}",
        );

        let mut source_error_report = Vec::<String>::new();

        for (index, source) in sources.iter().enumerate() {
            progress.set_style(download_style.clone());
            progress.set_length(0);
            progress.set_position(0);
            progress.set_message(format!(
                "📦 下载 Python {}（{}/{}：{}）...",
                version,
                index + 1,
                sources.len(),
                source.name
            ));

            match self
                .downloader
                .download(&source.url, installer_path, Some(progress))
                .await
            {
                Ok(()) => {
                    if index > 0 {
                        progress.println(format!("已从 {} 下载成功。", source.name));
                    }
                    return Ok(());
                }
                Err(err) => {
                    warn!(
                        "Download attempt failed from {} ({}): {:#}",
                        source.name, source.url, err
                    );
                    source_error_report.push(format!(
                        "  {}. {}: {}\n     错误: {}",
                        index + 1,
                        source.name,
                        source.url,
                        err
                    ));

                    if index + 1 < sources.len() {
                        progress.println(format!(
                            "从 {} 下载失败，正在自动切换到下一个下载源...",
                            source.name
                        ));
                    }
                }
            }
        }

        let source_errors = if source_error_report.is_empty() {
            "  - 未采集到具体错误，请重试并开启 --verbose".to_string()
        } else {
            source_error_report.join("\n")
        };

        anyhow::bail!(
            "{}",
            Self::build_download_failure_message(version, &source_errors)
        );
    }

    fn build_download_failure_message(version: &str, source_errors: &str) -> String {
        format!(
            "Python {} 下载失败了 😢\n\n我们尝试了多个下载源，但都没成功：\n{}\n\n别担心，可以试试这些方法：\n  - 换个版本试试：meetai runtime install python <version>\n  - 重试当前版本：meetai python install {}\n  - 安装最新版：meetai runtime install python latest\n\n{}",
            version,
            source_errors,
            version,
            network_diagnostic_tips()
        )
    }
}

#[async_trait::async_trait]
impl RuntimeInstaller for PythonInstaller {
    async fn install_version(&self, version: &str) -> Result<String> {
        PythonInstaller::install(self, version).await
    }
}

#[async_trait::async_trait]
impl RuntimeUninstaller for PythonInstaller {
    async fn uninstall_version(&self, version: &str) -> Result<()> {
        PythonInstaller::uninstall(self, version).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::common::{RuntimeInstaller, RuntimeUninstaller};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn make_installer(root: &std::path::Path) -> Result<PythonInstaller> {
        let config = Config {
            python_install_dir: root.join("python"),
            venv_dir: root.join("venvs"),
            cache_dir: root.join("cache"),
            current_python_version: None,
        };
        config.ensure_dirs()?;

        Ok(PythonInstaller {
            config,
            downloader: Downloader::new()?,
            executor: CommandExecutor::new(),
        })
    }

    #[tokio::test]
    async fn runtime_traits_delegate_to_inherent_impl() -> Result<()> {
        let temp = tempdir()?;
        let installer_impl = Arc::new(make_installer(temp.path())?);
        let installer: Arc<dyn RuntimeInstaller> = installer_impl.clone();
        let uninstaller: Arc<dyn RuntimeUninstaller> = installer_impl;

        let install_err = installer
            .install_version("not-a-version")
            .await
            .expect_err("invalid version should reach inherent install validation");
        assert!(
            !install_err.to_string().is_empty(),
            "install error should come from inherent implementation"
        );

        let uninstall_err = uninstaller
            .uninstall_version("not-a-version")
            .await
            .expect_err("invalid version should reach inherent uninstall validation");
        assert!(
            !uninstall_err.to_string().is_empty(),
            "uninstall error should come from inherent implementation"
        );

        Ok(())
    }

    async fn spawn_fallback_test_server(
        mirror_body: Vec<u8>,
    ) -> Result<(String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let requests_for_task = Arc::clone(&requests);

        let handle = tokio::spawn(async move {
            for _ in 0..2 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut request_buf = [0u8; 4096];
                let size = socket.read(&mut request_buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&request_buf[..size]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();

                requests_for_task
                    .lock()
                    .expect("lock request history")
                    .push(path.clone());

                if path.contains("official.exe") {
                    let response =
                        b"HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                    let _ = socket.write_all(response).await;
                } else if path.contains("mirror.exe") {
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                        mirror_body.len()
                    );
                    let _ = socket.write_all(headers.as_bytes()).await;
                    let _ = socket.write_all(&mirror_body).await;
                } else {
                    let response = b"HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                    let _ = socket.write_all(response).await;
                }

                let _ = socket.flush().await;
            }
        });

        Ok((format!("http://{}", addr), requests, handle))
    }

    #[test]
    fn is_installed_requires_python_executable() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;
        let version = "3.12.0";

        let install_dir = installer.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;
        assert!(
            !installer.is_installed(version)?,
            "install dir alone should not be treated as installed"
        );

        let python_exe = installer.get_python_exe(version);
        std::fs::write(&python_exe, b"stub")?;
        assert!(
            installer.is_installed(version)?,
            "existing executable should be treated as installed"
        );

        Ok(())
    }

    #[test]
    fn latest_installed_version_ignores_incomplete_install_dirs() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;

        let incomplete_version = "3.13.2";
        let healthy_version = "3.12.9";

        // Higher version exists only as a leftover directory without executable.
        std::fs::create_dir_all(installer.get_install_dir(incomplete_version))?;

        // Lower version has a valid executable and should be selected.
        let healthy_exe = installer.get_python_exe(healthy_version);
        if let Some(parent) = healthy_exe.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&healthy_exe, b"stub")?;

        let latest = installer.get_latest_installed_python_version()?;
        assert_eq!(
            latest.as_deref(),
            Some(healthy_version),
            "latest fallback should ignore incomplete install directories"
        );

        Ok(())
    }

    #[tokio::test]
    async fn verify_installation_fails_when_executable_missing() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;
        let version = "3.12.1";

        let install_dir = installer.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;

        let err = installer
            .verify_installation(version)
            .await
            .expect_err("verification should fail when executable is missing");

        assert!(
            err.to_string()
                .contains("Python executable not found after installation"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn cleanup_failed_install_removes_install_dir_and_installer_file() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;
        let version = "3.12.2";

        let install_dir = installer.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;
        std::fs::write(install_dir.join("leftover.txt"), b"partial")?;

        let installer_path = installer
            .config
            .cache_dir
            .join(format!("python-{}.exe", version));
        std::fs::write(&installer_path, b"installer-bytes")?;

        installer.cleanup_failed_install(version, &installer_path);

        assert!(
            !install_dir.exists(),
            "install directory should be removed by cleanup"
        );
        assert!(
            !installer_path.exists(),
            "installer file should be removed by cleanup"
        );

        Ok(())
    }

    #[test]
    fn copy_directory_contents_preserves_content_without_extra_root_layer() -> Result<()> {
        let temp = tempdir()?;
        let source = temp.path().join("source-python");
        let target = temp.path().join("target-python");

        std::fs::create_dir_all(source.join("Lib"))?;
        std::fs::write(source.join("python.exe"), b"binary")?;
        std::fs::write(source.join("Lib").join("site.py"), b"print('ok')")?;

        PythonInstaller::copy_directory_contents(&source, &target)?;

        assert!(target.join("python.exe").exists());
        assert!(target.join("Lib").join("site.py").exists());
        assert!(
            !target.join("source-python").join("python.exe").exists(),
            "copy should not introduce an extra root directory layer"
        );

        Ok(())
    }

    #[test]
    fn parse_latest_from_downloads_body_supports_download_button_copy() -> Result<()> {
        let body = r#"
            <section>
                <a href="/downloads/release/python-3143/">Download Python 3.14.3</a>
            </section>
            <ul>
                <li>Python 3.14.2</li>
                <li>Python 3.13.7</li>
            </ul>
        "#;

        let version = PythonInstaller::parse_latest_from_downloads_body(body)?;
        assert_eq!(version, "3.14.3");
        Ok(())
    }

    #[test]
    fn parse_latest_from_downloads_body_ignores_prerelease_suffix() -> Result<()> {
        let body = r#"
            <section>
                <a href="/downloads/release/python-3143/">Latest Python 3 Release - Python 3.14.3</a>
                <a href="/downloads/release/python-3150a6/">Download Python 3.15.0a6</a>
            </section>
        "#;

        let version = PythonInstaller::parse_latest_from_downloads_body(body)?;
        assert_eq!(version, "3.14.3");
        Ok(())
    }

    #[test]
    fn python_output_matches_requested_version_is_strict() -> Result<()> {
        assert!(PythonInstaller::python_output_matches_requested_version(
            "Python 3.12.9",
            "3.12.9"
        )?);
        assert!(PythonInstaller::python_output_matches_requested_version(
            "Python 3.12.9\r\n",
            "3.12.9"
        )?);
        assert!(!PythonInstaller::python_output_matches_requested_version(
            "Python 3.12.90",
            "3.12.9"
        )?);
        assert!(!PythonInstaller::python_output_matches_requested_version(
            "CPython 3.12.9",
            "3.12.9"
        )?);
        Ok(())
    }

    #[test]
    fn trusted_python_signer_subject_requires_exact_cn() {
        assert!(PythonInstaller::is_trusted_python_signer_subject(
            "CN=Python Software Foundation, O=Python Software Foundation, C=US"
        ));
        assert!(!PythonInstaller::is_trusted_python_signer_subject(
            "CN=Python Software Foundation DEV, O=Python Software Foundation, C=US"
        ));
        assert!(!PythonInstaller::is_trusted_python_signer_subject(
            "O=Python Software Foundation, C=US"
        ));
    }

    #[test]
    fn is_path_within_root_respects_segment_boundary() {
        if cfg!(windows) {
            assert!(PythonInstaller::is_path_within_root(
                Path::new(r"C:\Program Files\Python312"),
                Path::new(r"C:\Program Files")
            ));
            assert!(!PythonInstaller::is_path_within_root(
                Path::new(r"C:\Program FilesX\Python312"),
                Path::new(r"C:\Program Files")
            ));
        } else {
            assert!(PythonInstaller::is_path_within_root(
                Path::new("/usr/local/python"),
                Path::new("/usr/local")
            ));
            assert!(!PythonInstaller::is_path_within_root(
                Path::new("/usr/localx/python"),
                Path::new("/usr/local")
            ));
        }
    }

    #[test]
    fn resolve_windows_system_root_prefers_env_and_has_default_fallback() {
        assert_eq!(
            PythonInstaller::resolve_windows_system_root(
                Some(PathBuf::from(r"D:\Windows")),
                Some(PathBuf::from(r"E:\Win"))
            ),
            PathBuf::from(r"D:\Windows")
        );
        assert_eq!(
            PythonInstaller::resolve_windows_system_root(None, Some(PathBuf::from(r"E:\Win"))),
            PathBuf::from(r"E:\Win")
        );
        assert_eq!(
            PythonInstaller::resolve_windows_system_root(None, None),
            PathBuf::from(WINDOWS_SYSTEM_ROOT_DEFAULT)
        );
    }

    #[test]
    fn build_trusted_python_install_roots_prefers_env_paths_and_deduplicates() {
        let roots = PythonInstaller::build_trusted_python_install_roots(
            Some(PathBuf::from(r"D:\Users\Alice\AppData\Local")),
            Some(PathBuf::from(r"C:\Users\Alice")),
            Some(PathBuf::from(r"D:\Program Files")),
            Some(PathBuf::from(r"D:\Program Files")),
        );

        assert!(
            roots.contains(&PathBuf::from(
                r"D:\Users\Alice\AppData\Local\Programs\Python"
            )),
            "LOCALAPPDATA-derived Python root should be trusted"
        );

        let program_files_count = roots
            .iter()
            .filter(|p| {
                p.to_string_lossy()
                    .eq_ignore_ascii_case(r"D:\Program Files")
            })
            .count();
        assert_eq!(
            program_files_count, 1,
            "duplicate ProgramFiles roots should be deduplicated"
        );
    }

    #[test]
    fn build_trusted_python_install_roots_uses_defaults_when_env_missing() {
        let roots = PythonInstaller::build_trusted_python_install_roots(None, None, None, None);
        assert!(roots.contains(&PathBuf::from(WINDOWS_PROGRAM_FILES_DEFAULT)));
        assert!(roots.contains(&PathBuf::from(WINDOWS_PROGRAM_FILES_X86_DEFAULT)));
    }

    #[test]
    fn parse_latest_from_ftp_index_body_selects_highest_stable() -> Result<()> {
        let body = r#"
            <a href="3.12.11/">3.12.11/</a>
            <a href="3.13.0/">3.13.0/</a>
            <a href="3.13.2/">3.13.2/</a>
            <a href="3.14.0rc1/">3.14.0rc1/</a>
        "#;

        let version = PythonInstaller::parse_latest_from_ftp_index_body(body)?;
        assert_eq!(version, "3.13.2");
        Ok(())
    }

    #[test]
    fn choose_latest_python_version_follows_priority() {
        let chosen = PythonInstaller::choose_latest_python_version(
            Some("3.14.0".to_string()),
            Some("3.13.5".to_string()),
            Some("3.12.9".to_string()),
        );
        assert_eq!(chosen, "3.14.0");

        let chosen = PythonInstaller::choose_latest_python_version(
            None,
            Some("3.13.5".to_string()),
            Some("3.12.9".to_string()),
        );
        assert_eq!(chosen, "3.13.5");

        let chosen = PythonInstaller::choose_latest_python_version(None, None, None);
        assert_eq!(chosen, "3.11.0");
    }

    #[test]
    fn choose_latest_python_version_prefers_highest_semver_even_if_local() {
        let chosen = PythonInstaller::choose_latest_python_version(
            None,
            Some("3.14.2".to_string()),
            Some("3.14.3".to_string()),
        );
        assert_eq!(chosen, "3.14.3");
    }

    #[test]
    fn get_download_sources_uses_official_then_tuna() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;

        if cfg!(windows) {
            let sources = installer.get_download_sources("3.14.3")?;
            assert_eq!(sources.len(), 4);
            assert!(
                sources[0]
                    .url
                    .contains("https://www.python.org/ftp/python/3.14.3/"),
                "first source should be official ftp, got: {}",
                sources[0].url
            );
            assert!(
                sources[1]
                    .url
                    .contains("https://www.python.org/ftp/python/3.14.3/"),
                "second source should still be official ftp generic installer, got: {}",
                sources[1].url
            );
            assert!(
                sources[2]
                    .url
                    .contains("https://mirrors.tuna.tsinghua.edu.cn/python/3.14.3/"),
                "third source should be tuna mirror arch installer, got: {}",
                sources[2].url
            );
            assert!(
                sources[3]
                    .url
                    .contains("https://mirrors.tuna.tsinghua.edu.cn/python/3.14.3/"),
                "fourth source should be tuna mirror generic installer, got: {}",
                sources[3].url
            );
        } else {
            let err = installer
                .get_download_sources("3.14.3")
                .expect_err("non-windows should return unsupported error");
            assert!(
                err.to_string().contains("当前仅支持 Windows 自动安装"),
                "unexpected error: {err:#}"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn download_installer_from_sources_falls_back_to_mirror() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;
        let installer_path = temp.path().join("python-3.14.3.exe");
        let mirror_body = b"mirror-installer-bytes".to_vec();
        let (base_url, request_history, server_handle) =
            spawn_fallback_test_server(mirror_body.clone()).await?;
        let progress = ProgressBar::hidden();

        let sources = vec![
            DownloadSource {
                name: "Python 官方源",
                url: format!("{}/official.exe", base_url),
            },
            DownloadSource {
                name: "清华镜像源",
                url: format!("{}/mirror.exe", base_url),
            },
        ];

        installer
            .download_installer_from_sources("3.14.3", &installer_path, &progress, &sources)
            .await?;
        server_handle
            .await
            .expect("fallback server task should finish without panic");

        assert_eq!(std::fs::read(&installer_path)?, mirror_body);
        let requests = request_history
            .lock()
            .expect("lock request history")
            .clone();
        assert_eq!(
            requests,
            vec!["/official.exe".to_string(), "/mirror.exe".to_string()],
            "download order should be official first then mirror"
        );

        Ok(())
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn install_latest_without_local_version_fails_fast_on_non_windows() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;

        let err = installer
            .install("latest")
            .await
            .expect_err("non-windows latest install should fail without local versions");
        assert!(
            err.to_string()
                .contains("当前平台暂不支持自动下载安装 Python"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn build_download_failure_message_contains_network_diagnostics() {
        let message = PythonInstaller::build_download_failure_message(
            "3.14.3",
            "  1. Python 官方源: https://example.com/official.exe\n     错误: Download failed for URL https://example.com/official.exe with status: 404 Not Found",
        );

        assert!(
            message.contains(network_diagnostic_tips()),
            "error message should include shared network guidance, got: {message}"
        );
        assert!(
            message.contains("重试当前版本：meetai python install 3.14.3"),
            "error message should include version-specific retry command, got: {message}"
        );
    }
}
