use anyhow::Result;
use meetai::cli::{RuntimeAction, RuntimeArgs, RuntimeType};
use meetai::config::Config;
use meetai::runtime::handle_runtime_command;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn python_executable_in_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.join("python.exe")
    } else {
        install_dir.join("bin/python")
    }
}

fn test_version(seed: u8) -> String {
    format!("9.9.{}", 30 + seed)
}

fn prepare_fake_install(version: &str) -> Result<(Config, PathBuf)> {
    let config = Config::load()?;
    config.ensure_dirs()?;
    let install_dir = config.python_install_dir.join(format!("python-{version}"));
    std::fs::create_dir_all(&install_dir)?;
    let python_exe = python_executable_in_dir(&install_dir);
    if let Some(parent) = python_exe.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&python_exe, b"fake-python")?;
    Ok((config, install_dir))
}

fn with_path_prefixed(prefix: &Path) -> Result<Option<OsString>> {
    let old_path = std::env::var_os("PATH");
    let mut entries = vec![prefix.to_path_buf()];
    if let Some(existing) = &old_path {
        entries.extend(std::env::split_paths(existing));
    }
    let joined = std::env::join_paths(entries)?;
    // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
    unsafe { std::env::set_var("PATH", joined) };
    Ok(old_path)
}

fn restore_path(old_path: Option<OsString>) {
    match old_path {
        Some(value) => {
            // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
            unsafe { std::env::set_var("PATH", value) };
        }
        None => {
            // SAFETY: tests run under a global lock in this file to avoid concurrent env mutation.
            unsafe { std::env::remove_var("PATH") };
        }
    }
}

#[tokio::test]
async fn runtime_install_python_uses_python_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let version = test_version(1);
    let (_config, install_dir) = prepare_fake_install(&version)?;

    let result = handle_runtime_command(RuntimeArgs {
        action: RuntimeAction::Install {
            runtime: RuntimeType::Python,
            version: version.clone(),
        },
    })
    .await;

    assert!(result.is_ok(), "runtime install should succeed: {result:?}");

    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)?;
    }
    Ok(())
}

#[tokio::test]
async fn runtime_use_python_uses_python_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let version = test_version(2);
    let (mut config, install_dir) = prepare_fake_install(&version)?;
    let original_current = config.current_python_version.clone();

    let app_home = config
        .python_install_dir
        .parent()
        .expect("python_install_dir should have app home parent")
        .to_path_buf();
    let shims_dir = app_home.join("shims");
    std::fs::create_dir_all(&shims_dir)?;
    let old_path = with_path_prefixed(&shims_dir)?;

    let result = handle_runtime_command(RuntimeArgs {
        action: RuntimeAction::Use {
            runtime: RuntimeType::Python,
            version: version.clone(),
        },
    })
    .await;

    restore_path(old_path);
    assert!(result.is_ok(), "runtime use should succeed: {result:?}");

    config = Config::load()?;
    assert_eq!(
        config.current_python_version.as_deref(),
        Some(version.as_str())
    );

    config.current_python_version = original_current;
    config.save()?;
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)?;
    }
    Ok(())
}

#[tokio::test]
async fn runtime_uninstall_python_uses_python_service_path() -> Result<()> {
    let _guard = lock().lock().await;
    let version = test_version(3);
    let (_config, install_dir) = prepare_fake_install(&version)?;

    let result = handle_runtime_command(RuntimeArgs {
        action: RuntimeAction::Uninstall {
            runtime: RuntimeType::Python,
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
    Ok(())
}
