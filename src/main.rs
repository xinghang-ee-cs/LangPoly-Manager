mod cli;
mod config;
mod pip;
mod python;
mod quick_install;
mod runtime;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Commands, MeetAiCli};
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    env_logger::init();

    // 解析命令行参数
    let cli = MeetAiCli::parse();

    info!("MeetAI v{} started", env!("CARGO_PKG_VERSION"));

    // 根据子命令执行相应操作
    match cli.command {
        Commands::Runtime(args) => {
            runtime::handle_runtime_command(args).await?;
        }
        Commands::Python(args) => {
            python::handle_python_command(args).await?;
        }
        Commands::Pip(args) => {
            pip::handle_pip_command(args).await?;
        }
        Commands::Venv(args) => {
            python::handle_venv_command(args).await?;
        }
        Commands::QuickInstall(args) => {
            quick_install::handle_quick_install(args).await?;
        }
    }

    Ok(())
}
