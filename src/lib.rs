//! MeetAI 多语言开发环境管理工具的核心库。
//!
//! 本 crate 提供统一的运行时管理能力，支持 Python、Node.js、Java、Go 等
//! 编程语言的版本管理、安装、卸载和激活功能。通过模块化设计，将通用逻辑
//! 抽象到共享层，同时保留各运行时的特有实现。
//!
//! # 主要模块
//!
//! - `cli`: 命令行参数解析与命令定义
//! - `config`: 应用配置持久化与目录管理
//! - `python`: Python 安装、版本管理与虚拟环境
//! - `node`: Node.js 版本管理与项目集成
//! - `pip`: Python 包管理
//! - `runtime`: 统一运行时抽象与命令分发
//! - `quick_install`: 一键环境初始化流程
//! - `utils`: 共享工具（下载器、执行器、验证器等）
//!
//! # 架构设计
//!
//! 项目采用分层架构：
//! 1. **CLI 层**：解析用户命令，调用对应的 handler
//! 2. **Service 层**：各运行时的领域服务，封装业务逻辑
//! 3. **Runtime 层**：共享的版本管理、安装、卸载抽象
//! 4. **Utils 层**：通用工具组件
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use meetai::python::PythonService;
//!
//! let service = PythonService::new()?;
//! service.install("3.11.0").await?;
//! service.set_current_version("3.11.0")?;
//! let versions = service.list_installed()?;
//! ```

/// CLI argument schema and command types.
pub mod cli;
/// Persistent app configuration and directory policy.
pub mod config;
/// Node.js install and version management.
pub mod node;
/// Pip package and version management.
pub mod pip;
/// Python install, version, and venv management.
pub mod python;
/// One-command environment bootstrap flow.
pub mod quick_install;
/// Unified runtime command handlers.
pub mod runtime;
/// Shared utilities (downloader, executor, progress, validator).
pub mod utils;
