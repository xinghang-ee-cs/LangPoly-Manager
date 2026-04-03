//! 工具函数集合。
//!
//! 本模块汇集项目中各层共享的通用工具，避免代码重复。
//!
//! 子模块：
//! - `downloader`: 文件下载器，支持断点续传、回退机制、进度显示
//! - `executor`: 命令执行器，封装 `std::process::Command` 和 `tokio::process::Command`
//! - `guidance`: 用户指导信息，提供网络诊断、PATH 配置建议等友好提示
//! - `http_client`: HTTP 客户端工厂，配置超时、协议版本、User-Agent
//! - `progress`: 进度条样式定义，提供 "🌙 月亮" 等主题
//! - `validator`: 输入验证器，验证版本号和包名等输入 token
//!
//! 设计原则：
//! - 所有函数均为 `pub(crate)` 或 `pub`，供内部模块使用
//! - 无状态设计，尽量使用函数而非结构体
//! - 错误处理统一使用 `anyhow::Result` 或 `Result<T, Box<dyn Error>>`
//! - 平台差异通过 `cfg` 条件编译处理
//!
//! 使用示例：
//! ```rust,no_run
//! use meetai::utils::downloader::Downloader;
//! use meetai::utils::executor::CommandExecutor;
//! use meetai::utils::validator::Validator;
//!
//! let downloader = Downloader::new()?;
//! let executor = CommandExecutor::new();
//! let validator = Validator::new();
//! # let _ = (downloader, executor, validator);
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod downloader;
pub mod executor;
pub mod guidance;
pub mod http_client;
pub mod progress;
pub mod validator;
