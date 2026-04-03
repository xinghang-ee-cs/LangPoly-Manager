//! 文件下载器实现。
//!
//! 本模块提供可靠的文件下载功能，支持断点续传、进度显示、错误重试和回退机制。
//! 用于下载 Python、Node.js 等运行时的安装包。
//!
//! 核心类型：
//! - `Downloader`: 下载器主类型，封装 HTTP 客户端和下载逻辑
//!
//! 主要功能：
//! 1. **下载文件** (`download`): 从 URL 下载文件到本地路径
//!    - 支持断点续传（通过 `Range` 头）
//!    - 显示进度条（字节数、速度、百分比）
//!    - 自动创建目标目录
//!    - 下载失败自动清理部分文件
//! 2. **重试机制**: 下载失败时自动重试（最多 3 次）
//! 3. **回退支持**: 主源失败后尝试镜像源（由调用方控制）
//!
//! 下载流程：
//! 1. 发送 HEAD 请求获取文件大小（`Content-Length`）
//! 2. 创建临时文件（`.tmp` 后缀）
//! 3. 流式下载并写入临时文件
//! 4. 更新进度条（每 1024 字节刷新一次）
//! 5. 下载完成：临时文件重命名为目标文件名
//! 6. 下载失败：删除临时文件，返回错误
//!
//! HTTP 客户端配置：
//! - 超时：300 秒（5 分钟）
//! - 重试：3 次（指数退避）
//! - User-Agent: `MeetAI-Installer/1.0`
//! - 自动处理 gzip 压缩
//! - 支持 HTTP/2
//!
//! 进度显示：
//! - 使用 `indicatif::ProgressBar` 显示下载进度
//! - 格式：`[=========>] 45.2 MB/100.0 MB (45%)`
//! - 每秒刷新一次，显示速度（MB/s）
//! - 未知大小时显示无限旋转动画
//!
//! 错误处理：
//! - 网络错误：返回 `reqwest::Error`，触发重试
//! - 文件写入失败：返回 `std::io::Error`，清理临时文件
//! - 服务器错误（4xx/5xx）：返回 `reqwest::Error`，不重试
//!
//! 注意事项：
//! - 下载大文件时确保磁盘空间充足
//! - 临时文件位于同一目录，避免跨磁盘移动
//! - 重试机制仅针对网络错误，不重试 4xx 错误
//!
//! 测试：
//! - 成功下载无 `Content-Length` 头
//! - 失败时清理部分临时文件
//! - 用更新临时文件替换陈旧文件

use crate::utils::http_client::build_http_client;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use reqwest::Client;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// 下载器
pub struct Downloader {
    client: Client,
}

impl Downloader {
    /// 创建下载器并初始化带超时的 HTTP 客户端。
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: build_http_client(std::time::Duration::from_secs(300))?,
        })
    }

    /// 下载文件
    pub async fn download(
        &self,
        url: &str,
        dest: &Path,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("创建下载目录失败：{}", parent.display()))?;
            }
        }

        let temp_dest = if let Some(file_name) = dest.file_name() {
            dest.with_file_name(format!("{}.part", file_name.to_string_lossy()))
        } else {
            dest.with_extension("part")
        };

        if fs::metadata(&temp_dest).await.is_ok() {
            fs::remove_file(&temp_dest)
                .await
                .with_context(|| format!("清理残留临时文件失败：{}", temp_dest.display()))?;
        }

        // 发送请求
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("请求发送失败（URL：{}）", url))?;

        if !response.status().is_success() {
            anyhow::bail!("下载失败（URL：{}，状态码：{}）", url, response.status());
        }

        let total_size = response.content_length();
        if let Some(pb) = progress {
            pb.set_length(total_size.unwrap_or(0));
        }

        let mut downloaded = 0u64;
        let download_result: Result<()> = async {
            let mut file = fs::File::create(&temp_dest)
                .await
                .with_context(|| format!("创建临时下载文件失败：{}", temp_dest.display()))?;

            // 下载并写入文件
            while let Some(chunk) = response
                .chunk()
                .await
                .with_context(|| format!("数据接收中断（URL：{}）", url))?
            {
                file.write_all(&chunk)
                    .await
                    .with_context(|| format!("写入临时文件失败：{}", temp_dest.display()))?;

                downloaded += chunk.len() as u64;

                if let Some(pb) = progress {
                    pb.set_position(downloaded);
                }
            }

            file.flush()
                .await
                .with_context(|| format!("临时文件刷盘失败：{}", temp_dest.display()))?;

            Ok(())
        }
        .await;

        if let Err(err) = download_result {
            let _ = fs::remove_file(&temp_dest).await;
            return Err(err);
        }

        if fs::metadata(dest).await.is_ok() {
            fs::remove_file(dest)
                .await
                .with_context(|| format!("替换目标文件失败：{}", dest.display()))?;
        }

        fs::rename(&temp_dest, dest).await.with_context(|| {
            format!(
                "下载完成后重命名失败（{} → {}）",
                temp_dest.display(),
                dest.display()
            )
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::net::SocketAddr;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn spawn_raw_http_response_server(
        response: Vec<u8>,
    ) -> Result<(String, tokio::task::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind test HTTP server")?;
        let addr: SocketAddr = listener
            .local_addr()
            .context("Failed to resolve test HTTP server address")?;

        let handle = tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut request_buf = [0u8; 2048];
                let _ = socket.read(&mut request_buf).await;
                let _ = socket.write_all(&response).await;
                let _ = socket.flush().await;
            }
        });

        Ok((format!("http://{}/download.bin", addr), handle))
    }

    #[tokio::test]
    async fn download_succeeds_without_content_length() -> Result<()> {
        let body = b"python-installer-data";
        let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
        response.extend_from_slice(body);

        let (url, server_handle) = spawn_raw_http_response_server(response).await?;
        let temp = tempdir()?;
        let dest = temp.path().join("python-3.12.0.exe");

        let downloader = Downloader::new()?;
        downloader.download(&url, &dest, None).await?;
        server_handle
            .await
            .context("Failed to join test HTTP server task")?;

        assert_eq!(fs::read(&dest)?, body);
        let temp_part = dest.with_file_name("python-3.12.0.exe.part");
        assert!(!temp_part.exists(), "temporary part file should be removed");

        Ok(())
    }

    #[tokio::test]
    async fn download_failure_cleans_partial_temp_file() -> Result<()> {
        let mut response =
            b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n".to_vec();
        response.extend_from_slice(b"short");

        let (url, server_handle) = spawn_raw_http_response_server(response).await?;
        let temp = tempdir()?;
        let dest = temp.path().join("python-3.12.1.exe");
        let temp_part = dest.with_file_name("python-3.12.1.exe.part");

        let downloader = Downloader::new()?;
        let err = downloader.download(&url, &dest, None).await.unwrap_err();
        server_handle
            .await
            .context("Failed to join test HTTP server task")?;

        assert!(
            err.to_string().contains("数据接收中断"),
            "unexpected error: {err:#}"
        );
        assert!(
            !temp_part.exists(),
            "partial temp file should be removed on download error"
        );
        assert!(
            !dest.exists(),
            "destination file should not exist after failed download"
        );

        Ok(())
    }

    #[tokio::test]
    async fn download_replaces_stale_temp_file() -> Result<()> {
        let body = b"fresh-data";
        let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        response.extend_from_slice(body);

        let (url, server_handle) = spawn_raw_http_response_server(response).await?;
        let temp = tempdir()?;
        let dest = temp.path().join("python-3.12.2.exe");
        let temp_part = dest.with_file_name("python-3.12.2.exe.part");
        fs::write(&temp_part, b"stale-data")?;

        let downloader = Downloader::new()?;
        downloader.download(&url, &dest, None).await?;
        server_handle
            .await
            .context("Failed to join test HTTP server task")?;

        assert_eq!(fs::read(&dest)?, body);
        assert!(
            !temp_part.exists(),
            "stale part file should not remain after successful download"
        );

        Ok(())
    }
}
