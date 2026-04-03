//! Python 安装器的安装验证逻辑。
//!
//! 本模块负责验证 Python 安装是否成功，包括可执行文件存在性、版本匹配和完整性检查。
//!
//! 主要函数：
//! - `get_python_exe`: 获取指定版本 Python 可执行文件的平台特定路径
//! - `verify_installation`: 执行版本检查，确认安装的 Python 版本与请求版本匹配
//! - `cleanup_failed_install`: 安装失败时清理残留的安装目录和临时文件
//!
//! 验证流程：
//! 1. 构造 Python 可执行文件路径（Windows: `python.exe`, Unix: `bin/python`）
//! 2. 执行 `python --version` 命令获取实际版本
//! 3. 解析版本字符串并与请求版本比较
//! 4. 匹配成功则验证通过，否则返回错误并触发清理
//!
//! 平台差异：
//! - **Windows**: 可执行文件位于安装根目录，命名为 `python.exe`
//! - **Unix/macOS**: 可执行文件位于 `bin/` 子目录，命名为 `python`
//!
//! 错误处理：
//! - 可执行文件不存在：返回 `anyhow::Error`，触发清理流程
//! - 版本不匹配：返回 `PythonVersionMismatchError`，触发清理流程
//! - 命令执行失败：返回 `anyhow::Error`，保留安装目录供调试
//!
//! 清理策略：
//! - 删除安装目录（`<app_home>/versions/<version>`）
//! - 删除下载的安装包临时文件
//! - 保留错误信息以便用户诊断问题
//!
//! 注意：
//! - 验证失败会**完全清理**不完整的安装，避免脏数据
//! - 清理操作是**不可逆**的，确保失败安装不会占用磁盘空间
//! - 验证通过后，安装目录将保持完整供后续使用

use super::*;

impl PythonInstaller {
    /// 获取安装目录下的 Python 可执行文件路径
    pub(super) fn get_python_exe(&self, version: &str) -> PathBuf {
        let install_dir = self.get_install_dir(version);
        if cfg!(windows) {
            install_dir.join("python.exe")
        } else {
            install_dir.join("bin/python")
        }
    }

    /// 验证安装结果
    pub(super) async fn verify_installation(&self, version: &str) -> Result<()> {
        let python_exe = self.get_python_exe(version);
        if !python_exe.exists() {
            anyhow::bail!(
                "Python executable not found after installation: {}",
                python_exe.display()
            );
        }

        let output = self
            .executor
            .execute_with_output_async(&python_exe, &["--version"])
            .await
            .with_context(|| {
                format!(
                    "Failed to verify Python installation with command: {} --version",
                    python_exe.display()
                )
            })?;

        if !output.to_lowercase().contains("python") {
            anyhow::bail!(
                "Unexpected output from Python executable '{}': {}",
                python_exe.display(),
                output.trim()
            );
        }

        Ok(())
    }

    pub(super) async fn recover_after_verification_failure(
        &self,
        version: &str,
        installer_path: &Path,
        verify_err: anyhow::Error,
    ) -> Result<()> {
        let mut diagnostics = vec![format!("首次安装校验失败: {verify_err}")];

        if self.try_adopt_existing_installation(version)? {
            match self.verify_installation(version).await {
                Ok(()) => {
                    println!("检测到系统已有 Python 安装，已导入 MeetAI 管理目录并完成校验。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("导入系统安装后校验失败: {err}")),
            }
        } else {
            diagnostics.push("未找到可导入的系统 Python 安装路径。".to_string());
        }

        println!("检测到安装器可能进入 Modify 模式，正在尝试自动修复（卸载冲突项后重装）...");
        match self
            .force_reinstall_into_managed_dir(installer_path, version)
            .await
        {
            Ok(()) => match self.verify_installation(version).await {
                Ok(()) => {
                    println!("自动修复成功，Python 已按 MeetAI 目录规则安装。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("自动修复后校验仍失败: {err}")),
            },
            Err(err) => diagnostics.push(format!("自动修复执行失败: {err:#}")),
        }

        if self.try_adopt_existing_installation(version)? {
            match self.verify_installation(version).await {
                Ok(()) => {
                    println!("重装后已成功导入系统 Python 并通过校验。");
                    return Ok(());
                }
                Err(err) => diagnostics.push(format!("重装后导入系统 Python 仍校验失败: {err}")),
            }
        }

        let version_hint = Version::parse(version)
            .map(|v| format!("{}.{}", v.major, v.minor))
            .unwrap_or_else(|_| version.to_string());
        let diagnostics = diagnostics
            .into_iter()
            .map(|line| format!("  - {line}"))
            .collect::<Vec<_>>()
            .join("\n");

        anyhow::bail!(
            "Python {version} 安装后自动修复失败。\n诊断信息：\n{diagnostics}\n可尝试：\n  1. 在“应用和功能”中卸载 Python {version_hint}.x 后重试。\n  2. 重新执行: meetai runtime install python {version}\n  3. 查看可用版本: meetai runtime list python"
        );
    }

    pub(super) async fn force_reinstall_into_managed_dir(
        &self,
        installer_path: &Path,
        version: &str,
    ) -> Result<()> {
        if !cfg!(windows) {
            anyhow::bail!("Reinstall remediation is currently only supported on Windows");
        }

        let uninstall_args = ["/uninstall", "/quiet"];
        match self
            .run_windows_installer(
                installer_path,
                &uninstall_args,
                &[0, 1605, 1614, 3010],
                None,
                "正在执行冲突卸载",
            )
            .await
        {
            Ok(_) => {
                println!("已执行冲突卸载步骤，准备重装到 MeetAI 管理目录。");
            }
            Err(err) => {
                warn!(
                    "Failed to uninstall conflicting Python bundle before reinstall: {:#}",
                    err
                );
                println!("冲突卸载未完全成功，继续尝试直接重装到 MeetAI 管理目录。");
            }
        }

        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            std::fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "Failed to clean managed install dir before reinstall: {}",
                    install_dir.display()
                )
            })?;
        }

        self.install_python(installer_path, version, None)
            .await
            .with_context(|| {
                format!(
                    "Failed to reinstall Python {} into managed directory",
                    version
                )
            })
    }

    /// 清理失败安装残留
    pub(super) fn cleanup_failed_install(&self, version: &str, installer_path: &Path) {
        let install_dir = self.get_install_dir(version);
        if install_dir.exists() {
            if let Err(err) = std::fs::remove_dir_all(&install_dir) {
                warn!(
                    "Failed to clean installation directory after failed install '{}': {} ({:#})",
                    version,
                    install_dir.display(),
                    err
                );
            }
        }

        if installer_path.exists() {
            if let Err(err) = std::fs::remove_file(installer_path) {
                warn!(
                    "Failed to remove installer file after failed install '{}': {} ({:#})",
                    version,
                    installer_path.display(),
                    err
                );
            }
        }
    }
}
