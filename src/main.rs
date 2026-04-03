use anyhow::Result;
use clap::Parser;
use log::info;
use meetai::cli::{Commands, MeetAiCli};

/// MeetAI 应用程序入口点。
///
/// 负责初始化日志系统、解析命令行参数，并根据子命令分发到对应的处理器。
/// 支持的所有命令都在 `Commands` 枚举中定义。
///
/// # 日志级别
///
/// - 默认级别：`info` - 显示关键操作信息
/// - 使用 `--verbose` 参数：提升到 `debug` - 显示详细调试信息
///
/// # 命令分发
///
/// 当前支持的命令包括：
/// - `runtime`: 统一运行时管理（Python/Node.js/Java/Go）
/// - `python`: Python 版本管理
/// - `node`: Node.js 版本管理
/// - `pip`: Python 包管理
/// - `venv`: 虚拟环境管理
/// - `quick-install`: 一键安装并初始化环境
///
/// # 示例
///
/// ```bash
/// # 查看版本
/// meetai --version
///
/// # 查看所有 Python 版本
/// meetai python list
///
/// # 安装并使用 Python 3.11
/// meetai python install 3.11.0
/// meetai python use 3.11.0
/// ```
#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let cli = MeetAiCli::parse();

    // 初始化日志（默认 info，--verbose 提升到 debug）
    let default_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
        .init();

    info!("MeetAI v{} 启动完成", env!("CARGO_PKG_VERSION"));

    // 根据子命令执行相应操作
    match cli.command {
        Commands::Runtime(args) => {
            meetai::runtime::handle_runtime_command(args).await?;
        }
        Commands::Python(args) => {
            meetai::python::handle_python_command(args).await?;
        }
        Commands::Node(args) => {
            meetai::node::handle_node_command(args).await?;
        }
        Commands::Pip(args) => {
            meetai::pip::handle_pip_command(args).await?;
        }
        Commands::Venv(args) => {
            meetai::python::handle_venv_command(args).await?;
        }
        Commands::QuickInstall(args) => {
            meetai::quick_install::handle_quick_install(args).await?;
        }
    }

    Ok(())
}
