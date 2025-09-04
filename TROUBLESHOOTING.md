# Clash Verge Rev 构建故障排除指南

## 1. Sidecar 文件问题

### 错误信息

```
resource path `sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe` doesn't exist
```

或者

```
resource path `sidecar\verge-mihomo-$TARGET-x86_64-pc-windows-msvc.exe` doesn't exist
```

### 解决方案

这是最常见的问题，因为项目需要下载 Clash Meta 核心文件。

**方法一：使用环境检查脚本（推荐）**

```bash
# 双击运行或在命令行执行
setup-environment.bat
```

**方法二：手动下载**

```bash
# 强制重新下载所有必要文件
pnpm check x86_64-pc-windows-msvc --force
```

**方法三：检查 Tauri 配置**
如果看到 `$TARGET` 字段在错误信息中，请检查：

1. Windows 特定配置文件 `src-tauri/tauri.windows.conf.json` 中的 `externalBin` 配置
2. 确保配置指向正确的文件名（带完整架构后缀）
3. 确保主配置文件 `tauri.conf.json` 中没有冲突的 `externalBin` 配置
   检查以下文件是否存在：

- `src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe`
- `src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe`

如果文件不存在，重新运行检查命令。

## 2. 网络问题

### 症状

- 下载超时
- 连接失败
- 下载速度极慢

### 解决方案

**设置代理（如果有）**

```bash
# 设置 HTTP 代理
set HTTP_PROXY=http://127.0.0.1:8080
set HTTPS_PROXY=http://127.0.0.1:8080

# 或者设置为您的代理地址
```

**使用镜像源**
如果 GitHub 访问困难，可以考虑：

1. 使用科学上网工具
2. 寻找镜像源（但需要注意安全性）

## 3. 构建环境问题

### Node.js 版本问题

```bash
# 检查版本（需要 18+）
node --version

# 如果版本过低，请升级 Node.js
```

### Rust 工具链问题

```bash
# 检查 Rust 版本
rustc --version

# 更新 Rust
rustup update

# 添加 Windows 目标
rustup target add x86_64-pc-windows-msvc
```

### pnpm 问题

```bash
# 检查 pnpm 版本
pnpm --version

# 重新安装 pnpm
npm install -g pnpm

# 清理缓存
pnpm store prune
```

## 4. 内存问题

### 症状

- 构建过程中内存不足
- JavaScript heap out of memory

### 解决方案

```bash
# 方法一：使用预设的环境变量（脚本已包含）
set NODE_OPTIONS=--max_old_space_size=4096

# 方法二：增加内存限制
set NODE_OPTIONS=--max_old_space_size=8192
```

## 5. 权限问题

### 症状

- 文件无法写入
- 权限被拒绝

### 解决方案

1. 以管理员身份运行命令提示符
2. 确保项目目录有写入权限
3. 临时关闭防病毒软件（如果它阻止文件操作）

## 6. Tauri 相关问题

### WebView2 问题

```bash
# Windows 11 通常已预装，如果遇到问题：
# 1. 手动下载安装 WebView2 Runtime
# 2. 或使用嵌入式版本（项目已配置）
```

### 证书问题

对于无签名构建，确保 Tauri 配置正确：

- 不要设置签名相关的环境变量
- 使用无签名构建模式

## 7. 常用调试命令

```bash
# 清理构建缓存
cargo clean

# 清理 node_modules
rm -rf node_modules
pnpm install

# 重新下载所有资源
pnpm check x86_64-pc-windows-msvc --force

# 查看详细构建日志
pnpm build --target x86_64-pc-windows-msvc --verbose

# 检查 Tauri 配置
tauri info
```

## 8. 分步调试

如果自动构建脚本失败，可以手动执行以下步骤：

```bash
# 1. 安装依赖
pnpm install

# 2. 下载二进制文件
pnpm check x86_64-pc-windows-msvc

# 3. 验证文件存在
dir src-tauri\sidecar\

# 4. 构建前端
pnpm run web:build

# 5. 构建应用
pnpm build --target x86_64-pc-windows-msvc

# 6. 生成便携版
pnpm portable x86_64-pc-windows-msvc
```

## 9. 获取帮助

如果以上方法都无法解决问题：

1. 检查项目的 GitHub Issues
2. 查看原始项目文档
3. 确保您使用的是最新版本的代码
4. 记录完整的错误信息和环境信息

## 10. 环境信息收集

遇到问题时，请收集以下信息：

```bash
# 系统信息
systeminfo | findstr /C:"OS Name" /C:"OS Version"

# 工具版本
node --version
npm --version
pnpm --version
rustc --version
cargo --version
tauri --version

# 目标架构
rustc -vV | findstr host

# 项目状态
dir src-tauri\sidecar\
dir src-tauri\target\
```

这些信息有助于诊断和解决问题。
