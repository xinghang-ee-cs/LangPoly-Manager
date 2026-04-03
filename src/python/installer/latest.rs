//! Python 安装器的版本解析逻辑。
//!
//! 本模块负责解析用户请求的 Python 版本字符串，特别是 `"latest"` 特殊值。
//! 支持从 Python 官网和镜像源获取最新稳定版本信息。
//!
//! 主要函数：
//! - `resolve_target_version`: 解析用户指定的版本（直接返回或解析 latest）
//! - `resolve_latest_python_version`: 从官网下载页面解析最新稳定版
//! - `fetch_latest_from_downloads_page`: 从官网下载页面解析最新稳定版
//! - `fetch_latest_downloadable_from_ftp_index`: 从 FTP 索引中查找带官方安装包的版本
//! - `parse_latest_from_downloads_body`: 从 HTML 提取最新稳定版
//! - `parse_stable_versions_from_ftp_index_body`: 从 FTP 索引提取所有稳定版本
//! - `choose_latest_python_version`: 比较远程、FTP 和本地版本，选择最优值
//!
//! 版本解析策略：
//! 1. **精确版本** (如 `"3.11.5"`): 直接使用，不进行网络请求
//! 2. **"latest"**: 下载 Python 官网下载页面，解析最新稳定版
//! 3. **回退机制**: 官网失败时尝试 FTP 索引、本地已安装版本和内置默认版本
//!
//! HTML 解析逻辑：
//! - 查找下载按钮的文本内容（如 "Download Python 3.11.5"）
//! - 支持 prerelease 标记过滤（跳过 `a1`、`b1`、`rc1` 等）
//! - 使用 semver 库进行版本号比较
//!
//! 网络请求：
//! - 使用 `reqwest` 库发送 HTTP GET 请求
//! - 设置 User-Agent 模拟浏览器
//! - 超时时间 30 秒
//!
//! 错误处理：
//! - 网络失败：返回 `reqwest::Error`，触发回退逻辑
//! - HTML 解析失败：返回 `anyhow::Error`，提示手动指定版本
//! - 版本比较失败：返回 `semver::Error`
//!
//! 测试：
//! - 解析函数支持截断 stdout（测试 HTML 片段）
//! - 版本选择逻辑验证远程/本地优先级
//! - 回退逻辑验证镜像源使用

use super::*;

impl PythonInstaller {
    pub(super) async fn resolve_target_version(&self, requested_version: &str) -> Result<String> {
        if requested_version != "latest" {
            return Ok(requested_version.to_string());
        }

        println!("正在解析 latest 对应的 Python 稳定版本...");
        let resolved = self.resolve_latest_python_version().await?;
        println!("已解析 latest -> Python {}", resolved);
        Ok(resolved)
    }

    pub(super) async fn resolve_latest_python_version(&self) -> Result<String> {
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
                println!(
                    "无法在线获取最新版本，回退到 MeetAI 已管理版本: {}",
                    version
                );
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

    pub(super) fn choose_latest_python_version(
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

    pub(super) fn parse_latest_from_downloads_body(body: &str) -> Result<String> {
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
    pub(super) fn parse_latest_from_ftp_index_body(body: &str) -> Result<String> {
        let versions = Self::parse_stable_versions_from_ftp_index_body(body)?;
        versions
            .into_iter()
            .next()
            .map(|v| v.to_string())
            .context("No stable Python versions found in FTP index")
    }

    pub(super) fn parse_stable_versions_from_ftp_index_body(body: &str) -> Result<Vec<Version>> {
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

    pub(super) async fn fetch_latest_from_downloads_page(&self) -> Result<String> {
        let client =
            crate::utils::http_client::build_http_client(std::time::Duration::from_secs(30))?;
        let body = client
            .get(PYTHON_DOWNLOADS_URL)
            .send()
            .await
            .context("Failed to request Python downloads page")?
            .error_for_status()
            .context("Python downloads page returned non-success status")?
            .text()
            .await
            .context("Failed to read Python downloads page body")?;

        Self::parse_latest_from_downloads_body(&body)
    }

    pub(super) async fn fetch_latest_downloadable_from_ftp_index(&self) -> Result<Option<String>> {
        let client =
            crate::utils::http_client::build_http_client(std::time::Duration::from_secs(30))?;
        let body = client
            .get(PYTHON_FTP_INDEX_URL)
            .send()
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

    pub(super) async fn is_official_installer_available(&self, version: &str) -> bool {
        let Ok(url) = self.build_official_download_url(version) else {
            return false;
        };

        let client =
            crate::utils::http_client::build_http_client(std::time::Duration::from_secs(20));
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

    pub(super) fn get_latest_installed_python_version(&self) -> Result<Option<String>> {
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
            // 仅将“可执行文件存在”的版本视为可用版本，避免残缺目录被误判为 latest 候选。
            if !self.get_python_exe(version_str).exists() {
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
}
