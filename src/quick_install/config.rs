//! 一键安装配置模块。
//!
//! 本模块定义 `QuickInstallConfig` 结构体，用于"一键安装"功能的配置数据。
//! 该配置从 CLI 参数转换而来，经过验证后驱动完整的开发环境搭建流程。
//!
//! 核心类型：
//! - `QuickInstallConfig`: 一键安装配置，包含所有运行时和选项设置
//!
//! 配置字段说明：
//! | 字段 | 类型 | 说明 |
//! |------|------|------|
//! | `python_version` | `String` | Python 版本（精确版本或 "latest"） |
//! | `pip_version` | `String` | pip 版本（通常与 Python 版本一致） |
//! | `venv_name` | `String` | 虚拟环境名称（如 `default`、`myenv`） |
//! | `create_venv` | `bool` | 是否创建虚拟环境（默认 true） |
//! | `target_dir` | `PathBuf` | 项目目标目录 |
//! | `install_nodejs` | `bool` | 是否安装 Node.js |
//! | `nodejs_version` | `String` | Node.js 版本（如 `latest`、`newest`、`lts`、`project` 或精确版本） |
//! | `install_java` | `bool` | 是否在 quick-install 中纳入 Java（当前仅输出计划提示） |
//! | `java_version` | `String` | Java 版本（如 `"latest"`、`"17"`、`"21"`） |
//! | `install_go` | `bool` | 是否在 quick-install 中纳入 Go（当前仅输出计划提示） |
//! | `go_version` | `String` | Go 版本（如 `"latest"`、`"1.21"`、`"1.21.0"`） |
//! | `auto_activate` | `bool` | 安装后是否自动激活环境（默认 true） |
//!
//! 验证规则：
//! 1. **目标目录**: `target_dir` 会原样写入配置，后续 `validate()` 阶段要求路径存在且必须是目录
//! 2. **虚拟环境名称**: 仅当 `create_venv=true` 时验证格式
//!    - 当前复用 `Validator::validate_package_name()`
//!    - 允许字母、数字、`-`、`_` 字符
//! 3. **版本格式**: 各运行时版本通过 `Validator` 验证
//!    - Python: 精确版本或 "latest"
//!    - Node.js: 精确版本、`latest`、`newest`、`lts` 或 `project`
//!    - Java: `latest` 或数字版本（如 "17"、"21"）
//!    - Go: `latest` 或数字版本（如 "1.21"、"1.21.0"）
//! 4. **虚拟环境冲突**: 当 `create_venv=true` 时，不允许目标目录已存在 `.venv` 标记文件，
//!    也不允许全局 `venv_dir/<venv_name>` 已存在
//!
//! 配置转换：
//! - 从 `QuickInstallArgs` (CLI) → `QuickInstallConfig` (内部使用)
//! - `from_args()` 方法执行转换和验证
//!
//! 使用示例：
//! ```rust
//! use std::path::PathBuf;
//! use meetai::cli::QuickInstallArgs;
//! use meetai::quick_install::QuickInstallConfig;
//!
//! let args = QuickInstallArgs {
//!     python_version: "3.11.5".into(),
//!     pip_version: "latest".into(),
//!     venv_name: "default".into(),
//!     create_venv: true,
//!     auto_activate: true,
//!     target_dir: PathBuf::from("."),
//!     install_nodejs: true,
//!     nodejs_version: "lts".into(),
//!     install_java: false,
//!     java_version: "latest".into(),
//!     install_go: false,
//!     go_version: "latest".into(),
//! };
//! let config = QuickInstallConfig::from_args(args)?;
//! assert!(config.create_venv);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! 错误处理：
//! - 验证失败返回 `anyhow::Error`，包含具体错误原因
//! - 虚拟环境名称无效时，会返回包含允许字符范围的错误
//! - 版本格式错误时，会返回对应运行时的格式提示
//!
//! 测试：
//! - 模块内 `mod tests` 包含配置转换、验证逻辑、版本格式测试

use crate::cli::QuickInstallArgs;
use crate::config::Config;
use crate::utils::validator::Validator;
use anyhow::{Context, Result};
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
        validator.validate_python_install_version(&args.python_version)?;

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
            validator.validate_node_install_version(&args.nodejs_version)?;
        }

        // 验证 Java 版本
        if args.install_java {
            validator.validate_java_install_version(&args.java_version)?;
        }

        // 验证 Go 版本
        if args.install_go {
            validator.validate_go_install_version(&args.go_version)?;
        }

        Ok(Self {
            python_version: args.python_version,
            pip_version: args.pip_version,
            venv_name: args.venv_name,
            create_venv: args.create_venv,
            auto_activate: args.auto_activate,
            target_dir: args.target_dir,
            install_nodejs: args.install_nodejs,
            nodejs_version: args.nodejs_version,
            install_java: args.install_java,
            java_version: args.java_version,
            install_go: args.install_go,
            go_version: args.go_version,
        })
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        let app_config = Config::load().context("加载应用配置失败")?;
        self.validate_with_config(&app_config)
    }

    fn validate_with_config(&self, app_config: &Config) -> Result<()> {
        // 检查目标目录是否存在
        if !self.target_dir.exists() {
            anyhow::bail!(
                "Target directory does not exist: {}",
                self.target_dir.display()
            );
        }

        if !self.target_dir.is_dir() {
            anyhow::bail!(
                "Target directory is not a directory: {}",
                self.target_dir.display()
            );
        }

        // 如果创建虚拟环境，检查是否已存在同名环境
        if self.create_venv {
            let marker_path = self.target_dir.join(".venv");
            if marker_path.exists() {
                anyhow::bail!(
                    "Target directory already contains a .venv marker file: {}",
                    marker_path.display()
                );
            }

            let venv_path = app_config.venv_dir.join(&self.venv_name);
            if venv_path.exists() {
                anyhow::bail!(
                    "Virtual environment already exists: {}",
                    venv_path.display()
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_app_config(base_dir: &std::path::Path) -> Config {
        Config {
            python_install_dir: base_dir.join("python"),
            venv_dir: base_dir.join("venvs"),
            cache_dir: base_dir.join("cache"),
            current_python_version: None,
        }
    }

    fn make_args(create_venv: bool, target_dir: PathBuf) -> QuickInstallArgs {
        QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv,
            auto_activate: true,
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
    fn from_args_preserves_auto_activate_flag() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: false,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })?;
        assert!(!config.auto_activate);
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
            auto_activate: true,
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
            auto_activate: true,
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
        let temp = tempdir()?;
        let missing = temp.path().join("missing-target-dir");
        let config = QuickInstallConfig::from_args(make_args(true, missing.clone()))?;
        let app_config = make_app_config(temp.path());

        let err = config
            .validate_with_config(&app_config)
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
        let app_config = make_app_config(temp.path());
        config.validate_with_config(&app_config)?;
        Ok(())
    }

    #[test]
    fn validate_rejects_target_dir_when_it_is_a_file() -> Result<()> {
        let temp = tempdir()?;
        let target_file = temp.path().join("project.txt");
        std::fs::write(&target_file, b"not a directory")?;

        let config = QuickInstallConfig::from_args(make_args(true, target_file.clone()))?;
        let app_config = make_app_config(temp.path());
        let err = config
            .validate_with_config(&app_config)
            .expect_err("validation should fail when target path is a file");
        assert!(
            err.to_string()
                .contains("Target directory is not a directory"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn validate_rejects_existing_venv_marker_when_create_venv_true() -> Result<()> {
        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        std::fs::create_dir_all(&target_dir)?;
        std::fs::write(target_dir.join(".venv"), b"existing-venv")?;

        let config = QuickInstallConfig::from_args(make_args(true, target_dir.clone()))?;
        let app_config = make_app_config(temp.path());
        let err = config
            .validate_with_config(&app_config)
            .expect_err("validation should fail when .venv marker already exists");
        assert!(
            err.to_string()
                .contains("Target directory already contains a .venv marker file"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn validate_rejects_existing_named_venv_when_create_venv_true() -> Result<()> {
        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        std::fs::create_dir_all(&target_dir)?;

        let app_config = make_app_config(temp.path());
        std::fs::create_dir_all(app_config.venv_dir.join("test-env"))?;

        let config = QuickInstallConfig::from_args(make_args(true, target_dir))?;
        let err = config
            .validate_with_config(&app_config)
            .expect_err("validation should fail when global venv already exists");
        assert!(
            err.to_string()
                .contains("Virtual environment already exists"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[test]
    fn validate_allows_existing_venv_state_when_create_venv_false() -> Result<()> {
        let temp = tempdir()?;
        let target_dir = temp.path().join("project");
        std::fs::create_dir_all(&target_dir)?;
        std::fs::write(target_dir.join(".venv"), b"existing-venv")?;

        let app_config = make_app_config(temp.path());
        std::fs::create_dir_all(app_config.venv_dir.join("test-env"))?;

        let config = QuickInstallConfig::from_args(make_args(false, target_dir))?;
        config.validate_with_config(&app_config)?;
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
            auto_activate: true,
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

    #[test]
    fn from_args_rejects_invalid_nodejs_version_when_install_nodejs_true() -> Result<()> {
        let temp = tempdir()?;
        let err = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: true,
            nodejs_version: "../20.11.1".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })
        .expect_err("invalid nodejs version should be rejected when install_nodejs=true");
        assert!(
            err.to_string().contains("Node.js 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn from_args_allows_invalid_nodejs_version_when_install_nodejs_false() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "../20.11.1".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })?;
        assert_eq!(config.nodejs_version, "../20.11.1");
        Ok(())
    }

    #[test]
    fn from_args_rejects_invalid_java_version_when_install_java_true() -> Result<()> {
        let temp = tempdir()?;
        let err = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: true,
            java_version: "21-ea".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })
        .expect_err("invalid java version should be rejected when install_java=true");
        assert!(
            err.to_string().contains("Java 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn from_args_allows_invalid_java_version_when_install_java_false() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "21-ea".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
        })?;
        assert_eq!(config.java_version, "21-ea");
        Ok(())
    }

    #[test]
    fn from_args_rejects_invalid_go_version_when_install_go_true() -> Result<()> {
        let temp = tempdir()?;
        let err = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: true,
            go_version: "1.22beta1".to_string(),
        })
        .expect_err("invalid go version should be rejected when install_go=true");
        assert!(
            err.to_string().contains("Go 版本号格式不正确"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn from_args_allows_invalid_go_version_when_install_go_false() -> Result<()> {
        let temp = tempdir()?;
        let config = QuickInstallConfig::from_args(QuickInstallArgs {
            python_version: "latest".to_string(),
            pip_version: "latest".to_string(),
            venv_name: "test-env".to_string(),
            create_venv: true,
            auto_activate: true,
            target_dir: temp.path().to_path_buf(),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "1.22beta1".to_string(),
        })?;
        assert_eq!(config.go_version, "1.22beta1");
        Ok(())
    }
}
