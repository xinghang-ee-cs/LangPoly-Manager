use anyhow::{Context, Result};
use regex::Regex;
use semver::Version;

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

    /// 验证 Python 版本号格式
    pub fn validate_python_version(&self, version: &str) -> Result<()> {
        let re = Regex::new(r"^\d+\.\d+\.\d+$").context("Failed to compile regex")?;

        if !re.is_match(version) {
            anyhow::bail!(
                "Python 版本号格式不正确：{}，请使用 X.Y.Z 格式，例如: 3.13.2",
                version
            );
        }

        Ok(())
    }

    /// 验证 Pip 版本号格式
    pub fn validate_pip_version(&self, version: &str) -> Result<()> {
        Version::parse(version).context("Invalid pip version format")?;

        Ok(())
    }

    /// 验证包名格式
    pub fn validate_package_name(&self, name: &str) -> Result<()> {
        let re = Regex::new(r"^[a-zA-Z0-9_-]+$").context("Failed to compile regex")?;

        if !re.is_match(name) {
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

        let re = Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
            .context("Failed to compile pip package regex")?;
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

        let re = Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")
            .context("Failed to compile pip pin version regex")?;
        if !re.is_match(version) {
            anyhow::bail!(
                "pip 版本格式不正确：{}，只支持字母、数字、点、下划线、加号和连字符",
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
}
