use anyhow::Result;
use meetai::cli::{RuntimeAction, RuntimeArgs, RuntimeType};
use meetai::config::Config;
use meetai::node::NodeService;
use meetai::runtime::handle_runtime_command;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn node_executable_in_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.join("node.exe")
    } else {
        install_dir.join("bin/node")
    }
}

fn npm_executable_in_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.join("npm.cmd")
    } else {
        install_dir.join("bin/npm")
    }
}

fn npx_executable_in_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.join("npx.cmd")
    } else {
        install_dir.join("bin/npx")
    }
}

fn test_version(seed: u8) -> String {
    format!("8.8.{}", 40 + seed)
}

fn set_env_var(key: &str, value: &Path) -> Option<OsString> {
    let old_value = env::var_os(key);
    // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
    unsafe { env::set_var(key, value) };
    old_value
}

fn restore_env_var(key: &str, old_value: Option<OsString>) {
    match old_value {
        Some(value) => {
            // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
            unsafe { env::set_var(key, value) };
        }
        None => {
            // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
            unsafe { env::remove_var(key) };
        }
    }
}

fn with_path_prefixed(prefix: &Path) -> Result<Option<OsString>> {
    let old_path = env::var_os("PATH");
    let mut entries = vec![prefix.to_path_buf()];
    if let Some(existing) = &old_path {
        entries.extend(env::split_paths(existing));
    }
    let joined = env::join_paths(entries)?;
    // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
    unsafe { env::set_var("PATH", joined) };
    Ok(old_path)
}

fn restore_path(old_path: Option<OsString>) {
    restore_env_var("PATH", old_path);
}

fn prepare_fake_node_install(config: &Config, version: &str) -> Result<PathBuf> {
    let install_dir = config
        .app_home_dir_path()?
        .join("nodejs")
        .join("versions")
        .join(version);

    for executable in [
        node_executable_in_dir(&install_dir),
        npm_executable_in_dir(&install_dir),
        npx_executable_in_dir(&install_dir),
    ] {
        if let Some(parent) = executable.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(executable, b"fake-node")?;
    }

    Ok(install_dir)
}

#[tokio::test]
async fn runtime_install_nodejs_uses_node_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let temp = tempfile::tempdir()?;
    let app_home = temp.path().join("meetai-home");
    let old_home = set_env_var("MEETAI_HOME", &app_home);
    std::fs::create_dir_all(&app_home)?;

    let outcome = async {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let version = test_version(1);
        let install_dir = prepare_fake_node_install(&config, &version)?;
        assert!(
            node_executable_in_dir(&install_dir).exists(),
            "fake node executable should exist before runtime install"
        );
        assert!(
            NodeService::new()?
                .list_installed()?
                .iter()
                .any(|item| item.to_string() == version),
            "fake node version should be visible to NodeService before runtime install"
        );

        let result = handle_runtime_command(RuntimeArgs {
            action: RuntimeAction::Install {
                runtime: RuntimeType::NodeJs,
                version: version.clone(),
            },
        })
        .await;

        assert!(result.is_ok(), "runtime install should succeed: {result:?}");
        assert!(install_dir.exists(), "install dir should remain available");
        Ok(())
    }
    .await;

    restore_env_var("MEETAI_HOME", old_home);
    outcome
}

#[tokio::test]
async fn runtime_use_nodejs_project_uses_node_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let temp = tempfile::tempdir()?;
    let app_home = temp.path().join("meetai-home");
    let old_home = set_env_var("MEETAI_HOME", &app_home);
    std::fs::create_dir_all(&app_home)?;

    let outcome = async {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let version = test_version(2);
        let _install_dir = prepare_fake_node_install(&config, &version)?;

        let shims_dir = config.app_home_dir_path()?.join("shims");
        std::fs::create_dir_all(&shims_dir)?;
        let old_path = with_path_prefixed(&shims_dir)?;

        let old_cwd = env::current_dir()?;
        let project_root = temp.path().join("project");
        let nested = project_root.join("apps").join("web");
        std::fs::create_dir_all(&nested)?;
        std::fs::write(project_root.join(".nvmrc"), format!("v{version}\n"))?;
        env::set_current_dir(&nested)?;

        let result = handle_runtime_command(RuntimeArgs {
            action: RuntimeAction::Use {
                runtime: RuntimeType::NodeJs,
                version: "project".to_string(),
            },
        })
        .await;

        env::set_current_dir(&old_cwd)?;
        restore_path(old_path);
        assert!(result.is_ok(), "runtime use should succeed: {result:?}");

        let service = NodeService::new()?;
        assert_eq!(
            service.get_current_version()?.as_deref(),
            Some(version.as_str())
        );
        Ok(())
    }
    .await;

    restore_env_var("MEETAI_HOME", old_home);
    outcome
}

#[tokio::test]
async fn runtime_uninstall_nodejs_uses_node_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let temp = tempfile::tempdir()?;
    let app_home = temp.path().join("meetai-home");
    let old_home = set_env_var("MEETAI_HOME", &app_home);
    std::fs::create_dir_all(&app_home)?;

    let outcome = async {
        let config = Config::load()?;
        config.ensure_dirs()?;
        let version = test_version(3);
        let install_dir = prepare_fake_node_install(&config, &version)?;
        NodeService::new()?.set_current_version(&version)?;

        let result = handle_runtime_command(RuntimeArgs {
            action: RuntimeAction::Uninstall {
                runtime: RuntimeType::NodeJs,
                version: version.clone(),
            },
        })
        .await;

        assert!(
            result.is_ok(),
            "runtime uninstall should succeed: {result:?}"
        );
        assert!(
            !install_dir.exists(),
            "runtime uninstall should remove install dir: {}",
            install_dir.display()
        );
        assert!(
            NodeService::new()?.get_current_version()?.is_none(),
            "runtime uninstall should clear current node selection"
        );
        Ok(())
    }
    .await;

    restore_env_var("MEETAI_HOME", old_home);
    outcome
}
