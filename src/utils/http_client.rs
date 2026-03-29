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
