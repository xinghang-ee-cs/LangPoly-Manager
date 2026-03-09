use anyhow::Result;
use clap::Parser;
use log::info;
use meetai::cli::{Commands, MeetAiCli};

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
