# Clash Verge Rev - Windows 构建指南

## 环境要求

1. **Node.js**: 版本 18+ (推荐使用 v22)
2. **Rust**: 稳定版工具链
3. **pnpm**: 包管理器
4. **Windows 11**: x64 系统

## 快速开始

### 1. 安装依赖

首先确保您已安装所需工具：

```bash
# 安装 Node.js (如果尚未安装)
# 下载地址: https://nodejs.org/

# 安装 Rust (如果尚未安装)
# 下载地址: https://rustup.rs/

# 安装 pnpm
npm install -g pnpm

# 安装 Tauri CLI
cargo install tauri-cli
```

### 2. 安装项目依赖

```bash
pnpm install
```

### 3. 构建应用

#### 方法一：使用批处理脚本（推荐）

```bash
# 直接双击运行
build-local-windows.bat
```

#### 方法二：配置验证（遇到问题时）

```bash
# 验证配置和文件完整性
verify-config.bat
```

#### 方法三：手动命令

```bash
# 检查项目
pnpm check x86_64-pc-windows-msvc

# 构建应用
pnpm build --target x86_64-pc-windows-msvc

# 生成便携版
pnpm portable x86_64-pc-windows-msvc
```

### 4. 构建产物

构建完成后，您可以在以下位置找到生成的文件：

- **安装程序**: `src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\*.exe`
- **便携版**: `src-tauri\target\x86_64-pc-windows-msvc\release\portable\*`

## GitHub Actions 自动构建

如果您将代码推送到 GitHub，当您推送到 `main` 分支或手动触发工作流时，会自动构建 Windows x64 版本。

### 手动触发构建

1. 进入您的 GitHub 仓库
2. 点击 "Actions" 标签
3. 选择 "Build Windows x64" 工作流
4. 点击 "Run workflow" 按钮

## 故障排除

### 常见问题

1. **构建失败**: 确保所有依赖都已正确安装
2. **内存不足**: 脚本已配置 4GB 内存限制，如仍不足可调整
3. **权限问题**: 某些操作可能需要管理员权限
4. **Sidecar 文件未找到**: 如果遇到 `resource path 'sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe' doesn't exist` 错误，请先运行检查脚本

### 重要提示

在首次构建之前，请务必运行检查脚本下载必要的二进制文件：

```bash
# 下载所需的 sidecar 文件和资源
pnpm check x86_64-pc-windows-msvc
```

这个命令会自动下载：

- Clash Meta 核心文件
- Clash Meta Alpha 核心文件
- 地理位置数据库文件
- Windows 系统代理工具
- 其他必要的资源文件

### 调试命令

```bash
# 检查 Rust 安装
rustc --version

# 检查 Node.js 版本
node --version

# 检查 pnpm 版本
pnpm --version

# 检查 Tauri CLI
tauri --version

# 验证 sidecar 文件
dir src-tauri\sidecar\
```

### 详细故障排除

如果遇到复杂问题，请查看详细的故障排除指南：[TROUBLESHOOTING.md](./TROUBLESHOOTING.md)

该文档包含：

- Sidecar 文件问题的完整解决方案
- 网络和代理配置
- 环境配置问题
- 分步调试方法

## 注意事项

- 首次构建可能需要较长时间，因为需要下载和编译依赖
- 构建过程中请确保网络连接稳定
- 如果遇到 Rust 编译错误，尝试更新工具链：`rustup update`
