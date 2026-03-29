use std::path::Path;

/// 返回网络故障排查建议文本，用于下载失败等场景的错误提示补充。
pub fn network_diagnostic_tips() -> &'static str {
    "下载遇到问题了？试试这些方法：\n  1. 先用浏览器打开下载链接，看看能不能访问。\n  2. 如果在学校或公司网络，可能需要配置代理（可以问问网管或同学怎么设置）。\n  3. 检查电脑时间是否准确（时间不对可能导致连接失败）。\n  4. 换个网络试试，比如用手机热点。\n\n  还是不行？发邮件求助，我们会帮你：xinghang_a@proton.me"
}

/// 返回 quick-install 场景的常用恢复命令提示。
pub fn quick_install_help_commands() -> &'static str {
    "参考命令：\n  - meetai quick-install --python-version latest\n  - meetai runtime install python <version>\n  - meetai python list"
}

/// 根据当前 PATH 状态输出 Python 命令生效指引（Windows/Unix 分平台提示）。
pub fn print_python_path_guidance(shims_in_path: bool, shims_dir: &Path) {
    if shims_in_path {
        println!("终端中直接运行 python --version / pip --version 即可确认版本。");
        return;
    }

    println!("✅ Python 已经安装好了！最后一步，让终端认识它：");
    println!();
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
    println!("  配置完成后运行 python --version / pip --version 确认版本。");
}

/// 根据当前 PATH 状态输出 Node.js 命令生效指引（Windows/Unix 分平台提示）。
pub fn print_node_path_guidance(shims_in_path: bool, shims_dir: &Path) {
    if shims_in_path {
        println!("终端中直接运行 node --version / npm --version / npx --version 即可确认版本。");
        return;
    }

    println!("✅ Node.js 已经安装好了！最后一步，让终端认识它：");
    println!();
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
    println!("  配置完成后运行 node --version / npm --version / npx --version 确认版本。");
}
