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
