use crate::cli::{NpmAction, NpmArgs};
use crate::node::version::NodeVersionManager;
use crate::utils::executor::CommandExecutor;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const MISSING_NODE_SELECTION: &str = "还没有选择 Node.js 版本，请先执行: meetai node use <version>";

pub async fn handle_npm_command(args: NpmArgs) -> Result<()> {
    let manager = NpmManager::new()?;
    match args.action {
        NpmAction::Install { package } => {
            manager.install(&package).await?;
            println!("{} 已安装到当前 Node.js 版本的 npm 全局空间", package);
        }
        NpmAction::Uninstall { package } => {
            manager.uninstall(&package).await?;
            println!("{} 已从当前 Node.js 版本的 npm 全局空间卸载", package);
        }
        NpmAction::Upgrade { package } => {
            manager.upgrade(&package).await?;
            println!("{} 已在当前 Node.js 版本下升级", package);
        }
        NpmAction::List => {
            let packages = manager.list_current().await?;
            if packages.is_empty() {
                println!("当前 Node.js 版本还没有安装全局 npm 包。");
            } else {
                println!("当前 Node.js 版本的全局 npm 包：");
                for package in packages {
                    println!("  - {}", package);
                }
            }
        }
        NpmAction::Prefix => {
            println!("{}", manager.current_prefix()?.display());
        }
        NpmAction::Migrate { from, to } => {
            manager.migrate(&from, &to).await?;
        }
        NpmAction::RefreshShims => {
            manager.refresh_shims()?;
        }
    }
    Ok(())
}

pub struct NpmManager {
    node: NodeVersionManager,
    executor: CommandExecutor,
}

#[derive(Debug, Deserialize)]
struct NpmListPackage {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NpmListOutput {
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, NpmListPackage>,
}

impl NpmManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            node: NodeVersionManager::new()?,
            executor: CommandExecutor::new(),
        })
    }

    pub fn current_prefix(&self) -> Result<PathBuf> {
        self.node
            .ensure_current_npm_global_dirs(MISSING_NODE_SELECTION)
    }

    pub fn refresh_shims(&self) -> Result<()> {
        self.node.refresh_current_global_cli_shims()
    }

    pub async fn install(&self, package: &str) -> Result<()> {
        self.run_current_npm(&["install", "-g", package]).await?;
        self.refresh_shims()?;
        Ok(())
    }

    pub async fn uninstall(&self, package: &str) -> Result<()> {
        self.run_current_npm(&["uninstall", "-g", package]).await?;
        self.refresh_shims()?;
        Ok(())
    }

    pub async fn upgrade(&self, package: &str) -> Result<()> {
        self.run_current_npm(&["update", "-g", package]).await?;
        self.refresh_shims()?;
        Ok(())
    }

    pub async fn list_current(&self) -> Result<Vec<String>> {
        let npm = self.node.current_npm_executable(MISSING_NODE_SELECTION)?;
        let prefix = self.current_prefix()?;
        self.list_for_npm(&npm, &prefix).await
    }

    pub async fn migrate(&self, from: &str, to: &str) -> Result<()> {
        let from_dir = self.node.install_dir_for_version(from)?;
        let to_dir = self.node.install_dir_for_version(to)?;
        let from_npm = NodeVersionManager::npm_executable_for_install_dir(&from_dir);
        let to_npm = NodeVersionManager::npm_executable_for_install_dir(&to_dir);
        let from_prefix = NodeVersionManager::npm_global_prefix_for_install_dir(&from_dir);
        let to_prefix = NodeVersionManager::npm_global_prefix_for_install_dir(&to_dir);
        std::fs::create_dir_all(NodeVersionManager::npm_global_bin_for_prefix(&to_prefix))?;

        let packages = self.list_for_npm(&from_npm, &from_prefix).await?;
        let mut installed = Vec::new();
        let mut failed = Vec::new();
        for package in packages {
            let result = self
                .run_npm_with_prefix(&to_npm, &to_prefix, &["install", "-g", &package])
                .await;
            match result {
                Ok(()) => installed.push(package),
                Err(err) => failed.push(format!("{package}: {err}")),
            }
        }

        self.refresh_shims()?;
        println!("迁移完成：{} 成功，{} 失败", installed.len(), failed.len());
        for package in installed {
            println!("  ok  {}", package);
        }
        for item in failed {
            println!("  fail {}", item);
        }
        Ok(())
    }

    async fn run_current_npm(&self, args: &[&str]) -> Result<()> {
        let npm = self.node.current_npm_executable(MISSING_NODE_SELECTION)?;
        let prefix = self.current_prefix()?;
        self.run_npm_with_prefix(&npm, &prefix, args).await
    }

    async fn run_npm_with_prefix(&self, npm: &Path, prefix: &Path, args: &[&str]) -> Result<()> {
        let prefix_text = prefix.to_string_lossy().to_string();
        self.executor
            .execute_with_env(
                npm,
                args,
                &[
                    ("NPM_CONFIG_PREFIX", prefix_text.as_str()),
                    ("npm_config_prefix", prefix_text.as_str()),
                ],
            )
            .await
    }

    async fn list_for_npm(&self, npm: &Path, prefix: &Path) -> Result<Vec<String>> {
        let prefix_text = prefix.to_string_lossy().to_string();
        let output = self
            .executor
            .execute_with_output_async_env(
                npm,
                &["list", "-g", "--depth=0", "--json"],
                &[
                    ("NPM_CONFIG_PREFIX", prefix_text.as_str()),
                    ("npm_config_prefix", prefix_text.as_str()),
                ],
            )
            .await
            .context("读取 npm 全局包清单失败")?;

        parse_npm_list_output(&output)
    }
}

fn parse_npm_list_output(output: &str) -> Result<Vec<String>> {
    let parsed: NpmListOutput = serde_json::from_str(output).context("解析 npm 包清单失败")?;
    let mut packages = parsed
        .dependencies
        .into_iter()
        .filter(|(name, _)| name != "npm")
        .map(|(name, item)| match item.version {
            Some(version) if !version.is_empty() => format!("{name}@{version}"),
            _ => name,
        })
        .collect::<Vec<_>>();
    packages.sort();
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_npm_list_output_sorts_packages_and_filters_npm() -> Result<()> {
        let packages = parse_npm_list_output(
            r#"{
  "dependencies": {
    "typescript": { "version": "5.9.3" },
    "npm": { "version": "10.9.0" },
    "eslint": { "version": "9.39.1" }
  }
}"#,
        )?;

        assert_eq!(packages, vec!["eslint@9.39.1", "typescript@5.9.3"]);
        Ok(())
    }

    #[test]
    fn parse_npm_list_output_accepts_missing_dependencies() -> Result<()> {
        let packages = parse_npm_list_output(r#"{}"#)?;

        assert!(packages.is_empty());
        Ok(())
    }
}
