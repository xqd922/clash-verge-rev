@echo off
echo Building Clash Verge Rev for Windows x64...
echo.

echo [1/6] Installing dependencies...
pnpm install
if %errorlevel% neq 0 (
    echo Error: Failed to install dependencies
    pause
    exit /b 1
)

echo.
echo [2/6] Downloading required binary files and resources...
echo This may take some time on first run...
pnpm check x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Failed to download required files
    pause
    exit /b 1
)

echo.
echo [3/6] Verifying sidecar files and configuration...
if not exist "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" (
    echo Error: verge-mihomo-x86_64-pc-windows-msvc.exe not found
    echo Please run: pnpm check x86_64-pc-windows-msvc
    pause
    exit /b 1
)
if not exist "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" (
    echo Error: verge-mihomo-alpha-x86_64-pc-windows-msvc.exe not found
    echo Please run: pnpm check x86_64-pc-windows-msvc
    pause
    exit /b 1
)
echo Sidecar files verified successfully!
echo Checking Tauri configuration...
tauri info >nul 2>&1
if %errorlevel% neq 0 (
    echo Warning: Tauri configuration check failed, continuing anyway...
) else (
    echo Tauri configuration verified!
)

echo.
echo [4/6] Building application...
set NODE_OPTIONS=--max_old_space_size=4096
pnpm build --target x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Build failed
    pause
    exit /b 1
)

echo.
echo [5/6] Creating portable version...
pnpm portable x86_64-pc-windows-msvc
if %errorlevel% neq 0 (
    echo Error: Portable version creation failed
    pause
    exit /b 1
)

echo.
echo [6/6] Build completed successfully!
echo.
echo Build artifacts are located at:
echo - Installer: src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\
echo - Portable:  src-tauri\target\x86_64-pc-windows-msvc\release\portable\
echo.
pause