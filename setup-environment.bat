@echo off
echo Checking and setting up Clash Verge Rev build environment...
echo.

echo [1/4] Checking required tools...

:: Check Node.js
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Node.js not found. Please install Node.js 18+ from https://nodejs.org/
    pause
    exit /b 1
)
echo ✓ Node.js found: 
node --version

:: Check pnpm
pnpm --version >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: pnpm not found. Installing pnpm...
    npm install -g pnpm
    if %errorlevel% neq 0 (
        echo Error: Failed to install pnpm
        pause
        exit /b 1
    )
)
echo ✓ pnpm found: 
pnpm --version

:: Check Rust
rustc --version >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: Rust not found. Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)
echo ✓ Rust found: 
rustc --version

:: Check Tauri CLI
tauri --version >nul 2>&1
if %errorlevel% neq 0 (
    echo Warning: Tauri CLI not found. Installing...
    cargo install tauri-cli
    if %errorlevel% neq 0 (
        echo Error: Failed to install Tauri CLI
        pause
        exit /b 1
    )
)
echo ✓ Tauri CLI found: 
tauri --version

echo.
echo [2/4] Installing dependencies...
pnpm install
if %errorlevel% neq 0 (
    echo Error: Failed to install dependencies
    pause
    exit /b 1
)

echo.
echo [3/4] Downloading required binary files...
echo This may take several minutes on first run...
pnpm check x86_64-pc-windows-msvc --force
if %errorlevel% neq 0 (
    echo Error: Failed to download required files
    pause
    exit /b 1
)

echo.
echo [4/4] Verifying setup...
if not exist "src-tauri\sidecar\verge-mihomo-x86_64-pc-windows-msvc.exe" (
    echo Error: Core binary file missing
    pause
    exit /b 1
)
if not exist "src-tauri\sidecar\verge-mihomo-alpha-x86_64-pc-windows-msvc.exe" (
    echo Error: Alpha binary file missing  
    pause
    exit /b 1
)

echo.
echo ✓ Environment setup completed successfully!
echo.
echo You can now run the build script: build-local-windows.bat
echo Or manually build with: pnpm build --target x86_64-pc-windows-msvc
echo.
pause