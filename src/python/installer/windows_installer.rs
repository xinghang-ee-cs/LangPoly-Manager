//! Python 安装器的 Windows 平台专用实现。
//!
//! 本模块处理 Windows 上的安装包签名校验、静默安装，以及对系统 Python/
//! `py.exe` 启动器的受信任发现逻辑。
//!
//! 当前安装流程：
//! 1. 校验安装包 Authenticode 签名
//! 2. 执行 `installer.exe /quiet InstallAllUsers=0 TargetDir=<dir>` 静默安装
//! 3. 验证目标目录中的 Python 可执行文件与版本输出
//!
//! 与系统 Python 发现相关的辅助逻辑只信任受控路径，例如：
//! - `C:\Windows\py.exe`
//! - `C:\Windows\System32\py.exe`
//! - `%LOCALAPPDATA%\Programs\Python\Launcher\py.exe`
//!
//! 这些约束用于减少从 PATH 执行未知 `py.exe` 带来的风险。

use super::*;

impl PythonInstaller {
    /// 安装 Python
    pub(super) async fn install_python(
        &self,
        installer_path: &Path,
        version: &str,
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        let install_dir = self.get_install_dir(version);
        std::fs::create_dir_all(&install_dir)?;

        if cfg!(windows) {
            Self::verify_windows_installer_signature(installer_path).with_context(|| {
                format!(
                    "Installer signature verification failed: {}",
                    installer_path.display()
                )
            })?;
            if let Some(pb) = progress {
                pb.set_message(format!(
                    "🔐 已验证安装包签名，开始安装 Python {}...",
                    version
                ));
            }

            let target_arg = format!("TargetDir={}", install_dir.display());
            let install_args = [
                "/quiet".to_string(),
                "InstallAllUsers=0".to_string(),
                "PrependPath=1".to_string(),
                "Include_test=0".to_string(),
                target_arg,
            ];
            let install_args_ref = install_args
                .iter()
                .map(std::string::String::as_str)
                .collect::<Vec<_>>();
            let phase = format!("🔧 正在安装 Python {}", version);

            self.run_windows_installer(
                installer_path,
                &install_args_ref,
                &[0, 1638, 3010],
                progress,
                &phase,
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to run installer executable: {}",
                    installer_path.display()
                )
            })?;
        } else {
            anyhow::bail!("Automatic installation not yet supported on this platform");
        }

        Ok(())
    }

    pub(super) async fn run_windows_installer(
        &self,
        installer_path: &Path,
        args: &[&str],
        accepted_exit_codes: &[i32],
        progress: Option<&ProgressBar>,
        phase: &str,
    ) -> Result<i32> {
        let command_display = Self::format_command(installer_path, args);
        if let Some(pb) = progress {
            pb.set_message(format!("{phase}（启动安装器）..."));
        } else {
            println!("{phase}（启动安装器）...");
        }

        let child = TokioCommand::new(installer_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to execute installer command: {command_display}"))?;
        let mut wait_output = Box::pin(child.wait_with_output());
        let start = Instant::now();
        let mut heartbeat_count = 0u64;
        let output = loop {
            tokio::select! {
                output = &mut wait_output => {
                    break output.with_context(|| format!("Failed while waiting installer command: {command_display}"))?;
                }
                _ = tokio::time::sleep(Duration::from_secs(6)) => {
                    heartbeat_count += 1;
                    let elapsed = Self::format_elapsed(start.elapsed());
                    if let Some(pb) = progress {
                        let len = pb.length().unwrap_or(100);
                        let current = pb.position();
                        let next = (current + 2).min(len.saturating_sub(3));
                        pb.set_position(next);
                        pb.set_message(format!("{phase}（已等待 {elapsed}，请勿关闭窗口）..."));
                        if heartbeat_count == 1 || heartbeat_count.is_multiple_of(3) {
                            pb.println(format!(
                                "{} 已持续 {}，首次安装通常需要 1-3 分钟，请耐心等待。",
                                phase, elapsed
                            ));
                        }
                    } else {
                        println!("{phase} 已持续 {elapsed}，安装过程中短暂无新输出属正常。");
                    }
                }
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        if !accepted_exit_codes.contains(&exit_code) {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "Installer command failed: {}\nexit code: {}\nstdout: {}\nstderr: {}",
                command_display,
                exit_code,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        if exit_code == 3010 {
            println!("安装器提示需要重启系统后部分环境变量才会完全生效。");
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            println!("{stdout}");
        }

        if let Some(pb) = progress {
            pb.set_message(format!(
                "{} 完成（耗时 {}）",
                phase,
                Self::format_elapsed(start.elapsed())
            ));
        }

        Ok(exit_code)
    }

    pub(super) fn format_elapsed(elapsed: Duration) -> String {
        let total_secs = elapsed.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        if hours > 0 {
            format!("{hours:02}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes:02}:{seconds:02}")
        }
    }

    pub(super) fn format_command(program: &Path, args: &[&str]) -> String {
        let mut command = program.display().to_string();
        for arg in args {
            command.push(' ');
            command.push_str(arg);
        }
        command
    }

    pub(super) fn verify_windows_installer_signature(installer_path: &Path) -> Result<()> {
        if !cfg!(windows) {
            return Ok(());
        }

        let powershell = Self::windows_powershell_exe();
        if !powershell.exists() {
            anyhow::bail!(
                "PowerShell executable not found for signature verification: {}",
                powershell.display()
            );
        }

        let script = r#"$sig = Get-AuthenticodeSignature -LiteralPath $env:MEETAI_INSTALLER_PATH
if ($null -eq $sig) {
  Write-Output "UnknownError"
  exit 2
}
Write-Output ($sig.Status.ToString())
if ($null -ne $sig.SignerCertificate) {
  Write-Output $sig.SignerCertificate.Subject
}"#;

        let output = Command::new(&powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("MEETAI_INSTALLER_PATH", installer_path.as_os_str())
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute installer signature verification command with '{}'",
                    powershell.display()
                )
            })?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "Installer signature verification command failed (status: {}).\nstdout: {}\nstderr: {}",
                output.status,
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                },
                if stderr.is_empty() {
                    "<empty>"
                } else {
                    &stderr
                }
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty());
        let status = lines.next().unwrap_or_default();
        let subject = lines.next().unwrap_or_default();
        if status != "Valid" {
            anyhow::bail!(
                "Installer signature is not valid (status: {}). installer: {}",
                status,
                installer_path.display()
            );
        }

        if !Self::is_trusted_python_signer_subject(subject) {
            anyhow::bail!(
                "Unexpected installer signer. expected CN: '{}', actual subject: '{}', installer: {}",
                PYTHON_SIGNER_COMMON_NAME,
                if subject.is_empty() { "<empty>" } else { subject },
                installer_path.display()
            );
        }

        Ok(())
    }

    pub(super) fn is_trusted_python_signer_subject(subject: &str) -> bool {
        let canonical_cn = subject.split(',').find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            if key.trim().eq_ignore_ascii_case("CN") {
                Some(value.trim())
            } else {
                None
            }
        });

        canonical_cn.is_some_and(|cn| cn.eq_ignore_ascii_case(PYTHON_SIGNER_COMMON_NAME))
    }

    pub(super) fn windows_system_root() -> PathBuf {
        Self::resolve_windows_system_root(
            Self::env_path_var("SystemRoot"),
            Self::env_path_var("WINDIR"),
        )
    }

    pub(super) fn resolve_windows_system_root(
        system_root: Option<PathBuf>,
        windir: Option<PathBuf>,
    ) -> PathBuf {
        system_root
            .or(windir)
            .unwrap_or_else(|| PathBuf::from(WINDOWS_SYSTEM_ROOT_DEFAULT))
    }

    pub(super) fn windows_powershell_exe() -> PathBuf {
        Self::windows_system_root()
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    }

    pub(super) fn windows_reg_exe() -> PathBuf {
        Self::windows_system_root().join("System32").join("reg.exe")
    }

    pub(super) fn windows_py_launcher_candidates() -> Vec<PathBuf> {
        Self::build_windows_py_launcher_candidates(
            Self::windows_system_root(),
            Self::env_path_var("LOCALAPPDATA"),
        )
    }

    pub(super) fn find_windows_py_launcher() -> Option<PathBuf> {
        Self::windows_py_launcher_candidates()
            .into_iter()
            .find(|candidate| candidate.exists())
    }

    pub(super) fn build_windows_py_launcher_candidates(
        system_root: PathBuf,
        local_app_data: Option<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::<PathBuf>::new();

        candidates.push(system_root.join("py.exe"));
        candidates.push(system_root.join("System32").join("py.exe"));

        if let Some(local_app_data) = local_app_data {
            candidates.push(
                local_app_data
                    .join("Programs")
                    .join("Python")
                    .join("Launcher")
                    .join("py.exe"),
            );
        }

        let mut dedup = std::collections::HashSet::<String>::new();
        candidates.retain(|path| {
            dedup.insert(
                path.to_string_lossy()
                    .replace('/', "\\")
                    .trim_end_matches('\\')
                    .to_ascii_lowercase(),
            )
        });
        candidates
    }

    pub(super) fn trusted_python_install_roots() -> Vec<PathBuf> {
        Self::build_trusted_python_install_roots(
            Self::env_path_var("LOCALAPPDATA"),
            home_dir(),
            Self::env_path_var("ProgramFiles"),
            Self::env_path_var("ProgramFiles(x86)"),
        )
    }

    pub(super) fn build_trusted_python_install_roots(
        local_app_data: Option<PathBuf>,
        home: Option<PathBuf>,
        program_files: Option<PathBuf>,
        program_files_x86: Option<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut roots = Vec::<PathBuf>::new();

        if let Some(local) = local_app_data {
            roots.push(local.join("Programs").join("Python"));
        } else if let Some(home) = home {
            roots.push(
                home.join("AppData")
                    .join("Local")
                    .join("Programs")
                    .join("Python"),
            );
        }

        roots.push(program_files.unwrap_or_else(|| PathBuf::from(WINDOWS_PROGRAM_FILES_DEFAULT)));
        roots.push(
            program_files_x86.unwrap_or_else(|| PathBuf::from(WINDOWS_PROGRAM_FILES_X86_DEFAULT)),
        );

        let mut dedup = HashSet::<String>::new();
        roots.retain(|path| dedup.insert(path.to_string_lossy().to_ascii_lowercase()));
        roots
    }

    pub(super) fn env_path_var(name: &str) -> Option<PathBuf> {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    pub(super) fn normalize_path(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    pub(super) fn is_path_within_root(candidate: &Path, root: &Path) -> bool {
        if cfg!(windows) {
            let candidate_str = candidate
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            let root_str = root
                .to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            candidate_str == root_str || candidate_str.starts_with(&(root_str + "\\"))
        } else {
            candidate == root || candidate.starts_with(root)
        }
    }

    pub(super) fn is_trusted_system_python_dir(
        candidate: &Path,
        trusted_roots: &[PathBuf],
    ) -> bool {
        if trusted_roots.is_empty() {
            return false;
        }

        let normalized_candidate = Self::normalize_path(candidate);
        trusted_roots.iter().any(|root| {
            let normalized_root = Self::normalize_path(root);
            Self::is_path_within_root(&normalized_candidate, &normalized_root)
        })
    }

    pub(super) fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
        if metadata.file_type().is_symlink() {
            return true;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & WINDOWS_REPARSE_POINT_ATTRIBUTE != 0
        }

        #[cfg(not(windows))]
        {
            false
        }
    }

    pub(super) fn python_output_matches_requested_version(
        output: &str,
        version: &str,
    ) -> Result<bool> {
        let pattern = format!(r"(?m)^Python\s+{}(?:\s|$)", regex::escape(version));
        let regex = Regex::new(&pattern)
            .with_context(|| format!("Failed to compile Python version match regex: {pattern}"))?;
        Ok(regex.is_match(output))
    }
}
