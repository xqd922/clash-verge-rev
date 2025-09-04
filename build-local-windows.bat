@echo off
echo Building Clash Verge Rev for Windows x64...
echo.

echo [1/5] Installing dependencies...
pnpm install
if %errorlevel% neq 0 (
    echo Error: Failed to install dependencies
    pause
    exit /b 1
)

echo.
echo [2/5] Checking project...
pnpm check x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Project check failed
    pause
    exit /b 1
)

echo.
echo [3/5] Building application...
set NODE_OPTIONS=--max_old_space_size=4096
pnpm build --target x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Build failed
    pause
    exit /b 1
)

echo.
echo [4/5] Creating portable version...
pnpm portable x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Portable version creation failed
    pause
    exit /b 1
)

echo.
echo [5/5] Build completed successfully!
echo.
echo Build artifacts are located at:
echo - Installer: src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
echo - Portable:  src-tauri\target\x86_64-pc-windows-msvc\release\portable\
echo.
pause