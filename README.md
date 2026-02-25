# MeetAI

凌晨，你终于决定今晚动手写那个搁置已久的项目。

然后你遇到了环境配置。

两个小时后，你还在跟一条报错死磕。代码一行没写，激情却消磨了大半。

这种感觉，我们都懂。

MeetAI 是福州大学的创新创业社团，我们自己也是从这里走过来的。
那些因为环境问题而放弃的夜晚，那些差点被磨光的热情——我们不希望这件事继续发生在更多人身上。

这个仓库，是我们给后来者递出的那把钥匙：
**跳过最难熬的第一步，把时间还给真正值得的事情。**

## 什么是统一管理 CLI（通俗解释）

你可以把它理解成一个“技术工具总入口”。  
以后不需要记很多零散命令，只要记住一个入口指令：`meetai`。

- 今天，`meetai` 主要解决运行时安装与版本管理（Python / Node.js / Java / Go）
- 后续，`meetai` 会继续扩展到更多工程能力（例如项目部署一键指令等）

一句话：`meetai` 不只是某一个小工具，而是一组开发工具能力的统一入口。

## 项目简介

- 入口命令：`meetai`
- 当前定位：多语言运行时统一管理 CLI
- 当前重点：先稳定 Python 全链路，再逐步补齐 Node.js / Java / Go，并持续扩展更多开发场景能力

## 当前支持矩阵

1. Python
- 运行时安装/切换/卸载：已支持
- pip 管理：已支持
- venv 管理：已支持
- quick-install：已支持

2. Node.js
- 统一命令入口：已预留
- 自动安装与版本切换：开发中

3. Java
- 统一命令入口：已预留
- 自动安装与版本切换：开发中

4. Go
- 统一命令入口：已预留
- 自动安装与版本切换：开发中

## 命令示例

1. 统一 runtime 命令（推荐）
```powershell
# 查看支持矩阵
meetai runtime list

# 查看 Python 已安装版本
meetai runtime list python

# 安装并切换 Python（自动解析最新稳定版）
meetai runtime install python latest
meetai runtime use python 3.14.3

# 或安装指定版本
meetai runtime install python 3.13.2
meetai runtime use python 3.13.2

# 卸载 Python
meetai runtime uninstall python 3.13.2
```

2. Python 专项命令（兼容保留）
```powershell
meetai python list
meetai python install 3.13.2
meetai python use 3.13.2
meetai python uninstall 3.13.2
```

3. pip 与 venv
```powershell
meetai pip install requests
meetai pip list
meetai venv create demo --target-dir .
meetai venv activate demo
```

3.1 让 `python` 命令跟随 `meetai python use` 切换（推荐）
```powershell
# 当前会话立即生效
$env:Path = "<meetai.exe目录>/.meetai/shims;$env:Path"

# 持久生效（用户级，执行后重开终端）
[Environment]::SetEnvironmentVariable("Path", "<meetai.exe目录>/.meetai/shims;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```

4. 一键安装
```powershell
# Python 一键初始化
meetai quick-install

# 关闭 venv
meetai quick-install --create-venv false

# 多语言参数入口（Node.js / Java / Go 当前为规划能力，命令可接受参数并在流程中提示）
meetai quick-install --install-nodejs true --nodejs-version 20.11.1 --install-java true --java-version 21 --install-go true --go-version 1.22.2
```

## 快速开始

1. 构建
```powershell
cargo build
```

2. 查看帮助
```powershell
cargo run -- --help
```

3. 本地测试
```powershell
cargo test
```

## 配置目录

默认目录（与 `meetai.exe` 同级）：
- 配置：`<meetai.exe目录>/.meetai/config.json`
- Python 安装目录：`<meetai.exe目录>/.meetai/python`
- shims 目录：`<meetai.exe目录>/.meetai/shims`
- venv 目录：`<meetai.exe目录>/.meetai/venvs`
- 下载缓存：`<meetai.exe目录>/.meetai/cache`

迁移说明：
- 如果检测到旧目录 `<meetai.exe目录>/.python-manager` 且 `<meetai.exe目录>/.meetai` 不存在，程序会自动尝试迁移。

## 已知限制

1. Python 自动安装目前仅支持 Windows。
2. `latest` 版本解析（适用于 `runtime install python latest` 与 `quick-install --python-version latest`）采用多级回退：
- 下载页解析 -> FTP 索引解析 -> 本地最高版本 -> 内置版本 `3.11.0`
3. Windows 下 Python 安装包下载采用双源策略：
- 先尝试 Python 官方源
- 官方源失败后自动回退到清华镜像源
4. `venv activate` 会输出激活命令，不能直接修改父 shell。
5. `python use` 仅更新 MeetAI 配置，不会直接修改父终端环境；建议将 `.meetai/shims` 加入 PATH 以获得“切换即生效”体验。
6. Node.js / Java / Go 当前仅完成统一入口与参数预留，安装器仍在开发中。

## 路线图

1. Phase 1（已完成）
- MeetAI 命令品牌切换
- Python 安装链路稳定性增强
- 一键安装流程与测试体系完善

2. Phase 2（进行中）
- Node.js / Java / Go 安装器实现
- 多 runtime 版本切换落地
- 统一状态查看与诊断命令

3. Phase 3（计划）
- 离线缓存策略
- 项目级锁定文件
- 跨平台安装策略统一（Windows / macOS / Linux）

## 开源协作建议

- 先从 `runtime` 子命令补齐 Node.js / Java / Go 的安装器。
- 每个语言保持统一动作：`list / install / use / uninstall`。
- 新增能力同时补测试并保持 `cargo fmt --check` 与 `cargo test` 通过。
