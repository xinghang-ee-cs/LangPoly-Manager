use anyhow::{Context, Result};
use async_trait::async_trait;
use indicatif::ProgressBar;
use log::warn;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::node::NodeService;
use crate::pip::version::PipVersionManager;
use crate::python::environment::VenvManager;
use crate::python::installer::PythonInstaller;
use crate::python::version::PythonVersionManager;
use crate::quick_install::config::QuickInstallConfig;
use crate::quick_install::validator::QuickInstallValidator;
use crate::utils::guidance::{network_diagnostic_tips, print_python_path_guidance};
use crate::utils::progress::moon_bar_style;

#[async_trait]
trait PythonInstallerOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<String>;
}

trait PythonVersionOps: Send + Sync {
    fn list_installed_versions(&self) -> Result<Vec<String>>;
    fn set_current_version(&self, version: &str) -> Result<()>;
    fn get_current_version(&self) -> Result<Option<String>>;
    fn is_shims_in_path(&self) -> Result<bool>;
    fn shims_dir(&self) -> Result<PathBuf>;
}

#[async_trait]
trait NodeVersionOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<String>;
    fn set_current_version(&self, version: &str) -> Result<()>;
    fn get_current_version(&self) -> Result<Option<String>>;
}

#[async_trait]
trait PipVersionOps: Send + Sync {
    async fn install(&self, version: &str) -> Result<()>;
    async fn upgrade(&self) -> Result<()>;
    fn get_version_string(&self) -> Result<String>;
}

#[async_trait]
trait VenvManagerOps: Send + Sync {
    async fn create(&self, name: &str, target_dir: &Path) -> Result<()>;
}

#[async_trait]
trait QuickInstallValidatorOps: Send + Sync {
    async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()>;
}

struct PythonInstallerAdapter {
    inner: PythonInstaller,
}

#[async_trait]
impl PythonInstallerOps for PythonInstallerAdapter {
    async fn install(&self, version: &str) -> Result<String> {
        self.inner.install(version).await
    }
}

struct PythonVersionManagerAdapter {
    inner: PythonVersionManager,
}

impl PythonVersionOps for PythonVersionManagerAdapter {
    fn list_installed_versions(&self) -> Result<Vec<String>> {
        Ok(self
            .inner
            .list_installed()?
            .into_iter()
            .map(|v| v.to_string())
            .collect())
    }

    fn set_current_version(&self, version: &str) -> Result<()> {
        self.inner.set_current_version(version)
    }

    fn get_current_version(&self) -> Result<Option<String>> {
        self.inner.get_current_version()
    }

    fn is_shims_in_path(&self) -> Result<bool> {
        self.inner.is_shims_in_path()
    }

    fn shims_dir(&self) -> Result<PathBuf> {
        self.inner.shims_dir()
    }
}

struct PipVersionManagerAdapter {
    inner: PipVersionManager,
}

struct NodeVersionManagerAdapter {
    inner: NodeService,
}

#[async_trait]
impl NodeVersionOps for NodeVersionManagerAdapter {
    async fn install(&self, version: &str) -> Result<String> {
        self.inner.install(version).await
    }

    fn set_current_version(&self, version: &str) -> Result<()> {
        self.inner.set_current_version(version)
    }

    fn get_current_version(&self) -> Result<Option<String>> {
        self.inner.get_current_version()
    }
}

#[async_trait]
impl PipVersionOps for PipVersionManagerAdapter {
    async fn install(&self, version: &str) -> Result<()> {
        self.inner.install(version).await
    }

    async fn upgrade(&self) -> Result<()> {
        self.inner.upgrade().await
    }

    fn get_version_string(&self) -> Result<String> {
        Ok(self.inner.get_version()?.to_string())
    }
}

struct VenvManagerAdapter {
    inner: VenvManager,
}

#[async_trait]
impl VenvManagerOps for VenvManagerAdapter {
    async fn create(&self, name: &str, target_dir: &Path) -> Result<()> {
        self.inner.create(name, target_dir).await
    }
}

struct QuickInstallValidatorAdapter {
    inner: QuickInstallValidator,
}

#[async_trait]
impl QuickInstallValidatorOps for QuickInstallValidatorAdapter {
    async fn verify_installation(&self, config: &QuickInstallConfig) -> Result<()> {
        self.inner.verify_installation(config).await
    }
}

/// 一键安装器
pub struct QuickInstaller {
    python_installer: Box<dyn PythonInstallerOps>,
    node_manager: Box<dyn NodeVersionOps>,
    pip_manager: Box<dyn PipVersionOps>,
    venv_manager: Box<dyn VenvManagerOps>,
    validator: Box<dyn QuickInstallValidatorOps>,
    version_manager: Box<dyn PythonVersionOps>,
}

impl QuickInstaller {
    /// 创建一键安装器并装配依赖组件。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            python_installer: Box::new(PythonInstallerAdapter {
                inner: PythonInstaller::new()?,
            }),
            node_manager: Box::new(NodeVersionManagerAdapter {
                inner: NodeService::new()?,
            }),
            pip_manager: Box::new(PipVersionManagerAdapter {
                inner: PipVersionManager::new()?,
            }),
            venv_manager: Box::new(VenvManagerAdapter {
                inner: VenvManager::new()?,
            }),
            validator: Box::new(QuickInstallValidatorAdapter {
                inner: QuickInstallValidator::new(),
            }),
            version_manager: Box::new(PythonVersionManagerAdapter {
                inner: PythonVersionManager::new()?,
            }),
        })
    }

    #[cfg(test)]
    fn with_dependencies(
        python_installer: Box<dyn PythonInstallerOps>,
        node_manager: Box<dyn NodeVersionOps>,
        pip_manager: Box<dyn PipVersionOps>,
        venv_manager: Box<dyn VenvManagerOps>,
        validator: Box<dyn QuickInstallValidatorOps>,
        version_manager: Box<dyn PythonVersionOps>,
    ) -> Self {
        Self {
            python_installer,
            node_manager,
            pip_manager,
            venv_manager,
            validator,
            version_manager,
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
        let installed_version = self.node_manager.install(&config.nodejs_version).await?;
        self.node_manager.set_current_version(&installed_version)?;
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
            let installed_versions = self.version_manager.list_installed_versions()?;
            if installed_versions.iter().any(|v| v == requested_version) {
                println!("Python {} 已经安装", requested_version);
                self.version_manager
                    .set_current_version(requested_version)?;
                self.print_python_switch_guidance()?;
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
        self.version_manager
            .set_current_version(&installed_version)
            .with_context(|| Self::build_python_switch_failure_message(&installed_version))?;
        self.print_python_switch_guidance()?;

        Ok(())
    }

    fn print_python_switch_guidance(&self) -> Result<()> {
        let shims_in_path = self.version_manager.is_shims_in_path()?;
        let shims_dir = self.version_manager.shims_dir()?;
        print_python_path_guidance(shims_in_path, &shims_dir);
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
            "{} 暂未开放自动安装，跳过此步骤。如需使用，请访问官网手动安装：",
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
        let current_version = self.version_manager.get_current_version()?;

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
                self.node_manager.get_current_version().unwrap_or_else(|_| {
                    if config.nodejs_version == "latest" {
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
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockPythonInstaller {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl PythonInstallerOps for MockPythonInstaller {
        async fn install(&self, version: &str) -> Result<String> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("python_install:{version}"));
            Ok(version.to_string())
        }
    }

    struct MockFailingPythonInstaller {
        calls: Arc<Mutex<Vec<String>>>,
        reason: String,
    }

    #[async_trait]
    impl PythonInstallerOps for MockFailingPythonInstaller {
        async fn install(&self, version: &str) -> Result<String> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("python_install_fail:{version}"));
            anyhow::bail!("{}", self.reason);
        }
    }

    struct MockPythonVersions {
        calls: Arc<Mutex<Vec<String>>>,
        installed: Vec<String>,
        current: Mutex<Option<String>>,
    }

    impl PythonVersionOps for MockPythonVersions {
        fn list_installed_versions(&self) -> Result<Vec<String>> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_list_installed".to_string());
            Ok(self.installed.clone())
        }

        fn set_current_version(&self, version: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("python_set_current:{version}"));
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

        fn is_shims_in_path(&self) -> Result<bool> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_is_shims_in_path".to_string());
            Ok(true)
        }

        fn shims_dir(&self) -> Result<PathBuf> {
            self.calls
                .lock()
                .expect("lock call log")
                .push("python_shims_dir".to_string());
            Ok(PathBuf::from(".meetai/shims"))
        }
    }

    #[derive(Default)]
    struct MockPipManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    struct MockNodeManager {
        calls: Arc<Mutex<Vec<String>>>,
        current: Mutex<Option<String>>,
    }

    #[async_trait]
    impl NodeVersionOps for MockNodeManager {
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

        fn set_current_version(&self, version: &str) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("node_set_current:{version}"));
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

    #[async_trait]
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

    #[derive(Default)]
    struct MockVenvManager {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl VenvManagerOps for MockVenvManager {
        async fn create(&self, name: &str, _target_dir: &Path) -> Result<()> {
            self.calls
                .lock()
                .expect("lock call log")
                .push(format!("venv_create:{name}"));
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockValidator {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
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
            Box::new(MockNodeManager {
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
            Box::new(MockPythonVersions {
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
            calls.iter().any(|c| c == "python_set_current:3.12.9"),
            "current python should still be set, calls: {calls:?}"
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
            calls.iter().any(|c| c == "node_set_current:20.11.1"),
            "node current version should be updated, calls: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "validator_verify"),
            "flow should still verify installation, calls: {calls:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_surfaces_network_diagnostics_when_python_install_fails() -> Result<()> {
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
            message.contains("网络诊断建议"),
            "error should include network diagnostics, got: {message}"
        );
        assert!(
            message.contains("HTTP_PROXY") && message.contains("HTTPS_PROXY"),
            "error should include proxy hints, got: {message}"
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
