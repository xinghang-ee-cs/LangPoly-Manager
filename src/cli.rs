//! MeetAI 命令行接口定义。
//!
//! 本模块定义所有 CLI 参数结构体、子命令枚举和运行时类型枚举。
//! 使用 [`clap`](https://docs.rs/clap) 库进行命令行解析，提供友好的帮助信息和参数验证。
//!
//! # 命令结构
//!
//! ```text
//! meetai [全局选项] <子命令> [子命令选项]
//!
//! 全局选项：
//!   --verbose   启用详细输出（debug 级别日志）
//!   --version   显示版本信息
//!
//! 子命令：
//!   runtime     统一运行时管理（Python/Node.js/Java/Go）
//!   python      Python 版本管理
//!   node        Node.js 版本管理
//!   pip         Python 包管理
//!   venv        虚拟环境管理
//!   quick-install 一键安装并初始化环境
//! ```
//!
//! # 使用示例
//!
//! ```bash
//! # 查看所有 Python 版本
//! meetai python list
//!
//! # 安装并使用 Python 3.11
//! meetai python install 3.11.0
//! meetai python use 3.11.0
//!
//! # 一键安装环境（Python + Node.js + 虚拟环境）
//! meetai quick-install --python-version 3.11.0 --nodejs-version lts
//!
//! # 查看运行时列表
//! meetai runtime list
//! meetai runtime list python
//!
//! # 安装 Node.js LTS 版本
//! meetai node install lts
//! ```
//!

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// MeetAI CLI 顶层参数入口。
///
/// 定义全局选项和子命令分发结构。所有子命令都通过 `command` 字段分发到对应的处理器。
///
/// # 字段
///
/// - `verbose`: 启用详细输出（debug 级别日志），默认 `false`
/// - `command`: 要执行的子命令
///
/// # 示例
///
/// ```rust
/// use clap::Parser;
/// use meetai::cli::{Commands, MeetAiCli};
///
/// let cli = MeetAiCli::parse_from(["meetai", "python", "list"]);
/// match cli.command {
///     Commands::Python(_) => {}
///     _ => unreachable!("expected python subcommand"),
/// }
/// ```
#[derive(Parser, Debug)]
#[command(name = "meetai")]
#[command(
    about = "MeetAI 多语言开发环境管理工具（Python / Node.js / Java / Go）",
    long_about = None
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct MeetAiCli {
    /// 启用详细输出（debug 级别日志）
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub verbose: bool,

    /// 子命令
    #[command(subcommand)]
    pub command: Commands,
}

/// MeetAI 一级子命令集合。
///
/// 定义所有顶级子命令，每个子命令对应一个功能模块。
/// 子命令的具体参数由对应的结构体定义。
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 统一运行时版本管理（Python / Node.js / Java / Go）
    ///
    /// 提供统一的接口管理多种编程语言运行时，支持安装、卸载、切换版本。
    /// 不同运行时的安装能力可能因平台而异：
    /// - Python Windows: 支持自动下载安装
    /// - Python macOS/Linux: 采纳系统已安装版本
    /// - Node.js Windows/Linux x64/arm64: 支持自动下载安装
    /// - 其他平台/架构: 需手动安装后用 `use` 命令切换
    #[command(name = "runtime")]
    Runtime(RuntimeArgs),

    /// Python 版本管理
    ///
    /// 管理本地已安装的 Python 版本，包括安装、卸载、切换当前版本。
    /// 支持 `latest` 关键字；Windows 会下载安装，Linux/macOS 会采纳系统 Python。
    #[command(name = "python")]
    Python(PythonArgs),

    /// Node.js 版本管理
    ///
    /// 管理本地已安装的 Node.js 版本，支持从官方源自动下载安装（Windows/Linux x64/arm64）。
    /// 额外提供 `available` 子命令查看官方可安装版本列表（含 LTS 标记）。
    #[command(name = "node")]
    Node(NodeArgs),

    /// Pip 包管理
    ///
    /// 在当前激活的 Python 环境下管理包，支持安装、卸载、升级和列表查询。
    /// 依赖 `python use` 设置的当前 Python 版本。
    #[command(name = "pip")]
    Pip(PipArgs),

    /// 虚拟环境管理
    ///
    /// 创建、激活和列出虚拟环境。虚拟环境用于隔离项目依赖；
    /// 创建时仍依赖当前激活的 Python 版本。
    #[command(name = "venv")]
    Venv(VenvArgs),

    /// 一键安装运行时并完成基础环境初始化
    ///
    /// 自动安装指定的运行时版本（Python/Node.js/Java/Go），
    /// 并可选择创建虚拟环境、生成项目脚手架。
    /// 适合新机器快速搭建开发环境。
    #[command(name = "quick-install")]
    QuickInstall(QuickInstallArgs),
}

/// 受支持的运行时类型枚举。
///
/// 定义 MeetAI 统一管理的编程语言运行时类型。
/// 用于 `runtime` 命令的参数，指定要操作的运行时。
///
/// # 变体说明
///
/// - `Python`: Python 解释器
/// - `NodeJs`: Node.js 运行时（CLI 名称：`node`，兼容别名：`nodejs`、`node-js`）
/// - `Java`: Java 运行时（规划支持）
/// - `Go`: Go 语言运行时（规划支持）
///
/// # 示例
///
/// ```rust
/// use meetai::cli::RuntimeType;
///
/// let runtime = RuntimeType::Python;
/// println!("{}", runtime.display_name());  // 输出: Python
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum RuntimeType {
    Python,
    #[value(name = "node", aliases = ["nodejs", "node-js"])]
    NodeJs,
    Java,
    Go,
}

impl RuntimeType {
    /// 返回运行时类型的用户可读展示名。
    ///
    /// 用于日志输出、用户提示等场景，提供友好的名称显示。
    ///
    /// # 返回
    ///
    /// 运行时的展示名称字符串。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use meetai::cli::RuntimeType;
    ///
    /// assert_eq!(RuntimeType::Python.display_name(), "Python");
    /// assert_eq!(RuntimeType::NodeJs.display_name(), "Node.js");
    /// ```
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::NodeJs => "Node.js",
            Self::Java => "Java",
            Self::Go => "Go",
        }
    }
}

/// 统一运行时管理参数。
///
/// `runtime` 子命令的顶层参数，包含一个 `RuntimeAction` 指定具体操作。
#[derive(Parser, Debug)]
pub struct RuntimeArgs {
    #[command(subcommand)]
    pub action: RuntimeAction,
}

/// 统一 runtime 子命令动作集合。
///
/// 定义对运行时执行的具体操作：列出版本、安装、切换、卸载。
/// 所有操作都针对指定的 `RuntimeType`。
///
/// # 使用说明
///
/// - `list`: 列出所有支持的运行时，或指定运行时的已安装版本
/// - `install`: 安装指定版本。Python Windows 可下载、macOS/Linux 可采纳系统版本；Node.js Windows/Linux x64/arm64 可下载
/// - `use`: 切换当前激活的运行时版本（修改 shims 指向）
/// - `uninstall`: 卸载指定版本
///
/// # 示例
///
/// ```bash
/// # 列出所有支持的运行时
/// meetai runtime list
///
/// # 列出 Python 已安装版本
/// meetai runtime list python
///
/// # 安装 Python 3.11
/// meetai runtime install python 3.11.0
///
/// # 切换当前 Python 版本
/// meetai runtime use python 3.11.0
///
/// # 卸载 Python 3.10
/// meetai runtime uninstall python 3.10.0
/// ```
#[derive(Subcommand, Debug)]
pub enum RuntimeAction {
    /// 列出支持的运行时或指定运行时的已安装版本
    ///
    /// 不带参数时列出所有支持的运行时类型（Python/Node.js/Java/Go）。
    /// 指定 `runtime` 参数时，列出该运行时的所有已安装版本。
    List {
        /// 运行时类型，未指定时列出支持矩阵
        #[arg(value_enum)]
        runtime: Option<RuntimeType>,
    },

    /// 安装指定运行时版本（Python: Windows 下载、Linux/macOS 采纳系统版本；Node: Windows/Linux x64/arm64 下载）
    ///
    /// 安装指定运行时的指定版本。安装行为因平台而异：
    /// - Python Windows: 自动从官方源下载并安装
    /// - Python macOS/Linux: 采纳系统已安装的匹配版本
    /// - Node.js Windows/Linux x64/arm64: 自动从官方源下载并安装
    /// - 其他平台/架构: 需手动安装后使用 `use` 命令激活
    ///
    /// # 示例
    ///
    /// ```bash
    /// # 安装最新 Python（Windows）
    /// meetai runtime install python latest
    ///
    /// # 安装指定 Node.js 版本
    /// meetai runtime install node 20.11.1
    /// ```
    Install {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        ///
        /// 支持具体版本号（如 `3.11.0`、`20.11.1`）或该运行时支持的特殊值：
        /// - Python: `latest` 或 `X.Y.Z`
        /// - Node.js: `latest`、`newest`、`lts`、`project` 或 `X.Y.Z`
        version: String,
    },

    /// 切换当前运行时版本
    ///
    /// 设置指定运行时的当前激活版本，更新 shims 指向。
    /// 切换后，命令行中直接调用 `python`/`node` 等命令将使用该版本。
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai runtime use python 3.11.0
    /// meetai runtime use node 20.11.1
    /// ```
    Use {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        ///
        /// - Python: 必须为已安装的 `X.Y.Z` 版本
        /// - Node.js: 支持已安装的 `X.Y.Z` 版本，或 `project`（从 `.nvmrc` 读取）
        version: String,
    },

    /// 卸载指定运行时版本
    ///
    /// 从本地移除指定版本的运行时。卸载前会检查版本是否存在，
    /// 如果目标版本当前正在使用，具体处理方式由对应运行时实现决定。
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai runtime uninstall python 3.10.0
    /// ```
    Uninstall {
        /// 运行时类型
        #[arg(value_enum)]
        runtime: RuntimeType,
        /// 版本号
        version: String,
    },
}

/// Python 版本管理参数。
///
/// `python` 子命令的顶层参数，包含一个 `PythonAction` 指定具体操作。
#[derive(Parser, Debug)]
pub struct PythonArgs {
    #[command(subcommand)]
    pub action: PythonAction,
}

/// Python 子命令动作集合。
///
/// 定义对 Python 执行的具体操作：列出版本、安装、切换、卸载。
/// 这些操作直接映射到 `PythonService` 的方法。
///
/// # 使用说明
///
/// - `list`: 列出所有已安装的 Python 版本
/// - `install`: 安装/注册指定版本（Windows 可自动下载，Linux/macOS 采纳系统版本）
/// - `use`: 切换全局 Python 版本（更新 shims）
/// - `uninstall`: 卸载指定版本
///
/// # 示例
///
/// ```bash
/// # 列出已安装版本
/// meetai python list
///
/// # 安装 Python 3.11
/// meetai python install 3.11.0
///
/// # 切换到 Python 3.11
/// meetai python use 3.11.0
///
/// # 卸载 Python 3.10
/// meetai python uninstall 3.10.0
/// ```
#[derive(Subcommand, Debug)]
pub enum PythonAction {
    /// 列出所有已安装的 Python 版本
    List,

    /// 安装指定版本的 Python（Windows 可自动下载安装；macOS/Linux 采纳系统已安装版本）
    ///
    /// 安装 Python 到 MeetAI 管理目录。安装后版本会自动加入管理列表，
    /// 但不会自动设置为当前激活版本（需单独执行 `use`）。
    ///
    /// # 参数
    ///
    /// - `version`: Python 版本号（如 `3.11.0`）或 `latest`（最新稳定版）
    ///
    /// # 平台差异
    ///
    /// - **Windows**: 从 python.org 下载安装包并自动执行安装
    /// - **macOS/Linux**: 不下载/编译 Python，采纳系统已安装的匹配版本
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai python install 3.11.0
    /// meetai python install latest
    /// ```
    Install {
        /// Python 版本号
        version: String,
    },

    /// 切换全局 Python 版本
    ///
    /// 设置全局默认 Python 版本，更新 shims 指向该版本的解释器。
    /// 切换后，在终端中直接执行 `python`、`pip` 等命令将使用该版本。
    ///
    /// # 参数
    ///
    /// - `version`: 已安装的 Python 版本号
    ///
    /// # 错误
    ///
    /// - 版本未安装时报错
    /// - 如果 shims 目录未在 PATH 中，会输出配置指导
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai python use 3.11.0
    /// ```
    Use {
        /// Python 版本号
        version: String,
    },

    /// 卸载指定版本
    ///
    /// 从本地删除指定版本的 Python 安装目录。
    /// 如果该版本当前正在使用，具体行为取决于底层卸载实现。
    ///
    /// # 参数
    ///
    /// - `version`: 要卸载的 Python 版本号
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai python uninstall 3.10.0
    /// ```
    Uninstall {
        /// Python 版本号
        version: String,
    },
}

/// Node.js 版本管理参数。
///
/// `node` 子命令的顶层参数，包含一个 `NodeAction` 指定具体操作。
#[derive(Parser, Debug)]
pub struct NodeArgs {
    #[command(subcommand)]
    pub action: NodeAction,
}

/// Node.js 子命令动作集合。
///
/// 定义对 Node.js 执行的具体操作：列出版本、查看可用版本、安装、切换、卸载。
/// 额外支持从 `.nvmrc` 文件自动检测项目版本。
///
/// # 使用说明
///
/// - `list`: 列出所有已安装版本
/// - `available`: 查看官方可安装版本列表（含 LTS 标记）
/// - `install`: 安装指定版本（支持 `latest`/`lts`/`project` 等特殊值）
/// - `use`: 切换版本（支持从 `.nvmrc` 自动检测）
/// - `uninstall`: 卸载指定版本
///
/// # 示例
///
/// ```bash
/// # 列出已安装版本
/// meetai node list
///
/// # 查看可安装版本
/// meetai node available
///
/// # 安装 LTS 版本
/// meetai node install lts
///
/// # 使用项目指定的版本（从 .nvmrc 读取）
/// meetai node use project
/// ```
#[derive(Subcommand, Debug)]
pub enum NodeAction {
    /// 列出所有已安装的 Node.js 版本
    List,

    /// 查看官方可安装的 Node.js 版本（含 LTS 标记）
    ///
    /// 从 Node.js 官方源获取版本列表，显示每个版本的 LTS 状态。
    /// 列表按版本号降序排列，最新版本在前。
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai node available
    /// ```
    Available,

    /// 安装指定版本的 Node.js（支持 latest / newest / lts / project；Windows/Linux 可自动下载安装）
    ///
    /// 安装 Node.js 到 MeetAI 管理目录。支持特殊版本标识：
    /// - `latest`: 最新稳定版
    /// - `newest`: 最新版本（含 RC）
    /// - `lts`: 最新 LTS 版本
    /// - `project`: 从当前目录或父目录的 `.nvmrc` 文件读取版本
    ///
    /// # 平台差异
    ///
    /// - **Windows/Linux x64/arm64**: 自动下载并安装
    /// - **macOS/其他架构**: 需手动安装后使用 `use` 命令
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai node install lts
    /// meetai node install 20.11.1
    /// meetai node install project  # 从 .nvmrc 读取
    /// ```
    Install {
        /// Node.js 版本号
        version: String,
    },

    /// 切换全局 Node.js 版本（支持 project，从当前目录或父目录的 .nvmrc 读取）
    ///
    /// 设置全局默认 Node.js 版本，更新 shims 指向该版本的二进制文件。
    /// 支持特殊值 `project`，自动从项目根目录的 `.nvmrc` 文件读取版本号。
    ///
    /// # 参数
    ///
    /// - `version`: 已安装的版本号，或 `project`（从 `.nvmrc` 读取）
    ///
    /// # 错误
    ///
    /// - 使用 `project` 但未找到 `.nvmrc` 文件时报错
    /// - 版本未安装时报错
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai node use 20.11.1
    /// meetai node use project  # 从 .nvmrc 自动切换
    /// ```
    Use {
        /// Node.js 版本号
        version: String,
    },

    /// 卸载指定版本
    ///
    /// 从本地删除指定版本的 Node.js 安装目录。
    /// 如果该版本当前正在使用，会先清理当前版本记录和 shims，再继续卸载。
    ///
    /// # 参数
    ///
    /// - `version`: 要卸载的版本号
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai node uninstall 18.0.0
    /// ```
    Uninstall {
        /// Node.js 版本号
        version: String,
    },
}

/// Pip 包管理参数。
///
/// `pip` 子命令的顶层参数，包含一个 `PipAction` 指定具体操作。
#[derive(Parser, Debug)]
pub struct PipArgs {
    #[command(subcommand)]
    pub action: PipAction,
}

/// Pip 子命令动作集合。
///
/// 定义对 Python 包执行的操作：安装、卸载、升级、列表。
/// 所有操作都在当前激活的 Python 环境下执行（通过 `python use` 设置）。
///
/// # 使用说明
///
/// - `install`: 安装包（支持指定版本）
/// - `uninstall`: 卸载包
/// - `upgrade`: 升级包到最新版本
/// - `list`: 列出已安装的包
///
/// # 示例
///
/// ```bash
/// # 安装包
/// meetai pip install requests
/// meetai pip install django==4.2
///
/// # 卸载包
/// meetai pip uninstall requests
///
/// # 升级包
/// meetai pip upgrade requests
///
/// # 列出所有包
/// meetai pip list
/// ```
#[derive(Subcommand, Debug)]
pub enum PipAction {
    /// 安装包
    ///
    /// 使用 pip 安装 Python 包到当前激活的 Python 环境。
    /// 支持指定版本约束（如 `package==1.0.0`）。
    ///
    /// # 参数
    ///
    /// - `package`: 包名（可包含版本约束）
    /// - `version`: 可选，指定版本（`-v` 或 `--version` 参数）
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai pip install requests
    /// meetai pip install django -v 4.2.0
    /// ```
    Install {
        /// 包名
        package: String,
        /// 指定版本
        #[arg(short = 'v', long)]
        version: Option<String>,
    },

    /// 卸载包
    ///
    /// 从当前 Python 环境移除指定的包。
    ///
    /// # 参数
    ///
    /// - `package`: 包名
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai pip uninstall requests
    /// ```
    Uninstall {
        /// 包名
        package: String,
    },

    /// 更新包
    ///
    /// 将指定包升级到最新版本。
    ///
    /// # 参数
    ///
    /// - `package`: 包名
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai pip upgrade requests
    /// ```
    Upgrade {
        /// 包名
        package: String,
    },

    /// 列出已安装的包
    ///
    /// 查询当前 Python 环境中所有已安装的包，显示包名和版本。
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai pip list
    /// ```
    List,
}

/// 虚拟环境管理参数。
///
/// `venv` 子命令的顶层参数，包含一个 `VenvAction` 指定具体操作。
#[derive(Parser, Debug)]
pub struct VenvArgs {
    #[command(subcommand)]
    pub action: VenvAction,
}

/// 虚拟环境子命令动作集合。
///
/// 定义对虚拟环境执行的操作：创建、激活、列出。
/// 虚拟环境用于隔离项目依赖，避免包版本冲突。
///
/// # 使用说明
///
/// - `create`: 创建新的虚拟环境
/// - `activate`: 输出激活命令（需在终端中执行）
/// - `list`: 列出所有虚拟环境
///
/// # 示例
///
/// ```bash
/// # 创建虚拟环境
/// meetai venv create myenv
///
/// # 激活虚拟环境
/// meetai venv activate myenv
///
/// # 列出所有环境
/// meetai venv list
/// ```
#[derive(Subcommand, Debug)]
pub enum VenvAction {
    /// 创建虚拟环境
    ///
    /// 使用当前激活的 Python 版本创建新的虚拟环境。
    /// 创建后会在目标目录生成激活脚本和标记文件。
    ///
    /// # 参数
    ///
    /// - `name`: 虚拟环境名称（用于标识和激活）
    /// - `target_dir`: 虚拟环境创建的目标目录，默认为当前目录（`.`）
    ///
    /// # 示例
    ///
    /// ```bash
    /// # 在当前目录创建名为 "myenv" 的虚拟环境
    /// meetai venv create myenv
    ///
    /// # 在指定目录创建虚拟环境
    /// meetai venv create myenv --target-dir /path/to/project
    /// ```
    Create {
        /// 虚拟环境名称
        name: String,
        /// 目标目录，默认为当前目录
        #[arg(short, long, default_value = ".")]
        target_dir: PathBuf,
    },

    /// 激活虚拟环境
    ///
    /// 输出激活虚拟环境的命令，用户需复制并在终端中执行。
    /// 激活后，终端提示符前会显示 `(env_name)` 标识。
    ///
    /// # 参数
    ///
    /// - `name`: 要激活的虚拟环境名称
    ///
    /// # 平台差异
    ///
    /// - **Windows (PowerShell)**: 输出 `& "path/to/Scripts/Activate.ps1"`
    /// - **Unix/macOS**: 输出 `source path/to/bin/activate`
    ///
    /// # 示例
    ///
    /// ```bash
    /// $ meetai venv activate myenv
    /// 请在终端中执行以下命令来激活虚拟环境 myenv：
    ///   source /path/to/venvs/myenv/bin/activate
    /// 激活后命令提示符前会显示 (myenv)，表示已进入虚拟环境。
    /// ```
    Activate {
        /// 虚拟环境名称
        name: String,
    },

    /// 列出所有虚拟环境
    ///
    /// 查询 `venv_dir` 目录下所有已创建的虚拟环境，按名称排序。
    ///
    /// # 示例
    ///
    /// ```bash
    /// meetai venv list
    /// ```
    List,
}

/// 一键安装参数。
///
/// `quick-install` 子命令的顶层参数，定义环境初始化时的所有选项。
#[derive(Parser, Debug)]
pub struct QuickInstallArgs {
    /// Python 版本，默认为 latest
    ///
    /// 指定要安装的 Python 版本。`latest` 表示安装最新稳定版。
    #[arg(long, default_value = "latest")]
    pub python_version: String,

    /// Pip 版本，默认为 latest
    ///
    /// 指定要安装/升级的 pip 版本。`latest` 表示最新版。
    #[arg(long, default_value = "latest")]
    pub pip_version: String,

    /// 虚拟环境名称，默认为 "default"
    ///
    /// 一键安装时自动创建的虚拟环境名称。
    #[arg(long, default_value = "default")]
    pub venv_name: String,

    /// 是否创建虚拟环境，默认为 true
    ///
    /// 安装完 Python 后是否自动创建虚拟环境。
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub create_venv: bool,

    /// 是否启用自动激活提示，默认为 true
    ///
    /// 完成后是否输出虚拟环境激活命令。
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub auto_activate: bool,

    /// 安装目标目录，默认为当前目录
    ///
    /// 虚拟环境创建的位置，以及项目文件生成的根目录。
    #[arg(long, default_value = ".")]
    pub target_dir: PathBuf,

    /// 是否安装 Node.js（自动安装目前支持 Windows/Linux x64/arm64）
    ///
    /// 是否同时安装 Node.js 运行时。当前支持 Windows/Linux x64/arm64 自动安装。
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_nodejs: bool,

    /// Node.js 版本，默认为 lts（更适合新手与项目默认开发）
    ///
    /// 指定要安装的 Node.js 版本。推荐使用 `lts` 获取长期支持版。
    #[arg(long, default_value = "lts")]
    pub nodejs_version: String,

    /// 是否安装 Java（当前为规划支持能力）
    ///
    /// 是否同时安装 Java 运行时。当前版本为规划功能，尚未实现。
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_java: bool,

    /// Java 版本，默认为 latest
    ///
    /// 指定要安装的 Java 版本（规划中）。
    #[arg(long, default_value = "latest")]
    pub java_version: String,

    /// 是否安装 Go（当前为规划支持能力）
    ///
    /// 是否同时安装 Go 语言运行时。当前版本为规划功能，尚未实现。
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub install_go: bool,

    /// Go 版本，默认为 latest
    ///
    /// 指定要安装的 Go 版本（规划中）。
    #[arg(long, default_value = "latest")]
    pub go_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cli(args: &[&str]) -> MeetAiCli {
        MeetAiCli::try_parse_from(args).expect("cli parsing should succeed")
    }

    /// 测试 quick-install 命令的默认值
    #[test]
    fn quick_install_defaults_create_venv_to_true() {
        let cli = parse_cli(&["meetai", "quick-install"]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(args.create_venv);
        assert_eq!(args.nodejs_version, "lts");
    }

    /// 测试 quick-install 可以禁用虚拟环境创建
    #[test]
    fn quick_install_accepts_create_venv_false() {
        let cli = parse_cli(&["meetai", "quick-install", "--create-venv", "false"]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(!args.create_venv);
    }

    /// 测试 quick-install 可以禁用自动激活提示
    #[test]
    fn quick_install_accepts_auto_activate_false() {
        let cli = parse_cli(&["meetai", "quick-install", "--auto-activate", "false"]);
        let Commands::QuickInstall(args) = cli.command else {
            panic!("expected quick-install command");
        };

        assert!(!args.auto_activate);
    }

    /// 测试 quick-install 接受多运行时标志
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

    /// 测试 runtime install 命令解析
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

    /// 测试 runtime install node 命令解析
    #[test]
    fn runtime_install_node_parses() {
        let cli = parse_cli(&["meetai", "runtime", "install", "node", "20.11.1"]);
        let Commands::Runtime(args) = cli.command else {
            panic!("expected runtime command");
        };

        let RuntimeAction::Install { runtime, version } = args.action else {
            panic!("expected runtime install action");
        };

        assert_eq!(runtime, RuntimeType::NodeJs);
        assert_eq!(version, "20.11.1");
    }

    /// 测试 runtime install nodejs 兼容别名解析
    #[test]
    fn runtime_install_nodejs_alias_parses() {
        let cli = parse_cli(&["meetai", "runtime", "install", "nodejs", "20.11.1"]);
        let Commands::Runtime(args) = cli.command else {
            panic!("expected runtime command");
        };

        let RuntimeAction::Install { runtime, version } = args.action else {
            panic!("expected runtime install action");
        };

        assert_eq!(runtime, RuntimeType::NodeJs);
        assert_eq!(version, "20.11.1");
    }

    /// 测试 runtime install node-js 兼容别名解析
    #[test]
    fn runtime_install_node_js_alias_parses() {
        let cli = parse_cli(&["meetai", "runtime", "install", "node-js", "20.11.1"]);
        let Commands::Runtime(args) = cli.command else {
            panic!("expected runtime command");
        };

        let RuntimeAction::Install { runtime, version } = args.action else {
            panic!("expected runtime install action");
        };

        assert_eq!(runtime, RuntimeType::NodeJs);
        assert_eq!(version, "20.11.1");
    }

    /// 测试 node install 命令解析
    #[test]
    fn node_install_parses() {
        let cli = parse_cli(&["meetai", "node", "install", "20.11.1"]);
        let Commands::Node(args) = cli.command else {
            panic!("expected node command");
        };

        let NodeAction::Install { version } = args.action else {
            panic!("expected node install action");
        };

        assert_eq!(version, "20.11.1");
    }

    /// 测试 node available 命令解析
    #[test]
    fn node_available_parses() {
        let cli = parse_cli(&["meetai", "node", "available"]);
        let Commands::Node(args) = cli.command else {
            panic!("expected node command");
        };

        let NodeAction::Available = args.action else {
            panic!("expected node available action");
        };
    }
}
