pub mod config;
pub mod installer;
pub mod validator;

pub use config::QuickInstallConfig;
pub use installer::QuickInstaller;
pub use validator::QuickInstallValidator;

use crate::cli::QuickInstallArgs;
use crate::utils::guidance::{network_diagnostic_tips, quick_install_help_commands};
use anyhow::{Context, Result};

/// 处理一键安装命令
pub async fn handle_quick_install(args: QuickInstallArgs) -> Result<()> {
    // 创建配置
    let config = QuickInstallConfig::from_args(args).with_context(|| {
        format!(
            "无法解析安装参数，请检查命令格式。\n{}",
            quick_install_help_commands()
        )
    })?;
    config.validate().with_context(|| {
        format!(
            "安装参数不合法，请检查版本号或选项值。\n{}",
            quick_install_help_commands()
        )
    })?;

    // 验证配置
    let validator = QuickInstallValidator::new();
    validator.validate(&config).with_context(|| {
        format!(
            "安装前环境检查未通过，请确认运行环境后重试。\n{}",
            quick_install_help_commands()
        )
    })?;

    // 执行安装
    let installer = QuickInstaller::new()?;
    installer.install(&config).await.with_context(|| {
        format!(
            "安装过程中出现错误，请参考上方日志排查。\n{}\n{}",
            quick_install_help_commands(),
            network_diagnostic_tips()
        )
    })?;

    Ok(())
}
