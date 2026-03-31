use crate::config::Config;
use crate::node::version::NodeVersionManager;
use crate::python::version::PythonVersionManager;
use crate::quick_install::config::QuickInstallConfig;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[async_trait]
trait CommandExecutorOps: Send + Sync {
    async fn execute_with_output_async(&self, program: &Path, args: &[&str]) -> Result<String>;
}

trait PythonVersionOps: Send + Sync {
    fn current_python_executable(&self, missing_selection_message: &'static str)
        -> Result<PathBuf>;
}

trait NodeVersionOps: Send + Sync {
    fn current_node_executable(&self, missing_selection_message: &'static str) -> Result<PathBuf>;
}

trait ConfigLoaderOps: Send + Sync {
    fn load(&self) -> Result<Config>;
}

struct CommandExecutorAdapter {
    inner: CommandExecutor,
}

#[async_trait]
impl CommandExecutorOps for CommandExecutorAdapter {
    async fn execute_with_output_async(&self, program: &Path, args: &[&str]) -> Result<String> {
        CommandExecutor::execute_with_output_async(&self.inner, program, args).await
    }
}

struct PythonVersionManagerAdapter {
    inner: PythonVersionManager,
}

impl PythonVersionOps for PythonVersionManagerAdapter {
    fn current_python_executable(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        PythonVersionManager::current_python_executable(&self.inner, missing_selection_message)
    }
}

struct NodeVersionManagerAdapter {
    inner: NodeVersionManager,
}

impl NodeVersionOps for NodeVersionManagerAdapter {
    fn current_node_executable(&self, missing_selection_message: &'static str) -> Result<PathBuf> {
        NodeVersionManager::current_node_executable(&self.inner, missing_selection_message)
    }
}

struct ConfigLoaderAdapter;

impl ConfigLoaderOps for ConfigLoaderAdapter {
    fn load(&self) -> Result<Config> {
        Config::load()
    }
}

/// 一键安装验证器
pub struct QuickInstallValidator;

impl Default for QuickInstallValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickInstallValidator {
    /// 创建一键安装结果验证器。
    pub fn new() -> Self {
        Self
    }

    /// 验证安装结果
    pub async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()> {
        let python_version_manager = PythonVersionManagerAdapter {
            inner: PythonVersionManager::new()?,
        };
        let node_version_manager = NodeVersionManagerAdapter {
            inner: NodeVersionManager::new()?,
        };
        let executor = CommandExecutorAdapter {
            inner: CommandExecutor::new(),
        };
        let config_loader = ConfigLoaderAdapter;
        self.verify_installation_with(
            config,
            &python_version_manager,
            &node_version_manager,
            &executor,
            &config_loader,
        )
        .await
    }

    async fn verify_installation_with(
        &self,
        config: &QuickInstallConfig,
        python_version_manager: &dyn PythonVersionOps,
        node_version_manager: &dyn NodeVersionOps,
        executor: &dyn CommandExecutorOps,
        config_loader: &dyn ConfigLoaderOps,
    ) -> Result<()> {
        // 验证 Python 版本是否已设置并可执行
        let python_exe = python_version_manager.current_python_executable(
            "安装后未检测到已激活的 Python 版本，请手动执行: meetai runtime use python <version>",
        )?;

        // 验证 pip 是否可用
        executor
            .execute_with_output_async(&python_exe, &["-m", "pip", "--version"])
            .await
            .context(
                "pip 验证失败，Python 已安装但 pip 可能未正确配置，可尝试: python -m ensurepip",
            )?;

        // 验证虚拟环境是否创建成功
        if config.create_venv {
            let app_config = config_loader.load()?;
            let venv_path = app_config.venv_dir.join(&config.venv_name);

            if !venv_path.exists() {
                anyhow::bail!(
                    "虚拟环境未找到：{}，创建过程可能出现了问题，请尝试重新创建",
                    venv_path.display()
                );
            }

            let marker_path = config.target_dir.join(".venv");
            if !marker_path.exists() {
                anyhow::bail!(
                    "虚拟环境标记文件丢失：{}，可尝试重新执行: meetai venv create {}",
                    marker_path.display(),
                    config.venv_name
                );
            }
        }

        // 验证 Node.js（当用户显式请求安装时）
        if config.install_nodejs {
            let node_exe = node_version_manager.current_node_executable(
                "安装后未检测到已激活的 Node.js 版本，请手动执行: meetai runtime use nodejs <version>",
            )?;
            executor
                .execute_with_output_async(&node_exe, &["--version"])
                .await
                .context("Node.js 验证失败，可尝试重新执行: meetai node install <version>")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    struct MockPythonVersionManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl PythonVersionOps for MockPythonVersionManager {
        fn current_python_executable(
            &self,
            _missing_selection_message: &'static str,
        ) -> Result<PathBuf> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_current_executable".to_string());
            Ok(PathBuf::from("python"))
        }
    }

    struct MockNodeVersionManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl NodeVersionOps for MockNodeVersionManager {
        fn current_node_executable(
            &self,
            _missing_selection_message: &'static str,
        ) -> Result<PathBuf> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("node_current_executable".to_string());
            Ok(PathBuf::from("node"))
        }
    }

    struct MockCommandExecutor {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl CommandExecutorOps for MockCommandExecutor {
        async fn execute_with_output_async(&self, program: &Path, args: &[&str]) -> Result<String> {
            self.calls.lock().expect("lock call log").push(format!(
                "exec:{} {}",
                program.display(),
                args.join(" ")
            ));
            Ok(String::new())
        }
    }

    struct MockConfigLoader {
        calls: Arc<Mutex<Vec<String>>>,
        config: Option<Config>,
    }

    impl ConfigLoaderOps for MockConfigLoader {
        fn load(&self) -> Result<Config> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("config_load".to_string());
            self.config
                .clone()
                .context("config loader should not be called")
        }
    }

    fn make_config(install_nodejs: bool) -> QuickInstallConfig {
        QuickInstallConfig {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "default".to_string(),
            create_venv: false,
            target_dir: PathBuf::from("."),
            install_nodejs,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
            auto_activate: true,
        }
    }

    #[tokio::test]
    async fn verify_installation_skips_node_validation_when_node_install_disabled() -> Result<()> {
        let validator = QuickInstallValidator::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let python_version_manager = MockPythonVersionManager {
            calls: calls.clone(),
        };
        let node_version_manager = MockNodeVersionManager {
            calls: calls.clone(),
        };
        let executor = MockCommandExecutor {
            calls: calls.clone(),
        };
        let config_loader = MockConfigLoader {
            calls: calls.clone(),
            config: None,
        };

        validator
            .verify_installation_with(
                &make_config(false),
                &python_version_manager,
                &node_version_manager,
                &executor,
                &config_loader,
            )
            .await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|call| call == "python_current_executable"),
            "python executable lookup should run, calls: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|call| call == "exec:python -m pip --version"),
            "pip validation command should run, calls: {calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call == "node_current_executable"),
            "node executable lookup should be skipped, calls: {calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call == "exec:node --version"),
            "node validation command should be skipped, calls: {calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call == "config_load"),
            "config loader should be skipped when create_venv=false, calls: {calls:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn verify_installation_checks_node_when_node_install_enabled() -> Result<()> {
        let validator = QuickInstallValidator::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let python_version_manager = MockPythonVersionManager {
            calls: calls.clone(),
        };
        let node_version_manager = MockNodeVersionManager {
            calls: calls.clone(),
        };
        let executor = MockCommandExecutor {
            calls: calls.clone(),
        };
        let config_loader = MockConfigLoader {
            calls: calls.clone(),
            config: None,
        };

        validator
            .verify_installation_with(
                &make_config(true),
                &python_version_manager,
                &node_version_manager,
                &executor,
                &config_loader,
            )
            .await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|call| call == "python_current_executable"),
            "python executable lookup should run, calls: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|call| call == "exec:python -m pip --version"),
            "pip validation command should run, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|call| call == "node_current_executable"),
            "node executable lookup should run, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|call| call == "exec:node --version"),
            "node validation command should run, calls: {calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call == "config_load"),
            "config loader should be skipped when create_venv=false, calls: {calls:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn verify_installation_checks_venv_paths_when_create_venv_enabled() -> Result<()> {
        let validator = QuickInstallValidator::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let python_version_manager = MockPythonVersionManager {
            calls: calls.clone(),
        };
        let node_version_manager = MockNodeVersionManager {
            calls: calls.clone(),
        };
        let executor = MockCommandExecutor {
            calls: calls.clone(),
        };

        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        let venv_dir = temp.path().join("venvs");
        std::fs::create_dir_all(target_dir.clone())?;
        std::fs::create_dir_all(venv_dir.join("default"))?;
        std::fs::write(target_dir.join(".venv"), b"default")?;

        let mut config = make_config(false);
        config.create_venv = true;
        config.target_dir = target_dir;

        let config_loader = MockConfigLoader {
            calls: calls.clone(),
            config: Some(Config {
                python_install_dir: temp.path().join("python"),
                venv_dir,
                cache_dir: temp.path().join("cache"),
                current_python_version: None,
            }),
        };

        validator
            .verify_installation_with(
                &config,
                &python_version_manager,
                &node_version_manager,
                &executor,
                &config_loader,
            )
            .await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|call| call == "config_load"),
            "config loader should run when create_venv=true, calls: {calls:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn verify_installation_fails_when_venv_marker_missing() -> Result<()> {
        let validator = QuickInstallValidator::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let python_version_manager = MockPythonVersionManager {
            calls: calls.clone(),
        };
        let node_version_manager = MockNodeVersionManager {
            calls: calls.clone(),
        };
        let executor = MockCommandExecutor {
            calls: calls.clone(),
        };

        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        let venv_dir = temp.path().join("venvs");
        std::fs::create_dir_all(target_dir.clone())?;
        std::fs::create_dir_all(venv_dir.join("default"))?;

        let mut config = make_config(false);
        config.create_venv = true;
        config.target_dir = target_dir;

        let config_loader = MockConfigLoader {
            calls: calls.clone(),
            config: Some(Config {
                python_install_dir: temp.path().join("python"),
                venv_dir,
                cache_dir: temp.path().join("cache"),
                current_python_version: None,
            }),
        };

        let err = validator
            .verify_installation_with(
                &config,
                &python_version_manager,
                &node_version_manager,
                &executor,
                &config_loader,
            )
            .await
            .expect_err("missing .venv marker should fail verification");
        assert!(
            err.to_string().contains("虚拟环境标记文件丢失"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn verify_installation_fails_when_venv_directory_missing() -> Result<()> {
        let validator = QuickInstallValidator::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let python_version_manager = MockPythonVersionManager {
            calls: calls.clone(),
        };
        let node_version_manager = MockNodeVersionManager {
            calls: calls.clone(),
        };
        let executor = MockCommandExecutor {
            calls: calls.clone(),
        };

        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        let venv_dir = temp.path().join("venvs");
        std::fs::create_dir_all(target_dir.clone())?;
        std::fs::write(target_dir.join(".venv"), b"default")?;

        let mut config = make_config(false);
        config.create_venv = true;
        config.target_dir = target_dir;

        let config_loader = MockConfigLoader {
            calls,
            config: Some(Config {
                python_install_dir: temp.path().join("python"),
                venv_dir,
                cache_dir: temp.path().join("cache"),
                current_python_version: None,
            }),
        };

        let err = validator
            .verify_installation_with(
                &config,
                &python_version_manager,
                &node_version_manager,
                &executor,
                &config_loader,
            )
            .await
            .expect_err("missing venv directory should fail verification");
        assert!(
            err.to_string().contains("虚拟环境未找到"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }
}
