//! 用户指导信息模块。
//!
//! 本模块提供友好的用户提示信息，包括网络故障排查、PATH 配置指导、快速命令参考等。
//! 所有信息均为中文，面向中国用户群体。
//!
//! 主要函数：
//! - `network_diagnostic_tips()`: 网络故障排查步骤（代理、时间、网络切换）
//! - `quick_install_help_commands()`: 一键安装后的参考命令
//! - `print_python_path_guidance()`: 根据 PATH 状态输出 Python 激活指引
//! - `print_node_path_guidance()`: 根据 PATH 状态输出 Node.js 激活指引
//!
//! 设计理念：
//! - 错误消息**友好易懂**，避免技术术语堆砌
//! - 提供**可操作**的解决步骤，而非抽象描述
//! - 考虑中国用户网络环境（防火墙、代理、教育网）
//! - 区分 Windows 和 Unix 平台的命令差异
//!
//! 网络诊断建议内容：
//! 1. 浏览器测试：先用浏览器打开下载链接，确认网络可达
//! 2. 代理配置：学校/公司网络可能需要代理，询问网管或同学
//! 3. 系统时间：检查电脑时间是否准确（TLS 证书验证依赖）
//! 4. 网络切换：尝试手机热点或其他网络
//! 5. 联系支持：以上都不行，发邮件求助 xinghang_a@proton.me
//!
//! PATH 配置指引：
//! - 检测 shims 目录是否在 PATH 中
//! - Windows: 指导通过系统属性对话框添加
//! - Unix: 指导修改 `~/.bashrc`、`~/.zshrc` 等配置文件
//! - 提供验证命令：`python --version`、`which python`
//!
//! 使用示例：
//! ```rust
//! use std::path::Path;
//! use meetai::utils::guidance::print_python_path_guidance;
//!
//! let shims_dir = Path::new("/tmp/.meetai/shims");
//! print_python_path_guidance(false, shims_dir);
//! ```
//!
//! 该模块目前没有单独的单元测试，主要通过运行时流程和人工输出校对来验证。

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
