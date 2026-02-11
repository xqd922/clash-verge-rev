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
cargo clippy-all             # Alias for above

# i18n
pnpm i18n:check              # Check unused translations
pnpm i18n:format             # Align translation files
```

### Git Hooks (via cargo-make)

Pre-commit runs format checks only (fast). Pre-push runs full lint + typecheck (comprehensive).

```bash
cargo make pre-commit        # Rust format + lint-staged (Prettier + ESLint on staged files)
cargo make pre-push          # Rust clippy + ESLint + TypeScript typecheck
```

## Architecture

### Directory Structure

```
src/                         # React frontend
├── pages/                   # Route pages (proxies, profiles, connections, rules, logs, settings, unlock)
├── components/              # UI components organized by domain (base, home, layout, profile, proxy, setting)
├── services/                # API layer: cmds.ts (Tauri IPC), api.ts (mihomo HTTP API)
├── hooks/                   # Custom React hooks (SWR data fetching, WebSocket subscriptions)
├── providers/               # Context providers (AppData, Theme, Window, Loading, UpdateState)
└── locales/                 # i18n translation files

src-tauri/                   # Rust backend (Tauri application)
├── src/
│   ├── cmd/                 # IPC command handlers (27 modules, exposed to frontend)
│   ├── config/              # Config singletons: clash.rs, profiles.rs, verge.rs, runtime.rs
│   ├── core/                # Core services: service.rs, hotkey.rs, timer.rs, tray/, handle.rs
│   ├── feat/                # Feature implementations: profile.rs, backup.rs, clash.rs, window.rs
│   ├── enhance/             # Config enhancement pipeline: merge.rs, script.rs, chain.rs, tun.rs
│   ├── module/              # Modules: auto_backup, lightweight mode
│   └── utils/               # Utilities: dirs.rs, resolve/, server.rs
└── tauri.conf.json

crates/                      # Rust workspace members
├── clash-verge-draft/       # Draft<T> state management (Arc + RwLock, optimistic updates)
├── clash-verge-logging/     # Logging infrastructure (logging! macro with Type variants)
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
6. Real-time data (traffic, memory, connections, logs) via WebSocket through `tauri-plugin-mihomo`

### Rust State Management (Config + Draft Pattern)

Global config uses a singleton with `tokio::sync::OnceCell`, accessed via `Config::global().await`:

```rust
// Access config subsystems (each returns a Draft<T> wrapper)
let verge = Config::verge().await;     // App settings (IVerge)
let clash = Config::clash().await;     // Clash core config (IClashTemp)
let profiles = Config::profiles().await; // Profile list (IProfiles)
let runtime = Config::runtime().await;  // Runtime state (IRuntime)
```

The `Draft<T>` system (in `crates/clash-verge-draft/`) provides optimistic state:

- `latest_arc()` — get current state as `Arc<T>` (zero-copy read)
- `edit_draft(|data| { ... })` — modify a draft copy
- `apply()` — commit draft as official state
- `discard()` — rollback draft changes

### Adding a New Tauri Command (End-to-End)

1. **Rust handler** in `src-tauri/src/cmd/<module>.rs`:
   ```rust
   #[tauri::command]
   pub async fn my_command(arg: String) -> CmdResult<MyResponse> {
       // CmdResult<T> = Result<T, String>
       // Use .stringify_err() to convert anyhow errors
       do_something(arg).stringify_err()
   }
   ```
2. **Register** in `src-tauri/src/lib.rs` inside `generate_handlers![]` macro
3. **Frontend wrapper** in `src/services/cmds.ts`:
   ```typescript
   export async function myCommand(arg: string) {
     return invoke<MyResponse>("my_command", { arg });
   }
   ```

### Frontend Data Fetching

- **SWR** for all data fetching — never fetch directly; always use hooks
- **AppDataProvider** (`src/providers/app-data-provider.tsx`) manages global state via SWR
- **useMihomoWsSubscription** for WebSocket data (traffic, memory, connections, logs) with date-based cache keys
- **Handle events** from backend (RefreshClash, ProfileChanged, etc.) trigger SWR cache mutations
- React Compiler is enabled — `exhaustive-deps` and `rules-of-hooks` are enforced as errors

### Configuration Enhancement Pipeline

Processing order when `enhance_profiles()` is called:

1. Global merge → Global script
2. Profile-specific: rules → proxies → groups → merge → script
3. Builtin scripts (core-version-specific: Stable/Alpha/Smart)
4. Proxy group cleanup (remove invalid references)
5. TUN/DNS settings applied

Enhancement types: Merge (YAML overlay), Script (JS via Boa engine), Rules/Proxies/Groups (sequence operations)

### Platform-Specific Code

- Windows: `deelevate` for privilege elevation, WinAPI integration, UWP handling
- macOS: Autostart launcher, network interface prioritization
- Linux: DBus integration, webkit dependencies

### Backend Event System

`Handle` singleton (`src-tauri/src/core/handle.rs`) dispatches events to frontend:

- `Handle::send_event(FrontendEvent::RefreshClash)` — triggers frontend SWR refresh
- Events: RefreshClash, RefreshVerge, ProfileChanged, ProfileUpdateStarted/Completed, TimerUpdated

## Important Conventions

### Rust

- Workspace uses strict Clippy lints (see `Cargo.toml` [workspace.lints.clippy])
- Avoid `unwrap()` and `expect()` — they trigger warnings; use `anyhow` + `stringify_err()`
- `panic!()` and `unimplemented!()` are denied; `todo!()` is a warning
- Cognitive complexity threshold: 25
- Edition 2024, Rust 1.91+
- Use `logging!()` macro with Type variants (Core, Cmd, Config, etc.) for logging
- `wildcard_imports` denied — always use explicit imports

### Frontend

- React 19 with React Compiler — `react-compiler/react-compiler` lint rule is an error
- Material UI 7 for components
- SWR for data fetching/caching
- Monaco Editor for config editing
- `@typescript-eslint/no-explicit-any` is off (any is allowed)
- Unused imports are auto-removed (`unused-imports/no-unused-imports: error`)
- Import order enforced: builtin → external → internal → parent → sibling

### Package Manager

- pnpm 10.28.0 (enforced via corepack)
