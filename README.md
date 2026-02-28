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
- 当前定位：开发环境统一管理 CLI（当前 MVP 聚焦 Python）
- MVP 范围（当前版本）：`Python runtime + pip + venv + quick-install` 主链路可用
- 当前重点：持续打磨 Python 体验，并逐步补齐 Node.js / Java / Go

## 版本状态（MVP）

当前版本已达到 **Python 场景 MVP**：

1. 可用能力
- Python：安装 / 切换 / 卸载
- pip：安装 / 卸载 / 升级 / 列表
- venv：创建 / 激活提示 / 列表
- quick-install：Python 一键初始化（可选 venv）

2. 仍在开发
- Node.js / Java / Go：统一入口已预留，自动安装与版本切换尚未完成

## 当前支持矩阵

1. Python
- 运行时安装/切换/卸载：已支持
- pip 管理：已支持
- venv 管理：已支持
- quick-install：已支持

2. Node.js
- 统一命令入口：已预留
- 自动安装与版本切换：规划中（当前版本未开放）

3. Java
- 统一命令入口：已预留
- 自动安装与版本切换：规划中（当前版本未开放）

4. Go
- 统一命令入口：已预留
- 自动安装与版本切换：规划中（当前版本未开放）

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

# 多语言参数入口（Node.js / Java / Go 当前仅参数预留，实际会给出“手动安装”提示）
meetai quick-install --install-nodejs true --nodejs-version 20.11.1 --install-java true --java-version 21 --install-go true --go-version 1.22.2
```

## 安装

> 以下步骤无需安装 Rust，直接下载编译好的 exe 即可。
> 如果你想参与开发或从源码构建，参见下文"快速开始"。

### 第一步：下载 meetai.exe

前往 [GitHub Releases](https://github.com/meetai-club/meetai/releases)，下载最新版本的 `meetai.exe`。

### 第二步：将 exe 放入专属目录

```powershell
# 创建目录（若已存在会跳过）
New-Item -ItemType Directory -Force "$env:USERPROFILE\.meetai\bin"

# 将下载的 meetai.exe 移动到该目录（按实际下载路径修改）
Move-Item "$env:USERPROFILE\Downloads\meetai.exe" "$env:USERPROFILE\.meetai\bin\meetai.exe"
```

### 第三步：将目录加入 PATH（用户级，无需管理员权限）

```powershell
[Environment]::SetEnvironmentVariable(
    "Path",
    "$env:USERPROFILE\.meetai\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)
```

执行后**重新打开终端**，PATH 即永久生效。

> 如果你更习惯图形界面：`Win + S` → 搜索"**编辑系统环境变量**" → 用户变量 → `Path` → 新建 → 粘贴以下路径：
> ```
> C:\Users\<你的用户名>\.meetai\bin
> ```

### 第四步：验证安装

```powershell
meetai --help
```

看到帮助信息即安装成功。

---

> **升级**：从 Releases 下载新版 exe，重新执行第二步的 `Move-Item` 覆盖旧文件即可，无需重新配置 PATH。

## 快速开始

> 面向参与开发或想从源码构建的用户，需要先安装 [Rust 工具链](https://rustup.rs/)。

1. 构建
```powershell
cargo build --release
```

2. 查看帮助
```powershell
cargo run -- --help
```

3. 本地测试
```powershell
cargo test --locked
```

## MVP 验证流程（Windows）

下面这组命令用于验证当前 **Python 场景 MVP** 是否可用。  
如果你已经构建了二进制，可把 `cargo run --` 替换为 `meetai`。

1. 查看支持矩阵
```powershell
cargo run -- runtime list
```

2. 安装并切换 Python（latest）
```powershell
cargo run -- runtime install python latest
# 按上一条命令输出的实际版本号替换 <version>
cargo run -- runtime use python <version>
```

3. 验证 Python 版本已被管理
```powershell
cargo run -- runtime list python
```

4. 验证 pip 主链路
```powershell
cargo run -- pip install requests
cargo run -- pip list
cargo run -- pip uninstall requests
```

5. 验证 venv 主链路
```powershell
cargo run -- venv create demo --target-dir .
cargo run -- venv list
cargo run -- venv activate demo
```

6. 验证 quick-install 主链路
```powershell
cargo run -- quick-install --create-venv false
```

7. MVP 验收标准
- `runtime install/use/list python` 全部执行成功
- `pip install/list/uninstall` 全部执行成功
- `venv create/list/activate` 全部执行成功（`activate` 输出激活命令即可）
- `quick-install` 执行成功并输出安装摘要
- `cargo test --locked` 通过

## MVP 常见问题与最短排查路径

1. 网络下载失败（`runtime install python latest` / `quick-install`）
- 先改用具体版本重试：`cargo run -- runtime install python 3.13.2`
- 再查看详细日志：`cargo run -- --verbose runtime install python 3.13.2`
- 如仍失败，检查代理/网络策略后重试

2. 执行 `runtime use python <version>` 后 `python --version` 未切换
- 先确认版本已切换：`cargo run -- runtime list python`
- 将 `.meetai/shims` 加入 PATH（见上文“3.1”）
- 重开终端后再执行：`python --version`

3. pip/venv 命令提示“还没有选择 Python 版本”
- 先查看已安装版本：`cargo run -- runtime list python`
- 再执行切换：`cargo run -- runtime use python <version>`
- 最后重试：`cargo run -- pip list` 或 `cargo run -- venv list`

## 配置目录

默认目录（与 `meetai.exe` 同级）：
- 配置：`<meetai.exe目录>/.meetai/config.json`
- Python 安装目录：`<meetai.exe目录>/.meetai/python`
- shims 目录：`<meetai.exe目录>/.meetai/shims`
- venv 目录：`<meetai.exe目录>/.meetai/venvs`
- 下载缓存：`<meetai.exe目录>/.meetai/cache`

迁移说明：
- 如果检测到旧目录 `<meetai.exe目录>/.python-manager` 且 `<meetai.exe目录>/.meetai` 不存在，程序会自动尝试迁移。

## 使用须知

1. Python 自动安装目前仅支持 Windows；macOS / Linux 支持正在路线图中。
2. `latest` 版本解析（适用于 `runtime install python latest` 与 `quick-install --python-version latest`）依次尝试以下来源，任意一级成功即停止：
- 官方下载页解析 → FTP 索引解析 → 本地已安装最高版本 → 内置保底版本 `3.11.0`
3. Windows 下 Python 安装包采用双源下载策略，网络受限时无需手动切源：
- 优先请求 Python 官方源
- 官方源失败后自动回退到清华镜像源
4. `venv activate` 输出的是激活命令供你复制执行，程序无法直接修改父 shell 的环境变量，这是 Shell 的安全限制。
5. `python use` 更新 MeetAI 内部配置后，建议将 `.meetai/shims` 加入 PATH，之后终端中的 `python` 命令就会跟随 `meetai python use` 自动切换，无需每次手动操作（详见上文 “3.1”）。
6. Node.js / Java / Go 的统一命令入口已预留，自动安装与版本切换正在开发中。

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

## 参与贡献

欢迎一起把 MeetAI 做得更好，贡献方向优先级供参考：

- 从 `runtime` 子命令入手，补齐 Node.js / Java / Go 的安装器。
- 每个语言保持统一动作接口：`list / install / use / uninstall`。
- 新增能力请同步补测试，并确保 `cargo fmt --check` 与 `cargo test` 通过后再提交。
