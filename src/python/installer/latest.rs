use super::*;

impl PythonInstaller {
    pub(super) fn is_prerelease_version(version: &str) -> bool {
        version
            .chars()
            .any(|ch| !(ch.is_ascii_digit() || ch == '.'))
    }

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

    pub(super) async fn fetch_latest_downloadable_from_ftp_index(&self) -> Result<Option<String>> {
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

    pub(super) async fn is_official_installer_available(&self, version: &str) -> bool {
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
