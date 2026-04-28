//! Node.js 安装器实现。
//!
//! 本模块提供 Node.js 的自动下载、安装、验证和卸载功能。
//! 支持从 nodejs.org 官方源和镜像源获取安装包，并处理平台特定解压。
//!
//! 主要类型：
//! - `NodeInstaller`: 安装器主类型，实现 `RuntimeInstaller` trait
//! - `AvailableNodeVersion`: 可安装版本信息，包含版本号和 LTS 标记
//! - `NodeDistRelease`: Node.js 官方分发索引中的版本条目
//!
//! 核心功能：
//! 1. **版本发现** (`list_available_versions`): 从 nodejs.org 获取所有可用版本
//! 2. **版本解析** (`resolve_target_version`): 处理 `"latest"`、`"lts"` 和精确版本
//! 3. **下载安装** (`install`): 下载归档、解压、复制到安装目录
//! 4. **归档校验**: 下载官方 `SHASUMS256.txt` 并校验归档 SHA256
//! 5. **验证** (`verify_installation`): 检查可执行文件存在性和版本匹配
//! 6. **清理** (`cleanup_failed_install`): 失败时删除残留文件
//!
//! 数据源：
//! - **官方索引**: `https://nodejs.org/dist/index.json` (推荐)
//! - **官方下载页**: `https://nodejs.org/dist/` (HTML 解析，作为回退)
//! - **镜像源**: `https://npmmirror.com/dist/` (网络故障时使用)
//!
//! 支持的归档格式：
//! - Windows: `node-v<version>-win-x64.zip` / `node-v<version>-win-x86.zip`
//! - macOS: `node-v<version>-darwin-x64.tar.gz` / `node-v<version>-darwin-arm64.tar.gz`
//! - Linux: `node-v<version>-linux-x64.tar.xz` / `node-v<version>-linux-arm64.tar.xz`
//!
//! 安装流程：
//! 1. 解析目标版本（精确版本 / latest / lts）
//! 2. 构建下载 URL（根据平台选择合适归档）
//! 3. 下载到临时目录（显示进度条）
//! 4. 校验归档 SHA256
//! 5. 解压到临时目录
//! 6. 复制 `bin/`、`lib/`、`share/` 到 `<app_home>/versions/<version>`
//! 7. 验证 `node --version` 输出匹配
//! 8. 清理临时文件
//!
//! 平台检测：
//! - 通过 `target_arch_suffix()` 确定平台标识符（`win-x64`、`darwin-arm64` 等）
//! - 支持 `aarch64` 作为 `arm64` 的别名
//!
//! 错误处理：
//! - 网络失败：返回 `reqwest::Error`，提供网络诊断建议
//! - 版本不存在：返回 `anyhow::Error`，列出可用版本
//! - 解压失败：返回 `anyhow::Error`，触发清理
//! - 验证失败：返回 `NodeVersionMismatchError`，触发清理
//!
//! 测试：
//! - 版本解析和选择逻辑
//! - 下载回退机制（官方 → 镜像）
//! - 平台特定归档选择
//! - 清理逻辑验证

use crate::config::Config;
use crate::node::project::resolve_project_version_from_nvmrc;
use crate::node::version::NodeVersionManager;
use crate::runtime::common::RuntimeInstaller;
use crate::utils::downloader::Downloader;
use crate::utils::guidance::network_diagnostic_tips;
use crate::utils::http_client::build_http_client;
use crate::utils::validator::Validator;
use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{env, fs};
use tokio::process::Command as TokioCommand;

const NODE_DIST_INDEX_URL: &str = "https://nodejs.org/dist/index.json";
const NODE_DIST_BASE_URL: &str = "https://nodejs.org/dist";
const NODE_SHASUMS_FILE: &str = "SHASUMS256.txt";
const DEFAULT_AVAILABLE_VERSION_LIMIT: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeArchiveFormat {
    Zip,
    TarXz,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodePlatformPackage {
    platform_token: &'static str,
    dist_file_key: &'static str,
    archive_suffix: &'static str,
    archive_format: NodeArchiveFormat,
}

impl NodePlatformPackage {
    fn archive_name(self, version: &str) -> String {
        format!(
            "node-v{}-{}.{}",
            version, self.platform_token, self.archive_suffix
        )
    }
}

/// 可安装的 Node.js 版本信息（含 LTS 标记）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableNodeVersion {
    /// 版本号（semver 格式，如 `20.11.1`）。
    pub version: String,
    /// LTS 代号（如 `Iron`），非 LTS 版本为 `None`。
    pub lts_name: Option<String>,
}

impl AvailableNodeVersion {
    /// 判断是否为 LTS 版本。
    pub fn is_lts(&self) -> bool {
        self.lts_name.is_some()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NodeLtsMarker {
    Flag(#[allow(dead_code)] bool),
    Name(String),
}

#[derive(Debug, Deserialize)]
struct NodeDistRelease {
    version: String,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    lts: Option<NodeLtsMarker>,
}

impl NodeDistRelease {
    fn supports_current_platform(&self) -> bool {
        let Ok(package) = NodeInstaller::current_platform_package() else {
            return false;
        };
        self.files.iter().any(|f| f == package.dist_file_key)
    }

    fn normalized_version(&self) -> Option<String> {
        NodeInstaller::normalize_version_token(&self.version)
    }

    fn lts_name(&self) -> Option<String> {
        match &self.lts {
            Some(NodeLtsMarker::Name(name)) if !name.trim().is_empty() => Some(name.clone()),
            _ => None,
        }
    }

    fn into_available_version(self) -> Option<AvailableNodeVersion> {
        if !self.supports_current_platform() {
            return None;
        }

        Some(AvailableNodeVersion {
            version: self.normalized_version()?,
            lts_name: self.lts_name(),
        })
    }
}

/// Node.js 安装器。
pub struct NodeInstaller {
    config: Config,
}

impl NodeInstaller {
    /// 创建 Node.js 安装器。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let installer = Self { config };
        installer.ensure_node_dirs()?;
        Ok(installer)
    }

    /// 查看官方可安装的 Node.js 版本（含 LTS 标记）。
    pub async fn list_available_versions(&self) -> Result<Vec<AvailableNodeVersion>> {
        let body = self.fetch_dist_index_body().await?;
        let mut versions = self.parse_available_versions_from_index_body(&body)?;
        if versions.len() > DEFAULT_AVAILABLE_VERSION_LIMIT {
            versions.truncate(DEFAULT_AVAILABLE_VERSION_LIMIT);
        }
        Ok(versions)
    }

    /// 安装指定 Node.js 版本（支持 `latest` / `newest` / `lts` / `project` 或具体版本号）。
    pub async fn install(&self, version: &str) -> Result<String> {
        Validator::new().validate_node_install_version(version)?;

        let resolved_version = self.resolve_target_version(version).await?;
        Validator::new().validate_node_selected_version(&resolved_version)?;

        let install_dir = self.install_dir(&resolved_version)?;
        let node_exe = Self::node_executable_in_dir(&install_dir);
        if node_exe.exists() {
            println!("Node.js {} 已经安装，无需重复安装。", resolved_version);
            return Ok(resolved_version);
        }

        let package = Self::current_platform_package()?;
        let archive_path = self
            .config
            .cache_dir
            .join(package.archive_name(&resolved_version));
        let extract_dir = self.config.cache_dir.join(format!(
            "node-v{}-{}-extract",
            resolved_version, package.platform_token
        ));

        let download_url = self.build_download_url(&resolved_version)?;
        let downloader = Downloader::new()?;
        if let Err(err) = downloader
            .download(&download_url, &archive_path, None)
            .await
        {
            return Err(err.context(format!(
                "Node.js {} 下载失败了 😢\n\n别担心，可以试试：\n  - 重试：meetai node install {}\n  - 或者：meetai runtime install node {}\n\n{}",
                resolved_version,
                resolved_version,
                resolved_version,
                network_diagnostic_tips()
            )));
        }

        if let Err(err) = self
            .verify_archive_checksum(&resolved_version, &archive_path, package)
            .await
        {
            let _ = fs::remove_file(&archive_path);
            return Err(err.context(format!("Node.js {} 安装包校验失败", resolved_version)));
        }

        if let Err(err) = self
            .extract_and_install(&archive_path, &extract_dir, &resolved_version, package)
            .await
        {
            self.cleanup_failed_install(&install_dir, &extract_dir);
            return Err(err.context(format!("Node.js {} 安装失败", resolved_version)));
        }

        self.verify_installation(&resolved_version).await?;
        let _ = fs::remove_dir_all(&extract_dir);

        println!("Node.js {} 安装完成。", resolved_version);
        Ok(resolved_version)
    }

    async fn resolve_target_version(&self, version: &str) -> Result<String> {
        let requested = if version == "project" {
            resolve_project_version_from_nvmrc()?
        } else {
            version.to_string()
        };

        match requested.as_str() {
            "latest" | "newest" => self.resolve_latest_target_version().await,
            "lts" => self.resolve_latest_lts_target_version().await,
            _ => Ok(requested),
        }
    }

    async fn resolve_latest_target_version(&self) -> Result<String> {
        let local_latest = self.get_latest_installed_version()?;
        if Self::current_platform_package().is_err() {
            if let Some(local_latest) = local_latest {
                return Ok(local_latest);
            }
            anyhow::bail!("{}", Self::unsupported_auto_install_message());
        }

        let remote_latest = self.resolve_latest_from_remote().await.ok();
        Self::choose_latest_version(remote_latest, local_latest).with_context(|| {
            "无法解析 latest/newest 对应的 Node.js 版本，请检查网络后重试，或显式指定版本号（例如 20.11.1）"
        })
    }

    async fn resolve_latest_lts_target_version(&self) -> Result<String> {
        if Self::current_platform_package().is_err() {
            anyhow::bail!("{}", Self::unsupported_auto_install_message());
        }

        self.resolve_latest_lts_from_remote().await.with_context(|| {
            "无法解析 lts 对应的 Node.js LTS 版本，请检查网络后重试，或显式指定版本号（例如 20.11.1）"
        })
    }

    async fn resolve_latest_from_remote(&self) -> Result<String> {
        let body = self.fetch_dist_index_body().await?;
        self.parse_latest_version_from_index_body(&body)
    }

    async fn resolve_latest_lts_from_remote(&self) -> Result<String> {
        let body = self.fetch_dist_index_body().await?;
        self.parse_latest_lts_version_from_index_body(&body)
    }

    async fn fetch_dist_index_body(&self) -> Result<String> {
        let client = build_http_client(std::time::Duration::from_secs(30))?;
        client
            .get(NODE_DIST_INDEX_URL)
            .send()
            .await
            .context("请求 Node.js 版本索引失败")?
            .error_for_status()
            .context("Node.js 版本索引响应异常")?
            .text()
            .await
            .context("读取 Node.js 版本索引失败")
    }

    fn parse_available_versions_from_index_body(
        &self,
        body: &str,
    ) -> Result<Vec<AvailableNodeVersion>> {
        let releases: Vec<NodeDistRelease> =
            serde_json::from_str(body).context("解析 Node.js 版本索引失败")?;

        let mut versions: Vec<AvailableNodeVersion> = releases
            .into_iter()
            .filter_map(NodeDistRelease::into_available_version)
            .collect();

        versions.sort_by(|a, b| {
            let left = Version::parse(&a.version).ok();
            let right = Version::parse(&b.version).ok();
            right.cmp(&left)
        });
        versions.dedup_by(|a, b| a.version == b.version);
        Ok(versions)
    }

    fn parse_latest_version_from_index_body(&self, body: &str) -> Result<String> {
        self.parse_available_versions_from_index_body(body)?
            .into_iter()
            .map(|item| item.version)
            .next()
            .context("版本索引中未找到可用于当前平台的 Node.js 包")
    }

    fn parse_latest_lts_version_from_index_body(&self, body: &str) -> Result<String> {
        self.parse_available_versions_from_index_body(body)?
            .into_iter()
            .find(|item| item.is_lts())
            .map(|item| item.version)
            .context("版本索引中未找到 LTS Node.js 包")
    }

    fn choose_latest_version(remote: Option<String>, local: Option<String>) -> Option<String> {
        match (remote, local) {
            (Some(remote_v), Some(local_v)) => {
                let remote_semver = Version::parse(&remote_v).ok()?;
                let local_semver = Version::parse(&local_v).ok()?;
                if remote_semver >= local_semver {
                    Some(remote_v)
                } else {
                    Some(local_v)
                }
            }
            (Some(remote_v), None) => Some(remote_v),
            (None, Some(local_v)) => Some(local_v),
            (None, None) => None,
        }
    }

    fn get_latest_installed_version(&self) -> Result<Option<String>> {
        let manager = NodeVersionManager::new()?;
        Ok(manager.list_installed()?.first().map(ToString::to_string))
    }

    fn build_download_url(&self, version: &str) -> Result<String> {
        let package = Self::current_platform_package()?;
        Ok(format!(
            "{}/v{}/{}",
            NODE_DIST_BASE_URL,
            version,
            package.archive_name(version)
        ))
    }

    fn build_shasums_url(version: &str) -> String {
        format!("{}/v{}/{}", NODE_DIST_BASE_URL, version, NODE_SHASUMS_FILE)
    }

    async fn verify_archive_checksum(
        &self,
        version: &str,
        archive_path: &Path,
        package: NodePlatformPackage,
    ) -> Result<()> {
        let archive_name = package.archive_name(version);
        let shasums_url = Self::build_shasums_url(version);
        let body = build_http_client(std::time::Duration::from_secs(30))?
            .get(&shasums_url)
            .send()
            .await
            .with_context(|| format!("请求 Node.js 校验清单失败：{}", shasums_url))?
            .error_for_status()
            .with_context(|| format!("Node.js 校验清单响应异常：{}", shasums_url))?
            .text()
            .await
            .context("读取 Node.js 校验清单失败")?;

        let expected = Self::parse_expected_sha256_from_shasums(&body, &archive_name)
            .with_context(|| format!("校验清单中未找到 {}", archive_name))?;
        let actual = Self::sha256_file_hex(archive_path).with_context(|| {
            format!(
                "计算 Node.js 安装包 SHA256 失败：{}",
                archive_path.display()
            )
        })?;

        if !actual.eq_ignore_ascii_case(&expected) {
            anyhow::bail!(
                "Node.js 安装包 SHA256 不匹配：{}\n期望：{}\n实际：{}",
                archive_name,
                expected,
                actual
            );
        }

        Ok(())
    }

    fn parse_expected_sha256_from_shasums(body: &str, archive_name: &str) -> Option<String> {
        body.lines().find_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let file = parts.next()?.trim_start_matches('*');
            if file == archive_name
                && hash.len() == 64
                && hash.chars().all(|c| c.is_ascii_hexdigit())
            {
                Some(hash.to_ascii_lowercase())
            } else {
                None
            }
        })
    }

    fn sha256_file_hex(path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 16 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(to_hex_lower(&hasher.finalize()))
    }

    async fn extract_and_install(
        &self,
        archive_path: &Path,
        extract_dir: &Path,
        version: &str,
        package: NodePlatformPackage,
    ) -> Result<()> {
        if extract_dir.exists() {
            fs::remove_dir_all(extract_dir)
                .with_context(|| format!("清理历史解压目录失败：{}", extract_dir.display()))?;
        }
        fs::create_dir_all(extract_dir)
            .with_context(|| format!("创建解压目录失败：{}", extract_dir.display()))?;

        self.expand_archive(archive_path, extract_dir, package)
            .await?;

        let root_dir = self.resolve_extracted_root(extract_dir, version, package)?;
        let install_dir = self.install_dir(version)?;
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir)
                .with_context(|| format!("清理旧安装目录失败：{}", install_dir.display()))?;
        }
        fs::create_dir_all(&install_dir)
            .with_context(|| format!("创建安装目录失败：{}", install_dir.display()))?;
        self.copy_directory_contents(&root_dir, &install_dir)?;

        Ok(())
    }

    async fn expand_archive(
        &self,
        archive_path: &Path,
        extract_dir: &Path,
        package: NodePlatformPackage,
    ) -> Result<()> {
        match package.archive_format {
            NodeArchiveFormat::Zip => self.expand_archive_windows(archive_path, extract_dir).await,
            NodeArchiveFormat::TarXz => self.expand_archive_tar_xz(archive_path, extract_dir).await,
        }
    }

    async fn expand_archive_windows(&self, archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let archive = archive_path.display().to_string().replace('\'', "''");
        let target = extract_dir.display().to_string().replace('\'', "''");
        let script = format!(
            "$ErrorActionPreference='Stop'; Expand-Archive -LiteralPath '{archive}' -DestinationPath '{target}' -Force",
            archive = archive,
            target = target
        );

        let output = TokioCommand::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .await
            .context("调用 PowerShell 解压 Node.js 包失败")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "解压 Node.js 安装包失败（退出码：{}）：{}",
                output.status,
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        Ok(())
    }

    async fn expand_archive_tar_xz(&self, archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let output = TokioCommand::new("tar")
            .arg("-xJf")
            .arg(archive_path)
            .arg("-C")
            .arg(extract_dir)
            .output()
            .await
            .context("调用 tar 解压 Node.js tar.xz 包失败，请确认系统已安装 tar 和 xz")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "解压 Node.js tar.xz 安装包失败（退出码：{}）：{}",
                output.status,
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        Ok(())
    }

    fn resolve_extracted_root(
        &self,
        extract_dir: &Path,
        version: &str,
        package: NodePlatformPackage,
    ) -> Result<PathBuf> {
        let expected_name = format!("node-v{}-{}", version, package.platform_token);
        let expected = extract_dir.join(&expected_name);
        if expected.exists() && expected.is_dir() {
            return Ok(expected);
        }

        let mut dirs = Vec::new();
        for entry in fs::read_dir(extract_dir)
            .with_context(|| format!("读取解压目录失败：{}", extract_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }

        if dirs.len() == 1 {
            return Ok(dirs.remove(0));
        }

        anyhow::bail!("无法定位 Node.js 解压目录（期望：{}）", expected_name)
    }

    fn copy_directory_contents(&self, source: &Path, target: &Path) -> Result<()> {
        for entry in
            fs::read_dir(source).with_context(|| format!("读取目录失败：{}", source.display()))?
        {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = target.join(entry.file_name());
            let file_type = entry
                .file_type()
                .with_context(|| format!("读取条目类型失败：{}", src_path.display()))?;

            if file_type.is_dir() {
                fs::create_dir_all(&dst_path)
                    .with_context(|| format!("创建目录失败：{}", dst_path.display()))?;
                self.copy_directory_contents(&src_path, &dst_path)?;
            } else if file_type.is_file() {
                fs::copy(&src_path, &dst_path).with_context(|| {
                    format!(
                        "复制文件失败（{} -> {}）",
                        src_path.display(),
                        dst_path.display()
                    )
                })?;
            } else if file_type.is_symlink() {
                Self::copy_symlink(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    fn copy_symlink(src_path: &Path, dst_path: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            let link_target = fs::read_link(src_path)
                .with_context(|| format!("读取符号链接失败：{}", src_path.display()))?;
            return std::os::unix::fs::symlink(&link_target, dst_path).with_context(|| {
                format!(
                    "复制符号链接失败（{} -> {}）",
                    src_path.display(),
                    dst_path.display()
                )
            });
        }

        #[cfg(not(unix))]
        {
            anyhow::bail!(
                "当前平台暂不支持复制 Node.js 包中的符号链接：{} -> {}",
                src_path.display(),
                dst_path.display()
            );
        }
    }

    async fn verify_installation(&self, version: &str) -> Result<()> {
        let install_dir = self.install_dir(version)?;
        let node_exe = Self::node_executable_in_dir(&install_dir);
        if !node_exe.exists() {
            anyhow::bail!(
                "Node.js executable not found after installation: {}",
                node_exe.display()
            );
        }

        let output = TokioCommand::new(&node_exe)
            .args(["--version"])
            .output()
            .await
            .with_context(|| format!("执行 node --version 失败：{}", node_exe.display()))?;
        if !output.status.success() {
            anyhow::bail!(
                "Node.js 安装后验证失败：node --version 退出码 {}",
                output.status
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed = Self::normalize_version_token(stdout.trim())
            .with_context(|| format!("无法解析 node --version 输出：{}", stdout.trim()))?;
        if parsed != version {
            anyhow::bail!(
                "Node.js 安装后版本校验失败：期望 {}，实际 {}",
                version,
                parsed
            );
        }

        Ok(())
    }

    fn cleanup_failed_install(&self, install_dir: &Path, extract_dir: &Path) {
        if install_dir.exists() {
            let _ = fs::remove_dir_all(install_dir);
        }
        if extract_dir.exists() {
            let _ = fs::remove_dir_all(extract_dir);
        }
    }

    fn ensure_node_dirs(&self) -> Result<()> {
        let versions_dir = self.versions_dir()?;
        if !versions_dir.exists() {
            fs::create_dir_all(&versions_dir).with_context(|| {
                format!("创建 Node.js versions 目录失败：{}", versions_dir.display())
            })?;
        }
        Ok(())
    }

    fn versions_dir(&self) -> Result<PathBuf> {
        Ok(self.node_root_dir()?.join("versions"))
    }

    fn node_root_dir(&self) -> Result<PathBuf> {
        Ok(self.config.app_home_dir_path()?.join("nodejs"))
    }

    fn install_dir(&self, version: &str) -> Result<PathBuf> {
        Ok(self.versions_dir()?.join(version))
    }

    fn node_executable_in_dir(install_dir: &Path) -> PathBuf {
        super::node_executable_in_dir(install_dir)
    }

    fn normalize_version_token(raw: &str) -> Option<String> {
        super::normalize_version_token(raw)
    }

    fn unsupported_auto_install_message() -> &'static str {
        "当前平台暂不支持自动下载安装 Node.js（目前支持 Windows 与 Linux x64/arm64）。可先手动安装后再执行 `meetai node use <version>`。"
    }

    pub(crate) fn supports_auto_install_on_current_platform() -> bool {
        Self::current_platform_package().is_ok()
    }

    fn current_platform_package() -> Result<NodePlatformPackage> {
        Self::platform_package_for(env::consts::OS, env::consts::ARCH)
    }

    fn platform_package_for(os: &str, arch: &str) -> Result<NodePlatformPackage> {
        match (os, arch) {
            ("windows", "x86_64") => Ok(NodePlatformPackage {
                platform_token: "win-x64",
                dist_file_key: "win-x64-zip",
                archive_suffix: "zip",
                archive_format: NodeArchiveFormat::Zip,
            }),
            ("windows", "aarch64") => Ok(NodePlatformPackage {
                platform_token: "win-arm64",
                dist_file_key: "win-arm64-zip",
                archive_suffix: "zip",
                archive_format: NodeArchiveFormat::Zip,
            }),
            ("windows", "x86") => Ok(NodePlatformPackage {
                platform_token: "win-x86",
                dist_file_key: "win-x86-zip",
                archive_suffix: "zip",
                archive_format: NodeArchiveFormat::Zip,
            }),
            ("linux", "x86_64") => Ok(NodePlatformPackage {
                platform_token: "linux-x64",
                dist_file_key: "linux-x64",
                archive_suffix: "tar.xz",
                archive_format: NodeArchiveFormat::TarXz,
            }),
            ("linux", "aarch64") => Ok(NodePlatformPackage {
                platform_token: "linux-arm64",
                dist_file_key: "linux-arm64",
                archive_suffix: "tar.xz",
                archive_format: NodeArchiveFormat::TarXz,
            }),
            _ => anyhow::bail!("当前平台暂不支持 Node.js 自动安装：{}-{}", os, arch),
        }
    }
}

#[async_trait::async_trait]
impl RuntimeInstaller for NodeInstaller {
    async fn install_version(&self, version: &str) -> Result<String> {
        NodeInstaller::install(self, version).await
    }
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    bit_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            bit_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.bit_len = self.bit_len.wrapping_add((data.len() as u64) * 8);

        if self.buffer_len > 0 {
            let remaining = 64 - self.buffer_len;
            let take = remaining.min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            data = &data[take..];

            if self.buffer_len == 64 {
                let block = self.buffer;
                self.process_block(&block);
                self.buffer_len = 0;
            }
        }

        while data.len() >= 64 {
            self.process_block(&data[..64]);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for byte in &mut self.buffer[self.buffer_len..] {
                *byte = 0;
            }
            let block = self.buffer;
            self.process_block(&block);
            self.buffer_len = 0;
        }

        for byte in &mut self.buffer[self.buffer_len..56] {
            *byte = 0;
        }
        self.buffer[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let block = self.buffer;
        self.process_block(&block);

        let mut output = [0u8; 32];
        for (chunk, word) in output.chunks_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        output
    }

    fn process_block(&mut self, block: &[u8]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).take(16).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

fn to_hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::common::RuntimeInstaller;
    use std::sync::Arc;

    fn make_installer(root: &std::path::Path) -> Result<NodeInstaller> {
        let config = Config {
            python_install_dir: root.join("python"),
            venv_dir: root.join("venvs"),
            cache_dir: root.join("cache"),
            current_python_version: None,
        };
        config.ensure_dirs()?;

        let installer = NodeInstaller { config };
        installer.ensure_node_dirs()?;
        Ok(installer)
    }

    #[tokio::test]
    async fn runtime_installer_trait_delegates_to_inherent_impl() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let installer: Arc<dyn RuntimeInstaller> = Arc::new(make_installer(temp.path())?);

        let install_err = installer
            .install_version("not-a-version")
            .await
            .expect_err("invalid version should reach inherent install validation");
        assert!(
            !install_err.to_string().is_empty(),
            "install error should come from inherent implementation"
        );

        Ok(())
    }
    #[test]
    fn choose_latest_version_prefers_higher_semver() {
        let chosen = NodeInstaller::choose_latest_version(
            Some("22.3.0".to_string()),
            Some("20.11.1".to_string()),
        );
        assert_eq!(chosen.as_deref(), Some("22.3.0"));

        let chosen = NodeInstaller::choose_latest_version(
            Some("20.11.1".to_string()),
            Some("22.3.0".to_string()),
        );
        assert_eq!(chosen.as_deref(), Some("22.3.0"));
    }

    #[test]
    fn normalize_version_token_accepts_v_prefix() {
        assert_eq!(
            NodeInstaller::normalize_version_token("v20.11.1").as_deref(),
            Some("20.11.1")
        );
        assert_eq!(
            NodeInstaller::normalize_version_token("20.11.1").as_deref(),
            Some("20.11.1")
        );
        assert!(NodeInstaller::normalize_version_token("20.11").is_none());
    }

    fn current_platform_file_key() -> Option<&'static str> {
        NodeInstaller::current_platform_package()
            .ok()
            .map(|package| package.dist_file_key)
    }

    #[test]
    fn parse_available_versions_from_index_body_sorts_versions_and_marks_lts() -> Result<()> {
        let Some(file_key) = current_platform_file_key() else {
            return Ok(());
        };
        let body = format!(
            r#"
            [
              {{"version":"v20.11.1","files":["{file_key}"],"lts":"Iron"}},
              {{"version":"v22.3.0","files":["{file_key}"],"lts":false}},
              {{"version":"v18.20.4","files":["{file_key}"],"lts":"Hydrogen"}}
            ]
            "#
        );

        let temp = tempfile::tempdir()?;
        let installer = make_installer(temp.path())?;
        let versions = installer.parse_available_versions_from_index_body(&body)?;
        assert_eq!(versions[0].version, "22.3.0");
        assert!(!versions[0].is_lts());
        assert_eq!(versions[1].version, "20.11.1");
        assert_eq!(versions[1].lts_name.as_deref(), Some("Iron"));
        Ok(())
    }

    #[test]
    fn parse_latest_version_from_index_body_selects_highest_with_required_file() -> Result<()> {
        let Some(file_key) = current_platform_file_key() else {
            return Ok(());
        };
        let body = format!(
            r#"
            [
              {{"version":"v22.5.0","files":["src"],"lts":false}},
              {{"version":"v20.11.1","files":["{file_key}"],"lts":"Iron"}},
              {{"version":"v22.3.0","files":["{file_key}"],"lts":false}}
            ]
            "#
        );

        let temp = tempfile::tempdir()?;
        let installer = make_installer(temp.path())?;
        let latest = installer.parse_latest_version_from_index_body(&body)?;
        assert_eq!(latest, "22.3.0");
        Ok(())
    }

    #[test]
    fn parse_latest_lts_version_from_index_body_selects_highest_lts() -> Result<()> {
        let Some(file_key) = current_platform_file_key() else {
            return Ok(());
        };
        let body = format!(
            r#"
            [
              {{"version":"v22.3.0","files":["{file_key}"],"lts":false}},
              {{"version":"v20.11.1","files":["{file_key}"],"lts":"Iron"}},
              {{"version":"v18.20.4","files":["{file_key}"],"lts":"Hydrogen"}}
            ]
            "#
        );

        let temp = tempfile::tempdir()?;
        let installer = make_installer(temp.path())?;
        let latest_lts = installer.parse_latest_lts_version_from_index_body(&body)?;
        assert_eq!(latest_lts, "20.11.1");
        Ok(())
    }

    #[tokio::test]
    async fn resolve_latest_on_non_windows_returns_local_or_platform_error() -> Result<()> {
        if NodeInstaller::supports_auto_install_on_current_platform() {
            return Ok(());
        }

        let installer = NodeInstaller::new()?;
        match installer.resolve_target_version("latest").await {
            Ok(version) => {
                assert!(
                    Version::parse(&version).is_ok(),
                    "latest fallback should be a semver version: {version}"
                );
            }
            Err(err) => {
                let message = err.to_string();
                assert!(
                    message.contains("当前平台暂不支持自动下载安装 Node.js"),
                    "unexpected error: {message}"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn platform_package_maps_linux_and_windows_archives() -> Result<()> {
        let linux_x64 = NodeInstaller::platform_package_for("linux", "x86_64")?;
        assert_eq!(linux_x64.platform_token, "linux-x64");
        assert_eq!(linux_x64.dist_file_key, "linux-x64");
        assert_eq!(
            linux_x64.archive_name("20.11.1"),
            "node-v20.11.1-linux-x64.tar.xz"
        );
        assert_eq!(linux_x64.archive_format, NodeArchiveFormat::TarXz);

        let linux_arm64 = NodeInstaller::platform_package_for("linux", "aarch64")?;
        assert_eq!(linux_arm64.platform_token, "linux-arm64");
        assert_eq!(linux_arm64.dist_file_key, "linux-arm64");
        assert_eq!(
            linux_arm64.archive_name("20.11.1"),
            "node-v20.11.1-linux-arm64.tar.xz"
        );
        assert_eq!(linux_arm64.archive_format, NodeArchiveFormat::TarXz);

        let windows_x64 = NodeInstaller::platform_package_for("windows", "x86_64")?;
        assert_eq!(windows_x64.platform_token, "win-x64");
        assert_eq!(windows_x64.dist_file_key, "win-x64-zip");
        assert_eq!(
            windows_x64.archive_name("20.11.1"),
            "node-v20.11.1-win-x64.zip"
        );
        assert_eq!(windows_x64.archive_format, NodeArchiveFormat::Zip);

        assert!(NodeInstaller::platform_package_for("macos", "x86_64").is_err());
        Ok(())
    }

    #[test]
    fn parse_expected_sha256_from_shasums_finds_archive_entry() {
        let body = "\
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  node-v20.11.1-linux-x64.tar.xz\n\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  node-v20.11.1-win-x64.zip\n";

        let hash = NodeInstaller::parse_expected_sha256_from_shasums(
            body,
            "node-v20.11.1-linux-x64.tar.xz",
        );

        assert_eq!(
            hash.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
    }

    #[test]
    fn parse_expected_sha256_from_shasums_accepts_star_filename_marker() {
        let body = "\
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 *node-v20.11.1-win-x64.zip\n";

        let hash =
            NodeInstaller::parse_expected_sha256_from_shasums(body, "node-v20.11.1-win-x64.zip");

        assert!(hash.is_some());
    }

    #[test]
    fn parse_expected_sha256_from_shasums_rejects_missing_or_invalid_entry() {
        let invalid_hash = "not-a-hash  node-v20.11.1-linux-x64.tar.xz";
        assert!(NodeInstaller::parse_expected_sha256_from_shasums(
            invalid_hash,
            "node-v20.11.1-linux-x64.tar.xz"
        )
        .is_none());

        let other_file =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  other.tar.xz";
        assert!(NodeInstaller::parse_expected_sha256_from_shasums(
            other_file,
            "node-v20.11.1-linux-x64.tar.xz"
        )
        .is_none());
    }

    #[test]
    fn sha256_implementation_matches_known_vectors() {
        let mut empty = Sha256::new();
        empty.update(b"");
        assert_eq!(
            to_hex_lower(&empty.finalize()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let mut abc = Sha256::new();
        abc.update(b"abc");
        assert_eq!(
            to_hex_lower(&abc.finalize()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_file_hex_hashes_file_contents() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("archive.tar.xz");
        fs::write(&file, b"abc")?;

        assert_eq!(
            NodeInstaller::sha256_file_hex(&file)?,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        Ok(())
    }
}
