use std::path::Path;

/// 返回网络故障排查建议文本，用于下载失败等场景的错误提示补充。
pub fn network_diagnostic_tips() -> &'static str {
    "网络诊断建议：\n  1. 用浏览器直接打开下载链接，确认网络可达。\n  2. 如果在校园网/公司网络，请配置代理后重试（环境变量：HTTP_PROXY / HTTPS_PROXY）。\n  3. 检查系统时间是否准确，避免 TLS 证书校验失败。\n  4. 尝试切换网络（如手机热点）后重试。"
}

/// 返回 quick-install 场景的常用恢复命令提示。
pub fn quick_install_help_commands() -> &'static str {
    "参考命令：\n  - meetai quick-install --python-version latest\n  - meetai runtime install python <version>\n  - meetai python list"
}

/// 根据当前 PATH 状态输出 Python 命令生效指引（Windows/Unix 分平台提示）。
pub fn print_python_path_guidance(shims_in_path: bool, shims_dir: &Path) {
    if shims_in_path {
        println!("终端中直接运行 python --version 即可确认版本。");
        return;
    }

    println!("还需要把 MeetAI 的 shims 目录加入 PATH 才能生效，选一种方式：");
    println!();
    if cfg!(windows) {
        println!("  # 当前终端临时生效（关闭后失效）：");
        println!("  $env:Path = \"{};$env:Path\"", shims_dir.display());
        println!();
        println!("  # 永久生效（重开终端后生效）：");
        println!(
            "  [Environment]::SetEnvironmentVariable(\"Path\", \"{};\" + [Environment]::GetEnvironmentVariable(\"Path\", \"User\"), \"User\")",
            shims_dir.display()
        );
    } else {
        println!("  # 当前终端临时生效（关闭后失效）：");
        println!("  export PATH=\"{}:$PATH\"", shims_dir.display());
        println!();
        println!("  # 永久生效（添加到 ~/.bashrc 或 ~/.zshrc，重开终端后生效）：");
        println!(
            "  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc",
            shims_dir.display()
        );
    }
    println!();
    println!("  配置完成后运行 python --version 确认版本。");
}
