//! 运行时服务的共享抽象层。
//!
//! 本模块只保留真正跨运行时通用的流程：
//! - PATH / shims 状态检测
//! - 当前版本激活后的引导
//! - 安装、卸载能力的统一调度
//!
//! 边界按职责拆分为三类能力，避免把"安装"和"卸载"强塞进同一个抽象后再次出现
//! 递归委托或职责错位：
//! - `VersionManager`: 版本查询、切换、PATH / shims 管理
//! - `RuntimeInstaller`: 安装能力
//! - `RuntimeUninstaller`: 卸载能力
//!
//! 共享层不再承载各运行时的错误消息模板；Python / Node 仍在各自 service 中保留
//! 面向命令表面的文案，以免共享抽象反向侵入领域语义。
//!
//! # 使用示例
//!
//! ```rust,ignore
//! use crate::runtime::common::{
//!     GenericRuntimeService, RuntimeInstaller, RuntimeUninstaller, VersionManager,
//! };
//! use std::sync::Arc;
//!
//! let version_manager: Arc<dyn VersionManager> = Arc::new(MyVersionManager::new()?);
//! let installer_impl = Arc::new(MyInstaller::new()?);
//! let installer: Arc<dyn RuntimeInstaller> = installer_impl.clone();
//! let uninstaller: Arc<dyn RuntimeUninstaller> = installer_impl;
//! let service = GenericRuntimeService::new(version_manager, installer, uninstaller);
//!
//! let versions = service.list_installed()?;
//! service.install("latest").await?;
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::utils::progress::moon_spinner_style;
use anyhow::Result;
use async_trait::async_trait;
use indicatif::ProgressBar;

/// 共享的 PATH 配置结果枚举。
///
/// 该类型由 `runtime/common.rs` 统一定义，并由 Python/Node 的版本管理器复用。
/// 表示 `ensure_shims_in_path()` 操作的执行结果。
///
/// # 变体说明
///
/// - `AlreadyConfigured`: shims 目录已在用户级永久 PATH 中，无需重复配置
/// - `JustConfigured`: shims 目录本次已成功加入用户级永久 PATH
/// - `Failed(String)`: 自动配置失败，包含失败原因（供回退到手动提示时使用）
///
/// # 使用示例
///
/// ```rust,ignore
/// let result = version_manager.ensure_shims_in_path()?;
/// match result {
///     PathConfigResult::AlreadyConfigured => println!("PATH 已配置"),
///     PathConfigResult::JustConfigured => println!("PATH 已自动配置"),
///     PathConfigResult::Failed(reason) => eprintln!("配置失败：{}", reason),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathConfigResult {
    /// shims 目录已在用户级永久 PATH 中，无需重复配置。
    AlreadyConfigured,
    /// shims 目录本次已成功加入用户级永久 PATH。
    JustConfigured,
    /// 自动配置失败，含失败原因（供回退到手动提示时使用）。
    Failed(String),
}

/// 版本管理器 trait - 管理特定运行时的版本、shims 和 PATH。
///
/// 封装了与 PATH、shims、命令检测相关的操作。这些操作对于所有运行时都是相同的，
/// 因此提取到 trait 中共享实现。此 trait 是 `dyn` safe 的，可用于 trait 对象。
///
/// # 设计说明
///
/// - 所有方法均为同步，因为涉及文件系统和环境变量操作
/// - 版本信息以 `String` 形式传递，避免在 trait 中暴露具体版本类型
/// - 实现者需确保线程安全（`Send + Sync`）
///
/// # 实现示例
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use crate::runtime::common::VersionManager;
///
/// struct MyVersionManager { /* ... */ }
///
/// impl VersionManager for MyVersionManager {
///     fn command_name(&self) -> &'static str { "mycmd" }
///     fn shims_dir(&self) -> Result<PathBuf> { /* ... */ }
///     // ... 其他方法
/// }
///
/// let manager: Arc<dyn VersionManager> = Arc::new(MyVersionManager::new()?);
/// ```
///
/// # 生命周期
///
/// 该 trait 的设计允许在运行时动态切换版本管理器实现，支持热插拔和测试Mock。
pub trait VersionManager: Send + Sync {
    /// 获取运行时命令行工具名称（如 "python"、"node"）。
    ///
    /// 用于检测命令是否可用、生成用户提示等场景。
    ///
    /// # 返回
    ///
    /// 静态字符串，表示命令名称。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// assert_eq!(manager.command_name(), "python");
    /// ```
    fn command_name(&self) -> &'static str;

    /// 获取 shims 目录路径。
    ///
    /// shims 目录存放指向实际运行时版本的代理脚本/可执行文件。
    /// 该目录需要加入系统 PATH 才能让命令全局可用。
    ///
    /// # 返回
    ///
    /// - `Ok(PathBuf)`: shims 目录路径
    /// - `Err`: 获取失败（如配置未初始化）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let shims_dir = manager.shims_dir()?;
    /// println!("shims 目录：{}", shims_dir.display());
    /// ```
    fn shims_dir(&self) -> Result<PathBuf>;

    /// 检查 shims 目录是否已在当前 PATH 中。
    ///
    /// 检测系统环境变量 PATH 是否包含 shims 目录。
    ///
    /// # 返回
    ///
    /// - `Ok(true)`: shims 已在 PATH 中
    /// - `Ok(false)`: shims 不在 PATH 中
    /// - `Err`: 检测失败（如环境变量读取异常）
    ///
    /// # 使用场景
    ///
    /// 在 `use` 命令后判断是否需要提示用户配置 PATH。
    fn is_shims_in_path(&self) -> Result<bool>;

    /// 检查当前终端直接执行 `command_name() --version` 是否命中目标版本。
    ///
    /// 通过执行命令并解析输出来判断当前激活版本是否符合预期。
    /// 用于判断用户是否已能在当前终端直接使用目标版本（即使 shims 不在 PATH 中）。
    ///
    /// # 参数
    ///
    /// - `expected_version`: 期望的版本号字符串
    ///
    /// # 返回
    ///
    /// - `true`: 当前命令指向的版本与期望版本匹配
    /// - `false`: 不匹配或命令不可用
    ///
    /// # 实现说明
    ///
    /// 实现时应执行 `command_name() --version` 并解析输出，与 `expected_version` 比较。
    /// 注意处理命令不存在、输出格式变化等异常情况。
    fn command_matches_version(&self, expected_version: &str) -> bool;

    /// 自动确保 shims 目录加入用户级永久 PATH（仅首次写入）。
    ///
    /// 尝试将 shims 目录添加到系统 PATH 环境变量中。修改是持久的（永久生效），
    /// 但仅在 shims 目录尚未在 PATH 中时执行。
    ///
    /// # 返回
    ///
    /// - `Ok(PathConfigResult::AlreadyConfigured)`: 已配置，无需操作
    /// - `Ok(PathConfigResult::JustConfigured)`: 本次成功配置
    /// - `Err(PathConfigResult::Failed(reason))`: 配置失败，包含失败原因
    ///
    /// # 平台差异
    ///
    /// - **Windows**: 修改注册表 `HKCU\Environment\Path`
    /// - **macOS**: 修改 `~/.zshrc` 或 `~/.bash_profile`
    /// - **Linux**: 修改 `~/.profile` 或 `~/.bashrc`
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// match manager.ensure_shims_in_path()? {
    ///     PathConfigResult::AlreadyConfigured => {}
    ///     PathConfigResult::JustConfigured => println!("PATH 已配置，重启终端生效"),
    ///     PathConfigResult::Failed(reason) => eprintln!("自动配置失败：{}", reason),
    /// }
    /// ```
    fn ensure_shims_in_path(&self) -> Result<PathConfigResult>;

    /// 打印 PATH 配置指导（自动配置失败时调用）。
    ///
    /// 当 `ensure_shims_in_path()` 失败时，向用户输出手动配置 PATH 的步骤说明。
    ///
    /// # 参数
    ///
    /// - `shims_dir`: 需要加入 PATH 的 shims 目录路径
    ///
    /// # 输出格式
    ///
    /// 应输出清晰的步骤指导，包括：
    /// 1. 需要添加的路径
    /// 2. 不同 shell 的配置方法（Windows/macOS/Linux）
    /// 3. 生效方式（重启终端或 source 命令）
    fn print_path_guidance(&self, shims_dir: &Path);

    /// 列出所有已安装版本（返回版本字符串列表）。
    ///
    /// 扫描版本安装目录，返回所有已安装的版本号。
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 已安装版本列表（可能为空）
    /// - `Err`: 读取目录失败
    ///
    /// # 版本排序
    ///
    /// 返回列表应按版本号降序排列（最新版本在前）。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let versions = manager.list_installed()?;
    /// for v in versions {
    ///     println!("- {}", v);
    /// }
    /// ```
    fn list_installed(&self) -> Result<Vec<String>>;

    /// 获取当前激活版本。
    ///
    /// 读取当前指向的版本号（通常从符号链接或配置文件读取）。
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 当前激活的版本号
    /// - `Ok(None)`: 尚未设置激活版本
    /// - `Err`: 读取失败
    fn get_current_version(&self) -> Result<Option<String>>;

    /// 设置当前激活版本。
    ///
    /// 更新 shims 指向，使命令行调用指向指定版本。
    ///
    /// # 参数
    ///
    /// - `version`: 要激活的版本号（必须已安装）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err`: 设置失败（版本不存在、shims 创建失败等）
    ///
    /// # 副作用
    ///
    /// - 更新/创建 shims 目录中的命令代理
    /// - 可能修改配置文件记录当前版本
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// manager.set_current_version("3.11.0")?;
    /// ```
    fn set_current_version(&self, version: &str) -> Result<()>;
}

/// 安装能力 trait。
///
/// trait 方法显式使用 `install_version` 命名，避免与具体类型的固有方法同名后再次出现
/// 递归委托风险。
///
/// # 设计说明
///
/// 安装能力单独拆出，因为：
/// - 卸载并不总是属于"安装器"本身（如 Node.js 由 `NodeVersionManager` 实现卸载）
/// - 避免 `Installer`  trait 承载过多不相关的职责
/// - 支持不同安装策略（自动下载、手动引导、包管理器等）
///
/// # 实现要求
///
/// 实现者必须：
/// - 是 `Send + Sync` 的（可在多线程/异步上下文中安全使用）
/// - 正确处理网络下载、文件解压、路径配置等流程
/// - 安装失败时清理不完整的残留文件
/// - 返回实际安装的版本号（可能与请求版本不同，如 `latest` 解析后）
#[async_trait]
pub trait RuntimeInstaller: Send + Sync {
    /// 安装指定版本，返回实际安装的版本号。
    ///
    /// # 参数
    ///
    /// - `version`: 要安装的版本（如 `"3.11.0"`、`"latest"`）
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 实际安装的版本号（规范化后）
    /// - `Err`: 安装失败（下载失败、校验失败、解压失败等）
    ///
    /// # 特殊值处理
    ///
    /// - `latest`: 应查询最新稳定版并安装
    /// - `lts`: 应安装最新 LTS 版本（如果运行时支持）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let installer: Arc<dyn RuntimeInstaller> = get_installer();
    /// let version = installer.install_version("3.11.0").await?;
    /// println!("已安装 Python {}", version);
    /// ```
    async fn install_version(&self, version: &str) -> Result<String>;
}

/// 卸载能力 trait。
///
/// 卸载并不总是属于"安装器"本身，因此单独拆分为能力边界：
/// - Python 由 `PythonInstaller` 实现
/// - Node.js 由 `NodeVersionManager` 实现
///
/// # 设计理由
///
/// 将安装和卸载分离可以：
/// - 避免强迫不关心安装的组件实现安装逻辑
/// - 支持更灵活的组合（如只读管理器 + 独立卸载器）
/// - 明确职责边界，降低抽象复杂度
///
/// # 实现要求
///
/// 实现者必须：
/// - 是 `Send + Sync` 的
/// - 安全删除版本目录及其所有文件
/// - 如果版本是当前激活版本，应由具体实现决定是拒绝卸载、自动切换，还是先清理当前版本引用
/// - 清理相关的 shims 链接/文件
#[async_trait]
pub trait RuntimeUninstaller: Send + Sync {
    /// 卸载指定版本。
    ///
    /// 从系统中移除指定版本的运行时。卸载前应检查版本是否存在，
    /// 并妥善处理当前激活版本的引用或 shims。
    ///
    /// # 参数
    ///
    /// - `version`: 要卸载的版本号
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 卸载成功
    /// - `Err`: 卸载失败（版本不存在、文件删除失败、权限不足等）
    ///
    /// # 安全要求
    ///
    /// - 必须确认要删除的路径在版本安装目录内，防止路径遍历攻击
    /// - 删除前应二次确认（如通过版本管理器验证）
    /// - 失败时保留已删除的文件，避免部分删除导致数据不一致
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let uninstaller: Arc<dyn RuntimeUninstaller> = get_uninstaller();
    /// uninstaller.uninstall_version("3.10.0").await?;
    /// ```
    async fn uninstall_version(&self, version: &str) -> Result<()>;
}

/// 运行时版本使用 PATH 的状态。
///
/// 描述 `use` 命令执行后，当前终端会话中命令的可用性状态。
/// 用于决定后续是否需要引导用户配置 PATH。
///
/// # 变体说明
///
/// - `ShimsInPath`: shims 目录已在 PATH 中，命令直接可用
/// - `CommandReady`: shims 不在 PATH，但当前终端可直接执行目标版本命令
///   （可能是通过绝对路径或临时 PATH 修改）
/// - `NeedsPathConfiguration`: 需要配置 PATH 才能使用
///
/// # 状态机
///
/// ```text
/// 执行 `use` 命令
///      ↓
/// 检测 shims 是否在 PATH
///      ↓
///   是 → ShimsInPath（无需额外操作）
///      ↓
///   否 → 检测命令是否可用
///            ↓
///         是 → CommandReady（提示重启终端）
///            ↓
///         否 → NeedsPathConfiguration（自动配置或手动指导）
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsePathStatus {
    /// shims 目录已在 PATH 中，直接可用。
    ShimsInPath,
    /// shims 不在 PATH，但当前终端可直接执行目标版本命令。
    CommandReady,
    /// 需要配置 PATH 才能使用。
    NeedsPathConfiguration,
}

/// 自动确保 shims 目录加入 PATH 的执行结果。
///
/// 描述 `ensure_shims_in_path()` 操作的最终结果，用于向用户展示。
///
/// # 变体说明
///
/// - `JustConfigured`: 本次首次配置（写入了永久 PATH）
/// - `AlreadyConfigured`: 之前已配置过（永久 PATH 中已有 shims）
/// - `Failed { reason, shims_dir }`: 配置失败，包含失败原因和 shims 目录路径
///
/// # 使用示例
///
/// ```rust,ignore
/// let result = service.ensure_shims_in_path()?;
/// match result {
///     EnsureShimsResult::JustConfigured => {
///         println!("✅ PATH 已配置，重启终端后生效");
///     }
///     EnsureShimsResult::AlreadyConfigured => {
///         println!("PATH 已配置，重启终端即可");
///     }
///     EnsureShimsResult::Failed { reason, shims_dir } => {
///         println!("自动配置失败：{}", reason);
///         println!("请手动添加 {} 到 PATH", shims_dir.display());
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureShimsResult {
    /// 本次首次配置（写入了永久 PATH）。
    JustConfigured,
    /// 之前已配置过（永久 PATH 中已有 shims）。
    AlreadyConfigured,
    /// 配置失败，包含失败原因和 shims 目录。
    Failed { reason: String, shims_dir: PathBuf },
}

/// 通用运行时服务 - 封装所有共享逻辑。
///
/// 此结构体是 Python/Node/Java 等运行时服务的核心实现，
/// 通过组合三类职责边界来实现共享编排，而不强迫具体运行时接受不自然的抽象。
///
/// 所有与 PATH 配置、状态机、用户引导相关的逻辑都在这里集中实现，
/// 具体的运行时服务只需提供具体的版本管理、安装、卸载能力即可。
///
/// # 设计模式
///
/// 使用 **组合模式** 而非继承：
/// - 通过 `Arc<dyn Trait>` 组合三个能力接口
/// - 运行时服务（如 `PythonService`）委托给本结构体
/// - 支持在运行时动态替换组件（便于测试和扩展）
///
/// # 线程安全
///
/// 所有字段均为 `Arc<dyn Send + Sync>`，`GenericRuntimeService` 本身也是 `Send + Sync`。
///
/// # 示例
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use meetai::runtime::common::{
///     GenericRuntimeService, RuntimeInstaller, RuntimeUninstaller, VersionManager,
/// };
///
/// let version_manager: Arc<dyn VersionManager> = Arc::new(MyVersionManager::new()?);
/// let installer: Arc<dyn RuntimeInstaller> = Arc::new(MyInstaller::new()?);
/// let uninstaller: Arc<dyn RuntimeUninstaller> = Arc::new(MyUninstaller::new()?);
///
/// let service = GenericRuntimeService::new(version_manager, installer, uninstaller);
///
/// // 列出版本
/// let versions = service.list_installed()?;
///
/// // 安装新版本
/// let installed = service.install("latest").await?;
///
/// // 激活版本
/// service.activate_version(&installed)?;
/// ```
pub struct GenericRuntimeService {
    version_manager: Arc<dyn VersionManager>,
    installer: Arc<dyn RuntimeInstaller>,
    uninstaller: Arc<dyn RuntimeUninstaller>,
}

impl GenericRuntimeService {
    /// 创建新的运行时服务。
    ///
    /// # 参数
    ///
    /// - `version_manager`: 版本管理器实现（必须 `Send + Sync`）
    /// - `installer`: 安装器实现（必须 `Send + Sync`）
    /// - `uninstaller`: 卸载器实现（必须 `Send + Sync`）
    ///
    /// # 返回
    ///
    /// 新的 `GenericRuntimeService` 实例。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let service = GenericRuntimeService::new(vm, installer, uninstaller);
    /// ```
    pub fn new(
        version_manager: Arc<dyn VersionManager>,
        installer: Arc<dyn RuntimeInstaller>,
        uninstaller: Arc<dyn RuntimeUninstaller>,
    ) -> Self {
        Self {
            version_manager,
            installer,
            uninstaller,
        }
    }

    /// 列出所有已安装版本。
    ///
    /// 委托给 `version_manager.list_installed()` 实现。
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<String>)`: 已安装版本列表（可能为空）
    /// - `Err`: 查询失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let versions = service.list_installed()?;
    /// println!("已安装版本：{:?}", versions);
    /// ```
    pub fn list_installed(&self) -> Result<Vec<String>> {
        self.version_manager.list_installed()
    }

    /// 安装指定版本。
    ///
    /// 委托给 `installer.install_version()` 异步执行安装。
    ///
    /// # 参数
    ///
    /// - `version`: 要安装的版本（如 `"3.11.0"` 或 `"latest"`）
    ///
    /// # 返回
    ///
    /// - `Ok(String)`: 实际安装的版本号（规范化后）
    /// - `Err`: 安装失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let version = service.install("3.11.0").await?;
    /// println!("已安装版本：{}", version);
    /// ```
    pub async fn install(&self, version: &str) -> Result<String> {
        self.installer.install_version(version).await
    }

    /// 卸载指定版本。
    ///
    /// 委托给 `uninstaller.uninstall_version()` 异步执行卸载。
    ///
    /// # 参数
    ///
    /// - `version`: 要卸载的版本号
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 卸载成功
    /// - `Err`: 卸载失败（版本不存在、正在使用等）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// service.uninstall("3.10.0").await?;
    /// ```
    pub async fn uninstall(&self, version: &str) -> Result<()> {
        self.uninstaller.uninstall_version(version).await
    }

    /// 获取当前激活版本。
    ///
    /// 委托给 `version_manager.get_current_version()`。
    ///
    /// # 返回
    ///
    /// - `Ok(Some(String))`: 当前激活的版本号
    /// - `Ok(None)`: 尚未设置激活版本
    /// - `Err`: 查询失败
    pub fn get_current_version(&self) -> Result<Option<String>> {
        self.version_manager.get_current_version()
    }

    /// 设置当前激活版本。
    ///
    /// 委托给 `version_manager.set_current_version()`，更新 shims 指向。
    ///
    /// # 参数
    ///
    /// - `version`: 要激活的版本号（必须已安装）
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 设置成功
    /// - `Err`: 设置失败
    ///
    /// # 注意
    ///
    /// 此方法仅更新版本指向，**不**包含 PATH 引导。如需完整激活流程（包括
    /// PATH 配置提示），请使用 `activate_version()`。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        self.version_manager.set_current_version(version)
    }

    /// 检测 `use` 命令后当前会话的可用性状态。
    ///
    /// 结合 `is_shims_in_path()` 和 `command_matches_version()` 两个检查，
    /// 分类返回 `UsePathStatus`，用于决定后续操作。
    ///
    /// # 参数
    ///
    /// - `version`: 要检测的版本号
    ///
    /// # 返回
    ///
    /// - `Ok(UsePathStatus)`: 状态分类
    /// - `Err`: 检测失败
    ///
    /// # 状态分类逻辑
    ///
    /// ```text
    /// if shims_in_path {
    ///     UsePathStatus::ShimsInPath
    /// } else if command_ready {
    ///     UsePathStatus::CommandReady
    /// } else {
    ///     UsePathStatus::NeedsPathConfiguration
    /// }
    /// ```
    pub fn detect_use_path_status(&self, version: &str) -> Result<UsePathStatus> {
        let shims_in_path = self.version_manager.is_shims_in_path()?;
        let command_ready = !shims_in_path && self.version_manager.command_matches_version(version);
        Ok(classify_use_path_status(shims_in_path, command_ready))
    }

    /// 自动确保 shims 目录加入 PATH。
    ///
    /// 委托给 `version_manager.ensure_shims_in_path()`，并使用 `map_ensure_shims_result`
    /// 将结果转换为 `EnsureShimsResult`。
    ///
    /// # 返回
    ///
    /// - `Ok(EnsureShimsResult)`: 配置结果
    /// - `Err`: 配置过程出错（包括获取 shims 目录失败）
    pub fn ensure_shims_in_path(&self) -> Result<EnsureShimsResult> {
        let result = self.version_manager.ensure_shims_in_path()?;
        map_ensure_shims_result(result, || self.version_manager.shims_dir())
    }

    /// 完成版本激活，并复用统一的 PATH 引导流程。
    ///
    /// 这是 `use` 命令的完整流程：
    /// 1. 设置当前激活版本（`set_current_version`）
    /// 2. 检测 PATH 状态（`detect_use_path_status`）
    /// 3. 根据状态输出相应的引导信息或自动配置 PATH
    ///
    /// # 参数
    ///
    /// - `version`: 要激活的版本号
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 激活完成
    /// - `Err`: 激活失败（设置版本失败、PATH 配置失败等）
    ///
    /// # 用户引导
    ///
    /// 根据 `UsePathStatus` 输出不同的提示：
    /// - `ShimsInPath`: 提示运行 `--version` 确认
    /// - `CommandReady`: 提示当前终端已可用，但其他终端需重启
    /// - `NeedsPathConfiguration`: 尝试自动配置，失败则输出手动配置指导
    pub fn activate_version(&self, version: &str) -> Result<()> {
        self.set_current_version(version)?;
        self.handle_path_setup(version)
    }

    /// 处理 `use` 命令的后续 PATH 引导流程。
    ///
    /// 根据 `detect_use_path_status()` 的结果，执行相应的用户引导逻辑。
    /// 该方法通常由 `activate_version()` 调用，也可单独使用。
    ///
    /// # 参数
    ///
    /// - `version`: 刚激活的版本号（用于输出提示）
    ///
    /// # 行为
    ///
    /// - `ShimsInPath`: 输出简短的验证提示
    /// - `CommandReady`: 说明当前终端可用，但建议重启其他终端
    /// - `NeedsPathConfiguration`: 尝试自动配置 PATH，失败则输出详细的手动配置步骤
    ///
    /// # 示例输出
    ///
    /// ```text
    /// ✅ 已自动将 shims 目录加入 PATH（永久生效）。
    ///   重启终端后运行 python --version 即可（仅需配置一次）。
    /// ```
    pub fn handle_path_setup(&self, version: &str) -> Result<()> {
        match self.detect_use_path_status(version)? {
            UsePathStatus::ShimsInPath => {
                println!(
                    "  运行 {} --version 即可确认。",
                    self.version_manager.command_name()
                );
            }
            UsePathStatus::CommandReady => {
                println!(
                    "  当前终端已可直接使用目标版本，运行 {} --version 确认。",
                    self.version_manager.command_name()
                );
                println!("  如果后续在其他终端未生效，请重启终端后再试。");
            }
            UsePathStatus::NeedsPathConfiguration => {
                let pb = ProgressBar::new_spinner();
                pb.set_style(moon_spinner_style());
                pb.enable_steady_tick(Duration::from_millis(120));
                pb.set_message("正在配置 PATH...");

                let result = self.ensure_shims_in_path()?;
                pb.finish_and_clear();

                match result {
                    EnsureShimsResult::JustConfigured => {
                        println!("✅ 已自动将 shims 目录加入 PATH（永久生效）。");
                        println!(
                            "  重启终端后运行 {} --version 即可（仅需配置一次）。",
                            self.version_manager.command_name()
                        );
                    }
                    EnsureShimsResult::AlreadyConfigured => {
                        println!(
                            "  重启终端后运行 {} --version 即可生效。",
                            self.version_manager.command_name()
                        );
                    }
                    EnsureShimsResult::Failed { reason, shims_dir } => {
                        println!("自动配置 PATH 失败（{}）。", reason);
                        self.version_manager.print_path_guidance(&shims_dir);
                    }
                }
            }
        }

        Ok(())
    }
}

/// 根据 shims_in_path 和 command_ready 两个布尔值，分类 UsePathStatus。
///
/// 这是 `detect_use_path_status()` 的核心判断逻辑，提取为独立函数便于测试。
///
/// # 参数
///
/// - `shims_in_path`: shims 目录是否在 PATH 中
/// - `command_ready`: 当前终端是否可直接执行目标版本命令
///
/// # 返回
///
/// 对应的 `UsePathStatus` 枚举变体。
///
/// # 逻辑
///
/// ```text
/// shims_in_path = true  → ShimsInPath（最高优先级）
/// shims_in_path = false && command_ready = true  → CommandReady
/// 否则 → NeedsPathConfiguration
/// ```
pub fn classify_use_path_status(shims_in_path: bool, command_ready: bool) -> UsePathStatus {
    if shims_in_path {
        UsePathStatus::ShimsInPath
    } else if command_ready {
        UsePathStatus::CommandReady
    } else {
        UsePathStatus::NeedsPathConfiguration
    }
}

/// 将 PathConfigResult 映射为 EnsureShimsResult，并在失败时调用 loader 获取 shims_dir。
///
/// 用于 `ensure_shims_in_path()` 的结果转换。`PathConfigResult` 来自版本管理器，
/// 但 `EnsureShimsResult::Failed` 需要 `shims_dir` 字段，因此通过 `shims_dir_loader`
/// 延迟获取该值（仅在失败时调用）。
///
/// # 参数
///
/// - `result`: 版本管理器返回的原始结果
/// - `shims_dir_loader`: 延迟加载 shims 目录的闭包（`FnOnce() -> Result<PathBuf>`）
///
/// # 返回
///
/// - `Ok(EnsureShimsResult)`: 映射后的结果
/// - `Err`: `shims_dir_loader` 调用失败
///
/// # 映射规则
///
/// | `PathConfigResult`    | `EnsureShimsResult`              |
/// |-----------------------|----------------------------------|
/// | `AlreadyConfigured`  | `AlreadyConfigured`             |
/// | `JustConfigured`     | `JustConfigured`                |
/// | `Failed(reason)`     | `Failed { reason, shims_dir }`  |
///
/// # 示例
///
/// ```rust,ignore
/// let raw = version_manager.ensure_shims_in_path()?;
/// let mapped = map_ensure_shims_result(raw, || version_manager.shims_dir())?;
/// ```
pub fn map_ensure_shims_result<F>(
    result: PathConfigResult,
    shims_dir_loader: F,
) -> Result<EnsureShimsResult>
where
    F: FnOnce() -> Result<PathBuf>,
{
    let mapped = match result {
        PathConfigResult::JustConfigured => EnsureShimsResult::JustConfigured,
        PathConfigResult::AlreadyConfigured => EnsureShimsResult::AlreadyConfigured,
        PathConfigResult::Failed(reason) => EnsureShimsResult::Failed {
            reason,
            shims_dir: shims_dir_loader()?,
        },
    };
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：当 shims_in_path 为 true 时，优先返回 ShimsInPath（忽略 command_ready）
    #[test]
    fn classify_use_path_status_prefers_shims_in_path() {
        assert_eq!(
            classify_use_path_status(true, false),
            UsePathStatus::ShimsInPath
        );
        assert_eq!(
            classify_use_path_status(true, true),
            UsePathStatus::ShimsInPath
        );
    }

    /// 测试：当 shims 不在 PATH 但命令可用时，返回 CommandReady
    #[test]
    fn classify_use_path_status_marks_command_ready_without_shims() {
        assert_eq!(
            classify_use_path_status(false, true),
            UsePathStatus::CommandReady
        );
    }

    /// 测试：当 shims 不在 PATH 且命令不可用时，需要配置 PATH
    #[test]
    fn classify_use_path_status_requires_path_configuration_when_needed() {
        assert_eq!(
            classify_use_path_status(false, false),
            UsePathStatus::NeedsPathConfiguration
        );
    }

    /// 测试：成功状态直接映射，不调用 loader
    #[test]
    fn map_ensure_shims_result_passes_through_success_states() {
        assert_eq!(
            map_ensure_shims_result(PathConfigResult::JustConfigured, || {
                panic!("shims_dir_loader should not be called")
            })
            .expect("success mapping should not fail"),
            EnsureShimsResult::JustConfigured,
        );
        assert_eq!(
            map_ensure_shims_result(PathConfigResult::AlreadyConfigured, || {
                panic!("shims_dir_loader should not be called")
            })
            .expect("success mapping should not fail"),
            EnsureShimsResult::AlreadyConfigured,
        );
    }

    /// 测试：失败状态保留原因并调用 loader 获取 shims_dir
    #[test]
    fn map_ensure_shims_result_preserves_failure_reason_and_path() {
        let shims_dir = PathBuf::from(".meetai/shims");
        let mapped = map_ensure_shims_result(
            PathConfigResult::Failed("permission denied".to_string()),
            || Ok(shims_dir.clone()),
        )
        .expect("failed mapping should include shims path");
        assert_eq!(
            mapped,
            EnsureShimsResult::Failed {
                reason: "permission denied".to_string(),
                shims_dir
            }
        );
    }
}
