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

fn re_pip_package_name() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").unwrap())
}

fn re_pip_pin_version() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$").unwrap())
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

    /// 验证 Pip 版本号格式
    pub fn validate_pip_version(&self, version: &str) -> Result<()> {
        Version::parse(version).context("Invalid pip version format")?;

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
}
