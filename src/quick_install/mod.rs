//! 一键安装模块。
//!
//! 本模块提供"一键安装"完整开发环境的功能，自动安装 Python、Node.js、Java、Go
//! 并创建虚拟环境，适合新项目快速启动。
//!
//! 子模块：
//! - `config`: `QuickInstallConfig` 定义，配置一键安装参数
//! - `installer`: `QuickInstaller` 实现，编排完整安装流程
//! - `validator`: `QuickInstallValidator` 实现，安装后验证所有组件
//!
//! 主要函数：
//! - `handle_quick_install`: 处理 `meetai quick-install` 命令
//!
//! 使用场景：
//! - 新项目初始化：在空目录运行 `meetai quick-install`
//! - 团队环境标准化：确保所有成员使用相同工具链
//! - CI/CD 环境准备：自动化脚本安装开发依赖
//!
//! 安装的组件：
//! | 组件 | 是否必装 | 默认版本 | 安装目录 |
//! |------|----------|----------|----------|
//! | Python | ✅ 必装 | latest | `<app_home>/python/python-<version>` |
//! | Pip 包 | ✅ 必装 | 随 Python | 当前激活的 Python 环境 |
//! | 虚拟环境 | ✅ 必装 | `.venv` | 实体在 `<app_home>/venvs/<venv_name>`，项目目录写入 `<target_dir>/.venv` 标记 |
//! | Node.js | ❌ 可选 | lts | `<app_home>/nodejs/versions/<version>` |
//! | Java (JDK) | ❌ 可选 | 17 | 当前仅输出计划提示，尚未自动安装 |
//! | Go | ❌ 可选 | latest | 当前仅输出计划提示，尚未自动安装 |
//!
//! 典型工作流：
//! 1. 用户执行: `meetai quick-install --target-dir . --python-version latest --install-nodejs true`
//! 2. 解析参数 → 创建 `QuickInstallConfig`
//! 3. 验证配置 → 确保目标目录有效、版本格式正确
//! 4. 执行安装 → `QuickInstaller::install()`
//!    - 安装 Node.js（如果启用）
//!    - 安装 Java（如果启用）
//!    - 安装 Go（如果启用）
//!    - 安装 Python
//!    - 安装 pip 基础包（pip、setuptools、wheel）
//!    - 创建全局虚拟环境并写入项目 `.venv` 标记（如果启用）
//!    - 验证所有组件
//! 5. 打印摘要 → 显示激活命令、版本信息
//!
//! 错误处理：
//! - 任何步骤失败立即停止，保留已安装组件
//! - 错误消息包含网络诊断建议（如需要）
//! - 失败后可重试，已安装组件不会重复安装
//!
//! 与 CLI 集成：
//! - 对应 `meetai quick-install` 命令
//! - 参数映射：`QuickInstallArgs` → `QuickInstallConfig`
//!
//! 测试：
//! - 模块内 `mod tests` 包含完整的 mock 测试，覆盖所有安装路径

pub mod config;
pub mod installer;
pub mod validator;

pub use config::QuickInstallConfig;
pub use installer::QuickInstaller;

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
