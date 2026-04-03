//! 一键安装器实现。
//!
//! 本模块提供"一键安装"完整流程的编排器，负责串联 Python、pip、Node.js、
//! 虚拟环境以及安装后验证等步骤。
//!
//! 当前行为说明：
//! - Python: 始终安装或切换到请求版本，并激活为当前全局版本
//! - pip: 始终安装或升级
//! - Node.js: 当 `install_nodejs=true` 时安装并激活为当前全局版本
//! - Java / Go: 当对应标志开启时，仅作为 planned runtime 展示，自动安装尚未开放
//! - venv: 当 `create_venv=true` 时创建全局虚拟环境，并在项目目录写入 `.venv` 标记与激活脚本
//!
//! 核心类型：
//! - `QuickInstaller`: 主安装器，协调所有子组件的安装流程
//! - 测试用 Mock 类型：`MockPythonInstaller`、`MockPythonRuntime`、`MockPipManager`、`MockNodeRuntime`、`MockVenvManager`、`MockValidator`
//!
//! 主要流程 (`install` 方法)：
//! 1. **安装 Python**: 始终执行
//!    - 使用 `PythonInstaller` 安装到 `{app_home}/python/python-<version>`
//!    - 采纳系统 Python（如果已安装且版本匹配）
//! 2. **安装/升级 pip**: 始终执行
//!    - `latest` 时执行升级，否则安装指定版本
//! 3. **安装 Node.js** (可选): 如果 `install_nodejs=true`
//!    - 解析版本（如 `latest` / `newest` / `lts` / `project` / 精确版本）
//!    - 安装后切换为当前全局版本
//! 4. **处理 Java / Go** (可选)
//!    - 当前仅保留计划中的运行时版本信息，供摘要和后续流程使用
//! 5. **创建虚拟环境**: 如果 `create_venv=true`
//!    - 在 `{app_home}/venvs/<venv_name>` 创建实体目录
//!    - 在 `<target_dir>` 写入 `.venv` 标记文件和激活脚本（PowerShell / shell）
//! 6. **验证安装**: 运行 `QuickInstallValidator` 检查所有组件
//! 7. **打印摘要**: 显示安装路径、版本信息、激活命令
//!
//! 目录结构：
//! ```text
//! {app_home}/
//! ├── python/                   # MeetAI 管理的 Python 运行时
//! ├── nodejs/
//! │   └── versions/             # MeetAI 管理的 Node.js 运行时（如果启用）
//! ├── venvs/
//! │   └── <venv_name>/          # quick-install 创建的虚拟环境实体目录
//! └── shims/                    # 当前全局版本的命令入口
//!
//! <target_dir>/
//! ├── .venv                     # 指向全局虚拟环境目录的标记文件
//! ├── activate.ps1             # Windows 激活辅助脚本（如果启用）
//! └── activate.sh              # Unix 激活辅助脚本（如果启用）
//! ```
//!
//! 错误处理：
//! - 任何步骤失败都会立即停止并返回 `anyhow::Error`
//! - 失败时保留所有已安装组件，便于调试和重试
//! - 错误消息包含网络诊断建议（如需要）
//!
//! 进度显示：
//! - 使用 `indicatif::ProgressBar` 显示 "🌙 月亮" 风格进度条
//! - 主要步骤显示明确消息（"正在安装 Python..."、"正在创建虚拟环境..."）
//! - 下载过程显示字节数和速度
//!
//! 测试：
//! - 模块内 `mod tests` 包含完整的 mock 测试框架
//! - 验证各运行时安装流程、错误传播、跳过逻辑
//! - 网络故障场景测试（Python 安装失败诊断）

use anyhow::{Context, Result};
use indicatif::ProgressBar;
use log::warn;
use std::path::Path;

use crate::config::Config;
use crate::node::NodeService;
use crate::pip::version::PipVersionManager;
use crate::python::environment::VenvManager;
use crate::python::installer::PythonInstaller;
use crate::python::PythonService;
use crate::quick_install::config::QuickInstallConfig;
use crate::quick_install::validator::QuickInstallValidator;
use crate::utils::guidance::network_diagnostic_tips;
use crate::utils::progress::moon_bar_style;

/// Python 安装器操作 trait（已由 PythonInstaller 直接实现）。
#[async_trait::async_trait]
trait PythonInstallerOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<String>;
}

/// Python 运行时状态操作 trait。
///
/// 负责已安装版本查询、激活与当前版本读取，已由 `PythonService` 直接实现。
trait PythonRuntimeOps: Send + Sync {
    fn list_installed_versions(&self) -> Result<Vec<String>>;
    fn activate_version(&self, version: &str) -> Result<()>;
    fn get_current_version(&self) -> Result<Option<String>>;
}

/// Node.js 运行时操作 trait。
///
/// 负责安装、激活与当前版本读取，已由 `NodeService` 直接实现。
#[async_trait::async_trait]
trait NodeRuntimeOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<String>;
    fn activate_version(&self, version: &str) -> Result<()>;
    fn get_current_version(&self) -> Result<Option<String>>;
}

/// Pip 管理操作 trait（已由 PipVersionManager 直接实现）。
#[async_trait::async_trait]
trait PipVersionOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<()>;
    async fn upgrade(&self) -> Result<()>;
    fn get_version_string(&self) -> Result<String>;
}

/// 虚拟环境管理操作 trait（已由 VenvManager 直接实现）。
#[async_trait::async_trait]
trait VenvManagerOps: Send + Sync {
    async fn create(&self, name: &str, target_dir: &Path) -> Result<()>;
}

/// 安装验证操作 trait（已由 QuickInstallValidator 直接实现）。
#[async_trait::async_trait]
trait QuickInstallValidatorOps: Send + Sync {
    async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()>;
}

// --- 具体类型直接实现对应 trait（删除 adapter 层） ---

#[async_trait::async_trait]
impl PythonInstallerOps for PythonInstaller {
    async fn install(&self, version: &str) -> Result<String> {
        PythonInstaller::install(self, version).await
    }
}

impl PythonRuntimeOps for PythonService {
    fn list_installed_versions(&self) -> Result<Vec<String>> {
        PythonService::list_installed(self)
    }

    fn activate_version(&self, version: &str) -> Result<()> {
        PythonService::activate_version(self, version)
    }

    fn get_current_version(&self) -> Result<Option<String>> {
        PythonService::get_current_version(self)
    }
}

#[async_trait::async_trait]
impl NodeRuntimeOps for NodeService {
    async fn install(&self, version: &str) -> Result<String> {
        NodeService::install(self, version).await
    }

    fn activate_version(&self, version: &str) -> Result<()> {
        NodeService::activate_version(self, version)
    }

    fn get_current_version(&self) -> Result<Option<String>> {
        NodeService::get_current_version(self)
    }
}

#[async_trait::async_trait]
impl PipVersionOps for PipVersionManager {
    async fn install(&self, version: &str) -> Result<()> {
        PipVersionManager::install(self, version).await
    }

    async fn upgrade(&self) -> Result<()> {
        PipVersionManager::upgrade(self).await
    }

    fn get_version_string(&self) -> Result<String> {
        Ok(self.get_version()?.to_string())
    }
}

#[async_trait::async_trait]
impl VenvManagerOps for VenvManager {
    async fn create(&self, name: &str, target_dir: &Path) -> Result<()> {
        VenvManager::create(self, name, target_dir).await
    }
}

#[async_trait::async_trait]
impl QuickInstallValidatorOps for QuickInstallValidator {
    async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()> {
        QuickInstallValidator::verify_installation(self, config).await
    }
}

/// 一键安装器。
///
/// 负责编排 Python / Node.js / Pip / 虚拟环境安装，以及安装后的统一校验。
pub struct QuickInstaller {
    python_installer: Box<dyn PythonInstallerOps>,
    node_runtime: Box<dyn NodeRuntimeOps>,
    pip_manager: Box<dyn PipVersionOps>,
    venv_manager: Box<dyn VenvManagerOps>,
    validator: Box<dyn QuickInstallValidatorOps>,
    python_runtime: Box<dyn PythonRuntimeOps>,
}

impl QuickInstaller {
    /// 创建一键安装器并装配依赖组件。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            python_installer: Box::new(PythonInstaller::new()?),
            node_runtime: Box::new(NodeService::new()?),
            pip_manager: Box::new(PipVersionManager::new()?),
            venv_manager: Box::new(VenvManager::new()?),
            validator: Box::new(QuickInstallValidator::new()),
            python_runtime: Box::new(PythonService::new()?),
        })
    }

    #[cfg(test)]
    fn with_dependencies(
        python_installer: Box<dyn PythonInstallerOps>,
        node_runtime: Box<dyn NodeRuntimeOps>,
        pip_manager: Box<dyn PipVersionOps>,
        venv_manager: Box<dyn VenvManagerOps>,
        validator: Box<dyn QuickInstallValidatorOps>,
        python_runtime: Box<dyn PythonRuntimeOps>,
    ) -> Self {
        Self {
            python_installer,
            node_runtime,
            pip_manager,
            venv_manager,
            validator,
            python_runtime,
        }
    }

    /// 执行一键安装
    pub async fn install(&self, config: &QuickInstallConfig) -> Result<()> {
        let mut total_steps = 3u64; // Python + Pip + Verify
        if config.create_venv {
            total_steps += 1;
        }
        if config.install_nodejs {
            total_steps += 1;
        }
        if config.install_java {
            total_steps += 1;
        }
        if config.install_go {
            total_steps += 1;
        }

        let progress = ProgressBar::new(total_steps);
        progress.set_style(moon_bar_style(
            "{spinner} {elapsed_precise} [{bar:40}] {pos}/{len} {msg}",
        ));

        progress.set_message("🐍 准备安装 Python...");
        self.install_python(config).await.with_context(|| {
            Self::build_step_failure_message("Python 安装阶段失败", config, true)
        })?;
        progress.inc(1);

        progress.set_message("📦 安装/升级 Pip...");
        self.install_pip(config).await.with_context(|| {
            Self::build_step_failure_message("Pip 安装/升级阶段失败", config, true)
        })?;
        progress.inc(1);

        if config.install_nodejs {
            progress.set_message("🟢 安装 Node.js...");
            self.install_nodejs(config).await.with_context(|| {
                Self::build_step_failure_message("Node.js 安装阶段失败", config, true)
            })?;
            progress.inc(1);
        }

        if config.install_java {
            progress.set_message("☕ 安装 Java...");
            self.install_java(config).await.with_context(|| {
                Self::build_step_failure_message("Java 安装阶段失败", config, false)
            })?;
            progress.inc(1);
        }

        if config.install_go {
            progress.set_message("🐹 安装 Go...");
            self.install_go(config).await.with_context(|| {
                Self::build_step_failure_message("Go 安装阶段失败", config, false)
            })?;
            progress.inc(1);
        }

        if config.create_venv {
            progress.set_message("🌱 创建虚拟环境...");
            self.create_venv(config).await.with_context(|| {
                Self::build_step_failure_message("虚拟环境创建阶段失败", config, false)
            })?;
            progress.inc(1);
        }

        progress.set_message("🔍 验证安装结果...");
        self.verify_installation(config)
            .await
            .with_context(|| Self::build_step_failure_message("安装验证阶段失败", config, false))?;
        progress.inc(1);

        progress.finish_with_message("✅ 环境初始化完成！");
        self.print_install_summary(config)?;

        Ok(())
    }

    async fn install_nodejs(&self, config: &QuickInstallConfig) -> Result<()> {
        let installed_version = self.node_runtime.install(&config.nodejs_version).await?;
        self.node_runtime.activate_version(&installed_version)?;
        println!("Node.js {} 已安装并切换为当前版本。", installed_version);
        Ok(())
    }

    async fn install_java(&self, config: &QuickInstallConfig) -> Result<()> {
        self.install_planned_runtime("Java", &config.java_version)
            .await
    }

    async fn install_go(&self, config: &QuickInstallConfig) -> Result<()> {
        self.install_planned_runtime("Go", &config.go_version).await
    }

    async fn install_python(&self, config: &QuickInstallConfig) -> Result<()> {
        let requested_version = config.python_version.as_str();

        if requested_version != "latest" {
            let installed_versions = self.python_runtime.list_installed_versions()?;
            if installed_versions.iter().any(|v| v == requested_version) {
                println!("Python {} 已经安装", requested_version);
                self.python_runtime
                    .activate_version(requested_version)
                    .with_context(|| {
                        Self::build_python_switch_failure_message(requested_version)
                    })?;
                return Ok(());
            }
        }

        let installed_version = self
            .python_installer
            .install(requested_version)
            .await
            .with_context(|| {
                Self::build_python_install_failure_message(requested_version, config)
            })?;
        self.python_runtime
            .activate_version(&installed_version)
            .with_context(|| Self::build_python_switch_failure_message(&installed_version))?;

        Ok(())
    }

    async fn install_pip(&self, config: &QuickInstallConfig) -> Result<()> {
        if config.pip_version == "latest" {
            self.pip_manager.upgrade().await?;
        } else {
            self.pip_manager.install(&config.pip_version).await?;
        }

        Ok(())
    }

    async fn create_venv(&self, config: &QuickInstallConfig) -> Result<()> {
        self.venv_manager
            .create(&config.venv_name, &config.target_dir)
            .await?;
        Ok(())
    }

    async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()> {
        self.validator.verify_installation(config).await?;
        Ok(())
    }

    async fn install_planned_runtime(&self, runtime_name: &str, version: &str) -> Result<()> {
        warn!(
            "{} runtime installer is planned but not implemented yet (requested version: {})",
            runtime_name, version
        );
        println!(
            "{} 的自动安装功能正在开发中，暂时跳过。如果你需要用，可以先手动安装：",
            runtime_name
        );
        match runtime_name {
            "Java" => println!("  https://adoptium.net"),
            "Go" => println!("  https://go.dev/dl"),
            _ => {}
        }
        Ok(())
    }

    fn build_step_failure_message(
        step: &str,
        config: &QuickInstallConfig,
        include_network_tips: bool,
    ) -> String {
        let platform_guidance = if cfg!(windows) {
            "  - meetai runtime install python <version>\n  - meetai python install <version>\n  - meetai python list".to_string()
        } else {
            "  - meetai runtime list python\n  - meetai runtime use python <version>".to_string()
        };
        let mut message = format!(
            "{}。\n参考命令：\n  - meetai quick-install --python-version {} --pip-version {}\n{}",
            step, config.python_version, config.pip_version, platform_guidance
        );

        if include_network_tips {
            message.push('\n');
            message.push_str(network_diagnostic_tips());
        }

        message
    }

    fn build_python_install_failure_message(version: &str, config: &QuickInstallConfig) -> String {
        if cfg!(windows) {
            format!(
                "Python {} 安装失败（quick-install）。\n参考命令：\n  - meetai runtime install python {}\n  - meetai python install {}\n  - meetai quick-install --python-version {}\n{}",
                version,
                version,
                version,
                config.python_version,
                network_diagnostic_tips()
            )
        } else {
            format!(
                "Python {} 安装失败（quick-install）。\n当前平台暂不支持自动安装。\n参考命令：\n  - meetai runtime list python\n  - meetai runtime use python <version>\n  - meetai quick-install --python-version {}\n{}",
                version,
                config.python_version,
                network_diagnostic_tips()
            )
        }
    }

    fn build_python_switch_failure_message(installed_version: &str) -> String {
        format!(
            "Python {} 安装完成，但设置当前版本失败。\n参考命令：\n  - meetai runtime list python\n  - meetai runtime use python {}",
            installed_version, installed_version
        )
    }

    fn print_install_summary(&self, config: &QuickInstallConfig) -> Result<()> {
        let current_version = self.python_runtime.get_current_version()?;

        println!(
            "
安装摘要:"
        );
        println!(
            "  Python 版本: {}",
            current_version.unwrap_or_else(|| "未知".to_string())
        );
        let pip_version_display = self.pip_manager.get_version_string().unwrap_or_else(|_| {
            if config.pip_version == "latest" {
                "未知（安装后获取失败）".to_string()
            } else {
                config.pip_version.clone()
            }
        });
        println!("  Pip 版本: {}", pip_version_display);

        if config.create_venv {
            println!("  虚拟环境: {}", config.venv_name);
            let activate_hint = if config.auto_activate {
                if cfg!(windows) {
                    format!(
                        "已生成（请执行 .\\{}\\Scripts\\Activate.ps1 或 .\\{}\\Scripts\\activate.bat）",
                        config.venv_name, config.venv_name
                    )
                } else {
                    format!("已生成（请执行 source {}/bin/activate）", config.venv_name)
                }
            } else if cfg!(windows) {
                format!(
                    "已生成（未启用自动激活提示；可手动执行 .\\{}\\Scripts\\Activate.ps1 或 .\\{}\\Scripts\\activate.bat）",
                    config.venv_name, config.venv_name
                )
            } else {
                format!(
                    "已生成（未启用自动激活提示；可手动执行 source {}/bin/activate）",
                    config.venv_name
                )
            };
            println!("  激活脚本: {}", activate_hint);
        }

        if config.install_nodejs {
            let node_version_display =
                self.node_runtime.get_current_version().unwrap_or_else(|_| {
                    if matches!(
                        config.nodejs_version.as_str(),
                        "latest" | "newest" | "lts" | "project"
                    ) {
                        Some("未知（安装后获取失败）".to_string())
                    } else {
                        Some(config.nodejs_version.clone())
                    }
                });
            println!(
                "  Node.js 版本: {}",
                node_version_display.unwrap_or_else(|| "未知".to_string())
            );
        }
        if config.install_java {
            println!("  🔜 Java     {}（自动安装即将开放）", config.java_version);
        }
        if config.install_go {
            println!("  🔜 Go       {}（自动安装即将开放）", config.go_version);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    /// 测试用的 PythonInstaller mock（直接实现 trait）
    #[derive(Default)]
    struct MockPythonInstaller {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl PythonInstallerOps for MockPythonInstaller {
        async fn install(&self, version: &str) -> Result<String> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("python_install:{version}"));
            Ok(version.to_string())
        }
    }

    /// 测试用的 PythonRuntime mock
    struct MockPythonRuntime {
        calls: Arc<Mutex<Vec<String>>>,
        installed: Vec<String>,
        current: Mutex<Option<String>>,
    }

    impl PythonRuntimeOps for MockPythonRuntime {
        fn list_installed_versions(&self) -> Result<Vec<String>> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_list_installed".to_string());
            Ok(self.installed.clone())
        }

        fn activate_version(&self, version: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("python_activate:{version}"));
            *self.current.lock().expect("lock current version") = Some(version.to_string());
            Ok(())
        }

        fn get_current_version(&self) -> Result<Option<String>> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_get_current".to_string());
            Ok(self.current.lock().expect("lock current version").clone())
        }
    }

    /// 测试用的 PipManager mock
    #[derive(Default)]
    struct MockPipManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl PipVersionOps for MockPipManager {
        async fn install(&self, version: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("pip_install:{version}"));
            Ok(())
        }

        async fn upgrade(&self) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("pip_upgrade".to_string());
            Ok(())
        }

        fn get_version_string(&self) -> Result<String> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("pip_get_version".to_string());
            Ok("25.0.0".to_string())
        }
    }

    /// 测试用的 NodeRuntime mock
    struct MockNodeRuntime {
        calls: Arc<Mutex<Vec<String>>>,
        current: Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl NodeRuntimeOps for MockNodeRuntime {
        async fn install(&self, version: &str) -> Result<String> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("node_install:{version}"));
            if version == "latest" {
                Ok("22.0.0".to_string())
            } else {
                Ok(version.to_string())
            }
        }

        fn activate_version(&self, version: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("node_activate:{version}"));
            *self.current.lock().expect("lock current version") = Some(version.to_string());
            Ok(())
        }

        fn get_current_version(&self) -> Result<Option<String>> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("node_get_current".to_string());
            Ok(self.current.lock().expect("lock current version").clone())
        }
    }

    /// 测试用的 VenvManager mock
    #[derive(Default)]
    struct MockVenvManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl VenvManagerOps for MockVenvManager {
        async fn create(&self, name: &str, _target_dir: &Path) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("venv_create:{name}"));
            Ok(())
        }
    }

    /// 测试用的 Validator mock
    #[derive(Default)]
    struct MockValidator {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl QuickInstallValidatorOps for MockValidator {
        async fn verify_installation(&self, _config: &QuickInstallConfig) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("validator_verify".to_string());
            Ok(())
        }
    }

    fn make_test_installer(
        installed_versions: Vec<String>,
    ) -> (QuickInstaller, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));

        let installer = make_test_installer_with_python(
            Box::new(MockPythonInstaller {
                calls: calls.clone(),
            }),
            installed_versions,
            calls.clone(),
        );

        (installer, calls)
    }

    fn make_test_installer_with_python(
        python_installer: Box<dyn PythonInstallerOps>,
        installed_versions: Vec<String>,
        calls: Arc<Mutex<Vec<String>>>,
    ) -> QuickInstaller {
        QuickInstaller::with_dependencies(
            python_installer,
            Box::new(MockNodeRuntime {
                calls: calls.clone(),
                current: Mutex::new(None),
            }),
            Box::new(MockPipManager {
                calls: calls.clone(),
            }),
            Box::new(MockVenvManager {
                calls: calls.clone(),
            }),
            Box::new(MockValidator {
                calls: calls.clone(),
            }),
            Box::new(MockPythonRuntime {
                calls: calls.clone(),
                installed: installed_versions,
                current: Mutex::new(None),
            }),
        )
    }

    fn make_config(
        create_venv: bool,
        python_version: &str,
        pip_version: &str,
    ) -> QuickInstallConfig {
        QuickInstallConfig {
            python_version: python_version.to_string(),
            pip_version: pip_version.to_string(),
            venv_name: "test-env".to_string(),
            create_venv,
            target_dir: PathBuf::from("."),
            install_nodejs: false,
            nodejs_version: "latest".to_string(),
            install_java: false,
            java_version: "latest".to_string(),
            install_go: false,
            go_version: "latest".to_string(),
            auto_activate: true,
        }
    }

    #[tokio::test]
    async fn install_skips_venv_when_create_venv_is_false() -> Result<()> {
        let (installer, calls) = make_test_installer(vec![]);
        let config = make_config(false, "3.13.1", "latest");

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            !calls.iter().any(|c| c.starts_with("venv_create:")),
            "venv creation should be skipped, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "validator_verify"),
            "installation verification should still run, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_creates_venv_before_verification_when_enabled() -> Result<()> {
        let (installer, calls) = make_test_installer(vec![]);
        let config = make_config(true, "3.13.1", "latest");

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        let venv_idx = calls
            .iter()
            .position(|c| c == "venv_create:test-env")
            .expect("venv create call should exist");
        let verify_idx = calls
            .iter()
            .position(|c| c == "validator_verify")
            .expect("verify call should exist");
        assert!(
            venv_idx < verify_idx,
            "venv should be created before verify, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_skips_python_download_when_version_already_installed() -> Result<()> {
        let (installer, calls) = make_test_installer(vec!["3.12.9".to_string()]);
        let config = make_config(false, "3.12.9", "latest");

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            !calls.iter().any(|c| c == "python_install:3.12.9"),
            "python installer should be skipped when already installed, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "python_activate:3.12.9"),
            "current python should still be activated, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_uses_pip_install_for_explicit_version() -> Result<()> {
        let (installer, calls) = make_test_installer(vec![]);
        let config = make_config(false, "3.13.1", "24.3");

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|c| c == "pip_install:24.3"),
            "explicit pip version should use install path, calls: {calls:?}"
        );
        assert!(
            !calls.iter().any(|c| c == "pip_upgrade"),
            "explicit pip version should not use upgrade path, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_with_planned_runtimes_keeps_flow_running() -> Result<()> {
        let (installer, calls) = make_test_installer(vec![]);
        let mut config = make_config(false, "3.13.1", "latest");
        config.install_nodejs = true;
        config.nodejs_version = "20.11.1".to_string();
        config.install_java = true;
        config.java_version = "21".to_string();
        config.install_go = true;
        config.go_version = "1.22.2".to_string();

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|c| c == "node_install:20.11.1"),
            "node install should be invoked, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "node_activate:20.11.1"),
            "node current version should be activated through the shared flow, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "validator_verify"),
            "flow should still verify installation, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_surfaces_network_diagnostics_when_python_install_fails() -> Result<()> {
        #[derive(Default)]
        struct MockFailingPythonInstaller {
            calls: Arc<Mutex<Vec<String>>>,
            reason: String,
        }

        #[async_trait::async_trait]
        impl PythonInstallerOps for MockFailingPythonInstaller {
            async fn install(&self, version: &str) -> Result<String> {
                self.calls
                    .lock()
                    .expect("lock call log")
                    .push(format!("python_install_fail:{version}"));
                anyhow::bail!("{}", self.reason);
            }
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let installer = make_test_installer_with_python(
            Box::new(MockFailingPythonInstaller {
                calls: calls.clone(),
                reason:
                    "Download failed for URL https://example.com/python.exe with status: 404 Not Found"
                        .to_string(),
            }),
            vec![],
            calls.clone(),
        );
        let config = make_config(false, "latest", "latest");

        let err = installer
            .install(&config)
            .await
            .expect_err("installation should fail for this test");
        let message = err.to_string();

        assert!(
            message.contains(network_diagnostic_tips()),
            "error should include shared network diagnostics, got: {message}"
        );
        assert!(
            message.contains("meetai quick-install --python-version latest"),
            "error should include quick-install retry command, got: {message}"
        );
        if cfg!(windows) {
            assert!(
                message.contains("meetai runtime install python"),
                "error should include runtime fallback command on Windows, got: {message}"
            );
            assert!(
                message.contains("meetai python install"),
                "error should include python install fallback command on Windows, got: {message}"
            );
        } else {
            assert!(
                message.contains("当前平台暂不支持自动安装"),
                "error should explain platform limitation on non-Windows, got: {message}"
            );
            assert!(
                message.contains("meetai runtime list python"),
                "error should include runtime list guidance on non-Windows, got: {message}"
            );
            assert!(
                message.contains("meetai runtime use python <version>"),
                "error should include runtime use guidance on non-Windows, got: {message}"
            );
            assert!(
                !message.contains("meetai runtime install python"),
                "non-Windows guidance should not suggest unsupported runtime install, got: {message}"
            );
            assert!(
                !message.contains("meetai python install"),
                "non-Windows guidance should not suggest unsupported python install, got: {message}"
            );
        }

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|c| c.starts_with("python_install_fail:")),
            "failing python installer should be invoked, calls: {calls:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn install_latest_delegates_to_python_installer_directly() -> Result<()> {
        let (installer, calls) = make_test_installer(vec![]);
        let config = make_config(false, "latest", "latest");

        installer.install(&config).await?;

        let calls = calls.lock().expect("lock call log").clone();
        assert!(
            calls.iter().any(|c| c == "python_install:latest"),
            "quick-install should delegate latest to PythonInstaller, calls: {calls:?}"
        );
        Ok(())
    }
}
