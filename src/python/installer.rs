use crate::config::Config;
use crate::utils::downloader::Downloader;
use crate::utils::executor::CommandExecutor;
use crate::utils::guidance::network_diagnostic_tips;
use crate::utils::progress::{moon_bar_style, moon_spinner_style};
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
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            config,
            downloader: Downloader::new(),
            executor: CommandExecutor::new(),
        })
    }

    /// 安装指定版本的 Python
    pub async fn install(&self, version: &str) -> Result<String> {
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

        let resolved_version = self.resolve_target_version(version).await?;
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
            std::fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "清理残留安装目录失败：{}",
                    install_dir.display()
                )
            })?;
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
        if let Err(verify_err) = self.verify_installation(&resolved_version) {
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
                    return Err(recover_err.context(format!(
                        "Python {} 安装完成但验证失败",
                        resolved_version
                    )));
                }
            } else {
                progress.abandon_with_message(format!("❌ Python {} 校验失败", resolved_version));
                self.cleanup_failed_install(&resolved_version, &installer_path);
                return Err(verify_err.context(format!(
                    "Python {} 安装完成但验证失败",
                    resolved_version
                )));
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

        if !Self::is_prerelease_version(version) {
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
        }

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
        installer_path: &PathBuf,
        progress: &ProgressBar,
    ) -> Result<()> {
        let sources = self.get_download_sources(version)?;
        self.download_installer_from_sources(version, installer_path, progress, &sources)
            .await
    }

    async fn download_installer_from_sources(
        &self,
        version: &str,
        installer_path: &PathBuf,
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
            "Python {} 安装包下载失败。\n各下载源反馈：\n{}\n可尝试：\n  - meetai runtime install python <version>\n  - meetai python install <version>\n  - meetai runtime install python latest\n{}",
            version,
            source_errors,
            network_diagnostic_tips()
        )
    }

    fn is_prerelease_version(version: &str) -> bool {
        version
            .chars()
            .any(|ch| !(ch.is_ascii_digit() || ch == '.'))
    }

    async fn resolve_target_version(&self, requested_version: &str) -> Result<String> {
        if requested_version != "latest" {
            return Ok(requested_version.to_string());
        }

        println!("正在解析 latest 对应的 Python 稳定版本...");
        let resolved = self.resolve_latest_python_version().await?;
        println!("已解析 latest -> Python {}", resolved);
        Ok(resolved)
    }

    async fn resolve_latest_python_version(&self) -> Result<String> {
        let downloads_version = match self.fetch_latest_from_downloads_page().await {
            Ok(version) => Some(version),
            Err(err) => {
                warn!("Failed to fetch latest Python version from downloads page: {err:#}");
                None
            }
        };

        let downloads_version = if let Some(version) = downloads_version {
            if self.is_official_installer_available(&version).await {
                Some(version)
            } else {
                warn!(
                    "Downloads page candidate '{}' is not downloadable as official Windows installer, skip it",
                    version
                );
                None
            }
        } else {
            None
        };

        let ftp_version = if downloads_version.is_none() {
            match self.fetch_latest_downloadable_from_ftp_index().await {
                Ok(version) => version,
                Err(err) => {
                    warn!("Failed to fetch latest downloadable Python version from FTP index: {err:#}");
                    None
                }
            }
        } else {
            None
        };

        let local_version = if downloads_version.is_none() && ftp_version.is_none() {
            self.get_latest_installed_python_version()?
        } else {
            None
        };

        if downloads_version.is_none() && ftp_version.is_none() {
            if let Some(ref version) = local_version {
                println!("无法在线获取最新版本，回退到本地已安装版本: {}", version);
            } else {
                println!(
                    "无法在线获取最新版本，回退到内置默认版本: {}",
                    PYTHON_FALLBACK_VERSION
                );
            }
        }

        Ok(Self::choose_latest_python_version(
            downloads_version,
            ftp_version,
            local_version,
        ))
    }

    fn choose_latest_python_version(
        downloads_version: Option<String>,
        ftp_version: Option<String>,
        local_version: Option<String>,
    ) -> String {
        let mut candidates: Vec<(Version, String)> = Vec::new();
        for raw in [downloads_version, ftp_version, local_version]
            .into_iter()
            .flatten()
        {
            if let Ok(parsed) = Version::parse(&raw) {
                candidates.push((parsed, raw));
            }
        }

        candidates
            .into_iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, raw)| raw)
            .unwrap_or_else(|| PYTHON_FALLBACK_VERSION.to_string())
    }

    fn parse_latest_from_downloads_body(body: &str) -> Result<String> {
        let mut versions: Vec<Version> = Vec::new();
        let patterns = [
            r"Latest Python 3 Release\s*-\s*Python\s*(\d+\.\d+\.\d+)(?:[^0-9A-Za-z]|$)",
            r"Download Python\s*(\d+\.\d+\.\d+)(?:[^0-9A-Za-z]|$)",
            r"Python\s+(\d+\.\d+\.\d+)(?:[^0-9A-Za-z]|$)",
        ];

        for pattern in patterns {
            let re = Regex::new(pattern).with_context(|| {
                format!("Failed to compile downloads page regex pattern: {pattern}")
            })?;

            for captures in re.captures_iter(body) {
                let Some(version_match) = captures.get(1) else {
                    continue;
                };

                let Ok(version) = Version::parse(version_match.as_str()) else {
                    continue;
                };

                if version.pre.is_empty() {
                    versions.push(version);
                }
            }
        }

        let latest = versions
            .into_iter()
            .max()
            .context("Failed to parse latest stable Python version from downloads page")?;

        Ok(latest.to_string())
    }

    #[cfg(test)]
    fn parse_latest_from_ftp_index_body(body: &str) -> Result<String> {
        let versions = Self::parse_stable_versions_from_ftp_index_body(body)?;
        versions
            .into_iter()
            .next()
            .map(|v| v.to_string())
            .context("No stable Python versions found in FTP index")
    }

    fn parse_stable_versions_from_ftp_index_body(body: &str) -> Result<Vec<Version>> {
        let re = Regex::new(r#"href="(\d+\.\d+\.\d+)/""#)
            .context("Failed to compile FTP index regex")?;
        let mut versions: Vec<Version> = Vec::new();
        for capture in re.captures_iter(body) {
            let Some(version_match) = capture.get(1) else {
                continue;
            };

            let Ok(version) = Version::parse(version_match.as_str()) else {
                continue;
            };

            if !version.pre.is_empty() {
                continue;
            }
            versions.push(version);
        }

        versions.sort_by(|a, b| b.cmp(a));
        versions.dedup();

        if versions.is_empty() {
            anyhow::bail!("No stable Python versions found in FTP index");
        }

        Ok(versions)
    }

    async fn fetch_latest_from_downloads_page(&self) -> Result<String> {
        let body = reqwest::get(PYTHON_DOWNLOADS_URL)
            .await
            .context("Failed to request Python downloads page")?
            .error_for_status()
            .context("Python downloads page returned non-success status")?
            .text()
            .await
            .context("Failed to read Python downloads page body")?;

        Self::parse_latest_from_downloads_body(&body)
    }

    async fn fetch_latest_downloadable_from_ftp_index(&self) -> Result<Option<String>> {
        let body = reqwest::get(PYTHON_FTP_INDEX_URL)
            .await
            .context("Failed to request Python FTP index")?
            .error_for_status()
            .context("Python FTP index returned non-success status")?
            .text()
            .await
            .context("Failed to read Python FTP index body")?;

        let candidates = Self::parse_stable_versions_from_ftp_index_body(&body)?;
        for candidate in candidates {
            let raw = candidate.to_string();
            if self.is_official_installer_available(&raw).await {
                return Ok(Some(raw));
            }
        }

        Ok(None)
    }

    async fn is_official_installer_available(&self, version: &str) -> bool {
        let Ok(url) = self.build_official_download_url(version) else {
            return false;
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build();
        let Ok(client) = client else {
            return false;
        };

        let head_resp = client.head(&url).send().await;
        match head_resp {
            Ok(resp) if resp.status().is_success() => true,
            Ok(resp) if resp.status() == StatusCode::METHOD_NOT_ALLOWED => {
                let get_resp = client.get(&url).send().await;
                matches!(get_resp, Ok(r) if r.status().is_success())
            }
            Ok(_) => false,
            Err(_) => false,
        }
    }

    fn get_latest_installed_python_version(&self) -> Result<Option<String>> {
        if !self.config.python_install_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<Version> = None;

        for entry in std::fs::read_dir(&self.config.python_install_dir)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }

            let Some(raw_name) = entry.file_name().to_str().map(|s| s.to_string()) else {
                continue;
            };
            let Some(version_str) = raw_name.strip_prefix("python-") else {
                continue;
            };
            let Ok(version) = Version::parse(version_str) else {
                continue;
            };
            if !version.pre.is_empty() {
                continue;
            }

            let replace = match &latest {
                Some(current) => version.cmp(current).is_gt(),
                None => true,
            };
            if replace {
                latest = Some(version);
            }
        }

        Ok(latest.map(|v| v.to_string()))
    }

    /// 安装 Python
    async fn install_python(
        &self,
        installer_path: &PathBuf,
        version: &str,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        let install_dir = self.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;

        if cfg!(windows) {
            Self::verify_windows_installer_signature(installer_path).with_context(|| {
                format!(
                    "Installer signature verification failed: {}",
                    installer_path.display()
                )
            })?;
            if let Some(pb) = progress {
                pb.set_message(format!(
                    "🔐 已验证安装包签名，开始安装 Python {}...",
                    version
                ));
            }

            let target_arg = format!("TargetDir={}", install_dir.display());
            let install_args = vec![
                "/quiet".to_string(),
                "InstallAllUsers=0".to_string(),
                "PrependPath=1".to_string(),
                "Include_test=0".to_string(),
                target_arg,
            ];
            let install_args_ref = install_args
                .iter()
                .map(std::string::String::as_str)
                .collect::<Vec<_>>();
            let phase = format!("🔧 正在安装 Python {}", version);

            self.run_windows_installer(
                installer_path,
                &install_args_ref,
                &[0, 1638, 3010],
                progress,
                &phase,
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to run installer executable: {}",
                    installer_path.display()
                )
            })?;
        } else {
            anyhow::bail!("Automatic installation not yet supported on this platform");
        }

        Ok(())
    }

    /// 获取安装目录下的 Python 可执行文件路径
    fn get_python_exe(&self, version: &str) -> PathBuf {
        let install_dir = self.get_install_dir(version);
        if cfg!(windows) {
            install_dir.join("python.exe")
        } else {
            install_dir.join("bin/python")
        }
    }

    /// 验证安装结果
    fn verify_installation(&self, version: &str) -> Result<()> {
        let python_exe = self.get_python_exe(version);
        if !python_exe.exists() {
            anyhow::bail!(
                "Python executable not found after installation: {}",
                python_exe.display()
            );
        }

        let output = self
            .executor
            .execute_with_output(&python_exe, &["--version"])
            .with_context(|| {
                format!(
                    "Failed to verify Python installation with command: {} --version",
                    python_exe.display()
                )
            })?;

        if !output.to_lowercase().contains("python") {
            anyhow::bail!(
                "Unexpected output from Python executable '{}': {}",
                python_exe.display(),
                output.trim()
            );
        }

        Ok(())
    }

    async fn recover_after_verification_failure(
        &self,
        version: &str,
        installer_path: &PathBuf,
        verify_err: anyhow::Error,
    ) -> Result<()> {
        let mut diagnostics = vec![format!("首次安装校验失败: {verify_err}")];

        if self.try_adopt_existing_installation(version)? {
            match self.verify_installation(version) {
                Ok(()) => {
                    println!("检测到系统已有 Python 安装，已导入 MeetAI 管理目录并完成校验。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("导入系统安装后校验失败: {err}")),
            }
        } else {
            diagnostics.push("未找到可导入的系统 Python 安装路径。".to_string());
        }

        println!("检测到安装器可能进入 Modify 模式，正在尝试自动修复（卸载冲突项后重装）...");
        match self
            .force_reinstall_into_managed_dir(installer_path, version)
            .await
        {
            Ok(()) => match self.verify_installation(version) {
                Ok(()) => {
                    println!("自动修复成功，Python 已按 MeetAI 目录规则安装。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("自动修复后校验仍失败: {err}")),
            },
            Err(err) => diagnostics.push(format!("自动修复执行失败: {err:#}")),
        }

        if self.try_adopt_existing_installation(version)? {
            match self.verify_installation(version) {
                Ok(()) => {
                    println!("重装后已成功导入系统 Python 并通过校验。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("重装后导入系统 Python 仍校验失败: {err}")),
            }
        }

        let version_hint = Version::parse(version)
            .map(|v| format!("{}.{}", v.major, v.minor))
            .unwrap_or_else(|_| version.to_string());
        let diagnostics = diagnostics
            .into_iter()
            .map(|line| format!("  - {line}"))
            .collect::<Vec<_>>()
            .join("\n");

        anyhow::bail!(
            "Python {version} 安装后自动修复失败。\n诊断信息：\n{diagnostics}\n可尝试：\n  1. 在“应用和功能”中卸载 Python {version_hint}.x 后重试。\n  2. 重新执行: meetai runtime install python {version}\n  3. 查看可用版本: meetai runtime list python"
        );
    }

    async fn force_reinstall_into_managed_dir(
        &self,
        installer_path: &PathBuf,
        version: &str,
    ) -> Result<()> {
        if !cfg!(windows) {
            anyhow::bail!("Reinstall remediation is currently only supported on Windows");
        }

        let uninstall_args = ["/uninstall", "/quiet"];
        match self
            .run_windows_installer(
                installer_path,
                &uninstall_args,
                &[0, 1605, 1614, 3010],
                None,
                "正在执行冲突卸载",
            )
            .await
        {
            Ok(_) => {
                println!("已执行冲突卸载步骤，准备重装到 MeetAI 管理目录。");
            }
            Err(err) => {
                warn!(
                    "Failed to uninstall conflicting Python bundle before reinstall: {:#}",
                    err
                );
                println!("冲突卸载未完全成功，继续尝试直接重装到 MeetAI 管理目录。");
            }
        }

        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "Failed to clean managed install dir before reinstall: {}",
                    install_dir.display()
                )
            })?;
        }

        self.install_python(installer_path, version, None)
            .await
            .with_context(|| {
                format!(
                    "Failed to reinstall Python {} into managed directory",
                    version
                )
            })
    }

    async fn run_windows_installer(
        &self,
        installer_path: &Path,
        args: &[&str],
        accepted_exit_codes: &[i32],
        progress: Option<&ProgressBar>,
        phase: &str,
    ) -> Result<i32> {
        let command_display = Self::format_command(installer_path, args);
        if let Some(pb) = progress {
            pb.set_message(format!("{phase}（启动安装器）..."));
        } else {
            println!("{phase}（启动安装器）...");
        }

        let child = TokioCommand::new(installer_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to execute installer command: {command_display}"))?;
        let mut wait_output = Box::pin(child.wait_with_output());
        let start = Instant::now();
        let mut heartbeat_count = 0u64;
        let output = loop {
            tokio::select! {
                output = &mut wait_output => {
                    break output.with_context(|| format!("Failed while waiting installer command: {command_display}"))?;
                }
                _ = tokio::time::sleep(Duration::from_secs(6)) => {
                    heartbeat_count += 1;
                    let elapsed = Self::format_elapsed(start.elapsed());
                    if let Some(pb) = progress {
                        let len = pb.length().unwrap_or(100);
                        let current = pb.position();
                        let next = (current + 2).min(len.saturating_sub(3));
                        pb.set_position(next);
                        pb.set_message(format!("{phase}（已等待 {elapsed}，请勿关闭窗口）..."));
                        if heartbeat_count == 1 || heartbeat_count % 3 == 0 {
                            pb.println(format!(
                                "{} 已持续 {}，首次安装通常需要 1-3 分钟，请耐心等待。",
                                phase, elapsed
                            ));
                        }
                    } else {
                        println!("{phase} 已持续 {elapsed}，安装过程中短暂无新输出属正常。");
                    }
                }
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        if !accepted_exit_codes.contains(&exit_code) {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "Installer command failed: {}\nexit code: {}\nstdout: {}\nstderr: {}",
                command_display,
                exit_code,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        if exit_code == 3010 {
            println!("安装器提示需要重启系统后部分环境变量才会完全生效。");
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            println!("{stdout}");
        }

        if let Some(pb) = progress {
            pb.set_message(format!(
                "{} 完成（耗时 {}）",
                phase,
                Self::format_elapsed(start.elapsed())
            ));
        }

        Ok(exit_code)
    }

    fn format_elapsed(elapsed: Duration) -> String {
        let total_secs = elapsed.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        if hours > 0 {
            format!("{hours:02}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes:02}:{seconds:02}")
        }
    }

    fn format_command(program: &Path, args: &[&str]) -> String {
        let mut command = program.display().to_string();
        for arg in args {
            command.push(' ');
            command.push_str(arg);
        }
        command
    }

    fn verify_windows_installer_signature(installer_path: &Path) -> Result<()> {
        if !cfg!(windows) {
            return Ok(());
        }

        let powershell = Self::windows_powershell_exe();
        if !powershell.exists() {
            anyhow::bail!(
                "PowerShell executable not found for signature verification: {}",
                powershell.display()
            );
        }

        let script = r#"$sig = Get-AuthenticodeSignature -LiteralPath $env:MEETAI_INSTALLER_PATH
if ($null -eq $sig) {
  Write-Output "UnknownError"
  exit 2
}
Write-Output ($sig.Status.ToString())
if ($null -ne $sig.SignerCertificate) {
  Write-Output $sig.SignerCertificate.Subject
}"#;

        let output = Command::new(&powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("MEETAI_INSTALLER_PATH", installer_path.as_os_str())
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute installer signature verification command with '{}'",
                    powershell.display()
                )
            })?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "Installer signature verification command failed (status: {}).\nstdout: {}\nstderr: {}",
                output.status,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty());
        let status = lines.next().unwrap_or_default();
        let subject = lines.next().unwrap_or_default();
        if status != "Valid" {
            anyhow::bail!(
                "Installer signature is not valid (status: {}). installer: {}",
                status,
                installer_path.display()
            );
        }

        if !Self::is_trusted_python_signer_subject(subject) {
            anyhow::bail!(
                "Unexpected installer signer. expected CN: '{}', actual subject: '{}', installer: {}",
                PYTHON_SIGNER_COMMON_NAME,
                if subject.is_empty() { "<empty>" } else { subject },
                installer_path.display()
            );
        }

        Ok(())
    }

    fn is_trusted_python_signer_subject(subject: &str) -> bool {
        let canonical_cn = subject.split(',').find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            if key.trim().eq_ignore_ascii_case("CN") {
                Some(value.trim())
            } else {
                None
            }
        });

        canonical_cn.is_some_and(|cn| cn.eq_ignore_ascii_case(PYTHON_SIGNER_COMMON_NAME))
    }

    fn windows_system_root() -> PathBuf {
        PathBuf::from(WINDOWS_SYSTEM_ROOT_DEFAULT)
    }

    fn windows_powershell_exe() -> PathBuf {
        Self::windows_system_root()
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    }

    fn windows_reg_exe() -> PathBuf {
        Self::windows_system_root().join("System32").join("reg.exe")
    }

    fn windows_py_launcher_candidates() -> Vec<PathBuf> {
        vec![Self::windows_system_root().join("py.exe")]
    }

    fn find_windows_py_launcher() -> Option<PathBuf> {
        Self::windows_py_launcher_candidates()
            .into_iter()
            .find(|candidate| candidate.exists())
    }

    fn trusted_python_install_roots() -> Vec<PathBuf> {
        let mut roots = Vec::<PathBuf>::new();
        if let Some(home) = home_dir() {
            roots.push(
                home.join("AppData")
                    .join("Local")
                    .join("Programs")
                    .join("Python"),
            );
        }
        roots.push(PathBuf::from(r"C:\Program Files"));
        roots.push(PathBuf::from(r"C:\Program Files (x86)"));
        roots
    }

    fn normalize_path(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn is_path_within_root(candidate: &Path, root: &Path) -> bool {
        if cfg!(windows) {
            let candidate_str = candidate
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            let root_str = root
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            candidate_str == root_str || candidate_str.starts_with(&(root_str + "\\"))
        } else {
            candidate == root || candidate.starts_with(root)
        }
    }

    fn is_trusted_system_python_dir(candidate: &Path, trusted_roots: &[PathBuf]) -> bool {
        if trusted_roots.is_empty() {
            return false;
        }

        let normalized_candidate = Self::normalize_path(candidate);
        trusted_roots.iter().any(|root| {
            let normalized_root = Self::normalize_path(root);
            Self::is_path_within_root(&normalized_candidate, &normalized_root)
        })
    }

    fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
        if metadata.file_type().is_symlink() {
            return true;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            return metadata.file_attributes() & WINDOWS_REPARSE_POINT_ATTRIBUTE != 0;
        }

        #[cfg(not(windows))]
        {
            false
        }
    }

    fn python_output_matches_requested_version(output: &str, version: &str) -> Result<bool> {
        let pattern = format!(r"(?m)^Python\s+{}(?:\s|$)", regex::escape(version));
        let regex = Regex::new(&pattern)
            .with_context(|| format!("Failed to compile Python version match regex: {pattern}"))?;
        Ok(regex.is_match(output))
    }

    fn try_adopt_existing_installation(&self, version: &str) -> Result<bool> {
        self.try_adopt_existing_installation_with_progress(version, None)
    }

    fn try_adopt_existing_installation_with_progress(
        &self,
        version: &str,
        progress: Option<&ProgressBar>,
    ) -> Result<bool> {
        let Some(existing_dir) = self.find_existing_system_python_dir(version)? else {
            return Ok(false);
        };

        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "Failed to remove existing managed install dir before adoption: {}",
                    install_dir.display()
                )
            })?;
        }
        std::fs::create_dir_all(&install_dir).with_context(|| {
            format!(
                "Failed to create managed install dir before adoption: {}",
                install_dir.display()
            )
        })?;

        if let Some(pb) = progress {
            pb.set_position(20);
            pb.set_message("🔍 正在分析系统 Python 文件清单...");
        }

        let plan = Self::build_copy_plan(&existing_dir, progress).with_context(|| {
            format!(
                "Failed to analyze Python installation layout before import: {}",
                existing_dir.display()
            )
        })?;
        let mut status = DirectoryCopyStatus::default();

        if let Some(pb) = progress {
            pb.set_position(35);
            pb.set_message(format!(
                "📂 正在导入系统 Python（共 {} 个文件）...",
                plan.file_count
            ));
        }

        Self::copy_directory_contents_with_progress(
            &existing_dir,
            &install_dir,
            &plan,
            &mut status,
            progress,
        )
        .with_context(|| {
            format!(
                "Failed to import existing Python installation from '{}' to '{}'",
                existing_dir.display(),
                install_dir.display()
            )
        })?;
        if let Some(pb) = progress {
            pb.set_position(97);
            pb.set_message("✅ 正在完成导入收尾...");
        }

        println!(
            "检测到系统已安装 Python {}（{}），已导入到 MeetAI 目录。",
            version,
            existing_dir.display()
        );

        Ok(true)
    }

    #[cfg(test)]
    fn copy_directory_contents(source_dir: &Path, target_dir: &Path) -> Result<()> {
        let plan = Self::build_copy_plan(source_dir, None)?;
        let mut status = DirectoryCopyStatus::default();
        Self::copy_directory_contents_with_progress(
            source_dir,
            target_dir,
            &plan,
            &mut status,
            None,
        )
    }

    fn build_copy_plan(
        source_dir: &Path,
        progress: Option<&ProgressBar>,
    ) -> Result<DirectoryCopyPlan> {
        let mut plan = DirectoryCopyPlan::default();
        let mut scanned_files = 0u64;
        Self::collect_copy_plan_recursive(source_dir, &mut plan, &mut scanned_files, progress)?;
        Ok(plan)
    }

    fn collect_copy_plan_recursive(
        source_dir: &Path,
        plan: &mut DirectoryCopyPlan,
        scanned_files: &mut u64,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if !source_dir.exists() {
            anyhow::bail!(
                "Source directory for copy does not exist: {}",
                source_dir.display()
            );
        }

        for entry in std::fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source dir: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in {}", source_dir.display()))?;
            let source_path = entry.path();
            let metadata = std::fs::symlink_metadata(&source_path).with_context(|| {
                format!(
                    "Failed to read source metadata (without following symlink): {}",
                    source_path.display()
                )
            })?;
            if Self::is_symlink_or_reparse_point(&metadata) {
                anyhow::bail!(
                    "Refusing to import symbolic link/reparse point: {}",
                    source_path.display()
                );
            }

            if metadata.is_dir() {
                Self::collect_copy_plan_recursive(&source_path, plan, scanned_files, progress)?;
            } else if metadata.is_file() {
                plan.file_count += 1;
                plan.total_bytes += metadata.len();
                if plan.file_count > MAX_ADOPT_FILE_COUNT {
                    anyhow::bail!(
                        "Refusing to import Python installation with too many files ({} > {}): {}",
                        plan.file_count,
                        MAX_ADOPT_FILE_COUNT,
                        source_dir.display()
                    );
                }
                if plan.total_bytes > MAX_ADOPT_TOTAL_BYTES {
                    anyhow::bail!(
                        "Refusing to import Python installation larger than limit ({} bytes > {} bytes): {}",
                        plan.total_bytes,
                        MAX_ADOPT_TOTAL_BYTES,
                        source_dir.display()
                    );
                }
                *scanned_files += 1;
                if let Some(pb) = progress {
                    if *scanned_files % 40 == 0 {
                        let scanned_step = (*scanned_files / 40).min(14);
                        let next_pos = (20 + scanned_step).min(34);
                        if next_pos > pb.position() {
                            pb.set_position(next_pos);
                        }
                        pb.set_message(format!(
                            "🔍 正在分析系统 Python 文件清单（已扫描 {} 个文件）...",
                            scanned_files
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn copy_directory_contents_with_progress(
        source_dir: &Path,
        target_dir: &Path,
        plan: &DirectoryCopyPlan,
        status: &mut DirectoryCopyStatus,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if !source_dir.exists() {
            anyhow::bail!(
                "Source directory for copy does not exist: {}",
                source_dir.display()
            );
        }
        std::fs::create_dir_all(target_dir).with_context(|| {
            format!(
                "Failed to create target directory for copy: {}",
                target_dir.display()
            )
        })?;

        for entry in std::fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read source dir: {}", source_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("Failed to read entry in {}", source_dir.display()))?;
            let source_path = entry.path();
            let target_path = target_dir.join(entry.file_name());
            let metadata = std::fs::symlink_metadata(&source_path).with_context(|| {
                format!(
                    "Failed to read source metadata (without following symlink): {}",
                    source_path.display()
                )
            })?;
            if Self::is_symlink_or_reparse_point(&metadata) {
                anyhow::bail!(
                    "Refusing to import symbolic link/reparse point: {}",
                    source_path.display()
                );
            }

            if metadata.is_dir() {
                Self::copy_directory_contents_with_progress(
                    &source_path,
                    &target_path,
                    plan,
                    status,
                    progress,
                )?;
            } else if metadata.is_file() {
                Self::copy_file_with_progress(&source_path, &target_path, status, plan, progress)?;
            }
        }

        Ok(())
    }

    fn copy_file_with_progress(
        source_path: &Path,
        target_path: &Path,
        status: &mut DirectoryCopyStatus,
        plan: &DirectoryCopyPlan,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent dir for {}", target_path.display())
            })?;
        }

        let source_file = std::fs::File::open(source_path).with_context(|| {
            format!(
                "Failed to open source file for copy: {}",
                source_path.display()
            )
        })?;
        let target_file = std::fs::File::create(target_path).with_context(|| {
            format!(
                "Failed to create target file for copy: {}",
                target_path.display()
            )
        })?;

        let mut reader = BufReader::new(source_file);
        let mut writer = BufWriter::new(target_file);
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            let read = reader.read(&mut buffer).with_context(|| {
                format!(
                    "Failed to read source file chunk: {}",
                    source_path.display()
                )
            })?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read]).with_context(|| {
                format!(
                    "Failed to write target file chunk: {}",
                    target_path.display()
                )
            })?;
            status.copied_bytes += read as u64;
            Self::update_adopt_progress(progress, plan, status);
        }
        writer
            .flush()
            .with_context(|| format!("Failed to flush target file: {}", target_path.display()))?;

        status.copied_files += 1;
        Self::update_adopt_progress(progress, plan, status);
        Ok(())
    }

    fn update_adopt_progress(
        progress: Option<&ProgressBar>,
        plan: &DirectoryCopyPlan,
        status: &DirectoryCopyStatus,
    ) {
        let Some(pb) = progress else {
            return;
        };

        let stage_start = 35u64;
        let stage_end = 96u64;
        let stage_range = stage_end.saturating_sub(stage_start);

        let ratio = if plan.total_bytes > 0 {
            status.copied_bytes as f64 / plan.total_bytes as f64
        } else if plan.file_count > 0 {
            status.copied_files as f64 / plan.file_count as f64
        } else {
            1.0
        }
        .clamp(0.0, 1.0);

        let target_pos = stage_start + (ratio * stage_range as f64).round() as u64;
        if target_pos > pb.position() {
            pb.set_position(target_pos.min(stage_end));
        }

        pb.set_message(format!(
            "📂 正在导入系统 Python（{}/{} 文件）...",
            status.copied_files.min(plan.file_count),
            plan.file_count
        ));
    }

    fn find_existing_system_python_dir(&self, version: &str) -> Result<Option<PathBuf>> {
        if !cfg!(windows) {
            return Ok(None);
        }

        let parsed = Version::parse(version)
            .with_context(|| format!("Invalid Python version: {version}"))?;
        let mut candidates = Self::build_default_python_dir_candidates(parsed.major, parsed.minor);
        candidates.extend(Self::collect_registry_python_dir_candidates(
            parsed.major,
            parsed.minor,
        ));
        if let Some(path) = Self::collect_py_launcher_python_dir(parsed.major, parsed.minor) {
            candidates.push(path);
        }
        let trusted_roots = Self::trusted_python_install_roots();

        let mut unique = HashSet::<String>::new();
        candidates.retain(|path| unique.insert(path.to_string_lossy().to_lowercase()));

        for candidate in candidates {
            let python_exe = candidate.join("python.exe");
            if !python_exe.exists() {
                continue;
            }

            if !Self::is_trusted_system_python_dir(&candidate, &trusted_roots) {
                warn!(
                    "Skip untrusted Python installation candidate outside trusted roots: {}",
                    candidate.display()
                );
                continue;
            }

            if Self::python_exe_matches_version(&python_exe, version)? {
                return Ok(Some(candidate));
            }
        }

        Ok(None)
    }

    fn build_default_python_dir_candidates(major: u64, minor: u64) -> Vec<PathBuf> {
        let base_folder = format!("Python{}{}", major, minor);
        let folder_variants = [
            base_folder.clone(),
            format!("{base_folder}-64"),
            format!("{base_folder}-32"),
        ];
        let mut candidates = Vec::<PathBuf>::new();

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(local_app_data)
                .join("Programs")
                .join("Python");
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            let base = PathBuf::from(program_files);
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            let base = PathBuf::from(program_files_x86);
            for variant in &folder_variants {
                candidates.push(base.join(variant));
            }
        }

        candidates
    }

    fn collect_registry_python_dir_candidates(major: u64, minor: u64) -> Vec<PathBuf> {
        let version_key = format!("{major}.{minor}");
        let registry_paths = [
            format!(r"HKCU\Software\Python\PythonCore\{version_key}\InstallPath"),
            format!(r"HKLM\Software\Python\PythonCore\{version_key}\InstallPath"),
            format!(r"HKLM\Software\WOW6432Node\Python\PythonCore\{version_key}\InstallPath"),
        ];

        let mut candidates = Vec::<PathBuf>::new();
        for registry_path in registry_paths {
            candidates.extend(Self::query_registry_install_paths(&registry_path));
        }
        candidates
    }

    fn query_registry_install_paths(registry_path: &str) -> Vec<PathBuf> {
        let reg_exe = Self::windows_reg_exe();
        if !reg_exe.exists() {
            warn!("Registry command not found: {}", reg_exe.display());
            return Vec::new();
        }

        let output = Command::new(&reg_exe)
            .args(["query", registry_path])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };

        if !output.status.success() {
            return Vec::new();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter_map(Self::extract_registry_install_path_from_line)
            .collect()
    }

    fn extract_registry_install_path_from_line(line: &str) -> Option<PathBuf> {
        let (_, value_data) = line.split_once("REG_SZ")?;
        let raw = value_data.trim().trim_matches('"');
        if raw.is_empty() {
            return None;
        }

        let mut candidate = PathBuf::from(raw);
        if candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("python.exe"))
        {
            candidate.pop();
        }

        if candidate.as_os_str().is_empty() {
            None
        } else {
            Some(candidate)
        }
    }

    fn collect_py_launcher_python_dir(major: u64, minor: u64) -> Option<PathBuf> {
        let selector = format!("-{major}.{minor}");
        let py_launcher = Self::find_windows_py_launcher()?;
        let output = Command::new(&py_launcher)
            .args([selector.as_str(), "-c", "import sys; print(sys.executable)"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())?;
        let mut candidate = PathBuf::from(first_line.trim_matches('"'));

        if candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("python.exe"))
        {
            candidate.pop();
        }

        if candidate.as_os_str().is_empty() {
            None
        } else {
            Some(candidate)
        }
    }

    fn python_exe_matches_version(python_exe: &Path, version: &str) -> Result<bool> {
        let output = Command::new(python_exe)
            .arg("--version")
            .output()
            .with_context(|| format!("Failed to execute '{} --version'", python_exe.display()))?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout.trim(), stderr.trim());
        Self::python_output_matches_requested_version(&combined, version)
    }

    /// 清理失败安装残留
    fn cleanup_failed_install(&self, version: &str, installer_path: &PathBuf) {
        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            if let Err(err) = std::fs::remove_dir_all(&install_dir) {
                warn!(
                    "Failed to clean installation directory after failed install '{}': {} ({:#})",
                    version,
                    install_dir.display(),
                    err
                );
            }
        }

        if installer_path.exists() {
            if let Err(err) = std::fs::remove_file(installer_path) {
                warn!(
                    "Failed to remove installer file after failed install '{}': {} ({:#})",
                    version,
                    installer_path.display(),
                    err
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            downloader: Downloader::new(),
            executor: CommandExecutor::new(),
        })
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
    fn verify_installation_fails_when_executable_missing() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;
        let version = "3.12.1";

        let install_dir = installer.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;

        let err = installer
            .verify_installation(version)
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
                err.to_string()
                    .contains("当前仅支持 Windows 自动安装"),
                "unexpected error: {err:#}"
            );
        }

        Ok(())
    }

    #[test]
    fn get_download_sources_prerelease_uses_official_only() -> Result<()> {
        let temp = tempdir()?;
        let installer = make_installer(temp.path())?;

        if cfg!(windows) {
            let sources = installer.get_download_sources("3.15.0a6")?;
            assert_eq!(
                sources.len(),
                2,
                "pre-release should only use official source (arch + generic)"
            );
            assert_eq!(sources[0].name, "Python 官方源");
            assert_eq!(sources[1].name, "Python 官方源");
        } else {
            let err = installer
                .get_download_sources("3.15.0a6")
                .expect_err("non-windows should return unsupported error");
            assert!(
                err.to_string()
                    .contains("当前仅支持 Windows 自动安装"),
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

    #[test]
    fn build_download_failure_message_contains_network_diagnostics() {
        let message = PythonInstaller::build_download_failure_message(
            "3.14.3",
            "  1. Python 官方源: https://example.com/official.exe\n     错误: Download failed for URL https://example.com/official.exe with status: 404 Not Found",
        );

        assert!(
            message.contains("网络诊断建议"),
            "error message should include diagnostic header, got: {message}"
        );
        assert!(
            message.contains("HTTP_PROXY") && message.contains("HTTPS_PROXY"),
            "error message should include proxy environment hints, got: {message}"
        );
        assert!(
            message.contains("系统时间"),
            "error message should include system time hint, got: {message}"
        );
    }
}
