//! 应用程序配置管理与目录策略。
//!
//! 本模块负责 MeetAI 的配置持久化、目录结构规划和历史数据迁移。
//! 配置以 JSON 格式存储在 `{app_home}/config.json`，同时管理 Python 安装目录、
//! 虚拟环境目录和下载缓存目录的创建与访问。
//!
//! # 目录结构
//!
//! ```text
//! {app_home}/              # 应用主目录（优先级：MEETAI_HOME → 可执行文件目录 → 用户主目录）
//!   config.json            # 配置文件
//!   python/                # Python 安装目录（由 Config.python_install_dir 指向）
//!     python-3.11.0/       # 具体版本目录
//!   venvs/                 # 虚拟环境目录（由 Config.venv_dir 指向）
//!     myenv/               # 虚拟环境
//!   cache/                 # 下载缓存目录（由 Config.cache_dir 指向）
//!   shims/                 # 命令 shims 目录（自动创建在 python_install_dir 父目录下）
//! ```
//!
//! # 应用主目录解析优先级
//!
//! 1. `MEETAI_HOME` 环境变量（如果设置且非空）
//! 2. 可执行文件所在目录的父目录下的 `.meetai`（如果 exe 在 `bin` 子目录）
//! 3. 可执行文件所在目录下的 `.meetai`
//! 4. 用户主目录下的 `.meetai`
//! 5. 当前工作目录下的 `.meetai`
//!
//! # 历史数据迁移
//!
//! 自动检测旧版本（`.python-manager`）目录并迁移到新目录结构：
//! - 仅在目标目录不存在时执行迁移
//! - 优先使用原子重命名，跨设备时回退为复制
//! - 遇到同名文件冲突时停止迁移并报错
//!
//! # 示例
//!
//! ```rust,no_run
//! use meetai::config::Config;
//!
//! // 加载配置（如不存在则创建默认配置）
//! let config = Config::load()?;
//!
//! // 确保所有目录存在
//! config.ensure_dirs()?;
//!
//! // 获取应用主目录
//! let app_home = config.app_home_dir_path()?;
//! println!("数据目录：{}", app_home.display());
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::{Context, Result};
use dirs::home_dir;
use log::warn;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 应用主目录名称（相对用户主目录或可执行文件目录）。
const APP_HOME_DIR: &str = ".meetai";
/// 历史版本的应用目录名称（用于数据迁移）。
const LEGACY_APP_HOME_DIR: &str = ".python-manager";
/// 用于覆盖默认应用目录的环境变量名。
const APP_HOME_ENV: &str = "MEETAI_HOME";
/// 标记历史数据补齐修复已经完成的文件名。
const LEGACY_REPAIR_MARKER_FILE: &str = ".legacy-repair-complete";

/// 应用程序配置结构体。
///
/// 存储 MeetAI 运行所需的核心路径配置。这些路径在首次加载配置时
/// 自动创建（通过 `ensure_dirs()`），无需手动初始化。
///
/// # 字段说明
///
/// - `python_install_dir`: Python 解释器安装根目录，所有已安装版本存储在此目录下
///   （如 `{app_home}/python/`）。版本目录命名规则：`python-{major}.{minor}.{patch}`
/// - `venv_dir`: 虚拟环境存储目录（如 `{app_home}/venvs/`）
/// - `cache_dir`: 运行时下载文件的缓存目录（如 `{app_home}/cache/`）
/// - `current_python_version`: 当前全局激活的 Python 版本号（如 `"3.11.0"`）
///
/// # 默认值
///
/// 所有路径默认基于应用主目录构建：
/// - `python_install_dir`: `{app_home}/python`
/// - `venv_dir`: `{app_home}/venvs`
/// - `cache_dir`: `{app_home}/cache`
/// - `current_python_version`: `None`
///
/// # 线程安全
///
/// `Config` 是纯数据容器，内部字段均为 `PathBuf` 或 `String`，不可变时线程安全。
/// 修改配置需通过 `save()` 持久化；如存在并发写入场景，需要在调用方额外协调。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Python 安装根目录，所有已安装版本存储在此目录下。
    ///
    /// 目录结构示例：
    /// ```text
    /// {python_install_dir}/
    ///   python-3.11.0/
    ///   python-3.12.0/
    /// ```
    pub python_install_dir: PathBuf,
    /// 虚拟环境存储目录。
    ///
    /// 所有通过 `meetai venv create` 创建的虚拟环境都存储在此目录下。
    pub venv_dir: PathBuf,
    /// 下载文件缓存目录。
    ///
    /// 用于存储运行时安装包、临时下载文件等，避免重复下载。
    pub cache_dir: PathBuf,
    /// 当前全局激活的 Python 版本号（如 `"3.11.0"`）。
    ///
    /// 该值由 `set_current_version()` 设置，影响 `python use` 和 `pip` 命令的行为。
    /// 如果为 `None`，表示尚未设置默认 Python 版本。
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
    /// 加载配置文件。
    ///
    /// 加载流程：
    /// 1. 执行历史配置目录迁移（从 `.python-manager` 迁移到 `.meetai`）
    /// 2. 对已存在的新目录执行一次历史数据补齐修复（如果附近仍保留旧目录）
    /// 3. 确定配置文件路径 `{app_home}/config.json`
    /// 4. 如果文件不存在，创建默认配置并保存
    /// 5. 读取并解析 JSON 内容
    /// 6. 如有需要，修正迁移后遗留的路径和当前版本状态
    ///
    /// # 错误
    ///
    /// 返回 `anyhow::Result`，可能错误包括：
    /// - 读取文件失败（权限不足、磁盘错误等）
    /// - JSON 解析失败（配置文件损坏）
    /// - 目录迁移失败
    /// - 配置修复失败
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use meetai::config::Config;
    ///
    /// let config = Config::load()?;
    /// # let _ = config;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn load() -> Result<Self> {
        Self::migrate_legacy_home_dir_if_needed()?;
        let app_home = Self::app_home_dir();
        let legacy_candidates = Self::legacy_app_home_candidates();

        let config_path = Self::config_file_path();

        let mut config = if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            default_config
        } else {
            let content = std::fs::read_to_string(&config_path).context("读取配置文件失败")?;
            serde_json::from_str(&content).context("解析配置文件失败")?
        };

        if Self::repair_loaded_config_if_needed(&mut config, &app_home, &legacy_candidates)? {
            config.save()?;
        }

        Ok(config)
    }

    /// 保存配置到文件。
    ///
    /// 将当前配置序列化为 JSON 并写入 `{app_home}/config.json`。
    /// 会自动创建配置目录（如果不存在）。
    ///
    /// # 参数
    ///
    /// - `&self`: 要保存的配置对象
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 保存成功
    /// - `Err`: 保存失败，包括目录创建失败、序列化失败或写入失败
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use meetai::config::Config;
    ///
    /// let mut config = Config::load()?;
    /// config.current_python_version = Some("3.11.0".to_string());
    /// config.save()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        let config_dir = config_path.parent().context("获取配置目录失败")?;

        std::fs::create_dir_all(config_dir).context("创建配置目录失败")?;

        let content = serde_json::to_string_pretty(self).context("序列化配置失败")?;

        std::fs::write(&config_path, content).context("写入配置文件失败")?;

        Ok(())
    }

    /// 获取配置文件路径。
    ///
    /// 路径为 `{app_home}/config.json`。
    ///
    /// 该方法是私有的，配置路径由模块内部统一管理。
    fn config_file_path() -> PathBuf {
        Self::app_home_dir().join("config.json")
    }

    /// 确保所有必要的目录存在。
    ///
    /// 创建以下目录（如果不存在）：
    /// - `python_install_dir`: Python 安装根目录
    /// - `shims`: shims 目录（位于 `python_install_dir` 的父目录下）
    /// - `venv_dir`: 虚拟环境目录
    /// - `cache_dir`: 缓存目录
    ///
    /// # 错误
    ///
    /// 返回 `anyhow::Result`，目录创建失败时返回错误。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use meetai::config::Config;
    ///
    /// let config = Config::load()?;
    /// config.ensure_dirs()?;  // 确保所有目录就绪
    /// # Ok::<(), anyhow::Error>(())
    /// ```
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

    /// 从当前配置推导 MeetAI 应用主目录。
    ///
    /// 应用主目录是 `python_install_dir` 的父目录，即：
    /// ```text
    /// {python_install_dir} = {app_home}/python
    /// => {app_home} = {python_install_dir}.parent()
    /// ```
    ///
    /// # 错误
    ///
    /// 如果 `python_install_dir` 没有父目录（例如是根目录），返回错误。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use meetai::config::Config;
    ///
    /// let config = Config::load()?;
    /// let app_home = config.app_home_dir_path()?;
    /// println!("应用数据目录：{}", app_home.display());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn app_home_dir_path(&self) -> Result<PathBuf> {
        self.python_install_dir
            .parent()
            .map(Path::to_path_buf)
            .context("无法从 python_install_dir 推导 MeetAI app home 目录")
    }

    /// 解析应用主目录，优先级：`MEETAI_HOME` → `<exe_dir>/.meetai` → `~/.meetai` → `.meetai`（CWD）。
    ///
    /// 该方法是 `app_home_dir()` 的核心逻辑，不依赖 `Config` 实例。
    ///
    /// # 参数
    ///
    /// - `env_home`: `MEETAI_HOME` 环境变量的值（如果有）
    /// - `user_home`: 用户主目录（`~`）
    /// - `exe_dir`: 可执行文件所在目录
    ///
    /// # 返回
    ///
    /// 根据优先级返回第一个有效的应用主目录路径。
    ///
    /// # 设计说明
    ///
    /// - 如果可执行文件在 `bin` 子目录中，数据目录放在父目录（如 `D:\MeetAI\bin\meetai.exe` → `D:\MeetAI\.meetai`）
    /// - 否则数据目录放在可执行文件同级目录（兼容旧版本）
    fn app_home_dir() -> PathBuf {
        Self::resolve_app_home_dir(
            Self::env_app_home_dir(),
            home_dir(),
            Self::executable_parent_dir(),
        )
    }

    /// 应用主目录解析的核心实现。
    ///
    /// 按传入的候选路径按优先级顺序检查，返回第一个有效路径。
    ///
    /// # 参数
    ///
    /// - `env_home`: 环境变量指定的目录（最高优先级）
    /// - `user_home`: 用户主目录
    /// - `exe_dir`: 可执行文件目录
    ///
    /// # 返回
    ///
    /// 返回解析后的应用主目录路径。
    fn resolve_app_home_dir(
        env_home: Option<PathBuf>,
        user_home: Option<PathBuf>,
        exe_dir: Option<PathBuf>,
    ) -> PathBuf {
        if let Some(app_home) = env_home {
            return app_home;
        }
        if let Some(exe_dir) = exe_dir {
            // 如果 exe 在 bin 子目录，数据放在父目录的 .meetai
            // 例如：D:\MeetAI\bin\meetai.exe → D:\MeetAI\.meetai
            if exe_dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case("bin"))
                .unwrap_or(false)
            {
                if let Some(parent) = exe_dir.parent() {
                    return parent.join(APP_HOME_DIR);
                }
            }
            // 否则放在同级目录（兼容旧版本）
            return exe_dir.join(APP_HOME_DIR);
        }
        if let Some(home) = user_home {
            return home.join(APP_HOME_DIR);
        }
        PathBuf::from(APP_HOME_DIR)
    }

    /// 获取 `MEETAI_HOME` 环境变量的值。
    ///
    /// 仅当环境变量存在且非空时返回 `Some(PathBuf)`。
    fn env_app_home_dir() -> Option<PathBuf> {
        std::env::var_os(APP_HOME_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// 生成历史配置目录候选列表。
    ///
    /// 包含以下路径（按检查顺序）：
    /// - 可执行文件目录下的 `.python-manager`
    /// - 可执行文件目录下的 `.meetai`
    /// - 用户主目录下的 `.python-manager`（旧版本默认）
    /// - 用户主目录下的 `.meetai`（早期版本曾使用）
    ///
    /// 返回去重后的路径列表。
    fn legacy_app_home_candidates() -> Vec<PathBuf> {
        Self::build_legacy_app_home_candidates(home_dir(), Self::executable_parent_dir())
    }

    /// 生成历史配置目录候选列表的核心实现。
    ///
    /// 优先尝试可执行文件附近的历史目录，避免在用户主目录和部署目录同时存在旧数据时，
    /// 误把无关的历史用户目录迁移成当前安装目录的数据源。
    fn build_legacy_app_home_candidates(
        user_home: Option<PathBuf>,
        exe_dir: Option<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut candidates = Vec::<PathBuf>::new();

        if let Some(exe_dir) = exe_dir {
            candidates.push(exe_dir.join(LEGACY_APP_HOME_DIR));
            // 历史版本把 .meetai 放在可执行文件目录，此处做兼容迁移
            candidates.push(exe_dir.join(APP_HOME_DIR));
        }
        if let Some(home) = user_home {
            candidates.push(home.join(LEGACY_APP_HOME_DIR));
            // 旧默认目录为用户主目录 ~/.meetai，新策略下需迁移到新的 app home。
            candidates.push(home.join(APP_HOME_DIR));
        }

        Self::dedup_candidates(candidates)
    }

    /// 对路径列表进行去重。
    ///
    /// 使用规范化后的路径字符串作为去重键（Windows 下不区分大小写、统一斜杠方向）。
    ///
    /// # 参数
    ///
    /// - `candidates`: 待去重的路径列表
    ///
    /// # 返回
    ///
    /// 去重后的路径列表，保持原顺序。
    fn dedup_candidates(mut candidates: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut dedup = std::collections::HashSet::<String>::new();
        candidates.retain(|path| dedup.insert(Self::normalize_path_key(path)));
        candidates
    }

    /// 生成路径的规范化键值（用于去重比较）。
    ///
    /// - Windows: 转换为小写，并将 `/` 替换为 `\`
    /// - 其他平台: 原样返回字符串
    ///
    /// # 参数
    ///
    /// - `path`: 要规范化的路径
    ///
    /// # 返回
    ///
    /// 规范化后的字符串表示。
    fn normalize_path_key(path: &Path) -> String {
        if cfg!(windows) {
            path.to_string_lossy()
                .replace('/', "\\")
                .to_ascii_lowercase()
        } else {
            path.to_string_lossy().to_string()
        }
    }

    /// 获取当前可执行文件的父目录。
    ///
    /// 用于确定可执行文件部署位置，进而推导应用主目录。
    fn executable_parent_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
    }

    /// 将目录从 `from` 移动到 `to`：优先 rename（原子操作），跨设备时回退为递归 copy。
    ///
    /// 该方法是历史数据迁移的核心操作，保证尽可能不丢失数据。
    ///
    /// # 参数
    ///
    /// - `from`: 源目录路径
    /// - `to`: 目标目录路径
    ///
    /// # 错误
    ///
    /// - 重命名失败时尝试复制，复制失败则返回错误
    /// - 复制过程中遇到同名文件冲突也会失败
    ///
    /// # 变更日志
    ///
    /// - 2024-01-15: 添加跨设备回退复制逻辑
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
    ///
    /// 用于迁移过程中的回退方案，确保数据安全。
    ///
    /// # 参数
    ///
    /// - `from`: 源目录
    /// - `to`: 目标目录
    ///
    /// # 错误
    ///
    /// - 读取源目录失败
    /// - 创建目标子目录失败
    /// - 目标文件已存在（冲突）
    /// - 复制文件失败
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

    /// 递归合并目录内容，仅补齐目标目录中缺失的文件，不覆盖已存在文件。
    ///
    /// 用于修复历史迁移后留下的半迁移状态，例如新目录已经存在，但旧目录中仍保留
    /// 部分运行时版本、缓存或 shims 等缺失内容。
    fn merge_dir_contents_skip_existing(from: &Path, to: &Path) -> Result<usize> {
        let mut copied_entries = 0usize;

        for entry in std::fs::read_dir(from)
            .with_context(|| format!("读取修复源目录失败：{}", from.display()))?
        {
            let entry = entry.with_context(|| format!("读取目录条目失败：{}", from.display()))?;
            let from_path = entry.path();
            let to_path = to.join(entry.file_name());
            let file_type = entry
                .file_type()
                .with_context(|| format!("读取条目类型失败：{}", from_path.display()))?;

            if file_type.is_dir() {
                if !to_path.exists() {
                    std::fs::create_dir_all(&to_path).with_context(|| {
                        format!("创建修复目标子目录失败：{}", to_path.display())
                    })?;
                }
                copied_entries += Self::merge_dir_contents_skip_existing(&from_path, &to_path)?;
            } else if file_type.is_file() {
                if to_path.exists() {
                    continue;
                }
                std::fs::copy(&from_path, &to_path).with_context(|| {
                    format!(
                        "复制修复文件失败（{} → {}）",
                        from_path.display(),
                        to_path.display()
                    )
                })?;
                copied_entries += 1;
            }
        }

        Ok(copied_entries)
    }

    /// 判断历史目录是否属于当前应用目录所在树，用于限制自动修复范围。
    ///
    /// 这样可以优先修复当前部署附近的旧目录，避免把无关安装位置的数据自动并入。
    fn legacy_dir_is_in_same_app_tree(new_dir: &Path, legacy_dir: &Path) -> bool {
        let Some(app_root) = new_dir.parent() else {
            return false;
        };

        let app_root_key = Self::normalize_path_key(app_root);
        let legacy_key = Self::normalize_path_key(legacy_dir);
        if cfg!(windows) {
            legacy_key == app_root_key || legacy_key.starts_with(&(app_root_key + "\\"))
        } else {
            legacy_key == app_root_key || legacy_key.starts_with(&(app_root_key + "/"))
        }
    }

    /// 从单个历史目录向已存在的新目录补齐缺失内容。
    ///
    /// 返回本次补齐复制的文件数量；返回 0 表示没有缺失内容需要修复。
    fn repair_existing_app_home_from_legacy_dir(
        new_dir: &Path,
        legacy_dir: &Path,
    ) -> Result<usize> {
        if !legacy_dir.exists() {
            return Ok(0);
        }

        Self::merge_dir_contents_skip_existing(legacy_dir, new_dir)
    }

    /// 对已存在的新目录执行一次附近历史目录的非破坏性修复。
    ///
    /// 仅考虑和当前应用目录属于同一目录树的历史候选目录，避免跨安装位置自动混入数据。
    fn repair_existing_app_home_from_candidates_if_needed(
        new_dir: &Path,
        candidates: &[PathBuf],
    ) -> Result<()> {
        if !new_dir.exists() {
            return Ok(());
        }
        if Self::legacy_repair_marker_path(new_dir).exists() {
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
            if !Self::legacy_dir_is_in_same_app_tree(new_dir, legacy_dir) {
                continue;
            }

            let copied_entries = Self::repair_existing_app_home_from_legacy_dir(
                new_dir, legacy_dir,
            )
            .with_context(|| {
                format!(
                    "从历史数据目录补齐缺失内容失败（{} → {}）",
                    legacy_dir.display(),
                    new_dir.display()
                )
            })?;
            if copied_entries > 0 {
                println!(
                    "注意：检测到历史数据目录 {} 仍包含缺失内容，已补齐到 {}。",
                    legacy_dir.display(),
                    new_dir.display()
                );
            }
        }

        if let Err(err) = std::fs::write(Self::legacy_repair_marker_path(new_dir), b"completed\n") {
            warn!(
                "写入历史修复标记失败（{}）：{:#}",
                Self::legacy_repair_marker_path(new_dir).display(),
                err
            );
        }

        Ok(())
    }

    fn legacy_repair_marker_path(app_home: &Path) -> PathBuf {
        app_home.join(LEGACY_REPAIR_MARKER_FILE)
    }

    fn expected_layout_for_app_home(app_home: &Path) -> Self {
        Self {
            python_install_dir: app_home.join("python"),
            venv_dir: app_home.join("venvs"),
            cache_dir: app_home.join("cache"),
            current_python_version: None,
        }
    }

    fn current_python_executable_for_version(python_install_dir: &Path, version: &str) -> PathBuf {
        let install_dir = python_install_dir.join(format!("python-{version}"));
        if cfg!(windows) {
            install_dir.join("python.exe")
        } else {
            install_dir.join("bin/python")
        }
    }

    fn normalize_managed_paths_if_needed(config: &mut Self, app_home: &Path) -> bool {
        let expected = Self::expected_layout_for_app_home(app_home);
        let mut changed = false;

        if config.python_install_dir != expected.python_install_dir {
            config.python_install_dir = expected.python_install_dir;
            changed = true;
        }
        if config.venv_dir != expected.venv_dir {
            config.venv_dir = expected.venv_dir;
            changed = true;
        }
        if config.cache_dir != expected.cache_dir {
            config.cache_dir = expected.cache_dir;
            changed = true;
        }

        changed
    }

    fn recover_current_python_version_from_legacy_candidates(
        app_home: &Path,
        candidates: &[PathBuf],
        python_install_dir: &Path,
    ) -> Option<String> {
        for legacy_dir in candidates {
            if !legacy_dir.exists() {
                continue;
            }
            if !Self::legacy_dir_is_in_same_app_tree(app_home, legacy_dir) {
                continue;
            }

            let config_path = legacy_dir.join("config.json");
            if !config_path.exists() {
                continue;
            }

            let legacy_config = match std::fs::read_to_string(&config_path)
                .with_context(|| format!("读取历史配置文件失败：{}", config_path.display()))
                .and_then(|content| {
                    serde_json::from_str::<Config>(&content).context("解析历史配置文件失败")
                }) {
                Ok(config) => config,
                Err(err) => {
                    warn!(
                        "跳过无法读取的历史配置 {}：{:#}",
                        config_path.display(),
                        err
                    );
                    continue;
                }
            };

            let Some(version) = legacy_config.current_python_version else {
                continue;
            };
            let python_exe =
                Self::current_python_executable_for_version(python_install_dir, &version);
            if python_exe.exists() {
                return Some(version);
            }
        }

        None
    }

    fn repair_loaded_config_if_needed(
        config: &mut Self,
        app_home: &Path,
        candidates: &[PathBuf],
    ) -> Result<bool> {
        let mut changed = Self::normalize_managed_paths_if_needed(config, app_home);

        let current_valid = config
            .current_python_version
            .as_deref()
            .map(|version| {
                Self::current_python_executable_for_version(&config.python_install_dir, version)
                    .exists()
            })
            .unwrap_or(false);
        if current_valid {
            return Ok(changed);
        }

        let recovered = Self::recover_current_python_version_from_legacy_candidates(
            app_home,
            candidates,
            &config.python_install_dir,
        );
        if config.current_python_version != recovered {
            config.current_python_version = recovered;
            changed = true;
        }

        Ok(changed)
    }

    /// 若当前应用目录尚不存在，则从历史候选目录中找到第一个已存在的目录并迁移过来。
    ///
    /// 迁移时优先使用 rename，跨设备时回退为 copy。
    ///
    /// # 错误
    ///
    /// 迁移过程中任何一步失败都会返回错误。
    fn migrate_legacy_home_dir_if_needed() -> Result<()> {
        let new_dir = Self::app_home_dir();
        let legacy_candidates = Self::legacy_app_home_candidates();
        if !new_dir.exists() {
            Self::migrate_from_candidates_if_needed(&new_dir, &legacy_candidates)?;
        }
        Self::repair_existing_app_home_from_candidates_if_needed(&new_dir, &legacy_candidates)
    }

    /// 从候选目录列表迁移第一个存在的目录到新目录。
    ///
    /// # 参数
    ///
    /// - `new_dir`: 目标新目录
    /// - `candidates`: 历史目录候选列表（按优先级排序）
    ///
    /// # 行为
    ///
    /// - 如果 `new_dir` 已存在，跳过迁移
    /// - 遍历 `candidates`，使用第一个存在的目录进行迁移
    /// - 迁移成功后停止遍历（不迁移后续候选目录）
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

    /// 测试应用主目录解析优先级：环境变量 > 可执行文件目录 > 用户主目录 > 当前目录
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
            exe_dir.join(APP_HOME_DIR)
        );
        assert_eq!(
            Config::resolve_app_home_dir(None, Some(user_home.clone()), None),
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

    /// 测试历史目录候选优先级：优先可执行文件附近的数据目录，再回退到用户主目录。
    #[test]
    fn legacy_app_home_candidates_prioritize_executable_adjacent_dirs() {
        let user_home = PathBuf::from("X:/user-home");
        let exe_dir = PathBuf::from("X:/meetai/bin");

        let candidates = Config::build_legacy_app_home_candidates(
            Some(user_home.clone()),
            Some(exe_dir.clone()),
        );

        assert_eq!(candidates[0], exe_dir.join(LEGACY_APP_HOME_DIR));
        assert_eq!(candidates[1], exe_dir.join(APP_HOME_DIR));
        assert_eq!(candidates[2], user_home.join(LEGACY_APP_HOME_DIR));
        assert_eq!(candidates[3], user_home.join(APP_HOME_DIR));
    }

    #[test]
    fn repair_existing_app_home_merges_missing_files_from_same_tree_legacy_dir() -> Result<()> {
        let temp = tempdir()?;
        let app_root = temp.path().join("MeetAI");
        let new_dir = app_root.join(".meetai");
        let legacy_dir = app_root.join("bin").join(".meetai");

        std::fs::create_dir_all(new_dir.join("python").join("python-3.14.3").join("Doc"))?;
        std::fs::create_dir_all(legacy_dir.join("python").join("python-3.13.2"))?;
        std::fs::create_dir_all(legacy_dir.join("python").join("python-3.14.3"))?;
        std::fs::write(
            new_dir.join("config.json"),
            b"{\"current_python_version\":null}",
        )?;
        let legacy_py3132 =
            Config::current_python_executable_for_version(&legacy_dir.join("python"), "3.13.2");
        if let Some(parent) = legacy_py3132.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&legacy_py3132, b"py3132")?;
        let legacy_py3143 =
            Config::current_python_executable_for_version(&legacy_dir.join("python"), "3.14.3");
        if let Some(parent) = legacy_py3143.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&legacy_py3143, b"py3143")?;
        std::fs::write(legacy_dir.join("config.json"), b"{\"legacy\":true}")?;

        let copied = Config::repair_existing_app_home_from_legacy_dir(&new_dir, &legacy_dir)?;

        assert!(copied >= 2, "expected missing runtime files to be copied");
        assert!(
            Config::current_python_executable_for_version(&new_dir.join("python"), "3.13.2")
                .exists()
        );
        assert!(
            Config::current_python_executable_for_version(&new_dir.join("python"), "3.14.3")
                .exists()
        );
        assert_eq!(
            std::fs::read_to_string(new_dir.join("config.json"))?,
            "{\"current_python_version\":null}"
        );

        Ok(())
    }

    #[test]
    fn repair_existing_app_home_skips_unrelated_legacy_dirs() -> Result<()> {
        let temp = tempdir()?;
        let new_dir = temp.path().join("MeetAI").join(".meetai");
        let unrelated_legacy = temp.path().join("Elsewhere").join(".python-manager");

        std::fs::create_dir_all(&new_dir)?;
        std::fs::create_dir_all(unrelated_legacy.join("python").join("python-3.13.2"))?;
        let unrelated_python = Config::current_python_executable_for_version(
            &unrelated_legacy.join("python"),
            "3.13.2",
        );
        if let Some(parent) = unrelated_python.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&unrelated_python, b"py3132")?;

        Config::repair_existing_app_home_from_candidates_if_needed(
            &new_dir,
            std::slice::from_ref(&unrelated_legacy),
        )?;

        assert!(
            !Config::current_python_executable_for_version(&new_dir.join("python"), "3.13.2")
                .exists()
        );

        Ok(())
    }

    #[test]
    fn repair_existing_app_home_writes_marker_and_skips_follow_up_scans() -> Result<()> {
        let temp = tempdir()?;
        let app_root = temp.path().join("MeetAI");
        let new_dir = app_root.join(".meetai");
        let legacy_dir = app_root.join("bin").join(".meetai");

        std::fs::create_dir_all(&new_dir)?;
        std::fs::create_dir_all(legacy_dir.join("python").join("python-3.13.2"))?;
        let python_exe =
            Config::current_python_executable_for_version(&legacy_dir.join("python"), "3.13.2");
        if let Some(parent) = python_exe.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&python_exe, b"py3132")?;

        Config::repair_existing_app_home_from_candidates_if_needed(
            &new_dir,
            std::slice::from_ref(&legacy_dir),
        )?;
        assert!(Config::legacy_repair_marker_path(&new_dir).exists());

        let later_python_exe =
            Config::current_python_executable_for_version(&legacy_dir.join("python"), "3.14.3");
        if let Some(parent) = later_python_exe.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&later_python_exe, b"py3143")?;

        Config::repair_existing_app_home_from_candidates_if_needed(
            &new_dir,
            std::slice::from_ref(&legacy_dir),
        )?;
        assert!(
            !Config::current_python_executable_for_version(&new_dir.join("python"), "3.14.3")
                .exists(),
            "marker should prevent repeated deep scans after initial repair"
        );

        Ok(())
    }

    #[test]
    fn repair_loaded_config_normalizes_paths_and_recovers_current_version() -> Result<()> {
        let temp = tempdir()?;
        let app_root = temp.path().join("MeetAI");
        let app_home = app_root.join(".meetai");
        let legacy_dir = app_root.join("bin").join(".meetai");

        std::fs::create_dir_all(app_home.join("python").join("python-3.13.2"))?;
        std::fs::create_dir_all(&legacy_dir)?;
        let current_python_exe =
            Config::current_python_executable_for_version(&app_home.join("python"), "3.13.2");
        if let Some(parent) = current_python_exe.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&current_python_exe, b"py3132")?;
        std::fs::write(
            legacy_dir.join("config.json"),
            br#"{
  "python_install_dir": "X:\\legacy\\python",
  "venv_dir": "X:\\legacy\\venvs",
  "cache_dir": "X:\\legacy\\cache",
  "current_python_version": "3.13.2"
}"#,
        )?;

        let mut config = Config {
            python_install_dir: PathBuf::from("X:/legacy/python"),
            venv_dir: PathBuf::from("X:/legacy/venvs"),
            cache_dir: PathBuf::from("X:/legacy/cache"),
            current_python_version: Some("3.14.3".to_string()),
        };

        let changed = Config::repair_loaded_config_if_needed(
            &mut config,
            &app_home,
            std::slice::from_ref(&legacy_dir),
        )?;

        assert!(changed);
        assert_eq!(config.python_install_dir, app_home.join("python"));
        assert_eq!(config.venv_dir, app_home.join("venvs"));
        assert_eq!(config.cache_dir, app_home.join("cache"));
        assert_eq!(config.current_python_version.as_deref(), Some("3.13.2"));

        Ok(())
    }

    /// 测试路径去重：Windows 下不区分大小写和斜杠方向
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

    /// 测试：从 python_install_dir 正确推导 app_home_dir
    #[test]
    fn app_home_dir_path_uses_python_install_parent() -> Result<()> {
        let config = Config {
            python_install_dir: PathBuf::from("D:/meetai-home/python"),
            venv_dir: PathBuf::from("D:/meetai-home/venvs"),
            cache_dir: PathBuf::from("D:/meetai-home/cache"),
            current_python_version: None,
        };

        let app_home = config.app_home_dir_path()?;
        assert_eq!(app_home, PathBuf::from("D:/meetai-home"));

        Ok(())
    }

    /// 测试：当 python_install_dir 没有父目录时报错
    #[test]
    fn app_home_dir_path_errors_when_python_install_dir_has_no_parent() {
        let config = Config {
            python_install_dir: PathBuf::new(),
            venv_dir: PathBuf::from("venvs"),
            cache_dir: PathBuf::from("cache"),
            current_python_version: None,
        };

        let err = config
            .app_home_dir_path()
            .expect_err("empty python_install_dir should fail parent derivation");
        assert!(
            err.to_string()
                .contains("无法从 python_install_dir 推导 MeetAI app home 目录"),
            "unexpected error: {err:#}"
        );
    }

    /// 测试：rename 失败时回退到 copy，且保留已存在的目标文件
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

    /// 测试：只迁移第一个存在的候选目录，后续目录保持不动
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
