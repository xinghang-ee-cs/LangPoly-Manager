use crate::cli::QuickInstallArgs;
use crate::utils::validator::Validator;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 一键安装配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickInstallConfig {
    pub python_version: String,
    pub pip_version: String,
    pub venv_name: String,
    pub create_venv: bool,
    pub target_dir: PathBuf,
    pub install_nodejs: bool,
    pub nodejs_version: String,
    pub install_java: bool,
    pub java_version: String,
    pub install_go: bool,
    pub go_version: String,
    pub auto_activate: bool,
}

impl QuickInstallConfig {
    /// 从命令行参数创建配置
    pub fn from_args(args: QuickInstallArgs) -> Result<Self> {
        let validator = Validator::new();

        // 验证 Python 版本
        if args.python_version != "latest" {
            validator.validate_python_version(&args.python_version)?;
        }

        // 验证 Pip 版本
        if args.pip_version != "latest" {
            validator.validate_pip_version(&args.pip_version)?;
        }

        // 仅在创建虚拟环境时验证名称
        if args.create_venv {
            validator.validate_package_name(&args.venv_name)?;
        }

        // 验证 Node.js 版本
        if args.install_nodejs {
            Self::validate_runtime_version("Node.js", &args.nodejs_version)?;
        }

        // 验证 Java 版本
        if args.install_java {
            Self::validate_runtime_version("Java", &args.java_version)?;
        }

        // 验证 Go 版本
        if args.install_go {
            Self::validate_runtime_version("Go", &args.go_version)?;
        }

        Ok(Self {
            python_version: args.python_version,
            pip_version: args.pip_version,
            venv_name: args.venv_name,
            create_venv: args.create_venv,
            target_dir: args.target_dir,
            install_nodejs: args.install_nodejs,
            nodejs_version: args.nodejs_version,
            install_java: args.install_java,
            java_version: args.java_version,
            install_go: args.install_go,
            go_version: args.go_version,
            auto_activate: true,
        })
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 检查目标目录是否存在
        if !self.target_dir.exists() {
            anyhow::bail!(
                "Target directory does not exist: {}",
                self.target_dir.display()
            );
        }

        // 如果创建虚拟环境，检查是否已存在同名环境
        if self.create_venv {
            // 这里可以添加检查虚拟环境是否已存在的逻辑
        }

        Ok(())
    }

    fn validate_runtime_version(runtime_name: &str, version: &str) -> Result<()> {
        if version == "latest" {
            return Ok(());
        }

        let re = Regex::new(r"^\d+(\.\d+){0,2}$")?;
        if !re.is_match(version) {
            anyhow::bail!(
                "Invalid {} version format: {}. Expected 'latest' or numeric versions like 21 / 1.22 / 20.11.1",
                runtime_name,
                version
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_args(create_venv: bool, target_dir: PathBuf) -> QuickInstallArgs {
        QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv,
            target_dir,
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        }
    }

    #[test]
    fn from_args_preserves_create_venv_true() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(make_args(true, temp.path().to_path_buf()))?;
        assert!(config.create_venv);
        Ok(())
    }

    #[test]
    fn from_args_preserves_create_venv_false() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(make_args(false, temp.path().to_path_buf()))?;
        assert!(!config.create_venv);
        Ok(())
    }

    #[test]
    fn from_args_allows_invalid_venv_name_when_create_venv_false() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "bad name".to_string(),
            create_venv: false,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })?;
        assert_eq!(config.venv_name, "bad name");
        Ok(())
    }

    #[test]
    fn from_args_rejects_invalid_venv_name_when_create_venv_true() -> Result<()> {
        let temp = tempdir()?;
        let err = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "bad name".to_string(),
            create_venv: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })
        .expect_err("invalid venv name should be rejected when create_venv=true");
        assert!(
            err.to_string().contains("包名格式不正确"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn validate_rejects_missing_target_dir() -> Result<()> {
        let missing = std::env::temp_dir().join("meetai-missing-target-dir-for-test");
        let config = QuickInstallConfig::from_args(make_args(true, missing.clone()))?;

        let err = config
            .validate()
            .expect_err("validation should fail for missing target dir");
        assert!(
            err.to_string().contains("Target directory does not exist"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn validate_accepts_existing_target_dir() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(make_args(false, temp.path().to_path_buf()))?;
        config.validate()?;
        Ok(())
    }

    #[test]
    fn from_args_preserves_multiruntime_flags() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: true,
            nodejs_version: "20.11.1".to_string(),
            install_java: true,
            java_version: "21".to_string(),
            install_go: true,
            go_version: "1.22.2".to_string(),
        })?;

        assert!(config.install_nodejs);
        assert_eq!(config.nodejs_version, "20.11.1");
        assert!(config.install_java);
        assert_eq!(config.java_version, "21");
        assert!(config.install_go);
        assert_eq!(config.go_version, "1.22.2");

        Ok(())
    }
}
