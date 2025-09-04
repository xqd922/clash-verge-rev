@echo off
echo Verifying Clash Verge Rev Tauri Configuration...
echo.

echo [1/3] Checking sidecar files...
if not exist "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" (
    echo ❌ Missing: verge-mihomo-x86_64-pc-windows-msvc.exe
    set MISSING_FILES=1
) else (
    echo ✓ Found: verge-mihomo-x86_64-pc-windows-msvc.exe
)

if not exist "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" (
    echo ❌ Missing: verge-mihomo-alpha-x86_64-pc-windows-msvc.exe
    set MISSING_FILES=1
) else (
    echo ✓ Found: verge-mihomo-alpha-x86_64-pc-windows-msvc.exe
)

echo.
echo [2/3] Checking Tauri configuration...
if exist "src-tauri\tauri.conf.json" (
    echo ✓ Found: tauri.conf.json
) else (
    echo ❌ Missing: tauri.conf.json
    set CONFIG_ERROR=1
)

if exist "src-tauri\tauri.windows.conf.json" (
    echo ✓ Found: tauri.windows.conf.json
) else (
    echo ❌ Missing: tauri.windows.conf.json
    set CONFIG_ERROR=1
)

echo.
echo [3/3] Testing Tauri CLI...
tauri --version >nul 2>&1
if %errorlevel% equ 0 (
    echo ✓ Tauri CLI is working
    tauri info 2>nul | findstr "Platform" >nul
    if %errorlevel% equ 0 (
        echo ✓ Tauri configuration is valid
    ) else (
        echo ⚠ Tauri configuration may have issues
    )
) else (
    echo ❌ Tauri CLI not found or not working
    set TOOL_ERROR=1
)

echo.
echo === Summary ===
if defined MISSING_FILES (
    echo ❌ Some sidecar files are missing. Run: pnpm check x86_64-pc-windows-msvc
)
if defined CONFIG_ERROR (
    echo ❌ Tauri configuration files missing
)
if defined TOOL_ERROR (
    echo ❌ Tauri CLI issues detected
)

if not defined MISSING_FILES if not defined CONFIG_ERROR if not defined TOOL_ERROR (
    echo ✓ All checks passed! You can proceed with building.
) else (
    echo.
    echo Please fix the issues above before building.
    echo For detailed troubleshooting, see: TROUBLESHOOTING.md
)

echo.
pause