# MeetAI

> 📖 **快速导航**：[完整命令参考](./COMMANDS.md) | [开发文档](./AGENTS.md) | [问题反馈](https://github.com/meetai-club/meetai/issues)

凌晨，你终于决定今晚动手写那个搁置已久的项目。

然后你遇到了环境配置。

两个小时后，你还在跟一条报错死磕。代码一行没写，激情却消磨了大半。

这种感觉，我们都懂。

MeetAI 是福州大学的创新创业社团，我们自己也是从这里走过来的。
那些因为环境问题而放弃的夜晚，那些差点被磨光的热情——我们不希望这件事继续发生在更多人身上。

这个仓库，是我们给后来者递出的那把钥匙：
**跳过最难熬的第一步，把时间还给真正值得的事情。**

---

## MeetAI 是什么？

简单说，MeetAI 是你的**开发工具箱统一入口**。

不再需要记忆一堆零散的命令，所有开发工具都通过一个指令完成：`meetai`。

- ✅ 当前支持：Python 的安装、版本管理和依赖操作，以及 Node.js 的安装与版本管理
- 🚧 规划中：Java、Go 以及项目部署等更多能力

一句话：MeetAI 不是又一个小工具，而是**一套开发工具能力的统一入口**。

---

## 为什么需要 MeetAI？

如果你曾经历过以下场景，你会懂：

- 新项目需要特定 Python 版本，折腾半天才配好环境
- 同时维护多个项目，每个都有不同的 Node.js 版本
- 刚接触编程，被环境配置的报错劝退
- 换了一台新电脑，又要重新安装所有工具

MeetAI 的目标很简单：**让你在 5 分钟内准备好开发环境，而不是 5 小时。**

---

## 🎯 全新手友好指南

### 你需要准备什么？

- 💻 Windows 10 或 Windows 11 电脑
- 🌐 能访问互联网（下载需要）
- ⏰ 5 分钟时间

### 第一步：打开终端（PowerShell）

**什么是终端？**
终端是一个输入命令的地方，你可以把它理解为"和电脑对话的工具"。

**如何打开？**
1. 按 `Win + X` 键
2. 选择"Windows PowerShell"或"终端"
3. 你会看到一个蓝色或黑色的窗口，那就是终端

或者：
- 按 `Win + R`，输入 `powershell`，按回车
- 在开始菜单搜索"PowerShell"

### 第二步：下载 MeetAI

1. 打开浏览器（Chrome/Edge/Firefox 都可以）
2. 访问：https://github.com/meetai-club/meetai/releases
3. 找到最新版本（最上面那个）
4. 点击 `meetai.exe` 下载
5. 等待下载完成（文件不大，约 10MB）

> ⚠️ **安全提示**：Windows 可能会弹出"未知发布者"警告，点击"更多信息"→"仍要运行"即可。MeetAI 是开源免费工具，不会收集你的数据。

### 第三步：安装 MeetAI

现在回到终端窗口，**依次输入以下命令**（每输入一行按一次回车）：

```powershell
# 1. 创建文件夹（推荐 D 盘集中管理）
New-Item -ItemType Directory -Force "D:\MeetAI\bin"

# 2. 移动下载的 meetai.exe 到这个文件夹
# 注意：如果你的下载路径不同，请修改路径
Move-Item "$env:USERPROFILE\Downloads\meetai.exe" "D:\MeetAI\bin\meetai.exe"
```

**发生了什么？**
- 第一行：在 `D:\MeetAI\bin` 创建文件夹
- 第二行：把下载的 exe 文件移动进去

> 💡 为什么推荐 D 盘？因为 MeetAI 的所有数据（Python 安装、虚拟环境等）都会放在 `D:\MeetAI\.meetai\` 下，方便集中管理和删除。

### 第四步：让电脑认识 meetai 命令

```powershell
# 把 meetai 的路径告诉电脑（使用 D:\MeetAI\bin）
[Environment]::SetEnvironmentVariable(
    "Path",
    "D:\MeetAI\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)
```

**为什么要做这一步？**
这就像告诉电脑："以后有人输入 `meetai`，请到这个文件夹找它。"

> ⚠️ 如果你没有使用 `D:\MeetAI\bin`，请把上面的路径换成你实际使用的路径。

### 第五步：重启终端

**重要！** 必须**完全关闭**终端窗口，然后**重新打开**一个新的。

为什么？因为电脑需要"记住"你刚才的修改。

### 第六步：验证成功

在新打开的终端中输入：

```powershell
meetai --help
```

**预期输出：**
你会看到很多文字，开头类似：
```
MeetAI 多语言开发环境管理工具（Python / Node.js / Java / Go）

Usage: meetai.exe [OPTIONS] <COMMAND>

Commands:
  runtime        统一运行时版本管理（Python / Node.js / Java / Go）
  python         Python 版本管理
  node           Node.js 版本管理
  ...
```

🎉 **恭喜！安装成功！** 现在你可以继续看"快速开始"章节了。

---

## 快速安装（有经验者）

### 安装（3 步搞定）

> 无需安装 Rust，直接下载 exe 即可使用。

**1. 下载 meetai.exe**

前往 [GitHub Releases](https://github.com/meetai-club/meetai/releases)，下载最新版本。

**2. 放到专属目录（推荐 D 盘集中管理）**

MeetAI 的设计理念是**运行文件和数据集中管理**，主体内容都放在一个主目录里。

我们推荐在 D 盘创建专属文件夹：

```powershell
# 创建 D:\MeetAI 目录结构
New-Item -ItemType Directory -Force "D:\MeetAI\bin"

# 移动 exe 到该目录（按实际下载路径修改）
Move-Item "$env:USERPROFILE\Downloads\meetai.exe" "D:\MeetAI\bin\meetai.exe"
```

**为什么推荐 D 盘？**
- ✅ 所有数据集中在一个地方：`D:\MeetAI\.meetai\`
- ✅ 重装系统或换电脑时，整个文件夹复制即可迁移
- ✅ 删除主体文件时，直接删除 `D:\MeetAI` 即可；如果你把相关路径写进了 PATH，再顺手删除对应的 PATH 条目
- ✅ 不占用 C 盘空间（C 盘通常空间紧张）

> 💡 你也可以选择其他盘符或路径，如 `E:\Tools\MeetAI\` 或 `D:\Development\MeetAI\`，关键是**所有文件都在同一个父文件夹下**。

**如果你坚持用默认位置（不推荐）**
```powershell
# 这会放在 C:\Users\<你的用户名>\.meetai\bin
New-Item -ItemType Directory -Force "$env:USERPROFILE\.meetai\bin"
Move-Item "$env:USERPROFILE\Downloads\meetai.exe" "$env:USERPROFILE\.meetai\bin\meetai.exe"
```

**3. 加入 PATH（用户级，无需管理员权限）**

根据你选择的安装位置，设置对应的 PATH：

```powershell
# 如果你按照推荐使用了 D:\MeetAI
[Environment]::SetEnvironmentVariable(
    "Path",
    "D:\MeetAI\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)

# 如果你用了其他位置，请替换上面的路径
# 例如：E:\Tools\MeetAI\bin 或 C:\MeetAI\bin
```

执行后**重新打开终端**即可永久生效。

> 💡 喜欢图形界面？`Win + S` → 搜索"编辑系统环境变量" → 用户变量 → `Path` → 新建 → 粘贴你选择的 bin 目录路径（如 `D:\MeetAI\bin`）

**4. 验证安装**

```powershell
meetai --help
```

看到帮助信息就说明成功了！

---

## 核心功能一览

| 功能 | Python | Node.js | Java | Go |
|------|--------|---------|------|----|
| 安装指定版本 | ✅ (Windows 下载；macOS/Linux 采纳系统版本) | ✅ (Windows/Linux x64/arm64 下载) | 🚧 | 🚧 |
| 切换版本 | ✅ | ✅ | 🚧 | 🚧 |
| 卸载版本 | ✅ | ✅ | 🚧 | 🚧 |
| 列出已安装 | ✅ | ✅ | 🚧 | 🚧 |
| pip 管理 | ✅ | - | 🚧 | 🚧 |
| venv 管理 | ✅ | - | 🚧 | 🚧 |
| 一键初始化 | ✅ | 可选 | 🚧 | 🚧 |

✅ 已支持 | 🚧 规划中

---

## 常用命令速查

### 核心命令（3 分钟上手）

```powershell
# 统一运行时管理（推荐）
meetai runtime list                       # 查看运行时支持矩阵
meetai runtime list python                # 查看已安装的 Python 版本
meetai runtime list node                  # 查看已安装的 Node.js 版本
meetai runtime install python latest      # 安装最新 Python（Windows 下载；macOS/Linux 采纳系统版本）
meetai runtime install node lts           # 安装最新 LTS Node.js（Windows/Linux x64/arm64）
meetai runtime use python 3.13.2          # 切换版本

# 一键初始化（新手友好）
meetai quick-install                      # 在当前目录初始化 Python + 准备项目虚拟环境
meetai quick-install --install-nodejs true # 同时安装 Node.js（默认 LTS）

# Python 包管理
meetai pip install requests               # 安装包
meetai pip list                           # 查看已安装

# 虚拟环境
meetai venv create myenv --target-dir .   # 创建虚拟环境
meetai venv activate myenv                # 获取激活命令
```

> 📖 **完整命令参考**：查看 [COMMANDS.md](./COMMANDS.md) 了解所有命令和参数

---

## 📚 第一个项目（5分钟实战）

假设你要开始一个 Python 项目：

### 场景 1：全新开始，一键搭建

```powershell
# 1. 创建项目文件夹
mkdir myproject
cd myproject

# 2. 在项目目录中一键安装 Python + 准备项目虚拟环境
meetai quick-install

# 3. 先确认 Python 已就绪
python --version

# 4. 如果 quick-install 生成了虚拟环境，先按摘要里的激活提示进入该环境，再安装依赖
pip install requests
```

**发生了什么？**
- 在当前项目目录执行 `quick-install` 时，会自动安装最新 Python，并为该项目准备一个名为 `default` 的虚拟环境
- 虚拟环境实体统一放在 MeetAI 的 `venvs` 目录，项目目录会写入 `.venv` 标记和激活脚本
- 如果你想把依赖安装进这个项目虚拟环境，请先执行安装摘要里的激活命令，再使用 `pip`

---

### 场景 2：已有项目需要特定 Python 版本

```powershell
# 1. 安装项目需要的 Python 版本
meetai runtime install python 3.11.0

# 2. 切换到这个版本
meetai python use 3.11.0

# 3. 验证
python --version  # 应该显示 3.11.0

# 4. 安装项目依赖
pip install -r requirements.txt
```

**小提示：**
首次执行 `meetai python use <version>` 时，MeetAI 会在需要时自动尝试把 shims 目录写入用户级 PATH。
- 如果提示“已自动将 shims 目录加入 PATH”，重开终端后再执行 `python --version`
- 如果自动配置失败，再按下文"让 `python` 命令自动跟随切换"里的手动方法处理

---

### 场景 3：同时需要 Python 和 Node.js

```powershell
# 一键安装两者（Windows/Linux x64/arm64 会自动下载 Node.js）
meetai quick-install --install-nodejs true

# 分别切换到需要的版本
meetai python use 3.13.2
meetai node use 20.11.1

# 验证
python --version
node --version
```

---

### 场景 4：创建和管理虚拟环境

```powershell
# 1. 确保已选择 Python 版本
meetai python use 3.13.2

# 2. 创建虚拟环境
meetai venv create myenv --target-dir .

# 3. 查看所有虚拟环境
meetai venv list

# 4. 激活虚拟环境（复制输出的命令执行）
meetai venv activate myenv
# 输出类似：& "D:\MeetAI\.meetai\venvs\myenv\Scripts\Activate.ps1"
# 复制整条命令，粘贴到终端执行
```

**虚拟环境是什么？**
想象你在做一个项目 A，需要 `requests 2.0`；另一个项目 B，需要 `requests 3.0`。
虚拟环境就是为每个项目创建独立的"房间"，互不干扰。

---

## 让 `python` 命令自动跟随切换（自动优先，手动兜底）

执行 `meetai python use <version>` 时，MeetAI 会先检查 shims 是否已在 PATH 中；如果缺少，会自动尝试写入用户级 PATH。

大多数情况下只要按提示重开终端即可。下面的命令只在自动配置失败，或你想提前手动配置时使用：

```powershell
# 方法1：自动检测（推荐）
# 兼容两种布局：
# - D:\MeetAI\bin\meetai.exe      -> D:\MeetAI\.meetai\shims
# - D:\Tools\meetai.exe           -> D:\Tools\.meetai\shims
$meetaiExeDir = Split-Path -Parent (Get-Command meetai).Source
$meetaiBaseDir = if ((Split-Path -Leaf $meetaiExeDir) -ieq "bin") {
    Split-Path -Parent $meetaiExeDir
} else {
    $meetaiExeDir
}
$meetaiShims = Join-Path $meetaiBaseDir ".meetai\shims"

# 当前会话立即生效
$env:Path = "$meetaiShims;$env:Path"

# 永久生效（用户级，执行后重开终端）
[Environment]::SetEnvironmentVariable(
    "Path",
    "$meetaiShims;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)

# 方法2：手动指定（如果你用了 D:\MeetAI）
[Environment]::SetEnvironmentVariable(
    "Path",
    "D:\MeetAI\.meetai\shims;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)
```

**如何验证？**
如果你刚执行的是永久生效步骤，请打开新终端后运行：
```powershell
python --version
```
应该显示你通过 `meetai python use` 切换的版本。

> ⚠️ 如果设置了 `MEETAI_HOME` 环境变量，请使用 `$env:MEETAI_HOME\shims` 替代上述路径。

---

## 让 `node` / `npm` / `npx` 命令自动跟随切换（自动优先，手动兜底）

执行 `meetai node use <version>` 时也一样，MeetAI 会先自动检查并在需要时尝试写入用户级 PATH。

如果自动配置失败，手动方式和上面完全一样，因为 Python 和 Node.js 共用同一个 shims 目录：

```powershell
# 方法1：自动检测（推荐）
$meetaiExeDir = Split-Path -Parent (Get-Command meetai).Source
$meetaiBaseDir = if ((Split-Path -Leaf $meetaiExeDir) -ieq "bin") {
    Split-Path -Parent $meetaiExeDir
} else {
    $meetaiExeDir
}
$meetaiShims = Join-Path $meetaiBaseDir ".meetai\shims"

$env:Path = "$meetaiShims;$env:Path"

[Environment]::SetEnvironmentVariable(
    "Path",
    "$meetaiShims;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)

# 方法2：手动指定（如果你用了 D:\MeetAI）
[Environment]::SetEnvironmentVariable(
    "Path",
    "D:\MeetAI\.meetai\shims;" + [Environment]::GetEnvironmentVariable("Path", "User"),
    "User"
)
```

**如何验证？**
打开新终端后运行：

```powershell
node --version
npm --version
npx --version
```

应该显示当前通过 `meetai node use` 切换后的版本链路。

> ⚠️ 如果设置了 `MEETAI_HOME` 环境变量，请使用 `$env:MEETAI_HOME\shims` 替代上述路径。

---

## 从源码构建（面向开发者）

> 需要先安装 [Rust 工具链](https://rustup.rs/)。

```powershell
# 构建
cargo build --release

# 查看帮助
cargo run -- --help

# 运行测试
cargo test --locked
```

---

## MVP 验证流程（Windows）

如果你已经构建了二进制文件，可以用这组命令验证核心功能是否正常：

```powershell
# 1. 查看支持矩阵
cargo run -- runtime list

# 2. 安装并切换 Python
cargo run -- runtime install python latest
# 替换为实际输出的版本号
cargo run -- runtime use python <version>

# 3. 验证 pip 主链路
cargo run -- pip install requests
cargo run -- pip list
cargo run -- pip uninstall requests

# 4. 验证 venv 主链路
cargo run -- venv create demo --target-dir .
cargo run -- venv list
cargo run -- venv activate demo

# 5. 验证 quick-install
cargo run -- quick-install --create-venv false

# 6. （可选）验证 Node.js（Windows/Linux x64/arm64）
cargo run -- runtime install node lts
cargo run -- runtime use node <version>
cargo run -- runtime list node
cargo run -- runtime uninstall node <version>
```

**验收标准：**
- ✅ `runtime install/use/list python` 全部成功
- ✅ `pip install/list/uninstall` 全部成功
- ✅ `venv create/list/activate` 全部成功（`activate` 输出激活命令即可）
- ✅ `quick-install` 执行成功并输出摘要
- ✅ `cargo test --locked` 通过

---

## 常见问题与排查

### 网络下载失败

**现象：** `runtime install python latest` 或 `quick-install` 报错

**解决：**
1. 改用具体版本重试：`meetai runtime install python 3.13.2`
2. 查看详细日志：`meetai --verbose runtime install python 3.13.2`
3. 检查代理或网络策略后重试
4. 如果你在 macOS/Linux 上执行 `runtime install python latest` 或 `quick-install`：
   - MeetAI 不会自动下载或编译 Python
   - 若已存在 MeetAI 已管理版本，`latest` 会回退到本地最高版本
   - 若没有已管理版本，MeetAI 会尝试采纳系统 PATH 或常见系统目录中的 Python
   - 若系统 Python 也不存在，请先用系统包管理器安装后执行 `meetai python install <version>`

---

### 切换 Python 版本后 `python --version` 未变化

**现象：** 执行 `meetai python use 3.13.2` 后，`python --version` 还是旧版本

**解决：**
1. 确认版本已切换：`meetai runtime list python`
2. 再执行一次 `meetai python use <version>`，留意终端是否提示“已自动将 shims 目录加入 PATH”
3. 如果提示已自动配置，重开终端后再执行 `python --version`
4. 如果自动配置失败或仍未生效，再按上文"让 `python` 命令自动跟随切换"中的手动方法加入 shims 目录

---

### `pip` / `venv create` 提示"还没有选择 Python 版本"

**现象：** 执行 `meetai pip list`、`meetai pip install ...` 或 `meetai venv create ...` 时报错

**解决：**
1. 查看已安装版本：`meetai runtime list python`
2. 切换版本：`meetai python use <version>`
3. 重试命令

> 💡 `meetai venv list` 不依赖当前 Python 版本，它只是扫描虚拟环境目录；如果列表不符合预期，优先检查目标环境是否已经创建，以及当前使用的 `.meetai`/`MEETAI_HOME` 是否与创建时一致。

---

## 配置说明

### 目录结构（集中管理示例）

如果你按照推荐把 `meetai.exe` 放在 `D:\MeetAI\bin\`，那么所有文件都会集中在 `D:\MeetAI\.meetai\`：

```
D:\MeetAI\
├── bin\
│   └── meetai.exe          # 可执行文件
└── .meetai\                 # 所有数据（可整体删除）
    ├── config.json          # 配置文件
    ├── shims\               # 命令代理（让 python/pip/node/npm/npx 自动切换）
    ├── python\              # Python 安装目录
    │   ├── python-3.13.2\
    │   └── python-3.14.3\
    ├── nodejs\              # Node.js 安装目录
    │   └── versions\
    │       └── 20.11.1\
    ├── venvs\               # 虚拟环境
    │   └── myproject\
    └── cache\               # 下载缓存
```

**删除时：** 直接删除 `D:\MeetAI` 文件夹即可移除主体文件；如果之前把 `D:\MeetAI\bin` 或 `D:\MeetAI\.meetai\shims` 写进了用户 PATH，再顺手删除对应 PATH 条目即可。

### 配置目录优先级

MeetAI 会自动决定数据放在哪里，优先级如下：

1. `MEETAI_HOME` 环境变量（如果已设置）
2. 可执行文件附近的 `.meetai`
   - 如果 `meetai.exe` 在 `bin` 子目录，使用父目录下的 `.meetai`
   - 否则使用 `meetai.exe` 同级的 `.meetai`
3. 用户主目录：`~/.meetai`（前两者都不可用时回退）

**最佳实践：**
- 把 `meetai.exe` 放在你希望数据存放的父目录下的 `bin` 子目录
- 例如：`D:\MeetAI\bin\meetai.exe` → 数据自动放在 `D:\MeetAI\.meetai\`
- 这样所有文件都在 `D:\MeetAI` 下，便于管理

---

## 升级

1. 从 [GitHub Releases](https://github.com/meetai-club/meetai/releases) 下载新版 `meetai.exe`
2. 覆盖旧文件即可（PATH 无需重新配置）

---

## 参与开发

欢迎贡献！如果你有兴趣参与 MeetAI 的开发：

- 📖 阅读 [AGENTS.md](./AGENTS.md) 了解项目架构和开发规范
- 🛠️ 查看 `src/` 目录了解代码结构
- 🐛 提交 Issue 报告问题或提出建议
- 💡 提交 Pull Request 贡献代码

---

## 📖 术语解释

为了让全新手更好地理解，这里解释一些常见术语：

| 术语 | 是什么意思？ | 为什么需要？ |
|------|------------|------------|
| **Python** | 一种流行的编程语言，适合数据分析、Web 开发、自动化等 | 如果你想写 Python 代码，就需要它 |
| **Node.js** | 让 JavaScript 能在电脑上运行的工具 | 如果你想做 Web 开发、写工具脚本，就需要它 |
| **版本** | 软件的版本号，如 Python 3.13.2 | 不同项目可能需要不同版本，比如项目 A 需要 3.11，项目 B 需要 3.13 |
| **PATH** | 电脑记住命令位置的列表 | 让你能在任何文件夹输入 `meetai`、`python` 等命令，而不需要输入完整路径 |
| **虚拟环境 (venv)** | 独立的 Python 环境，只对当前项目生效 | 避免不同项目的依赖冲突，比如项目 A 需要 requests 2.0，项目 B 需要 requests 3.0 |
| **pip** | Python 的包管理器，用来安装第三方库 | 安装各种功能模块，如 `pip install requests` 就能在代码中使用网络请求功能 |
| **runtime** | 运行环境，这里指 Python 或 Node.js 本身 | MeetAI 管理的核心对象，就是这些编程语言的安装版本 |
| **shims** | 中间层，用来"代理"命令 | 当你用 `meetai python use 3.13.2` 或 `meetai node use 20.11.1` 切换版本后，shims 会让对应命令自动指向新版本 |
| **GitHub Releases** | 软件的发布页面，可以下载最新版本 | 这是获取 MeetAI 可执行文件的地方 |

---

## 更多帮助

- 📖 详细开发文档：[AGENTS.md](./AGENTS.md)
- 🐛 报告问题：[GitHub Issues](https://github.com/meetai-club/meetai/issues)
- 💬 交流讨论：欢迎加入我们的社区（待建立）

---

## 许可证

MIT License. 详见 [LICENSE](./LICENSE) 文件.
