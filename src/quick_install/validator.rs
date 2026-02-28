use crate::config::Config;
use crate::python::version::PythonVersionManager;
use crate::quick_install::config::QuickInstallConfig;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};

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
        // 验证 Python 版本是否已设置并可执行
        let version_manager = PythonVersionManager::new()?;
        let python_exe = version_manager.current_python_executable(
            "安装后未检测到已激活的 Python 版本，请手动执行: meetai runtime use python <version>",
        )?;

        // 验证 pip 是否可用
        let executor = CommandExecutor::new();
        executor
            .execute_with_output_async(&python_exe, &["-m", "pip", "--version"])
            .await
            .context(
                "pip 验证失败，Python 已安装但 pip 可能未正确配置，可尝试: python -m ensurepip",
            )?;

        // 验证虚拟环境是否创建成功
        if config.create_venv {
            let app_config = Config::load()?;
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

        Ok(())
    }
}
