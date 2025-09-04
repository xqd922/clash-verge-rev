@echo off
echo Clash Verge Rev - 优化构建脚本
echo.

cd /d "%~dp0"

echo [1/7] 环境变量设置...
set NODE_OPTIONS=--max_old_space_size=4096
set CARGO_INCREMENTAL=0
set RUST_BACKTRACE=short

echo [2/7] 检查依赖...
pnpm install
if %errorlevel% neq 0 (
    echo Error: 依赖安装失败
    pause
    exit /b 1
)

echo [3/7] 下载二进制文件...
pnpm check x86_64-pc-windows-msvc --force
if %errorlevel% neq 0 (
    echo Error: 二进制文件下载失败
    pause
    exit /b 1
)

echo [4/7] 验证 sidecar 文件...
if not exist "src-tauri\sidecar\verge-mihomo.exe" (
    echo 创建 Tauri 兼容的 sidecar 文件...
    copy "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo.exe" >nul 2>&1
)
if not exist "src-tauri\sidecar\verge-mihomo-alpha.exe" (
    copy "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo-alpha.exe" >nul 2>&1
)

echo [5/7] 清理构建缓存...
cd src-tauri
cargo clean >nul 2>&1
cd ..

echo [6/7] 开始构建...
echo 注意：构建可能需要 10-30 分钟，请耐心等待...
echo 如果看到 "Building [====>" 进度条长时间不动，可以按 Ctrl+C 重试
echo.

timeout /t 3 >nul
pnpm build --target x86_64-pc-windows-msvc

if %errorlevel% equ 0 (
    echo.
    echo [7/7] 构建成功！创建便携版...
    pnpm portable x86_64-pc-windows-msvc
    echo.
    echo === 构建完成 ===
    echo 安装程序位置: src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
    echo 便携版位置: src-tauri\target\x86_64-pc-windows-msvc\release\portable\
) else (
    echo.
    echo 构建失败，错误代码: %errorlevel%
    echo 您可以：
    echo 1. 重新运行此脚本
    echo 2. 检查 TROUBLESHOOTING.md 获取更多帮助
    echo 3. 尝试清理后重新构建：cargo clean
)

echo.
pause