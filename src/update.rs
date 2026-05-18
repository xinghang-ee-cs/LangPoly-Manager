use crate::cli::{UpdateAction, UpdateArgs};
use crate::utils::downloader::Downloader;
use crate::utils::http_client::build_http_client;
use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::Duration;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/meetai-club/meetai/releases/latest";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub async fn handle_update_command(args: UpdateArgs) -> Result<()> {
    let updater = SelfUpdater::new()?;
    match args.action {
        Some(UpdateAction::Check) => updater.check().await,
        None => updater.update().await,
    }
}

struct SelfUpdater {
    client: reqwest::Client,
}

impl SelfUpdater {
    fn new() -> Result<Self> {
        Ok(Self {
            client: build_http_client(Duration::from_secs(30))?,
        })
    }

    async fn check(&self) -> Result<()> {
        let release = self.fetch_latest_release().await?;
        if self.is_newer(&release.tag_name)? {
            println!(
                "发现新版本：{}（当前 {}）",
                release.tag_name,
                env!("CARGO_PKG_VERSION")
            );
        } else {
            println!("MeetAI 已是最新版本：{}", env!("CARGO_PKG_VERSION"));
        }
        Ok(())
    }

    async fn update(&self) -> Result<()> {
        let release = self.fetch_latest_release().await?;
        if !self.is_newer(&release.tag_name)? {
            println!("MeetAI 已是最新版本：{}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }

        let asset_name = platform_asset_name()?;
        let asset = find_asset(&release, asset_name)?;
        let sha_asset = find_asset(&release, &format!("{asset_name}.sha256"))?;

        let current_exe = std::env::current_exe().context("无法确定当前 meetai 可执行文件路径")?;
        let temp_dir = std::env::temp_dir().join(format!("meetai-update-{}", release.tag_name));
        fs::create_dir_all(&temp_dir)?;
        let download_path = temp_dir.join(asset_name);
        let sha_path = temp_dir.join(format!("{asset_name}.sha256"));

        let downloader = Downloader::new()?;
        downloader
            .download(&asset.browser_download_url, &download_path, None)
            .await?;
        downloader
            .download(&sha_asset.browser_download_url, &sha_path, None)
            .await?;

        verify_sha256(&download_path, &sha_path)?;
        replace_current_executable(&current_exe, &download_path)?;
        println!("MeetAI 已更新到 {}。", release.tag_name);
        Ok(())
    }

    async fn fetch_latest_release(&self) -> Result<GithubRelease> {
        self.client
            .get(LATEST_RELEASE_API)
            .send()
            .await
            .context("请求 GitHub Releases 失败")?
            .error_for_status()
            .context("GitHub Releases 返回错误状态")?
            .json::<GithubRelease>()
            .await
            .context("解析 GitHub Releases 响应失败")
    }

    fn is_newer(&self, tag: &str) -> Result<bool> {
        let remote = Version::parse(tag.trim_start_matches('v'))
            .with_context(|| format!("远端版本号格式不正确：{tag}"))?;
        let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
        Ok(remote > current)
    }
}

fn platform_asset_name() -> Result<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Ok("meetai.exe")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok("meetai")
    } else {
        anyhow::bail!("当前平台暂不支持自动更新，请从 GitHub Releases 手动下载。")
    }
}

fn find_asset<'a>(release: &'a GithubRelease, name: &str) -> Result<&'a GithubAsset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .with_context(|| format!("Release {} 未找到资产：{}", release.tag_name, name))
}

fn verify_sha256(file: &Path, sha_file: &Path) -> Result<()> {
    let sha_content = fs::read_to_string(sha_file)
        .with_context(|| format!("读取 sha256 文件失败：{}", sha_file.display()))?;
    let expected = sha_content
        .split_whitespace()
        .next()
        .context("sha256 文件内容为空")?
        .to_ascii_lowercase();
    let actual = sha256_file(file)?;
    if actual != expected {
        anyhow::bail!(
            "sha256 校验失败：expected {}, actual {}。已保留当前版本。",
            expected,
            actual
        );
    }
    Ok(())
}

fn sha256_file(file: &Path) -> Result<String> {
    let bytes = fs::read(file).with_context(|| format!("读取文件失败：{}", file.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(unix)]
fn replace_current_executable(current: &Path, downloaded: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::metadata(current)
        .or_else(|_| fs::metadata(downloaded))
        .context("读取可执行文件权限失败")?
        .permissions()
        .mode();
    fs::set_permissions(downloaded, fs::Permissions::from_mode(perms | 0o755))?;
    fs::rename(downloaded, current)
        .with_context(|| format!("替换当前可执行文件失败：{}", current.display()))
}

#[cfg(windows)]
fn replace_current_executable(current: &Path, downloaded: &Path) -> Result<()> {
    let staged = current.with_extension("exe.new");
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    fs::copy(downloaded, &staged)?;

    match fs::rename(&staged, current) {
        Ok(()) => Ok(()),
        Err(_) => {
            let script = current.with_extension("update.cmd");
            let script_content = format!(
                "@echo off\r\nping 127.0.0.1 -n 2 >nul\r\ncopy /Y \"{}\" \"{}\" >nul\r\ndel \"{}\" >nul 2>nul\r\ndel \"%~f0\" >nul 2>nul\r\n",
                staged.display(),
                current.display(),
                staged.display()
            );
            fs::write(&script, script_content)?;
            println!(
                "当前 meetai.exe 正在运行，已准备延迟替换脚本：{}。关闭当前命令后运行该脚本完成更新。",
                script.display()
            );
            Ok(())
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_current_executable(_current: &Path, _downloaded: &Path) -> Result<()> {
    anyhow::bail!("当前平台暂不支持自动替换可执行文件")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn verify_sha256_accepts_release_format() -> Result<()> {
        let temp = tempdir()?;
        let file = temp.path().join("meetai");
        let sha = temp.path().join("meetai.sha256");
        fs::write(&file, b"abc")?;
        fs::write(
            &sha,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *meetai",
        )?;
        verify_sha256(&file, &sha)
    }

    #[test]
    fn is_newer_compares_release_tag_with_current_version() -> Result<()> {
        let updater = SelfUpdater::new()?;

        assert!(updater.is_newer("v999.0.0")?);
        assert!(!updater.is_newer(env!("CARGO_PKG_VERSION"))?);
        assert!(!updater.is_newer("0.0.1")?);
        Ok(())
    }

    #[test]
    fn find_asset_returns_matching_release_asset() -> Result<()> {
        let release = GithubRelease {
            tag_name: "v1.0.0".to_string(),
            assets: vec![
                GithubAsset {
                    name: "meetai.exe".to_string(),
                    browser_download_url: "https://example.com/meetai.exe".to_string(),
                },
                GithubAsset {
                    name: "meetai.exe.sha256".to_string(),
                    browser_download_url: "https://example.com/meetai.exe.sha256".to_string(),
                },
            ],
        };

        let asset = find_asset(&release, "meetai.exe.sha256")?;

        assert_eq!(
            asset.browser_download_url,
            "https://example.com/meetai.exe.sha256"
        );
        Ok(())
    }

    #[test]
    fn find_asset_reports_missing_release_asset() {
        let release = GithubRelease {
            tag_name: "v1.0.0".to_string(),
            assets: Vec::new(),
        };

        let err = find_asset(&release, "meetai").expect_err("asset should be missing");

        assert!(err.to_string().contains("未找到资产"));
    }

    #[test]
    fn platform_asset_name_matches_supported_targets() -> Result<()> {
        let asset_name = platform_asset_name()?;

        if cfg!(windows) {
            assert_eq!(asset_name, "meetai.exe");
        } else if cfg!(target_os = "linux") {
            assert_eq!(asset_name, "meetai");
        }

        Ok(())
    }
}
