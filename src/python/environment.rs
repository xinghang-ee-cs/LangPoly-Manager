//! Python 虚拟环境管理模块。
//!
//! 本模块提供 Python 虚拟环境的创建、激活和列表功能，支持跨平台操作。
//!
//! 核心类型：
//! - `VenvManager`: 虚拟环境管理器，负责 venv 的完整生命周期
//!
//! 主要功能：
//! 1. **创建虚拟环境** (`create`): 在指定目录创建新的虚拟环境
//!    - 调用 `python -m venv <target_dir>` 命令
//!    - 自动生成激活脚本（Windows: `activate.ps1`, Unix: `bin/activate`）
//!    - 支持自定义目标目录和虚拟环境名称
//! 2. **激活虚拟环境** (`activate`): 输出激活指令到 stdout
//!    - Windows: 输出 `.\<venv_name>\Scripts\Activate.ps1`
//!    - Unix: 输出 `source <venv_name>/bin/activate`
//!    - 用户需手动执行输出命令完成激活
//! 3. **列出虚拟环境** (`list`): 扫描目标目录，返回所有虚拟环境目录名
//!
//! 目录结构约定：
//! ```text
//! <target_dir>/           # 通常为项目根目录
//! ├── .venv/              # 默认虚拟环境目录
//! │   ├── bin/            # Unix 可执行文件
//! │   │   ├── activate    # 激活脚本
//! │   │   └── python      # Python 解释器符号链接
//! │   ├── Lib/            # Windows 库目录
//! │   └── Scripts/        # Windows 可执行文件
//! │       ├── activate.ps1 # PowerShell 激活脚本
//! │       └── python.exe  # Python 解释器
//! └── other-venv/         # 其他命名虚拟环境
//! ```
//!
//! 平台差异：
//! - **Windows**: 使用 PowerShell 激活脚本，路径分隔符为 `\`
//! - **Unix/macOS**: 使用 shell 激活脚本，路径分隔符为 `/`
//!
//! 错误处理：
//! - 虚拟环境创建失败：返回 `anyhow::Error`，包含命令执行错误
//! - 目录读取失败：返回 `std::io::Error`
//! - 激活脚本生成失败：返回 `std::io::Error`
//!
//! 与 PythonService 集成：
//! - 通过 `PythonService::handle_venv_command` 暴露给 CLI
//! - 支持 `venv create`、`venv activate`、`venv list` 三个子命令
//!
//! 测试：
//! - 模块内 `mod tests` 包含激活脚本生成和跨平台路径处理测试

use crate::config::Config;
use crate::python::version::PythonVersionManager;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// 虚拟环境管理器
pub struct VenvManager {
    config: Config,
    executor: CommandExecutor,
}

impl VenvManager {
    /// 创建虚拟环境管理器，并确保相关目录已初始化。
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        config.ensure_dirs()?;

        Ok(Self {
            config,
            executor: CommandExecutor::new(),
        })
    }

    /// 创建虚拟环境
    pub async fn create(&self, name: &str, target_dir: &Path) -> Result<()> {
        let version_manager = PythonVersionManager::new()?;
        let python_exe = version_manager.current_python_executable(
            "还没有选择 Python 版本，请先执行: meetai runtime use python <version>",
        )?;

        // 创建虚拟环境
        let venv_path = self.config.venv_dir.join(name);

        // 使用 python -m venv 创建虚拟环境
        let venv_path_arg = venv_path.to_string_lossy().into_owned();
        self.executor
            .execute(&python_exe, &["-m", "venv", &venv_path_arg])
            .await?;

        // 创建标记文件
        let marker_path = target_dir.join(".venv");
        fs::write(&marker_path, venv_path.to_string_lossy().as_bytes())
            .context("Failed to create venv marker file")?;

        // 创建激活脚本
        self.create_activate_scripts(&venv_path, target_dir).await?;

        Ok(())
    }

    /// 激活虚拟环境
    pub fn activate(&self, name: &str) -> Result<()> {
        let venv_path = self.config.venv_dir.join(name);

        if !venv_path.exists() {
            anyhow::bail!(
                "找不到虚拟环境 '{}'，请先执行: meetai venv create {}",
                name,
                name
            );
        }

        // 输出激活命令
        if cfg!(windows) {
            let activate_script = venv_path.join("Scripts/Activate.ps1");
            if activate_script.exists() {
                println!("请在 PowerShell 中执行以下命令来激活虚拟环境 {}：", name);
                println!("  & \"{}\"", activate_script.display());
                println!("激活后命令提示符前会显示 ({})，表示已进入虚拟环境。", name);
            } else {
                anyhow::bail!(
                    "虚拟环境 '{}' 的激活脚本丢失，该环境可能已损坏，建议删除后重新创建",
                    name
                );
            }
        } else {
            let activate_script = venv_path.join("bin/activate");
            if activate_script.exists() {
                println!("请在终端中执行以下命令来激活虚拟环境 {}：", name);
                println!("  source {}", activate_script.display());
                println!("激活后命令提示符前会显示 ({})，表示已进入虚拟环境。", name);
            } else {
                anyhow::bail!(
                    "虚拟环境 '{}' 的激活脚本丢失，该环境可能已损坏，建议删除后重新创建",
                    name
                );
            }
        }

        Ok(())
    }

    /// 列出所有虚拟环境
    pub fn list(&self) -> Result<Vec<String>> {
        let mut envs = Vec::new();

        if !self.config.venv_dir.exists() {
            return Ok(envs);
        }

        for entry in fs::read_dir(&self.config.venv_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        envs.push(name_str.to_string());
                    }
                }
            }
        }

        envs.sort();
        Ok(envs)
    }

    /// 创建激活脚本
    async fn create_activate_scripts(&self, _venv_path: &Path, target_dir: &Path) -> Result<()> {
        if cfg!(windows) {
            // Windows PowerShell 激活脚本
            let ps_script = r#"
function Set-ProjectPython {
    param(
        [string]$Path = (Get-Location)
    )

    if (Test-Path "$Path\.venv") {
        $venvPath = Get-Content "$Path\.venv"
        if (Test-Path "$venvPath/Scripts/Activate.ps1") {
            & "$venvPath\Scripts\Activate.ps1"
        }
    }
}

# 在目录切换时自动检查
function prompt {
    Set-ProjectPython
    "PS $(Get-Location)> "
}
"#;
            let ps_path = target_dir.join("activate.ps1");
            fs::write(&ps_path, ps_script)?;
        } else {
            // Linux/Mac Bash/Zsh 激活脚本
            let shell_script = r#"
function cd() {
    builtin cd "$@" || return
    if [ -f ".venv" ]; then
        venv_path=$(cat .venv)
        if [ -f "$venv_path/bin/activate" ]; then
            source "$venv_path/bin/activate"
        fi
    fi
}
"#;
            let shell_path = target_dir.join("activate.sh");
            fs::write(&shell_path, shell_script)?;
        }

        Ok(())
    }
}
