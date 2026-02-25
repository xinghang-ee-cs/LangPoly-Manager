use anyhow::{Context, Result};
use dirs::home_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const APP_HOME_DIR: &str = ".meetai";
const LEGACY_APP_HOME_DIR: &str = ".python-manager";

/// 应用程序配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Python 安装目录
    pub python_install_dir: PathBuf,
    /// 虚拟环境目录
    pub venv_dir: PathBuf,
    /// 下载缓存目录
    pub cache_dir: PathBuf,
    /// 当前使用的 Python 版本
    pub current_python_version: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let config_dir = Self::app_home_dir();

        Self {
            python_install_dir: config_dir.join("python"),
            venv_dir: config_dir.join("venvs"),
            cache_dir: config_dir.join("cache"),
            current_python_version: None,
        }
    }
}

impl Config {
    /// 加载配置文件
    pub fn load() -> Result<Self> {
        Self::migrate_legacy_home_dir_if_needed()?;

        let config_path = Self::config_file_path();

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;

        let config: Config =
            serde_json::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    /// 保存配置文件
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        let config_dir = config_path
            .parent()
            .context("Failed to get config directory")?;

        std::fs::create_dir_all(config_dir).context("Failed to create config directory")?;

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, content).context("Failed to write config file")?;

        Ok(())
    }

    /// 获取配置文件路径
    fn config_file_path() -> PathBuf {
        Self::app_home_dir().join("config.json")
    }

    /// 确保所有必要的目录存在
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.python_install_dir)
            .context("Failed to create Python install directory")?;

        let shims_dir = self
            .python_install_dir
            .parent()
            .context("Failed to derive shims directory from python install directory")?
            .join("shims");
        std::fs::create_dir_all(&shims_dir).context("Failed to create shims directory")?;

        std::fs::create_dir_all(&self.venv_dir).context("Failed to create venv directory")?;

        std::fs::create_dir_all(&self.cache_dir).context("Failed to create cache directory")?;

        Ok(())
    }

    fn app_home_dir() -> PathBuf {
        if let Some(exe_dir) = Self::executable_parent_dir() {
            return exe_dir.join(APP_HOME_DIR);
        }

        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(APP_HOME_DIR)
    }

    fn legacy_app_home_dir() -> PathBuf {
        if let Some(exe_dir) = Self::executable_parent_dir() {
            return exe_dir.join(LEGACY_APP_HOME_DIR);
        }

        let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(LEGACY_APP_HOME_DIR)
    }

    fn executable_parent_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
    }

    /// 将旧目录 ~/.python-manager 尝试迁移到 ~/.meetai
    fn migrate_legacy_home_dir_if_needed() -> Result<()> {
        let new_dir = Self::app_home_dir();
        let legacy_dir = Self::legacy_app_home_dir();

        if new_dir.exists() || !legacy_dir.exists() {
            return Ok(());
        }

        match std::fs::rename(&legacy_dir, &new_dir) {
            Ok(_) => return Ok(()),
            Err(err) => {
                warn!(
                    "Failed to rename legacy config directory '{}' to '{}', fallback to copy: {:#}",
                    legacy_dir.display(),
                    new_dir.display(),
                    err
                );
            }
        }

        std::fs::create_dir_all(&new_dir).with_context(|| {
            format!(
                "Failed to create new app directory during migration: {}",
                new_dir.display()
            )
        })?;

        let mut options = fs_extra::dir::CopyOptions::new();
        options.copy_inside = true;
        options.overwrite = false;

        fs_extra::dir::copy(&legacy_dir, &new_dir, &options).with_context(|| {
            format!(
                "Failed to copy legacy app directory from '{}' to '{}'",
                legacy_dir.display(),
                new_dir.display()
            )
        })?;

        Ok(())
    }
}
