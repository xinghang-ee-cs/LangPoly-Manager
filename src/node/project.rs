//! Node.js 项目版本解析模块。
//!
//! 本模块提供从项目目录中的 `.nvmrc` 文件自动检测 Node.js 版本的功能。
//! 支持从当前工作目录向上递归查找，直到找到 `.nvmrc` 或到达文件系统根目录。
//!
//! 核心函数：
//! - `resolve_project_version_from_nvmrc()`: 从当前目录解析项目版本
//! - `resolve_project_version_from(start_dir)`: 从指定起始目录解析项目版本
//! - `find_nearest_nvmrc(start_dir)`: 向上查找最近的 `.nvmrc` 文件
//! - `parse_nvmrc_version(content)`: 解析 `.nvmrc` 文件内容，提取版本号
//!
//! `.nvmrc` 文件格式：
//! ```text
//! v18.17.0      # 精确版本（推荐）
//! 20.11.1       # 无 v 前缀也支持
//! ```
//! 支持注释行（`#` 开头）和空行，会被自动忽略。
//!
//! 查找算法：
//! 1. 从 `start_dir` 开始
//! 2. 检查当前目录是否存在 `.nvmrc`
//! 3. 如果不存在，移动到父目录
//! 4. 重复步骤 2-3，直到找到或到达根目录
//! 5. 返回找到的第一个 `.nvmrc` 的路径
//!
//! 解析规则：
//! - 读取文件第一行非空非注释内容
//! - 去除首尾空白字符
//! - 支持 `v` 前缀（如 `v18.17.0`），会规范化为 `18.17.0`
//! - 仅接受完整语义化版本号（`X.Y.Z`）
//!
//! 错误处理：
//! - 未找到 `.nvmrc`: 返回 `anyhow::Error`，提示创建文件或直接指定版本
//! - 文件读取失败: 返回 `std::io::Error`
//! - 内容为空或无效: 返回 `anyhow::Error`，提示正确格式
//!
//! 使用方式：
//! ```text
//! meetai node use project
//! meetai node install project
//! ```
//!
//! 与 node 命令集成：
//! - `install_node_for_surface` / `use_node_for_surface` 在版本参数为 `project` 时调用 `resolve_project_version_from_nvmrc`
//! - 解析出的版本会继续走普通 Node.js 安装或切换流程
//! - `.nvmrc` 内容无效时，错误消息会提示正确格式
//!
//! 测试：
//! - 模块内 `mod tests` 包含版本解析、目录查找、错误处理测试

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// 将 `project` 请求解析为当前目录（或父目录）中的 `.nvmrc` 版本。
pub(crate) fn resolve_project_version_from_nvmrc() -> Result<String> {
    let cwd = env::current_dir().context("无法获取当前工作目录")?;
    resolve_project_version_from(&cwd)
}

/// 从指定目录向上查找 `.nvmrc`，解析出项目声明的 Node.js 版本。
pub(crate) fn resolve_project_version_from(start_dir: &Path) -> Result<String> {
    let nvmrc_path = find_nearest_nvmrc(start_dir).with_context(|| {
        format!(
            "当前目录 {} 及其父目录未找到 .nvmrc，请先创建 .nvmrc 或直接指定版本号，例如: meetai node use 20.11.1",
            start_dir.display()
        )
    })?;

    let raw = fs::read_to_string(&nvmrc_path)
        .with_context(|| format!("读取 .nvmrc 失败：{}", nvmrc_path.display()))?;
    parse_nvmrc_version(&raw).with_context(|| {
        format!(
            ".nvmrc 内容无法识别：{}。请使用 X.Y.Z 或 vX.Y.Z 格式，例如: 20.11.1 / v20.11.1",
            nvmrc_path.display()
        )
    })
}

fn find_nearest_nvmrc(start_dir: &Path) -> Option<PathBuf> {
    for dir in start_dir.ancestors() {
        let candidate = dir.join(".nvmrc");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn parse_nvmrc_version(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .and_then(super::normalize_version_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nvmrc_version_accepts_plain_and_v_prefixed_semver() {
        assert_eq!(parse_nvmrc_version("20.11.1\n").as_deref(), Some("20.11.1"));
        assert_eq!(
            parse_nvmrc_version("v20.11.1\n").as_deref(),
            Some("20.11.1")
        );
    }

    #[test]
    fn parse_nvmrc_version_skips_comments_and_blank_lines() {
        let content = "\n# project node version\n\nv22.3.0\n";
        assert_eq!(parse_nvmrc_version(content).as_deref(), Some("22.3.0"));
    }

    #[test]
    fn parse_nvmrc_version_rejects_non_semver_tokens() {
        assert!(parse_nvmrc_version("lts/*\n").is_none());
        assert!(parse_nvmrc_version("20\n").is_none());
    }

    #[test]
    fn resolve_project_version_from_searches_parent_directories() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let project_root = temp.path().join("project");
        let nested = project_root.join("apps").join("web");
        fs::create_dir_all(&nested)?;
        fs::write(project_root.join(".nvmrc"), b"v20.11.1\n")?;

        let resolved = resolve_project_version_from(&nested)?;
        assert_eq!(resolved, "20.11.1");
        Ok(())
    }

    #[test]
    fn resolve_project_version_from_errors_when_nvmrc_missing() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let nested = temp.path().join("apps").join("web");
        fs::create_dir_all(&nested)?;

        let err = resolve_project_version_from(&nested)
            .expect_err("missing .nvmrc should return an error");

        assert!(
            err.to_string().contains("未找到 .nvmrc"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn resolve_project_version_from_errors_when_nvmrc_content_is_invalid() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root)?;
        fs::write(project_root.join(".nvmrc"), b"lts/*\n")?;

        let err = resolve_project_version_from(&project_root)
            .expect_err("invalid .nvmrc content should return an error");

        assert!(
            err.to_string().contains(".nvmrc 内容无法识别"),
            "unexpected error: {err:#}"
        );
        Ok(())
    }
}
