//! HTTP 客户端工厂模块。
//!
//! 本模块提供统一的 HTTP 客户端构建函数，配置超时、User-Agent、协议版本等通用参数。
//! 所有模块共享同一套客户端配置，确保一致的行为和资源复用。
//!
//! 主要函数：
//! - `build_http_client`: 构建配置完成的 `reqwest::Client` 实例
//!
//! 配置参数：
//! | 参数 | 值 | 说明 |
//! |------|-----|------|
//! | 总超时 | 调用方传入 | 例如下载场景常用 300 秒 |
//! | 连接超时 | 30 秒 | TCP 握手超时 |
//! | 协议版本 | HTTP/1.1 only | 避免 Windows 下 HTTP/2 TLS 兼容问题 |
//! | User-Agent | `meetai/<cargo_pkg_version>` | 标识客户端身份 |
//!
//! 设计考虑：
//! - **HTTP/1.1 强制**: 某些 Windows 环境（特别是国内杀毒软件/防火墙）对 HTTP/2 ALPN 协商支持不佳，强制 HTTP/1.1 提高兼容性
//! - **长超时**: 下载大文件（如 Python 安装包 ~50MB）需要足够时间
//! - **连接池**: `reqwest::Client` 内部维护连接池，建议全局复用而非每次创建
//! - **无重试**: 重试逻辑由调用方（如 `Downloader`）控制，避免客户端层隐藏错误
//!
//! 使用示例：
//! ```rust,no_run
//! use meetai::utils::http_client::build_http_client;
//! use std::time::Duration;
//!
//! async fn fetch() -> anyhow::Result<()> {
//!     let client = build_http_client(Duration::from_secs(300))?;
//!     let response = client
//!         .get("https://nodejs.org/dist/index.json")
//!         .send()
//!         .await?;
//!     println!("status = {}", response.status());
//!     Ok(())
//! }
//! ```
//!
//! 错误处理：
//! - 客户端构建失败（TLS 配置等）：返回 `anyhow::Error`
//! - 实际请求错误由 `reqwest::Error` 传播，调用方决定是否重试
//!
//! 平台差异：
//! - Windows: 使用 `schannel` 或 `openssl` 作为 TLS 后端（由 `reqwest` 特性决定）
//! - Unix/macOS: 使用系统 `openssl` 或 `rustls`（推荐 `rustls` 特性）
//!
//! 测试：
//! - 客户端构建成功
//! - User-Agent 格式正确
//! - 超时配置生效

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_USER_AGENT: &str = concat!("meetai/", env!("CARGO_PKG_VERSION"));

/// 构建共享 HTTP 客户端。
///
/// 当前固定使用 HTTP/1.1，以降低部分 Windows 环境下对特定站点的 TLS/ALPN 兼容风险。
pub fn build_http_client(timeout: Duration) -> Result<Client> {
    Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
        .http1_only()
        .user_agent(DEFAULT_USER_AGENT)
        .build()
        .context("HTTP 客户端初始化失败")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn build_http_client_uses_http1_and_default_user_agent() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind test HTTP server")?;
        let addr = listener
            .local_addr()
            .context("Failed to resolve test HTTP server address")?;

        let server_handle = tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .context("Failed to accept test HTTP connection")?;
            let mut request_buf = [0u8; 4096];
            let size = socket
                .read(&mut request_buf)
                .await
                .context("Failed to read test HTTP request")?;

            socket
                .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
                .await
                .context("Failed to write test HTTP response")?;
            socket
                .flush()
                .await
                .context("Failed to flush test HTTP response")?;

            Ok::<String, anyhow::Error>(String::from_utf8_lossy(&request_buf[..size]).into_owned())
        });

        let client = build_http_client(Duration::from_secs(5))?;
        client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .context("Failed to send request with shared HTTP client")?
            .error_for_status()
            .context("Shared HTTP client request returned error status")?;

        let request = server_handle
            .await
            .context("Failed to join test HTTP server task")??;
        let request_lowercase = request.to_ascii_lowercase();

        assert!(
            request.starts_with("GET /health HTTP/1.1\r\n"),
            "expected HTTP/1.1 request, got: {request:?}"
        );
        assert!(
            request_lowercase.contains(&format!(
                "user-agent: {}\r\n",
                DEFAULT_USER_AGENT.to_ascii_lowercase()
            )),
            "expected default user agent header, got: {request:?}"
        );

        Ok(())
    }
}
