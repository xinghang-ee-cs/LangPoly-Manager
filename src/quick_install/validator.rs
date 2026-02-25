use crate::config::Config;
use crate::python::version::PythonVersionManager;
use crate::quick_install::config::QuickInstallConfig;
use crate::utils::executor::CommandExecutor;
use crate::utils::validator::Validator;
use anyhow::{Context, Result};
use regex::Regex;

/// 一键安装验证器
pub struct QuickInstallValidator {
    validator: Validator,
}

impl QuickInstallValidator {
    pub fn new() -> Self {
        Self {
            validator: Validator::new(),
        }
    }

    /// 验证安装配置
    pub fn validate(&self, config: &QuickInstallConfig) -> Result<()> {
        // 验证目标目录
        if !config.target_dir.exists() {
            anyhow::bail!(
                "目标目录不存在：{}，请先创建该目录或更换安装路径",
                config.target_dir.display()
            );
        }

        // 验证 Python 版本
        if config.python_version != "latest" {
            self.validator
                .validate_python_version(&config.python_version)?;
        }

        // 验证 Pip 版本
        if config.pip_version != "latest" {
            self.validator.validate_pip_version(&config.pip_version)?;
        }

        // 验证虚拟环境名称
        if config.create_venv {
            self.validator.validate_package_name(&config.venv_name)?;
        }

        // 验证多语言版本参数（已选中安装时）
        if config.install_nodejs {
            validate_generic_runtime_version("Node.js", &config.nodejs_version)?;
        }
        if config.install_java {
            validate_generic_runtime_version("Java", &config.java_version)?;
        }
        if config.install_go {
            validate_generic_runtime_version("Go", &config.go_version)?;
        }

        Ok(())
    }

    /// 验证安装结果
    pub async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<bool> {
        // 验证 Python 版本是否已设置并可执行
        let version_manager = PythonVersionManager::new()?;
        let current_version = version_manager.get_current_version()?.context(
            "安装后未检测到已激活的 Python 版本，请手动执行: meetai runtime use python <version>",
        )?;

        let python_path = version_manager
            .get_python_path(&current_version)
            .context("找不到已选 Python 的安装路径，可能需要重新安装")?;

        let python_exe = if cfg!(windows) {
            python_path.join("python.exe")
        } else {
            python_path.join("bin/python")
        };

        if !python_exe.exists() {
            anyhow::bail!(
                "Python 可执行文件不存在：{}，安装可能未成功完成，请尝试重新安装",
                python_exe.display()
            );
        }

        // 验证 pip 是否可用
        let executor = CommandExecutor::new();
        executor
            .execute_with_output(&python_exe, &["-m", "pip", "--version"])
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

        Ok(true)
    }
}

fn validate_generic_runtime_version(runtime_name: &str, version: &str) -> Result<()> {
    if version == "latest" {
        return Ok(());
    }

    let re = Regex::new(r"^\d+(\.\d+){0,2}$")?;
    if !re.is_match(version) {
        anyhow::bail!(
            "{} 版本号格式不正确：{}，请填写 'latest' 或版本号（如 18 / 1.22 / 20.11.1）",
            runtime_name,
            version
        );
    }

    Ok(())
}
