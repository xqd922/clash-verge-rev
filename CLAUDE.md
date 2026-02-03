# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Clash Verge Rev is a cross-platform GUI for managing Clash Meta (mihomo) proxy configurations. Built with Tauri 2 (Rust backend) and React 19 (TypeScript frontend), supporting Windows, Linux, and macOS.

## Development Commands

### Initial Setup

```bash
corepack enable              # Enable pnpm
pnpm install                 # Install Node.js dependencies
pnpm run prebuild            # Download mihomo core binaries (required before first run)
pnpm run prebuild --force    # Force re-download core binaries
```

### Development

```bash
pnpm dev                     # Start dev server with Tauri
pnpm dev:diff                # Dev server when app instance exists
pnpm web:dev                 # Frontend-only dev server
```

### Build

```bash
pnpm build                   # Full production build
pnpm build:fast              # Quick build for testing (--profile fast-release)
pnpm portable                # Windows portable build
```

### Code Quality

```bash
# Frontend
pnpm lint                    # ESLint check
pnpm lint:fix                # ESLint fix
pnpm typecheck               # TypeScript check
pnpm format                  # Prettier format

# Rust
cargo fmt                    # Format Rust code
cargo clippy --all-targets --all-features -- -D warnings   # Lint Rust code

# i18n
pnpm i18n:check              # Check unused translations
pnpm i18n:format             # Align translation files
```

### Git Hooks (via cargo-make)

```bash
cargo make pre-commit        # Format checks
cargo make pre-push          # Lint + typecheck
```

## Architecture

### Directory Structure

```
src/                         # React frontend
├── pages/                   # Route pages (home, profiles, proxies, connections, rules, logs, settings)
├── components/              # UI components (base, home, layout, profile, proxy, setting, etc.)
├── services/                # API integration (api.ts for mihomo HTTP API, cmds.ts for Tauri IPC)
├── hooks/                   # Custom React hooks
├── providers/               # Context providers
└── locales/                 # i18n translation files

src-tauri/                   # Rust backend (Tauri application)
├── src/
│   ├── cmd/                 # IPC command handlers exposed to frontend
│   ├── config/              # Configuration management (clash.rs, profiles.rs, verge.rs, runtime.rs)
│   ├── core/                # Core services (service.rs, hotkey.rs, timer.rs, tray/, sysopt.rs)
│   ├── feat/                # Feature implementations (profile.rs, backup.rs, clash.rs, window.rs)
│   ├── enhance/             # Config enhancements (merge.rs, script.rs via Boa JS engine, tun.rs)
│   ├── module/              # Modules (auto_backup, lightweight mode)
│   └── utils/               # Utilities (dirs.rs, resolve/, server.rs)
└── tauri.conf.json          # Tauri config

crates/                      # Rust workspace members
├── clash-verge-draft/       # Config draft management
├── clash-verge-logging/     # Logging infrastructure
├── clash-verge-signal/      # Signal handling
├── clash-verge-i18n/        # i18n support
├── clash-verge-limiter/     # Rate limiting
└── tauri-plugin-clash-verge-sysinfo/  # System info plugin
```

### Key Data Flow

1. React frontend calls Tauri commands via `src/services/cmds.ts`
2. Commands are handled in `src-tauri/src/cmd/*.rs`
3. Core business logic in `src-tauri/src/core/` and `src-tauri/src/feat/`
4. Config processing in `src-tauri/src/config/` and `src-tauri/src/enhance/`
5. Mihomo core communication via HTTP API (wrapper in `src/services/api.ts`)

### Platform-Specific Code

- Windows: `deelevate` for privilege elevation, WinAPI integration, UWP handling
- macOS: Autostart launcher, network interface prioritization
- Linux: DBus integration, webkit dependencies

### Configuration Enhancement System

User configs can be enhanced via:

- **Merge profiles**: YAML-based config merging (`enhance/merge.rs`)
- **Script profiles**: JavaScript execution via Boa engine (`enhance/script.rs`)
- **Chain processing**: Multiple enhancements applied in sequence (`enhance/chain.rs`)

## Important Conventions

### Rust

- Workspace uses strict Clippy lints (see `Cargo.toml` [workspace.lints.clippy])
- Avoid `unwrap()` and `expect()` - they trigger warnings
- Cognitive complexity threshold: 25
- Edition 2024, Rust 1.91+

### Frontend

- React 19 with React Compiler integration
- Material UI 7 for components
- SWR for data fetching/caching
- Monaco Editor for config editing
- Strict ESLint with unused import removal

### Package Manager

- pnpm 10.28.0 (enforced via corepack)
