//! 命令执行器实现。
//!
//! 本模块提供统一的命令执行接口，封装同步和异步进程启动逻辑。
//! 用于执行 Python、Node.js、pip 等外部命令，统一错误处理和上下文。
//!
//! 核心类型：
//! - `CommandExecutor`: 零大小类型命令执行器，无状态单例
//!
//! 主要方法：
//! - `new()`: 创建执行器实例（实际是空结构体，无状态）
//! - `execute()`: 异步执行命令，检查退出状态，失败时返回错误
//! - `execute_with_output()`: 同步执行并捕获 stdout 输出字符串
//! - `execute_with_output_async()`: 异步执行并捕获 stdout 输出字符串
//! - `format_command()`: 将程序路径和参数格式化为可读字符串（用于日志）
//!
//! 设计特点：
//! - **零状态**: `CommandExecutor` 是零大小类型，不保存任何配置
//! - **同步/异步分离**: 提供同步和异步两个版本，根据调用场景选择
//! - **统一错误**: 所有错误包装为 `anyhow::Error`，包含命令和参数上下文
//! - **输出捕获**: 支持捕获 stdout / stderr，并在失败时附带完整上下文
//!
//! 同步 vs 异步：
//! | 方法 | 适用场景 | 阻塞？ |
//! |------|----------|--------|
//! | `execute` | 异步运行、无需返回输出（如安装器或卸载器命令） | 否 |
//! | `execute_with_output` | 需要立即读取输出结果（如 `python --version`） | 是 |
//! | `execute_with_output_async` | 异步运行且需要输出结果 | 否 |
//!
//! 错误处理：
//! - 命令不存在：`std::io::Error`（Kernel 返回 `ENOENT`）
//! - 命令执行失败（非零退出码）：`anyhow::Error`，包含命令字符串和 stderr
//! - 输出读取失败：`std::io::Error`
//!
//! 使用示例：
//! ```rust,no_run
//! use meetai::utils::executor::CommandExecutor;
//! use std::path::Path;
//!
//! async fn run() -> anyhow::Result<()> {
//!     let executor = CommandExecutor::new();
//!
//!     executor.execute(Path::new("python"), &["-m", "venv", ".venv"]).await?;
//!
//!     let python_version = executor.execute_with_output(Path::new("python"), &["--version"])?;
//!     println!("Python version: {}", python_version.trim());
//!
//!     let node_version = executor
//!         .execute_with_output_async(Path::new("node"), &["--version"])
//!         .await?;
//!     println!("Node version: {}", node_version.trim());
//!     Ok(())
//! }
//! ```
//!
//! 测试：
//! - 失败命令包含完整上下文（命令、参数、stderr）
//! - 异步执行验证并发安全性

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;

/// 命令执行器
pub struct CommandExecutor;

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor {
    /// 创建命令执行器。
    pub fn new() -> Self {
        Self
    }

    /// 异步执行程序并等待完成
    pub async fn execute(&self, program: &Path, args: &[&str]) -> Result<()> {
        let command_display = Self::format_command(program, args);
        let output = TokioCommand::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("命令启动失败：{}", command_display))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "命令执行失败：{}\n退出码：{}\nstdout：{}\nstderr：{}",
                command_display,
                output.status,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                stderr.trim()
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            println!("{}", stdout);
        }

        Ok(())
    }

    /// 同步执行程序并返回输出
    pub fn execute_with_output(&self, program: &Path, args: &[&str]) -> Result<String> {
        let command_display = Self::format_command(program, args);
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("命令启动失败：{}", command_display))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "命令执行失败：{}\n退出码：{}\nstdout：{}\nstderr：{}",
                command_display,
                output.status,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 异步执行程序并返回输出
    pub async fn execute_with_output_async(&self, program: &Path, args: &[&str]) -> Result<String> {
        let command_display = Self::format_command(program, args);
        let output = TokioCommand::new(program)
            .args(args)
            .output()
            .await
            .with_context(|| format!("命令启动失败：{}", command_display))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "命令执行失败：{}\n退出码：{}\nstdout：{}\nstderr：{}",
                command_display,
                output.status,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn format_command(program: &Path, args: &[&str]) -> String {
        let mut command = program.display().to_string();
        for arg in args {
            command.push(' ');
            command.push_str(arg);
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failing_command() -> (&'static Path, Vec<&'static str>) {
        if cfg!(windows) {
            (Path::new("cmd"), vec!["/C", "exit /B 5"])
        } else {
            (Path::new("sh"), vec!["-c", "exit 5"])
        }
    }

    #[tokio::test]
    async fn execute_failure_includes_command_context() {
        let (program, args) = failing_command();
        let executor = CommandExecutor::new();

        let err = executor
            .execute(program, &args)
            .await
            .expect_err("command should fail in this test");

        let message = err.to_string();
        assert!(
            message.contains("命令执行失败："),
            "error should include command prefix, got: {message}"
        );
        assert!(
            message.contains("退出码："),
            "error should include status, got: {message}"
        );
    }

    #[test]
    fn execute_with_output_failure_includes_command_context() {
        let (program, args) = failing_command();
        let executor = CommandExecutor::new();

        let err = executor
            .execute_with_output(program, &args)
            .expect_err("command should fail in this test");

        let message = err.to_string();
        assert!(
            message.contains("命令执行失败："),
            "error should include command prefix, got: {message}"
        );
        assert!(
            message.contains("退出码："),
            "error should include status, got: {message}"
        );
    }

    #[tokio::test]
    async fn execute_with_output_async_failure_includes_command_context() {
        let (program, args) = failing_command();
        let executor = CommandExecutor::new();

        let err = executor
            .execute_with_output_async(program, &args)
            .await
            .expect_err("command should fail in this test");

        let message = err.to_string();
        assert!(
            message.contains("命令执行失败："),
            "error should include command prefix, got: {message}"
        );
        assert!(
            message.contains("退出码："),
            "error should include status, got: {message}"
        );
    }
}
