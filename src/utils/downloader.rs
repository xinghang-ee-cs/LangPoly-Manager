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
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
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
                fs::create_dir_all(parent).await.with_context(|| {
                    format!("Failed to create download directory: {}", parent.display())
                })?;
            }
        }

        let temp_dest = if let Some(file_name) = dest.file_name() {
            dest.with_file_name(format!("{}.part", file_name.to_string_lossy()))
        } else {
            dest.with_extension("part")
        };

        if fs::metadata(&temp_dest).await.is_ok() {
            fs::remove_file(&temp_dest).await.with_context(|| {
                format!(
                    "Failed to remove stale temp file before download: {}",
                    temp_dest.display()
                )
            })?;
        }

        // 发送请求
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to send request for URL: {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Download failed for URL {} with status: {}",
                url,
                response.status()
            );
        }

        let total_size = response.content_length();
        if let Some(pb) = progress {
            pb.set_length(total_size.unwrap_or(0));
        }

        let mut downloaded = 0u64;
        let download_result: Result<()> = async {
            let mut file = fs::File::create(&temp_dest).await.with_context(|| {
                format!(
                    "Failed to create temp download file: {}",
                    temp_dest.display()
                )
            })?;

            // 下载并写入文件
            while let Some(chunk) = response
                .chunk()
                .await
                .with_context(|| format!("Failed to download chunk from URL: {}", url))?
            {
                file.write_all(&chunk).await.with_context(|| {
                    format!(
                        "Failed to write chunk to temp file: {}",
                        temp_dest.display()
                    )
                })?;

                downloaded += chunk.len() as u64;

                if let Some(pb) = progress {
                    pb.set_position(downloaded);
                }
            }

            file.flush()
                .await
                .with_context(|| format!("Failed to flush temp file: {}", temp_dest.display()))?;

            Ok(())
        }
        .await;

        if let Err(err) = download_result {
            let _ = fs::remove_file(&temp_dest).await;
            return Err(err);
        }

        if fs::metadata(dest).await.is_ok() {
            fs::remove_file(dest).await.with_context(|| {
                format!(
                    "Failed to replace existing destination file: {}",
                    dest.display()
                )
            })?;
        }

        fs::rename(&temp_dest, dest).await.with_context(|| {
            format!(
                "Failed to finalize downloaded file from {} to {}",
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
            err.to_string()
                .contains("Failed to download chunk from URL"),
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
