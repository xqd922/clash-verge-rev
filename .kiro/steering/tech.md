# Technology Stack

## Architecture

- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Rust + Tauri 2.8
- **UI Framework**: Material-UI (MUI) v7
- **State Management**: Zustand + SWR for data fetching
- **Styling**: SCSS + CSS-in-JS (Emotion)

## Key Libraries & Tools

### Frontend Dependencies

- **React Ecosystem**: React 19, React Router v7, React Hook Form
- **UI Components**: MUI Material, MUI Icons, MUI Data Grid, MUI Lab
- **Code Editor**: Monaco Editor with YAML support
- **Utilities**: Lodash-ES, Day.js, Axios, i18next
- **Development**: Vite, TypeScript, ESLint, Prettier

### Backend Dependencies

- **Tauri Plugins**: Shell, Dialog, FS, Process, Clipboard, Updater, Notification
- **HTTP/Network**: Reqwest, Warp server, Network Interface detection
- **System Integration**: Sysproxy, Sysinfo, System locale detection
- **Configuration**: Serde (JSON/YAML), Regex, Parking Lot (async)
- **Security**: AES-GCM encryption, HMAC, SHA2

## Build System & Package Management

### Package Manager

- **Primary**: pnpm (v9.13.2)
- **Lock File**: pnpm-lock.yaml

### Development Commands

```bash
# Install dependencies
pnpm i

# Development server (with Rust backend)
pnpm dev

# Web-only development
pnpm web:dev

# Production build
pnpm build

# Fast development build (optimized for speed)
pnpm build:fast

# Prebuild setup (required before first run)
pnpm prebuild
```

### Code Quality Commands

```bash
# Frontend linting
pnpm lint

# Code formatting
pnpm format
pnpm format:check

# Rust formatting
pnpm fmt

# Rust linting
pnpm clippy
```

### Release Commands

```bash
# Create updater packages
pnpm updater
pnpm updater-fixed-webview2

# Create portable versions
pnpm portable
pnpm portable-fixed-webview2

# Version management
pnpm release-version
pnpm publish-version
```

## Build Configuration

### Vite Configuration

- **Target**: ES2020 with legacy browser support
- **Bundling**: Terser minification, tree-shaking enabled
- **Code Splitting**: Optimized chunks for React, MUI, Monaco Editor, Tauri plugins
- **Aliases**: `@/*` → `src/*`, `@root/*` → `./*`

### Rust Configuration

- **Edition**: 2021
- **Profiles**: Development (fast compilation), Release (optimized), Fast-Release (hybrid)
- **Features**: Custom protocol, Verge-dev mode, Tokio tracing
- **Linting**: Strict Clippy rules for safety, performance, and async code

### TypeScript Configuration

- **Target**: ESNext with strict mode
- **Module Resolution**: Node with path mapping
- **JSX**: React JSX transform
- **Includes**: Only `src/` directory

## Platform Support

- **Windows**: x64, x86, ARM64 (WebView2 required)
- **Linux**: x64, ARM64 (system WebKit)
- **macOS**: 10.15+ Intel and Apple Silicon
