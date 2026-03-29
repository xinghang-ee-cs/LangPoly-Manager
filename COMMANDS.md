# MeetAI 命令参考手册

> 💡 **快速查找**：按 `Ctrl+F` 搜索你需要的命令

---

## 📋 命令总览

| 命令组 | 用途 | 常用度 |
|--------|------|--------|
| [runtime](#runtime-统一运行时管理) | 统一管理所有运行时（推荐） | ⭐⭐⭐ |
| [python](#python-python-管理) | Python 专项管理 | ⭐⭐⭐ |
| [node](#node-nodejs-管理) | Node.js 专项管理 | ⭐⭐⭐ |
| [pip](#pip-包管理) | Python 包管理 | ⭐⭐⭐ |
| [venv](#venv-虚拟环境管理) | 虚拟环境管理 | ⭐⭐ |
| [quick-install](#quick-install-一键初始化) | 一键初始化环境 | ⭐⭐ |

---

## 全局选项

```powershell
meetai --help              # 查看帮助
meetai --version           # 查看版本
meetai -v <command>        # 启用详细输出（调试用）
```

---

## `runtime` 统一运行时管理

> 推荐使用这套命令，统一管理所有运行时

### 查看

```powershell
meetai runtime list                    # 列出所有支持的运行时
meetai runtime list python             # 列出已安装的 Python 版本
meetai runtime list nodejs             # 列出已安装的 Node.js 版本
```

### 安装

```powershell
# Python
meetai runtime install python latest   # 最新稳定版
meetai runtime install python 3.13.2   # 指定版本

# Node.js（Windows 支持自动安装）
meetai runtime install nodejs lts      # 最新 LTS（推荐）
meetai runtime install nodejs latest   # 最新版本
meetai runtime install nodejs 20.11.1  # 指定版本
```

### 切换

```powershell
meetai runtime use python 3.13.2
meetai runtime use nodejs 20.11.1
```

### 卸载

```powershell
meetai runtime uninstall python 3.13.2
meetai runtime uninstall nodejs 20.11.1
```

---

## `python` Python 管理

### 查看

```powershell
meetai python list                     # 列出已安装版本
```

### 安装

```powershell
meetai python install latest           # 最新稳定版
meetai python install 3.13.2           # 指定版本
```

**支持的版本格式：**
- `latest` - 最新稳定版
- `3.13.2` - 精确版本号
- 当前 Python 版本号需要使用精确的 `X.Y.Z` 格式

### 切换

```powershell
meetai python use 3.13.2
```

### 卸载

```powershell
meetai python uninstall 3.13.2
```

---

## `node` Node.js 管理

### 查看

```powershell
meetai node list                       # 列出已安装版本
meetai node available                  # 查看官方可安装版本（含 LTS）
```

### 安装

```powershell
meetai node install lts                # 最新 LTS（推荐）
meetai node install latest             # 最新版本
meetai node install 20.11.1            # 指定版本
meetai node install project            # 从 .nvmrc 读取
```

### 切换

```powershell
meetai node use 20.11.1
meetai node use project                # 从 .nvmrc 读取
```

### 项目模式（.nvmrc）

在项目根目录创建 `.nvmrc`：
```
20.11.1
```

然后：
```powershell
meetai node install project
meetai node use project
```

### 卸载

```powershell
meetai node uninstall 20.11.1
```

---

## `pip` 包管理

> ⚠️ 需要先用 `meetai python use <version>` 选择 Python 版本

### 安装包

```powershell
meetai pip install requests
meetai pip install requests flask      # 多个包
```

### 卸载包

```powershell
meetai pip uninstall requests
```

### 升级包

```powershell
meetai pip upgrade requests
```

### 查看

```powershell
meetai pip list                        # 列出所有已安装包
```

---

## `venv` 虚拟环境管理

> ⚠️ 需要先用 `meetai python use <version>` 选择 Python 版本

### 创建

```powershell
meetai venv create myenv --target-dir .           # 当前目录
meetai venv create myenv --target-dir D:\projects # 指定目录
```

### 激活

```powershell
meetai venv activate myenv
# 复制输出的命令执行
```

### 查看

```powershell
meetai venv list
```

---

## `quick-install` 一键初始化

### 基础用法

```powershell
meetai quick-install                   # 默认配置
```

**默认行为：**
- 安装最新 Python
- 创建 "default" 虚拟环境
- 启用自动激活提示

### 常用选项

```powershell
# 指定版本
meetai quick-install --python-version 3.13.2

# 不创建虚拟环境
meetai quick-install --create-venv false

# 同时安装 Node.js（Windows）
meetai quick-install --install-nodejs true

# 指定 Node.js 版本
meetai quick-install --install-nodejs true --nodejs-version 20.11.1

# 自定义虚拟环境名称
meetai quick-install --venv-name myproject

# 关闭自动激活提示
meetai quick-install --auto-activate false
```

### 完整参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--python-version` | `latest` | Python 版本 |
| `--pip-version` | `latest` | Pip 版本 |
| `--venv-name` | `default` | 虚拟环境名称 |
| `--create-venv` | `true` | 是否创建虚拟环境 |
| `--auto-activate` | `true` | 是否启用自动激活提示 |
| `--target-dir` | `.` | 安装目标目录 |
| `--install-nodejs` | `false` | 是否安装 Node.js（Windows） |
| `--nodejs-version` | `lts` | Node.js 版本 |

---

## 常见场景示例

### 场景 1：全新学习 Python

```powershell
meetai quick-install
python --version
```

### 场景 2：项目需要特定版本

```powershell
meetai runtime install python 3.11.0
meetai python use 3.11.0
pip install -r requirements.txt
```

### 场景 3：同时需要 Python 和 Node.js

```powershell
meetai quick-install --install-nodejs true
```

### 场景 4：创建项目虚拟环境

```powershell
meetai python use 3.13.2
meetai venv create myproject --target-dir .
meetai venv activate myproject
```

### 场景 5：多版本切换

```powershell
meetai runtime install python 3.11.0
meetai runtime install python 3.13.2
meetai runtime list python
meetai python use 3.11.0  # 切换
```

---

## 💡 使用技巧

1. **让命令自动跟随版本**
   - 将 shims 目录加入 PATH（详见 [README](./README.md)）
   - `python`、`pip`、`node`、`npm` 会自动使用当前版本

2. **查看详细日志**
   - 加 `-v` 选项：`meetai -v runtime install python 3.13.2`

3. **平台差异**
   - Windows：支持自动下载安装
   - macOS/Linux：需手动安装后用 `use` 切换

---

📖 **更多帮助**：[README](./README.md) | [开发文档](./AGENTS.md) | [问题反馈](https://github.com/meetai-club/meetai/issues)
