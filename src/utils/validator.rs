//! 输入验证器模块。
//!
//! 本模块集中封装 CLI 输入参数的格式校验，重点覆盖版本号、包名以及
//! 可能影响命令执行安全性的 token。
//!
//! 核心类型：
//! - `Validator`: 验证器主类型，封装所有验证方法
//!
//! 主要校验能力：
//! - Python 安装版本：`latest` 或 `X.Y.Z`
//! - Python / Node.js 已安装版本选择：严格要求 `X.Y.Z`
//! - Node.js 安装版本：`latest`、`newest`、`lts`、`project` 或 `X.Y.Z`
//! - Java / Go 安装版本：`latest` 或最多三段数字版本
//! - pip 包名与精确版本：拒绝空白、控制字符和选项注入形式
//! - quick-install 中的虚拟环境名称：复用 `validate_package_name()`
//!
//! 主要方法：
//! - `validate_python_install_version()`: 验证 Python 安装请求版本
//! - `validate_python_selected_version()`: 验证已安装 Python 版本选择
//! - `validate_node_install_version()`: 验证 Node.js 安装请求版本
//! - `validate_node_use_version()`: 验证 Node.js 切换请求版本
//! - `validate_java_install_version()`: 验证 Java 安装请求版本
//! - `validate_go_install_version()`: 验证 Go 安装请求版本
//! - `validate_pip_package_name()`: 验证 pip 包名格式
//! - `validate_pip_pin_version()`: 验证 `name==version` 中的版本片段
//! - `validate_package_name()`: 验证普通名称 token（如 quick-install 的 venv 名称）
//!
//! 设计特点：
//! - **零状态**: `Validator` 是零大小、无状态类型
//! - **正则缓存**: 使用 `OnceLock` 缓存编译后的正则表达式，避免重复编译
//! - **错误详细**: 验证失败返回 `anyhow::Error`，包含具体错误原因和正确格式示例
//! - **按入口收紧**: 是否允许 `latest`、`lts`、`project` 等特殊 token 由具体方法决定
//!
//! 使用示例：
//! ```rust
//! use meetai::utils::validator::Validator;
//!
//! let validator = Validator::new();
//! validator.validate_python_install_version("latest")?;
//! validator.validate_node_install_version("lts")?;
//! validator.validate_pip_package_name("requests")?;
//! validator.validate_pip_pin_version("2.31.0")?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! 错误处理：
//! - 格式不匹配：返回 `anyhow::Error`
//! - 错误消息包含：实际输入、期望格式、允许的字符集
//!
//! 测试：
//! - 模块内 `mod tests` 包含各类验证函数的正负测试用例

use anyhow::{Context, Result};
use regex::Regex;
use semver::Version;
use std::sync::OnceLock;

fn re_python_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+\.\d+\.\d+$").unwrap())
}

fn re_package_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap())
}

fn re_node_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+\.\d+\.\d+$").unwrap())
}

fn re_pip_package_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").unwrap())
}

fn re_pip_pin_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$").unwrap())
}

fn re_optional_numeric_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+(\.\d+){0,2}$").unwrap())
}

/// 验证器
pub struct Validator;

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    /// 创建输入验证器实例。
    pub fn new() -> Self {
        Self
    }

    /// 验证 Python 版本号格式（接受 X.Y.Z，不接受 latest）。
    pub fn validate_python_version(&self, version: &str) -> Result<()> {
        if !re_python_version().is_match(version) {
            anyhow::bail!(
                "Python 版本号格式不正确：{}，请使用 X.Y.Z 格式，例如: 3.13.2",
                version
            );
        }

        Ok(())
    }

    /// 验证 Python 安装请求版本（允许 latest）。
    pub fn validate_python_install_version(&self, version: &str) -> Result<()> {
        self.validate_python_version_token(version, true)
    }

    /// 验证 Python 已安装版本选择（不允许 latest）。
    pub fn validate_python_selected_version(&self, version: &str) -> Result<()> {
        self.validate_python_version_token(version, false)
    }

    /// 验证 Node.js 安装请求版本（允许 latest / newest / lts / project）。
    pub fn validate_node_install_version(&self, version: &str) -> Result<()> {
        self.validate_node_version_token(version, &["latest", "newest", "lts", "project"])
    }

    /// 验证 Node.js 切换请求版本（允许 project）。
    pub fn validate_node_use_version(&self, version: &str) -> Result<()> {
        self.validate_node_version_token(version, &["project"])
    }

    /// 验证 Node.js 已安装版本选择（仅允许 X.Y.Z）。
    pub fn validate_node_selected_version(&self, version: &str) -> Result<()> {
        self.validate_node_version_token(version, &[])
    }

    /// 验证 Java 安装请求版本（允许 latest 或最多三段数字版本）。
    pub fn validate_java_install_version(&self, version: &str) -> Result<()> {
        self.validate_optional_latest_numeric_version("Java", version, "21 / 21.0.2")
    }

    /// 验证 Go 安装请求版本（允许 latest 或最多三段数字版本）。
    pub fn validate_go_install_version(&self, version: &str) -> Result<()> {
        self.validate_optional_latest_numeric_version("Go", version, "1.22 / 1.22.0")
    }

    /// 验证 Pip 版本号格式
    pub fn validate_pip_version(&self, version: &str) -> Result<()> {
        Version::parse(version).with_context(|| {
            format!(
                "pip 版本号格式不正确：{}，请使用语义化版本格式，例如: 24.0.0",
                version
            )
        })?;

        Ok(())
    }

    /// 验证包名格式
    pub fn validate_package_name(&self, name: &str) -> Result<()> {
        if !re_package_name().is_match(name) {
            anyhow::bail!("包名格式不正确：{}，只支持字母、数字、下划线和连字符", name);
        }

        Ok(())
    }

    /// 验证 pip 包名（防止参数注入）
    pub fn validate_pip_package_name(&self, name: &str) -> Result<()> {
        if name.starts_with('-') {
            anyhow::bail!("pip 包名不能以 '-' 开头：{}", name);
        }
        if name.chars().any(char::is_whitespace) {
            anyhow::bail!("pip 包名不能包含空白字符：{}", name);
        }
        if name.chars().any(char::is_control) {
            anyhow::bail!("pip 包名不能包含控制字符：{}", name);
        }

        let re = re_pip_package_name();
        if !re.is_match(name) {
            anyhow::bail!(
                "pip 包名格式不正确：{}，只支持字母、数字、点、下划线和连字符",
                name
            );
        }

        Ok(())
    }

    /// 验证 pip 精确版本号（用于 name==version）
    pub fn validate_pip_pin_version(&self, version: &str) -> Result<()> {
        if version.starts_with('-') {
            anyhow::bail!("pip 版本不能以 '-' 开头：{}", version);
        }
        if version.chars().any(char::is_whitespace) {
            anyhow::bail!("pip 版本不能包含空白字符：{}", version);
        }
        if version.chars().any(char::is_control) {
            anyhow::bail!("pip 版本不能包含控制字符：{}", version);
        }

        let re = re_pip_pin_version();
        if !re.is_match(version) {
            anyhow::bail!(
                "pip 版本格式不正确：{}，只支持字母、数字、点、下划线、加号和连字符",
                version
            );
        }

        Ok(())
    }

    /// 内部实现：验证 Python 版本号 token。
    /// `allow_latest = true` 时接受字面量 `"latest"`；否则只接受 `X.Y.Z` 格式。
    fn validate_python_version_token(&self, version: &str, allow_latest: bool) -> Result<()> {
        if allow_latest && version == "latest" {
            return Ok(());
        }

        if version.chars().any(char::is_whitespace) {
            anyhow::bail!("Python 版本号不能包含空白字符：{}", version);
        }
        if version.chars().any(char::is_control) {
            anyhow::bail!("Python 版本号不能包含控制字符：{}", version);
        }

        let re = re_python_version();
        if !re.is_match(version) {
            if allow_latest {
                anyhow::bail!(
                    "Python 版本号格式不正确：{}，请使用 latest 或 X.Y.Z 格式，例如: latest / 3.13.2",
                    version
                );
            }
            anyhow::bail!(
                "Python 版本号格式不正确：{}，请使用 X.Y.Z 格式，例如: 3.13.2",
                version
            );
        }

        Ok(())
    }

    /// 内部实现：验证 Node.js 版本号 token。
    fn validate_node_version_token(
        &self,
        version: &str,
        allowed_special_tokens: &[&str],
    ) -> Result<()> {
        if allowed_special_tokens.contains(&version) {
            return Ok(());
        }

        if version.chars().any(char::is_whitespace) {
            anyhow::bail!("Node.js 版本号不能包含空白字符：{}", version);
        }
        if version.chars().any(char::is_control) {
            anyhow::bail!("Node.js 版本号不能包含控制字符：{}", version);
        }

        let re = re_node_version();
        if !re.is_match(version) {
            if allowed_special_tokens.is_empty() {
                anyhow::bail!(
                    "Node.js 版本号格式不正确：{}，请使用 X.Y.Z 格式，例如: 20.11.1",
                    version
                );
            }

            let token_examples = allowed_special_tokens.join(" / ");
            anyhow::bail!(
                "Node.js 版本号格式不正确：{}，请使用 {} 或 X.Y.Z 格式，例如: {} / 20.11.1",
                version,
                token_examples,
                token_examples
            );
        }

        Ok(())
    }

    /// 内部实现：验证允许 latest 的数字版本 token。
    fn validate_optional_latest_numeric_version(
        &self,
        runtime_name: &str,
        version: &str,
        example: &str,
    ) -> Result<()> {
        if version == "latest" {
            return Ok(());
        }

        if version.chars().any(char::is_whitespace) {
            anyhow::bail!("{} 版本号不能包含空白字符：{}", runtime_name, version);
        }
        if version.chars().any(char::is_control) {
            anyhow::bail!("{} 版本号不能包含控制字符：{}", runtime_name, version);
        }

        let re = re_optional_numeric_version();
        if !re.is_match(version) {
            anyhow::bail!(
                "{} 版本号格式不正确：{}，请使用 latest 或数字版本，例如: {}",
                runtime_name,
                version,
                example
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_package_name_rejects_option_like_input() {
        let validator = Validator::new();
        assert!(validator.validate_pip_package_name("--index-url").is_err());
    }

    #[test]
    fn pip_package_name_allows_common_forms() {
        let validator = Validator::new();
        assert!(validator.validate_pip_package_name("requests").is_ok());
        assert!(validator
            .validate_pip_package_name("zope.interface")
            .is_ok());
        assert!(validator.validate_pip_package_name("my_pkg-name").is_ok());
    }

    #[test]
    fn pip_pin_version_rejects_whitespace_and_options() {
        let validator = Validator::new();
        assert!(validator.validate_pip_pin_version("--pre").is_err());
        assert!(validator.validate_pip_pin_version("1.2.3 rc1").is_err());
    }

    #[test]
    fn pip_pin_version_allows_pep440_like_tokens() {
        let validator = Validator::new();
        assert!(validator.validate_pip_pin_version("1.2.3").is_ok());
        assert!(validator.validate_pip_pin_version("1.2.3rc1").is_ok());
        assert!(validator.validate_pip_pin_version("1.2.3.post1").is_ok());
    }

    #[test]
    fn python_install_version_allows_latest_and_three_segment_form() {
        let validator = Validator::new();
        assert!(validator.validate_python_install_version("latest").is_ok());
        assert!(validator.validate_python_install_version("3.13.2").is_ok());
        assert!(validator
            .validate_python_install_version("3.15.0a6")
            .is_err());
    }

    #[test]
    fn python_version_rejects_two_segment_form() {
        let validator = Validator::new();
        assert!(validator.validate_python_install_version("3.13").is_err());
        assert!(validator.validate_python_selected_version("3.13").is_err());
        assert!(validator.validate_python_version("3.13").is_err());
    }

    #[test]
    fn python_install_version_rejects_path_like_input() {
        let validator = Validator::new();
        assert!(validator
            .validate_python_install_version("../3.13.2")
            .is_err());
        assert!(validator
            .validate_python_install_version("3.13.2/../../evil")
            .is_err());
    }

    #[test]
    fn python_selected_version_rejects_latest() {
        let validator = Validator::new();
        assert!(validator
            .validate_python_selected_version("latest")
            .is_err());
        assert!(validator.validate_python_selected_version("3.13.2").is_ok());
    }

    #[test]
    fn node_install_version_allows_special_tokens_and_three_segment_form() {
        let validator = Validator::new();
        assert!(validator.validate_node_install_version("latest").is_ok());
        assert!(validator.validate_node_install_version("newest").is_ok());
        assert!(validator.validate_node_install_version("lts").is_ok());
        assert!(validator.validate_node_install_version("project").is_ok());
        assert!(validator.validate_node_install_version("20.11.1").is_ok());
        assert!(validator.validate_node_install_version("20.11").is_err());
    }

    #[test]
    fn node_install_version_rejects_path_like_input() {
        let validator = Validator::new();
        assert!(validator
            .validate_node_install_version("../20.11.1")
            .is_err());
        assert!(validator
            .validate_node_install_version("20.11.1/../../evil")
            .is_err());
    }

    #[test]
    fn node_use_version_allows_project_but_rejects_latest() {
        let validator = Validator::new();
        assert!(validator.validate_node_use_version("project").is_ok());
        assert!(validator.validate_node_use_version("latest").is_err());
        assert!(validator.validate_node_use_version("20.11.1").is_ok());
    }

    #[test]
    fn node_selected_version_rejects_special_tokens() {
        let validator = Validator::new();
        assert!(validator.validate_node_selected_version("latest").is_err());
        assert!(validator.validate_node_selected_version("project").is_err());
        assert!(validator.validate_node_selected_version("20.11.1").is_ok());
    }

    #[test]
    fn java_install_version_allows_latest_and_numeric_forms() {
        let validator = Validator::new();
        assert!(validator.validate_java_install_version("latest").is_ok());
        assert!(validator.validate_java_install_version("21").is_ok());
        assert!(validator.validate_java_install_version("21.0.2").is_ok());
    }

    #[test]
    fn java_install_version_rejects_suffix_form() {
        let validator = Validator::new();
        assert!(validator.validate_java_install_version("21-ea").is_err());
    }

    #[test]
    fn go_install_version_allows_latest_and_numeric_forms() {
        let validator = Validator::new();
        assert!(validator.validate_go_install_version("latest").is_ok());
        assert!(validator.validate_go_install_version("1.22").is_ok());
        assert!(validator.validate_go_install_version("1.22.2").is_ok());
    }

    #[test]
    fn go_install_version_rejects_suffix_form() {
        let validator = Validator::new();
        assert!(validator.validate_go_install_version("1.22beta1").is_err());
    }
}
