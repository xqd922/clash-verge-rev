@echo off
echo Fixing sidecar file names for Tauri build...
echo.

cd /d "%~dp0"

echo Creating sidecar files with correct names...

if exist "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" (
    copy "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe.bak" >nul 2>&1
    copy "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo.exe" >nul 2>&1
    if %errorlevel% equ 0 (
        echo ✓ Created verge-mihomo.exe
    ) else (
        echo ❌ Failed to create verge-mihomo.exe
    )
) else (
    echo ❌ Source file verge-mihomo-x86_64-pc-windows-msvc.exe not found
)

if exist "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" (
    copy "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe.bak" >nul 2>&1
    copy "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" "src-tauri\sidecar\verge-mihomo-alpha.exe" >nul 2>&1
    if %errorlevel% equ 0 (
        echo ✓ Created verge-mihomo-alpha.exe
    ) else (
        echo ❌ Failed to create verge-mihomo-alpha.exe
    )
) else (
    echo ❌ Source file verge-mihomo-alpha-x86_64-pc-windows-msvc.exe not found
)

echo.
echo Listing sidecar directory:
dir "src-tauri\sidecar\" /b

echo.
echo Sidecar files setup completed!
pause