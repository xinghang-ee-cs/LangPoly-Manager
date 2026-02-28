use crate::config::Config;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// shims 目录加入永久 PATH 的操作结果
pub enum PathConfigResult {
    /// shims 目录已在用户级永久 PATH 中，无需重复配置
    AlreadyConfigured,
    /// shims 目录本次已成功加入用户级永久 PATH
    JustConfigured,
    /// 自动配置失败，含失败原因（供回退到手动提示时使用）
    Failed(String),
}

/// Python 版本信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PythonVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub path: PathBuf,
}

impl std::fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PythonVersion {
    /// 从版本字符串创建 PythonVersion
    pub fn from_string(version: &str, path: PathBuf) -> Result<Self> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid Python version format: {}", version);
        }

        Ok(Self {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
            path,
        })
    }

    /// 比较版本
    pub fn compare(&self, other: &Self) -> std::cmp::Ordering {
        if self.major != other.major {
            return self.major.cmp(&other.major);
        }
        if self.minor != other.minor {
            return self.minor.cmp(&other.minor);
        }
        self.patch.cmp(&other.patch)
    }
}

/// Python 版本管理器
pub struct PythonVersionManager {
    config: Config,
}

impl PythonVersionManager {
    /// 创建 Python 版本管理器，并确保安装/缓存目录已初始化。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;
        Ok(Self { config })
    }

    /// 列出已安装的 Python 版本
    pub fn list_installed(&self) -> Result<Vec<PythonVersion>> {
        let mut versions = Vec::new();

        if !self.config.python_install_dir.exists() {
            return Ok(versions);
        }

        for entry in fs::read_dir(&self.config.python_install_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(version_str) = name.to_str() {
                        if version_str.starts_with("python-") {
                            if let Some(version) = version_str.strip_prefix("python-") {
                                if let Ok(py_version) =
                                    PythonVersion::from_string(version, path.clone())
                                {
                                    versions.push(py_version);
                                }
                            }
                        }
                    }
                }
            }
        }

        versions.sort_by(|a, b| b.compare(a).reverse());
        Ok(versions)
    }

    /// 设置当前使用的 Python 版本
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|v| v.to_string() == version)
            .with_context(|| {
                format!(
                    "找不到已安装的 Python {} 版本，请先执行: meetai python list 确认版本号",
                    version
                )
            })?;
        let python_exe = Self::python_executable_in_dir(&target.path);
        if !python_exe.exists() {
            anyhow::bail!(
                "Python {} 的可执行文件不存在（{}），请尝试重新安装: meetai python install {}",
                version,
                python_exe.display(),
                version
            );
        }

        let mut config = Config::load()?;
        config.current_python_version = Some(version.to_string());
        config.save()?;

        self.refresh_python_shims(&python_exe)?;

        Ok(())
    }

    /// 获取当前使用的 Python 版本
    pub fn get_current_version(&self) -> Result<Option<String>> {
        let config = Config::load()?;
        Ok(config.current_python_version)
    }

    /// 获取指定版本的 Python 路径
    pub fn get_python_path(&self, version: &str) -> Result<PathBuf> {
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|v| v.to_string() == version)
            .with_context(|| {
                format!(
                    "找不到已安装的 Python {} 版本，请先执行: meetai python list 确认版本号",
                    version
                )
            })?;

        Ok(target.path.clone())
    }

    /// 获取当前选中 Python 的可执行文件路径，并校验文件存在性。
    pub fn current_python_executable(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        let current_version = self
            .get_current_version()?
            .context(missing_selection_message)?;
        let python_path = self.get_python_path(&current_version)?;
        let python_exe = Self::python_executable_in_dir(&python_path);
        if !python_exe.exists() {
            anyhow::bail!(
                "当前已选 Python 的可执行文件不存在：{}。请尝试重新安装或切换版本：meetai python install <version> / meetai runtime use python <version>",
                python_exe.display()
            );
        }
        Ok(python_exe)
    }

    /// 获取 shim 目录路径
    pub fn shims_dir(&self) -> Result<PathBuf> {
        let app_home = self
            .config
            .python_install_dir
            .parent()
            .context("Failed to derive MeetAI app home directory from python_install_dir")?;
        Ok(app_home.join("shims"))
    }

    /// 检查 shim 目录是否已在当前 PATH
    pub fn is_shims_in_path(&self) -> Result<bool> {
        let shims_dir = self.shims_dir()?;
        let Some(path_var) = env::var_os("PATH") else {
            return Ok(false);
        };

        for entry in env::split_paths(&path_var) {
            if Self::paths_equal(&entry, &shims_dir) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 检查当前终端直接执行 `python --version` 是否已命中目标版本
    pub fn python_command_matches_version(&self, expected_version: &str) -> bool {
        let output = match std::process::Command::new("python")
            .args(["--version"])
            .output()
        {
            Ok(output) if output.status.success() => output,
            _ => return false,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Self::parse_python_version_output(&stdout)
            .or_else(|| Self::parse_python_version_output(&stderr))
            .map(|version| version == expected_version)
            .unwrap_or(false)
    }

    fn python_executable_in_dir(install_dir: &Path) -> PathBuf {
        if cfg!(windows) {
            install_dir.join("python.exe")
        } else {
            install_dir.join("bin/python")
        }
    }

    fn refresh_python_shims(&self, python_exe: &Path) -> Result<()> {
        let shims_dir = self.shims_dir()?;
        fs::create_dir_all(&shims_dir).with_context(|| {
            format!("Failed to create shims directory: {}", shims_dir.display())
        })?;

        if cfg!(windows) {
            Self::write_windows_python_shim(&shims_dir, "python.cmd", python_exe, false)?;
            Self::write_windows_python_shim(&shims_dir, "python3.cmd", python_exe, false)?;
            Self::write_windows_python_shim(&shims_dir, "pip.cmd", python_exe, true)?;
        } else {
            Self::write_unix_python_shim(&shims_dir, "python", python_exe, false)?;
            Self::write_unix_python_shim(&shims_dir, "python3", python_exe, false)?;
            Self::write_unix_python_shim(&shims_dir, "pip", python_exe, true)?;
        }

        Ok(())
    }

    fn write_windows_python_shim(
        shims_dir: &Path,
        shim_name: &str,
        python_exe: &Path,
        as_pip: bool,
    ) -> Result<()> {
        let python_exe_str = python_exe.display().to_string();
        let invoke_line = if as_pip {
            r#""%MEETAI_PYTHON_EXE%" -m pip %*"#
        } else {
            r#""%MEETAI_PYTHON_EXE%" %*"#
        };
        let script = format!(
            "@echo off\r\nset \"MEETAI_PYTHON_EXE={python_exe}\"\r\nif not exist \"%MEETAI_PYTHON_EXE%\" (\r\n  echo [meetai] 当前 Python 可执行文件不存在: %MEETAI_PYTHON_EXE% 1>&2\r\n  echo [meetai] 请先执行: meetai runtime use python ^<version^> 1>&2\r\n  exit /b 1\r\n)\r\n{invoke}\r\n",
            python_exe = python_exe_str,
            invoke = invoke_line
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("Failed to write shim file: {}", shim_path.display()))?;
        Ok(())
    }

    fn write_unix_python_shim(
        shims_dir: &Path,
        shim_name: &str,
        python_exe: &Path,
        as_pip: bool,
    ) -> Result<()> {
        let escaped = Self::escape_sh_single_quotes(&python_exe.display().to_string());
        let invoke_line = if as_pip {
            r#"exec "$MEETAI_PYTHON_EXE" -m pip "$@""#
        } else {
            r#"exec "$MEETAI_PYTHON_EXE" "$@""#
        };
        let script = format!(
            "#!/usr/bin/env sh\nMEETAI_PYTHON_EXE='{python_exe}'\nif [ ! -x \"$MEETAI_PYTHON_EXE\" ]; then\n  echo \"[meetai] 当前 Python 可执行文件不存在: $MEETAI_PYTHON_EXE\" >&2\n  echo \"[meetai] 请先执行: meetai runtime use python <version>\" >&2\n  exit 1\nfi\n{invoke}\n",
            python_exe = escaped,
            invoke = invoke_line
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("Failed to write shim file: {}", shim_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms).with_context(|| {
                format!(
                    "Failed to set executable permission on: {}",
                    shim_path.display()
                )
            })?;
        }

        Ok(())
    }

    fn escape_sh_single_quotes(raw: &str) -> String {
        raw.replace('\'', "'\"'\"'")
    }

    fn parse_python_version_output(output: &str) -> Option<String> {
        let trimmed = output.trim();
        let mut parts = trimmed.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("Python"), Some(version)) => Some(version.to_string()),
            _ => None,
        }
    }

    /// 将 shims 目录自动加入用户级永久 PATH（仅首次调用时实际写入）
    pub fn ensure_shims_in_path(&self) -> Result<PathConfigResult> {
        let shims_dir = self.shims_dir()?;
        Ok(Self::ensure_shims_in_path_platform(&shims_dir))
    }

    /// Windows 实现：通过 PowerShell 读写 HKCU\Environment 中的 Path
    #[cfg(windows)]
    fn ensure_shims_in_path_platform(shims_dir: &Path) -> PathConfigResult {
        use std::process::Command;

        let shims_str = shims_dir.to_string_lossy();
        // PowerShell 单引号字符串中，单引号需要双写转义
        let shims_escaped = shims_str.replace('\'', "''");

        // 一条 PowerShell 命令完成：检测 → 按需写入 → 输出结果
        // {{/}} 在 Rust format! 中表示字面量 {/}
        // 注意：PowerShell 5.1 不支持把 if 语句直接内联为 .NET 方法参数，
        // 必须先赋值给中间变量 $np，否则 SetEnvironmentVariable 会静默失败。
        let script = format!(
            "$ErrorActionPreference='Stop';\
$s='{shims}';\
$p=[string][Environment]::GetEnvironmentVariable('Path','User');\
if(($p -split ';') -icontains $s){{Write-Output 'already'}}else{{\
if($p){{$np=$s+';'+$p}}else{{$np=$s}};\
[Environment]::SetEnvironmentVariable('Path',$np,'User');\
Write-Output 'added'}}",
            shims = shims_escaped
        );

        match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout == "already" {
                    PathConfigResult::AlreadyConfigured
                } else {
                    PathConfigResult::JustConfigured
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                PathConfigResult::Failed(if stderr.is_empty() {
                    "PowerShell 执行失败（退出码非零）".to_string()
                } else {
                    stderr
                })
            }
            Err(e) => PathConfigResult::Failed(format!("无法启动 PowerShell: {}", e)),
        }
    }

    /// Unix 实现：向 .bashrc / .zshrc / .bash_profile / .profile 追加 export 语句
    #[cfg(not(windows))]
    fn ensure_shims_in_path_platform(shims_dir: &Path) -> PathConfigResult {
        use std::io::Write;

        let shims_str = shims_dir.to_string_lossy();
        let export_line = format!("\n# Added by MeetAI\nexport PATH=\"{}:$PATH\"\n", shims_str);

        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return PathConfigResult::Failed("无法确定用户主目录".to_string()),
        };

        let candidates = [".bashrc", ".zshrc", ".bash_profile", ".profile"];

        // 任意一个文件中已包含 shims 路径，则视为已配置
        for candidate in &candidates {
            let profile_path = home.join(candidate);
            if !profile_path.exists() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&profile_path) {
                if content.contains(shims_str.as_ref()) {
                    return PathConfigResult::AlreadyConfigured;
                }
            }
        }

        // 未配置：向所有存在的 shell 配置文件追加 export 语句
        let mut written = false;
        for candidate in &candidates {
            let profile_path = home.join(candidate);
            if !profile_path.exists() {
                continue;
            }
            if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&profile_path) {
                if file.write_all(export_line.as_bytes()).is_ok() {
                    written = true;
                }
            }
        }

        if written {
            PathConfigResult::JustConfigured
        } else {
            PathConfigResult::Failed(
                "无法写入任何 shell 配置文件（.bashrc / .zshrc / .profile）".to_string(),
            )
        }
    }

    fn paths_equal(a: &Path, b: &Path) -> bool {
        if cfg!(windows) {
            let left = a.to_string_lossy().replace('/', "\\").to_lowercase();
            let right = b.to_string_lossy().replace('/', "\\").to_lowercase();
            left == right
        } else {
            a == b
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PythonVersionManager;
    use std::path::PathBuf;

    #[test]
    fn parse_python_version_output_supports_trimmed_stdout() {
        let parsed = PythonVersionManager::parse_python_version_output("Python 3.14.3\r\n");
        assert_eq!(parsed.as_deref(), Some("3.14.3"));
    }

    #[test]
    fn parse_python_version_output_rejects_non_standard_prefix() {
        let parsed = PythonVersionManager::parse_python_version_output("CPython 3.14.3");
        assert!(parsed.is_none());
    }

    #[test]
    fn python_executable_in_dir_uses_platform_specific_location() {
        let install_dir = PathBuf::from("python-3.13.2");
        let python_exe = PythonVersionManager::python_executable_in_dir(&install_dir);
        if cfg!(windows) {
            assert!(python_exe.ends_with("python.exe"));
        } else {
            assert!(python_exe.ends_with("bin/python"));
        }
    }
}
