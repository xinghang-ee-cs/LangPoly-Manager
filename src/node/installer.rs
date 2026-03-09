use crate::config::Config;
use crate::node::version::NodeVersionManager;
use crate::utils::downloader::Downloader;
use crate::utils::guidance::network_diagnostic_tips;
use crate::utils::validator::Validator;
use anyhow::{Context, Result};
use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command as TokioCommand;

const NODE_DIST_INDEX_URL: &str = "https://nodejs.org/dist/index.json";
const NODE_DIST_BASE_URL: &str = "https://nodejs.org/dist";

#[derive(Debug, Deserialize)]
struct NodeDistRelease {
    version: String,
    #[serde(default)]
    files: Vec<String>,
}

/// Node.js 安装器。
pub struct NodeInstaller {
    config: Config,
    downloader: Downloader,
}

impl NodeInstaller {
    /// 创建 Node.js 安装器。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let installer = Self {
            config,
            downloader: Downloader::new()?,
        };
        installer.ensure_node_dirs()?;
        Ok(installer)
    }

    /// 安装指定 Node.js 版本（支持 `latest` 或具体版本号）。
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

        if !cfg!(windows) {
            anyhow::bail!("{}", Self::unsupported_auto_install_message());
        }

        let platform_token = Self::windows_platform_token()?;
        let archive_path = self
            .config
            .cache_dir
            .join(format!("node-v{}-{}.zip", resolved_version, platform_token));
        let extract_dir = self.config.cache_dir.join(format!(
            "node-v{}-{}-extract",
            resolved_version, platform_token
        ));

        let download_url = self.build_download_url(&resolved_version)?;
        if let Err(err) = self
            .downloader
            .download(&download_url, &archive_path, None)
            .await
        {
            return Err(err.context(format!(
                "Node.js {} 下载失败。\n{}\n可重试命令：\n  - meetai node install {}\n  - meetai runtime install nodejs {}\n{}",
                resolved_version,
                download_url,
                resolved_version,
                resolved_version,
                network_diagnostic_tips()
            )));
        }

        if let Err(err) = self
            .extract_and_install(&archive_path, &extract_dir, &resolved_version)
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
        if version != "latest" {
            return Ok(version.to_string());
        }

        if !cfg!(windows) {
            if let Some(local_latest) = self.get_latest_installed_version()? {
                return Ok(local_latest);
            }
            anyhow::bail!("{}", Self::unsupported_auto_install_message());
        }

        let remote_latest = self.resolve_latest_from_remote().await.ok();
        let local_latest = self.get_latest_installed_version()?;
        Self::choose_latest_version(remote_latest, local_latest).with_context(|| {
            "无法解析 latest 对应的 Node.js 版本，请检查网络后重试，或显式指定版本号（例如 20.11.1）"
        })
    }

    async fn resolve_latest_from_remote(&self) -> Result<String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("HTTP 客户端初始化失败")?;
        let body = client
            .get(NODE_DIST_INDEX_URL)
            .send()
            .await
            .context("请求 Node.js 版本索引失败")?
            .error_for_status()
            .context("Node.js 版本索引响应异常")?
            .text()
            .await
            .context("读取 Node.js 版本索引失败")?;

        self.parse_latest_version_from_index_body(&body)
    }

    fn parse_latest_version_from_index_body(&self, body: &str) -> Result<String> {
        let releases: Vec<NodeDistRelease> =
            serde_json::from_str(body).context("解析 Node.js 版本索引失败")?;
        let required_file_key = format!("{}-zip", Self::windows_platform_token()?);

        let latest = releases
            .into_iter()
            .filter(|item| item.files.iter().any(|f| f == &required_file_key))
            .filter_map(|item| Self::normalize_version_token(&item.version))
            .max();

        latest.context("版本索引中未找到可用于当前平台的 Node.js 包")
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
        let platform = Self::windows_platform_token()?;
        Ok(format!(
            "{}/v{}/node-v{}-{}.zip",
            NODE_DIST_BASE_URL, version, version, platform
        ))
    }

    async fn extract_and_install(
        &self,
        archive_path: &Path,
        extract_dir: &Path,
        version: &str,
    ) -> Result<()> {
        if extract_dir.exists() {
            fs::remove_dir_all(extract_dir)
                .with_context(|| format!("清理历史解压目录失败：{}", extract_dir.display()))?;
        }
        fs::create_dir_all(extract_dir)
            .with_context(|| format!("创建解压目录失败：{}", extract_dir.display()))?;

        self.expand_archive_windows(archive_path, extract_dir)
            .await?;

        let root_dir = self.resolve_extracted_root(extract_dir, version)?;
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

    fn resolve_extracted_root(&self, extract_dir: &Path, version: &str) -> Result<PathBuf> {
        let expected_name = format!("node-v{}-{}", version, Self::windows_platform_token()?);
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
            }
        }
        Ok(())
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
        "当前平台暂不支持自动下载安装 Node.js。可先手动安装后再执行 `meetai node use <version>`。"
    }

    fn windows_platform_token() -> Result<&'static str> {
        if !cfg!(windows) {
            anyhow::bail!("当前仅支持 Windows 自动安装");
        }

        if cfg!(target_arch = "x86_64") {
            Ok("win-x64")
        } else if cfg!(target_arch = "aarch64") {
            Ok("win-arm64")
        } else if cfg!(target_arch = "x86") {
            Ok("win-x86")
        } else {
            anyhow::bail!("不支持的系统架构")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn parse_latest_version_from_index_body_selects_highest_with_required_file() -> Result<()> {
        let body = r#"
        [
          {"version":"v22.5.0","files":["linux-x64","src"]},
          {"version":"v20.11.1","files":["win-x64-zip","linux-x64"]},
          {"version":"v22.3.0","files":["win-x64-zip","linux-x64"]}
        ]
        "#;

        if cfg!(windows) {
            let installer = NodeInstaller::new()?;
            let latest = installer.parse_latest_version_from_index_body(body)?;
            assert_eq!(latest, "22.3.0");
        }

        Ok(())
    }

    #[tokio::test]
    async fn resolve_latest_on_non_windows_returns_local_or_platform_error() -> Result<()> {
        if cfg!(windows) {
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
}
