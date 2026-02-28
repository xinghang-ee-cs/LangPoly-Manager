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
