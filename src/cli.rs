use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "meetai")]
#[command(
    about = "MeetAI 多语言开发环境管理工具（Python / Node.js / Java / Go）",
    long_about = None
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
/// MeetAI CLI 顶层参数入口。
pub struct MeetAiCli {
    /// 启用详细输出
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub verbose: bool,

    /// 子命令
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
/// MeetAI 一级子命令集合。
pub enum Commands {
    /// 统一运行时版本管理（Python / Node.js / Java / Go）
    #[command(name = "runtime")]
    Runtime(RuntimeArgs),

    /// Python 版本管理
    #[command(name = "python")]
    Python(PythonArgs),

    /// Pip 包管理
    #[command(name = "pip")]
    Pip(PipArgs),

    /// 虚拟环境管理
    #[command(name = "venv")]
    Venv(VenvArgs),

    /// 一键安装运行时并完成基础环境初始化
    #[command(name = "quick-install")]
    QuickInstall(QuickInstallArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
/// 受支持的运行时类型枚举。
pub enum RuntimeType {
    Python,
    Nodejs,
    Java,
    Go,
}

impl RuntimeType {
    /// 返回运行时类型的用户可读展示名。
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::Nodejs => "Node.js",
            Self::Java => "Java",
            Self::Go => "Go",
        }
    }
}

/// 统一运行时管理参数
#[derive(Parser, Debug)]
pub struct RuntimeArgs {
    #[command(subcommand)]
    pub action: RuntimeAction,
}

#[derive(Subcommand, Debug)]
/// 统一 runtime 子命令动作集合。
pub enum RuntimeAction {
    /// 列出支持的运行时或指定运行时的已安装版本
    List {
        /// 运行时类型，未指定时列出支持矩阵
        #[arg(value_enum)]
        runtime: Option<RuntimeType>,
    },
    /// 安装指定运行时版本
    Install {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        version: String,
    },
    /// 切换当前运行时版本
    Use {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        version: String,
    },
    /// 卸载指定运行时版本
    Uninstall {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        version: String,
    },
}

/// Python 版本管理参数
#[derive(Parser, Debug)]
pub struct PythonArgs {
    #[command(subcommand)]
    pub action: PythonAction,
}

#[derive(Subcommand, Debug)]
/// Python 子命令动作集合。
pub enum PythonAction {
    /// 列出所有已安装的 Python 版本
    List,
    /// 安装指定版本的 Python
    Install {
        /// Python 版本号
        version: String,
    },
    /// 切换全局 Python 版本
    Use {
        /// Python 版本号
        version: String,
    },
    /// 卸载指定版本
    Uninstall {
        /// Python 版本号
        version: String,
    },
}

/// Pip 包管理参数
#[derive(Parser, Debug)]
pub struct PipArgs {
    #[command(subcommand)]
    pub action: PipAction,
}

#[derive(Subcommand, Debug)]
/// Pip 子命令动作集合。
pub enum PipAction {
    /// 安装包
    Install {
        /// 包名
        package: String,
        /// 指定版本
        #[arg(short = 'v', long)]
        version: Option<String>,
    },
    /// 卸载包
    Uninstall {
        /// 包名
        package: String,
    },
    /// 更新包
    Upgrade {
        /// 包名
        package: String,
    },
    /// 列出已安装的包
    List,
}

/// 虚拟环境管理参数
#[derive(Parser, Debug)]
pub struct VenvArgs {
    #[command(subcommand)]
    pub action: VenvAction,
}

#[derive(Subcommand, Debug)]
/// 虚拟环境子命令动作集合。
pub enum VenvAction {
    /// 创建虚拟环境
    Create {
        /// 虚拟环境名称
        name: String,
        /// 目标目录，默认为当前目录
        #[arg(short, long, default_value = ".")]
        target_dir: PathBuf,
    },
    /// 激活虚拟环境
    Activate {
        /// 虚拟环境名称
        name: String,
    },
    /// 列出所有虚拟环境
    List,
}

/// 一键安装参数
#[derive(Parser, Debug)]
pub struct QuickInstallArgs {
    /// Python 版本，默认为 latest
    #[arg(long, default_value = "latest")]
    pub python_version: String,

    /// Pip 版本，默认为 latest
    #[arg(long, default_value = "latest")]
    pub pip_version: String,

    /// 虚拟环境名称，默认为 "default"
    #[arg(long, default_value = "default")]
    pub venv_name: String,

    /// 是否创建虚拟环境
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub create_venv: bool,

    /// 安装目标目录，默认为当前目录
    #[arg(long, default_value = ".")]
    pub target_dir: PathBuf,

    /// 是否安装 Node.js（当前为规划支持能力）
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_nodejs: bool,

    /// Node.js 版本，默认为 latest
    #[arg(long, default_value = "latest")]
    pub nodejs_version: String,

    /// 是否安装 Java（当前为规划支持能力）
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_java: bool,

    /// Java 版本，默认为 latest
    #[arg(long, default_value = "latest")]
    pub java_version: String,

    /// 是否安装 Go（当前为规划支持能力）
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_go: bool,

    /// Go 版本，默认为 latest
    #[arg(long, default_value = "latest")]
    pub go_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cli(args: &[&str]) -> MeetAiCli {
        MeetAiCli::try_parse_from(args).expect("cli parsing should succeed")
    }

    #[test]
    fn quick_install_defaults_create_venv_to_true() {
        let cli = parse_cli(&["meetai", "quick-install"]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(args.create_venv);
    }

    #[test]
    fn quick_install_accepts_create_venv_false() {
        let cli = parse_cli(&["meetai", "quick-install", "--create-venv", "false"]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(!args.create_venv);
    }

    #[test]
    fn quick_install_accepts_multiruntime_flags() {
        let cli = parse_cli(&[
            "meetai",
            "quick-install",
            "--install-nodejs",
            "true",
            "--nodejs-version",
            "20.11.1",
            "--install-java",
            "true",
            "--java-version",
            "21",
            "--install-go",
            "true",
            "--go-version",
            "1.22.2",
        ]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(args.install_nodejs);
        assert_eq!(args.nodejs_version, "20.11.1");
        assert!(args.install_java);
        assert_eq!(args.java_version, "21");
        assert!(args.install_go);
        assert_eq!(args.go_version, "1.22.2");
    }

    #[test]
    fn runtime_install_parses() {
        let cli = parse_cli(&["meetai", "runtime", "install", "python", "3.13.2"]);
        let Commands::Runtime(args) = cli.command else {
            panic!("expected runtime command");
        };

        let RuntimeAction::Install { runtime, version } = args.action else {
            panic!("expected runtime install action");
        };

        assert_eq!(runtime, RuntimeType::Python);
        assert_eq!(version, "3.13.2");
    }
}
