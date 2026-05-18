use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;

/// Shared command runner with consistent error context and optional environment injection.
pub struct CommandExecutor;

impl Default for CommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor {
    /// Create a stateless command executor.
    pub fn new() -> Self {
        Self
    }

    /// Run an async command and print non-empty stdout.
    pub async fn execute(&self, program: &Path, args: &[&str]) -> Result<()> {
        self.execute_with_env(program, args, &[]).await
    }

    /// Run an async command with additional environment variables.
    pub async fn execute_with_env(
        &self,
        program: &Path,
        args: &[&str],
        envs: &[(&str, &str)],
    ) -> Result<()> {
        let command_display = Self::format_command(program, args);
        let mut command = TokioCommand::new(program);
        command.args(args);
        for (key, value) in envs {
            command.env(key, value);
        }
        let output = command
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

    /// Run a command synchronously and return stdout.
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

    /// Run an async command and return stdout.
    pub async fn execute_with_output_async(&self, program: &Path, args: &[&str]) -> Result<String> {
        self.execute_with_output_async_env(program, args, &[]).await
    }

    /// Run an async command with additional environment variables and return stdout.
    pub async fn execute_with_output_async_env(
        &self,
        program: &Path,
        args: &[&str],
        envs: &[(&str, &str)],
    ) -> Result<String> {
        let command_display = Self::format_command(program, args);
        let mut command = TokioCommand::new(program);
        command.args(args);
        for (key, value) in envs {
            command.env(key, value);
        }
        let output = command
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
        assert!(message.contains("命令执行失败："));
        assert!(message.contains("退出码："));
    }

    #[test]
    fn execute_with_output_failure_includes_command_context() {
        let (program, args) = failing_command();
        let executor = CommandExecutor::new();

        let err = executor
            .execute_with_output(program, &args)
            .expect_err("command should fail in this test");

        let message = err.to_string();
        assert!(message.contains("命令执行失败："));
        assert!(message.contains("退出码："));
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
        assert!(message.contains("命令执行失败："));
        assert!(message.contains("退出码："));
    }

    #[tokio::test]
    async fn execute_with_output_async_env_passes_environment_variables() -> Result<()> {
        let executor = CommandExecutor::new();
        let output = if cfg!(windows) {
            executor
                .execute_with_output_async_env(
                    Path::new("cmd"),
                    &["/C", "echo %MEETAI_EXECUTOR_TEST%"],
                    &[("MEETAI_EXECUTOR_TEST", "from-env")],
                )
                .await?
        } else {
            executor
                .execute_with_output_async_env(
                    Path::new("sh"),
                    &["-c", "printf '%s' \"$MEETAI_EXECUTOR_TEST\""],
                    &[("MEETAI_EXECUTOR_TEST", "from-env")],
                )
                .await?
        };

        assert_eq!(output.trim(), "from-env");
        Ok(())
    }
}
