use anyhow::{Context, Result};
use dirs::home_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_HOME_DIR: &str = ".meetai";
const LEGACY_APP_HOME_DIR: &str = ".python-manager";
const APP_HOME_ENV: &str = "MEETAI_HOME";

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

        let content = std::fs::read_to_string(&config_path).context("读取配置文件失败")?;

        let config: Config = serde_json::from_str(&content).context("解析配置文件失败")?;

        Ok(config)
    }

    /// 保存配置文件
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        let config_dir = config_path.parent().context("获取配置目录失败")?;

        std::fs::create_dir_all(config_dir).context("创建配置目录失败")?;

        let content = serde_json::to_string_pretty(self).context("序列化配置失败")?;

        std::fs::write(&config_path, content).context("写入配置文件失败")?;

        Ok(())
    }

    /// 获取配置文件路径
    fn config_file_path() -> PathBuf {
        Self::app_home_dir().join("config.json")
    }

    /// 确保所有必要的目录存在
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.python_install_dir).context("创建 Python 安装目录失败")?;

        let shims_dir = self
            .python_install_dir
            .parent()
            .context("无法从 python_install_dir 推导 shims 目录")?
            .join("shims");
        std::fs::create_dir_all(&shims_dir).context("创建 shims 目录失败")?;

        std::fs::create_dir_all(&self.venv_dir).context("创建 venv 目录失败")?;

        std::fs::create_dir_all(&self.cache_dir).context("创建 cache 目录失败")?;

        Ok(())
    }

    /// 解析应用主目录，优先级：`MEETAI_HOME` 环境变量 → `~/.meetai` → `<exe_dir>/.meetai` → `.meetai`（CWD）。
    fn app_home_dir() -> PathBuf {
        Self::resolve_app_home_dir(
            Self::env_app_home_dir(),
            home_dir(),
            Self::executable_parent_dir(),
        )
    }

    fn resolve_app_home_dir(
        env_home: Option<PathBuf>,
        user_home: Option<PathBuf>,
        exe_dir: Option<PathBuf>,
    ) -> PathBuf {
        if let Some(app_home) = env_home {
            return app_home;
        }
        if let Some(home) = user_home {
            return home.join(APP_HOME_DIR);
        }
        if let Some(exe_dir) = exe_dir {
            return exe_dir.join(APP_HOME_DIR);
        }
        PathBuf::from(APP_HOME_DIR)
    }

    fn env_app_home_dir() -> Option<PathBuf> {
        std::env::var_os(APP_HOME_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    fn legacy_app_home_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::<PathBuf>::new();

        if let Some(home) = home_dir() {
            candidates.push(home.join(LEGACY_APP_HOME_DIR));
        }
        if let Some(exe_dir) = Self::executable_parent_dir() {
            candidates.push(exe_dir.join(LEGACY_APP_HOME_DIR));
            // 历史版本把 .meetai 放在可执行文件目录，此处做兼容迁移
            candidates.push(exe_dir.join(APP_HOME_DIR));
        }

        Self::dedup_candidates(candidates)
    }

    fn dedup_candidates(mut candidates: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut dedup = std::collections::HashSet::<String>::new();
        candidates.retain(|path| dedup.insert(Self::normalize_path_key(path)));
        candidates
    }

    fn normalize_path_key(path: &Path) -> String {
        if cfg!(windows) {
            path.to_string_lossy()
                .replace('/', "\\")
                .to_ascii_lowercase()
        } else {
            path.to_string_lossy().to_string()
        }
    }

    fn executable_parent_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
    }

    /// 将目录从 `from` 移动到 `to`：优先 rename（原子操作），跨设备时回退为递归 copy。
    fn rename_or_copy_dir(from: &Path, to: &Path) -> Result<()> {
        match std::fs::rename(from, to) {
            Ok(_) => return Ok(()),
            Err(err) => {
                warn!(
                    "旧配置目录重命名失败（{} → {}），回退为复制：{:#}",
                    from.display(),
                    to.display(),
                    err
                );
            }
        }

        std::fs::create_dir_all(to)
            .with_context(|| format!("迁移时创建新应用目录失败：{}", to.display()))?;
        Self::copy_dir_contents_no_overwrite(from, to)?;

        Ok(())
    }

    /// 递归复制目录内容（仅复制 from 内部条目），遇到同名文件冲突时失败，不覆盖目标文件。
    fn copy_dir_contents_no_overwrite(from: &Path, to: &Path) -> Result<()> {
        for entry in std::fs::read_dir(from)
            .with_context(|| format!("读取迁移源目录失败：{}", from.display()))?
        {
            let entry = entry.with_context(|| format!("读取目录条目失败：{}", from.display()))?;
            let from_path = entry.path();
            let to_path = to.join(entry.file_name());
            let file_type = entry
                .file_type()
                .with_context(|| format!("读取条目类型失败：{}", from_path.display()))?;

            if file_type.is_dir() {
                std::fs::create_dir_all(&to_path)
                    .with_context(|| format!("创建迁移目标子目录失败：{}", to_path.display()))?;
                Self::copy_dir_contents_no_overwrite(&from_path, &to_path)?;
            } else if file_type.is_file() {
                if to_path.exists() {
                    anyhow::bail!("迁移目标已存在同名文件：{}", to_path.display());
                }
                std::fs::copy(&from_path, &to_path).with_context(|| {
                    format!(
                        "复制迁移文件失败（{} → {}）",
                        from_path.display(),
                        to_path.display()
                    )
                })?;
            }
        }

        Ok(())
    }

    /// 若当前应用目录尚不存在，则从历史候选目录中找到第一个已存在的目录并迁移过来。
    /// 迁移时优先使用 rename，跨设备时回退为 copy。
    fn migrate_legacy_home_dir_if_needed() -> Result<()> {
        let new_dir = Self::app_home_dir();
        let legacy_candidates = Self::legacy_app_home_candidates();
        Self::migrate_from_candidates_if_needed(&new_dir, &legacy_candidates)
    }

    fn migrate_from_candidates_if_needed(new_dir: &Path, candidates: &[PathBuf]) -> Result<()> {
        if new_dir.exists() {
            return Ok(());
        }

        let new_key = Self::normalize_path_key(new_dir);
        for legacy_dir in candidates {
            if !legacy_dir.exists() {
                continue;
            }
            if Self::normalize_path_key(legacy_dir) == new_key {
                continue;
            }
            println!(
                "注意：检测到历史数据目录 {}，正在迁移至 {}...",
                legacy_dir.display(),
                new_dir.display()
            );
            Self::rename_or_copy_dir(legacy_dir, new_dir)?;
            println!("✓ 数据目录迁移完成。");
            break;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_app_home_dir_respects_priority_order() {
        let env_home = PathBuf::from("X:/env-home");
        let user_home = PathBuf::from("X:/user-home");
        let exe_dir = PathBuf::from("X:/exe-dir");

        assert_eq!(
            Config::resolve_app_home_dir(
                Some(env_home.clone()),
                Some(user_home.clone()),
                Some(exe_dir.clone())
            ),
            env_home
        );
        assert_eq!(
            Config::resolve_app_home_dir(None, Some(user_home.clone()), Some(exe_dir.clone())),
            user_home.join(APP_HOME_DIR)
        );
        assert_eq!(
            Config::resolve_app_home_dir(None, None, Some(exe_dir.clone())),
            exe_dir.join(APP_HOME_DIR)
        );
        assert_eq!(
            Config::resolve_app_home_dir(None, None, None),
            PathBuf::from(APP_HOME_DIR)
        );
    }

    #[test]
    fn dedup_candidates_removes_duplicates_by_normalized_path_key() {
        let deduped = Config::dedup_candidates(vec![
            PathBuf::from("/tmp/legacy"),
            PathBuf::from("/tmp/legacy"),
            PathBuf::from("/tmp/legacy-2"),
        ]);
        assert_eq!(deduped.len(), 2);

        if cfg!(windows) {
            let deduped = Config::dedup_candidates(vec![
                PathBuf::from(r"C:\Legacy\Config"),
                PathBuf::from(r"c:/legacy/config"),
                PathBuf::from(r"D:\Legacy\Config"),
            ]);
            assert_eq!(
                deduped.len(),
                2,
                "windows normalization should dedup slash/case variants"
            );
        }
    }

    #[test]
    fn rename_or_copy_dir_falls_back_to_copy_when_target_exists() -> Result<()> {
        let temp = tempdir()?;
        let from = temp.path().join("legacy-home");
        let to = temp.path().join("meetai-home");

        std::fs::create_dir_all(&from)?;
        std::fs::write(from.join("config.json"), b"{\"ok\":true}")?;
        std::fs::create_dir_all(&to)?;
        std::fs::write(to.join("keep.txt"), b"keep")?;

        Config::rename_or_copy_dir(&from, &to)?;

        assert!(
            to.join("config.json").exists(),
            "fallback copy should move legacy files into target dir"
        );
        assert!(
            to.join("keep.txt").exists(),
            "existing target files should be preserved during fallback copy"
        );
        assert!(
            from.exists(),
            "fallback copy should not delete source directory automatically"
        );

        Ok(())
    }

    #[test]
    fn migrate_from_candidates_uses_first_existing_candidate_only() -> Result<()> {
        let temp = tempdir()?;
        let new_dir = temp.path().join("new-home");
        let first = temp.path().join("legacy-a");
        let second = temp.path().join("legacy-b");

        std::fs::create_dir_all(&first)?;
        std::fs::create_dir_all(&second)?;
        std::fs::write(first.join("from-first.txt"), b"first")?;
        std::fs::write(second.join("from-second.txt"), b"second")?;

        let candidates = vec![first.clone(), second.clone()];
        Config::migrate_from_candidates_if_needed(&new_dir, &candidates)?;

        assert!(new_dir.join("from-first.txt").exists());
        assert!(
            !new_dir.join("from-second.txt").exists(),
            "only the first existing legacy dir should be migrated"
        );
        assert!(
            second.exists(),
            "later candidates should remain untouched after first successful migration"
        );

        Ok(())
    }
}
