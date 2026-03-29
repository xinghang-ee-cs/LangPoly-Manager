use crate::config::Config;
use anyhow::{Context, Result};
use semver::Version;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Node.js 版本信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeVersion {
    pub version: Version,
    pub path: PathBuf,
}

/// shims 目录加入永久 PATH 的操作结果。
pub enum PathConfigResult {
    /// shims 目录已在用户级永久 PATH 中，无需重复配置。
    AlreadyConfigured,
    /// shims 目录本次已成功加入用户级永久 PATH。
    JustConfigured,
    /// 自动配置失败，含失败原因（供回退到手动提示时使用）。
    Failed(String),
}
impl std::fmt::Display for NodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl NodeVersion {
    fn from_dir_name(dir_name: &str, path: PathBuf) -> Option<Self> {
        let version = Self::extract_version(dir_name)?;
        Some(Self { version, path })
    }

    fn extract_version(dir_name: &str) -> Option<Version> {
        if let Ok(version) = Version::parse(dir_name) {
            return Some(version);
        }

        if let Some(stripped) = dir_name.strip_prefix('v') {
            if let Ok(version) = Version::parse(stripped) {
                return Some(version);
            }
        }

        if let Some(stripped) = dir_name.strip_prefix("node-v") {
            let token = stripped.split('-').next()?;
            if let Ok(version) = Version::parse(token) {
                return Some(version);
            }
        }

        None
    }
}

/// Node.js 版本管理器。
pub struct NodeVersionManager {
    config: Config,
}

impl NodeVersionManager {
    /// 创建 Node.js 版本管理器。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let manager = Self { config };
        manager.ensure_node_dirs()?;
        Ok(manager)
    }

    /// 列出已安装的 Node.js 版本（按版本从高到低）。
    pub fn list_installed(&self) -> Result<Vec<NodeVersion>> {
        let versions_dir = self.versions_dir()?;
        if !versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&versions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(version) = NodeVersion::from_dir_name(dir_name, path.clone()) {
                versions.push(version);
            }
        }

        versions.sort_by(|a, b| b.version.cmp(&a.version));
        Ok(versions)
    }

    /// 设置当前使用的 Node.js 版本，并刷新 shims。
    pub fn set_current_version(&self, version: &str) -> Result<()> {
        let normalized = Self::normalize_version_token(version)
            .with_context(|| format!("Node.js 版本号格式不正确：{}", version))?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == normalized)
            .with_context(|| {
                format!(
                    "找不到已安装的 Node.js {} 版本，请先执行: meetai node list 确认版本号",
                    version
                )
            })?;

        let node_exe = Self::node_executable_in_dir(&target.path);
        if !node_exe.exists() {
            anyhow::bail!(
                "Node.js {} 的可执行文件不存在（{}），请尝试重新安装。",
                normalized,
                node_exe.display()
            );
        }

        let npm_exe = Self::npm_executable_in_dir(&target.path);
        let npx_exe = Self::npx_executable_in_dir(&target.path);

        fs::write(self.current_version_file()?, &normalized)
            .context("写入当前 Node.js 版本失败")?;
        self.refresh_node_shims(&node_exe, &npm_exe, &npx_exe)?;

        Ok(())
    }

    /// 获取当前 Node.js 版本。
    pub fn get_current_version(&self) -> Result<Option<String>> {
        let file_path = self.current_version_file()?;
        if !file_path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&file_path).context("读取当前 Node.js 版本失败")?;
        let version = raw.trim().to_string();
        if version.is_empty() {
            return Ok(None);
        }
        Ok(Some(version))
    }

    /// 获取当前选中 Node.js 可执行文件路径，并校验存在性。
    pub fn current_node_executable(
        &self,
        missing_selection_message: &'static str,
    ) -> Result<PathBuf> {
        let current_version = self
            .get_current_version()?
            .context(missing_selection_message)?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == current_version)
            .with_context(|| {
                format!(
                    "当前 Node.js 版本 {} 不存在，请先执行: meetai node list",
                    current_version
                )
            })?;

        let node_exe = Self::node_executable_in_dir(&target.path);
        if !node_exe.exists() {
            anyhow::bail!(
                "当前 Node.js 可执行文件不存在：{}。请尝试重新安装或切换版本：meetai node use <version>",
                node_exe.display()
            );
        }
        Ok(node_exe)
    }

    /// 卸载指定 Node.js 版本。
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let normalized = Self::normalize_version_token(version)
            .with_context(|| format!("Node.js 版本号格式不正确：{}", version))?;
        let versions = self.list_installed()?;
        let target = versions
            .iter()
            .find(|item| item.version.to_string() == normalized)
            .with_context(|| {
                format!(
                    "找不到已安装的 Node.js {} 版本，请先执行: meetai node list 确认版本号",
                    version
                )
            })?;

        if self.get_current_version()?.as_deref() == Some(normalized.as_str()) {
            self.clear_current_version()?;
            self.remove_node_shims()?;
        }

        fs::remove_dir_all(&target.path).with_context(|| {
            format!(
                "卸载 Node.js {} 失败，无法删除目录：{}",
                normalized,
                target.path.display()
            )
        })?;

        Ok(())
    }

    /// 获取 shims 目录路径。
    pub fn shims_dir(&self) -> Result<PathBuf> {
        Ok(self.app_home_dir()?.join("shims"))
    }

    /// 检查 shims 是否已在当前 PATH。
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

    /// 检查当前终端 `node --version` 是否与目标版本一致。
    pub fn node_command_matches_version(&self, expected_version: &str) -> bool {
        let output = match Command::new("node").args(["--version"]).output() {
            Ok(output) if output.status.success() => output,
            _ => return false,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Self::parse_node_version_output(&stdout)
            .or_else(|| Self::parse_node_version_output(&stderr))
            .map(|version| version == expected_version)
            .unwrap_or(false)
    }

    /// 将 shims 目录自动加入用户级永久 PATH（仅首次调用时实际写入）。
    pub fn ensure_shims_in_path(&self) -> Result<PathConfigResult> {
        let shims_dir = self.shims_dir()?;
        Ok(Self::ensure_shims_in_path_platform(&shims_dir))
    }
    fn ensure_node_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.versions_dir()?).context("创建 Node.js 版本目录失败")?;
        fs::create_dir_all(self.shims_dir()?).context("创建 shims 目录失败")?;
        Ok(())
    }

    fn app_home_dir(&self) -> Result<PathBuf> {
        self.config.app_home_dir_path()
    }

    fn node_root_dir(&self) -> Result<PathBuf> {
        Ok(self.app_home_dir()?.join("nodejs"))
    }

    fn versions_dir(&self) -> Result<PathBuf> {
        Ok(self.node_root_dir()?.join("versions"))
    }

    fn current_version_file(&self) -> Result<PathBuf> {
        Ok(self.node_root_dir()?.join("current"))
    }

    fn clear_current_version(&self) -> Result<()> {
        let file_path = self.current_version_file()?;
        if file_path.exists() {
            fs::remove_file(file_path).context("清除当前 Node.js 版本失败")?;
        }
        Ok(())
    }

    fn node_executable_in_dir(install_dir: &Path) -> PathBuf {
        super::node_executable_in_dir(install_dir)
    }

    fn npm_executable_in_dir(install_dir: &Path) -> PathBuf {
        if cfg!(windows) {
            install_dir.join("npm.cmd")
        } else {
            install_dir.join("bin/npm")
        }
    }

    fn npx_executable_in_dir(install_dir: &Path) -> PathBuf {
        if cfg!(windows) {
            install_dir.join("npx.cmd")
        } else {
            install_dir.join("bin/npx")
        }
    }

    fn refresh_node_shims(&self, node_exe: &Path, npm_exe: &Path, npx_exe: &Path) -> Result<()> {
        let shims_dir = self.shims_dir()?;
        fs::create_dir_all(&shims_dir)
            .with_context(|| format!("创建 shims 目录失败：{}", shims_dir.display()))?;

        if cfg!(windows) {
            Self::write_windows_executable_shim(
                &shims_dir,
                "node.cmd",
                "MEETAI_NODE_EXE",
                node_exe,
                "Node.js",
                "meetai node use <version>",
                false,
            )?;
            Self::write_windows_executable_shim(
                &shims_dir,
                "npm.cmd",
                "MEETAI_NPM_EXE",
                npm_exe,
                "npm",
                "meetai node use <version>",
                true,
            )?;
            Self::write_windows_executable_shim(
                &shims_dir,
                "npx.cmd",
                "MEETAI_NPX_EXE",
                npx_exe,
                "npx",
                "meetai node use <version>",
                true,
            )?;
        } else {
            Self::write_unix_executable_shim(
                &shims_dir,
                "node",
                "MEETAI_NODE_EXE",
                node_exe,
                "Node.js",
                "meetai node use <version>",
            )?;
            Self::write_unix_executable_shim(
                &shims_dir,
                "npm",
                "MEETAI_NPM_EXE",
                npm_exe,
                "npm",
                "meetai node use <version>",
            )?;
            Self::write_unix_executable_shim(
                &shims_dir,
                "npx",
                "MEETAI_NPX_EXE",
                npx_exe,
                "npx",
                "meetai node use <version>",
            )?;
        }

        Ok(())
    }

    fn remove_node_shims(&self) -> Result<()> {
        let shims_dir = self.shims_dir()?;
        let names = if cfg!(windows) {
            vec!["node.cmd", "npm.cmd", "npx.cmd"]
        } else {
            vec!["node", "npm", "npx"]
        };

        for name in names {
            let path = shims_dir.join(name);
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("删除 Node.js shim 失败：{}", path.display()))?;
            }
        }

        Ok(())
    }

    fn write_windows_executable_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        runtime_name: &str,
        guidance: &str,
        use_call: bool,
    ) -> Result<()> {
        let executable_str = executable.display().to_string();
        let invoke = if use_call {
            format!("call \"%{}%\" %*\r\n", env_var)
        } else {
            format!("\"%{}%\" %*\r\n", env_var)
        };

        let runtime_name_echo = Self::escape_cmd_echo_text(runtime_name);
        let guidance_echo = Self::escape_cmd_echo_text(guidance);
        let script = format!(
            "@echo off\r\nset \"{env_var}={exe}\"\r\nif not exist \"%{env_var}%\" (\r\n  >&2 echo [meetai] 当前 {runtime_name} 可执行文件不存在: %{env_var}%\r\n  >&2 echo [meetai] 请先执行: {guidance}\r\n  exit /b 1\r\n)\r\n{invoke}",
            env_var = env_var,
            exe = executable_str,
            runtime_name = runtime_name_echo,
            guidance = guidance_echo,
            invoke = invoke
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 shim 失败：{}", shim_path.display()))?;
        Ok(())
    }

    fn write_unix_executable_shim(
        shims_dir: &Path,
        shim_name: &str,
        env_var: &str,
        executable: &Path,
        runtime_name: &str,
        guidance: &str,
    ) -> Result<()> {
        let escaped = Self::escape_sh_single_quotes(&executable.display().to_string());

        let script = format!(
            "#!/usr/bin/env sh\n{env_var}='{executable}'\nif [ ! -x \"${env_var}\" ]; then\n  echo \"[meetai] 当前 {runtime_name} 可执行文件不存在: ${env_var}\" >&2\n  echo \"[meetai] 请先执行: {guidance}\" >&2\n  exit 1\nfi\nexec \"${env_var}\" \"$@\"\n",
            env_var = env_var,
            executable = escaped,
            runtime_name = runtime_name,
            guidance = guidance
        );
        let shim_path = shims_dir.join(shim_name);
        fs::write(&shim_path, script)
            .with_context(|| format!("写入 shim 失败：{}", shim_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms)
                .with_context(|| format!("设置 shim 执行权限失败：{}", shim_path.display()))?;
        }

        Ok(())
    }

    fn normalize_version_token(raw: &str) -> Option<String> {
        super::normalize_version_token(raw)
    }

    fn parse_node_version_output(output: &str) -> Option<String> {
        let first = output.split_whitespace().next()?;
        Self::normalize_version_token(first)
    }

    fn escape_cmd_echo_text(raw: &str) -> String {
        raw.replace('^', "^^")
            .replace('&', "^&")
            .replace('|', "^|")
            .replace('<', "^<")
            .replace('>', "^>")
            .replace('(', "^(")
            .replace(')', "^)")
    }

    fn escape_sh_single_quotes(raw: &str) -> String {
        raw.replace('\'', "'\"'\"'")
    }

    /// Windows 实现：通过 PowerShell 读写 HKCU\Environment 中的 Path。
    #[cfg(windows)]
    fn ensure_shims_in_path_platform(shims_dir: &Path) -> PathConfigResult {
        let shims_str = shims_dir.to_string_lossy();
        let shims_escaped = shims_str.replace('\'', "''");

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

    /// Unix 实现：向 .bashrc / .zshrc / .bash_profile / .profile 追加 export 语句。
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
                "未找到可写入的 shell 配置文件（.bashrc/.zshrc/.bash_profile/.profile）"
                    .to_string(),
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
    use super::NodeVersionManager;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn parse_node_version_output_supports_v_prefix() {
        let parsed = NodeVersionManager::parse_node_version_output("v20.11.1\r\n");
        assert_eq!(parsed.as_deref(), Some("20.11.1"));
    }

    #[test]
    fn parse_node_version_output_rejects_non_semver() {
        let parsed = NodeVersionManager::parse_node_version_output("node 20");
        assert!(parsed.is_none());
    }

    #[test]
    fn node_executable_in_dir_uses_platform_specific_location() {
        let install_dir = PathBuf::from("v20.11.1");
        let node_exe = NodeVersionManager::node_executable_in_dir(&install_dir);
        if cfg!(windows) {
            assert!(node_exe.ends_with("node.exe"));
        } else {
            assert!(node_exe.ends_with("bin/node"));
        }
    }

    #[test]
    fn windows_node_shim_uses_stderr_prefix_echo() {
        let temp = tempdir().expect("tempdir should be created");
        let shim_path = temp.path().join("node.cmd");
        NodeVersionManager::write_windows_executable_shim(
            temp.path(),
            "node.cmd",
            "MEETAI_NODE_EXE",
            &PathBuf::from(r"D:\Node\node.exe"),
            "Node.js",
            "meetai node use <version>",
            false,
        )
        .expect("shim should be written");

        let script = fs::read_to_string(shim_path).expect("shim should be readable");
        assert!(script.contains(">&2 echo [meetai] 当前 Node.js 可执行文件不存在"));
        assert!(script.contains(">&2 echo [meetai] 请先执行: meetai node use ^<version^>"));
        assert!(!script.contains(" 1>&2"));
    }
}
